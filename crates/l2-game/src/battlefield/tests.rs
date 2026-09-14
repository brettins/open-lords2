#![allow(unused_imports)]
use super::*;

use l2_sim::runner::{BattleRunner, Conclusion, Formation};
use l2_sim::terrain::DIM;
use crate::input::Rect;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_buttons_tile_the_bottom_of_the_panel_and_nothing_else() {
        let mut last = BUTTON_ORIGIN.0;
        for b in Button::ALL {
            let r = b.rect();
            assert_eq!(r.x, last, "{b:?} does not abut its neighbour");
            assert_eq!((r.y, r.w, r.h), (0x1C0, 32, 32));
            last = r.x + r.w;
        }
        assert_eq!(last, 640, "the five buttons end at the right edge");
        assert_eq!(Button::at(0x1E0, 0x1C0), Some(Button::Pause));
        assert_eq!(Button::at(639, 479), Some(Button::Autocalc));
        assert_eq!(Button::at(0x1DF, 0x1C0), None, "one pixel left of the strip");
        assert_eq!(Button::at(0x1E0, 0x1BF), None, "one pixel above it");
    }

    /// The three banner layouts, straight out of `DAT_004D31F4`: the first
    /// three rectangles of each, the slot counts, and that none of them
    /// overlaps the overview panel above or the buttons below.
    #[test]
    fn the_banner_layouts_match_the_table_and_stay_between_the_panels() {
        assert_eq!(BannerLayout::for_count(0).slots, 12);
        assert_eq!(BannerLayout::for_count(12).slots, 12);
        assert_eq!(BannerLayout::for_count(13).slots, 18);
        assert_eq!(BannerLayout::for_count(18).slots, 18);
        assert_eq!(BannerLayout::for_count(19).slots, 50);
        assert_eq!(BannerLayout::for_count(80).slots, 50);

        // Table entries 0, 1, 3 of the twelve-slot layout.
        assert_eq!(BANNERS_FEW.rect(0), Rect::new(488, 189, 45, 50));
        assert_eq!(BANNERS_FEW.rect(1), Rect::new(541, 189, 45, 50));
        assert_eq!(BANNERS_FEW.rect(3), Rect::new(488, 244, 45, 50));
        // Entries 12, 13, 15 of the table are slots 0, 1, 3 of the second.
        assert_eq!(BANNERS_SOME.rect(0), Rect::new(488, 186, 45, 35));
        assert_eq!(BANNERS_SOME.rect(1), Rect::new(541, 186, 45, 35));
        assert_eq!(BANNERS_SOME.rect(3), Rect::new(488, 223, 45, 35));
        // Entries 30, 31, 36 are slots 0, 1, 6 of the third.
        assert_eq!(BANNERS_MANY.rect(0), Rect::new(484, 185, 22, 18));
        assert_eq!(BANNERS_MANY.rect(1), Rect::new(510, 185, 22, 18));
        assert_eq!(BANNERS_MANY.rect(6), Rect::new(484, 204, 22, 18));

        for l in [&BANNERS_FEW, &BANNERS_SOME, &BANNERS_MANY] {
            let last = l.rect(l.slots - 1);
            assert!(l.rect(0).y > OVERVIEW.y + OVERVIEW.h - 1, "banners start under the overview");
            assert!(last.y + last.h <= 0x1C0, "banners end above the buttons");
        }
    }

    /// `OVERVIEW` and `l2_view::scene`'s overview constants are the same four
    /// numbers out of `FUN_004BC107(…, 0x1E0, 0x18, 2)` and `FUN_004BC020(…,
    /// 0x50, 0x50, …)`, so they may not drift apart: the painter uses one and
    /// `BattleMap_Click` uses the other.
    #[test]
    fn the_overview_rect_is_the_raster_the_painter_fills() {
        use l2_view::scene::{OVERVIEW_ORIGIN_X, OVERVIEW_ORIGIN_Y, OVERVIEW_SCALE, OVERVIEW_SIDE};
        assert_eq!((OVERVIEW.x, OVERVIEW.y), (OVERVIEW_ORIGIN_X, OVERVIEW_ORIGIN_Y));
        assert_eq!((OVERVIEW.w, OVERVIEW.h), (OVERVIEW_SIDE as i32, OVERVIEW_SIDE as i32));
        assert_eq!(OVERVIEW_SCALE * l2_sim::terrain::DIM as i32, OVERVIEW_SIDE as i32);
        // And it ends exactly where `Screen_DrawBattlefield` puts `Misc_bat.pl8`
        // frame 0, at `(0x1E0, 0xB8)`.
        assert_eq!(OVERVIEW.y + OVERVIEW.h, 0xB8);
    }

    #[test]
    fn only_the_nine_digit_keys_are_control_groups() {
        assert_eq!(group_slot(b'1'), Some(0));
        assert_eq!(group_slot(b'9'), Some(8));
        assert_eq!(group_slot(b'0'), None, "0x30 is below the original's bound");
        assert_eq!(group_slot(b'A'), None);
    }

    #[test]
    fn the_viewport_is_fifteen_by_fourteen_tiles_at_the_top_left() {
        assert_eq!(VIEW, Rect::new(0, 24, 480, 448));
        assert!(!VIEW.contains(480, 100), "the panel is not the field");
        assert!(!VIEW.contains(100, 23), "nor the menu bar");
        assert!(VIEW.contains(479, 471));
    }

    /// **The hit test and the picture agree about where the field is.**
    ///
    /// `docs/decisions.md` C61's other lesson: every campaign-map test ran on
    /// `Assets::placeholder`, the one configuration in which a broken hit test
    /// and the picture agree. Here they are two modules — `l2-view`'s renderer
    /// has its own copy of `Battle_LoadAssets`' geometry — and if they ever part
    /// company a click would land on a different cell from the one under the
    /// pointer, at every zoom and with any artwork. This needs no install
    /// because both sides are constants out of the binary; that is what makes it
    /// safe to test without one.
    #[test]
    fn our_hit_test_and_the_renderer_read_the_same_geometry() {
        use l2_view::scene;
        assert_eq!(TILE, scene::TILE);
        assert_eq!(VIEW_COLS as usize, scene::VIEW_COLS);
        assert_eq!(VIEW_ROWS as usize, scene::VIEW_ROWS);
        assert_eq!(VIEW.x, scene::ORIGIN_X);
        assert_eq!(VIEW.y, scene::ORIGIN_Y);
        assert_eq!(VIEW.w, scene::VIEW_COLS as i32 * scene::TILE);
        assert_eq!(VIEW.h, scene::VIEW_ROWS as i32 * scene::TILE);
    }

    /// The camera the renderer is handed is the one the hit test converts from,
    /// clamp included — a camera clamped on one side and not the other is the
    /// same defect one step later.
    #[test]
    fn the_camera_clamps_the_same_way_on_both_sides() {
        for (x, y) in [(-5, -5), (0, 0), (40, 40), (200, 200)] {
            let ours = (
                x.clamp(0, DIM as i32 - VIEW_COLS),
                y.clamp(0, DIM as i32 - VIEW_ROWS),
            );
            let theirs = l2_view::scene::Camera::clamped(x, y);
            assert_eq!((ours.0 as usize, ours.1 as usize), (theirs.x, theirs.y), "at {x},{y}");
        }
    }
}

