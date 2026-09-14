#![allow(unused_imports)]
use super::*;
use super::render::*;
use super::screen::*;
use super::*;
use super::helpers::*;
use super::constants::*;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::setup::SetupOptions;
use crate::shell::{self, font, Pen};
use crate::text::{self, TextField};

impl SetupScreen {
    fn colour(&self, index: usize) -> u8 {
        if index == self.selected {
            font::HIGHLIGHT
        } else {
            font::TEXT
        }
    }

    /// A menu item: the recess, then the caption centred in it.
    fn draw_item(
        &self,
        canvas: &mut Canvas,
        pen: &Pen,
        index: usize,
        rect: Rect,
        group: usize,
        s: usize,
    ) {
        shell::button_recess(canvas, rect.x, rect.y, rect.w, rect.h);
        pen.eng_centred(
            canvas,
            group,
            s,
            rect.x,
            rect.y + ITEM_TEXT,
            rect.w,
            self.colour(index),
        );
    }

    pub(super) fn paint(&self, ctx: &Ctx, canvas: &mut Canvas, pen: &Pen, head: &Pen, page: SetupPage) {
        match page {
            SetupPage::Title => {
                pen.window_from(canvas, BOX_SHEET, 0xA0, 10, 0x14, 0xF);
                head.eng_heading_centred(canvas, GROUP, 0, TITLE_X, TITLE_Y, TITLE_W, font::TEXT);
                pen.eng_centred(canvas, GROUP, 1, TITLE_X, SUBTITLE_Y, TITLE_W, font::TEXT);
                for (i, s) in TITLE_ITEMS.iter().enumerate() {
                    self.draw_item(canvas, pen, i, item_rect(i), GROUP, *s);
                }
                // **The one thing on this page that must not be quiet.**
                // Every `Pen` method degrades to the 5 × 7 debug font per call
                // and says nothing, so a checkout that cannot read its fonts
                // draws a complete, correct, illegible front end. The load-time
                // complaint goes to stderr, which a player double-clicking an
                // executable never sees. This is the same sentence on the first
                // screen he does.
                missing_fonts_banner(ctx, canvas);
                // **Ours, **
                // Not the original's — see [`crate::build_id`], which exists
                // because a player spent an evening reporting three defects
                // against a binary four merges old.
                crate::build_id::draw(canvas, pen);
                // **Ours too, and a deliberate divergence: `FUN_0041EA14`
                // draws no clock.** A player asked for the time in MST on this
                // screen. Bottom-right, opposite the build stamp, below
                // everything the original's page-1 painter reaches (its window
                // ends at y 250). Do not "fix" it toward the binary and do not
                // count it as a reproduction — `crate::wallclock` carries the
                // reading of `FUN_0041EA14` that says there is nothing there.
                //
                // The time itself is `ctx.assets.wall_clock`, which only the
                // shell ever fills: no clock is read here or anywhere below it
                // (`docs/netcode.md` D-5)
// on the page.
                if let Some(now) = ctx.assets.wall_clock {
                    crate::wallclock::draw(canvas, pen, now);
                }
            }
            SetupPage::Options => {
                pen.window_from(canvas, BOX_SHEET, 0xB0, 10, 0x12, 0x12);
                head.eng_heading_centred(canvas, GROUP, 5, 0xB0, 0x2D, 0x120, font::TEXT);
                for (i, s) in OPTION_ITEMS.iter().enumerate() {
                    self.draw_item(canvas, pen, i, item_rect(i), GROUP, *s);
                }
            }
            SetupPage::Load => {
                // `FUN_004148E4(5)`, transcribed. The box, the caption, one
                // **recess** and three **outlines** — and the difference
                // between the two was wrong here until the draw-call audit read
                // the painter: `FUN_00403EE4` is the bevelled recess (top and
                // right `0x35`, bottom and left `0x28`) and `FUN_00403CF4` is a
                // flat one-pixel rectangle in a single colour. Only the outer
                // frame is a recess; the name field, the file list and the
                // status line are outlines in `0x3F`.
                pen.window_from(canvas, BOX_SHEET, 0x60, 10, 0x1C, 0x15);
                head.eng_heading_centred(canvas, GROUP_FILE, 5, 0x60, 0x22, 0x1C0, font::TEXT);
                shell::button_recess(canvas, 0x70, 0x42, 400, 0x100);
                for r in LOAD_OUTLINES {
                    outline_rect(canvas, r, font::TEXT);
                }
                // **What used to be here was invented.** The line under the
                // list read `L2.eng` 40/8 *"Right click to exit."*, which the
                // original draws on **page 13** and never here.
                // `SaveLoad_DrawStatus` puts 40/2 *"Loading game. Please
                // wait."* at (128, 292) and only while `DAT_0057D3C4` — a
// frame countdown set to 150 or 400 when a load
                // starts, and zeroed when the box opens — is running. An idle
                // load box has an empty status line, so ours has one too.
                //
                // The rest of `SaveLoad_DrawStatus` is not drawn: the four
                // `Panels2.pl8` plates, the file name being typed with its
                // caret, and up to thirty save names in three columns from
                // (128, 118). [`super::saveload`] is the same function on
                // screens `0x35`/`0x36`; page 3 is that screen inside the front
                // end's window, and joining them is a job on its own.
            }
            SetupPage::Shield => {
                pen.window_from(canvas, BOX_SHEET, 0x50, 10, 0x1E, 0x10);
                head.eng_heading_centred(canvas, GROUP, 10, 0x50, 0x23, 0x1E0, font::TEXT);
                self.paint_shields(canvas, pen);
                for (i, (x, y, s)) in SHIELD_BUTTONS.iter().enumerate() {
                    self.draw_item(
                        canvas,
                        pen,
                        5 + i,
                        Rect::new(*x, *y, ITEM_W, ITEM_H),
                        GROUP,
                        *s,
                    );
                }
            }
            SetupPage::Campaign | SetupPage::GameType => {
                pen.window_from(canvas, BOX_SHEET, 0x40, 0x32, 0x20, 8);
                head.eng_heading_centred(canvas, GROUP_EXPANSION, 0, 0x40, 0x50, 0x200, font::TEXT);
                let items: [usize; 2] =
                    if page == SetupPage::Campaign { [4, 5] } else { [1, 2] };
                for i in 0..2 {
                    shell::button_recess(canvas, PAIR_X[i], PAIR_Y, PAIR_W, ITEM_H);
                    pen.eng_centred(
                        canvas,
                        GROUP_EXPANSION,
                        items[i],
                        PAIR_TEXT_X[i],
                        PAIR_Y + 6,
                        PAIR_TEXT_W,
                        self.colour(i),
                    );
                }
            }
            SetupPage::NoCd => {
                pen.window_from(canvas, BOX_SHEET, 0x50, 10, 0x1E, 0x13);
                head.eng_heading_centred(canvas, GROUP, 0x10, 0x50, 0x24, 0x1E0, font::TEXT);
                // Three wrapped paragraphs at width 0x180 and one plain line.
                // The plain one is drawn in colour 1, not 0x3F — the only
                // string on any of these pages that is.
                let a = pen.assets;
                for (i, y) in [(0x11usize, 0x48), (0x12, 0x78)] {
                    let t = a.text(GROUP, i).to_string();
                    pen.body_wrapped(canvas, 0x80, y, 0x180, &t, font::TEXT);
                }
                pen.eng(canvas, GROUP, 0x30, 0x80, 0xDC, 1);
                let t = a.text(GROUP, 0x31).to_string();
                pen.body_wrapped(canvas, 0x80, 0xF0, 0x180, &t, font::TEXT);
            }
            SetupPage::Custom | SetupPage::CustomMulti | SetupPage::Dropdown => {
                self.paint_custom(ctx, canvas, pen, page)
            }
            SetupPage::Skirmish | SetupPage::SkirmishMulti | SetupPage::SkirmishFile => {
                self.paint_skirmish(canvas, pen, head, page)
            }
        }
    }

    /// `FUN_0041F1DD` and `FUN_0041F321`: the name field and the five shields.
    ///
    /// **[V]** and worth writing down, because the obvious reading is wrong.
    /// The painter blits from `DAT_004EABEC` — the general scratch buffer,
    /// which on this page holds **`panels2.pl8`** — and *not* from
    /// `g_miscCtySheet`. `Misc_sel.pl8` has seventeen frames; the indices here
    /// run to 215, and `Panels2.pl8` has 216. The file settles it: frames
    /// 205 … 214 are five pairs of roughly 60 × 65 shields, one pair per realm
    /// colour, 204 is a 224 × 32 plate the size of a name field, and 215 is a
    /// 54 × 27 plaque. Nothing else in either file is that shape.
    ///
    /// Frame `2i + 0xCB` is the shield when the colour is free and `2i + 0xCC`
    /// when it is taken, at `x = 0x70 + 88(i - 1)`, `y = 0x8C`; the 54 × 27
    /// plaque marks the chosen one at `(x + 4, 0x70)`.
    fn paint_shields(&self, canvas: &mut Canvas, pen: &Pen) {
        // **The name field, and it now has a name in it.**
        //
        // `FUN_0041F321` is five statements and every one of them is here:
        //
        // ```c
        // g_caretPlaced = 0; g_caretX = 0; g_drawIndex = 0;
        // g_editDrawing = 1; g_penAdvance = 0;
        // Pl8_DrawFrameHere(panels2, 0xCC, 0xD0, 0x48);      /* the plate      */
        // Ui_DrawText(&g_options, 0xD6, 0x50, &g_fontBody, 0x3F);
        // if (!g_caretPlaced) { g_caretX = g_penAdvance; g_caretPlaced = 1; }
        // g_caretX += 0xD6;  g_caretY = 0x52;
        // Edit_DrawCaret(0x5AF8F0, 0x3F);                    /* the caret      */
        // ```
        //
        // The plate is drawn **before** the text and the caret **after** it,
// so the caret is a solid bar
        // eats. `g_caretPlaced` is set by `Ui_DrawText` itself when the drawing
        // index reaches `g_editCaret`, so the caret x is the pen after that
        // many characters and needs nothing from the caller;
        // `TextField::caret_x` computes the same number the same way.
        //
        // **The plate is the same frame whether or not the field is being
// typed into.** the caret
// is the whole of the affordance, so it had to be built
        //
        let sheet = pen.assets.sheet(BOX_SHEET);
        match sheet.and_then(|s| s.frame(0xCC)) {
            Some(f) => canvas.blit(&f, NAME_PLATE_X, NAME_PLATE_Y),
            // No `Panels2.pl8`. A recess of our own, so the field is still a
            // field on a placeholder install and a test can still find it.
            None => shell::button_recess(canvas, NAME_PLATE_X, NAME_PLATE_Y, 0xE0, 0x20),
        }
        pen.body(canvas, NAME_X, NAME_Y, &self.name.text(), font::TEXT);
        // `font::TEXT` is `0x3F`, a palette index; with no font loaded the
        // fallback renderer draws in named interface colours instead, and the
        // caret has to follow the text it belongs to.
        let ink = if pen.assets.body.is_some() { font::TEXT } else { pen.ink.text };
        self.name.draw_caret(canvas, NAME_X, NAME_Y, ink, &text::FontMetrics::of(pen.assets));
        let Some(sheet) = sheet else { return };
        for i in 1..6usize {
            let x = SHIELD_X + (i as i32 - 1) * SHIELD_STEP;
            if i - 1 == self.shield {
                if let Some(f) = sheet.frame(0xD7) {
                    canvas.blit(&f, x + 4, 0x70);
                }
            }
            // Free, not taken: this shell has no lobby, so every colour is
            // offered and the taken variant (`2i + 0xCC`)
            if let Some(f) = sheet.frame(i * 2 + 0xCB) {
                canvas.blit(&f, x, 0x8C);
            }
        }
    }

    /// Pages 7 and 8: the twelve options, the map list, the buttons.
    fn paint_custom(&self, ctx: &Ctx, canvas: &mut Canvas, pen: &Pen, page: SetupPage) {
        let a = pen.assets;
        if page == SetupPage::Custom {
            if let Some(s) = a.sheet(ICON_SHEET) {
                if let Some(f) = s.frame(0x0F) {
                    canvas.blit(&f, 0xA0, 0);
                }
            }
        }
        // The map list plate, its five rows, and the row the pointer is on.
        if let Some(s) = a.sheet(ICON_SHEET) {
            if let Some(f) = s.frame(0x10) {
                canvas.blit(&f, MAP_LIST_X, 9);
            }
        }
        // **The thumbnail of the map the list is pointing at**, which nothing
        // here drew. `ScenarioList_Draw`'s second statement is
        // `FUN_00410C71(0, 0x1F0, 9)` — the same helper the send-supplies panel
        // and the diplomacy county picker use, at the same `(x - 2, y + 3)`
        // offset — so the plate is a frame round a live minimap and not a
        // picture of one. County 0 is passed, so nothing is highlighted.
        if let Some(m) = ctx.assets.minimap(self.map) {
            let owner = |c: u8| ctx.game.kingdom.counties.get(c as usize).map_or(0, |c| c.owner);
            l2_view::chrome::draw_minimap_at(
                canvas,
                &m,
                MAP_THUMB,
                0,
                &l2_view::chrome::MinimapTint::Owner(&owner),
            );
        }
        for row in 0..MAP_LIST_ROWS {
            let slot = self.map_top + row;
            if slot >= MAP_COUNT {
                break;
            }
            let y = MAP_LIST_Y + row as i32 * MAP_LIST_ROW;
            let chosen = slot == self.map;
            canvas.fill_rect(
                MAP_LIST_X,
                y,
                MAP_LIST_W,
                MAP_LIST_ROW,
                if chosen { font::TEXT } else { font::DISABLED },
            );
            let name = a.text(GROUP_MAPS, slot).to_string();
            pen.body(
                canvas,
                MAP_LIST_TEXT_X,
                y + 1,
                &name,
                if chosen { font::DISABLED } else { font::TEXT },
            );
        }
        self.paint_scrollbar(canvas);
        // The twelve options.
        let chrome = pen.chrome;
        for (i, &(x, boxy, labely)) in OPTION_CELLS.iter().enumerate() {
            // `FUN_0040328E(102, i, x, labelY, 100, …)`: wrapped at 100 pixels,
            // which is what makes "Advanced Farming" two lines that end where
            // the box begins instead of one that runs into the next column.
            let label = a.text(GROUP_OPTIONS, i).to_string();
            pen.body_wrapped(canvas, x, labely, OPTION_LABEL_W, &label, font::TEXT);
            match chrome {
                // `FUN_004093E0` is `Ui_DrawBox` with border set 1.
                Some(c) => c.draw_box(canvas, x, boxy, 6, 3, 1),
                None => shell::button_recess(canvas, x, boxy, OPTION_BOX_W, OPTION_BOX_H),
            }
            let value = a.text(GROUP_VALUES, self.option_value(i)).to_string();
            pen.body_centred(canvas, x + 1, boxy + 16, 0x60, &value, font::HIGHLIGHT);
        }
        let n = if page == SetupPage::Custom { 3 } else { 4 };
        for (i, (x, s)) in CUSTOM_BUTTONS.iter().take(n).enumerate() {
            pen.eng_centred(
                canvas,
                GROUP,
                *s,
                *x,
                CUSTOM_BUTTON_Y,
                CUSTOM_BUTTON_W,
                self.colour(12 + MAP_LIST_ROWS + i),
            );
        }
        // **Both custom pages draw the five player cards, not only page 8.**
        // This comment used to say page 8; `FUN_0041F6C7` — page 7's painter —
        // calls `FUN_0041FBCB` with no guard
        // The card is `misc_sel` frame `2 * shieldIndex - 2` at (10, 94n + 6),
        // the lord's portrait frame `lord + 9` (14 for a human) at (84, 94n +
        // 10), and the name centred in 160 pixels at y = 94n + 80 in the
        // realm's own palette byte. `Realms_AssignLords` runs *inside* the
        // painter, so the cards are the assignment as much as a picture of it.
        //
        // Not drawn: the front end has not assigned lords in this workspace and
        // a card built from `Realm::default()` would be five copies of one
        // face. The chat log (`FUN_0041FF75`) and the chat input line
        // (`FUN_00420147`) are **multiplayer only** — both open with
        // `if (g_multiplayer != 0)` — so on page 7 the original draws nothing
        // for them either, and that is four call sites correctly absent rather
        // than missing.
        self.paint_gaps(canvas, pen, ctx.game.prefs.debug_overlay);
    }

    /// `ScenarioList_Draw`'s scroll bar, transcribed.
    ///
    /// Three stacked fills: the run above the window in `0x20`, the window in
    /// `0x3F`, the run below in `0x20`. The thumb absorbs the rounding error of
    /// all three percentages so the track is always exactly 44 pixels.
    ///
    /// **[V] with an empty list it is one full-length thumb**: `PctOf` returns
    /// 0 when the total is 0, so all three heights come out 0 and the
    /// correction hands the whole 44 to the middle segment.
    fn paint_scrollbar(&self, canvas: &mut Canvas) {
        // **The total is the one number here that is not the original's.**
        // `ScenarioList_Draw` divides by `DAT_00554018`, which
        // `FUN_0046A101` sets to *how many of the sixty slots have a
        // `MAPnn.PL8` on disk* — 44 on a shipped install, because the game
        // ships eleven of the fifteen files. We divide by all sixty. The
        // module header says what it would take to have the real one.
        let total = MAP_COUNT as i32;
        let pct_of = |a: i32, b: i32| if b == 0 { 0 } else { a * 100 / b };
        let pct = |x: i32, p: i32| p * x / 100;
        let top = self.map_top as i32;
        let rows = MAP_LIST_ROWS as i32;
        let above = pct(SCROLLBAR_H, pct_of(top, total));
        let below = pct(SCROLLBAR_H, pct_of(total - top - rows, total));
        // `iVar2 + ((0x2C - iVar4) - iVar2 - iVar3)`, which is the window's own
        // percentage plus whatever the three roundings lost — and simplifies to
        // the track minus the other two, exactly.
        let thumb = SCROLLBAR_H - above - below;
        for (y, h, colour) in [
            (SCROLLBAR_Y, above, font::DISABLED),
            (SCROLLBAR_Y + above, thumb, font::TEXT),
            (SCROLLBAR_Y + above + thumb, below, font::DISABLED),
        ] {
            if h != 0 {
                canvas.fill_rect(SCROLLBAR_X, y, SCROLLBAR_W, h, colour);
            }
        }
    }

    /// **What this build cannot honour, said on the page.**
    ///
    /// `docs/decisions.md` C21: a switch wired to nothing must not look
    /// finished. Two things go here — an option whose behaviour does not exist
    /// (*Exploration*), and the map, which the list can select and the world
    /// builder cannot yet build.
    ///
    /// **In our own font, never the original's**, for the same reason
    /// [`Screen::draw`]'s missing-background line is: nothing the original
    /// never drew may appear in its typeface, or a screenshot stops being
    /// evidence of anything.
    ///
    /// **The *NOT IMPLEMENTED* lines are debug overlay only**; the refusal to
    /// start a map is not, because without it the Start button silently does
    /// nothing.
    fn paint_gaps(&self, canvas: &mut Canvas, pen: &Pen, debug: bool) {
        let mut y = 462;
        let mut say = |line: &str| {
            l2_view::text::draw(canvas, MAP_LIST_X - 180, y, line, font::HIGHLIGHT);
            y += 9;
        };
        for &i in self.unhonoured.iter().filter(|_| debug) {
            let label = pen.assets.text(GROUP_OPTIONS, i).to_string();
            say(&format!("NOT IMPLEMENTED: {}", label.to_uppercase()));
        }
        // **The map line is gone**, and that is the point of this commit: the
        // slot the list names is now the world *Start* builds. What is left is
        // the case where it cannot be built at all.
        if let Some(why) = &self.failure {
            say(&format!("CANNOT START THIS MAP: {}", why.to_uppercase()));
        }
    }

    /// Page 9: the box the option opens, over the page underneath.
    pub(super) fn paint_dropdown(&self, canvas: &mut Canvas, pen: &Pen, _ctx: &Ctx) {
        // `DAT_00553FB4` is the item count and the box is that plus the two
        // border cells — the painter and the hit test read the one number, so a
        // *Nobles* list shortened to the map cannot draw four rows and accept
        // three.
        let n = self.rows(self.open);
        let (x, y, _) = OPTION_LIST[self.open];
        let y = self.dropdown_y(y, n);
        pen.window(canvas, x, y, 6, n as i32 + 2, 1);
        for i in 0..n {
            let s = pen.assets.text(GROUP_VALUES, OPTION_BASE[self.open] + i).to_string();
            let colour = if i == self.selected { font::HIGHLIGHT } else { font::TEXT };
            pen.body_centred(canvas, x + 1, y + 16 + i as i32 * 16, 0x60, &s, colour);
        }
    }

    /// Pages 11, 12 and 13. The battlefield itself is `l2-sim`'s, and the
    /// skirmish editor's palette of tools is `L2.eng` group 41; neither is
    /// drawn here. What is drawn is the page's own furniture: the background,
    /// and the three captions at `y = 0x1B8` that `FUN_0042051C` and
    /// `FUN_00420630` put there.
    fn paint_skirmish(&self, canvas: &mut Canvas, pen: &Pen, head: &Pen, page: SetupPage) {
        let items: [usize; 3] = match page {
            // 12 and 11: "Back", "Cust.", "Go".
            SetupPage::SkirmishFile => [0x24, 0x25, 0x26],
            _ => [0x25, 0x26, 0x24],
        };
        for (i, x) in [0x1CD, 0x207, 0x241].iter().enumerate() {
            pen.eng_centred(canvas, GROUP, items[i], *x, 0x1B8, 0x38, self.colour(i));
        }
        if page == SetupPage::SkirmishFile {
            // `FUN_0042150B` opens `Ui_DrawBox(0x60, 100, 0x1C, 0x12)` — border
            // set **0**, the only window on any of these pages that is not set
            // 1 — over the skirmish page, and draws group 40's file captions
            // into it. The rest is `FUN_00414E06(999)`: a parchment plate, the
            // same outline again, and up to ten `.skr` names at (128, 176 + 16n)
            // with the selected one on a `0x3F` bar. Nothing here reads a
            // directory of skirmish files, so the rows are absent and the box
            // they sit in is not.
            pen.window(canvas, 0x60, 100, 0x1C, 0x12, 0);
            head.eng_heading_centred(canvas, GROUP_FILE, 6, 0x60, 0x84, 0x1C0, font::TEXT);
            outline_rect(canvas, SKIRMISH_FILE_LIST, font::TEXT);
            pen.eng_centred(canvas, GROUP_FILE, 8, 0x60, 0x164, 0x1C0, font::TEXT);
            // `FUN_00414E06`'s own two, in its order: the parchment first and
            // **the same outline again** over it. The plate is 336 x 176 and
            // the outline 352 x 165, so the plate overhangs the bottom edge and
            // the second draw puts it back — the original's overdraw, kept.
            pen.box_interior(canvas, 0x7E, 0xAE, 0x15, 0xB);
            outline_rect(canvas, SKIRMISH_FILE_LIST, font::TEXT);
            // `Ui_OkButton(0x1F8, 0x164, 0)` — the last statement of the
            // painter, and the only page of the thirteen that has one.
            pen.ok_button(canvas, SKIRMISH_FILE_OK.0, SKIRMISH_FILE_OK.1, 0);
        }
    }
}


