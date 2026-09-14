#![allow(unused_imports)]
use super::*;
use super::constants::*;
use super::screen::*;
use l2_kingdom::industry::{self, CastleRefusal};
use l2_view::{text, Canvas};
use crate::press::{Press, Widget};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

#[cfg(test)]
mod tests {
    use super::*;

    /// **The five strips tile the row with no gap and no overlap**, which is
    /// what says the widget table was decoded at the right base address: a
    /// mis-aligned read would not produce five abutting rectangles.
    #[test]
    fn the_five_castle_strips_tile_the_row_exactly() {
        for level in 0..5 {
            let r = type_rect(level);
            assert_eq!(r.y, 270);
            assert_eq!(r.h, 146);
            assert!(r.w > 0);
        }
        for level in 0..4 {
            let a = type_rect(level);
            let b = type_rect(level + 1);
            assert_eq!(a.x + a.w, b.x, "strip {level} does not meet strip {}", level + 1);
        }
        assert_eq!(type_rect(0).x, 17);
        assert_eq!(type_rect(4).x + type_rect(4).w, 619);
    }

    /// **Three tables read at three base addresses agree with each other, five
    /// times each.**
    ///
    /// `g_castleTypeWidgets` (`0x004DC818`) holds the five hit rectangles;
    /// `DAT_004D2E28` holds the `cas_bits.pl8` selection mark for each
    /// selection; `DAT_004D2E64` holds the *"you already have this one"* mark
    /// for each standing castle type. Nothing in the binary relates them —
    /// they are read by two different functions in two different passes — and
    /// every one of the ten marks lands inside the strip it belongs to. A
    /// mis-aligned read of any of the three would put a mark in the wrong
    /// strip or off the row.
    ///
/// The literals here are pinned from the decompilation
    /// computed from the constants, so ablating a constant reddens the test.
    #[test]
    fn the_two_mark_tables_land_inside_the_five_strips() {
        let strips: [(i32, i32); 5] = [(17, 95), (96, 209), (210, 290), (291, 414), (415, 618)];
        for (i, &(x0, x1)) in strips.iter().enumerate() {
            // …and the pinned copy is the table's, so ablating TYPE_BOUNDS
// reddens this too.
            assert_eq!((TYPE_BOUNDS[i].0, TYPE_BOUNDS[i].2), (x0, x1), "strip {i}");
            let (_, mx, my) = SELECTED_MARK[i];
            assert!((x0..=x1).contains(&mx), "selection mark {i} at x {mx} is not in {x0}..{x1}");
            assert!((270..=415).contains(&my), "selection mark {i} at y {my} is off the row");
            // The standing mark is indexed by castle **type**, so strip i is
            // type i + 1, and the painter subtracts ten before drawing.
            let sx = STANDING_MARK_X[i + 1] + STANDING_MARK_DX;
            assert!((x0..=x1).contains(&sx), "standing mark {i} at x {sx} is not in {x0}..{x1}");
        }
        assert_eq!(STANDING_MARK_X[0], 0, "slot 0 is never read: castleType 0 skips the draw");
    }

    /// **`caspics.pl8` has four big pictures for five castles**, and the file
    /// says so independently of the table: 256,072 bytes is 72 of header plus
    /// `4 * 320 * 200`, and [`PICTURE`] is 320 × 200.
    #[test]
    fn the_motte_and_bailey_alone_has_no_big_picture() {
        assert_eq!(PICTURE_FRAME, [1, 0, 2, 3, 4]);
        assert_eq!(PICTURE_FRAME.iter().filter(|&&f| f == 0).count(), 1);
        assert_eq!(PICTURE_FRAME[1], 0, "selection 1 is the motte and bailey");
        let mut used: Vec<usize> =
            PICTURE_FRAME.iter().filter(|&&f| f != 0).map(|&f| f - 1).collect();
        used.sort_unstable();
        assert_eq!(used, vec![0, 1, 2, 3], "four distinct frames, and no gap");
        assert_eq!((PICTURE.w, PICTURE.h), (320, 200));
    }

    /// The OK and the cancel are clear of the strips and of each other.
    #[test]
    fn the_two_buttons_are_clear_of_the_five() {
        for level in 0..5 {
            let r = type_rect(level);
            assert!(r.y + r.h <= OK.y, "strip {level} runs into the buttons");
        }
        // Both are read out of `g_castleBuildWidgets`
// one onto the other should say so.
        assert!(
            core::hint::black_box(OK).x + OK.w <= CANCEL.x,
            "the tick and the cross overlap"
        );
    }
}

