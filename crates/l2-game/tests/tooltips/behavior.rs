#![allow(unused_imports)]
use super::*;
use super::rendering::*;
use super::battle::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::options::{self, Setting};
use l2_game::shell::Pen;
use l2_game::tooltip::{self, Shown, Sidebar};
use l2_game::Game;
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_view::Canvas;

/// `FUN_00477131`: `999 < (int)(timeGetTime() - stamp)`, the stamp written on
/// the frame the mouse last changed. At 16 ms a tick, 62 ticks is 992 ms and 63
/// is 1,008. The tick that *sees* the move writes the stamp and is not counted.
///
/// Ablations: `REST_MS` 999 → 991 shows on tick 62 and goes red; dropping the
/// `if changed { stamp = now }` arm shows on the first still tick (the stamp is
/// *long ago* when the campaign arrives) and goes red.
#[test]
fn end_turn_names_itself_on_the_sixty_third_still_tick_and_not_the_sixty_second() {
    let (mut g, a, mut m) = campaign();
    point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
    tick(&mut m, &mut g, &a); // the tick that saw the move
    for n in 1..=62 {
        tick(&mut m, &mut g, &a);
        assert_eq!(shown(&m), None, "still tick {n} is too early");
    }
    tick(&mut m, &mut g, &a);
    let s = shown(&m).expect("the sixty-third still tick shows it");
    assert_eq!(s.id, 14, "End Turn's id in FUN_00477320");
    assert_eq!(tooltip::words(&a.shell, s.id), "End your turn");
    assert_eq!((s.x, s.y), (340, 440));
}

/// `FUN_004770B8` hides without touching the stamp, and `FUN_00477131` writes
/// the stamp only on a frame the mouse changed *while no tip is up*. So a
/// one-tick nudge of a tip leaves the stamp where the tip's own resolve put
/// it: the tip comes back 63 ticks after *that*, not 63 after the nudge — and a
/// tip that had been up a whole second comes back on the very next still tick.
///
/// Ablations: stamp the hide in `frame`'s shown branch and the return moves a
/// tick later and goes red; drop `pointer_changed` for a `Pointer` event and
/// the reset goes red.
#[test]
fn moving_resets_the_rest_and_moving_under_a_tip_takes_it_away() {
    let (mut g, a, mut m) = campaign();
    point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
    tick(&mut m, &mut g, &a);
    ticks(&mut m, &mut g, &a, 40);
    point(&mut m, &mut g, &a, END_TURN.0 + 1, END_TURN.1);
    tick(&mut m, &mut g, &a);
    ticks(&mut m, &mut g, &a, 62);
    assert_eq!(shown(&m), None, "the rest restarted at the move");
    tick(&mut m, &mut g, &a); // tick S: shown, and stamped
    assert_eq!(shown(&m).map(|s| s.id), Some(14));

    point(&mut m, &mut g, &a, END_TURN.0 + 2, END_TURN.1);
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m), None, "FUN_004770B8 hides it");
    for n in 2..=62 {
        tick(&mut m, &mut g, &a);
        assert_eq!(shown(&m), None, "tick S + {n}");
    }
    tick(&mut m, &mut g, &a);
    let s = shown(&m).expect("the hide wrote no stamp");
    assert_eq!((s.id, s.x), (14, END_TURN.0 + 2 - 220), "at the new place");

    ticks(&mut m, &mut g, &a, 63);
    point(&mut m, &mut g, &a, END_TURN.0 + 3, END_TURN.1);
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m), None);
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m).map(|s| s.x), Some(END_TURN.0 + 3 - 220), "no rest after a one-tick nudge");

    send(&mut m, &mut g, &a, Event::RightClick { x: END_TURN.0 + 3, y: END_TURN.1 });
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m), None, "g_mouseInputChanged is set by a button");
}

/// `FUN_00476E95` is `if (g_optToolTips != 0) { … }` round everything, and
/// `Opt_ToggleToolTips` writes `_DAT_004EA830 = 0` — so the tick after the
/// option comes back on shows the tip with no second of rest.
///
/// Ablations: remove the `enabled` guard in `Tooltips::frame` and the first
/// assertion goes red; remove the `tool_tips_seen` rearm in `Machine` and the
/// last one does (the tip then waits until 63 ticks after the stamp).
#[test]
fn the_option_off_shows_nothing_and_turning_it_on_shows_the_tip_at_once() {
    let (mut g, a, mut m) = campaign();
    point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
    tick(&mut m, &mut g, &a); // stamp
    {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        options::toggle(Setting::ToolTips, &mut ctx);
    }
    assert!(!g.prefs.tool_tips);
    ticks(&mut m, &mut g, &a, 200);
    assert_eq!(shown(&m), None, "g_optToolTips is 0");

    let toggle = |g: &mut Game| {
        let mut ctx = Ctx { game: g, assets: &a };
        options::toggle(Setting::ToolTips, &mut ctx);
    };
    toggle(&mut g);
    assert!(g.prefs.tool_tips);
    point(&mut m, &mut g, &a, END_TURN.0 + 1, END_TURN.1);
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m), None, "a changed frame stamps");

    toggle(&mut g);
    tick(&mut m, &mut g, &a);
    toggle(&mut g);
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m).map(|s| s.id), Some(14), "Opt_ToggleToolTips zeroed the stamp");
}

#[test]
fn the_option_off_draws_no_box() {
    let (mut g, a, mut m) = campaign();
    point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
    ticks(&mut m, &mut g, &a, 64);
    let s = shown(&m).expect("a tip is up");
    let rect = {
        let ctx = Ctx { game: &mut g, assets: &a };
        tooltip::layout(&ctx, s).0
    };
    let on = region(&draw(&mut m, &mut g, &a), rect);
    g.prefs.tool_tips = false;
    let off = region(&draw(&mut m, &mut g, &a), rect);
    assert_ne!(on, off, "the box is drawn with the option on and not with it off");
    assert!(off.iter().any(|&p| p != tooltip::FILL), "no fill under the option off");
}

/// **`DAT_004D6FB8`: the sidebar's tips follow it onto the screens that sit on
/// it, and nowhere else.**
#[test]
fn a_county_panel_keeps_the_sidebar_tips_and_the_other_lords_and_a_tip_screen_do_not() {
    for (over, want) in [
        (Some(ScreenId::County(1, Panel::Tax)), Some(14)),
        (Some(ScreenId::Diplomacy), None),
        (None, None), // a tip screen, below
    ] {
        let (mut g, a, mut m) = campaign();
        match over {
            Some(id) => m.push(id),
            None => {
                g.prefs.tip_screens = true;
                assert!(l2_game::tip::show(&mut g, 200), "a tip is posted");
            }
        }
        point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
        ticks(&mut m, &mut g, &a, 70);
        if over.is_none() {
            assert!(m.ids().contains(&ScreenId::Tip), "screen 0x27 is up");
        }
        assert_eq!(shown(&m).map(|s| s.id), want, "over {over:?}");
    }
}

/// **A repaint takes the tip away and the ordinary rule brings it back** —
/// `Screen_Draw`'s `FUN_0047703A`, which keeps the stamp.
///
/// Ablations: remove the `drop_tip` call in `Machine::run_tooltips` and the
/// first `None` goes red; stamp the drop in `Tooltips::drop_tip` and the last
/// assertion does.
#[test]
fn opening_a_screen_drops_the_tip_and_its_own_stamp_brings_it_back() {
    let (mut g, a, mut m) = campaign();
    point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
    tick(&mut m, &mut g, &a); // the stamp, at tick 1
    ticks(&mut m, &mut g, &a, 63); // the tip, and the stamp again, at tick 64
    assert_eq!(shown(&m).map(|s| s.id), Some(14));
    ticks(&mut m, &mut g, &a, 10);
    m.push(ScreenId::County(1, Panel::Tax));
    tick(&mut m, &mut g, &a); // tick 75
    assert_eq!(shown(&m), None, "the panel's repaint dropped it");
    ticks(&mut m, &mut g, &a, 51); // tick 126: 62 after the stamp
    assert_eq!(shown(&m), None, "the stamp is not yet a second old");
    tick(&mut m, &mut g, &a); // tick 127: 63 after it
    assert_eq!(shown(&m).map(|s| s.id), Some(14), "back, over the panel");

    ticks(&mut m, &mut g, &a, 100);
    m.push(ScreenId::Diplomacy);
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m), None, "0x0B has no tips at all");
}


