#![allow(unused_imports)]
use super::*;
use crate::screens::setup::skirmish::STRENGTH_WEIGHT;

/// What page 12 draws in place of the battle description and the two army
/// boxes once `DAT_0056899C` is up — `FUN_0042051C` (`00420000.c:124-131`)
/// calls `FUN_0042130F` and `FUN_00420DE4` instead of `FUN_00421231` and
/// `FUN_004209C1`, and `FUN_0041E61D` swaps the background for `skircust.pl8`.
impl SetupScreen {
    /// **`FUN_00420D40` (`0x00420D40`, `00420000.c:296-325`)** — the
    /// twelve-frame animation, whose only state is
    /// [`SetupScreen::tick_banner`]'s counter. The anchor moves with the flag:
    /// `(0xB2, 0xF4)` normally, `(0x10E, 0xCE)` custom.
    ///
    /// [`BATTLE_SHEET`] and not `Misc_sel.pl8`: `g_miscCtySheet` on this page
    /// is what `FUN_00498DCB` (`00490000.c:3999`) left in it.
    pub(crate) fn paint_skirmish_banner(&self, canvas: &mut Canvas, pen: &Pen) {
        let (x, y) = BANNER_AT[usize::from(self.skirmish.custom)];
        if let Some(f) =
            pen.assets.sheet(BATTLE_SHEET).and_then(|s| s.frame(BANNER_FRAME_BASE + self.banner))
        {
            canvas.blit(&f, x, y);
        }
    }

    /// **`FUN_0042130F` (`0x0042130F`, `00420000.c:448-499`) and
    /// `FUN_00420DE4` (`0x00420DE4`, `00420000.c:330-364`)**, in the painter's
    /// order.
    pub(crate) fn paint_skirmish_custom(&self, ctx: &Ctx, canvas: &mut Canvas, pen: &Pen) {
        self.paint_custom_muster(canvas, pen);
        self.paint_custom_sides(ctx, canvas, pen);
    }

    /// `FUN_0042130F` — the strength still unspent, the head count, and the
    /// chosen army's column of counts with each one's strength weight beside it.
    ///
    /// **Eleven rows when the category is a castle and seven otherwise**
    /// (`00420000.c:467-472`): the four siege engines are only offered for a
    /// siege. The count is `0x20` when it is not zero and `0x3F` when it is;
    /// the weight beside it is always `0x3F`.
    fn paint_custom_muster(&self, canvas: &mut Canvas, pen: &Pen) {
        let s = &self.skirmish;
        let (mine, _) = s.fill_armies(&self.troops);
        // **`DAT_00521C90` is what is LEFT to spend, not the army's strength.**
        // `FUN_0042C476` (`00420000.c:4803`) is
        // `DAT_0051FBBC - sum(count x weight)` over the army record, and
        // `Skirmish_FillArmies` (`00420000.c:4743`) and `FUN_0042C2ED` (`:4784`)
        // both zero it.
        //
        // **[D]** it is therefore 0 on every frame we draw: only the `+`/`-`
        // arms (`FUN_0043D74E`, `FUN_0043D7A7`, `00430000.c:7846`, `:7883`)
        // move the record off the table row, and their widget table is not
        // wired.
        let spent: i32 = mine.counts.iter().zip(STRENGTH_WEIGHT).map(|(&c, w)| c as i32 * w).sum();
        let w = pen.eng(canvas, GROUP, 0x1C, 0x3C, 0x1B0, font::TEXT);
        pen.number_in(Face::Body, canvas, w, 0x1B0, mine.strength - spent, ' ', SUFFIX, font::TEXT);

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
    fn paint_custom_sides(&self, ctx: &Ctx, canvas: &mut Canvas, pen: &Pen) {
        let s = &self.skirmish;
        let (mine, theirs) = s.fill_armies(&self.troops);
        let (realm1, realm2) = if s.local_realm() == 1 { (mine, theirs) } else { (theirs, mine) };
        // `Pl8_DrawFrame(g_miscCtySheet, g_realms[n].shieldIndex + 0xD, 0xE6, …)`
        // at `:337` and `:348`, and `Ui_DrawText(&DAT_00553D80, 0x113, 0x6E)`
        // and `(&DAT_00553DAC, 0x113, 0x14D)` at `:338` and `:349`.
        //
        // **[D]** realm 2's name box is empty: `FUN_0042BA40` fills
        // `DAT_00553DAC` from `L2.eng` group 7 by the opponent's lord index out
        // of the table at `0x004D4BC8`, and neither is built.
        self.shield_frame(ctx, canvas, pen, 1, 0xD, 0xE6, 0x73);
        pen.body(canvas, 0x113, 0x6E, &self.name(), font::TEXT);
        self.shield_frame(ctx, canvas, pen, 2, 0xD, 0xE6, 0x152);
        pen.body(canvas, 0x113, 0x14D, "", font::TEXT);
        for (y, army) in [(0x82, realm1), (0x161, realm2)] {
            let w = pen.eng(canvas, GROUP, 0x1C, 0x113, y, font::TEXT);
            pen.number_in(Face::Body, canvas, w, y, army.strength, ' ', SUFFIX, font::TEXT);
        }
        // `Ui_DrawCentred(11, 0x17, 300, 0x1AE, 0x78)` at `:359` — the
        // *attacker* caption alone, with no defender opposite it.
        pen.eng_centred(canvas, GROUP, 0x17, 300, 0x1AE, 0x78, font::TEXT);
        // `DAT_00553DA4 + 0x2A` and `DAT_00553DD0 + 0x2A`: the five difficulty
        // names, 11/0x2A…0x2E, one per realm, centred in 0x81.
        for (y, diff) in [(0x9C, s.difficulty[0]), (0x17B, s.difficulty[1])] {
            pen.eng_centred(canvas, GROUP, 0x2A + diff as usize, 0x108, y, 0x81, font::TEXT);
        }
        // `g_realms[DAT_0053EF5C].shieldIndex + 0x12` at `:362` — the marker
        // that says which of the two stacks is attacking.
        let attacker = if s.realm1_attacks() { 1 } else { 2 };
        self.shield_frame(ctx, canvas, pen, attacker, 0x12, 0x114, 0x1AF);
    }

    /// `Pl8_DrawFrame(g_miscCtySheet, g_realms[realm].shieldIndex + base, x, y)`.
    fn shield_frame(
        &self,
        ctx: &Ctx,
        canvas: &mut Canvas,
        pen: &Pen,
        realm: usize,
        base: usize,
        x: i32,
        y: i32,
    ) {
        let Some(r) = ctx.game.kingdom.realms.get(realm) else { return };
        let frame = r.shield_index as usize + base;
        if let Some(f) = pen.assets.sheet(BATTLE_SHEET).and_then(|s| s.frame(frame)) {
            canvas.blit(&f, x, y);
        }
    }
}

/// Every suffix these two painters pass — `DAT_004D4384` … `DAT_004D4398`,
/// six pointers — is the one-space string, read out of `Lords2.exe` at file
/// offsets `0xD2584` … `0xD2598`. `docs/decisions.md` C140.
const SUFFIX: &str = " ";
