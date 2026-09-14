#![allow(unused_imports)]
use super::*;
use super::screen_tips::*;
use super::game_events::*;
use super::audio_and_narration::*;
use super::*;
use std::collections::BTreeMap;
use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, Frame};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::options::{self, Page, Setting};
use l2_game::tip::{self, Tips, View};
use l2_game::Game;
use l2_kingdom::units_tick::Incursion;

/// **The minimap is live under a tip, and it drops the byte with no restore.**
///
/// Screen `0x27` has no arm in `Screen_FrameInput` (`0x0042FF10`) and none in
/// `Screen_HandleInput` (`0x004BA9C8`) — both were read
/// is a bare `return 0`. But every arm of the first ends `goto LAB_00431F25`,
/// which jumps *over* the epilogue, and `0x27` has no arm to jump: it falls
/// into
///
/// ```text
/// if ((leftPressed || rightPressed) && g_screenId != 0x12 && FUN_004323FE()) {
///     if (g_battlePhase == 0) g_screenId = 0;
/// }
/// ```
///
/// That is a bare assignment, not `Msg_Dismiss`, so `FUN_00476E21` does not run:
/// the screen the tip was shown over is **not** put back and `DAT_004F0358` is
/// **not** re-armed. Both halves are asserted, because the second is the whole
/// difference between this and the dismissal above.
///
/// **Ablation, run:** make `TipScreen::handle` return `Transition::Stay` for
/// everything again and the first assertion goes red — the options page comes
/// back instead of the map
#[test]
fn a_minimap_press_under_a_tip_drops_the_screen_without_re_arming_the_delay() {
    let (mut g, a, mut m) = campaign();
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 100), Some(21));
    assert!(g.tips.hosting(), "the tip is hosting 0x27");

    // The tip window is on top and takes the click first; the minimap raster is
    // outside it, so this reaches the tip host underneath.
    let mini = l2_view::chrome::minimap_hit_area();
    send(&mut m, &mut g, &a, Event::Click { x: mini.x0 + 4, y: mini.y0 + 4 });

    assert!(!g.tips.hosting(), "g_screenId = 0, so the byte is not 0x27 any more");
    assert_eq!(
        g.tips.delay(),
        0,
        "the epilogue is an assignment, not FUN_00476E21: DAT_004F0358 is not re-armed",
    );
    assert!(g.messages.is_open(), "and the window stays: Msg_Pump runs on 0x00 too");

    // The seat comes off on the next tick, and with no re-arm the ladder posts
    // the next tip on that same tick — where a dismissal would have bought
    // twenty quiet frames. Asserted as the difference, because that is what the
    // missing re-arm *is*.
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 100), Some(1), "the next tip, immediately");
}

/// **The right press does the same, on the same edge** — the epilogue's guard
/// is `g_mouseLeftPressed || g_mouseRightPressed` (`0x0042FF10`)
/// host is the one arm we build that reads the down edge of the right button
/// (`g_mouseRightPressed`, `0x004EABE0`).
///
/// Driven as the *press*, not the release: `Event::RightClick` on this screen is
/// `Msg_Dismiss`'s road, which restores the screen underneath and re-arms
/// `DAT_004F0358`. The two must not be confused, so the second half asserts the
/// no-restore signature — no re-arm, window still up — that tells this arm from
/// that one.
///
/// **Ablation, run:** drop `Event::RightPress` from `TipScreen::handle`'s match
/// and the first assertion goes red (the host is still seated); answer it with
/// `Event::RightClick` instead and `delay()` comes back 20, the dismissal's
/// re-arm, which is the wrong arm firing.
#[test]
fn a_right_press_on_the_minimap_under_a_tip_drops_the_screen_like_the_left() {
    let (mut g, a, mut m) = campaign();
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 100), Some(21));
    assert!(g.tips.hosting(), "the tip is hosting 0x27");

    let mini = l2_view::chrome::minimap_hit_area();
    send(&mut m, &mut g, &a, Event::RightPress { x: mini.x0 + 4, y: mini.y0 + 4 });

    assert!(!g.tips.hosting(), "either button drops the byte: the guard names both flags");
    assert_eq!(g.tips.delay(), 0, "an assignment, not FUN_00476E21: no re-arm");
    assert!(g.messages.is_open(), "and the window stays over the map");
}

/// **Off the raster it does nothing**, because `FUN_004323FE` → `Minimap_Click`
/// is the third term of the same `if`: a right press anywhere else on screen
/// `0x27` reaches no arm at all, the screen underneath not being `g_screenId`.
///
/// The contrast is the point. A right *release* at the same pixel is
/// `FUN_0047685D`, which dismisses the scroll; the press is not a dismissal
/// anywhere in the image.
///
/// **Ablation, run:** take the `minimap_hit_area().contains` test out of
/// `TipScreen::handle` and both assertions go red.
#[test]
fn a_right_press_off_the_minimap_under_a_tip_reaches_nothing() {
    let (mut g, a, mut m) = campaign();
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 100), Some(21));

    send(&mut m, &mut g, &a, Event::RightPress { x: 4, y: 4 });

    assert!(g.tips.hosting(), "the press is not a dismissal: the host is still seated");
    assert!(g.messages.is_open(), "and the tip is still up");
}

// ----------------------------------------------------------------- the window

/// **Where the OK button is**, for the wraps that exercise every rule in the
/// layout. The numbers are worked by hand from `Msg_DrawWindow`'s arm and typed.
///
/// Ablation: delete `if measured < 0x61 { y += 0x40 }`
/// go red; drop `+ y` from the height and all four do.
#[test]
fn the_ok_button_sits_where_the_wrapped_text_puts_it() {
    // One line: measured 0x40, below 0x61, so the window drops 64 pixels AND
    // grows by them, because its height includes its own top.
    let one = message::paragraph_layout(&[1]);
    assert_eq!(one.frame, Frame { x: 0x10, y: 0x80, w: 0x1C0, h: 0xC0 });
    assert_eq!(one.frame.ok_button(), (416, 272));
    assert_eq!(one.heading, (0x20, 0x94));
    assert_eq!(one.tops, vec![0xC0]);

    // Two paragraphs of one line: 0x60, still below 0x61.
    assert_eq!(message::paragraph_layout(&[1, 1]).frame.ok_button(), (416, 304));
    // One more line and it is 0x70 — not dropped, so MORE text puts the OK
    // button HIGHER.
    assert_eq!(message::paragraph_layout(&[1, 2]).frame.ok_button(), (416, 192));

    // A five-paragraph tip, "Castle Building:"'s shape: 0x20 + 0x30 + 0x40 +
    // 3 × 0x20 = 0xF0 measured, so the box is 0xF0 + 0x40 tall.
    let five = message::paragraph_layout(&[2, 3, 1, 1, 1]);
    assert_eq!(five.frame, Frame { x: 0x10, y: 0x40, w: 0x1C0, h: 0x130 });
    assert_eq!(five.frame.ok_button(), (416, 320));
    assert_eq!(five.tops, vec![0x80, 0xB0, 0xF0, 0x110, 0x130]);
}

/// **`FUN_0040328E`'s line breaks**, with every glyph ten pixels wide so the
/// arithmetic is visible: a space is four pixels whatever the font, it belongs
/// to the word after it
#[test]
fn a_line_breaks_where_the_original_breaks_it() {
    let ten = |_| 10;
    // "aaaa" 40, " bbbb" 44 -> 84, " cccc" 44 -> 128: over 100.
    assert_eq!(message::break_lines("aaaa bbbb cccc", 100, ten), vec!["aaaa bbbb", "cccc"]);
    // Exactly the width is a break, not a fit.
    assert_eq!(message::break_lines("aaaa bbbb", 84, ten), vec!["aaaa", "bbbb"]);
    assert_eq!(message::break_lines("aaaa bbbb", 85, ten), vec!["aaaa bbbb"]);
    // The leading space of a wrapped word is measured and not drawn.
    assert_eq!(message::break_lines("aaaa  bbbb", 50, ten), vec!["aaaa", "bbbb"]);
    // Always a line, because the draw is inside the loop.
    assert_eq!(message::break_lines("", 100, ten), vec![""]);
}

/// **The words fall back to our transcription**, and only where the file is
/// silent — here, a machine with no `L2.eng` at all.
#[test]
fn without_the_players_file_the_tip_speaks_our_transcription() {
    let bare = Assets::placeholder();
    assert_eq!(tip::words(&bare.shell, 207, 0), "The Town Center:");
    assert_eq!(tip::words(&bare.shell, 211, 1), tip::transcribed(211, 1));
    assert!(tip::words(&bare.shell, 211, 1).starts_with("To try to conquer a county"));
}

// ------------------------------------------------------------ the two gates

