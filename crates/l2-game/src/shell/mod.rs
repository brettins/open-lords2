//! What a *shell* screen needs: the original's artwork, the original's strings,
//! the original's fonts, and the handful of primitives its painters call.
//!
//! # What a shell is, and what it is not
//!
//! Twenty-nine screens are identified in `docs/screens-county.md` §1 and five
//! are implemented. The rest are shells: each one loads **the `.pl8` its
//! painter loads** and draws **the `L2.eng` group its painter draws**, at the
//! coordinates read out of that painter, with nothing behind it. A shell is a
//! real surface with no logic, not a mock-up.
//!
//! The distinction matters because this project has thrown an invented
//! interface away once already — `screens/county.rs` opens by admitting it used
//! to be a made-up full-screen page listing twenty-two fields, built because
//! nobody had looked at what the original drew. Nothing in this module invents
//! a layout. Where a coordinate is genuinely unknown it says so and places the
//! thing plainly, so that a reader can tell a gap from a guess.
//!
//! # Every screen has its own palette
//!
//! The management screens run under the campaign palette. The front end does
//! not: `gateway.256`, `merchant.256`, `armoury.256`, `cas_back.256`,
//! `custom.256`, `skirmish.256` and `score1.256` are each read into the display
//! palette by the screen that wants them (`File_ReadChunk("gateway.256",
//! 0x004EA8A0, 0x300)` then `Palette_Set`). A canvas of palette indices is
//! meaningless without knowing which one, so [`Screen::palette`] names it and
//! the presenter asks the top screen rather than assuming.
//!
//! [`Screen::palette`]: crate::screen::Screen::palette

pub mod eng;
pub mod font;

use std::collections::BTreeMap;

use l2_formats::Palette;
use l2_mods::vfs::Vfs;
use l2_view::sheet::Sheet;
use l2_view::Canvas;

pub use eng::Eng;
pub use font::Font;

/// The full-screen background sheets, one 640 × 480 frame each, plus the
/// smaller sheets a shell draws on top of one.
///
/// Every name here is a literal in the painter that loads it. The list is
/// eager because it is small — the six full-screen backgrounds are 300 KB each
/// and [`Sheet`] decodes lazily, so what this costs is the read, not the
/// decode — and because a lazy cache would need interior mutability on a path
/// that `draw` is only allowed to see through `&`.
pub const SHEETS: &[&str] = &[
    // 0x1C the conquest screen and 0x1F pages 1..6, 10
    "Gateway.pl8",
    "Panels2.pl8",
    // 0x1F pages 7..9 — the custom game
    "Custom.pl8",
    // 0x1F pages 11..13 — skirmish
    "Skirmish.pl8",
    "Skircust.pl8",
    // the setup mode's icon sheet, g_miscCtySheet
    "Misc_sel.pl8",
    // 0x08 the merchant, and 0x0C trade goods, which draws over it
    "Merchant.pl8",
    "Mercgrid.pl8",
    "Icontrad.pl8",
    // 0x0A / 0x0D the armoury
    "Armoury.pl8",
    "Arm_grid.pl8",
    // 0x0B the other lords
    "Faces.pl8",
    // 0x1B castle building
    "Cas_back.pl8",
    "Caspics.pl8",
    "Cas_bits.pl8",
    // 0x2E the ratings
    "Score1.pl8",
    // 0x1D siege preparations
    "Sgeplans.pl8",
    // 0x11 army division
    "Icon_tmp.pl8",
];

/// The `.256` files those screens set as the display palette.
pub const PALETTES: &[&str] = &[
    "Gateway.256",
    "Custom.256",
    "Skirmish.256",
    "Misc_sel.256",
    "Merchant.256",
    "Armoury.256",
    "Cas_back.256",
    "Score1.256",
];

/// The artwork and text a shell screen draws with.
///
/// Everything is optional: a partial install, or a machine with no copy of the
/// game at all, still lays every screen out — with our own flat panels and
/// blank labels, and looking it. `docs/plan.md`'s rule that a stub should be
/// visibly ours rather than look finished applies here too.
pub struct ShellAssets {
    pub eng: Option<Eng>,
    pub body: Option<Font>,
    pub heading: Option<Font>,
    pub small: Option<Font>,
    sheets: BTreeMap<String, Sheet>,
    palettes: BTreeMap<String, Palette>,
    /// `mercgrid.pl8`'s 80 x 60 byte map, with its 24-byte header stripped.
    /// Empty when the file is not installed. See [`ShellAssets::merchant_grid`].
    merchant_grid: Vec<u8>,
}

/// `mercgrid.pl8` is an **80 x 60 grid of eight-pixel cells over the whole
/// screen**, one byte a cell holding the good id under it.
///
/// `File_ReadChunk("mercgrid.pl8", &g_villageGrid, 0x12D8, 0)` reads the file
/// whole — 4,824 bytes, which is these 4,800 cells plus a 24-byte `.pl8`
/// header — and `FUN_004357A6` then indexes it as
/// `grid[(x >> 3) + (y >> 3) * 0x50]`. It shares its buffer with the village's
/// own drop grid (`vill_gd8.pl8`, 45 x 40, 1,824 bytes) and with the armoury's
/// (`arm_grid.pl8`, also 4,824), which is why the buffer is named for the
/// village and read by three unrelated screens.
pub const MERCHANT_GRID_COLS: usize = 80;
pub const MERCHANT_GRID_ROWS: usize = 60;
pub const MERCHANT_GRID_CELL: i32 = 8;
pub const MERCHANT_GRID_LEN: usize = MERCHANT_GRID_COLS * MERCHANT_GRID_ROWS;
/// The bytes of a `.pl8` before its single frame's data.
pub const GRID_HEADER: usize = 24;

impl ShellAssets {
    pub fn load(vfs: &Vfs) -> ShellAssets {
        let read = |name: &str| vfs.read(name).ok();
        let mut sheets = BTreeMap::new();
        for name in SHEETS {
            if let Some(bytes) = read(name) {
                if let Ok(s) = Sheet::new(bytes) {
                    sheets.insert(key(name), s);
                }
            }
        }
        let mut palettes = BTreeMap::new();
        for name in PALETTES {
            if let Ok(p) = vfs.palette(name) {
                palettes.insert(key(name), p);
            }
        }
        ShellAssets {
            eng: read("L2.eng").and_then(|b| Eng::parse(b).ok()),
            body: read(font::BODY).and_then(|b| Font::new(b, 16).ok()),
            heading: read(font::HEADING).and_then(|b| Font::new(b, 24).ok()),
            small: read(font::SMALL).and_then(|b| Font::new(b, 12).ok()),
            sheets,
            palettes,
            merchant_grid: read("mercgrid.pl8")
                .filter(|b| b.len() >= GRID_HEADER + MERCHANT_GRID_LEN)
                .map(|b| b[GRID_HEADER..GRID_HEADER + MERCHANT_GRID_LEN].to_vec())
                .unwrap_or_default(),
        }
    }

    /// Nothing at all: what the tests run against, and what an install missing
    /// its interface files degrades to.
    pub fn empty() -> ShellAssets {
        ShellAssets {
            eng: None,
            body: None,
            heading: None,
            small: None,
            sheets: BTreeMap::new(),
            palettes: BTreeMap::new(),
            merchant_grid: Vec::new(),
        }
    }

    /// `FUN_004357A6`'s hit test: the good id under a screen pixel, or `None`
    /// when the pointer is on no ware — or when `mercgrid.pl8` is not installed,
    /// which the merchant screen answers with its own rectangles.
    ///
    /// **The shipped grid holds exactly twelve ids** — 1, 2, 4, 6, 7, 8, 9, 10,
    /// 11, 12, 13, 14. Sheep (3) and wool (5) appear in no cell, so there is
    /// nowhere on the stall to click for either. That is the fourth independent
    /// statement that the two goods are not in the game, after the missing
    /// `Merchant_Trade` branch, the price of zero and the (0, 0) plaque
    /// position — and it is the one made by the artwork rather than the code.
    pub fn merchant_grid(&self, x: i32, y: i32) -> Option<u8> {
        if self.merchant_grid.len() != MERCHANT_GRID_LEN {
            return None;
        }
        let (col, row) = (x / MERCHANT_GRID_CELL, y / MERCHANT_GRID_CELL);
        if col < 0 || row < 0 {
            return None;
        }
        let (col, row) = (col as usize, row as usize);
        if col >= MERCHANT_GRID_COLS || row >= MERCHANT_GRID_ROWS {
            return None;
        }
        match self.merchant_grid[row * MERCHANT_GRID_COLS + col] {
            0 => None,
            good => Some(good),
        }
    }

    /// Whether `mercgrid.pl8` was found, so a screen can say which hit test it
    /// is using rather than leaving a player to wonder why a ware will not
    /// click.
    pub fn has_merchant_grid(&self) -> bool {
        self.merchant_grid.len() == MERCHANT_GRID_LEN
    }

    pub fn sheet(&self, name: &str) -> Option<&Sheet> {
        self.sheets.get(&key(name))
    }

    pub fn palette(&self, name: &str) -> Option<&Palette> {
        self.palettes.get(&key(name))
    }

    /// One `L2.eng` string, or `""`.
    pub fn text(&self, group: usize, index: usize) -> &str {
        self.eng.as_ref().map_or("", |e| e.text(group, index))
    }

    /// Whether this install supplied enough for a shell to look like the
    /// original rather than like our placeholder. Screens report it so a
    /// reader of a screenshot can tell which they are looking at.
    pub fn has_artwork(&self) -> bool {
        self.eng.is_some() && self.body.is_some() && !self.sheets.is_empty()
    }
}

/// The VFS is case-insensitive; this map is ours, so it has to be too. The
/// install spells the same file three ways (`SCORE1.PL8`, `Misc_sel.pl8`,
/// `Fntl2_14.pl8`) and the painters ask for a fourth.
fn key(name: &str) -> String {
    name.to_ascii_lowercase()
}

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
/// caller-supplied sheet rather than from `Panels.pl8`.
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

/// A pen: the two fonts, the emboss colours the screen wants, and a fallback.
///
/// Every shell screen draws through one of these so that the *same* painter
/// works on a full install and on a machine with no copy of the game. With the
/// fonts present it is the original's text, embossed the original's way; with
/// them absent it is `l2_view::text`'s 5 × 7 font in the palette-resolved
/// interface colours, which lays out in the same places and is obviously ours.
#[derive(Clone, Copy)]
pub struct Pen<'a> {
    pub assets: &'a ShellAssets,
    pub ink: &'a l2_view::Ink,
    /// `Panels.pl8` and the button sheet, when the install has them. Several
    /// shells draw a `Ui_DrawBox` from that kit over their own background.
    pub chrome: Option<&'a l2_view::chrome::Chrome>,
    /// `Ui_DrawText`'s two shadow colours — [`font::SHADOW`] on most screens,
    /// [`font::SHADOW_GATEWAY`] on the setup and conquest pages — or `None`
    /// for flat text, which is `DAT_005AEA40 != 0`.
    pub shadow: Option<(u8, u8)>,
    /// `DAT_0058FE2C`: the colour `A` … `Z` are drawn in instead of the
    /// caller's. Always 1 where the original sets it.
    pub caps: Option<u8>,
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
    /// What the fallback font uses when there is no `Fntl2_*.pl8` to draw with.
    /// The original's colour indices mean nothing under our own palette, so
    /// they are mapped to the three named interface colours instead.
    fn fallback(&self, colour: u8) -> u8 {
        match colour {
            font::HIGHLIGHT => self.ink.highlight,
            font::DISABLED => self.ink.dim,
            _ => self.ink.text,
        }
    }

    pub fn body(&self, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) -> i32 {
        match &self.assets.body {
            Some(f) => f.draw(canvas, x, y, s, &self.style(colour)),
            None => l2_view::text::draw(canvas, x, y, s, self.fallback(colour)),
        }
    }

    pub fn heading(&self, canvas: &mut Canvas, x: i32, y: i32, s: &str, colour: u8) -> i32 {
        match &self.assets.heading {
            Some(f) => f.draw(canvas, x, y, s, &self.style(colour)),
            None => l2_view::text::draw(canvas, x, y, s, self.fallback(colour)),
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
    /// space from every line but the first, which is why a wrapped paragraph in
    /// the original has no ragged left edge.
    ///
    /// The custom game's twelve option labels go through this at a width of
    /// 100, which is why *"Advanced Farming"* is two lines and not one long one
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

    /// An `L2.eng` string, drawn in the body font.
    pub fn eng(&self, canvas: &mut Canvas, group: usize, index: usize, x: i32, y: i32, colour: u8) {
        let s = self.assets.text(group, index).to_string();
        self.body(canvas, x, y, &s, colour);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sheet_and_palette_name_is_unique_once_folded() {
        let mut seen: Vec<String> = SHEETS.iter().map(|n| key(n)).collect();
        seen.sort();
        let before = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), before, "two SHEETS entries fold to the same key");
        let mut seen: Vec<String> = PALETTES.iter().map(|n| key(n)).collect();
        seen.sort();
        let before = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), before);
    }

    #[test]
    fn an_empty_shell_answers_everything_without_panicking() {
        let a = ShellAssets::empty();
        assert!(!a.has_artwork());
        assert_eq!(a.text(11, 0), "");
        assert!(a.sheet("Gateway.pl8").is_none());
        assert!(a.palette("Gateway.256").is_none());
        let mut c = Canvas::screen();
        assert!(!background(&mut c, &a, "Gateway.pl8"));
        assert_eq!(c.count(0), 640 * 480, "and it drew nothing at all");
    }

    #[test]
    fn the_recess_lights_its_top_and_right_and_shades_its_bottom_and_left() {
        let mut c = Canvas::new(20, 10);
        button_recess(&mut c, 2, 2, 10, 6);
        assert_eq!(c.at(2, 2), 0x28, "the top-left corner belongs to the left edge");
        assert_eq!(c.at(6, 2), 0x35, "top");
        assert_eq!(c.at(11, 4), 0x35, "right");
        assert_eq!(c.at(6, 7), 0x28, "bottom");
        assert_eq!(c.at(2, 4), 0x28, "left");
        assert_eq!(c.at(6, 4), 0, "and the middle is left alone");
    }
}
