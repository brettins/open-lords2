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
    // 0x0A / 0x0D the armoury. `Arm_it_<c>.pl8` is the one sheet the armoury
    // and the raise-army screen share — the weapons on the walls, the eight
    // troop portraits along the bottom, and the six little weapon icons the
    // levy screen prints its stocks beside. Which of the five is loaded is the
    // realm's `shield_index`; see [`crate::screens::armoury::items_sheet`].
    "Armoury.pl8",
    "Arm_grid.pl8",
    "Arm_it_r.pl8",
    "Arm_it_y.pl8",
    "Arm_it_k.pl8",
    "Arm_it_p.pl8",
    "Arm_it_b.pl8",
    // 0x0D, one per weapon type: a 24-frame 100 x 100 animation of the weapon
    // being made. `Armoury_LoadScreen` reads exactly one of them, chosen by
    // `DAT_00553F20`, the rack the player clicked.
    "Arm_cros.pl8",
    "Arm_mace.pl8",
    "Arm_swor.pl8",
    "Arm_pike.pl8",
    "Arm_bow.pl8",
    "Arm_mail.pl8",
    // 0x0A / 0x0D, the room in motion: two guttering torches and the soldier
    // who walks over and takes the weapon down off the wall. `Armtorch.pl8` is
    // 26 frames — thirteen per torch — and each `Trp_<weapon>_<colour>.pl8` is
    // 21 of 89 × 158: eight walking in, five taking it down, eight carrying it
    // out. See [`crate::screens::armoury::Walker`].
    //
    // **Thirty sheets, 3.3 MB, and the original reads exactly one of them.**
    // `FUN_004AABD8` picks it by the local player's shield colour and the
    // weapon just assigned, at the moment the walk starts — which is a read
    // during a click, and this list is eager because a lazy cache would need
    // interior mutability on a path `draw` may only see through `&`. What that
    // costs is the read: `Sheet` decodes lazily, so a colour nobody plays is
    // never turned into pixels.
    "Armtorch.pl8",
    "Trp_xb_r.pl8",
    "Trp_ma_r.pl8",
    "Trp_sw_r.pl8",
    "Trp_pi_r.pl8",
    "Trp_ar_r.pl8",
    "Trp_kn_r.pl8",
    "Trp_xb_y.pl8",
    "Trp_ma_y.pl8",
    "Trp_sw_y.pl8",
    "Trp_pi_y.pl8",
    "Trp_ar_y.pl8",
    "Trp_kn_y.pl8",
    "Trp_xb_k.pl8",
    "Trp_ma_k.pl8",
    "Trp_sw_k.pl8",
    "Trp_pi_k.pl8",
    "Trp_ar_k.pl8",
    "Trp_kn_k.pl8",
    "Trp_xb_p.pl8",
    "Trp_ma_p.pl8",
    "Trp_sw_p.pl8",
    "Trp_pi_p.pl8",
    "Trp_ar_p.pl8",
    "Trp_kn_p.pl8",
    "Trp_xb_b.pl8",
    "Trp_ma_b.pl8",
    "Trp_sw_b.pl8",
    "Trp_pi_b.pl8",
    "Trp_ar_b.pl8",
    "Trp_kn_b.pl8",
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
    // 0x29 … 0x2B, the battlefield. Not a painter's own read: `Res_LoadStatic`
    // (`0x00499859`) preloads it into `0x00568EE0` as record 2 of
    // `g_preloadTable`, and `Screen_DrawBattlefield` (`0x004233F7`) sets it
    // with `Palette_Set(0x568EE0)`. **It was missing from this list** while
    // `BattlefieldScreen::palette` named it, so the lookup failed and the
    // presenter drew a battle in `base01.256` — *"blue grainy madness"*.
    //
    // NOT PORTED: the siege arm, `Palette_Set(0x5675A0)` = `t32_stn1.256`. It
    // belongs with `t32_stn1.pl8`, and `l2_view::scene` draws every battle
    // from `T32_bat1.pl8`; one without the other would be wrong both ways.
    "T32_bat1.256",
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
    /// `Fnt_8.pl8`, `g_font8` — see [`font::EIGHT`] for where the original
    /// loads it and why nothing here draws with it yet.
    pub eight: Option<Font>,
    /// `Font_10.pl8`, `g_font10` — the county strip's numbers. A numeral face:
    /// see [`font::TEN`] for why nothing but a number may be drawn in it.
    pub ten: Option<Font>,
    sheets: BTreeMap<String, Sheet>,
    palettes: BTreeMap<String, Palette>,
    /// `mercgrid.pl8`'s 80 x 60 byte map, with its 24-byte header stripped.
    /// Empty when the file is not installed. See [`ShellAssets::merchant_grid`].
    merchant_grid: Vec<u8>,
    /// `arm_grid.pl8`'s, the same shape and read by the same arithmetic.
    /// See [`ShellAssets::armoury_grid`].
    armoury_grid: Vec<u8>,
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
        let loaded = ShellAssets {
            eng: read("L2.eng").and_then(|b| Eng::parse(b).ok()),
            body: read(font::BODY).and_then(|b| Font::new(b, 16).ok()),
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
    /// correct. A player reported precisely that as *"the title screen is
    /// illegible, all caps of that font is ridiculous"*, and there was nothing
    /// anywhere — no log line, no screen, no exit code — to distinguish it from
    /// a font we had chosen.
    ///
    /// This is `docs/agents.md`'s *a tool that degrades silently is worse the
    /// more people use it*, in the shipped program rather than in a script. The
    /// degradation is still the right behaviour: the game must run on a bare
    /// checkout. What was wrong is that it happened without a word.
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
    /// position — and it is the one made by the artwork rather than the code.
    pub fn merchant_grid(&self, x: i32, y: i32) -> Option<u8> {
        grid_cell(&self.merchant_grid, x, y)
    }

    /// Whether `mercgrid.pl8` was found, so a screen can say which hit test it
    /// is using rather than leaving a player to wonder why a ware will not
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
    /// because there is no faithful reproduction of an out-of-bounds read.
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
    /// original rather than like our placeholder. Screens report it so a
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

/// **Four pixels of trailing space after every string**, and it is the last
/// statement of `Ui_DrawText` (`0x00402637`): `g_penAdvance = g_penAdvance + 4;`.
///
/// It is the gap between the two halves of every sentence the original builds
/// out of pieces — *"Raising an army in"* and the county's name, a number and
/// its noun — and none of `L2.eng`'s strings carries a trailing space of its
/// own, so without it the two halves touch. `[V]`
pub const TRAILING: i32 = 4;

/// **`L2.eng` group 8 is the noun table**, and `Ui_DrawCount`'s second argument
/// is an index into it. Seventy-four strings in singular/plural pairs: 0/1
/// *"Crown."*, 2/3 *"Sack."*, 4/5 *"Animal."*, `0x34 + t * 2` the seven troop
/// types, 68 *"Grain"*, 70/71 *"Cow."*, 72/73 *"Total men"*.
pub const COUNT_NOUN_GROUP: usize = 8;

/// **Which face a call site names** — the `font` argument `Ui_DrawText` and
/// everything above it take: `&g_fontBody` (`Fntl2_14.pl8`), `&g_fontHeading`
/// (`Fntl2_22.pl8`) or `&g_font8` (`Fnt_8.pl8`).
///
/// They are not interchangeable, and the choice is the call site's, not the
/// helper's: `Ui_DrawCount` is body at 78 of its 80 call sites and heading at
/// the other two, and `Court_Draw` puts every store value in heading beside a
/// heading label where we had drawn them in body. A helper that hard-codes one
/// face is a helper that draws some screen in the wrong one.
///
/// **The original preloads five faces, not three or four** — `Res_LoadStatic`'s
/// records 3…7 are `fnt_8`, `fntl2_9`, `font_10`, `fntl2_14`, `fntl2_22` — and
/// all five are loaded. Two have no variant here, because no `Pen` caller names
/// them: `&g_fontSmall` and `&g_font10` are drawn only on the county strip (and
/// `&g_fontSmall` once more, in `Screen_DrawEndTurn`), which draws through
/// `ShellAssets::small` and `ShellAssets::ten` directly. See [`font::TEN`] for
/// why the fifth face is a numeral face. Counted from the bytes, not the
/// decompilation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Body,
    Heading,
    /// `g_font8`. **No screen of ours names it**: every call site in the
    /// original is one of six developer read-outs — [`font::EIGHT`].
    Eight,
}

/// `L2.eng` group 26, `Ui_DrawYear`'s era: index 0 *"BC"*, index 1 *"AD"*.
pub const YEAR_GROUP: usize = 26;
pub const YEAR_BC: usize = 0;
pub const YEAR_AD: usize = 1;

/// `Ui_DrawCount`'s lead, which it does not take from its caller: always
/// `'@'`, the blank sign column. `0x0041AB67`. **[V]**
pub const COUNT_LEAD: char = '@';

/// `Ui_DrawCount`'s suffix: `&DAT_004D41F4`, a NUL, so the empty string. Read
/// out of the shipped `Lords2.exe` at file offset `0xD23F4`. **[V]**
pub const COUNT_SUFFIX: &str = "";

/// Which of `Ui_DrawCount`'s singular/plural pair a value takes.
///
/// **`|value| == 1`, not `value == 1`**, and this is a free function so that
/// the rule can be asserted without a canvas, a font or an install. It is the
/// exact ladder at `0x0041AB67`:
///
/// ```c
/// if (value == 1)       Eng_DrawString(8, unitIndex,     ...);
/// else if (value == -1) Eng_DrawString(8, unitIndex,     ...);
/// else                  Eng_DrawString(8, unitIndex + 1, ...);
/// ```
///
/// Three arms where two would do, because **minus one is singular**. We had
/// only the first, so `-1` drew the plural — *"−1 Sacks."* where the original
/// writes *"−1 Sack."* It is reachable: the trade screen's quantity is signed,
/// and the map information panel draws
/// `Ui_DrawCount(-g_counties[c].field_0x24C, 2, ...)` with the sign negated at
/// the call site. Found by the draw-call audit reading the *primitive* rather
/// than the screens that call it, which is the argument for auditing the leaves
/// — `docs/draws.md` §7. **[V]**
pub fn count_noun(value: i32, noun: usize) -> usize {
    if value == 1 || value == -1 {
        noun
    } else {
        noun + 1
    }
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

    /// One line in the body font. **Returns the x the next glyph would go at**,
    /// so a caller building a sentence out of pieces can hand the answer
    /// straight back in.
    ///
    /// **That was not true until it was looked at with the real fonts loaded.**
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
    /// as wells inside a window it has already drawn, which is why the border
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
    // rather than being copied.

    /// `Ui_DrawInsetRect(x, y, w, h)` — the recessed well, in **pixels**.
    pub fn inset(&self, canvas: &mut Canvas, r: crate::input::Rect) {
        inset_rect(canvas, r.x, r.y, r.w, r.h);
    }

    /// `FUN_00403CF4(x, y, w, h, colour)` — **a one-pixel rectangle outline in
    /// one palette index**, which is four `FUN_00403A8F` line draws.
    ///
    /// It is a primitive of the original's and not a widget of ours, which is
    /// the whole reason it lives here rather than staying [`crate::widget::frame`].
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
    /// zeroes `g_penAdvance` first and the era is placed from it, which is why
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

    /// **A palette a screen names is a palette something loads.** The
    /// battlefield named `T32_bat1.256` for as long as it existed and this list
    /// never carried it, so the presenter's `shell.palette(name)` found nothing,
    /// fell through to `base01.256`, and a player saw the battle in the
    /// campaign's colours. It needs no install: both halves are names.
    ///
    /// Ablation: delete the `"T32_bat1.256"` line above — red.
    #[test]
    fn the_battlefields_palette_is_one_the_shell_loads() {
        let name = crate::screen::ScreenId::Battlefield.build().palette();
        let name = name.expect("the battlefield names a palette of its own");
        assert!(
            PALETTES.iter().any(|p| key(p) == key(name)),
            "{name} is named by the battlefield and loaded by nobody"
        );
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

    /// **All five faces are announced, and `Fntl2_9.pl8` was the one that was
    /// not.** The complaint checked `body` and `heading`; `small` draws the
    /// build stamp and the county strip, and its absence was silent. `ten`
    /// draws every number on the jobs plate, and joins the list with it.
    ///
    /// Ablated by deleting the `small` arm of `missing_fonts`: the list comes
    /// back with two names and this goes red naming the third. Verified.
    #[test]
    fn a_bare_shell_names_every_font_it_could_not_load() {
        let missing = ShellAssets::empty().missing_fonts();
        assert_eq!(
            missing,
            vec![font::BODY, font::HEADING, font::SMALL, font::EIGHT, font::TEN],
            "every face a Pen or a painter falls back from has to be in this list"
        );
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
