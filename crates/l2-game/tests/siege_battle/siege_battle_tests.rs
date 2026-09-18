#![allow(unused_imports)]
use super::*;

use l2_game::game::Assets;
use l2_game::input::{Event, Key, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{battle, map};
use l2_game::Game;
use l2_kingdom::map::{flags, terrain, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;
use l2_game::battlefield as bf;

/// `Battle_ChooseSettlement` (`0x004A6A30`) is two `if`s and the second
/// overrides the first:
///
/// so a human defender attacked by an AI holds the choice himself, and only two
/// humans put it in the other man's hands. Ours tested the *defender's*
/// `ownerIsHuman` where the original tests the attacker's, handed the defender
/// a 2, and `BattlePromptScreen` draws no widgets under 2 — because
/// `Battle_ChooseSettlement` writes `DAT_00554408 = 2` only under 1. In the
/// original a bystander's prompt waits for the multiplayer answer timeout;
/// single player has no such timeout.
#[test]
fn a_human_besieged_by_an_ai_holds_the_choice_and_the_turn_can_end() {
    let (mut g, a, mut m) = on_the_map();
    let (keep, _garrison) = castle_with_garrison(&mut g, &mut m, &a, 1, 2, 200);

    let camp = (keep.0 - 1, keep.1 + 1);
    let besieger = army_at(&mut g, 2, 2, 600, camp);
    lay_siege(&mut m, &mut g, &a, besieger, keep, 1);
    end_turns_until_the_assault(&mut m, &mut g, &a);

    let q = l2_game::turn::pending_question(&g).expect("a question");
    assert!(q.is_siege);
    assert_eq!(
        q.choice_owner, 1,
        "an AI attacker leaves the choice with the human defender -- and without \
         this the prompt has no widgets and the turn never ends",
    );

    take_the_field(&mut m, &mut g, &a);
    assert_eq!(g.battle.as_ref().map(|b| b.owner), Some(1));
    for _ in 0..500 {
        tick(&mut m, &mut g, &a);
    }
    {
        let live = g.battle.as_ref().expect("a live battle");
        assert!(live.is_siege());
        assert!(live.runner.tick > 0, "the frames are reaching the simulation");
    }

    // `FUN_0043BA29` opens `Ui_OpenConfirm(11)` — *"Surrender castle?"* —
    // rather than the field battle's *"Retreat from field?"*, and both
    // callbacks reach `FUN_0043BE65`.
    assert_eq!(
        g.battle.as_mut().expect("a live battle").press_retreat(),
        Some(11),
        "a garrison surrenders its castle; it does not retreat from a field",
    );
    click(&mut m, &mut g, &a, on(bf::Button::Retreat.rect()));
    click(&mut m, &mut g, &a, on(l2_game::screens::battlefield::CONFIRM_YES));
    run_until(&mut m, &mut g, &a, "the result screen", |m, _| {
        m.top_id() == Some(ScreenId::BattleResult)
    });
    click(&mut m, &mut g, &a, on(battle::ok_rect()));
    // **Or the game is over, which in this two-county world it is.** The human
    // surrenders his only castle, `County_ChangeOwner` recounts realm 1 at zero
    // strength, and `Realm_RecountStrength` enqueues `L2.eng` group 224
    // *"Defeat!"*. `Msg_Pump` shows that on the campaign map the moment the map
    // is on top — **mid-turn, because the pump runs every frame** — and
    // `Msg_Dismiss` reads `DAT_0053F0C4` and enters screen `0x1C`.
    run_until(&mut m, &mut g, &a, "the rest of the turn", |m, g| {
        !l2_game::turn::turn_in_flight(g) || m.top_id() == Some(ScreenId::Conquest)
    });
    assert!(
        matches!(m.top_id(), Some(ScreenId::Campaign) | Some(ScreenId::Conquest)),
        "and the campaign is back: {:?}",
        m.top_id(),
    );
}


/// `FUN_0043BBE7` → `FUN_00496B9F`. The four guards are the button's and were
/// already built; the routine underneath was a once-per-battle latch that did
/// nothing at all. Now it lays the 7 × 4 patch, opens the gate, moves both siege
/// scores and rebuilds the pathfinding, and the way out of the castle it leaves
/// behind is a thing men can walk through.
///
/// The battle is ended with the **autocalc button**, and
/// that is a second assertion: `FUN_0043BE65` leaves for
/// the report without passing through the outcome banner's frame counter, which
/// is `Siege_RecordCastleDamage`'s only caller — so giving up on a battle
/// un-does the damage exactly as it un-does the casualties.
#[test]
fn the_garrison_lowers_the_drawbridge_and_the_besieger_sees_the_gate_open() {
    let (mut g, a, mut m) = on_the_map();
    let (keep, _garrison) = castle_with_garrison(&mut g, &mut m, &a, 1, 4, 400);

    let camp = (keep.0 - 1, keep.1 + 1);
    let besieger = army_at(&mut g, 2, 2, 600, camp);
    lay_siege(&mut m, &mut g, &a, besieger, keep, 1);
    end_turns_until_the_assault(&mut m, &mut g, &a);
    assert_eq!(
        l2_game::turn::pending_question(&g).map(|q| q.castle_level),
        Some(Some(3)),
        "a stone castle",
    );

    take_the_field(&mut m, &mut g, &a);

    // **`t32_stn1.256`, not the field's.** `Screen_DrawBattlefield`
    // (`0x004233F7`) ends every repaint with `if (g_battleIsSiege == 0)
    // Palette_Set(0x568ee0); else Palette_Set(0x5675a0);`, and those two
    // buffers are records 2 and 1 of `g_preloadTable` (`0x004D9F48`) —
    // `t32_bat1.256` and `t32_stn1.256`, spelled in the table's own bytes.
    tick(&mut m, &mut g, &a);
    assert_eq!(
        m.palette_name(),
        Some("T32_stn1.256"),
        "a siege runs under the field battle's palette",
    );

    {
        let live = g.battle.as_ref().expect("a live battle");
        assert!(live.runner.has_drawbridge(), "a stone castle has one");
        assert!(!live.runner.siege.gate_breached, "and it is shut");
        assert!(!live.sallied);
    }

    click(&mut m, &mut g, &a, on(bf::Button::Sally.rect()));
    {
        let live = g.battle.as_ref().expect("a live battle");
        assert!(live.sallied, "the latch, DAT_0052AF9C");
        assert!(live.runner.siege.drawbridge_down);
        assert!(
            live.runner.siege.gate_breached,
            "_DAT_00569588 -- the besieger AI now reads an open gate, which is \
             what lowering your own drawbridge costs you",
        );
        assert!(
            !live.runner.has_drawbridge(),
            "every cell of the patch had its 0x40 cleared, so the way is open",
        );
        let walkable = live
            .runner
            .field
            .cells
            .iter()
            .filter(|c| c.surface == l2_sim::siege::SURFACE_GROUND && !c.impassable())
            .count();
        assert!(walkable > 0, "and what is left is ground men can stand on");
    }
    // A second press is refused with `L2.eng` 157, *"Drawbridge is down."*
    assert_eq!(g.battle.as_mut().expect("a live battle").press_sally(true), Err(0x9D));

    click(&mut m, &mut g, &a, on(bf::Button::Autocalc.rect()));
    click(&mut m, &mut g, &a, on(l2_game::screens::battlefield::CONFIRM_YES));
    run_until(&mut m, &mut g, &a, "the result screen", |m, _| {
        m.top_id() == Some(ScreenId::BattleResult)
    });
    let report = l2_game::turn::pending_report(&g).expect("a settled battle");
    assert_eq!(report.resolution, l2_game::engagement::Resolution::Autocalc);
    assert_eq!(
        g.kingdom.counties[1].castle_degraded, 0,
        "a battle nobody watched to its end bills no repair, however much of it \
         was fought first -- Siege_RecordCastleDamage has one caller and it is \
         the outcome banner frame counter",
    );
}


/// `Siege_RecordCastleDamage` (`0x004784CA`) is the only writer of
/// [`l2_kingdom::siege::CASTLE_DEGRADED_DAMAGED`] in the binary, and its three
/// readers were reachable only from their own tests until this route existed.
///
/// `Formation_RectIsClear` then caches *"this unit was ordered onto water"*,
/// every figure enters state 9, and `FUN_0047DD86` counts the cells they fill.
#[test]
fn the_player_besieges_watches_fills_the_ditch_and_the_castle_is_billed_for_it() {
    let (mut g, a, mut m) = on_the_map();
    let (keep, garrison) = castle_with_garrison(&mut g, &mut m, &a, 2, 3, 200);

    let camp = (keep.0 - 1, keep.1 + 1);
    let besieger = army_at(&mut g, 1, 1, 120, camp);
    click(&mut m, &mut g, &a, pixel(camp.0, camp.1).unwrap());
    click(&mut m, &mut g, &a, pixel(keep.0, keep.1).unwrap());
    run_until(&mut m, &mut g, &a, "the siege", |_, g| {
        g.kingdom.campaign.units.get(besieger).is_some_and(|u| u.besieging_county == 2)
    });

    end_turns_until_the_assault(&mut m, &mut g, &a);
    let q = l2_game::turn::pending_question(&g).expect("a question");
    assert!(q.is_siege, "the prompt knows it is a siege");
    assert_eq!(q.castle_level, Some(2), "and which castle it is");
    assert_eq!(q.choice_owner, 1, "the besieger is the human, so he chooses");

    take_the_field(&mut m, &mut g, &a);
    {
        let live = g.battle.as_ref().expect("a live battle");
        assert!(live.is_siege());
        assert_eq!(live.castle_level, Some(2));
        assert!(live.runner.siege.is_siege);
        assert!(
            live.runner.field.cells.iter().any(|c| c.flags & l2_sim::siege::FLAG_WALL != 0),
            "there is a wall to fight at",
        );
    }

    let (home, ditch) = {
        let live = g.battle.as_ref().expect("a live battle");
        let home = live.runner.home(l2_sim::SIDE_B);
        let dim = l2_sim::terrain::DIM;
        let ditch = live
            .runner
            .field
            .cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.surface == l2_sim::siege::SURFACE_WATER)
            .map(|(i, _)| ((i % dim) as u8, (i / dim) as u8))
            .min_by_key(|&(x, y)| {
                (x as i32 - home.0 as i32).abs() + (y as i32 - home.1 as i32).abs()
            })
            .expect("a Norman keep has a ditch");
        (home, ditch)
    };

    let look = overview_px(home);
    send(&mut m, &mut g, &a, Event::RightClick { x: look.0, y: look.1 });
    box_select_the_viewport(&mut m, &mut g, &a);
    assert!(
        g.battle.as_ref().is_some_and(|b| b.runner.selected_count(1) > 0),
        "the box picked nobody up",
    );
    click(&mut m, &mut g, &a, overview_px(ditch));

    let mut n = 0;
    while n < 40_000 {
        let done = g
            .battle
            .as_ref()
            .is_none_or(|b| b.conclusion.is_some() || b.runner.siege.moat_filled > 0);
        if done {
            break;
        }
        tick(&mut m, &mut g, &a);
        n += 1;
    }
    let filled = g.battle.as_ref().map(|b| b.runner.siege.moat_filled).unwrap_or(0);
    assert!(filled > 0, "the ditch was never filled in {n} frames");
    eprintln!("the ditch started going in after {n} frames");

    press_the_assault_home(&mut m, &mut g, &a);
    watch_to_the_end(&mut m, &mut g, &a, 400_000);

    let report = l2_game::turn::pending_report(&g).expect("a settled battle").clone();
    assert!(report.is_siege);
    assert!(
        matches!(report.resolution, l2_game::engagement::Resolution::Fought { .. }),
        "it was fought rather than calculated: {:?}",
        report.resolution,
    );
    assert!(report.castle_damage.any(), "the accumulators reached the report");
    let moat = report.castle_damage.moat_filled as i32;
    let wall = report.castle_damage.wall_damage as i32;
    assert!(moat > 0);

    let c = &g.kingdom.counties[2];
    assert_eq!(
        c.castle_degraded,
        l2_kingdom::siege::CASTLE_DEGRADED_DAMAGED,
        "the value three readers could not be reached through",
    );
    assert_eq!(c.castle_level_left, 2, "a Norman keep is what is left standing");
    assert_eq!(c.castle_percent, 0, "and the map tile goes back to scaffolding");
    assert_eq!(
        c.castle_work_left,
        moat * l2_kingdom::siege::REPAIR_WORK_PER_MOAT
            + wall * l2_kingdom::siege::REPAIR_WORK_PER_WALL,
    );
    assert_eq!(c.castle_work_total, c.castle_work_left);
    assert_eq!(
        c.castle_stone_owed,
        wall * l2_kingdom::siege::REPAIR_STONE_PER_WALL,
        "a Norman keep is level 2, so it is repaired in stone",
    );
    assert_eq!(c.castle_wood_owed, 0, "and never in wood");
    // The scars are stored, so the next assault on the same castle picks them
// up — `FUN_004787A4`, which is what makes them state.
    assert_eq!(c.siege_scars.moat_filled as i32, moat);
    assert_eq!(c.siege_scars.wall_damage as i32, wall);

    click(&mut m, &mut g, &a, on(battle::ok_rect()));
    run_until(&mut m, &mut g, &a, "the rest of the turn", |_, g| {
        !l2_game::turn::turn_in_flight(g)
    });
    for id in [besieger, garrison] {
        if let Some(u) = g.kingdom.campaign.units.get(id) {
            assert_eq!(u.besieging_county, 0);
            assert_eq!(u.besieged_by, 0);
        }
    }
}

