#![allow(unused_imports)]
use super::*;
use crate::screens::setup::skirmish::{rows_in, ROWS_SHOWN};

/// Page 12's own furniture, under the three captions `FUN_00420630` draws
/// first: the battle list, the four categories, the two sides and the
/// handicap. The animated banner (`FUN_00420D40`, twelve `Misc_sel.pl8`
/// frames on a 100 ms timer) and the battle's name and description
/// (`FUN_00421231`, out of `BATTLES.ENG`, which is not `L2.eng`) are not
/// drawn.
impl SetupScreen {
    pub(crate) fn paint_skirmish_page(&self, canvas: &mut Canvas, pen: &Pen) {
        let s = &self.skirmish;

        // **`FUN_00421005`** — six rows at `(0x1D4, 0xB9 + 16n)`, 0x85 by 0x10.
        // The lit row is a `0x3F` bar with `0x66` text and the others are the
        // other way round. The names are `BATTLES.ENG`'s, three strings per
        // battle, and nothing reads that file yet — so the bars are here and
        // the words are not.
        for n in 0..ROWS_SHOWN {
            if s.top + n >= rows_in(s.kind) {
                break;
            }
            let y = 0xB9 + n as i32 * 0x10;
            let lit = s.slot == n;
            canvas.fill_rect(0x1D4, y, 0x85, 0x10, if lit { 0x3F } else { 0x66 });
        }
        // The scrollbar's thumb, sized by `PctOf` over the same two counts.
        let rows = rows_in(s.kind).max(1) as i32;
        let above = 0x3C * s.top as i32 / rows;
        let thumb = (0x3C * ROWS_SHOWN as i32 / rows).min(0x3C - above);
        canvas.fill_rect(0x25C, 0xCB + above, 0x14, thumb.max(1), 0x3F);

        // **`FUN_004207C3`** — the four categories, the chosen one in `0x20`.
        // `L2.eng` 11/0x14 is the castle row, 0x15 and 0x16 the two field
        // rows, and the fourth is the `.skr` file's own name, or 11/0x23 when
        // there is none. Top to bottom the categories are 2, 0, 1, 3.
        for (kind, y) in [(2usize, 299i32), (0, 0x14B), (1, 0x16B)] {
            let colour = if s.kind == kind { 0x20 } else { font::TEXT };
            pen.eng_centred(canvas, GROUP, 0x14 + kind_caption(kind), 0x1FC, y, 0x78, colour);
        }
        match &s.file {
            Some(name) => {
                let colour = if s.kind == 3 { 0x20 } else { font::TEXT };
                pen.body(canvas, 0x1EF, 0x188, name, colour);
            }
            None => pen.eng_centred(canvas, GROUP, 0x23, 0x1EF, 0x188, 0x8D, 0xF9),
        }

        // **`FUN_004209C1`** — the two names, the two roles and the strengths.
        // `DAT_0053EF5C == 1` puts the attacker on the left; 11/0x17 is the
        // attacker's caption and 11/0x18 the defender's.
        let (left, right) =
            if s.local_attacks { (0x17, 0x18) } else { (0x18, 0x17) };
        pen.eng_centred(canvas, GROUP, left, 0xB, 0x112, 0x78, font::TEXT);
        pen.eng_centred(canvas, GROUP, right, 0x116, 0x112, 0x78, font::TEXT);
        pen.body_centred(canvas, 0xE, 0x140, 0x96, &self.name(), font::TEXT);

        // The handicap, 11/0x28 or 11/0x29 when realm 1 is on 2 — the seesaw's
        // middle. `FUN_0043DAF3` is the arm.
        let caption = if s.difficulty[0] == 2 { 0x28 } else { 0x29 };
        pen.eng_centred(canvas, GROUP, caption, 0xA0, 0x1C8, 0x8C, font::TEXT);

        // `Eng_DrawString(11, 0x1C, ...)` then `Ui_DrawNumber` at the advance
        // it left — the autocalc strength of each army, `DAT_0051FBBC` and
        // `DAT_0051FAD0`, which `Skirmish_FillArmies` recomputes on every one
        // of this page's arms.
        let (mine, theirs) = s.fill_armies(&self.troops);
        for (x, army) in [(0x34, mine), (0x148, theirs)] {
            let w = pen.eng(canvas, GROUP, 0x1C, x, 0x1C6, font::TEXT);
            pen.body(canvas, x + w, 0x1C6, &army.strength.to_string(), font::TEXT);
        }
    }
}

/// `L2.eng` 11/0x14 … 0x16 are in the order the painter draws them — castle,
/// then the two field categories — so the caption of category `k` is not `k`.
fn kind_caption(kind: usize) -> usize {
    match kind {
        2 => 0,
        0 => 1,
        _ => 2,
    }
}
