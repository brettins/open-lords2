#![allow(unused_imports)]
use super::*;
use super::assets::*;
use std::collections::BTreeMap;
use l2_formats::Palette;
use l2_mods::vfs::Vfs;
use l2_view::sheet::Sheet;
use l2_view::Canvas;
use eng::Eng;
use font::Font;

// ------------------------------------------------------------------ painting

/// A full-screen background: frame 0 of a sheet whose only frame is 640 × 480.
///
/// `FUN_00408FCB(name, 0x1E0)` reads one of these straight into the display
/// buffer — the `0x1E0` is 480, the row count. Returns false when the sheet is
/// missing, so the caller can fill instead of drawing nothing.
pub fn background(canvas: &mut Canvas, assets: &ShellAssets, name: &str) -> bool {
    let Some(sheet) = assets.sheet(name) else { return false };
    let Some(frame) = sheet.frame(0) else { return false };
    canvas.blit_opaque(&frame, 0, 0);
    true
}

/// `FUN_00409346(sheet, x, y, cols, rows)` — a framed box drawn from a
/// caller-supplied sheet.
///
/// **[V]** `Panels2.pl8` has the same frame layout as `Panels.pl8`: four
/// corners, four twelve-frame edges, then the 144-frame interior field at 0x34.
/// The function is `Ui_DrawBoxBorder(0, …)` reading from `sheet` plus
/// `Ui_DrawBoxInterior` inset by one cell, which is exactly what
/// `l2_view::chrome::Chrome::draw_box` already does for `Panels.pl8`.
///
/// Sizes are in 16-pixel cells, and the box includes its border: a box of
/// `cols` × `rows` covers `cols * 16` by `rows * 16` pixels.
pub fn box_from(canvas: &mut Canvas, sheet: &Sheet, x: i32, y: i32, cols: i32, rows: i32) {
    use l2_view::chrome::panels;
    let cell = panels::CELL;
    for r in 0..rows {
        for c in 0..cols {
            let frame = if r == 0 && c == 0 {
                panels::CORNER_TL
            } else if r == 0 && c == cols - 1 {
                panels::CORNER_TR
            } else if r == rows - 1 && c == 0 {
                panels::CORNER_BL
            } else if r == rows - 1 && c == cols - 1 {
                panels::CORNER_BR
            } else if r == 0 {
                panels::EDGE_TOP + (c as usize - 1) % panels::EDGE_LEN
            } else if r == rows - 1 {
                panels::EDGE_BOTTOM + (c as usize - 1) % panels::EDGE_LEN
            } else if c == 0 {
                panels::EDGE_LEFT + (r as usize - 1) % panels::EDGE_LEN
            } else if c == cols - 1 {
                panels::EDGE_RIGHT + (r as usize - 1) % panels::EDGE_LEN
            } else {
                panels::TEXTURE
                    + (c as usize - 1) % panels::TEXTURE_DIM
                    + ((r as usize - 1) % panels::TEXTURE_DIM) * panels::TEXTURE_DIM
            };
            if let Some(f) = sheet.frame(frame) {
                canvas.blit(&f, x + c * cell, y + r * cell);
            }
        }
    }
}

/// `Ui_DrawInsetRect` (`0x00403DEB`) — **four lines and no fill.**
///
/// Colour `0x10` along the top and right edges, `0x1F` along the bottom and
/// left, clipped to the screen. That is the whole function, and the *no fill*
/// is the part worth stating: every one of these on the raise-army screen sits
/// on the panel's own parchment, so a caller that filled the rectangle first —
/// as this crate's did — painted a black hole in the middle of a window. It
/// went unseen because the fill used the interface's `background` index, which
/// is the panel colour under our own palette and pitch black under
/// `armoury.256`. `docs/decisions.md` C61.
pub fn inset_rect(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    const TOP_RIGHT: u8 = 0x10;
    const BOTTOM_LEFT: u8 = 0x1F;
    if w < 1 || h < 1 {
        return;
    }
    canvas.fill_rect(x, y, w, 1, TOP_RIGHT);
    canvas.fill_rect(x + w - 1, y, 1, h, TOP_RIGHT);
    canvas.fill_rect(x, y + h - 1, w, 1, BOTTOM_LEFT);
    canvas.fill_rect(x, y, 1, h, BOTTOM_LEFT);
}

/// The recessed rectangle the setup pages put every menu item in —
/// `FUN_00403EE4(x, y, w, h)`. **[D]** from its own body: the top and right
/// edges are colour `0x35` and the bottom and left `0x28`, which is the
/// opposite lighting to `Ui_DrawInsetRect` (`0x00403DEB`, `0x10` and `0x1F`)
/// and reads as *raised* under `gateway.256`.
///
/// Pixels, not cells.
pub fn button_recess(canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32) {
    const LIGHT: u8 = 0x35;
    const DARK: u8 = 0x28;
    canvas.fill_rect(x, y, w, 1, LIGHT);
    canvas.fill_rect(x + w - 1, y, 1, h, LIGHT);
    canvas.fill_rect(x, y + h - 1, w, 1, DARK);
    canvas.fill_rect(x, y, 1, h, DARK);
}

impl<'a> Pen<'a> {
    /// The same pen with the emboss switched off — `DAT_005AEA40 = 1`, which
    /// is what the front end sets around every menu item and body line.
    pub fn flat(&self) -> Pen<'a> {
        Pen { shadow: None, caps: None, ..*self }
    }

    /// The same pen with the drop-capital colour on — `DAT_0058FE2C = 1`.
    pub fn drop_caps(&self) -> Pen<'a> {
        Pen { caps: Some(1), ..*self }
    }

    fn style(&self, colour: u8) -> font::Style {
        font::Style { colour, shadow: self.shadow, caps: self.caps }
    }
    /// What the fallback font uses when
    /// The original's colour indices mean nothing under our own palette, so
    /// they are mapped to the three named interface colours instead.
    fn fallback(&self, colour: u8) -> u8 {
        match colour {
            font::HIGHLIGHT => self.ink.highlight,
            font::DISABLED => self.ink.dim,
            _ => self.ink.text,
        }
    }

    /// One line in the body font. **Returns the x the next glyph would go at**,
    /// so a caller building a sentence out of pieces can hand the answer
    /// straight back in.
    ///
    /// **
    /// [`Font::draw`] returns `pen - x`, the *advance* — which is the
    /// original's `g_penAdvance`, and correct there, because every call site in
    /// the binary reads it as `Eng_DrawString(…, g_penAdvance + 0x70, …)`.
    /// `l2_view::text::draw`, the fallback, returns the absolute x. So this one
    /// method meant two different things depending on whether the install had
    /// `Fntl2_14.pl8` in it, and **every caller in this crate reads it as
    /// absolute** — nine of them, in three screens. With no artwork they were
    /// right and with artwork the second half of each sentence landed on top of
    /// the first. Found by looking at the armoury with the game's own fonts;
    /// `docs/decisions.md` C61.
    pub fn body(&self, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) -> i32 {
        match &self.assets.body {
            Some(f) => x + f.draw(canvas, x, y, s, &self.style(colour)) + TRAILING,
            None => l2_view::text::draw(canvas, x, y, s, self.fallback(colour)) + TRAILING,
        }
    }

    /// The same in the heading font, and the same return.
    pub fn heading(&self, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) -> i32 {
        match &self.assets.heading {
            Some(f) => x + f.draw(canvas, x, y, s, &self.style(colour)) + TRAILING,
            None => l2_view::text::draw(canvas, x, y, s, self.fallback(colour)) + TRAILING,
        }
    }

    pub fn body_centred(
        &self,
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        width: i32,
        s: &str,
        colour: u8,
    ) -> i32 {
        match &self.assets.body {
            Some(f) => f.draw_centred(canvas, x, y, width, s, &self.style(colour)),
            None => {
                let w = l2_view::text::width(s);
                let off = ((width - w) / 2).max(0);
                l2_view::text::draw(canvas, x + off, y, s, self.fallback(colour))
            }
        }
    }

    pub fn heading_centred(
        &self,
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        width: i32,
        s: &str,
        colour: u8,
    ) -> i32 {
        match &self.assets.heading {
            Some(f) => f.draw_centred(canvas, x, y, width, s, &self.style(colour)),
            None => {
                let w = l2_view::text::width(s);
                let off = ((width - w) / 2).max(0);
                l2_view::text::draw(canvas, x + off, y, s, self.fallback(colour))
            }
        }
    }

    /// `FUN_0040328E(group, index, x, y, width, …)` — one `L2.eng` string,
    /// **wrapped** to `width` and stepped down a line each time.
    ///
    /// **[V]** the step: the function ends with
    /// `if (font == &g_fontHeading) y += 0x18; else y += 0x10;` — 24 pixels for
    /// the 22-pixel font and 16 for the 14-pixel one. It also strips a leading
/// space from every line but the first, so a wrapped paragraph in
    /// the original has no ragged left edge.
    ///
    /// The custom game's twelve option labels go through this at a width of
/// 100, so *"Advanced Farming"* is two lines and not one long one
    /// running into its neighbour.
    pub fn body_wrapped(
        &self,
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        width: i32,
        s: &str,
        colour: u8,
    ) -> i32 {
        let mut line = y;
        for text in self.wrap(s, width) {
            self.body(canvas, x, line, &text, colour);
            line += self.assets.body.as_ref().map_or(16, |f| f.line);
        }
        line - y
    }

    /// Greedy word wrap at the font's own measured widths.
    pub fn wrap(&self, s: &str, width: i32) -> Vec<String> {
        let measure = |t: &str| -> i32 {
            match &self.assets.body {
                Some(f) => f.width(t),
                None => l2_view::text::width(t),
            }
        };
        let mut out: Vec<String> = Vec::new();
        let mut line = String::new();
        for word in s.split_whitespace() {
            let candidate =
                if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && measure(&candidate) > width {
                out.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
        if out.is_empty() {
            out.push(String::new());
        }
        out
    }

    /// An `L2.eng` string, drawn in the body font. Returns where it ended, like
    /// [`Pen::body`], because the original's sentences are built out of a
    /// string and a number and a string.
    pub fn eng(
        &self,
        canvas: &mut Canvas,
        group: usize,
        index: usize,
        x: i32,
        y: i32,
        colour: u8,
    ) -> i32 {
        let s = self.assets.text(group, index).to_string();
        self.body(canvas, x, y, &s, colour)
    }

    /// `Eng_DrawString(group, index, x, y, font, colour)` in the face the call
    /// site names, with [`Pen::body`]'s return. [`Pen::eng`] is this with
    /// [`Face::Body`], and a call site that passes `&g_fontHeading` is not that.
    #[allow(clippy::too_many_arguments)]
    pub fn eng_in(
        &self,
        face: Face,
        canvas: &mut Canvas,
        group: usize,
        index: usize,
        x: i32,
        y: i32,
        colour: u8,
    ) -> i32 {
        let s = self.assets.text(group, index).to_string();
        self.text_in(face, canvas, x, y, &s, colour)
    }

    /// `Ui_DrawCentred(group, index, x, y, width, body, colour)`.
    #[allow(clippy::too_many_arguments)]
    pub fn eng_centred(
        &self,
        canvas: &mut Canvas,
        group: usize,
        index: usize,
        x: i32,
        y: i32,
        width: i32,
        colour: u8,
    ) {
        let s = self.assets.text(group, index).to_string();
        self.body_centred(canvas, x, y, width, &s, colour);
    }

    /// `Ui_DrawBox`/`FUN_004093E0`: a framed window from `Panels.pl8`, border
    /// set 0 or 1. Falls back to a flat plate in the interface colours, which
    /// is visibly ours.
    pub fn window(&self, canvas: &mut Canvas, x: i32, y: i32, cols: i32, rows: i32, set: usize) {
        match self.chrome {
            Some(c) => c.draw_box(canvas, x, y, cols, rows, set),
            None => {
                canvas.fill_rect(x, y, cols * 16, rows * 16, self.ink.panel);
                canvas.fill_rect(x, y, cols * 16, 1, self.ink.border);
                canvas.fill_rect(x, y + rows * 16 - 1, cols * 16, 1, self.ink.border);
                canvas.fill_rect(x, y, 1, rows * 16, self.ink.border);
                canvas.fill_rect(x + cols * 16 - 1, y, 1, rows * 16, self.ink.border);
            }
        }
    }

    /// `Ui_DrawBoxInterior(x, y, cols, rows)` — **the parchment on its own,
    /// with no border round it.** The armoury's rack panel draws two of these
/// as wells inside a window it has already drawn, so the border
    /// half would be wrong.
    ///
    /// `Ui_DrawBox` is `Ui_DrawBoxBorder(1, …)` followed by this inset one
    /// cell, so the tiling is the same 12 × 12 field at frame `0x34` and only
    /// the edges are missing.
    pub fn box_interior(&self, canvas: &mut Canvas, x: i32, y: i32, cols: i32, rows: i32) {
        use l2_view::chrome::panels;
        let Some(chrome) = self.chrome else {
            canvas.fill_rect(x, y, cols * panels::CELL, rows * panels::CELL, self.ink.panel);
            return;
        };
        for r in 0..rows {
            for c in 0..cols {
                let frame = panels::TEXTURE
                    + (c as usize) % panels::TEXTURE_DIM
                    + ((r as usize) % panels::TEXTURE_DIM) * panels::TEXTURE_DIM;
                chrome.draw_panel_frame(canvas, frame, x + c * panels::CELL, y + r * panels::CELL);
            }
        }
    }

    /// The same, but from a sheet the caller names — `FUN_00409346`, which the
    /// setup pages use to draw their windows out of `Panels2.pl8`.
    pub fn window_from(
        &self,
        canvas: &mut Canvas,
        sheet: &str,
        x: i32,
        y: i32,
        cols: i32,
        rows: i32,
    ) {
        match self.assets.sheet(sheet) {
            Some(s) => box_from(canvas, s, x, y, cols, rows),
            None => self.window(canvas, x, y, cols, rows, 0),
        }
    }

    // --------------------------------------------------- numbers and plates
    //
    // The management screens are built out of four calls this crate did not
    // have: `Ui_DrawNumber`, `Ui_DrawCount`, `Ui_DrawNumberRight` and
    // `Ui_DrawInsetRect`, plus the two sheet blits every one of them uses.
    // Five screens graduated in one session needing all six, so they live here

    /// `Ui_DrawInsetRect(x, y, w, h)` — the recessed well, in **pixels**.
    pub fn inset(&self, canvas: &mut Canvas, r: crate::input::Rect) {
        inset_rect(canvas, r.x, r.y, r.w, r.h);
    }

    /// `FUN_00403CF4(x, y, w, h, colour)` — **a one-pixel rectangle outline in
    /// one palette index**, which is four `FUN_00403A8F` line draws.
    ///
    /// It is a primitive of the original's and not a widget of ours, which is
/// the whole reason it lives here.
    /// The two functions are the same four `fill_rect`s; what differs is the
    /// **colour argument**, and that is what decides whether a call reproduces
    /// something or invents it. `FUN_00403CF4` takes a literal palette index out
    /// of the painter — `Diplo_DrawLordCard`'s selected card is `0xF9` inside
    /// `0x3F` — where `widget::frame` takes one of the interface's own `Ink`
    /// colours, which mean nothing under the game's palettes.
    ///
    /// So: **`pen.outline` where the decompilation shows the call, with its own
    /// literal; `widget::frame` only as the picture-is-missing fallback.** The
    /// draw audit counts the first as real and the second as ours, and until
    /// this method existed there was no way to write the first — three of
    /// `screendraws.js`'s leaves (`FUN_00403CF4`, `FUN_0040437D`,
    /// `FUN_00403A8F`) count on the original's side and had no counterpart on
    /// ours.
    pub fn outline(&self, canvas: &mut Canvas, x: i32, y: i32, w: i32, h: i32, colour: u8) {
        if w < 1 || h < 1 {
            return;
        }
        canvas.fill_rect(x, y, w, 1, colour);
        canvas.fill_rect(x, y + h - 1, w, 1, colour);
        canvas.fill_rect(x, y, 1, h, colour);
        canvas.fill_rect(x + w - 1, y, 1, h, colour);
    }

    /// One line in `face`, with [`Pen::body`]'s return.
    pub fn text_in(
        &self,
        face: Face,
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        s: &str,
        colour: u8,
    ) -> i32 {
        match face {
            Face::Body => self.body(canvas, x, y, s, colour),
            Face::Heading => self.heading(canvas, x, y, s, colour),
            Face::Eight => match &self.assets.eight {
                Some(f) => x + f.draw(canvas, x, y, s, &self.style(colour)) + TRAILING,
                None => l2_view::text::draw(canvas, x, y, s, self.fallback(colour)) + TRAILING,
            },
        }
    }

    /// `Ui_DrawNumber(value, lead, suffix, x, y, font, colour)` **as the call
    /// site wrote it** — its own lead character, its own suffix string, its own
    /// face. **The only way this crate draws one.**
    ///
    /// `Ui_DrawNumber` (`0x00402F64`) writes the digits from index 1 of
    /// `g_numberBuffer`, puts `lead` at index 0, appends `suffix` and makes one
    /// `Ui_DrawText` of the lot. So `x` is where the *lead* goes and the digits
    /// start one lead-advance to its right: four pixels for `' '` and for `'@'`
    /// alike, since neither has a glyph.
    ///
    /// **There used to be a `Pen::number(…, blank_lead: bool)` beside this**,
    /// which mapped `true` to *no lead* and `false` to `' '` and hard-coded a
    /// `" "` suffix. It had 25 call sites and every one was read against its
    /// original: all 19 `true` sites pass `'@'` (16 with an empty suffix, 3
    /// with one space) and all six `false` sites pass `' '` and `" "`. A choice
    /// the original does not offer cannot be made correctly, so it is gone.
    /// `docs/decisions.md` C155.
    #[allow(clippy::too_many_arguments)]
    pub fn number_in(
        &self,
        face: Face,
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        value: i32,
        lead: char,
        suffix: &str,
        colour: u8,
    ) -> i32 {
        self.text_in(face, canvas, x, y, &format!("{lead}{value}{suffix}"), colour)
    }

    /// `Ui_DrawYear(year, x, y, style)` (`0x0041A900`) — a year, and for three
    /// of its four styles the `L2.eng` group 26 era after or before it. **[V]**
    ///
    /// ```c
    /// style 0, 2: Ui_DrawNumber(|year|, ' ', " ", x, y, body);
    ///             Eng_DrawString(26, year < 0 ? 0 : 1, x + g_penAdvance, y, body);
    /// style 1:    Eng_DrawString(26, year < 0 ? 0 : 1, x, y, heading);
    ///             Ui_DrawNumber(|year|, ' ', " ", x + g_penAdvance,
    ///                           year < 0 ? y : y - 1, heading);
    /// style 3:    Ui_DrawNumber(|year|, ' ', " ", x, y, body);
    /// ```
    ///
    /// Group 26 is *"BC"*, *"AD"*. All eight suffixes, `&DAT_004D41D4` …
    /// `&DAT_004D41F0`, are one space, read out of the image. The function
/// zeroes `g_penAdvance` first and the era is placed from it, so
    /// the number's own lead, digits, suffix and trailer all come before it.
    ///
    /// Returns where the next glyph goes, like every `Pen` method.
    #[allow(clippy::too_many_arguments)]
    pub fn year(&self, canvas: &mut Canvas, x: i32, y: i32, year: i32, style: u8, colour: u8) -> i32 {
        let era = if year < 0 { YEAR_BC } else { YEAR_AD };
        let digits = year.abs();
        match style {
            1 => {
                let era = self.assets.text(YEAR_GROUP, era).to_string();
                let next = self.heading(canvas, x, y, &era, colour);
                let dy = if year < 0 { 0 } else { -1 };
                self.number_in(Face::Heading, canvas, next, y + dy, digits, ' ', " ", colour)
            }
            3 => self.number_in(Face::Body, canvas, x, y, digits, ' ', " ", colour),
            _ => {
                let next = self.number_in(Face::Body, canvas, x, y, digits, ' ', " ", colour);
                self.eng(canvas, YEAR_GROUP, era, next, y, colour)
            }
        }
    }

    /// `Ui_DrawCount(value, nounIndex, x, y, &g_fontBody, colour)` — a number and
    /// then the `L2.eng` **group 8** noun that goes with it.
    ///
    /// Group 8 holds its nouns in pairs, singular then plural, and the original
    /// picks `nounIndex` for one and `nounIndex + 1` for anything else —
    /// including **zero**, which takes the plural. That is worth stating
    /// because the obvious implementation gets it wrong: *"0 Crowns."*, not
    /// *"0 Crown."*
    ///
    /// The body font is the face 78 of the 80 call sites draw in: **70** name
    /// `&g_fontBody` at the call, and **8** are inside `FUN_004224E7`, which
    /// takes its font as `param_5` — and all three of its callers
    /// (`UnitPanel_Draw` and two in the battle roster) pass `&g_fontBody`. The
    /// other two name `&g_fontHeading` — `Court_Draw`'s treasury and the
    /// armoury's troop count — and are [`Pen::count_in`] with
    /// [`Face::Heading`]. Counted over the decompilation and checked against
    /// the 80 `CALL 0x0041AB67` in the shipped exe. **[V]**
    pub fn count(
        &self,
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        value: i32,
        noun: usize,
        colour: u8,
    ) -> i32 {
        self.count_in(Face::Body, canvas, x, y, value, noun, colour)
    }

    /// [`Pen::count`] in the face the call site names.
    #[allow(clippy::too_many_arguments)]
    pub fn count_in(
        &self,
        face: Face,
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        value: i32,
        noun: usize,
        colour: u8,
    ) -> i32 {
        let s = self.assets.text(COUNT_NOUN_GROUP, count_noun(value, noun)).to_string();
        self.count_with_noun(face, canvas, x, y, value, &s, colour)
    }

    /// `Ui_DrawCount`'s two draws with the noun already chosen — for a caller
    /// that keeps an English fallback for an install with no `L2.eng`.
    ///
    /// **The lead and the suffix are not the caller's to pick.**
    /// `Ui_DrawCount` (`0x0041AB67`) is
    ///
    /// ```c
    /// Ui_DrawNumber(value, '@', &DAT_004D41F4, x, y, font, colour);
    /// Eng_DrawString(8, noun, x + g_penAdvance, y, font, colour);
    /// ```
    ///
    /// for all 80 of its call sites, and `DAT_004D41F4` is a NUL — read out of
    /// the shipped `Lords2.exe` at file offset `0xD23F4`. So the string is always
    /// `"@1000"`: the digits start [`font::SPACE_ADVANCE`] right of `x`, and the
    /// noun starts one [`TRAILING`] after the last digit. **[V]**
    ///
    /// This method used to take a `blank_lead: bool` and hand it to
    /// `Pen::number`, a choice the original does not have. Both answers were
    /// wrong, in opposite halves: `true` (eleven sites, the menu bar's treasury
    /// among them) drew no lead and invented a trailing space, so the digits sat
    /// four pixels left and the noun landed right by coincidence; `false` (six
    /// sites) drew the lead and the invented space, so the digits were right and
    /// the noun four pixels too far. `docs/decisions.md`
    /// C155.
    ///
    /// **`next`, not `x + next`.** Every pen method returns the *absolute* x the
    /// following glyph occupies, and this line once added `x` a second time, so
    /// the noun landed `x` pixels right of the number. Measured then:
    /// `number(x = 100, 5)` returned 116 and `count(x = 100, 5)` put its noun at
    /// **216**.
    #[allow(clippy::too_many_arguments)]
    pub fn count_with_noun(
        &self,
        face: Face,
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        value: i32,
        noun: &str,
        colour: u8,
    ) -> i32 {
        let next = self.number_in(face, canvas, x, y, value, COUNT_LEAD, COUNT_SUFFIX, colour);
        self.text_in(face, canvas, next, y, noun, colour)
    }

    /// `Ui_DrawNumberRight(value, lead, suffix, x, y, width, font, colour)` —
    /// which **does not right-align**.
    ///
    /// Its whole body after building the string is `FUN_004025D7`, and that is
    /// `local_c = (width - textWidth) / 2; if (local_c < 0) local_c = 0;` — the
    /// *same helper* `Ui_DrawCentred` calls. `docs/symbols.json` names it
    /// *"Ui_DrawNumber, right-aligned inside width"* and that is wrong for
    /// every caller in the binary. The name is kept here because it is the
    /// name in the database; the behaviour is the code's. **[V]**
    ///
    /// # `lead` and `suffix` are the call site's, and they move the digits
    ///
    /// The string the original centres is `lead + digits + suffix`, built in
    /// `g_numberBuffer`, and `FUN_004025D7` measures **that whole string**:
    /// `FUN_004014F0` charges [`font::SPACE_ADVANCE`] for a space, charges
    /// `frameWidth + 1` for every other glyph, and trims nothing at either end.
    /// So a suffix we invent widens the measure by four and moves the digits
    /// **two pixels left** of where the original puts them — the same defect as
    /// the anchoring one it replaced, at a quarter of the size.
    ///
    /// This method used to build `" {value} "` for every caller. Measured over
    /// the twenty `Ui_DrawNumberRight` call sites in the image: **every one
    /// passes `' '` as the lead, fifteen pass a one-space suffix, and the five
    /// on `Panel_Ration` pass the empty string** — `&DAT_004D3E04`,
    /// `…08`, `…0C`, `…10`, `…14`, five addresses in a run of zero bytes ending
    /// where `"villani1.pl8"` begins, so each is a NUL and each suffix is empty.
    /// Our ration panel drew all five two pixels left.
    /// `docs/decisions.md` C140. **[V]**
    #[allow(clippy::too_many_arguments)]
    pub fn number_centred(
        &self,
        canvas: &mut Canvas,
        x: i32,
        y: i32,
        width: i32,
        value: i32,
        lead: char,
        suffix: &str,
        colour: u8,
    ) {
        self.body_centred(canvas, x, y, width, &format!("{lead}{value}{suffix}"), colour);
    }

    /// `Pl8_DrawFrame(g_miscCtySheet, frame, x, y)` — the county sheet, which
    /// is `Misc_cty.pl8` in campaign mode.
    ///
    /// **The slot is not always that file.** `g_miscCtySheet` (`0x005530C8`)
    /// holds `misc_cty.pl8`, `misc_bat.PL8`, `misc_ske.PL8` or `misc_sel.PL8`
    /// depending on `DAT_0053F050`, so a frame number is only meaningful with
    /// the mode beside it. Everything drawn through *this* helper is campaign
    /// mode; the skirmish screens name their sheet.
    pub fn misc_frame(&self, canvas: &mut Canvas, frame: usize, x: i32, y: i32) -> bool {
        self.chrome.is_some_and(|c| c.draw_misc(canvas, frame, x, y))
    }

    /// `Pl8_DrawFrame(g_systemSheet, frame, x, y)` — the button sheet.
    pub fn system_frame(&self, canvas: &mut Canvas, frame: usize, x: i32, y: i32) -> bool {
        self.chrome.is_some_and(|c| c.draw_system(canvas, frame, x, y))
    }

    /// `Ui_OkButton(x, y, mode)` — the corner picture that closes a panel, with
    /// our own recess where the sheet is missing. Mode 0 is `System.pl8` frame
    /// `0x33`, mode 1 is frame `0x10`.
    pub fn ok_button(&self, canvas: &mut Canvas, x: i32, y: i32, mode: usize) {
        let frame =
            if mode == 0 { l2_view::chrome::system::OK } else { l2_view::chrome::system::OK_ALT };
        if !self.system_frame(canvas, frame, x, y) {
            button_recess(canvas, x, y, 24, 24);
        }
    }

    /// The same in the heading font.
    #[allow(clippy::too_many_arguments)]
    pub fn eng_heading_centred(
        &self,
        canvas: &mut Canvas,
        group: usize,
        index: usize,
        x: i32,
        y: i32,
        width: i32,
        colour: u8,
    ) {
        let s = self.assets.text(group, index).to_string();
        self.heading_centred(canvas, x, y, width, &s, colour);
    }
}

