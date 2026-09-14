#![allow(unused_imports)]
use super::*;
use super::drawing::*;
use std::collections::BTreeMap;
use l2_formats::Palette;
use l2_mods::vfs::Vfs;
use l2_view::sheet::Sheet;
use l2_view::Canvas;
use eng::Eng;
use font::Font;

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
        let loaded = ShellAssets {
            eng: read("L2.eng").and_then(|b| Eng::parse(b).ok()),
            // `&g_fontBody` is the one face `Glyph_Draw` compares against, and
            // raises three ranges of accented characters in — see
            // [`font::ACCENT_RAISE`]. The identity is the global, so it is
            // given here, where the file becomes that global.
            body: read(font::BODY).and_then(|b| Font::new(b, 16).ok()).map(Font::raising_accents),
            heading: read(font::HEADING).and_then(|b| Font::new(b, 24).ok()),
            small: read(font::SMALL).and_then(|b| Font::new(b, 12).ok()),
            // Twelve: every painter that uses `g_font8` steps its rows `0x0C`
            // apart — `BattleDebug_Panel`'s `0x2C, 0x38, 0x44 …` and
            // `Net_DrawDebugOverlay`'s `y + 4, y + 0x10, y + 0x1C …`.
            eight: read(font::EIGHT).and_then(|b| Font::new(b, 12).ok()),
            // No painter steps a line in `g_font10`: each of its nine call
            // sites is one number at an absolute `y`. Twelve is the digits'
            // ten rows and the drop shadow's one, rounded up the way `eight`'s is.
            ten: read(font::TEN).and_then(|b| Font::new(b, 12).ok()),
            sheets,
            palettes,
            merchant_grid: read_grid(&read, "mercgrid.pl8"),
            armoury_grid: read_grid(&read, "arm_grid.pl8"),
        };
        loaded.complain_about_what_is_missing();
        loaded
    }

    /// **Say so when the interface files did not load, because the game does
    /// not look broken without them — it looks badly made.**
    ///
    /// Every `Pen` method falls back to `l2_view::text`, our 5 × 7 debug font,
    /// when `body` or `heading` is `None`. That fallback is per-call and
    /// silent, so a checkout that cannot find its install renders the *entire*
    /// front end in a squat all-capitals font with every call site perfectly
/// correct. A player reported that as *"the title screen is
    /// illegible, all caps of that font is ridiculous"*, and there was nothing
    /// anywhere — no log line, no screen, no exit code — to distinguish it from
    /// a font we had chosen.
    ///
    /// This is `docs/agents.md`'s *a tool that degrades silently is worse the
/// more people use it*, in the shipped program. The
    /// degradation is still the right behaviour: the game must run on a bare
    /// checkout.
    fn complain_about_what_is_missing(&self) {
        let mut missing: Vec<&str> = self.missing_fonts();
        if self.eng.is_none() {
            missing.push("L2.eng");
        }
        if missing.is_empty() {
            return;
        }
        eprintln!(
            "lords2: could not load {} - the interface will be drawn in the 5x7 debug font \
             and captions will be our own English rather than the game's.",
            missing.join(", "),
        );
        eprintln!(
            "lords2: this is what a missing or unreadable game install looks like. Point \
             LORDS2_DIR at a Lords of the Realm II directory containing {} and {}.",
            font::BODY,
            font::HEADING,
        );
    }

    /// **Which of the game's five faces did not load** — the file names, or
    /// empty. Five because `Res_LoadStatic` preloads five; see [`Face`].
    ///
    /// It exists so that the *painter* and the *complaint* ask the same
    /// question. They used to be two lists: `complain_about_what_is_missing`
    /// checked `body` and `heading` and **not `small`**, so an install missing
    /// `Fntl2_9.pl8` alone said nothing at all and drew the build stamp — the
    /// one caption whose whole job is to be read back into a bug report — in
    /// the 5 × 7 font, silently. That is the same defect this method was added
    /// to announce, one file down.
    ///
    /// `docs/agents.md`: *two artefacts that must agree* is the pattern that
    /// catches things; one method both of them call is the shape where the
    /// mistake cannot be made at all.
    pub fn missing_fonts(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if self.body.is_none() {
            missing.push(font::BODY);
        }
        if self.heading.is_none() {
            missing.push(font::HEADING);
        }
        if self.small.is_none() {
            missing.push(font::SMALL);
        }
        if self.eight.is_none() {
            missing.push(font::EIGHT);
        }
        if self.ten.is_none() {
            missing.push(font::TEN);
        }
        missing
    }

    /// Nothing at all: what the tests run against, and what an install missing
    /// its interface files degrades to.
    pub fn empty() -> ShellAssets {
        ShellAssets {
            eng: None,
            body: None,
            heading: None,
            small: None,
            eight: None,
            ten: None,
            sheets: BTreeMap::new(),
            palettes: BTreeMap::new(),
            merchant_grid: Vec::new(),
            armoury_grid: Vec::new(),
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
/// position — and it is the one made by the artwork.
    pub fn merchant_grid(&self, x: i32, y: i32) -> Option<u8> {
        grid_cell(&self.merchant_grid, x, y)
    }

    /// Whether `mercgrid.pl8` was found, so a screen can say which hit test it
/// is using
    /// click.
    pub fn has_merchant_grid(&self) -> bool {
        self.merchant_grid.len() == MERCHANT_GRID_LEN
    }

    /// `FUN_0043582A`'s hit test: the **weapon type** whose rack is under a
    /// screen pixel, or `None`.
    ///
    /// The same arithmetic as [`ShellAssets::merchant_grid`] — the two painters
    /// read the same buffer with the same expression — and the cell value here
    /// is the basket slot, 1…6, which is also the [`l2_kingdom::unit::TroopType`]
    /// index of the man who carries that weapon.
    ///
    /// **The shipped grid has 26 cells that are not a rack**, holding 60…63:
    /// column 0 for the first fifteen rows, and an eleven-cell sliver at
    /// y 216…223 between x 512 and 599. In the original those are live — the
    /// hit test accepts any non-zero cell and the handler then indexes the
    /// eight-slot levy basket with 60-something. **We answer `None` for them**,
    /// because
    /// `docs/bugs.md` N13.
    pub fn armoury_grid(&self, x: i32, y: i32) -> Option<u8> {
        match grid_cell(&self.armoury_grid, x, y) {
            Some(t) if (1..=l2_kingdom::tables::WEAPON_TYPE_COUNT as u8).contains(&t) => Some(t),
            _ => None,
        }
    }

    /// Whether `arm_grid.pl8` was found. The armoury falls back to the six
    /// rectangles of its own hotspot table, which is a coarser hit test than
    /// the picture but is the original's too — see
    /// [`crate::screens::armoury::RACK_HOTSPOTS`].
    pub fn has_armoury_grid(&self) -> bool {
        self.armoury_grid.len() == MERCHANT_GRID_LEN
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
/// original. Screens report it so a
    /// reader of a screenshot can tell which they are looking at.
    pub fn has_artwork(&self) -> bool {
        self.eng.is_some() && self.body.is_some() && !self.sheets.is_empty()
    }
}

/// One `.pl8` region grid, header stripped, or empty when the file is missing
/// or the wrong size. `mercgrid.pl8` and `arm_grid.pl8` are byte-for-byte the
/// same shape and the original reads both into the same buffer.
fn read_grid(read: &impl Fn(&str) -> Option<Vec<u8>>, name: &str) -> Vec<u8> {
    read(name)
        .filter(|b| b.len() >= GRID_HEADER + MERCHANT_GRID_LEN)
        .map(|b| b[GRID_HEADER..GRID_HEADER + MERCHANT_GRID_LEN].to_vec())
        .unwrap_or_default()
}

/// `grid[(x >> 3) + (y >> 3) * 0x50]`, with a zero cell meaning nothing.
fn grid_cell(grid: &[u8], x: i32, y: i32) -> Option<u8> {
    if grid.len() != MERCHANT_GRID_LEN {
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
    match grid[row * MERCHANT_GRID_COLS + col] {
        0 => None,
        id => Some(id),
    }
}

/// The VFS is case-insensitive; this map is ours, so it has to be too. The
/// install spells the same file three ways (`SCORE1.PL8`, `Misc_sel.pl8`,
/// `Fntl2_14.pl8`) and the painters ask for a fourth.
pub(super) fn key(name: &str) -> String {
    name.to_ascii_lowercase()
}

