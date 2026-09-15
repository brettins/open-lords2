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

#[test]
fn the_scroll_paints_and_painting_it_again_changes_nothing() {
    let (mut g, a, mut m) = painted!();
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

/// The probe is the window's own top edge — inside the border and outside every
/// glyph and every button — so it is about `FUN_004093E0` and nothing else.
///
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

