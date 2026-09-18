#![allow(unused_imports)]
use super::*;

use crate::county::County;
use crate::math::clamp;
use crate::tables::{Season, Tables, Weather};
use l2_net::Pcg32;

/// **`County_Reset` (`0x00451150`), county `+0x21E`.** A climate band 0…4, cut
/// straight out of the county's index at new-game:
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
pub fn local_modifier(county_id: usize, season: Season) -> i32 {
    let band = climate_band(county_id);
    match season {
        Season::Summer => match band {
            0 => 4,
            1 => 2,
            2 => -8,
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

pub fn seasonal_delta(t: &Tables, season: Season, rng: &mut Pcg32) -> i32 {
    let jitter = (rng.below(WEATHER_JITTER_BOUND) >> WEATHER_JITTER_SHIFT) as i32;
    t.season[season.index() as usize].dryness - jitter
}

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

/// The band loop's tail is three calls the rule layer used to have nowhere to
/// put: `FUN_0046942C` clears last season's ruined fields back to waste, then a
/// county reading *Drought* has one field parched (`FUN_00469A9C(county, 0x18)`)
/// and one reading *Flooding* has one flooded (`0x17`), each behind a
/// `Msg_Enqueue` to the owner. See [`crate::field::blight_one_field`].
///
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

    if !advanced_farming {
        for id in 1..=county_count {
            counties[id].weather = Weather::Cloudy;
        }
    }
}

fn push(county: &mut County, by: i32) {
    county.dryness = clamp(county.dryness + by, DRYNESS_MIN, DRYNESS_MAX);
}

