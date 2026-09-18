#![allow(unused_imports)]

mod data;
pub use data::*;

use super::*;
use super::types::*;
use super::screen::*;
use super::painters::*;
use l2_kingdom::conquest::LeftCastle;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};

pub const OK: Rect = Rect::new(0x1AC, 0x1B6, 24, 24);

/// **`g_tilePanelWidgets` (`0x004DD640`) record 0 — *"View these troops?"***
///
/// `Widget_Test(8, 0x20, &DAT_004DD640, DAT_00568474)` and the matching
/// `Widget_Draw`, so the record's own `(336, 382)` lands at **(344, 414)**,
/// 24 square. `L2.eng` 71/14 is the caption above it.
///
/// The count is a **runtime global**: `TileInfo_DrawCastle` opens
/// `DAT_00568474 = (g_counties[g_pickedTileCounty].garrisonUnit != 0)`, so the
/// panel has one widget on a castle tile with a garrison and none anywhere
/// else. There is **no ownership gate** — the branch above it draws either
/// *"…currently stationed here."* for your own men or 71/19 *"Enemy troops are
/// barracked here."* for somebody else's, and offers the button either way.
pub const GARRISON_WIDGET: Rect = Rect::new(336 + 8, 382 + 0x20, 24, 24);
/// `L2.eng` group 71 — the castle block of the tile half. 14 is *"View these
/// troops?"*.
pub const CASTLE_GROUP: usize = 0x47;
pub const VIEW_THESE_TROOPS: usize = 0x0E;

pub const HEADING_X: i32 = 0x28;
pub const HEADING_DY: i32 = 0x40;
pub const ICON_AT: (i32, i32) = (0x28, 0x60);
/// The wrapped description: `FUN_0040328E(group, i, 0x68, row*16 + 100, W, …)`.
pub const BODY_X: i32 = 0x68;
pub const BODY_DY: i32 = 100;
/// `UnitPanel_Draw`'s, group 31.
pub const BODY_WRAP: i32 = 0x120;
/// `TileInfo_Draw`'s, group 30 — sixteen pixels wider.
pub const TILE_BODY_WRAP: i32 = 0x130;

pub const ICON_SHEET: &str = "Icon_tmp.pl8";

pub mod icon {
    pub const MERCHANT: usize = 0x23;
    pub const TRANSPORT: usize = 0x24;
    pub const PEASANTS: usize = 0x25;
    pub const ENEMY_ARMY: usize = 0x26;
    pub const OWN_ARMY: usize = 0x27;
    pub const MOVE: usize = 0x28;
    pub const SORTIE: usize = 0x29;
    pub const DISBAND: usize = 0x2A;
    pub const SPLIT: usize = 0x2B;
    pub const BRUSH_ABANDON: usize = 0x00;
    pub const BRUSH_RECLAIM: usize = 0x11;
    pub const BRUSH_FALLOW: usize = 0x01;
    pub const BRUSH_GRAIN: usize = 0x0D;
    pub const BRUSH_PASTURE: usize = 0x16;
}

pub const MERCHANT: (usize, usize, usize) = (0, 12, icon::MERCHANT);
pub const PEASANTS: (usize, usize, usize) = (5, 13, icon::PEASANTS);
pub const TRANSPORT: (usize, usize, usize) = (2, 14, icon::TRANSPORT);
pub const ENEMY_ARMY: (usize, usize, usize) = (6, 15, icon::ENEMY_ARMY);
pub const OWN_ARMY: (usize, usize, usize) = (6, 16, icon::OWN_ARMY);

pub const ARMY_FROM: usize = 9;
pub const FORMED: usize = 0x14;
pub const WAGES: usize = 8;
pub const MOVES_LEFT: usize = 0x16;
pub const SUPPLY0: usize = 0x17;
pub const HEALTH0: usize = 0x1B;
/// The starvation line's colour once the counter leaves zero —
/// `local_8 = 0xF9` in `UnitPanel_Draw` (`0x0041B19D`), the same red the county
/// panel's negative deltas take.
pub const STARVING: u8 = 0xF9;
pub const MERC_GROUP: usize = 16;

/// **The unit half's heading-face lines**, every one `&g_fontHeading` in
/// `UnitPanel_Draw` (`0x0041B19D`) and every y `R * 0x10 + k`.
pub const TRANSPORT_HEADING_AT: (i32, i32) = (0x18, 0x30);
/// An army's name: `Eng_DrawString((char)owner + 0x5D, nameIndex, 0x28,
/// R * 0x10 + 0x30, &g_fontHeading, 0x3F)`. `L2.eng` 94…98 are the five realms'
/// 24 names each (`docs/formats/eng.md` §5).
pub const ARMY_NAME_GROUP: usize = 0x5D;
pub const ARMY_NAME_DY: i32 = 0x30;
/// 16/0 alone, or `Ui_DrawNumber(mercMen, '@', &DAT_004D422C, 0x38, …)`, 16/band
/// and `Ui_DrawUnitNoun(mercMen, mercTroop * 2 + 0x34, …)`. `DAT_004D422C` is a
/// NUL, read out of the image.
pub const MERC_LINE_AT: (i32, i32) = (0x38, 0x130);

pub const COUNTY_TOWN_HEADING: usize = 7;
pub const COUNTY_TOWN_BODY: usize = 0x1B;
pub const COUNTY_TOWN_ICON: usize = 0x1B;

/// Every row is three literals off one arm of `TileInfo_Draw` (`0x0041C208`),
/// which is `local_20`, `local_1c` and `local_8` in the decompilation and
/// `local_c` (the second heading) zero in all six:
///
/// ```c
/// if      (flags & 0x01) { local_20 = 1; local_1c = 0x17; local_8 = 0x18; }  /* road */
/// else if (flags & 0x04) { local_20 = 3; local_1c = 0x18; local_8 = 0x1d; }  /* sea  */
/// else if (flags & 0x10) { graphic == 0x10 ? (0x30, 0x31) : (0x32, 0x33); local_8 = 0x1b; }
/// else if (flags & 0x08) { DAT_005651bc ? (4, 0x19, 0x19) : (5, 0x1a, 0x1a); }
/// ...
/// else                   { local_20 = 0; local_1c = 0x16; local_8 = 0x17; }  /* scrub */
/// ```
///
/// `DAT_005651BC` is `Map_ResolvePick`'s (`0x0046D5FE`) mountain bit — see
/// [`l2_kingdom::map::CampaignMap::is_mountain`].
pub const TILE_LADDER: [(TileKind, usize, usize, usize); 6] = [
    (TileKind::Road, 1, 0x17, 0x18),
    (TileKind::Sea, 3, 0x18, 0x1D),
    (TileKind::Village, 0x30, 0x31, 0x1B),
    (TileKind::RuinedVillage, 0x32, 0x33, 0x1B),
    (TileKind::Mountain, 4, 0x19, 0x19),
    (TileKind::Woodland, 5, 0x1A, 0x1A),
];

pub const SCRUBLAND_INFO: (usize, usize, usize) = (0, 0x16, 0x17);

pub const VILLAGE_GRAPHIC: u8 = 0x10;

pub const CASTLE_HEADING: usize = 8;
pub const CASTLE_HEADING_BUILDING: usize = 0x0E;
pub const CASTLE_HEADING_REPAIR: usize = 0x0F;
pub const CASTLE_ICON: usize = 0x1C;
pub const CASTLE_TAX_BONUS: usize = 0x10;
pub const CASTLE_BARRACKS: usize = 0x0B;
pub const CASTLE_TROOPS: usize = 0x0C;
pub const CASTLE_STATIONED: usize = 0x0D;
pub const CASTLE_ENEMY_BARRACKED: usize = 0x13;

/// `local_18` is the county's industry record and the four ranges are the
/// *same* ones [`l2_kingdom::industry::map_toggle_for_graphic`] uses to decide
/// which industry a **left** click on the tile toggles, so the site this panel
/// describes and the site that click switches agree by construction
/// by two transcriptions of one ladder. `[V]` — and
/// [`crate::audio::names::resource_site_slot`] is the third reader of it.
///
/// `local_c == 0`, so the body takes the ladder's generic tail
/// `FUN_0040328E(30, local_1c, 0x68, row*16 + 100, 0x130, …)` — [`BODY_X`],
/// [`BODY_DY`], [`TILE_BODY_WRAP`] — with **both** sides of the
/// `g_localPlayer == g_pickedCountyOwner` test being the identical call. There
/// is no ownership gate on a resource site.
pub const SITE_INFO: [(usize, usize, usize); 4] = [
    (0x0B, 0x45, 0x20),
    (0x09, 0x35, 0x1E),
    (0x52, 0x53, 0x12),
    (0x0A, 0x3D, 0x1F),
];

/// `total − totalSnapshot` is last season's output, which is
/// [`l2_kingdom::county::Industry::output`] here — the same difference, kept as
/// a field. Group 22 is never touched: 53…57 read *"A small mine."*, *"A medium
/// mine."*, *"A large mine."*, *"A very large mine."*, *"A destroyed mine."*,
/// `+4` as destroyed. `[V]` against the player's `L2.eng`. The brief that
/// opened this arm called it a fertility table; the correction is
/// `docs/decisions.md` C204.
///
/// The same three thresholds, on the same difference, pick the tile's own
/// *sprite* in `Sprite_TopIt` (`0x004071A0`) — a painter that shares no code
/// with this one, reading `industry[1]`, `[3]`, `[2]` and `[0]` over the same
/// four graphic ranges. Two independent readers of one table is what makes the
/// bucketing `[V]`.
pub const SITE_BAND_SMALL: usize = 0;
pub const SITE_BAND_MEDIUM: usize = 1;
pub const SITE_BAND_LARGE: usize = 2;
pub const SITE_BAND_VERY_LARGE: usize = 3;
pub const SITE_BAND_DESTROYED: usize = 4;
pub const SITE_BAND_EDGES: [i32; 3] = [10, 0x19, 0x32];

pub const SITE_SHUT_DOWN: usize = 0x4D;
pub const SITE_OPERATIONAL: usize = 0x4E;
pub const SITE_STATUS_DY: i32 = 0x74;

/// ```c
/// if ((g_pickedTileFlags & 0x80) == 0) {
///   if ((g_pickedTileFlags & 0x40) != 0 && county.mercenaryOffer != 0) {
///     Pl8_DrawFrameClipped(g_flagsSheet, 0x81, 0x32, DAT_00553d2c * 0x10 + 0x9c);
///     FUN_0040328e(0x1e, 0x3b, 0x68, DAT_00553d2c * 0x10 + 0xa0, 0x140, …);
///   }
/// }
/// ```
///
/// `0x1e` is [`TILE_GROUP`] and `0x3b` is 59 — *"Mercenaries are available for
/// hire in the county."* — read out of the player's own `L2.eng` like every
/// other string here. The frame is `g_flagsSheet` `0x81`, the **same index the
/// map's marker uses** ([`l2_view::campaign::MERCENARY_MARKER_FRAME`]); two
/// unrelated painters passing one constant is what makes it `[V]`.
pub const MERCENARIES_AVAILABLE: usize = 0x3B;
pub const MERC_MARKER_AT: (i32, i32) = (0x32, 0x9C);
pub const MERC_TEXT_AT: (i32, i32, i32) = (0x68, 0xA0, 0x140);

pub const MOVE_ALLOWANCE: i32 = 15;

/// The three army buttons, 48 pixels square. `g_infoUnitButtons`
/// (`0x004DC560`) is **kind 1**: they fire on left *press*.
pub const BUTTON_DIM: i32 = 40;
pub const BUTTON_X: [i32; 3] = [48, 112, 176];
pub const BUTTON_DY: i32 = 352;

pub const BRUSH_ROW_Y: i32 = 376;
pub const BRUSH_BEVEL: Rect = Rect::new(40, 368, 384, 64);
/// The caption, `L2.eng` 30/52 — *"Click on an icon to alter field usage."*
pub const BRUSH_CAPTION_AT: (i32, i32, i32) = (48, 386, 192);
pub const BRUSH_CAPTION: usize = 52;
pub const BRUSH_FIELD_X: [i32; 3] = [240, 304, 368];
pub const BRUSH_WASTE_X: [i32; 2] = [304, 368];
pub const BRUSH_DIM: i32 = 48;

pub const BRUSH_FIELD_ID: [u8; 3] = [1, 2, 0x13];
pub const BRUSH_WASTE_ID: [u8; 2] = [0x19, 0];

/// **`DAT_004D2EC8` — `TileInfo_Draw`'s farmland table**, one row per terrain
/// value `0 … 0x1C`: `[heading, body, icon, mode]`, the four `i32`s the painter
/// reads as `local_20`, `local_1c`, `local_8` and `local_c`.
pub const FARM_TILE_INFO: [[usize; 4]; 0x1D] = [
    [6, 33, 0, 17],
    [6, 34, 1, 18],
    [6, 35, 2, 19],
    [6, 36, 3, 19],
    [6, 37, 4, 19],
    [6, 38, 5, 19],
    [6, 39, 6, 19],
    [6, 36, 7, 19],
    [6, 37, 8, 19],
    [6, 38, 9, 19],
    [6, 39, 10, 19],
    [6, 36, 11, 19],
    [6, 37, 12, 19],
    [6, 38, 13, 19],
    [6, 39, 14, 19],
    [6, 40, 15, 20],
    [6, 41, 16, 20],
    [6, 42, 17, 20],
    [6, 43, 18, 20],
    [6, 44, 19, 21],
    [6, 45, 20, 21],
    [6, 46, 21, 21],
    [6, 47, 22, 21],
    [12, 80, 33, 17],
    [13, 81, 34, 17],
    [6, 40, 17, 20],
    [6, 41, 17, 20],
    [6, 42, 17, 20],
    [6, 43, 17, 20],
];

/// `local_c`, the table's fourth column: the `L2.eng` 30 index drawn after the
/// heading
pub mod mode {
    pub const BARREN: usize = 0x11;
    pub const FALLOW: usize = 0x12;
    pub const WHEAT: usize = 0x13;
    pub const RECLAIMING: usize = 0x14;
    pub const CATTLE: usize = 0x15;
}

pub const FALLOW_BODY_PLAIN: usize = 0x3A;
pub const FALLOW_BODY_ADVANCED: usize = 0x22;

/// `L2.eng` group 77 — the grain and herd report's words.
pub const REPORT_GROUP: usize = 77;
/// `L2.eng` group 22 — the seven fertility phrases.
pub const FERTILITY_GROUP: usize = 22;

pub const WORKERS_SHORT: u8 = 0xF9;
pub const WORKERS_IDLE: u8 = 0xFC;

pub const REPORT_NEG: u8 = 0xF9;

/// **One string of this arm's vocabulary**: the player's `L2.eng` first, our
/// transcription where the file has nothing at that index.
pub fn words(a: &crate::shell::ShellAssets, group: usize, index: usize) -> String {
    let s = a.text(group, index);
    if !s.is_empty() {
        return s.to_string();
    }
    let ours = match group {
        TILE_GROUP => TILE_WORDS.iter().find(|&&(i, _)| i == index).map(|&(_, s)| s),
        REPORT_GROUP => REPORT_WORDS.get(index).copied(),
        FERTILITY_GROUP => FERTILITY_WORDS.get(index).copied(),
        CASTLE_GROUP => Some(super::super::job::ours(group, index)),
        _ => None,
    };
    ours.unwrap_or("").to_string()
}


