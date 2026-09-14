#![allow(unused_imports)]
use super::*;
use super::constants::*;
use super::screen::*;
use l2_kingdom::divide::{SplitBasket, SplitInto, SplitRefusal};
use l2_kingdom::unit::{ALL_TROOP_TYPES, TroopType};
use l2_view::{text, Canvas};
use crate::press::{Press, Widget};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};
use crate::widget;

#[cfg(test)]
mod tests {
    use super::*;

    /// The eight rows are inside the well the painter draws them in, and the
    /// totals line is below the last of them.
    #[test]
    fn the_eight_rows_and_both_totals_sit_inside_the_rows_well() {
        let (well, w) = (rows_well(), window());
        for row in 0..=MERC_ROW {
            let y = row_y(row);
            assert!(y >= well.y && y < well.y + well.h, "row {row} at {y} is outside the well");
            // The buttons are drawn at `y - 8`, so row 0's pair **overhangs the
            // well's top edge by eight pixels**. That is the painter's own
            // layout — `Ui_DrawBoxInterior(0x18, 0x80, …)` and the sprites at
            // `0x80 - 8` — and asserting they were inside it is what caught it.
            for b in [parent_button(row), daughter_button(row)] {
                assert!(w.contains(b.x, b.y), "row {row}'s button is off the window");
                assert!(w.contains(b.x + b.w - 1, b.y + b.h - 1));
            }
        }
        assert_eq!(parent_button(0).y, well.y - 8, "the first row's button overhangs the well");
        // Without a band the totals take row 7's own y exactly; with one they
        // sit four pixels below it.
        assert_eq!(totals_y(false), row_y(MERC_ROW));
        assert_eq!(totals_y(true) - row_y(MERC_ROW), 0x24);
        assert!(totals_y(true) < well.y + well.h, "the totals run off the well");
    }

    /// **The buttons are `g_splitWidgets`' own records**, transcribed rather
    /// than derived from the painter, and this is where that is pinned.
    ///
    /// The literals come from `node tools/oracle/widgets.js widgets 4dd388 18`
    /// against the player's own `Lords2.exe`: records 2 … 17 are `(256 | 288,
    /// row * 0x20 + 0x78)`, 24 square, alternating `SplitScreen_ToParent` and
    /// `SplitScreen_ToDaughter`, and records 0 and 1 are the tick and the
    /// cross.
    ///
    /// It replaces a test that asserted the buttons were *left of their own
    /// numbers*, which they were — because they were sitting on the troop-type
    /// icons at `0xA8` and `0x158`, 88 pixels from where the game hit-tests
    /// them. A geometry check derived from a painter agrees with a hit box
    /// derived from the same painter, and neither knows about the table.
    #[test]
    fn the_row_buttons_are_the_widget_tables_own_geometry() {
        for row in 0..=MERC_ROW {
            let (p, d) = (parent_button(row), daughter_button(row));
            assert_eq!((p.x, p.w, p.h), (256, 24, 24), "row {row} parent");
            assert_eq!((d.x, d.w, d.h), (288, 24, 24), "row {row} daughter");
            assert_eq!(p.y, row as i32 * 0x20 + 0x78, "row {row} y");
            assert_eq!(d.y, p.y);
            // The gutter the table puts them in: right of the parent's number,
            // left of the daughter's icon.
            assert!(PARENT_NUMBER_X < p.x, "row {row}: the button covers its number");
            assert!(d.x + d.w <= DAUGHTER_ICON_X, "row {row}: the button covers the icon");
        }
    }

    /// **Nothing this screen hit-tests overlaps anything else it hit-tests.**
    ///
    /// The three buttons this replaced — SPLIT, DISBAND and CANCEL at y 446 —
    /// had a test of their own that *passed*: it asserted they were left of
    /// `OK`, calling `OK` *"the original's tick"*. `OK` is the corner picture at
    /// (428, 436); the tick is record 0 of `g_splitWidgets` at (288, 420), and
    /// CANCEL at (264, 446) 100 × 18 overlapped it in a 32 × 6 strip. **Our
    /// cancel sat on the original's confirm.** The check was right about the
    /// rectangle it named and the rectangle it named was not the one it meant.
    ///
    /// So this one enumerates every box the screen tests and compares them
    /// pairwise, which has no rectangle to name wrongly.
    #[test]
    fn no_two_hotspots_on_this_screen_overlap() {
        let mut boxes: Vec<(String, Rect)> = vec![
            ("tick".into(), SPLIT_TICK),
            ("cross".into(), SPLIT_CROSS),
            ("ok".into(), OK),
        ];
        for row in 0..=MERC_ROW {
            boxes.push((format!("parent {row}"), parent_button(row)));
            boxes.push((format!("daughter {row}"), daughter_button(row)));
        }
        for (i, (an, a)) in boxes.iter().enumerate() {
            assert!(
                a.x >= 0 && a.y >= 0 && a.x + a.w <= 640 && a.y + a.h <= 480,
                "{an} {a:?} is off the screen",
            );
            for (bn, b) in boxes.iter().skip(i + 1) {
                let hit = a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
                assert!(!hit, "{an} {a:?} overlaps {bn} {b:?}");
            }
        }
    }
}

