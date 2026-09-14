//! **The peasants' letter has a picture in it** — `Msg_DrawWindow`'s
//! category-`0x02` arm (`0x0047309E`), which the player reported blank.
//!
//! The arm's portrait is a `Faces.pl8` frame like a lord's, but the frame is
//! chosen by the county's **population** and not by a realm:
//!
//! ```c
//! if (*(int *)(&DAT_0053F9D4 + county * 0x300) < 0xF0) FUN_00475d73(6);
//! else                                                 FUN_00475d73(0);
//! …
//! Ui_DrawInsetRect(x + 0xF, y + 0x11, 0x52, 0x4E);
//! Blit_Raster(0x4EEB80, x + 0x10, y + 0x12, 0x50, 0x4C);
//! ```
//!
//! Both tests need the artwork, so both are install-gated like the rest of this
//! suite.

#![allow(unused_imports)]
use super::*;
use l2_game::game::Assets;
use l2_game::message::{self, category, Record};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::message::peasant_face_frame;
use l2_game::Game;

/// `County_GreetArmy` (`0x004ABF77`) posts message `0x86` in group 134 with the
/// county it marched into.
fn greeting(county: u8) -> Record {
    Record { to: 0, group: 134, category: category::COUNTY_PORTRAIT, county, ..Record::default() }
}

/// Paint the greeting for one county at one population. A macro and not a
/// function because `painted!`'s install gate is a bare `return`.
macro_rules! letter {
    ($population:expr, $county:expr) => {{
        let (mut g, a, mut m) = painted!();
        g.kingdom.counties[$county as usize].population = $population;
        post(&mut g, greeting($county));
        open_the_scroll(&mut m, &mut g, &a);
        painting(&mut g, &a, &mut m)
    }};
}

/// **The well holds a frame, and which frame is the county's own.**
///
/// One county, one letter, two populations either side of the arm's `0xF0`.
/// Everything a greeting draws — the window, `L2.eng` 109/1, the county name,
/// the body, the corner button — is a function of the record and the county
/// *id*, which are identical in both paintings. The population reaches the page
/// through exactly one draw call, so:
///
/// * **the two pictures differ** — the ablation. Delete the `Blit_Raster` and
///   both letters paint the empty well the player reported, the difference
///   collapses to nothing and this fails;
/// * **they differ only inside the face rectangle**, computed here from
///   `message::frame_of` plus the arm's own `(+0x10, +0x12, 0x50, 0x4C)` — a
///   blit landing anywhere else fails the second assertion, so the test cannot
///   be satisfied by drawing the picture in the wrong place.
#[test]
fn the_peasants_letter_blits_a_face_the_county_chooses() {
    let county = 3u8;
    let sparse = letter!(0xEF, county);
    let full = letter!(0x1000, county);

    let differing = sparse.diff_count(&full);
    assert!(differing > 0, "the county's population picked no picture: the well is empty");

    let f = message::frame_of(&greeting(county)).expect("category 0x02 has a window");
    let (fx, fy, fw, fh) = (f.x + 0x10, f.y + 0x12, 0x50, 0x4C);
    let mut inside = 0usize;
    for y in fy..fy + fh {
        for x in fx..fx + fw {
            if sparse.at(x as usize, y as usize) != full.at(x as usize, y as usize) {
                inside += 1;
            }
        }
    }
    assert_eq!(
        inside, differing,
        "{} pixels changed outside the portrait well at ({fx}, {fy}, {fw}, {fh})",
        differing - inside
    );
}

/// **The frame ladder's two ends**, as `FUN_00475D73` reaches them: argument 6
/// falls through to `0x11` and argument 0 to `0x10`. Neither is a lord.
#[test]
fn the_two_peasant_frames_are_the_ladders_non_lord_ends() {
    assert_eq!(peasant_face_frame(0), 17, "an empty county");
    assert_eq!(peasant_face_frame(0xEF), 17, "one short of the threshold");
    assert_eq!(peasant_face_frame(0xF0), 16, "the threshold itself is the other face");
    assert_eq!(peasant_face_frame(50_000), 16);
}
