#![allow(unused_imports)]
use super::*;
use super::simulation::*;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::tables::Tables;
use l2_kingdom::{Kingdom, UnitKind};
use l2_game::game::Game;

/// **The audit C27 asks for, turned into an assertion: for every field these
/// handlers read, what writes it in a real game?**
#[test]
fn a_played_game_writes_the_four_fields_the_war_handlers_read() {
    let mut game = six_county_world();
    assert_eq!(game.kingdom.realms[1].pair(2).standing, 0, "realm 1 is the person");
    assert_eq!(game.kingdom.realms[2].pair(1).standing, 5);
    assert_eq!(game.kingdom.realms[2].pair(2).standing, 5);

    let mut ever_allied = 0;
    let mut first_alliance = None;
    for turn in 1..=TURNS {
        l2_game::turn::end_turn(&mut game).expect("the machine comes round");
        let standing = (1..=5).filter(|&r| game.kingdom.realms[r].ally != 0).count();
        if standing > ever_allied {
            ever_allied = standing;
            first_alliance.get_or_insert(turn);
        }
    }

    let mut opinions = 0;
    let mut hostile = 0;
    let mut at_war = 0;
    let mut war_targets = 0;
    let mut allies = 0;
    for realm in 1..=5 {
        let r = &game.kingdom.realms[realm];
        if r.war_target != 0 {
            war_targets += 1;
        }
        if r.ally != 0 {
            allies += 1;
        }
        for other in 1..l2_kingdom::MAX_REALMS {
            let p = r.pair(other as u8);
            if p.standing != 0 {
                opinions += 1;
            }
            if p.standing < -10 {
                hostile += 1;
            }
            if p.at_war {
                at_war += 1;
            }
        }
    }
    eprintln!(
        "after {TURNS} turns: {opinions} non-zero standings, {hostile} below −10, \
         {at_war} at war, {war_targets} war targets, {allies} realms allied"
    );
    for realm in 1..=5 {
        let r = &game.kingdom.realms[realm];
        let row: Vec<String> = (1..l2_kingdom::MAX_REALMS)
            .map(|o| {
                let p = r.pair(o as u8);
                format!(
                    "{:>4}{}{}",
                    p.standing,
                    if p.allied { "A" } else { " " },
                    if p.at_war { "W" } else { " " }
                )
            })
            .collect();
        eprintln!(
            "  realm {realm} ally={} warTarget={} | {}",
            r.ally,
            r.war_target,
            row.join(" ")
        );
    }

    assert!(opinions > 0, "forty turns and nobody has an opinion of anybody");

    // **These two assertions are about step 2 specifically, and the weaker ones
    // they replaced were not.** The first draft asserted "somebody has a war
    // target, an ally, or an opinion below −10" — and it **passed with the step
    // 2 dispatch deleted**, because the battle hook writes standings on its
    // own. `docs/agents.md`: *ablate the thing the test is about, and watch it
    // go red.* This version does.
    let healed = (1..l2_kingdom::MAX_REALMS).any(|r| {
        (1..l2_kingdom::MAX_REALMS)
            .any(|o| game.kingdom.realms[r].pair(o as u8).standing > 5)
    });
    assert!(
        healed,
        "no standing anywhere is above `Diplo_Init`'s opening 5 — `AI_Diplomacy`'s \
         heal is the only thing that can raise one without a letter, so step 2 is \
         not being dispatched"
    );
    eprintln!(
        "  {allies} realms allied at the end; the most ever standing at once was \
         {ever_allied}, first on turn {first_alliance:?}"
    );
    assert!(
        ever_allied > 0,
        "forty turns and not one alliance ever formed — `AI_Diplomacy`'s courtship \
         is the only way two AI realms can reach one, so step 2 is not being \
         dispatched"
    );
    assert!(
        hostile > 0 || war_targets > 0,
        "forty turns of war and nobody resents anybody — `Diplo_Offend` is not \
         reaching the battle and trample seams"
    );

    let mut game = six_county_world();
    l2_game::turn::end_turn(&mut game).expect("one turn to settle the muster county");
    game.kingdom.realms[2].pair_mut(3).standing = -20;
    game.kingdom.realms[2].raid_timer = 0;
    let before = game.kingdom.campaign.units.len();
    let raider = game.kingdom.run_ai_raid(2);
    assert!(raider.is_some(), "a rival at −20 is a rival worth raiding");
    assert_eq!(game.kingdom.campaign.units.len(), before + 1);
    let u = game.kingdom.campaign.units.get(raider.unwrap()).expect("just raised");
    assert_eq!(u.mission, l2_kingdom::ai_army::Mission::RAID);
    assert_eq!(
        game.kingdom.counties[u.dest_county as usize].owner, 3,
        "and it is aimed at the realm it thinks worst of"
    );
    // `FUN_004A5003` opens no armoury: a raiding party carries nothing.
    assert_eq!(u.troops.iter().sum::<i32>(), u.troops[0], "peasants and nothing else");
}

#[test]
fn a_letter_posted_by_a_person_is_answered_on_the_ai_s_next_turn() {
    let mut game = six_county_world();
    assert_eq!(game.kingdom.realms[2].lord, 1, "realm 2 is the Knight");
    let purse = game.kingdom.realms[1].gold;
    let theirs = game.kingdom.realms[2].gold;

    game.kingdom.post_letter(1, 2, l2_kingdom::DiploKind::Gift, 100, 0);
    assert_eq!(game.kingdom.realms[1].gold, purse - 100, "spent at the post office");
    assert_eq!(game.kingdom.realms[2].gold, theirs + 100);
    assert!(game.kingdom.realms[2].pair(1).has_mail);
    assert_eq!(game.kingdom.diplomacy.pending(2).count(), 1);

    let before = game.kingdom.realms[2].pair(1).standing;
    l2_game::turn::end_turn(&mut game).expect("the machine comes round");

    assert_eq!(game.kingdom.diplomacy.pending(2).count(), 0, "the inbox is emptied every turn");
    assert!(!game.kingdom.realms[2].pair(1).has_mail);
    assert_eq!(game.kingdom.realms[2].pair(1).best_gift, 100, "and the bar has ratcheted");
    assert_eq!(
        game.kingdom.realms[2].pair(1).standing,
        before + 10,
        "ten for a top-tier gift, and not eleven: `AI_Diplomacy` never heals \
         towards a person"
    );
}

/// `Ai_ManageCountyFarms` (`0x0049DD01`) *writes* county `+0x1FE` from the
/// **lord's** personality record on every pass and dispatches on what it just
/// wrote. So an AI-owned county's stored style is an output, not an input, and
/// a game loaded with the field at zero reaches the lord's real style on its
/// first turn. The field is a genuine input in exactly one place —
/// `AI_ManageFields(0)`, the pass over the counties **nobody owns** — which is
/// where a missing import does change behaviour.
#[test]
fn an_ai_owned_countys_farm_style_comes_from_its_lord_and_not_from_the_county() {
    let mut game = six_county_world();
    for id in 1..=6 {
        game.kingdom.counties[id].farm_style = 7;
    }
    l2_game::turn::end_turn(&mut game).expect("the machine comes round");
    for realm in 2..=5u8 {
        let lord = game.kingdom.realms[realm as usize].lord;
        let expected = Tables::DEFAULT.ai_farm_style(lord).expect("a lord with a record");
        for id in 1..=6 {
            if game.kingdom.counties[id].owner != realm {
                continue;
            }
            assert_eq!(
                game.kingdom.counties[id].farm_style, expected,
                "county {id} kept its own byte instead of taking realm {realm}'s lord's"
            );
        }
    }
    assert_eq!(game.kingdom.counties[1].farm_style, 7, "the person's county is left alone");
}

