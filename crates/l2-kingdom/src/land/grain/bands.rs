#![allow(unused_imports)]
use super::*;
use super::factors::*;
use super::steps::*;
use super::actions::*;
use super::display::*;
use super::*;

/// **`FUN_0044CF6F` — the crop's density band**, and the only producer of a
/// non-zero `Terrain_Set` variant in the game.
///
/// Four bands at 41 and 81 sacks a field, and the value **is** the terrain byte
/// `Grain_SeasonTick` then writes onto every grain tile of the county. `[D]`
pub fn grain_crop_band(crop: i32, fields: i32) -> u8 {
    if crop < 1 || fields < 1 {
        return 2;
    }
    match crop / fields {
        d if d < 0x29 => 3,
        d if d < 0x51 => 7,
        _ => 11,
    }
}

/// **Which band a county's wheat is drawn at this season** — the three
/// `FUN_0044CF6F` calls in `Grain_SeasonTick` (`0x0044C8AE`), one per arm.
///
/// ```c
/// if (g_season == 1) { … sow …;    band = FUN_0044CF6F(crop[1], (byte)+0x206); }
/// else if (2 or 3)   { … grow …;   band = FUN_0044CF6F(crop[1], (byte)+0x206); }
/// else if (4)        { … harvest …; band = FUN_0044CF6F(crop[2], (byte)+0x206); }
/// ```
///
/// **This is the second time the wheat was fixed, and the first fix read
/// neither argument.** C124 transcribed the call as
/// `FUN_0044CF6F(county.crop[2], county.fieldsGrain)` — one line, stated for all
/// four seasons — and it is the Winter arm's first argument with a divisor
/// none of the three arms use. `crop[2]` is cleared at the top of every
/// season and filled only by the harvest, so in Spring, Summer and Autumn it
/// is always `0`, `FUN_0044CF6F` returns `2` for a zero crop, and every grain
/// field on the map was drawn at variant 0 for three seasons in four. A player,
/// on the build that carried that fix: *"wheat fields still not showing the
/// different stages of wheat growth."*
///
/// The divisor is `+0x206`, [`County::fields_grain_standing`]: the fields
/// sown this year less those since destroyed. `docs/decisions.md`
/// C195. `[D]`
pub fn grain_stage_band(county: &County, season: Season) -> u8 {
    let crop = match season {
        Season::Spring | Season::Summer | Season::Autumn => county.crop[1],
        Season::Winter => county.crop[2],
    };
    grain_crop_band(crop, county.fields_grain_standing & 0xFF)
}

