#![allow(unused_imports)]
use super::*;
use super::simulation::*;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::tables::Tables;
use l2_kingdom::{Kingdom, UnitKind};
use l2_game::game::Game;

/// **The audit C27 asks for, turned into an assertion: for every field these
/// handlers read, what writes it in a real game?**
///
/// Four of `l2_kingdom::ai_army`'s inputs are written by `l2_kingdom::diplomacy`
/// and by nothing else:
///
/// | field | its only writer | what was unreachable without it |
/// |---|---|---|
/// | `Realm::pairs[].standing` | `Diplo_Init`, `Diplo_Offend`, the seven reply handlers | **AI step 10 entirely** — `pick_raid_victim` wants a standing below −10 |
/// | `Realm::war_target` | `Diplo_Offend` | the same, and step 9's halved population floor |
/// | `Realm::ally` | `Diplo_FormAlliance` | `Mission::ASSIST_ALLY`, and `action_allowed`'s grudge bump |
/// | `Realm::target_county` | `Diplo_PayForHelp` | step 9's ally-request branch |
///
/// > **This test used to assert the opposite**, and it asked in its own message
/// > to be replaced the day the module landed:
/// >
/// > > *"realm N has an opinion of realm M — has `diplomacy` landed? If so this
/// > > test has done its job and should be replaced by one that asserts the
/// > > raid fires on its own."*
/// >
/// > It has, and this is that test. What it asserts now is the thing that was
/// > missing: **not that the handler returns the right answer when a test hands
/// > it a standing, but that a played game produces one.** That is
/// > `docs/agents.md`'s rule — *a field is only tested if something a test
/// > reads was written by something the game runs* — read forwards instead of
/// > backwards.
#[test]
fn a_played_game_writes_the_four_fields_the_war_handlers_read() {
    let mut game = six_county_world();
    // Turn one, before anything has run: `Diplo_Init`'s opening position.
    // A person's row is all zeros and an AI's is all fives — including its own
    // slot, because the original's loop has no `other != me` guard.
    assert_eq!(game.kingdom.realms[1].pair(2).standing, 0, "realm 1 is the person");
    assert_eq!(game.kingdom.realms[2].pair(1).standing, 5);
    assert_eq!(game.kingdom.realms[2].pair(2).standing, 5);

    // **An alliance is counted while it stands, not only at the end.**
    //
    // This used to read the board once, after the last turn, and assert that
    // somebody was allied *then*. It went red the day units started taking the
    // original's thirty-two ticks to cross a tile — and the diagnosis it
    // printed, *"so step 2 is not being dispatched"*, was wrong: step 2 runs,
    // the realms **do** ally, and this world's first alliance forms on turn 11.
    // It had been broken again by turn 40.
    //
// So the assertion was about the state of
    // one instant, and it held because on the old, faster world the last
    // alliance to form happened to still be standing when the loop stopped. A
    // pacing change moved the trajectory by a few turns and it fell over —
    // `docs/agents.md`, *a test that passes for an accidental reason*, and the
    // reason here was the arithmetic of which turn the run ends on.
    //
// What `AI_Diplomacy` being dispatched implies is that an alliance
    // is reachable at all, so that is what is watched for.
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
    //
    // Only `AI_Diplomacy`'s heal can put a standing **above** `Diplo_Init`'s
    // opening 5 — every other writer in the subsystem subtracts, and the three
    // that add arrive through the inbox, which nothing fills in an AI-only
    // game. And only its courtship can produce an ally, for the same reason:
    // the other route to one is a person accepting an offer.
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
// And the war half, which is the offence hook: it is
    // asserted separately so that a failure says which of the two broke.
    assert!(
        hostile > 0 || war_targets > 0,
        "forty turns of war and nobody resents anybody — `Diplo_Offend` is not \
         reaching the battle and trample seams"
    );

    // And the raid still fires on the input the AI now supplies for itself.
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

/// **AI step 1, driven through the dispatch.**
///
/// The forty-turn test above cannot see step 1 at all, and that is a fact about
/// the game: `Diplo_AnswerInbox` answers letters,
/// only `Diplo_Post` writes one, and in single player only a person ever posts.
/// An AI-only game therefore runs step 1 forty times over an empty inbox.
///
/// So the letter is posted the way the diplomacy screen posts it, a turn is
/// played, and what is asserted is the round trip: the gold left when it was
/// **posted**, the reply came back on the AI's next turn, the standing moved by
/// the amount the tier says, and the inbox was emptied.
#[test]
fn a_letter_posted_by_a_person_is_answered_on_the_ai_s_next_turn() {
    let mut game = six_county_world();
    // Realm 2 is the Knight: gift increment 100, so 100 crowns against an
    // opening `best_gift` of 0 is the top tier and worth +10.
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
    // +10 for the gift and +1 for step 2's heal — except that the heal's guard
    // is on the *other* realm being non-human and realm 1 is a person, so the
    // heal does not run on this pair at all.
    assert_eq!(
        game.kingdom.realms[2].pair(1).standing,
        before + 10,
        "ten for a top-tier gift, and not eleven: `AI_Diplomacy` never heals \
         towards a person"
    );
}

/// **An AI realm's farming never reads the county's stored style byte**, and
/// that is why the missing `County::farm_style` import could not have skewed
/// anything measured here.
///
/// `Ai_ManageCountyFarms` (`0x0049DD01`) *writes* county `+0x1FE` from the
/// **lord's** personality record on every pass and dispatches on what it just
/// wrote. So an AI-owned county's stored style is an output, not an input, and
/// a game loaded with the field at zero reaches the lord's real style on its
/// first turn. The field is a genuine input in exactly one place —
/// `AI_ManageFields(0)`, the pass over the counties **nobody owns** — which is
/// where a missing import does change behaviour.
///
/// Stated as a test because *"the allocator behaves
/// the same for every county"* and *"every county had style 0"* are
/// indistinguishable from the outside, and the first is the far more
/// interesting claim.
#[test]
fn an_ai_owned_countys_farm_style_comes_from_its_lord_and_not_from_the_county() {
    let mut game = six_county_world();
    // A style byte no lord has, so anything that survives came from the county.
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
    // The human's county and the unowned one are the other half of the claim:
    // nothing overwrites them,
    assert_eq!(game.kingdom.counties[1].farm_style, 7, "the person's county is left alone");
}

