#![allow(unused_imports)]
use super::*;
use super::hires_and_sieges::*;
use super::prompts_and_settling::*;
use super::battle_ai::*;
use super::tactical_controls::*;
use super::*;
use super::raising::*;
use super::marching::*;
use super::division::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::battlefield as bf;
use l2_game::screens::{armoury, army, battle, divide, info, map};
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

/// Put the player's army one tile from **county 2's town** — plane-0 `0x40`,
///
/// (`0x004A6C68`) is reached from — and order the march. No turn is ended.
fn a_town_is_about_to_be_attacked() -> (Game, Assets, Machine, usize) {
    let (mut g, a, m) = on_the_map();
    let (mine, town) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    g.kingdom.campaign.map.set_flags(town.0, town.1, flags::CASTLE);
    g.kingdom.counties[2].anchor_x = town.0;
    g.kingdom.counties[2].anchor_y = town.1;
    let attacker = army_at(&mut g, 1, 1, 400, mine);
    g.order_unit_move(attacker, town).expect("a path one tile long");
    (g, a, m, attacker)
}

/// **The report: a town attacked from the map resolved with no prompt.**
///
/// `Battle_ChooseSettlement` (`0x004A6A30`) does not care which frame the army
/// arrived on — an army whose owner is a person is `Settlement::Prompt` and
/// screen `0x12` goes up. Ours only asked when the arrival happened inside the
/// turn machine.
#[test]
fn a_town_attacked_while_the_player_watches_asks_before_it_resolves() {
    let (mut g, a, mut m, attacker) = a_town_is_about_to_be_attacked();
    let before = g.kingdom.turn_count;

    run_until(&mut m, &mut g, &a, "the battle prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });

    assert_eq!(g.kingdom.turn_count, before, "no turn was ended: he was watching");
    assert!(
        g.kingdom.campaign.units.get(attacker).is_some(),
        "nothing may be resolved while the question is on the table",
    );
    let q = l2_game::turn::pending_question(&g).expect("a question");
    assert_eq!(q.choice_owner, 1, "the local player holds the choice");
    assert_eq!(q.attacker_men, 400);
    assert_eq!(q.county, 2, "the county fought over is the defender's");
    assert!(!q.is_siege);
}

/// The same door, the other way in: two armies meeting on an ordinary frame —
/// `Unit_EnterOccupiedTile` (`0x004658C1`).
#[test]
fn an_army_walked_into_while_the_player_watches_asks_too() {
    let (mut g, a, mut m, attacker, defender) = a_battle_is_about_to_happen();
    run_until(&mut m, &mut g, &a, "the battle prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });
    assert!(
        g.kingdom.campaign.units.get(attacker).is_some()
            && g.kingdom.campaign.units.get(defender).is_some(),
        "neither army may be touched while the question is on the table",
    );
}

/// **And the answer settles it and hands the map back**,
/// having been started. This is the half that would break if the suspension
/// were done by faking a turn: declining runs the autocalc, `0x13` shows the
/// result, and dismissing it must leave the campaign where it was
/// winding a season on.
#[test]
fn answering_a_watched_battle_settles_it_and_does_not_end_the_turn() {
    let (mut g, a, mut m, attacker) = a_town_is_about_to_be_attacked();
    let before = g.kingdom.turn_count;
    run_until(&mut m, &mut g, &a, "the battle prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });

    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::DECLINE)));
    assert_eq!(m.top_id(), Some(ScreenId::BattleResult), "the result screen follows");
    let r = l2_game::turn::pending_report(&g).expect("a settled battle");
    assert_eq!(r.resolution, l2_game::engagement::Resolution::Autocalc);
    assert_eq!(r.attacker_men.0, 400);

    click(&mut m, &mut g, &a, on(battle::ok_rect()));
    run_until(&mut m, &mut g, &a, "the map to come back", |m, g| {
        m.top_id() == Some(ScreenId::Campaign) && !l2_game::turn::turn_in_flight(g)
    });
    assert_eq!(g.kingdom.turn_count, before, "answering a battle is not ending a turn");
    assert!(
        g.kingdom.campaign.units.get(attacker).is_none()
            || g.kingdom.counties[2].owner == 1,
        "the battle was actually fought: he lost the army or he took the county",
    );
}

/// **An AI's battle the player is not in still never opens a screen.**
///
/// `Battle_ChooseSettlement`'s first line — `if (!A.ownerIsHuman &&
/// !B.ownerIsHuman) return 0` —
/// prompt whenever a battle happens on a frame"*. Realm 2 marches on realm 3's
/// town while the human watches,
///
/// **Ablated**: dropping the `Settlement` test in `tick_units_only` and always
/// suspending turns this red.
#[test]
fn two_ai_armies_meeting_while_the_player_watches_open_nothing() {
    let (mut g, a, mut m) = on_the_map();
    g.kingdom.counties[3].owner = 3;
    g.kingdom.realms[3].in_play = true;
    g.kingdom.realms[3].strength = 5;
    let (mine, town) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    // County 2's town, attacked by realm 3 — neither realm is a person's.
    g.kingdom.campaign.map.set_flags(town.0, town.1, flags::CASTLE);
    g.kingdom.counties[2].anchor_x = town.0;
    g.kingdom.counties[2].anchor_y = town.1;
    let attacker = army_at(&mut g, 3, 3, 400, mine);
    // Not `Game::order_unit_move`: that door is the player's own, and this army
    // is not his.
    let map = g.kingdom.campaign.map.clone();
    l2_kingdom::movement::order_move(
        &map,
        &mut g.kingdom.campaign.units,
        attacker,
        town,
        l2_kingdom::movement::Routing::Direct,
    )
    .expect("a path one tile long");

    for _ in 0..8 {
        tick(&mut m, &mut g, &a);
    }
    // **No PROMPT, and nothing suspended.** This used to assert the campaign map
    // was on top, which was the same thing until the message window landed: a
    // battle between two other lords now raises a report, and a scroll over the
// map is the original's own behaviour. The
    // machine, so it is now made about those directly — the earlier form was a
    // stronger assertion than the test's own doc comment claims, and it went red
    // for a correct change.
    assert_ne!(
        m.top_id(),
        Some(ScreenId::BattlePrompt),
        "somebody else's war asked the player to take the field",
    );
    assert_ne!(
        m.top_id(),
        Some(ScreenId::Battlefield),
        "somebody else's war put the player on the field",
    );
    assert!(
        matches!(m.top_id(), Some(ScreenId::Campaign) | Some(ScreenId::Message)),
        "unexpected screen {:?} for somebody else's war",
        m.top_id(),
    );
    assert!(l2_game::turn::pending_question(&g).is_none());
    assert!(l2_game::turn::pending_report(&g).is_none());
    assert!(!l2_game::turn::turn_in_flight(&g), "and nothing left suspended");
}


