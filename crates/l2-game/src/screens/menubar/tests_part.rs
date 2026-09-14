#![allow(unused_imports)]
use super::*;
use super::items::*;
use super::dropdown::*;
use super::render::*;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::options::Page;
use crate::screens::saveload::Mode;
use crate::shell::font;

#[cfg(test)]
mod tests {
    use super::*;

    /// Four items, five items, seven items — and `L2.eng` groups 1, 2 and 3
    /// hold exactly that many strings after their label. The `y` column is the
    /// table's own and has no gaps.
    #[test]
    fn the_three_tables_are_the_shapes_the_binary_has() {
        assert_eq!(MENUS[0].items.len(), 4);
        assert_eq!(MENUS[1].items.len(), 5);
        assert_eq!(MENUS[2].items.len(), 7);
        for m in &MENUS {
            assert_eq!(m.items.len(), m.item_fallbacks.len(), "group {}", m.group);
            for (n, &(y, index, _)) in m.items.iter().enumerate() {
                assert_eq!(y, n as i32 * ITEM_PITCH, "group {}: the y column has a gap", m.group);
                assert_eq!(index, n + 1, "group {}: the string indices have a gap", m.group);
            }
        }
        // Sixteen items over three menus, which is what the row count of
        // docs/screens-county.md 10.1 has to be.
        assert_eq!(MENUS.iter().map(|m| m.items.len()).sum::<usize>(), 16);
    }

    /// The five help topics are five **consecutive** message ids. That is the
    /// property that says the table was read and not assembled.
    #[test]
    fn the_help_topics_are_consecutive_message_ids() {
        let ids: Vec<u32> = MENUS[2]
            .items
            .iter()
            .filter_map(|&(_, _, it)| match it {
                Item::Help(id) => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec![0x123, 0x124, 0x125, 0x126, 0x127]);
    }

    /// **The plate is the original's, and its height is the item count.**
    /// `g_spriteHeight = (count * 0x15) / 16 + 2`, in cells: 4 items → 7,
    /// 5 → 8, 7 → 11. Pinned as literals from the decompilation
    /// recomputed from `plate_rows`, which would test nothing.
    #[test]
    fn the_dropdown_plate_is_twelve_cells_wide_and_grows_with_the_item_count() {
        assert_eq!(plate_rows(4), 7);
        assert_eq!(plate_rows(5), 8);
        assert_eq!(plate_rows(7), 11);
        let t = [Rect::new(10, 6, 30, 12), Rect::new(72, 6, 50, 12), Rect::new(154, 6, 30, 12)];
        let p = plate_rect(&t, 2);
        assert_eq!((p.x, p.y, p.w, p.h), (154, 6 + 0x12, 192, 11 * 16));
        // Three widths for one row, and they really are three.
        assert_eq!(HIGHLIGHT_W, 176);
        assert_eq!(ITEM_W, 144);
        assert!(HIGHLIGHT_W < p.w && ITEM_W < HIGHLIGHT_W);
        // Every caption sits inside the plate it is drawn on.
        for i in 0..MENUS[2].items.len() {
            let x = p.x + CAPTION_DX;
            let y = BAR_Y + MENUS[2].items[i].0 + CAPTION_DY;
            assert!(x > p.x && x < p.x + p.w, "caption {i} starts outside the plate");
            assert!(y > p.y && y < p.y + p.h, "caption {i} at y {y} is outside the plate");
        }
    }

    /// **The menu bar's words are `L2.eng`'s, on a machine that has the game.**
    ///
    /// `docs/agents.md` records *"our own labels drawn where the menu bar's
    /// words are"* as one of five defects that existed **only** against real
    /// assets, so this is asserted against the player's own `L2.eng` and not
    /// against `MENUS`' fallbacks. Every string here is pinned as a literal.
    #[test]
    fn the_bar_draws_the_games_own_words_and_not_ours() {
        let Some(dir) = l2_testkit::install_dir() else {
            eprintln!("skipping: no game install");
            return;
        };
        let bytes = std::fs::read(dir.join("L2.eng")).expect("L2.eng");
        let eng = crate::shell::Eng::parse(bytes).expect("L2.eng parses");

        // The three titles, index 0 of groups 1, 2 and 3.
        assert_eq!(eng.get(1, 0), Some("File"));
        assert_eq!(eng.get(2, 0), Some("Options"));
        assert_eq!(eng.get(3, 0), Some("Help"));

        // All sixteen items, in the tables' own order.
        let expected: [&[&str]; 3] = [
            &["New Game", "Load", "Save", "Quit"],
            &["Advanced", "Sounds", "Display", "Game Speed", "Scroll Speed"],
            &[
                "Game Help",
                "How do I...",
                "Grow grain?",
                "Build a castle?",
                "Make Weapons?",
                "Manage each turn.?",
                "About",
            ],
        ];
        for (m, want) in MENUS.iter().zip(expected) {
            assert_eq!(
                m.items.len(),
                want.len(),
                "group {} has {} items in the table",
                m.group,
                want.len()
            );
            // **The group holds the title plus exactly those items and no
            // more.** A seventeenth string would mean a row we do not draw.
            assert_eq!(
                eng.group(m.group).len(),
                want.len() + 1,
                "group {} is one title and {} items",
                m.group,
                want.len()
            );
            for (n, &(_, index, _)) in m.items.iter().enumerate() {
                assert_eq!(eng.get(m.group, index), Some(want[n]), "group {}", m.group);
            }
        }

        // And none of the fallbacks is ever what a player with the game sees.
        for (m, want) in MENUS.iter().zip(expected) {
            assert_ne!(eng.get(m.group, 0), Some(m.fallback));
            for (n, f) in m.item_fallbacks.iter().enumerate() {
                assert_ne!(*f, want[n], "fallback {f:?} is the game's own word");
            }
        }
    }

    /// **The titles are measured in the game's own font**
    /// the hit boxes right — `Ui_DrawMenuTitles` starts the pen at 10 and adds
    /// the drawn width plus 32 after each. With `Fntl2_14.pl8` loaded the three
    /// boxes must be three different widths, must not overlap, and must leave
    /// exactly 32 pixels between one and the next.
    #[test]
    fn the_title_boxes_are_measured_with_fntl2_14() {
        let Some(dir) = l2_testkit::install_dir() else {
            eprintln!("skipping: no game install");
            return;
        };
        let bytes = std::fs::read(dir.join(font::BODY)).expect("Fntl2_14.pl8");
        let font = crate::shell::Font::new(bytes, 16).expect("Fntl2_14.pl8 decodes");
        // The module's own rule, driven with the install's own font. The
        // captions and every number asserted are literals out of `L2.eng` and
        // `Ui_DrawMenuTitles`, so ablating `BAR_X`, `BAR_Y`, `TITLE_H` or
        // `TITLE_GAP` turns this red.
        let captions =
            ["File".to_string(), "Options".to_string(), "Help".to_string()];
        let boxes = title_boxes(&captions, |s| font.width(s));

        assert_eq!(boxes[0].x, 10, "the first title starts at the record's own x");
        for (i, b) in boxes.iter().enumerate() {
            assert!(b.w > 0, "title {i} measured zero: the font did not load");
            assert_eq!(b.y, 6, "every record's y is 6");
            assert_eq!(b.h, 12, "Menu_HitTitle's height is a fixed 12");
        }
        for pair in boxes.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert_eq!(b.x - (a.x + a.w), 32, "g_penAdvance += 0x20 between titles");
        }
        // "Options" is the longest of the three in any reasonable typeface, and
        // if all three came out equal the font is not being consulted at all.
        assert!(boxes[1].w > boxes[2].w, "Options must be wider than Help");
        // And the fallback metrics really are different metrics, so a machine
        // with no game is not silently getting the same answer.
        let fallback = title_boxes(&captions, l2_view::text::width);
        assert_ne!(fallback[1].w, boxes[1].w, "the 5 x 7 font must not measure Fntl2_14's widths");
    }

    /// Every item row is 144 wide and 15 tall in a 20-pixel pitch, and the first
    /// is 31 pixels below the title's baseline. Read off `FUN_0040E099`.
    #[test]
    fn the_item_rows_are_the_hit_tests_geometry() {
        let t = [Rect::new(10, 6, 30, 12), Rect::new(72, 6, 50, 12), Rect::new(154, 6, 30, 12)];
        let first = item_rect(&t, 1, 0);
        assert_eq!((first.x, first.y, first.w, first.h), (72, 6 + 0x1F, 0x90, 0x0F));
        let second = item_rect(&t, 1, 1);
        assert_eq!(second.y - first.y, ITEM_PITCH, "20-pixel pitch");
        assert!(second.y > first.y + first.h, "the five pixels between rows are dead");
    }
}

