#![allow(unused_imports)]
use super::*;
use super::dismissal_and_timing::*;
use super::prompts_and_alliances::*;
use super::outcomes_and_obituaries::*;
use super::posting_and_sync::*;
use super::battle_drain::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, category, Prompt, Record};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_kingdom::victory::Outcome;

/// **Drawing the scroll twice over itself changes nothing.**
///
/// Every glyph and every frame is an opaque blit,
/// *but only if the first one happened*. It is the build stamp's assertion, and
/// it needs no threshold and no knowledge of what else is on the page.
#[test]
fn the_scroll_paints_and_painting_it_again_changes_nothing() {
    let (mut g, a, mut m) = painted!();
    // **The map alone first.** Comparing against a blank canvas would measure
    // the campaign map underneath and pass with the window deleted, which is the
    // build stamp's mistake exactly.
    let bare = painting(&mut g, &a, &mut m);

    post(&mut g, notice(146));
    open_the_scroll(&mut m, &mut g, &a);
    let once = painting(&mut g, &a, &mut m);
    assert!(once.diff_count(&bare) > 0, "the window is on top of the map");

    let mut twice = once.clone();
    {
        let ctx = Ctx { game: &mut g, assets: &a };
        m.draw(&ctx, &mut twice);
    }
    assert_eq!(once.diff_count(&twice), 0, "an opaque blit drawn twice is the same picture");
}

/// **The BODY of the message is drawn, and it is the message's own.**
///
/// Two records of the **same group** and different variants: the heading is the
/// group's own label in both, so the only thing that can differ is the body
/// string `Eng_DrawString(group, variant + 1)` picks. Group 194 holds a label
/// and sixteen laments,
///
/// If the body draw did nothing the two pictures would be identical, and this is
/// the assertion that says so — with no threshold and no knowledge of where the
/// text lands.
#[test]
fn two_variants_of_one_group_draw_two_different_bodies() {
    let first = {
        let (mut g, a, mut m) = painted!();
        post(&mut g, Record { to: 0, group: 194, variant: 0, ..Record::default() });
        open_the_scroll(&mut m, &mut g, &a);
        painting(&mut g, &a, &mut m)
    };
    let second = {
        let (mut g, a, mut m) = painted!();
        post(&mut g, Record { to: 0, group: 194, variant: 5, ..Record::default() });
        open_the_scroll(&mut m, &mut g, &a);
        painting(&mut g, &a, &mut m)
    };
    assert!(
        first.diff_count(&second) > 0,
        "one group, two variants, one picture -- the body was not drawn",
    );
}

/// **The corner button and the answer buttons are really on the canvas.**
///
/// Not a pixel count and not a comparison against a different message: draw the
/// frame, copy it, draw *those two things again* on the copy, and require the
/// two canvases to be identical. A sprite is an opaque blit,
/// over itself changes nothing — but only if the first one happened.
///
/// Ablating `pen.ok_button` moves 500-odd pixels here and nothing anywhere else
/// in this file, which is the property the build-stamp test was written to have
/// and the pixel count it replaced did not.
#[test]
fn the_scrolls_clickable_pictures_are_drawn_where_the_hit_tests_look() {
    let (mut g, a, mut m) = painted!();
    post(&mut g, alliance_offer(3));
    open_the_scroll(&mut m, &mut g, &a);

    let once = painting(&mut g, &a, &mut m);
    let mut again = once.clone();
    let record = *g.messages.open().expect("a record");
    {
        let ctx = Ctx { game: &mut g, assets: &a };
        l2_game::screens::message::repaint_clickables(&ctx, &mut again, &record);
    }
    assert_eq!(
        once.diff_count(&again),
        0,
        "the corner button and the two mailed hands were already painted",
    );
}

/// The same for a plain notice, which has the corner button and no widgets.
#[test]
fn a_notices_corner_button_is_drawn_too() {
    let (mut g, a, mut m) = painted!();
    post(&mut g, notice(146));
    open_the_scroll(&mut m, &mut g, &a);

    let once = painting(&mut g, &a, &mut m);
    let mut again = once.clone();
    let record = *g.messages.open().expect("a record");
    {
        let ctx = Ctx { game: &mut g, assets: &a };
        l2_game::screens::message::repaint_clickables(&ctx, &mut again, &record);
    }
    assert_eq!(once.diff_count(&again), 0);
}

/// **The window frame itself is painted**, and not only the words on it.
///
/// The probe is the window's own top edge — inside the border and outside every
/// glyph and every button — so it is about `FUN_004093E0` and nothing else.
/// Ablating the text or either button leaves it alone; ablating the frame lets
/// the campaign map show through and this goes red.
///
/// The coordinates are pinned from the decompilation, not read back from
/// `frame_of`: `Msg_DrawWindow`'s category-`0x0B` arm is
/// `FUN_004093E0(0x10, 0x80, 0x1C, 0x0F)`.
#[test]
fn the_window_frame_covers_the_map_under_it() {
    let (mut g, a, mut m) = painted!();
    let bare = painting(&mut g, &a, &mut m);
    post(&mut g, alliance_offer(3));
    open_the_scroll(&mut m, &mut g, &a);
    let with_window = painting(&mut g, &a, &mut m);

    let (x, y) = (0x10usize, 0x80usize);
    let differs = (0..16)
        .filter(|dx| bare.at(x + dx, y) != with_window.at(x + dx, y))
        .count();
    assert!(
        differs > 0,
        "sixteen pixels of the window's top edge and not one of them changed -- \
         the frame was not drawn, or it was drawn somewhere else",
    );
}

