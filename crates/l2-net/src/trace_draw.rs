//! Draw and text records: the sheet a blit came from and the original leaf that drew it.
//! C246; `tools/oracle/scenarios/FORMAT.md`. Only with `--features trace`.

use std::cell::RefCell;
use std::collections::BTreeMap;

use crate::trace::{self, Arg};

#[derive(Default)]
struct Draws {
    files: BTreeMap<(usize, u32), String>,
    sheets: BTreeMap<usize, String>,
    strings: BTreeMap<String, (i64, i64)>,
    leaf: Option<(u32, &'static str)>,
    held: u32,
    claimed: bool,
    cached: BTreeMap<&'static str, (u64, Vec<trace::Record>)>,
    capturing: Option<usize>,
    at: Option<(i32, i32)>,
}

// Outlives `trace::start`: the registries fill while the assets load, before the scenario.
thread_local! {
    static DRAWS: RefCell<Draws> = RefCell::new(Draws::default());
}

fn fnv(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193))
}

/// `Vfs::read`: a `.pl8`'s bytes, so the `Sheet` built from them knows its file.
pub fn name_file(name: &str, bytes: &[u8]) {
    let name = name.to_ascii_lowercase();
    if name.ends_with(".pl8") {
        DRAWS.with(|d| d.borrow_mut().files.insert((bytes.len(), fnv(bytes)), name));
    }
}

/// `Sheet::new`: the sheet's byte buffer, which moves with it, keyed to its file.
pub fn sheet_born(ptr: usize, bytes: &[u8]) {
    DRAWS.with(|d| {
        let mut d = d.borrow_mut();
        match d.files.get(&(bytes.len(), fnv(bytes))).cloned() {
            Some(n) => d.sheets.insert(ptr, n),
            None => d.sheets.remove(&ptr),
        };
    });
}

/// `Sheet::frame`: the frame, paired with its sheet's file for the next blit.
pub fn frame_of(ptr: usize, index: usize) {
    match DRAWS.with(|d| d.borrow().sheets.get(&ptr).cloned()) {
        Some(name) => trace::set_sprite(name, index as i64),
        None => trace::note_frame(index),
    }
}

/// Every `L2.eng` string, so a literal drawn from one is recorded by its group and index.
pub fn name_string(group: usize, index: usize, s: &str) {
    if !s.is_empty() {
        DRAWS.with(|d| {
            d.borrow_mut().strings.entry(s.to_string()).or_insert((group as i64, index as i64));
        });
    }
}

/// The original's blitter for the next sprite (`TileBlit_*`, `Blit_Unclipped`, `FUN_004b4537` ...).
pub fn leaf(addr: u32, name: &'static str) {
    DRAWS.with(|d| d.borrow_mut().leaf = Some((addr, name)));
}

/// `Canvas::blit_full`: the noted sprite's record, under the noted leaf, else `Pl8_DrawFrame`.
pub fn blit(x: i32, y: i32) {
    let (leaf, held, at) = DRAWS.with(|d| {
        let mut d = d.borrow_mut();
        (d.leaf.take(), d.held, d.at.take())
    });
    let (x, y) = at.unwrap_or((x, y));
    let sprite = trace::take_sprite();
    if held > 0 {
        return;
    }
    if let Some((sheet, frame)) = sprite {
        let (addr, name) = leaf.unwrap_or((0x0040A21A, "Pl8_DrawFrame"));
        let args = vec![("sheet", Arg::S(sheet)), ("frame", Arg::I(frame)), ("x", Arg::I(x as i64)), ("y", Arg::I(y as i64))];
        trace::emit("draw", addr, name, args);
    }
}

/// A hooked `Ui_*` leaf: its record; nothing it draws inside is recorded (FORMAT.md, last line).
/// An `s` argument that is an `L2.eng` string gains its `group` and `index`.
pub fn enter(kind: &'static str, addr: u32, name: &'static str, mut args: Vec<(&'static str, Arg)>) {
    let (held, found) = DRAWS.with(|d| {
        let mut d = d.borrow_mut();
        d.held += 1;
        let s = args.iter().find_map(|(k, v)| match (k, v) { (&"s", Arg::S(s)) => Some(s), _ => None });
        (d.held, s.and_then(|s| d.strings.get(s).copied()))
    });
    if let Some((g, i)) = found {
        args.splice(0..0, [("group", Arg::I(g)), ("index", Arg::I(i))]);
    }
    if held == 1 && addr != 0 {
        trace::emit(kind, addr, name, args);
    }
}

pub fn leave() {
    DRAWS.with(|d| {
        let mut d = d.borrow_mut();
        d.held = d.held.saturating_sub(1);
    });
}

/// The record just emitted (`Eng_DrawString`, `Ui_DrawNumber`) stands for the next glyph run.
pub fn claim_text() {
    DRAWS.with(|d| d.borrow_mut().claimed = true);
}

/// A glyph run: `Ui_DrawText` `0x00402637`, by `L2.eng` group/index where the string is one.
/// `font` tells the game's faces from our 5x7 fallback.
pub fn text(font: &'static str, s: &str, x: i32, y: i32) {
    let found = DRAWS.with(|d| {
        let mut d = d.borrow_mut();
        let skip = d.held > 0 || d.claimed;
        d.claimed = false;
        if skip || !trace::on() {
            return None;
        }
        Some(d.strings.get(s).copied())
    });
    let Some(found) = found else { return };
    let mut args = Vec::new();
    if let Some((g, i)) = found {
        args.push(("group", Arg::I(g)));
        args.push(("index", Arg::I(i)));
    }
    args.extend([("s", Arg::S(s.to_string())), ("x", Arg::I(x as i64)), ("y", Arg::I(y as i64)), ("font", Arg::S(font.into()))]);
    trace::emit("text", 0x00402637, "Ui_DrawText", args);
}

/// The next blit's record at g_drawX/Y, where the leaf's own placement differs from ours.
pub fn record_at(x: i32, y: i32) {
    DRAWS.with(|d| d.borrow_mut().at = Some((x, y)));
}

/// The campaign map's direct blitters, by what they draw.
#[derive(Clone, Copy)]
pub enum Map {
    /// `Map_DrawTile` `0x004063C1`: `TileBlit_{Plain,M3,M4}_Z{zoom}`.
    Tile,
    /// `Map_DrawArmies` `0x00408438`, caller `0x00408930`.
    Figure,
    /// `Sprite_TopIt` `0x004071A0` flags, markers; `Map_DrawArmies`' banner; caller `0x00407E1C`.
    Flag,
}

const TILES: [[(u32, &str); 3]; 3] = [
    [(0x00452820, "TileBlit_Plain_Z0"), (0x0045337B, "TileBlit_M3_Z0"), (0x004538F6, "TileBlit_M4_Z0")],
    [(0x00460501, "TileBlit_Plain_Z1"), (0x0046079D, "TileBlit_M3_Z1"), (0x004608DE, "TileBlit_M4_Z1")],
    [(0x00463B42, "TileBlit_Plain_Z2"), (0x00463C06, "TileBlit_M3_Z2"), (0x00463C62, "TileBlit_M4_Z2")],
];
const FIGURES: [(u32, &str); 3] =
    [(0x004B43B1, "Blit_Unclipped"), (0x004B446D, "Blit_ClippedLeft"), (0x004B44D2, "Blit_ClippedRight")];
// [I] 4814 left, 48b8 right: the original's 48b8 draws flags at x 454, the view's right seam.
const FLAGS: [(u32, &str); 3] =
    [(0x004B4537, "FUN_004b4537"), (0x004B4814, "FUN_004b4814"), (0x004B48B8, "FUN_004b48b8")];

/// The leaf for a sprite at `x`, `w` wide, in the view `x0..x1`: unclipped, clipped left or right.
/// [I] the tile blitters' M3/M4 are the left and right seams: the original's M3 draws at x 0, M4 at 450.
pub fn map_leaf(what: Map, zoom: u8, x: i32, w: i32, x0: i32, x1: i32) {
    let side = if x < x0 { 1 } else if x + w > x1 { 2 } else { 0 };
    let (addr, name) = match what {
        Map::Tile => TILES[(zoom as usize).min(2)][side],
        Map::Figure => FIGURES[side],
        Map::Flag => FLAGS[side],
    };
    leaf(addr, name);
}
/// `trace::start`: a cache painted in an earlier run is not this run's.
pub fn new_run() {
    DRAWS.with(|d| {
        let mut d = d.borrow_mut();
        d.cached.clear();
        (d.leaf, d.held, d.claimed, d.capturing, d.at) = (None, 0, false, None, None);
    });
}

/// A painter that draws into a cache (`MapScreen::ensure`'s `base`): its records are kept...
pub fn capture() {
    DRAWS.with(|d| d.borrow_mut().capturing = Some(trace::count()));
}

pub fn captured(key: &'static str) {
    let Some(at) = DRAWS.with(|d| d.borrow_mut().capturing.take()) else { return };
    let (frame, recs) = trace::records_from(at);
    let recs = recs.into_iter().filter(|r| r.kind == "draw" || r.kind == "text").collect();
    DRAWS.with(|d| d.borrow_mut().cached.insert(key, (frame, recs)));
}

/// ...and emitted again on each later frame the cache is copied out, as the original redraws them.
pub fn replay(key: &'static str) {
    let Some((frame, recs)) = DRAWS.with(|d| d.borrow().cached.get(key).cloned()) else { return };
    if trace::records_from(usize::MAX).0 == frame {
        return;
    }
    recs.into_iter().for_each(|r| trace::emit(r.kind, r.addr, r.name, r.args));
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blit_is_recorded_by_its_file_under_the_leaf_and_not_inside_a_held_leaf() {
        let bytes = vec![7u8; 24];
        name_file("Flags1a.PL8", &bytes);
        trace::start();
        sheet_born(bytes.as_ptr() as usize, &bytes);
        frame_of(bytes.as_ptr() as usize, 91);
        leaf(0x004B4537, "FUN_004b4537");
        blit(304, 215);
        enter("draw", 0x0040D1BC, "Ui_OkButton", vec![]);
        frame_of(bytes.as_ptr() as usize, 3);
        blit(1, 2);
        leave();
        name_string(220, 6, "View population");
        text("body", "View population", 284, 229);
        let r = trace::stop();
        assert_eq!(r.len(), 3, "{r:?}");
        assert_eq!((r[0].name, &r[0].args[0].1), ("FUN_004b4537", &Arg::S("flags1a.pl8".into())));
        assert_eq!(r[1].name, "Ui_OkButton", "the held leaf's own blit is not recorded");
        assert_eq!(r[2].args[0], ("group", Arg::I(220)));
    }
}