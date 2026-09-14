//! Drawing a battle: the tile viewport, then the men on it.
//!
//! The geometry is the original's, taken from the arguments
//! `Battle_LoadAssets` (`0x004987B7`) passes to the tile renderer's setup
//! (`0x004BC020`):
//!
//! ```text
//! FUN_004bc020(tileset, tileset2, &g_battlefield, 0x50, 0x50, 8, 0, 0x18, 0xf, 0xe, 0x20)
//!                                                  80    80   8  0    24   15   14    32
//! ```
//!
//! **[V]** a 15 x 14 viewport of 32-pixel tiles, pinned at screen `(0, 24)`,
//! over an 80 x 80 map of 8-byte cells. The renderer that consumes it
//! (`0x004BCBDC`) steps the destination by `0x20` per tile in both axes, so the
//! battlefield is a **plain square grid seen from above** — not isometric.
//!
//! Figures are drawn between the two terrain passes, sorted by map y ascending
//! (`0x004BDA92`), which makes a man lower on the field overlap one behind him.
//! **[V]**
//!
//! # Three things a player saw, and the functions they are
//!
//! * **Where a walking man is.** `BattleMan_Step` (`0x0048F1DD`) *enters* the
//!   next cell before it walks — see [`drawn_cell`] — so the picture trails him
//!   behind the cell he is in. Ours drew the trail behind the cell he was
//!   leaving, and a player saw every man *"reset on their square once as they
//!   move"*.
//! * **The clip.** `FUN_004BC020` also stores the viewport as the rectangle
//!   every battle sprite is clipped to — [`FIELD_CLIP`]. Ours blitted men
//! unclipped into the menu bar, the right column and the strip under the
//!   field, where nothing repaints them: the *"ghosting"*.
//! * **The palette** is not here: this crate draws indices, and which `.256`
//!   they mean is the presenter's, which resolves [`TILE_PALETTE`] through
//!   `l2_game::shell::PALETTES` like every other page's name.

mod render;
pub use render::*;

use l2_formats::Palette;
use l2_sim::runner::{BattleRunner, Fighter};
use l2_sim::terrain::{Battlefield, DIM};
use l2_sim::{Troop, SIDE_A};

use crate::canvas::{Canvas, Clip};
use crate::engines;
use crate::figures::{self, Anim, Colour};
use crate::missiles;
use crate::sheet::Sheet;

pub const TILE: i32 = 32;
pub const VIEW_COLS: usize = 15;
pub const VIEW_ROWS: usize = 14;
pub const ORIGIN_X: i32 = 0;
pub const ORIGIN_Y: i32 = 24;

/// **The rectangle every man, horse and missile on the field is clipped to.**
///
/// `FUN_004BC020` (`0x004BC020`) stores it beside the geometry —
/// `DAT_004E6564 = param_7`, `DAT_004E5D54 = param_9 * param_11 + param_7`,
/// `DAT_004E5D48 = param_8`, `DAT_004E5D6C = param_10 * param_11 + param_8` —
/// which for the battle's arguments is `x 0 … 480`, `y 24 … 472`. And
/// `BattleFigure_Draw` (`0x004BDC31`), the horse under a knight
/// (`FUN_004BE4DF`) and the tile renderer's overlay blit all call
/// `Clip_Horizontal(DAT_004E6564, DAT_004E5D54)` and
/// `Clip_Vertical(DAT_004E5D48, DAT_004E5D6C)` before they blit. **[V]**
///
/// A man in the top row is drawn sixteen pixels above his cell, and one a cell
/// outside the view is still collected (`FUN_004BD938`), so without this the
/// menu bar, the right column and the bottom strip took his pixels — and
/// nothing on the battlefield screen repaints any of those.
pub const FIELD_CLIP: Clip = Clip::new(
    ORIGIN_X,
    ORIGIN_Y,
    ORIGIN_X + VIEW_COLS as i32 * TILE,
    ORIGIN_Y + VIEW_ROWS as i32 * TILE,
);

/// The field-battle tileset. [`Ground`] carries the siege pair —
/// `t32_stn1`/`t32_stn2` and `t32_wod1`/`t32_wod2`.
pub const TILESET: &str = "T32_bat1.pl8";

/// **The two battle palettes**, records 2 and 1 of `g_preloadTable`
/// (`0x004D9F48`) — the filenames are the table's own bytes.
///
/// **Neither is `Battle_LoadAssets`'.** `Res_LoadStatic` (`0x00499859`)
/// preloads both at start-up, into `0x00568EE0` and `0x005675A0`, and
/// `Screen_DrawBattlefield` (`0x004233F7`) ends every repaint with
/// `if (g_battleIsSiege == 0) Palette_Set(0x568ee0); else
/// Palette_Set(0x5675a0);`. **[V]**
///
/// C200 loaded the siege one over the *field* tileset, because `T32_stn1.pl8`
/// was not ported — the original's colour over the wrong tiles, which still
/// measured better than a whole siege in the field's colours. **C201 ported
/// the tiles**, so the pairing is no longer a compromise: a siege is drawn
/// from its castle's own sheets in its own palette. One palette serves both
/// castle families; See [`Ground`].
pub const TILE_PALETTE: &str = "T32_bat1.256";
pub const SIEGE_PALETTE: &str = "T32_stn1.256";

/// **Which ground a battle is fought on, and therefore which sheets and which
/// palette** — `Battle_LoadAssets` (`0x004987B7`), slots 0/1 and 0x0B/0x0C of
/// the battle asset table at `0x004DA550`:
///
/// ```c
/// if (slot < 2) {                                  /* the 32-pixel tiles */
///   if (g_battleIsSiege == 0)      { if (slot == 1) continue; entry = 0;   }
///   else if (DAT_0057C910 == 0)    { entry = slot == 1 ? 5 : 4;            }
///   else                           { entry = slot == 1 ? 3 : 2;            }
/// }
/// ```
///
/// and the same ladder again at slots `0x0B`/`0x0C` with entries
/// `0x0B / 0x0D,0x0E / 0x0F,0x10` for the overview panel. The table's entries
/// 0…5 are `t32_bat1, t32_bat2, t32_stn1, t32_stn2, t32_wod1, t32_wod2` and
/// `0x0B`…`0x10` the matching `t2_*`. **[V]** — read out of `Lords2.exe`.
///
/// `DAT_0057C910` is `Siege_LaunchAssault`'s `(uint)(1 < g_castleLevel)`
/// (`0x004A8AAB`), so **levels 0 and 1 are the wooden sheets and 2, 3 and 4 the
/// stone ones**. `FUN_00498DCB`, the skirmish loader, picks the same two
/// families from `DAT_0057C96C`, its copy of the same flag. **[V]**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ground {
    /// `t32_bat1.pl8`. Slot 1 is skipped: `t32_bat2.pl8`'s size in the table is
    /// **0** and the install does not ship the file. **[V]**
    Field,
    /// Castle levels 2, 3 and 4 — `t32_stn1` over `t32_stn2`.
    Stone,
    /// Castle levels 0 and 1 — `t32_wod1` over `t32_wod2`.
    Wood,
}

impl Ground {
    pub const ALL: [Ground; 3] = [Ground::Field, Ground::Stone, Ground::Wood];

    /// `g_battleIsSiege` and `DAT_0057C910`, as the loader reads them.
    pub fn for_battle(castle_level: Option<u8>) -> Ground {
        match castle_level {
            None => Ground::Field,
            Some(level) if level > 1 => Ground::Stone,
            Some(_) => Ground::Wood,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Ground::Field => 0,
            Ground::Stone => 1,
            Ground::Wood => 2,
        }
    }

    /// Table entry 0, 2 or 4 — **the castle**: masonry, towers, gates, doors,
    /// the keep and the wall tops. See [`Ground::tileset2`].
    pub fn tileset(self) -> &'static str {
        match self {
            Ground::Field => TILESET,
            Ground::Stone => "T32_stn1.pl8",
            Ground::Wood => "T32_wod1.pl8",
        }
    }

    /// Table entry 3 or 5 — **the ground the castle stands on**: sixteen grass
    /// variants at 0…15, the moat's 49-variant water set and the rubble a
    /// collapsed wall leaves. **Not** the drawbridge:
    /// `Siege_LowerDrawbridge` (`FUN_00496B9F`) clears the selector when it
    /// lays its patch, so that is the castle sheet.
    ///
    /// That split is the builder's, not a guess: `Battlefield_BuildCastle`'s
    /// three escape codes all write this sheet's selector —
    /// `FUN_0047E1DC` (`0xEF`, open ground) takes `rand & 0x0F`,
    /// `FUN_0047DCCE` (`0xEE`, moat) auto-tiles water out of the same 49-entry
    /// table `0x004D7610` a field battle uses, and `Wall_Collapse`
    /// (`FUN_0047DFE0`) auto-tiles rubble out of `0x004D7930` — while every
    /// cell taken straight from the raster keeps selector 0 and the castle
    /// sheet. **[V]**
    pub fn tileset2(self) -> Option<&'static str> {
        match self {
            Ground::Field => None,
            Ground::Stone => Some("T32_stn2.pl8"),
            Ground::Wood => Some("T32_wod2.pl8"),
        }
    }

    /// `Screen_DrawBattlefield` (`0x004233F7`): `Palette_Set(0x568EE0)` for a
    /// field battle and `Palette_Set(0x5675A0)` for a siege. **Both castle
    /// families use `t32_stn1.256`** — in the
    /// preload table or in the install. **[V]**
    pub fn palette(self) -> &'static str {
        match self {
            Ground::Field => TILE_PALETTE,
            Ground::Stone | Ground::Wood => "T32_stn1.256",
        }
    }

    pub fn overview_tileset(self) -> &'static str {
        match self {
            Ground::Field => OVERVIEW_TILESET,
            Ground::Stone => "T2_stn1.pl8",
            Ground::Wood => "T2_wod1.pl8",
        }
    }

    pub fn overview_tileset2(self) -> Option<&'static str> {
        match self {
            Ground::Field => None,
            Ground::Stone => Some("T2_stn2.pl8"),
            Ground::Wood => Some("T2_wod2.pl8"),
        }
    }
}

/// **The overview panel's two sheets.** `Battle_LoadAssets` (`0x004987B7`)
/// registers them with
///
/// ```text
/// FUN_004BC107(DAT_0053F044, DAT_0056D680, DAT_0056D5A0, 0x1E0, 0x18, 2)
///              t2_bat1.pl8   t2_bat2.pl8   t2_spri.pl8   480    24    mode
/// ```
///
/// and the three buffers are entries `0x0B`, `0x0C` and `0x11` of the battle
/// asset table at `0x004DA550` — `t2_bat1.pl8`, `t2_bat2.pl8`, `t2_spri.pl8`
/// for a field battle, `t2_stn1`/`t2_stn2` or `t2_wod1`/`t2_wod2` in place of
/// the first two for a siege. **[V]** from the table's bytes and the loader's
/// `local_10 → local_18` ladder.
///
/// **The second sheet is never drawn in a field battle.** `t2_bat2.pl8`'s size
/// in that table is `0`, the file is not in the install, and the loader's
/// non-siege arm jumps over its slot entirely; `FUN_004BC51A` reaches it only
/// when a cell's flag byte has `flags & 0x1C == 4`. So the field's raster is
/// `t2_bat1.pl8` alone. **[V]**
pub const OVERVIEW_TILESET: &str = "T2_bat1.pl8";
pub const OVERVIEW_SPRITES: &str = "T2_spri.pl8";

/// Two pixels a cell, at `(0x1E0, 0x18)` — `FUN_004BC51A`'s
/// `g_drawY = row * 2 + _DAT_004E5D60`, `g_drawX = DAT_004E5D68` then `+= 2`
/// a column. 80 × 80 cells makes a 160 × 160 raster, which ends exactly where
/// `Screen_DrawBattlefield` puts `Misc_bat.pl8` frame 0, at `(0x1E0, 0xB8)`.
/// **[V]**
pub const OVERVIEW_ORIGIN_X: i32 = 0x1E0;
pub const OVERVIEW_ORIGIN_Y: i32 = 0x18;
pub const OVERVIEW_SCALE: i32 = 2;
pub const OVERVIEW_SIDE: usize = DIM * OVERVIEW_SCALE as usize;

/// **Four rows a frame** — `Battle_Frame` (`0x004B99C0`) calls
/// `FUN_004BC1D1(4)` once a frame while `g_battlePhase == 2` and the screen is
/// `0x28 … 0x2A`, and `FUN_004BC1D1` advances a row cursor by its argument,
/// wraps it to zero at `DAT_004E6570 − n` (the map's 80 rows), and paints
/// `FUN_004BC51A(cursor, n)`. A full sweep of the panel therefore takes
/// **20 frames**. **[V]**
pub const OVERVIEW_ROWS_PER_FRAME: usize = 4;

/// The two 2 × 2 sheets the overview panel is built from. Optional on
/// [`BattleAssets`] because an install that lacks them still plays — the panel
/// falls back to the flat fill.
pub struct OverviewSheets {
    /// `t2_bat1.pl8`: 252 frames of 2 × 2, one for each of `T32_bat1.pl8`'s 252
    /// 32 × 32 tiles, so a cell's `gfx` byte indexes both. **[V]** from the two
    /// files' headers.
    pub tiles: Sheet,
    /// The second 2 × 2 sheet, `t2_stn2` / `t2_wod2`, reached by the same cell
    /// selector the 32-pixel renderer uses — `FUN_004BC51A` takes it when
    /// `cell[+2] & 0x1C == 4`. `None` for a field battle, whose slot is empty.
    /// **[V]**
    pub tiles2: Option<Sheet>,
    /// `t2_spri.pl8`: seven frames of 2 × 2. Frame 0 is the erase tile
    /// `FUN_004BC51A` stamps on a cell a man has just left; frames 1 … 6 are
    /// flat colours, indexed by the owning realm's `shieldIndex` — or 6 for the
    /// neutral owner. **[V]**
    pub men: Sheet,
}

/// Where the camera's top-left tile is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Camera {
    pub x: usize,
    pub y: usize,
}

impl Camera {
    /// Clamp so the viewport never runs off the 80 x 80 map, which is what the
    /// original's scroll clamp does.
    pub fn clamped(x: i32, y: i32) -> Self {
        Camera {
            x: x.clamp(0, (DIM - VIEW_COLS) as i32) as usize,
            y: y.clamp(0, (DIM - VIEW_ROWS) as i32) as usize,
        }
    }

    /// Centre the viewport on a cell.
    pub fn centred_on(x: usize, y: usize) -> Self {
        Camera::clamped(x as i32 - VIEW_COLS as i32 / 2, y as i32 - VIEW_ROWS as i32 / 2)
    }
}

/// One [`Ground`]'s three sheets and its palette — what `Battle_LoadAssets`
/// puts in slots 0, 1, `0x0B`, `0x0C` and the palette
/// `Screen_DrawBattlefield` ends on.
pub struct GroundArt {
    pub palette: Palette,
    pub tiles: Sheet,
    /// Slot 1. `None` for a field battle; see [`Ground::tileset2`].
    pub tiles2: Option<Sheet>,
    /// [`OverviewSheets`], when the install has the files.
    pub overview: Option<OverviewSheets>,
}

/// Everything a battle needs to draw. Sheets are loaded once and their frames
/// decoded on demand.
///
/// The three [`Ground`]s are all loaded up front because the original loads
/// one *per battle* — `Battle_LoadAssets` runs from `Battle_Start` — and we
/// have no per-battle load step. The troop sheets are shared across them,
/// which is what the original's table does too: slots `0x12`…`0x1F` do not
/// move with the siege flag.
pub struct BattleAssets {
    /// Indexed by [`Ground::index`]. `Field` is always present — [`load`] fails
    /// without it; the siege pair is `None` on an install that lacks them.
    ///
    /// [`load`]: BattleAssets::load
    grounds: [Option<GroundArt>; 3],
    /// One sheet per troop type that has one, for each side. `None` for the
    /// siege engines, which are drawn from `engine` instead.
    side4: Vec<Option<Sheet>>,
    side0: Vec<Option<Sheet>>,
    horse: Option<Sheet>,
    /// `A2_miss.pl8` — every missile in the game, both sides, all five drawn
    /// classes. [`crate::missiles`].
    ///
    /// These three are on `BattleAssets` and not on [`GroundArt`] because they
    /// do not move with the ground: slots `0x12`…`0x1F` of the asset table are
    /// the same whichever way `g_battleIsSiege` falls. The overview sheets
    /// *do* move, and live on [`GroundArt`] — C201.
    missiles: Option<Sheet>,
    /// `Engine.pl8` — troop types 7 … 10, **colourless**: `FUN_00480F8B`
    /// hands all four the same buffer in both banks. [`crate::engines`].
    engine: Option<Sheet>,
    /// `Catarm1.pl8` and `Catarm2.pl8`, the near and far halves of the
    /// catapult's arm.
    catarm: [Option<Sheet>; 2],
}

/// The seven troop types with battlefield sprites, indexed by
/// `Troop::index()`.
const SPRITE_TROOPS: [Troop; 7] = [
    Troop::Peasants,
    Troop::Crossbowmen,
    Troop::Macemen,
    Troop::Swordsmen,
    Troop::Pikemen,
    Troop::Archers,
    Troop::Knights,
];

impl BattleAssets {
    /// Load through a caller-supplied reader, so this works equally against a
    /// plain directory and against the mod overlay's case-insensitive VFS.
    pub fn load<F>(mut read: F, side4: Colour, side0: Colour) -> Result<Self, String>
    where
        F: FnMut(&str) -> Result<Vec<u8>, String>,
    {
        let mut grounds: [Option<GroundArt>; 3] = [None, None, None];
        for g in Ground::ALL {
            let palette = match read(g.palette()).and_then(|b| {
                Palette::from_bytes(&b).map_err(|e| format!("{}: {e}", g.palette()))
            }) {
                Ok(p) => p,
                Err(e) if g == Ground::Field => return Err(e),
                Err(_) => continue,
            };
            let tiles = match read(g.tileset())
                .and_then(|b| Sheet::new(b).map_err(|e| format!("{}: {e}", g.tileset())))
            {
                Ok(s) => s,
                Err(e) if g == Ground::Field => return Err(e),
                Err(_) => continue,
            };
            let mut opt = |name: &str| read(name).ok().and_then(|b| Sheet::new(b).ok());
            let tiles2 = g.tileset2().and_then(&mut opt);
            let over_tiles = opt(g.overview_tileset());
            let over_two = g.overview_tileset2().and_then(&mut opt);
            let men = opt(OVERVIEW_SPRITES);
            let overview = match (over_tiles, men) {
                (Some(tiles), Some(men)) => {
                    Some(OverviewSheets { tiles, tiles2: over_two, men })
                }
                _ => None,
            };
            grounds[g.index()] = Some(GroundArt { palette, tiles, tiles2, overview });
        }

        let mut load_side = |colour: Colour| -> Result<Vec<Option<Sheet>>, String> {
            let mut out: Vec<Option<Sheet>> = (0..11).map(|_| None).collect();
            for troop in SPRITE_TROOPS {
                let Some(name) = figures::sprite_file(colour, troop) else { continue };
                let bytes = read(&name)?;
                out[troop.index()] = Some(Sheet::new(bytes).map_err(|e| format!("{name}: {e}"))?);
            }
            Ok(out)
        };
        let a = load_side(side4)?;
        let b = load_side(side0)?;
        let horse = read(figures::HORSE_FILE).ok().and_then(|b| Sheet::new(b).ok());
        let mut sheet = |name: &str| read(name).ok().and_then(|b| Sheet::new(b).ok());
        let missiles = sheet(missiles::MISSILE_SHEET);
        let engine = sheet(engines::ENGINE_SHEET);
        let catarm = [sheet(engines::ARM_SHEETS[0]), sheet(engines::ARM_SHEETS[1])];

        Ok(BattleAssets { grounds, side4: a, side0: b, horse, missiles, engine, catarm })
    }

    /// The art for one ground, falling back to the field's on an install that
    /// has no siege sheets — which is the placeholder case and looks it,
    ///
    pub fn ground(&self, g: Ground) -> &GroundArt {
        self.grounds[g.index()]
            .as_ref()
            .or(self.grounds[Ground::Field.index()].as_ref())
            .expect("the field ground is required by BattleAssets::load")
    }

    /// Whether this install supplied `g`'s own sheets, or [`BattleAssets::ground`]
    /// is falling back to the field's.
    pub fn has_ground(&self, g: Ground) -> bool {
        self.grounds[g.index()].is_some()
    }

    /// The field ground's palette and tiles, for the callers that never fight
    /// a siege.
    pub fn palette(&self) -> &Palette {
        &self.ground(Ground::Field).palette
    }

    pub fn tiles(&self) -> &Sheet {
        &self.ground(Ground::Field).tiles
    }

    /// `A2_miss.pl8` — the missiles and, at `0x21` and up, the banner.
    pub fn missiles(&self) -> Option<&Sheet> {
        self.missiles.as_ref()
    }

    pub fn overview(&self) -> Option<&OverviewSheets> {
        self.ground(Ground::Field).overview.as_ref()
    }

    /// The sheet a figure's body is drawn from.
    ///
    /// `FUN_00480F8B` (`0x00480F8B`): troop types 0 … 6 take the bank's
    /// colour sheet, **7 … 10 all take `engine.pl8` in both banks**, so a
    /// siege engine has no side colour and this does not consult `side`.
    fn sheet_for(&self, side: l2_sim::Side, troop: Troop) -> Option<&Sheet> {
        if troop.is_siege() {
            return self.engine.as_ref();
        }
        let bank = if side == SIDE_A { &self.side0 } else { &self.side4 };
        bank.get(troop.index()).and_then(|s| s.as_ref())
    }
}

/// **The damage overlay** — `Battlefield_Draw32`'s second pass over the same
/// cell, `frame = cell[+0] + 0x8B` out of slot 1, capped at `0x9A`, drawn
/// transparently and clipped whenever `cell[+0]` is non-zero and `cell[+4]` is
/// 1, 2 or 3.
///
/// **On a castle, cell byte `+0`
/// counter**, and the two writers say so:
///
/// * `Missile_Step` (`0x00492C8B`) — *"if (cell.elevation < 4) { cell.terrain++; if (0xF <
///   cell.terrain) Wall_Collapse(cell); }"*. A catapult shot chips the byte up
///   to 15.
/// * `BattleMan_StateFillMoat` (`0x00483FE1`) — *"if (cell.terrain <
///   g_moatFillSteps) { … cell.terrain++; } else { cell.terrain = 0;
///   Moat_Fill(cell); }"*. A shovelled load raises the same byte, from the 11
///   the castle builder seeds a ditch with to 15.
///
/// So `0x8C`…`0x9A` is one growing pile of rubble, and the sheet's sixteen
/// frames are a wall being knocked down and a ditch being filled in, animated
/// by the same addition. `0x8B` is unreachable: at `cell[+0] == 0` the pass
/// does not run.
///
/// **The elevation gate is the shot's own.** `Missile_Step` refuses to count a
/// hit on a rampart 4 or more high, and this refuses to draw damage on one —
/// the same `1 ..= 3`, from two unrelated functions. **[V]** on the formula
/// and the gate; `[I]` that "damage" is the right word for the picture.
///
/// A `.skr` field battlefield never reaches any of it: `elevation` is the one
/// cell byte `Battlefield_BuildFromSkr` never writes, which is also why the
/// field's empty slot-1 pointer is never dereferenced. **[V]**
pub const OVERLAY_BASE: usize = 0x8B;
pub const OVERLAY_CAP: usize = 0x9A;
pub const OVERLAY_ELEVATIONS: std::ops::RangeInclusive<u8> = 1..=3;

/// **The castle's banner** — `BattleBanner_Draw` (`FUN_004BD574`,
/// `0x004BD574`), the other arm of the overlap pass.
///
/// ```c
/// if (g_pulse80) DAT_004e5b18++;
/// if (DAT_004e5b18 < 0 || 7 < DAT_004e5b18) DAT_004e5b18 = 0;
/// DAT_005c9288 = (&DAT_0052f0b2)[g_battleArmyB * 0x1a4] * 8 + DAT_004e5b18 + 0x21;
/// blit(DAT_00566518 /* A2_miss.pl8 */, DAT_005c9288);
/// ```
///
/// `0x0052F0B2` is `g_units` (`0x0052F0B0`) `+0x02` — the shield, a copy of
/// realm `+0x0A` (`docs/armies.md`) — and `g_battleArmyB` is the **side-0**
/// army, the castle's garrison. So one castle flies one realm's colours, in an
/// eight-frame cycle off `g_pulse80`. **[V]** from the function's bytes; six
/// shields × 8 lands on `A2_miss.pl8`'s last frame, 80.
pub const BANNER_BASE: usize = 0x21;
pub const BANNER_PHASES: u8 = 8;

#[cfg(test)]
mod tests {
    use super::*;
    use l2_sim::runner::Army;

    #[test]
    fn the_viewport_matches_the_original_and_fits_the_screen() {
        assert_eq!(VIEW_COLS as i32 * TILE, 480);
        assert_eq!(VIEW_ROWS as i32 * TILE + ORIGIN_Y, 472);
        assert!(VIEW_COLS as i32 * TILE <= crate::canvas::WIDTH as i32);
        assert!(ORIGIN_Y + VIEW_ROWS as i32 * TILE <= crate::canvas::HEIGHT as i32);
    }

    /// The clip, against `FUN_004BC020`'s four stores worked out by hand for the
    /// battle's arguments `(…, 0, 0x18, 0xF, 0xE, 0x20)`.
    #[test]
    fn the_sprite_clip_is_the_one_fun_004bc020_stores() {
        assert_eq!(FIELD_CLIP, Clip::new(0, 0x18, 0xF * 0x20, 0xE * 0x20 + 0x18));
    }

    #[test]
    fn the_camera_never_lets_the_viewport_leave_the_map() {
        assert_eq!(Camera::clamped(-5, -5), Camera { x: 0, y: 0 });
        assert_eq!(Camera::clamped(500, 500), Camera { x: 65, y: 66 });
        let c = Camera::centred_on(40, 40);
        assert!(c.x + VIEW_COLS <= DIM && c.y + VIEW_ROWS <= DIM);
    }

    /// **A man walking east is drawn further east every tick, from the running
    /// simulation** — the ungated half of `tests/battle_picture.rs`, which finds
    /// him in the pixels.
    ///
    /// One maceman, ordered five cells east over open ground, stepped by the
    /// real runner. The origin this module draws him at must never move west,
    /// must move on at least eight sub-steps a cell, and must end exactly 160
    /// pixels east — the original's `g_walkOffset32` column `walking` 1, 3 … 15
    /// and then the cell.
    ///
    /// Ablation: return `(f.x + dx, f.y + dy)` with `walking = substep − 1`
    /// from [`drawn_cell`], the compensation this file carried while the
    /// runner crossed in the other order — red on the first sub-step, a whole
    /// cell east of where he is.
    #[test]
    fn a_man_walking_east_is_drawn_further_east_every_tick() {
        let mut layer = vec![0u8; l2_sim::terrain::CELLS];
        layer[36 * DIM + 40] = 0x04;
        layer[74 * DIM + 40] = 0x0F;
        let field = l2_sim::terrain::build(&layer, 1);
        let mut runner = BattleRunner::deploy_armies(
            field,
            0x5EED,
            Army { troops: &[(Troop::Peasants, 1)], owner: 2, human: false },
            Army { troops: &[(Troop::Macemen, 1)], owner: 1, human: true },
        );
        let man = runner.fighters.iter().position(|f| f.side == SIDE_A).unwrap();
        let (x0, y0) = (runner.fighters[man].x, runner.fighters[man].y);
        runner.order_side(SIDE_A, x0 + 5, y0);
        let cam = Camera::clamped(x0 as i32 - 4, y0 as i32 - 6);

        let mut xs = vec![figure_origin(&runner.fighters[man], cam).0];
        for _ in 0..(5 * 18 + 40) {
            runner.step();
            let (x, y) = figure_origin(&runner.fighters[man], cam);
            assert_eq!(y, ORIGIN_Y + (y0 as i32 - cam.y as i32) * TILE, "he left his row");
            xs.push(x);
        }
        for (t, w) in xs.windows(2).enumerate() {
            assert!(w[1] >= w[0], "tick {t}: drawn {} pixels west — {xs:?}", w[0] - w[1]);
        }
        assert_eq!(xs.last().unwrap() - xs[0], 5 * TILE, "{xs:?}");
        let steps = xs.windows(2).filter(|w| w[1] > w[0]).count();
        assert!(steps >= 5 * 8, "{steps} forward steps over five cells: {xs:?}");
        assert_eq!((runner.fighters[man].x, runner.fighters[man].y), (x0 + 5, y0));
    }
}

