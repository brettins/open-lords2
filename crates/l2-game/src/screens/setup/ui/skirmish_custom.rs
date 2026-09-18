#![allow(unused_imports)]
use super::*;
use crate::screens::setup::skirmish::STRENGTH_WEIGHT;

/// What page 12 draws in place of the battle description and the two army
/// boxes once `DAT_0056899C` is up — `FUN_0042051C` (`00420000.c:128-130`)
/// calls `FUN_0042130F` and `FUN_00420DE4` instead of `FUN_00421231` and
/// `FUN_004209C1`, and `FUN_0041E61D` swaps the background for `skircust.pl8`.
impl SetupScreen {
    /// **`FUN_00420D40` (`0x00420D40`)** — the twelve-frame animation, whose
    /// only state is [`SetupScreen::tick_banner`]'s counter. The anchor moves
    /// with the flag: `(0xB2, 0xF4)` normally, `(0x10E, 0xCE)` custom.
    pub(crate) fn paint_skirmish_banner(&self, canvas: &mut Canvas, pen: &Pen) {
        let (x, y) = BANNER_AT[usize::from(self.skirmish.custom)];
        if let Some(f) =
            pen.assets.sheet(ICON_SHEET).and_then(|s| s.frame(BANNER_FRAME_BASE + self.banner))
        {
            canvas.blit(&f, x, y);
        }
    }

    /// **`FUN_0042130F` (`0x0042130F`, `00420000.c:5341-5369`) and
    /// `FUN_00420DE4` (`0x00420DE4`, `00420000.c:4786-4811`)**, in the
    /// painter's order.
    pub(crate) fn paint_skirmish_custom(&self, canvas: &mut Canvas, pen: &Pen) {
        self.paint_custom_muster(canvas, pen);
        self.paint_custom_sides(canvas, pen);
    }

    /// `FUN_0042130F` — the chosen army's column of counts, the strength
    /// weight beside each, the total defensive advantage and the head count.
    ///
    /// **Eleven rows when the category is a castle and seven otherwise**
    /// (`00420000.c:5361-5365`): the four siege engines are only offered for a
    /// siege. The count is `0x20` when it is not zero and `0x3F` when it is;
    /// the weight beside it is always `0x3F`.
    fn paint_custom_muster(&self, canvas: &mut Canvas, pen: &Pen) {
        let s = &self.skirmish;
        // `DAT_00521C90` is the number `Eng_DrawString(11, 0x1C)` — the same
        // *Strength* caption the normal page uses — is followed by.
        let (mine, _) = s.fill_armies(&self.troops);
        let w = pen.eng(canvas, GROUP, 0x1C, 0x3C, 0x1B0, font::TEXT);
        pen.number_in(Face::Body, canvas, w, 0x1B0, mine.strength, ' ', SUFFIX, font::TEXT);

        // `g_units[DAT_00553EFC].menTotal` at `(0x2B, 0x1C5)`, and then 11/0x21
        // for one man and 11/0x1B for any other number — at `g_penAdvance +
        // 0x35`, which is a *different* origin from the number's own.
        let men = mine.men;
        let end = pen.number_in(Face::Body, canvas, 0x2B, 0x1C5, men, ' ', SUFFIX, font::TEXT);
        let noun = if men == 1 { 0x21 } else { 0x1B };
        pen.eng(canvas, GROUP, noun, 0x35 + (end - 0x2B), 0x1C5, font::TEXT);

        let rows = if s.kind == 2 { STRENGTH_WEIGHT.len() } else { 7 };
        for slot in 0..rows {
            let y = 0x6C + slot as i32 * 0x1E;
            let count = mine.counts[slot] as i32;
            let colour = if count == 0 { 0x3F } else { 0x20 };
            // `Ui_DrawNumberRight` **centres** in its width (C119), so both
            // columns are 0x14 wide and the digits sit in the middle of them.
            pen.number_centred(canvas, 0xA6, y, 0x14, count, ' ', SUFFIX, colour);
            pen.number_centred(canvas, 0x37, y, 0x14, STRENGTH_WEIGHT[slot], ' ', SUFFIX, 0x3F);
        }
    }

    /// `FUN_00420DE4` — the two realms stacked instead of side by side: a
    /// shield, the name, the strength, and the handicap caption under each.
    fn paint_custom_sides(&self, canvas: &mut Canvas, pen: &Pen) {
        let s = &self.skirmish;
        let (mine, theirs) = s.fill_armies(&self.troops);
        let (realm1, realm2) = if s.local_realm() == 1 { (mine, theirs) } else { (theirs, mine) };
        // `Ui_DrawText(&DAT_00553D80, 0x113, 0x6E)` then the strength at
        // `0x82`; realm 2's pair is the same 0xDF lower.
        pen.body(canvas, 0x113, 0x6E, &self.name(), font::TEXT);
        for (y, army) in [(0x82, realm1), (0x161, realm2)] {
            let w = pen.eng(canvas, GROUP, 0x1C, 0x113, y, font::TEXT);
            pen.number_in(Face::Body, canvas, w, y, army.strength, ' ', SUFFIX, font::TEXT);
        }
        // `Ui_DrawCentred(11, 0x17, 300, 0x1AE, 0x78)` — the *attacker*
        // caption alone, with no defender opposite it.
        pen.eng_centred(canvas, GROUP, 0x17, 300, 0x1AE, 0x78, font::TEXT);
        // `DAT_00553DA4 + 0x2A` and `DAT_00553DD0 + 0x2A`: the five difficulty
        // names, 11/0x2A…0x2E, one per realm, centred in 0x81.
        for (y, diff) in [(0x9C, s.difficulty[0]), (0x17B, s.difficulty[1])] {
            pen.eng_centred(canvas, GROUP, 0x2A + diff as usize, 0x108, y, 0x81, font::TEXT);
        }
    }
}

/// Every suffix these two painters pass — `DAT_004D4384` … `DAT_004D4398`,
/// six pointers — is the one-space string, read out of `Lords2.exe` at file
/// offsets `0xD2584` … `0xD2598`. `docs/decisions.md` C140.
const SUFFIX: &str = " ";
