#![allow(unused_imports)]
use super::*;
use crate::screens::setup::skirmish::ROWS_SHOWN;

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
            if s.top + n >= s.rows {
                break;
            }
            let y = 0xB9 + n as i32 * 0x10;
            let lit = s.slot == n;
            canvas.fill_rect(0x1D4, y, 0x85, 0x10, if lit { 0x3F } else { 0x66 });
        }
        // **The scrollbar is three segments** (`00420000.c:401-416`): the gap
        // above the thumb and the gap below it in `0x66`, the thumb in `0x3F`.
        // All three are `Pct(0x3C, PctOf(n, DAT_0053F0D4))`, and the thumb is
        // not `Pct` of six — it is `0x3C` less the other two, so it carries
        // both roundings. Each is drawn only when it is not zero.
        let rows = s.rows.max(1) as i32;
        // `Pct(0x3C, PctOf(n, DAT_0053F0D4))` — `PctOf` (`0x00404DC1`) is zero
        // safe, `Pct` (`0x00404D6B`) is `p * x / 100`.
        let seg = |n: i32| {
            let p = if rows == 0 { 0 } else { n * 100 / rows };
            p * 0x3C / 100
        };
        let above = seg(s.top as i32);
        let below = seg(rows - s.top as i32 - ROWS_SHOWN as i32);
        let thumb = 0x3C - above - below;
        for (y, h, colour) in
            [(0xCB, above, 0x66), (0xCB + above, thumb, 0x3F), (0xCB + above + thumb, below, 0x66)]
        {
            if h != 0 {
                canvas.fill_rect(0x25C, y, 0x14, h, colour);
            }
        }

        // **`FUN_004207C3`** — the four categories, the chosen one in `0x20`.
        // `L2.eng` 11/0x14 is the castle row, 0x15 and 0x16 the two field
        // rows, and the fourth is the `.skr` file's own name, or 11/0x23 when
        // there is none. Top to bottom the categories are 2, 0, 1, 3.
        for (kind, y) in [(2usize, 299i32), (0, 0x14B), (1, 0x16B)] {
            let colour = if s.kind == kind { 0x20 } else { font::TEXT };
            pen.eng_centred(canvas, GROUP, 0x14 + kind_caption(kind), 0x1FC, y, 0x78, colour);
        }
        match &s.file {
            // `FUN_004025D7(&DAT_0053F5FC, 0x1EF, 0x188, 0x8D, …)`
            // (`00420000.c:217`) — the name is centred in the same 0x8D box
            // the *no file* caption uses, not drawn left-aligned.
            Some(name) => {
                let colour = if s.kind == 3 { 0x20 } else { font::TEXT };
                pen.body_centred(canvas, 0x1EF, 0x188, 0x8D, name, colour);
            }
            None => pen.eng_centred(canvas, GROUP, 0x23, 0x1EF, 0x188, 0x8D, 0xF9),
        }

        // **`FUN_004209C1`** — the two names, the two roles and the strengths.
        // The two name boxes come first (`00420000.c:244-245`): `DAT_00553D80`
        // at x 0xE and `DAT_00553DAC` at x 0x118, both `FUN_004025D7`-centred
        // in 0x96. They are realm 1's and realm 2's, keyed to the realm and
        // not to the side, so they do not move when the sides swap.
        //
        // **[D]** the right box is empty: `FUN_0042BA40` fills `DAT_00553DAC`
        // from `L2.eng` group 7 by the opponent's lord index, out of the table
        // at `0x004D4BC8`, and neither the table nor that read is built.
        pen.body_centred(canvas, 0xE, 0x140, 0x96, &self.name(), font::TEXT);

        // The roles do move: `DAT_0053EF5C == 1` (`00420000.c:246`) — realm 1
        // is the attacker — puts 11/0x17 on the left and 11/0x18 on the
        // right. Realm 1 is the local player in single player, so this is not
        // the same test as `local_attacks`.
        let (left, right) = if s.realm1_attacks() { (0x17, 0x18) } else { (0x18, 0x17) };
        pen.eng_centred(canvas, GROUP, left, 0xB, 0x112, 0x78, font::TEXT);
        pen.eng_centred(canvas, GROUP, right, 0x116, 0x112, 0x78, font::TEXT);

        // The handicap caption, on `DAT_00553DA4` — realm 1's column
        // (`00420000.c:260-264`): 11/0x29 **on** the seesaw's middle and
        // 11/0x28 off it. `FUN_0043DAF3` is the arm.
        let caption = if s.difficulty[0] == 2 { 0x29 } else { 0x28 };
        pen.eng_centred(canvas, GROUP, caption, 0xA0, 0x1C8, 0x8C, font::TEXT);

        // `Eng_DrawString(11, 0x1C, ...)` then `Ui_DrawNumber` at the advance
        // it left — the autocalc strength of each army, `DAT_0051FBBC` and
        // `DAT_0051FAD0`, which `Skirmish_FillArmies` recomputes on every one
        // of this page's arms.
        //
        // The boxes are realm 1's and realm 2's, like the names above them:
        // `FUN_004209C1` (`00420000.c:277-289`) puts the local strength
        // `DAT_0051FBBC` in the left box when `g_localPlayer == 1` and in the
        // right box when it is 2, and `DAT_0051FAD0` in the other. Realm 1 is
        // the local player here, so his army is on the left — by the realm,
        // not by the side.
        let (mine, theirs) = s.fill_armies(&self.troops);
        let (realm1, realm2) = if s.local_realm() == 1 { (mine, theirs) } else { (theirs, mine) };
        for (x, army) in [(0x34, realm1), (0x148, realm2)] {
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
