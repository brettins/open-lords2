use super::*;

/// The labour figure the herd is staffed from — county `+0xD0`, which is
/// labour record 1 of the nine at `+0xC4 + job * 0x0C`.
///
/// **`[V]`, three ways.** `Herd_SeasonTick` passes `+0xD0`; `0xD0 - 0xC4` is
/// exactly one 12-byte record; and the England turn-one fixture's own arithmetic closes —
/// county 1 holds 218 cattle farmers and 217 wood cutters against a population
/// of 435, and county 2 holds 323 and 133 against 456. Both sum to the
/// population exactly.
pub fn herd_labour(t: &Tables, county: &County) -> i32 {
    county.labour[t.job.cattle_farming]
}

/// `herd / fieldsCattle`, head per pasture field — the input to the crowding
/// bands. `FUN_0044D913`'s first three lines.
pub fn herd_density(t: &Tables, herd: i32, fields_cattle: i32) -> i32 {
    if herd < 1 {
        0
    } else if fields_cattle == 0 {
        t.herd.no_pasture_density
    } else {
        herd / fields_cattle
    }
}

/// `FUN_0044D913` — the crowding level stored in county `+0x25C`, one of the
/// four values `L2.eng` group 77 names (`docs/kingdom.md` §13.1).
pub fn herd_crowding(t: &Tables, herd: i32, fields_cattle: i32) -> i32 {
    let last = t.herd.crowding[HERD_CROWDING_COUNT - 1];
    if fields_cattle == 0 {
        return last.level;
    }
    let density = herd_density(t, herd, fields_cattle);
    for row in t.herd.crowding.iter() {
        if density <= row.density_max {
            return row.level;
        }
    }
    last.level
}

/// **The picture on the pasture** — `FUN_0044D913`'s *other* output, which is a
/// terrain value and not a level.
///
/// So the map is a lossy view of the meter, deliberately, and a renderer that
/// derived the tile from `county.herd_crowding` would show three counties out
/// of the England position wrongly. **[V]** — the England turn-one save carries
/// both halves and `crates/l2-kingdom/tests/fields/main.rs` diffs them on all
/// fourteen counties.
pub fn herd_graphic(t: &Tables, herd: i32, fields_cattle: i32) -> u8 {
    if herd < 1 {
        return crate::field::terrain::PASTURE;
    }
    let density = herd_density(t, herd, fields_cattle);
    if density <= t.herd.crowding[0].density_max {
        crate::field::terrain::PASTURE_LOW
    } else if density <= t.herd.crowding[1].density_max {
        crate::field::terrain::PASTURE_CROWDED
    } else {
        crate::field::terrain::PASTURE_PACKED
    }
}

fn crowding_band(t: &Tables, crowding: i32) -> HerdCrowdingRow {
    let last = t.herd.crowding[HERD_CROWDING_COUNT - 1];
    for row in t.herd.crowding.iter().take(HERD_CROWDING_COUNT - 1) {
        if crowding == row.level {
            return *row;
        }
    }
    last
}

/// **`FUN_0044DA99` — the rule that a herd has to be tended.**
pub fn herd_growth(
    t: &Tables,
    herd: i32,
    fields_cattle: i32,
    labour: i32,
    crowding: i32,
    season: u8,
) -> HerdGrowth {
    if herd == 0 {
        return HerdGrowth::default();
    }
    if fields_cattle == 0 {
        let deaths = if herd < t.herd.no_pasture_kill_all_below {
            herd
        } else {
            herd / t.herd.no_pasture_divisor
        };
        return HerdGrowth { births: 0, deaths };
    }

    let mut staffing = pct_of(labour, herd.saturating_mul(t.herd.labour_per_head));
    if staffing > t.herd.staffing_max - 1 {
        staffing = t.herd.staffing_max;
    }
    let band = crowding_band(t, crowding);

    let understaffed =
        if staffing < 100 { -((staffing - 100) / t.herd.understaffing_divisor) } else { 0 };
    let death_rate = band.death_rate + understaffed;

    let mut birth_rate = pct(band.birth_rate, staffing);
    if staffing >= 100 {
        for &(below, bonus) in t.herd.small_bonus.iter() {
            if herd < below {
                birth_rate += bonus;
                break;
            }
        }
    }

    let (num, den) = t.herd.season_bonus;
    let mut deaths = per_myriad(herd.saturating_mul(100), death_rate);
    if season == t.herd.culling_season {
        deaths = deaths * num / den;
    }
    let mut births = per_myriad(herd, birth_rate);
    if season == t.herd.calving_season {
        births = births * num / den;
    }

    if births == 0 {
        if birth_rate == 0 {
            if deaths == 0 && death_rate != 0 {
                deaths = 1;
            }
        } else {
            births = 1;
        }
    }
    if herd < deaths {
        deaths = herd;
    }
    HerdGrowth { births, deaths }
}

/// `FUN_0044DD4D`'s second call — next season's *"Calf births expected"*,
/// *"Cow deaths expected"* and *"Change due to farming"* (`L2.eng` group 77).
///
/// The herd it forecasts from is `herd - herdEaten`, and `change` is
/// `net - herdEaten` again — **one slaughter, counted once in each, because
/// `herdEaten` here is next season's.** `Ration_ApplyAll`'s shadow `+0x190` is
/// what `Herd_SeasonTick` already spent; `Pass::RationPreview` then prices the
/// coming season into `+0x17C` before `County_RefreshEstimates` calls this.
///
/// Not a double subtraction — the old comment here said it was, from the
/// retracted rule where the ration pass debited the store (C149).
///
/// `[V]` against `england-turn1.sav`: nine unowned counties, herd 67 and
/// `herdEaten` 13, reproduce the stored `+0x250/+0x254/+0x258` of 22, 0 and 9.
pub fn herd_preview(t: &Tables, county: &mut County, season_next: u8) {
    county.herd_change_expected = 0;
    if county.pop_band == 0 {
        return;
    }
    let head = county.herd - county.herd_eaten;
    let g = herd_growth(
        t,
        head,
        county.fields_cattle,
        herd_labour(t, county),
        county.herd_crowding,
        season_next,
    );
    county.herd_births_expected = g.births;
    county.herd_deaths_expected = g.deaths;
    county.herd_change_expected = g.net() - county.herd_eaten;
}

/// `Herd_SeasonTick` (`0x0044D60D`) — births and deaths from
/// [`herd_growth`], then the weather's percentage swing and the random-event
/// modifier, then a fresh [`herd_crowding`] and next season's forecast.
///
/// The event modifier is tested for the sentinel **before** it is tested for
/// its sign: `if (mod == 99) { change = 0; births = 0; }`. So 99 is not
/// "+99%", it is *"Cattle will not reproduce this season"* — the *"No bull"*
/// event, `L2.eng` group 314. See [`crate::event::HERD_NO_GROWTH`].
pub fn herd_season_tick(t: &Tables, county: &mut County, season: u8, season_next: u8) {
    // `herd = herd - +0x190`, the shadow `Ration_ApplyAll` (`0x0044BF04`) left.
    county.herd -= county.herd_eaten_shadow;
    let growth = herd_growth(
        t,
        county.herd,
        county.fields_cattle,
        herd_labour(t, county),
        county.herd_crowding,
        season,
    );
    let mut births = growth.births;
    let mut deaths = growth.deaths;

    let mut weather_change = pct(county.herd, t.weather[county.weather.index() as usize].herd_pct);
    county.herd_event_change = 0;
    if county.event_herd_pct == crate::event::HERD_NO_GROWTH {
        weather_change = 0;
        births = 0;
    } else if county.event_herd_pct < 0 {
        county.herd_event_change = pct(county.herd, -county.event_herd_pct);
        deaths += county.herd_event_change;
    } else if county.event_herd_pct > 0 {
        county.herd_event_change = pct(county.herd, county.event_herd_pct);
        births += county.herd_event_change;
    }
    county.event_herd_pct = 0;
    county.herd_weather_change = weather_change;

    if weather_change < 0 {
        deaths -= weather_change;
    } else {
        births += weather_change;
    }

    county.herd += births - deaths;
    county.herd = county.herd.max(0);
    county.herd_crowding = herd_crowding(t, county.herd, county.fields_cattle);
    herd_preview(t, county, season_next);
}


/// `Herd_LabourEstimate` (`0x0044DD4D`) — the **cattle** floor *and* ceiling.
///
/// arrangement of 150 people that would have tended 80 cows. **The game's one
/// way of saying so is this floor**: `Panel_JobDetail` colours the worker count
/// red when `labour < labour_wanted` and `Village_RebuildIcons` draws the
/// shortfall as extra unselectable icons, and [`County::minimap_bands`]'s
/// labour band reads it too. Nothing wrote it, so the count was drawn in black
/// and the herd died without comment. `docs/decisions.md` C187.
///
/// **`[V]` on the whole of it, including the fallback arm**, against the stored
/// `+0xD4` of every county of every original save this machine can open — see
/// `crates/l2-kingdom/tests/cattle.rs`. The fallback is not a cosmetic corner:
///
/// **`[I]`, and the one thing the saves cannot settle:** what the floor holds
/// when the loop never runs at all. The ceiling's 999,999 is visible in the
/// data; no save has a county with no people, so the floor's initial value is
/// unobserved. [`crate::county::LABOUR_NO_FLOOR`] is used, because that is what
/// every *other* job's estimate writes for "no requirement" and because the
/// alternative would paint an empty county's zero milkmaids red.
///
/// **`[D]`.** Three things about the ceiling are worth keeping.
///
/// That much is a `[V]` bound, asserted over every herd size 1 … 400 in all
/// four seasons by `the_dairy_ceiling_is_the_fewest_milkmaids_that_reach_the_best_herd`.
pub fn herd_labour_estimate(t: &Tables, county: &County, season: u8) -> HerdEstimate {
    let herd = county.herd - county.herd_eaten;
    let mut best = -1_000_000;
    let mut ceiling = crate::county::LABOUR_UNSET;
    let mut floor: Option<i32> = None;
    for workers in 0..county.population {
        let g = herd_growth(t, herd, county.fields_cattle, workers, county.herd_crowding, season);
        let net = g.births - g.deaths;
        if net >= 0 && floor.is_none() {
            floor = Some(workers);
        }
        if best < net {
            ceiling = workers;
            best = net;
        }
    }
    // Break-even if the herd can reach it, otherwise the least-bad staffing —
    // which is the argmax, and therefore the ceiling. The loop never having run
    // at all is the `[I]` arm above.
    let wanted = match floor {
        Some(w) => w,
        None if ceiling == crate::county::LABOUR_UNSET => crate::county::LABOUR_NO_FLOOR,
        None => ceiling,
    };
    HerdEstimate { wanted, useful: ceiling }
}

