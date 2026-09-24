//! Two reports from one game, 2026-09-18: *"the AI seems to have just made 3
//! armies and they're just sitting"* and *"the knight didn't message me back"*.

use l2_kingdom::tables::Tables;
use l2_kingdom::{map, UnitKind};

/// **An AI army leaves its home county.** `FUN_0049F977` (AI step 9) is the
/// only path that raises one and points it at somebody, and its guards set the
/// earliest turn: realm `+0x45` counts up to the lord's `+0x28`
/// ([`l2_kingdom::tables::AI_PERSONALITY_MUSTER_PATIENCE`], 2…4 turns), the
/// muster county's population must clear `+0x20`
/// ([`l2_kingdom::tables::AI_PERSONALITY_HELP_POPULATION_FLOOR`], 750…1000)
/// and the realm's weapon stock `+0x138` must clear `+0x68`
/// ([`l2_kingdom::tables::AI_PERSONALITY_MUSTER_ARMS`], 100…250). Sixteen
/// turns is four years of England for the first of the four lords to clear all
/// three; measured, it is turn 13.
#[test]
fn an_ai_army_leaves_its_home_county() {
    let save = l2_testkit::england!();
    let mut game =
        l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    let mut left = None;
    for turn in 1..=16 {
        l2_game::turn::end_turn(&mut game).expect("the turn machine asked nothing");
        let k = &game.kingdom;
        if let Some((id, u)) = k.campaign.units.iter().find(|(_, u)| {
            u.kind == UnitKind::Army
                && !k.realms[u.owner as usize].is_human
                && u.county != 0
                && u.county != u.home_county
        }) {
            left = Some((turn, id, u.owner, u.home_county, u.county));
            break;
        }
    }
    let Some((turn, id, owner, home, county)) = left else {
        panic!(
            "sixteen turns of England and every AI army is still standing in the county it was \
             raised in — `Ai_AdvanceArmies` (`0x004A5667`) moved nobody"
        );
    };
    assert!(turn <= 16, "unit {id} of realm {owner} left {home} for {county} on turn {turn}");
}

/// **A garrison is not aimed at the county town.** `Ai_FindCastleBuildingTile`
/// (`0x004A65A3`) falls back to county `+0x74`/`+0x75` — the plot corner —
/// when no tile carries castle terrain; ours fell back to the county anchor,
/// which is the *town*, and `movement::step`'s [`Entry::Castle`] arm ends a
/// move on the town without entering it. An army aimed there re-aims at the
/// same tile every turn for ever, which is the three armies the person
/// watched. Ablate the `Aim::Castle` fallback in
/// `crates/l2-kingdom/src/ai_army/aim.rs` and this goes red.
#[test]
fn a_garrison_is_not_aimed_at_the_county_town() {
    let save = l2_testkit::england!();
    let game = l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    // County 2 is unowned on this map, so its 2×2 is the bare `0x14` plot.
    let county = 2u8;
    let k = &game.kingdom;
    let plot = map::castle_tile(&k.campaign.map, county).expect("the county has a plot");
    let anchor = (k.counties[county as usize].anchor_x, k.counties[county as usize].anchor_y);
    assert!(
        !(0x15..=0x19).contains(&k.campaign.map.terrain[plot]),
        "county {county} already carries a built castle, so this proves nothing"
    );

    let aimed = l2_kingdom::ai_army::aim_tile(
        &k.campaign.map,
        &k.counties,
        anchor,
        county,
        l2_kingdom::ai_army::Aim::Castle,
    );
    assert_eq!(
        aimed,
        ((plot % map::MAP_DIM) as u8, (plot / map::MAP_DIM) as u8),
        "aimed at {aimed:?}; the anchor is {anchor:?}"
    );
}

/// **A letter gets its reply.** `Diplo_Post` (`0x004A2621`) drops it in the
/// recipient's inbox at the click; `Diplo_AnswerInbox` (`0x004A277D`), AI step
/// 1, answers it on that realm's next turn and ends each answer on
/// `Msg_Enqueue` (`0x00472BC5`) — so the reply is one turn behind the send.
///
/// The turn is driven the interactive way on purpose: `end_turn`'s headless
/// arm runs `message::drain`, which empties the ring nobody is watching.
///
/// The ablation is one line: drop the `letters.extend(…)` in
/// `crates/l2-game/src/turn/ai_part.rs`'s `AiStep::Diplomacy` arm and the
/// standing still moves, the inbox still empties, and the person is told
/// nothing.
#[test]
fn a_letter_to_a_lord_is_answered_on_his_next_turn() {
    let save = l2_testkit::england!();
    let mut game =
        l2_game::scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    let player = game.player;
    let them = (1..l2_kingdom::MAX_REALMS as u8)
        .find(|&r| r != player && game.kingdom.realms[r as usize].in_play)
        .expect("a lord to write to");
    game.kingdom.post_letter(player, them, l2_kingdom::diplomacy::Kind::Compliment, 0, 0);
    assert!(
        game.kingdom.realms[them as usize].pair(player).has_mail,
        "`Diplo_Post` did not reach realm {them}'s inbox"
    );

    let mut step = l2_game::turn::begin_turn(&mut game);
    let mut ticks = 0u32;
    loop {
        match step {
            l2_game::turn::TurnStep::Done(_) => break,
            l2_game::turn::TurnStep::Running => {}
            other => panic!("nobody is at war on turn one: {other:?}"),
        }
        ticks += 1;
        assert!(ticks < l2_game::turn::MAX_TICKS, "the machine never came round");
        step = l2_game::turn::tick_turn(&mut game);
    }

    let reply = game
        .messages
        .waiting()
        .into_iter()
        .chain(game.messages.open().copied())
        .find(|r| r.from == them && r.to == player)
        .unwrap_or_else(|| {
            panic!(
                "realm {them} answered the compliment and the ring is empty — \
                 `Diplo_ReplyCompliment` (`0x004A2CD6`)'s letter never reached `Msg_Enqueue`"
            )
        });
    assert!(
        (l2_kingdom::diplomacy::group::COMPLIMENT_FIRST
            ..=l2_kingdom::diplomacy::group::COMPLIMENT_ENOUGH)
            .contains(&reply.group),
        "the reply is group {}, not one of 174/175/176",
        reply.group
    );
    assert!(
        !game.kingdom.realms[them as usize].pair(player).has_mail,
        "`Diplo_AnswerInbox` clears the inbox and the six mail flags every turn"
    );
}

/// `AI_Taunt` (`0x004A13A6`) stage 0 at a timer of 7: `Msg_Enqueue(realm, human,
/// 0xC1, …, '\x01', …)`, decomp 004a0000.c:502. Ablate: drop the
/// `letters.extend` in `ai_part.rs`'s `AiStep::Taunt` arm.
#[test]
fn a_winning_lord_taunts_the_person() {
    let mut game = l2_game::game::Game::new(0x7A);
    for realm in 1..=2usize {
        let r = &mut game.kingdom.realms[realm];
        r.in_play = true;
        r.strength = 3;
    }
    game.kingdom.realms[1].is_human = true;
    let r = &mut game.kingdom.realms[2];
    r.rank = 1;
    r.share_of_map_pct = 40;
    r.taunt_timer = 7;
    r.lord = 1;
    r.ai_step = l2_kingdom::ai::AiStep::Taunt.counter();
    // AI_RunTurnStep 0x0049A581 steps the realm at DAT_0057C944 alone, the human's turn included [V].
    game.kingdom.turn.ai_cursor = 2;
    let mut granted = true;
    let mut letters = Vec::new();
    l2_game::turn::drive_ai(&mut game.kingdom, &mut granted, &mut letters);
    let taunt = letters
        .iter()
        .find(|l| l.group == 0xC1)
        .expect("`AI_Taunt`'s group-193 letter reached no ring");
    assert_eq!((taunt.from, taunt.to, taunt.category), (2, 1, 1));
    assert_eq!(taunt.variant, 0, "lord 1 * 4 + rotation 0 - 4");
}
