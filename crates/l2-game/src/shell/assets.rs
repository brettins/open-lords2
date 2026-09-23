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
        let mut troops = BTreeMap::new();
        for name in TROOPS_FILES {
            if let Some(t) = read(name).and_then(|b| l2_formats::Troops::load(&b).ok()) {
                troops.insert(key(name), t);
            }
        }
        let loaded = ShellAssets {
            eng: read("L2.eng").and_then(|b| Eng::parse(b).ok()),
            battles: l2_formats::Battles::parse(&read(BATTLES_FILE).unwrap_or_default()),
            troops,
            skirmish_files: skirmish_files(vfs),
            skirmish_skr: skirmish_files(vfs)
                .into_iter()
                .filter_map(|n| read(&n).map(|b| (n, b)))
                .collect(),
            body: read(font::BODY).and_then(|b| Font::new(b, 16).ok()).map(Font::raising_accents),
            heading: read(font::HEADING).and_then(|b| Font::new(b, 24).ok()),
            small: read(font::SMALL).and_then(|b| Font::new(b, 12).ok()),
            eight: read(font::EIGHT).and_then(|b| Font::new(b, 12).ok()),
            ten: read(font::TEN).and_then(|b| Font::new(b, 12).ok()),
            sheets,
            palettes,
            merchant_grid: read_grid(&read, "mercgrid.pl8"),
            armoury_grid: read_grid(&read, "arm_grid.pl8"),
        };
        loaded.complain_about_what_is_missing();
        loaded
    }

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

    pub fn empty() -> ShellAssets {
        ShellAssets {
            eng: None,
            battles: l2_formats::Battles::default(),
            troops: BTreeMap::new(),
            skirmish_files: Vec::new(),
            skirmish_skr: BTreeMap::new(),
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
    pub fn merchant_grid(&self, x: i32, y: i32) -> Option<u8> {
        grid_cell(&self.merchant_grid, x, y)
    }

    pub fn has_merchant_grid(&self) -> bool {
        self.merchant_grid.len() == MERCHANT_GRID_LEN
    }

    /// `FUN_0043582A`'s hit test: the **weapon type** whose rack is under a
    /// screen pixel, or `None`.
    pub fn armoury_grid(&self, x: i32, y: i32) -> Option<u8> {
        match grid_cell(&self.armoury_grid, x, y) {
            Some(t) if (1..=l2_kingdom::tables::WEAPON_TYPE_COUNT as u8).contains(&t) => Some(t),
            _ => None,
        }
    }

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

    /// The table `Troops_Load` (`00420000.c:4349-4369`) would have parsed:
    /// `TROOPS.ENG` in multiplayer, else `TROOPS2.ENG` when the local player
    /// attacks and `TROOPS3.ENG` when he does not.
    pub fn troops(&self, multiplayer: bool, local_defends: bool) -> Option<&l2_formats::Troops> {
        self.troops.get(&key(l2_formats::troops::file_for(multiplayer, local_defends)))
    }

    /// One `.skr` by the name page 13 lists, parsed — `DAT_0053F5FC` as
    /// `FUN_0042D4AA` and `Skr_ReadMap` (`0x0042D806`) read it.
    pub fn skr(&self, name: &str) -> Option<l2_formats::Skr<'_>> {
        l2_formats::Skr::parse(self.skirmish_skr.get(&key(name))?).ok()
    }

    pub fn has_artwork(&self) -> bool {
        self.eng.is_some() && self.body.is_some() && !self.sheets.is_empty()
    }
}

/// `Files_Enumerate("*.skr", 1)` (`0x004AECF0`): every `.skr` in the working
/// directory, lower-cased, extension kept, the first hundred only.
///
/// **[D]** the `/` filter and the order. `FindFirstFileA("*.skr")` sees one
/// directory; our VFS index is recursive, so a `.skr` in a mod subdirectory
/// would be listed where the original lists none, and the filter drops it.
///
/// The order is the original's `FindFirstFileA`, bubbled by
/// [`FUN_004AEE3E`]'s comparison — which reads **only the first byte** of each
/// name (`004a0000.c:6590`) — so two files sharing a first letter keep
/// directory order. Ours is the VFS index's full sort, which that pass would
/// leave alone.
fn skirmish_files(vfs: &Vfs) -> Vec<String> {
    vfs.entries_with_extension("skr")
        .into_iter()
        .filter(|name| !name.contains('/'))
        .take(SKIRMISH_FILE_MAX)
        .map(str::to_string)
        .collect()
}

fn read_grid(read: &impl Fn(&str) -> Option<Vec<u8>>, name: &str) -> Vec<u8> {
    read(name)
        .filter(|b| b.len() >= GRID_HEADER + MERCHANT_GRID_LEN)
        .map(|b| b[GRID_HEADER..GRID_HEADER + MERCHANT_GRID_LEN].to_vec())
        .unwrap_or_default()
}

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

pub(super) fn key(name: &str) -> String {
    name.to_ascii_lowercase()
}

