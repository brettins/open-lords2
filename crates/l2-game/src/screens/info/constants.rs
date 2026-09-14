#![allow(unused_imports)]
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

/// `Ui_OkButton(0x1AC, 0x1B6, 0)`, in **both** halves.
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

/// The heading's column, and the icon's.
pub const HEADING_X: i32 = 0x28;
pub const HEADING_DY: i32 = 0x40;
pub const ICON_AT: (i32, i32) = (0x28, 0x60);
/// The wrapped description: `FUN_0040328E(group, i, 0x68, row*16 + 100, W, …)`.
///
/// **The width is not one number, and this line used to say it was.** The two
/// halves wrap differently and every call in each half agrees with its own:
/// `UnitPanel_Draw`'s five calls all pass `0x120` and `TileInfo_Draw`'s three
/// all pass `0x130` — plus `0x150` for the village variant and `0x140` for the
/// mercenary line. The constant was read off a group-31 call site and then used
/// as though it belonged to both. [`TILE_BODY_WRAP`] is the tile half's.
pub const BODY_X: i32 = 0x68;
pub const BODY_DY: i32 = 100;
/// `UnitPanel_Draw`'s, group 31.
pub const BODY_WRAP: i32 = 0x120;
/// `TileInfo_Draw`'s, group 30 — sixteen pixels wider.
pub const TILE_BODY_WRAP: i32 = 0x130;

/// `Icon_tmp.pl8` — **57 raw frames**, re-read from disk by both painters on
/// every repaint. `crate::shell::ShellAssets` already loads it and, until this
/// module, no frame of it was ever drawn.
pub const ICON_SHEET: &str = "Icon_tmp.pl8";

/// The five unit-kind icons, identified by rendering the frames.
pub mod icon {
    pub const MERCHANT: usize = 0x23;
    pub const TRANSPORT: usize = 0x24;
    pub const PEASANTS: usize = 0x25;
    pub const ENEMY_ARMY: usize = 0x26;
    pub const OWN_ARMY: usize = 0x27;
    /// The three action buttons: move, move-out-of-a-castle, disband, split.
    pub const MOVE: usize = 0x28;
    pub const SORTIE: usize = 0x29;
    pub const DISBAND: usize = 0x2A;
    pub const SPLIT: usize = 0x2B;
    /// The five brush buttons, and **the frame is not the brush id**:
    /// abandon 0 → `0x00`, reclaim `0x19` → `0x11`, fallow 1 → `0x01`,
    /// grain 2 → `0x0D`, pasture `0x13` → `0x16`. The lookup between them was
    /// not found in the binary; these are the frames the two brush painters
    /// pass literally.
    pub const BRUSH_ABANDON: usize = 0x00;
    pub const BRUSH_RECLAIM: usize = 0x11;
    pub const BRUSH_FALLOW: usize = 0x01;
    pub const BRUSH_GRAIN: usize = 0x0D;
    pub const BRUSH_PASTURE: usize = 0x16;
}

/// The unit half's per-kind constants: `(heading index, description index,
/// icon frame)`, and the row the panel opens at.
pub const MERCHANT: (usize, usize, usize) = (0, 12, icon::MERCHANT);
pub const PEASANTS: (usize, usize, usize) = (5, 13, icon::PEASANTS);
pub const TRANSPORT: (usize, usize, usize) = (2, 14, icon::TRANSPORT);
pub const ENEMY_ARMY: (usize, usize, usize) = (6, 15, icon::ENEMY_ARMY);
pub const OWN_ARMY: (usize, usize, usize) = (6, 16, icon::OWN_ARMY);

/// 31/9, *"An army from"* — **drawn for an enemy army too**, which is the
/// correction above.
pub const ARMY_FROM: usize = 9;
/// 31/20 *"Formed"*, 31/8 *"Wages"*, 31/22 *"moves left."* — all three inside
/// the ownership gate.
pub const FORMED: usize = 0x14;
pub const WAGES: usize = 8;
pub const MOVES_LEFT: usize = 0x16;
/// 31/23…26 the four supply states, 31/27…31 the five health states.
pub const SUPPLY0: usize = 0x17;
pub const HEALTH0: usize = 0x1B;
/// The starvation line's colour once the counter leaves zero —
/// `local_8 = 0xF9` in `UnitPanel_Draw` (`0x0041B19D`), the same red the county
/// panel's negative deltas take.
pub const STARVING: u8 = 0xF9;
/// 16/0 *"No mercenaries in the army."*, 16/1…12 the nationalities.
pub const MERC_GROUP: usize = 16;

/// **The unit half's heading-face lines**, every one `&g_fontHeading` in
/// `UnitPanel_Draw` (`0x0041B19D`) and every y `R * 0x10 + k`.
///
/// The transport's 31/2 and its destination county sit at `(0x18, 0x30)`,
/// sixteen pixels left of and sixteen above where the merchant's and the
/// peasants' heading goes ([`HEADING_X`], [`HEADING_DY`]).
pub const TRANSPORT_HEADING_AT: (i32, i32) = (0x18, 0x30);
/// An army's name: `Eng_DrawString((char)owner + 0x5D, nameIndex, 0x28,
/// R * 0x10 + 0x30, &g_fontHeading, 0x3F)`. `L2.eng` 94…98 are the five realms'
/// 24 names each (`docs/formats/eng.md` §5).
pub const ARMY_NAME_GROUP: usize = 0x5D;
pub const ARMY_NAME_DY: i32 = 0x30;
/// The mercenary line, inside the ownership gate and after everything else:
/// 16/0 alone, or `Ui_DrawNumber(mercMen, '@', &DAT_004D422C, 0x38, …)`, 16/band
/// and `Ui_DrawUnitNoun(mercMen, mercTroop * 2 + 0x34, …)`. `DAT_004D422C` is a
/// NUL, read out of the image.
pub const MERC_LINE_AT: (i32, i32) = (0x38, 0x130);

/// **The county-town arm of the tile half**, and the only part of the group-30
/// ladder this module draws.
///
/// `TileInfo_Draw`'s `(flags & 0x40)` branch is four literals:
///
/// ```c
/// else { local_20 = 7; local_1c = 0x1b; local_8 = 0x1b; local_c = 0; }
/// ```
///
/// — heading 30/7 *"County town."*, body 30/27 *"Your troops may capture a
/// castleless county by attacking its county town."*, and `Icon_tmp.pl8` frame
/// `0x1B` through `Sprite_WGenSprite(local_8, 0x28, row * 0x10 + 0x60)`.
pub const COUNTY_TOWN_HEADING: usize = 7;
pub const COUNTY_TOWN_BODY: usize = 0x1B;
pub const COUNTY_TOWN_ICON: usize = 0x1B;

/// **The rest of `TileInfo_Draw`'s ladder: heading, body and `Icon_tmp.pl8`
/// frame, one row per [`TileKind`] the flags reach.**
///
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

/// The fall-through at the bottom of `TileInfo_Draw`'s flag ladder: heading
/// 30/0, body 30/22, icon `0x17`. No bit of the plane-0 byte is set.
pub const SCRUBLAND_INFO: (usize, usize, usize) = (0, 0x16, 0x17);

/// `Map_ResolvePick`'s `g_pickedTileGraphic == 0x10` — the terrain byte of an
/// *occupied* dwelling plot. Anything else on a `0x10` tile is the ruin.
pub const VILLAGE_GRAPHIC: u8 = 0x10;

/// **The castle arm's three headings and its icon.** `TileInfo_Draw`'s `0x80`
/// branch at `0x0C < graphic < 0x1A`: 30/8 *"Castle."*, or 30/14 and 30/15
/// while `castleDegraded` is 1 or 2, and `Icon_tmp.pl8` frame `0x1C`. The body
pub const CASTLE_HEADING: usize = 8;
pub const CASTLE_HEADING_BUILDING: usize = 0x0E;
pub const CASTLE_HEADING_REPAIR: usize = 0x0F;
pub const CASTLE_ICON: usize = 0x1C;
/// 71/16 *"Boosts tax revenues by"*, 71/11 *"Barracks for"* + 71/12
/// *"troops."*, 71/13 *"currently stationed here."*, 71/19 *"Enemy troops are
/// barracked here."* — `TileInfo_DrawCastle`'s own indices.
pub const CASTLE_TAX_BONUS: usize = 0x10;
pub const CASTLE_BARRACKS: usize = 0x0B;
pub const CASTLE_TROOPS: usize = 0x0C;
pub const CASTLE_STATIONED: usize = 0x0D;
pub const CASTLE_ENEMY_BARRACKED: usize = 0x13;

/// **`TileInfo_Draw`'s `0x80` arm below `0x0D` — the four resource sites**, as
/// `(heading, body base, `Icon_tmp.pl8` frame)` indexed by
/// [`l2_kingdom::tables::Commodity::index`].
///
/// ```c
/// else if (g_pickedTileGraphic < 0xd) {
///   if      (g < 4)  { local_20 = 9;    local_18 = 1; local_1c = 0x35; local_8 = 0x1e; Sound_RestartSlot(10); }
///   else if (g < 7)  { local_20 = 10;   local_18 = 3; local_1c = 0x3d; local_8 = 0x1f; Sound_RestartSlot(8);  }
///   else if (g < 10) { local_20 = 0x52; local_18 = 2; local_1c = 0x53; local_8 = 0x12; Sound_RestartSlot(8);  }
///   else             { local_20 = 0xb;  local_18 = 0; local_1c = 0x45; local_8 = 0x20; Sound_RestartSlot(9);  }
/// ```
///
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
    // wood — "Lumber mill (timber)." graphic 10…12
    (0x0B, 0x45, 0x20),
    // iron — "Mine (iron)." graphic 0…3
    (0x09, 0x35, 0x1E),
    // weapons — "Blacksmith (armour)." graphic 7…9
    (0x52, 0x53, 0x12),
    // stone — "Quarry (stone)." graphic 4…6
    (0x0A, 0x3D, 0x1F),
];

/// **The body is a SIZE band, not a fertility band.** The offset added to a
/// site's [`SITE_INFO`] base:
///
/// ```c
/// iVar2 = industry[c].total - industry[c].totalSnapshot;
/// if (industry[c].disabledSeasons == 0) {
///     if (9 < iVar2) {
///         if      (iVar2 < 0x19) local_1c += 1;
///         else if (iVar2 < 0x32) local_1c += 2;
///         else                   local_1c += 3;
///     }
/// } else                         local_1c += 4;
/// ```
///
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
/// `10`, `0x19`, `0x32` — the half-open edges of the three upper bands.
pub const SITE_BAND_EDGES: [i32; 3] = [10, 0x19, 0x32];

/// **The working-or-idle line, a separate block at the painter's tail** keyed
/// on `industry[c].enabled` and **not** on `disabledSeasons`:
///
/// ```c
/// if ((char)enabled < 1) Eng_DrawString(0x1e, 0x4d, 0x68, R*0x10 + 0x74, &g_fontBody, 0x3f);
/// else                   Eng_DrawString(0x1e, 0x4e, 0x68, R*0x10 + 0x74, &g_fontBody, 0x3f);
/// ```
///
/// 30/77 *"This industry is shut down."*, 30/78 *"This industry is
/// operational."* **A site can read operational and destroyed in the same
/// panel** — two different bytes, and that is the original's.
pub const SITE_SHUT_DOWN: usize = 0x4D;
pub const SITE_OPERATIONAL: usize = 0x4E;
pub const SITE_STATUS_DY: i32 = 0x74;

/// **The mercenary tail — the words the marker on the map does not carry.**
///
/// `TileInfo_Draw`'s last block before the icon, and the *only* place in the
/// binary that says in English why there is a figure standing in the town:
///
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
/// `(0x32, row * 0x10 + 0x9C)` and `(0x68, row * 0x10 + 0xA0)`, wrap `0x140`.
pub const MERC_MARKER_AT: (i32, i32) = (0x32, 0x9C);
pub const MERC_TEXT_AT: (i32, i32, i32) = (0x68, 0xA0, 0x140);

/// `local_c = max(0, 15 - movesUsed)` — the moves-left line, drawn **only when
/// the army is not garrisoned**.
pub const MOVE_ALLOWANCE: i32 = 15;

/// The three army buttons, 48 pixels square. `g_infoUnitButtons`
/// (`0x004DC560`) is **kind 1**: they fire on left *press*.
pub const BUTTON_DIM: i32 = 40;
pub const BUTTON_X: [i32; 3] = [48, 112, 176];
pub const BUTTON_DY: i32 = 352;

/// **The brush's absolute row**, which is 192 pixels below where `map.rs` puts
/// it. Both variants land here; see the module docs.
pub const BRUSH_ROW_Y: i32 = 376;
/// `Ui_DrawBevelRect(0x28, …, 0x180, 0x40)` — the bevel behind the buttons.
pub const BRUSH_BEVEL: Rect = Rect::new(40, 368, 384, 64);
/// The caption, `L2.eng` 30/52 — *"Click on an icon to alter field usage."*
pub const BRUSH_CAPTION_AT: (i32, i32, i32) = (48, 386, 192);
pub const BRUSH_CAPTION: usize = 52;
/// The three buttons of a live field and the two of waste, at their x columns.
/// `Hotspot_Test` is half-open and every box is 48 square.
pub const BRUSH_FIELD_X: [i32; 3] = [240, 304, 368];
pub const BRUSH_WASTE_X: [i32; 2] = [304, 368];
pub const BRUSH_DIM: i32 = 48;

/// What one brush button paints — the raw terrain the hotspot record carries.
pub const BRUSH_FIELD_ID: [u8; 3] = [1, 2, 0x13];
pub const BRUSH_WASTE_ID: [u8; 2] = [0x19, 0];

/// **`DAT_004D2EC8` — `TileInfo_Draw`'s farmland table**, one row per terrain
/// value `0 … 0x1C`: `[heading, body, icon, mode]`, the four `i32`s the painter
/// reads as `local_20`, `local_1c`, `local_8` and `local_c`.
///
/// Transcribed from the player's `Lords2.exe` and asserted against it,
/// row for row, by `tests/screens_info.rs`
/// `the_farmland_table_is_the_images_own`. Row `0x1D` onward is other data —
/// the image carries no bound, and `Terrain_Set` writes nothing above `0x1C`
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
    /// *"- Barren."*
    pub const BARREN: usize = 0x11;
    /// *"- Lying Fallow."* — the body is swapped on `g_optAdvancedFarming`.
    pub const FALLOW: usize = 0x12;
    /// *"- Wheat."* — `TileInfo_DrawGrain`, and no body.
    pub const WHEAT: usize = 0x13;
    /// *"- Being reclaimed."*
    pub const RECLAIMING: usize = 0x14;
    /// *"- Cattle."* — `TileInfo_DrawHerd`, and no body.
    pub const CATTLE: usize = 0x15;
}

/// The fallow body's two strings: 30/58 *"Traditionally, fields were left
/// fallow…"* with advanced farming off, 30/34 *"Presently being rested…"* with
/// it on. `if (local_c == 0x12) local_1c = g_optAdvancedFarming == 0 ? 0x3A : 0x22;`
pub const FALLOW_BODY_PLAIN: usize = 0x3A;
pub const FALLOW_BODY_ADVANCED: usize = 0x22;

/// `L2.eng` group 77 — the grain and herd report's words.
pub const REPORT_GROUP: usize = 77;
/// `L2.eng` group 22 — the seven fertility phrases.
pub const FERTILITY_GROUP: usize = 22;

/// `TileInfo_DrawGrain`'s and `TileInfo_DrawHerd`'s worker colour: `0x3F`, or
/// `0xF9` while the job is short of its floor, or `0xFC` while it has more
/// hands than it can use. Literals of the two painters, not the strip's.
pub const WORKERS_SHORT: u8 = 0xF9;
pub const WORKERS_IDLE: u8 = 0xFC;

/// `Ui_DrawDelta`'s `colourNeg` at all five report call sites; `colourPos` is
/// `0x3F`, [`font::TEXT`].
pub const REPORT_NEG: u8 = 0xF9;

/// **Our transcription of the words this arm draws**, used only where the
/// player's `L2.eng` has none — an install with no file, or a placeholder test
/// asset. Group 30's are the eighteen indices the farmland arm can reach.
const TILE_WORDS: [(usize, &str); 49] = [
    (6, "Farmland"),
    (9, "Mine (iron)."),
    (10, "Quarry (stone)."),
    (11, "Lumber mill (timber)."),
    (12, "Flooded Field."),
    (13, "Parched Field."),
    (17, "- Barren."),
    (18, "- Lying Fallow."),
    (19, "- Wheat."),
    (20, "- Being reclaimed."),
    (21, "- Cattle."),
    (33, "Unusable at present. Farmers may be diverted from other tasks to reclaim this field."),
    (34, "Presently being rested. The more fallow land in a county, the higher its fertility for growing wheat."),
    (35, "This wheat field is currently unused."),
    (36, "This wheat field has just been sown."),
    (37, "This wheat field contains ripening crops."),
    (38, "This wheat field will be harvested next season."),
    (39, "This wheat field is ready for planting next season."),
    (40, "Over time and with continued labor, this field will return to a usable state and increase your overall farming capacity."),
    (41, "This field has improved somewhat, but needs further work before it is usable."),
    (42, "Partially restored, this field is on the way to returning to its former state."),
    (43, "Almost reclaimed, this field will very shortly be ready for use."),
    (52, "Click on an icon to alter field usage."),
    // The four size bands and the destroyed one, per site — `SITE_INFO`'s base
    // plus `SITE_BAND_*`.
    (53, "A small mine."),
    (54, "A medium mine."),
    (55, "A large mine."),
    (56, "A very large mine."),
    (57, "A destroyed mine."),
    (58, "Traditionally, fields were left fallow for a season as part of a crop rotation system."),
    (61, "A small quarry."),
    (62, "A medium quarry."),
    (63, "A large quarry."),
    (64, "A very large quarry."),
    (65, "A destroyed quarry."),
    (69, "A small lumber mill."),
    (70, "A medium lumber mill."),
    (71, "A large lumber mill."),
    (72, "A very large lumber mill."),
    (73, "A destroyed lumber mill."),
    (77, "This industry is shut down."),
    (78, "This industry is operational."),
    (80, "This field was flooded in the recent deluge, and any crops held within it were drowned."),
    (81, "This field was baked dry in the recent drought, and any crops held within it withered and died."),
    (82, "Blacksmith (armour)."),
    (83, "A small blacksmiths."),
    (84, "A medium blacksmiths."),
    (85, "A large blacksmiths."),
    (86, "A very large blacksmiths."),
    (87, "A destroyed blacksmiths."),
];

/// Group 77, indices 0 … 28.
const REPORT_WORDS: [&str; 29] = [
    "from",
    "to be sown, yielding",
    "in 4 seasons.",
    "harvested in",
    "sown in spring.",
    "Calf births expected",
    "Cow deaths expected",
    "Change due to farming",
    "Low herd crowding.",
    "Average herd crowding.",
    "Herd overcrowded.",
    "Massive overcrowding!!",
    "field being reclaimed",
    "fields being reclaimed",
    "Next field reclaimed in",
    "No field reclamation with 0 labourers",
    "gained last season, due to weather.",
    "lost last season, due to weather.",
    "Weather had no effect last season.",
    "No outside events affected the herd this season.",
    "died of disease.",
    "taken by wolves.",
    "had to be put down.",
    "born, over expectations.",
    "No outside factors affected stored grain.",
    "eaten by rats.",
    "found as surplus.",
    "Change due to eating",
    "Overall change",
];

/// Group 22, indices 0 … 6.
const FERTILITY_WORDS: [&str; 7] = [
    "Infertile - almost no production.",
    "Very poor fertility - mainly weeds.",
    "Poor fertility - crops grow less well.",
    "Average fertility - no effect on crops.",
    "Good fertility - crops are boosted.",
    "Very High fertility - many extra crops.",
    "Excellent fertility - bumper crop!",
];

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
        // `TileInfo_DrawCastle` and `Castle_DrawStatusBlock` draw the same
        // group on two screens; the transcription lives with the other one.
        CASTLE_GROUP => Some(super::job::ours(group, index)),
        _ => None,
    };
    ours.unwrap_or("").to_string()
}

