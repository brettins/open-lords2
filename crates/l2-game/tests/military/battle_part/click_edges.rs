//! **The battlefield's two edges**, driven through the real [`Machine`].
//!
//! `Battle_DragSelect` (`0x0043BF07`) is four arms in one function and the
//! player's hand is in all of them: the press opens the drag (`g_screenId`
//! `0x2A`), the held branch re-boxes live, the release commits. The order arm
//! `Battle_OrderClicked` (`0x0043C57D`) sits *behind* it in the `0x29` ladder
//! and takes only `g_mouseLeftReleased`. What these tests hold down is the
//! clause a modern port drops: **the held branch classifies before it touches
//! anything**, and kind 0 — under 25 pixels on both axes, over nobody — does
//! nothing at all.
#![allow(unused_imports)]
use super::*;
use super::tactical_controls::look_at_the_players_men;
use l2_game::battlefield as bf;
use l2_game::input::Event;
use l2_game::screen::{Machine, ScreenId};
use l2_game::screens::battle;
use l2_game::Game;

/// A paused battlefield with the camera over the local player's men.
fn a_paused_battlefield() -> (Game, l2_game::game::Assets, Machine) {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::TAKE_THE_FIELD)));
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield));
    click(&mut m, &mut g, &a, on(bf::Button::Pause.rect()));
    look_at_the_players_men(&mut g);
    (g, a, m)
}

/// The screen pixel one of the player's own men is standing on.
fn a_standing_mans_pixel(g: &Game) -> (i32, i32) {
    let live = g.battle.as_ref().expect("a live battle");
    let i = (0..live.runner.fighters.len())
        .find(|&i| live.runner.is_alive(i) && live.runner.fighters[i].side != l2_sim::SIDE_A)
        .expect("the player has men on the field");
    let f = &live.runner.fighters[i];
    (
        bf::VIEW.x + (f.x as i32 - live.cam.0) * bf::TILE + bf::TILE / 2,
        bf::VIEW.y + (f.y as i32 - live.cam.1) * bf::TILE + bf::TILE / 2,
    )
}

/// Every man's position, destination and state, and every unit's destination.
fn a_snapshot(g: &Game) -> (Vec<(u8, u8, (u8, u8), u8)>, Vec<(i16, i16)>) {
    let live = g.battle.as_ref().expect("a live battle");
    let men = (0..live.runner.fighters.len())
        .map(|i| {
            let f = &live.runner.fighters[i];
            (f.x, f.y, f.target, live.runner.sim.figures[f.sim].state as u8)
        })
        .collect();
    let units = (1..=80usize)
        .map(|u| {
            let u = live.runner.units.get(u);
            (u.target_x, u.target_y)
        })
        .collect();
    (men, units)
}

/// Empty ground inside the viewport, as a pixel and as the cell it is.
fn some_empty_ground(g: &Game) -> (i32, i32, (u8, u8)) {
    let live = g.battle.as_ref().expect("a live battle");
    for row in 0..bf::VIEW_ROWS {
        for col in 0..bf::VIEW_COLS {
            let cell =
                ((live.cam.0 + col).clamp(0, 79) as u8, (live.cam.1 + row).clamp(0, 79) as u8);
            if live.runner.occupant_of(cell.0, cell.1).is_none() {
                return (
                    bf::VIEW.x + col * bf::TILE + 4,
                    bf::VIEW.y + row * bf::TILE + 4,
                    cell,
                );
            }
        }
    }
    panic!("some empty ground in view")
}

/// **A player: *"the troops seem to try to find a formation when I first click
/// them"*.** Selecting is not ordering: `Battle_DragSelect`'s kind-2 release
/// commits a 16-pixel box and nothing else, and `Battle_OrderClicked`
/// (`0x0043C57D`) refuses the release outright while `g_battleHoverFriendly` or
/// `g_battleHoverFriendlyPicked` is set — which is what makes a click on one of
/// your own men a selection and never a destination.
///
/// The men *do* close up twenty frames later:
/// `BattleUnits_RegroupSelection` (`0x00478987`) writes `reTarg = 0x14` on the
/// selected unit and the per-unit tick counts it down into `BattleUnit_Reform`
/// (`0x0048970E`). That is the original's, so this test asserts on the *order*
/// — no man's destination, position or state moves on the click itself.
///
/// **Ablated**: the ladder short-circuit in `Event::Release` and `order_at`'s
/// two hover guards are belt and braces — either alone still refuses, and
/// removing both turns the destination assertion red.
#[test]
fn a_click_on_a_standing_man_selects_him_and_orders_nobody() {
    let (mut g, a, mut m) = a_paused_battlefield();
    let (px, py) = a_standing_mans_pixel(&g);
    let before = a_snapshot(&g);

    send(&mut m, &mut g, &a, Event::Pointer { x: px, y: py });
    send(&mut m, &mut g, &a, Event::Click { x: px, y: py });
    assert_eq!(g.battle.as_ref().map(|b| b.screen_id()), Some(0x2A), "the press opens the drag");
    // A hand shake of three pixels, which is under `FUN_00479CF7`'s 25.
    send(&mut m, &mut g, &a, Event::Pointer { x: px + 3, y: py + 2 });
    send(&mut m, &mut g, &a, Event::Release { x: px + 3, y: py + 2 });
    assert_eq!(g.battle.as_ref().map(|b| b.screen_id()), Some(0x29), "and the release ends it");

    assert!(
        g.battle.as_ref().expect("a live battle").runner.selected_count(1) > 0,
        "the click selected nobody",
    );
    let after = a_snapshot(&g);
    assert_eq!(before.0, after.0, "a man moved, or was re-aimed, on a selection click");
    assert_eq!(before.1, after.1, "a unit was given a destination by a selection click");
}

/// **The mouse down and the mouse up are two different things, and the order is
/// on the up.** `Battle_OrderClicked`'s fourth guard is `g_mouseLeftReleased`;
/// the press before it only opens the drag.
///
/// And the drag in flight must keep its hands off: the held branch of
/// `FUN_0043BF07` runs `Battle_ClassifyDrag` *first* and returns 0 on kind 0, so
/// a shaking hand between the press and the release leaves the selection alone.
/// Ours re-boxed on every pixel of motion, which cleared the selection under the
/// press and left the release with nobody to order — the move order went missing
/// unless the mouse never moved.
///
/// **Ablated**: re-boxing unconditionally in `LiveBattle::drag_to` turns the
/// last assertion red; moving `order_at` onto `Event::Click` turns the middle
/// one red.
#[test]
fn a_click_on_empty_ground_orders_on_the_release_and_not_on_the_press() {
    let (mut g, a, mut m) = a_paused_battlefield();
    // Select some men with a real box first.
    let start = (bf::VIEW.x + 4, bf::VIEW.y + 4);
    let end = (bf::VIEW.x + bf::VIEW.w - 4, bf::VIEW.y + bf::VIEW.h - 4);
    send(&mut m, &mut g, &a, Event::Pointer { x: start.0, y: start.1 });
    send(&mut m, &mut g, &a, Event::Click { x: start.0, y: start.1 });
    send(&mut m, &mut g, &a, Event::Pointer { x: end.0, y: end.1 });
    send(&mut m, &mut g, &a, Event::Release { x: end.0, y: end.1 });
    assert!(
        g.battle.as_ref().expect("a live battle").runner.selected_count(1) > 0,
        "the box picked nobody, so the order proves nothing",
    );

    let (px, py, want) = some_empty_ground(&g);
    let units_before = a_snapshot(&g).1;
    send(&mut m, &mut g, &a, Event::Pointer { x: px, y: py });
    send(&mut m, &mut g, &a, Event::Click { x: px, y: py });
    assert_eq!(a_snapshot(&g).1, units_before, "the press issued the order");

    // The same three-pixel shake, then the release.
    send(&mut m, &mut g, &a, Event::Pointer { x: px + 3, y: py + 2 });
    send(&mut m, &mut g, &a, Event::Release { x: px + 3, y: py + 2 });
    let live = g.battle.as_ref().expect("a live battle");
    let unit = live.current_unit;
    assert_ne!(unit, 0, "the selection did not become a unit");
    let u = live.runner.units.get(unit);
    assert_eq!(
        (u.target_x as u8, u.target_y as u8),
        want,
        "the release did not order the selection to the ground under it",
    );
}

/// **The band commits on the release.** `Battle_CommitSelection` (`0x0043C247`)
/// takes `param_2 = 0` on every frame of the drag and `1` once, on the release:
/// the highlight follows the box live and the *units* are rearranged only when
/// the button comes up, which is what `DAT_0053E984` — our `current_unit` —
/// records.
///
/// **Ablated**: passing `true` from `drag_to`'s `pick_box` turns the
/// mid-drag assertion red.
#[test]
fn the_selection_band_highlights_live_and_regroups_only_on_the_release() {
    let (mut g, a, mut m) = a_paused_battlefield();
    let start = (bf::VIEW.x + 4, bf::VIEW.y + 4);
    let end = (bf::VIEW.x + bf::VIEW.w - 4, bf::VIEW.y + bf::VIEW.h - 4);
    send(&mut m, &mut g, &a, Event::Pointer { x: start.0, y: start.1 });
    send(&mut m, &mut g, &a, Event::Click { x: start.0, y: start.1 });
    send(&mut m, &mut g, &a, Event::Pointer { x: end.0, y: end.1 });
    {
        let live = g.battle.as_ref().expect("a live battle");
        assert!(live.runner.selected_count(1) > 0, "the band did not highlight while it was drawn");
        assert_eq!(live.current_unit, 0, "the drag regrouped before the button came up");
    }
    send(&mut m, &mut g, &a, Event::Release { x: end.0, y: end.1 });
    let live = g.battle.as_ref().expect("a live battle");
    assert!(live.runner.selected_count(1) > 0);
    assert_ne!(live.current_unit, 0, "the release did not commit the band");
}
