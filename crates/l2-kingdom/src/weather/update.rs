#![allow(unused_imports)]
use super::*;

use crate::county::County;
use crate::math::clamp;
use crate::tables::{Season, Tables, Weather};
use l2_net::Pcg32;

/// **`County_Reset` (`0x00451150`), county `+0x21E`.** A climate band 0…4, cut
/// straight out of the county's index at new-game:
///
/// ```c
/// if      (id < 4)  band = 0;   /* counties 1..3   */
/// else if (id < 6)  band = 1;   /* counties 4..5   */
/// else if (id < 10) band = 2;   /* counties 6..9   */
/// else if (id < 12) band = 3;   /* counties 10..11 */
/// else              band = 4;   /* counties 12 up  */
/// ```
///
/// **Nothing else in the binary writes `+0x21E`** — one writer, one reader
/// ([`local_modifier`]) — so it is derived here. A saved
/// game holds the byte
/// the two cannot disagree
/// (`docs/agents.md`, *a field is only tested if something a test reads was
/// written by something the game runs*). `[V]`
pub fn climate_band(county_id: usize) -> u8 {
    match county_id {
        0..=3 => 0,
        4..=5 => 1,
        6..=9 => 2,
        10..=11 => 3,
        _ => 4,
    }
}

/// **`FUN_00449D6E`** — the local swing `Weather_UpdateAll` adds to the chosen
/// county and to each of its neighbours, on top of the regional push.
///
/// ```c
/// if (season == Summer) {
///     if (band == 0) return   4;
///     if (band == 1) return   2;
///     if (band == 2) return  -8;
///     if (band == 4) return -12;
///     if (band == 4) return -24;     /* unreachable: the test above it */
/// } else if (season == Winter) {
///     if (band == 1) return  -2;
///     if (band == 2) return  -4;
///     if (band == 3) return  -6;
///     if (band == 4) return -10;
/// }
/// return 0;
/// ```
///
/// **Summer's ladder has a hole and a dead arm, and they are the same slip.**
/// The five bands plainly want `+4, +2, −8, −12, −24`; the fourth test reads
/// `band == 4` where it should read `band == 3`, so **band 3 falls through to
/// zero** and band 4 takes band 3's −12 while the −24 arm can never run.
/// Reproduced literally — `docs/bugs.md` B92. Winter's ladder is
/// complete: band 0 has no arm because its value is the fall-through 0.
///
/// Spring and Autumn get nothing at all
/// the same everywhere on the map.
pub fn local_modifier(county_id: usize, season: Season) -> i32 {
    let band = climate_band(county_id);
    match season {
        Season::Summer => match band {
            0 => 4,
            1 => 2,
            2 => -8,
            // `band == 3` is the slip: the original tests 4 twice, so 3 gets
            // nothing and 4 gets the −12 that was written for 3.
            4 => -12,
            _ => 0,
        },
        Season::Winter => match band {
            1 => -2,
            2 => -4,
            3 => -6,
            4 => -10,
            _ => 0,
        },
        _ => 0,
    }
}

/// The season's push on every county's dryness, jitter included.
///
/// The jitter is `draw >> 3` of a 0..=127 draw, which is 0..=15 — enough to
/// cancel Spring and Autumn outright and to take two thirds off Summer.
pub fn seasonal_delta(t: &Tables, season: Season, rng: &mut Pcg32) -> i32 {
    let jitter = (rng.below(WEATHER_JITTER_BOUND) >> WEATHER_JITTER_SHIFT) as i32;
    t.season[season.index() as usize].dryness - jitter
}

/// Which county gets the local swing.
///
/// ```c
/// c = g_seasonRandomA & 0xF;              /* 0 .. 15 */
/// if (c > g_countyCount) c = g_weatherCounty + 1;   /* last season's, plus one */
/// if (c > g_countyCount) c = 1;
/// if (c < 1)             c = 1;
/// g_weatherCounty = c;
/// ```
///
/// **`[V]`.** The mask is a
/// flat 0..=15 whatever the map's county count is, so on a 14-county map the
/// two out-of-range values fall through to *"the county after last season's"* —
/// which makes the local swing walk steadily around the map about an eighth of
/// the time. `previous` is `g_weatherCounty`
/// (`0x00554020`), which is its own four-byte save block.
pub fn chosen_county(draw: u32, county_count: usize, previous: usize) -> usize {
    let mut c = (draw & WEATHER_COUNTY_MASK) as usize;
    if c > county_count {
        c = previous + 1;
    }
    if c > county_count {
        c = 1;
    }
    if c < 1 {
        c = 1;
    }
    c
}

/// Band a dryness reading, applying the two mean-reversion clamps in place.
pub fn band(dryness: &mut i32) -> Weather {
    if *dryness < DRYNESS_LADDER[0] {
        *dryness = DRYNESS_AFTER_FLOOD;
        Weather::Flooding
    } else if *dryness < DRYNESS_LADDER[1] {
        Weather::Storms
    } else if *dryness < DRYNESS_LADDER[2] {
        Weather::Cloudy
    } else if *dryness < DRYNESS_LADDER[3] {
        Weather::Sunny
    } else {
        *dryness = DRYNESS_AFTER_DROUGHT;
        Weather::Drought
    }
}

/// The cold-season rewrite. `dryness` is the reading the band came from,
/// **before** the drought clamp would have moved it — which does not matter,
/// because the only band the threshold applies to is Sunny.
/// touches Drought.
pub fn apply_frost(band: Weather, dryness: i32, season: Season) -> Weather {
    if season != Season::Winter && season != Season::Spring {
        return band;
    }
    match band {
        Weather::Drought => Weather::Frost,
        Weather::Sunny if dryness > FROST_SUNNY_THRESHOLD => Weather::Frost,
        other => other,
    }
}

/// `Weather_UpdateAll` — one regional push, one local swing, then band
/// everybody.
///
/// The randomly chosen county is drawn once from the shared generator, so the
/// number of draws this pass makes is `1 + 1` regardless of the kingdom's size
/// (`docs/netcode.md` §3: the stream must not depend on how much work there was
/// to do).
///
/// # The two blighted fields
///
/// The band loop's tail is three calls the rule layer used to have nowhere to
/// put: `FUN_0046942C` clears last season's ruined fields back to waste, then a
/// county reading *Drought* has one field parched (`FUN_00469A9C(county, 0x18)`)
/// and one reading *Flooding* has one flooded (`0x17`), each behind a
/// `Msg_Enqueue` to the owner. See [`crate::field::blight_one_field`].
///
/// **The *Advanced Farming* override runs after them, in a loop of its own.**
/// So with the option off the fields are still ruined and the letters still
/// sent, and only the weather *byte* is flattened to Cloudy. `[V]` — two
/// separate loops in the decompilation.
#[allow(clippy::too_many_arguments)]
pub fn update_all(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    map: &mut crate::map::CampaignMap,
    season: Season,
    advanced_farming: bool,
    rng: &mut Pcg32,
    previous_county: &mut usize,
    owner_is_human: &impl Fn(u8) -> bool,
    messages: &mut Vec<crate::report::Message>,
) {
    if county_count == 0 {
        return;
    }

    let delta = seasonal_delta(t, season, rng);
    // The original draws from its *other* LFSR here; one draw either way, so
    // the stream's length is unchanged.
    let chosen = chosen_county(rng.below(WEATHER_JITTER_BOUND), county_count, *previous_county);
    *previous_county = chosen;

    for id in 1..=county_count {
        push(&mut counties[id], delta);
    }

    let local = delta + local_modifier(chosen, season);
    push(&mut counties[chosen], local);

    let neighbours: Vec<u8> = counties[chosen].neighbours().to_vec();
    for n in neighbours {
        let n = n as usize;
        if n == 0 || n > county_count {
            continue;
        }
        // `delta / 2` truncates toward zero, which in Winter — the one season
        // with a negative push — means a neighbour is wetted by one less than
        // half. That is C's division and therefore the original's.
        let swing = delta / 2 + local_modifier(n, season);
        push(&mut counties[n], swing);
    }

    for id in 1..=county_count {
        let mut dryness = counties[id].dryness;
        let b = band(&mut dryness);
        counties[id].dryness = clamp(dryness, DRYNESS_MIN, DRYNESS_MAX);
        let b = apply_frost(b, counties[id].dryness, season);
        counties[id].weather = b;
        // `FUN_0046942C` first: last season's blight goes back to waste.
        crate::field::clear_blight(&counties[id], map);
        // Frost has already eaten Drought in Winter and Spring, so a field can
        // only be parched in the two warm seasons.
        if b == Weather::Drought {
            if owner_is_human(counties[id].owner) {
                messages.push(crate::report::Message::Drought { county: id as u8 });
            }
            crate::field::blight_one_field(&mut counties[id], map, crate::field::terrain::PARCHED);
        }
        if b == Weather::Flooding {
            if owner_is_human(counties[id].owner) {
                messages.push(crate::report::Message::Flooding { county: id as u8 });
            }
            crate::field::blight_one_field(&mut counties[id], map, crate::field::terrain::FLOODED);
        }
    }

    // "and every county's weather byte is 3 (Cloudy)" - docs/kingdom.md §9
    // point 3. A second loop, after the blight, because that is where the
    // original's `if (g_optAdvancedFarming == 0)` sits.
    if !advanced_farming {
        for id in 1..=county_count {
            counties[id].weather = Weather::Cloudy;
        }
    }
}

fn push(county: &mut County, by: i32) {
    county.dryness = clamp(county.dryness + by, DRYNESS_MIN, DRYNESS_MAX);
}

