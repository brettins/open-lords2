//! The arm loads the portrait the way every other lord's layout does, and the
//! realm it loads is `DAT_00553EE0` = `g_messageFrom`, the sender:
//!
//! ```c
//! FUN_00475d73(DAT_00553ee0);
//! …
//! Ui_DrawInsetRect(x + 0xF, y + 0x11, 0x52, 0x4E);
//! Blit_Raster(0x4EEB80, x + 0x10, y + 0x12, 0x50, 0x4C);
//! ```

#![allow(unused_imports)]
use super::*;
use l2_game::game::Assets;
use l2_game::message::{self, category, Record};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::message::face_frame;
use l2_game::Game;

fn ending(group: u16, from: u8) -> Record {
    Record { to: 0, from, group, category: category::ENDING, ..Record::default() }
}

macro_rules! letter {
    ($group:expr, $from:expr) => {{
        let (mut g, a, mut m) = painted!();
        post(&mut g, ending($group, $from));
        open_the_scroll(&mut m, &mut g, &a);
        painting(&mut g, &a, &mut m)
    }};
}

/// Pixels that differ, and how many of those lie inside the arm's own blit
/// rectangle `(+0x10, +0x12, 0x50, 0x4C)`.
fn differences(a: &l2_view::Canvas, b: &l2_view::Canvas, r: &Record) -> (usize, usize) {
    let total = a.diff_count(b);
    let f = message::frame_of(r).expect("category 0x0E has a window");
    let (fx, fy, fw, fh) = (f.x + 0x10, f.y + 0x12, 0x50, 0x4C);
    let mut inside = 0usize;
    for y in fy..fy + fh {
        for x in fx..fx + fw {
            if a.at(x as usize, y as usize) != b.at(x as usize, y as usize) {
                inside += 1;
            }
        }
    }
    (total, inside)
}

/// * **the ablation** is a group-225 pair with different senders. 225 is the
///   one ending whose heading is the *local player's* name whatever `from`
///   holds, and its label and body are the group's, so `from` reaches the page
///   through the `Blit_Raster` and nothing else. Delete the blit and this pair
///   is pixel-identical. The pair is a probe, not a letter the game posts —
///   `Msg_Enqueue` always posts 225 with `from = 0`;
/// * **the difference is confined to the blit rectangle**, computed from
/// `message::frame_of` plus the arm's own offsets, so a picture drawn in the
///   wrong place fails.
#[test]
fn the_ending_letter_blits_the_senders_face() {
    let one = letter!(225, 3);
    let other = letter!(225, 5);

    let (total, inside) = differences(&one, &other, &ending(225, 3));
    assert!(total > 0, "the sender picked no picture: the well is empty");
    assert_eq!(inside, total, "{} pixels changed outside the portrait well", total - inside);
}

#[test]
fn the_three_endings_a_game_posts_all_differ() {
    let lost = letter!(224, 1);
    let obituary = letter!(194, 3);
    let won = letter!(225, 0);

    assert!(lost.diff_count(&obituary) > 0, "Defeat! drew what an AI's obituary drew");
    assert!(won.diff_count(&obituary) > 0, "Victory! drew what an AI's obituary drew");
    assert!(won.diff_count(&lost) > 0, "Victory! drew what Defeat! drew");
}

#[test]
fn victory_draws_the_realm_zero_face_and_defeat_the_players() {
    let won = letter!(225, 0);
    let lost = letter!(224, 1);

    let (total, inside) = differences(&won, &lost, &ending(225, 0));
    assert!(total > 0, "Victory! and Defeat! painted identically");
    assert!(inside > 0, "the two endings differ nowhere inside the portrait well");
}

#[test]
fn the_three_endings_reach_three_frames() {
    assert_eq!(face_frame(0, false, 0), 16, "Victory!, posted with from = 0");
    assert_eq!(face_frame(1, true, 1), 12, "Defeat!, and a person plays realm 1");
    assert_eq!(face_frame(2, false, 3), 3, "realm 3's obituary");
    assert_eq!(face_frame(4, false, 5), 9, "realm 5's obituary");
}
