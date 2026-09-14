#![allow(unused_imports)]
use super::*;
use super::reset::*;
use super::placement::*;
use super::scenario::*;
use l2_formats::maps::{MapSlot, Plane, PLANE_DIM};
use l2_kingdom::county::{MAX_COUNTIES, MAX_COUNTY_ID, MAX_FIELDS, MAX_NEIGHBOURS};
use l2_kingdom::map::{CampaignMap, MAP_TILES};
use l2_kingdom::merchant::{self, MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::MercenaryBands;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{Weather, JOB_COUNT};
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::Options;
use crate::{Clock, CountyState, IndustryState, RealmState, Scenario};

/// `Merchant_PickStartCounties` (`0x004291B3`) — one start county per trade
/// route, each different from the ones already handed out.
///
/// **The dedup walk is odd and it is reproduced exactly**, because a merchant
/// that starts in the wrong county walks the wrong route for the rest of the
/// game. Candidate 0 is the route's first town; after that it tries cells
/// 2, 4, 6 … and, on running off the end of the row, cells 3, 5, 7 …; **cell 1
/// is never tried**; and after five retries it stores whatever it has, taken or
/// not. `Merchant_StartCountyTaken` returns 1 for county 0 as well, because
/// unset entries are 0 —
/// `Merchant_SpawnAll` stops dead at the first zero.
fn pick_merchant_starts(w: &mut MapWorld) {
    let taken = |starts: &[u8; ROUTES], c: u8| starts.iter().any(|&s| s == c);
    let mut starts = [0u8; ROUTES];
    for row in 0..ROUTES {
        let cells = *w.routes.row(row);
        let mut cursor = 0usize;
        let mut candidate = cells[0];
        let mut tries = 0;
        while taken(&starts, candidate) {
            tries += 1;
            if tries >= 6 {
                break;
            }
            candidate = cells.get(cursor + 2).copied().unwrap_or(0);
            cursor += 2;
            if candidate == 0 {
                cursor = 1;
            }
        }
        starts[row] = candidate;
    }
    w.merchant_start = starts;
}

// ---------------------------------------------------------------- the seating

/// `FUN_00497E65` (`0x00497E65`) — **the start table is shuffled, and that is
/// why which realm you play is different every game.**
///
/// `docs/environment.md` records the fact from the other end: the England
/// turn-one fixture's fingerprint deliberately excludes the realm→county
/// assignment, because two independently created saves of the same map
/// disagree about it. This is the code that disagrees. `Game_NewGame` calls it
/// between `Mercenary_Init` and `PlayerStart_Compact`, and it re-deals the
/// live entries of `g_playerStartTable` into each other's slots:
///
/// ```c
/// for (i = 1; i <= live; i++) {
///     pos = (rand & 3) + 1 + i;  if (pos > live) pos = 1;
///     for (tries = 0; tries < live; tries++) {
///         if (table[pos] == 0) { table[pos] = src[i]; break; }
///         if (++pos > live) pos = 1;
///     }
/// }
/// ```
///
/// **`[D]`, and the function had no name until now.** Its second half deals a
/// 48-entry table into `DAT_0057CAE0` and fills `DAT_00553080` with a value per
/// entry (`0`, `100`, or `6 + 5n`); neither destination has been traced and
/// neither is a player start, so neither is reproduced.
///
/// **The generator is ours and the shape is the original's**, for the reason
/// `l2_game::scenario::SEED` gives: the original draws from two 31-bit LFSRs
/// whose state no save records, and reproducing its *stream* is impossible from
/// a file. What is reproduced is the deal — a random offset, then a linear
/// probe forward for a free slot, wrapping at the live count.
fn shuffle_starts(w: &MapWorld, seed: u64) -> Vec<u8> {
    // `local_ec`: how many of entries 1..5 the map filled.
    let src: Vec<u8> = (1..w.player_start.len()).map(|m| w.player_start[m]).collect();
    let live = src.iter().filter(|&&c| c != 0).count();
    if live == 0 {
        return Vec::new();
    }
    let mut rng = l2_kingdom::Pcg32::from_seed(seed);
    let mut table = vec![0u8; live + 1];
    for i in 1..=live {
        let mut pos = (rng.next_u32() & 3) as usize + 1 + i;
        if pos > live {
            pos = 1;
        }
        for _ in 0..live {
            if table[pos] == 0 {
                table[pos] = src[i - 1];
                break;
            }
            pos += 1;
            if pos > live {
                pos = 1;
            }
        }
    }
    // The probe can leave an entry unplaced only if every slot was full, which
// needs more sources than slots; there are
    debug_assert!(table[1..].iter().all(|&c| c != 0), "the deal placed every start");
    table[1..].to_vec()
}

/// `PlayerStart_Compact` (`0x0049BC5F`) — the start counties the live realms
/// get, realm 1 first.
///
/// The original bubbles entries whose *slot number* is above the live realm
/// count out of a six-entry table, repeatedly, until none is left. Every
/// populated entry's slot number is its own index (`PlayerStart_Record` writes
/// both),
/// result is the first `lords` entries of the table as
/// [`shuffle_starts`] left it. Written that way, with
/// the equivalence stated here and checked over all 44 shipped maps in
/// `tests/newgame.rs`.
fn start_counties(w: &MapWorld, lords: usize, seed: u64) -> Result<Vec<u8>, MapError> {
    let seats = shuffle_starts(w, seed);
    if seats.is_empty() {
        return Err(MapError::NoPlayerStarts);
    }
    if lords > seats.len() {
        return Err(MapError::TooManyLords { lords, seats: seats.len() });
    }
    for (i, &c) in seats.iter().take(lords).enumerate() {
        if c as usize > w.county_count {
            return Err(MapError::StartCounty { marker: i + 1, county: c });
        }
    }
    Ok(seats)
}

/// What [`assign_lords`] decides for each realm: its shield, then its lord.
///
/// One struct because **the second is a function of the
/// first** and a caller that could take one without the other would be able to
/// build a realm whose colour and lord disagree — which is exactly the state
/// `shield = realm` used to produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Assignment {
    /// Realm `+0x0A`, `shieldIndex`, 1 … 5. **Zero means the walk gave this
    /// realm nothing** — it is above the lord count — and the caller falls back
/// to `FUN_0049C995`'s seed.
    shield: [u8; MAX_REALMS],
    /// Realm `+0x28`, the lord id. Zero for a human and for a realm out of
    /// play, which is what the original writes at the top of every iteration.
    lord: [u8; MAX_REALMS],
}

/// `Realms_AssignLords` (`0x0049CAAA`) — **the shield first, by position, and
/// then the lord from the shield.**
///
/// # The arrow runs colour → lord, and no lord is ever consulted
///
/// 1. Mark every **human's** chosen shield taken. The original reads
///    `g_playerSlots + realm * 0x2C + 0x25` — the six-slot record
///    `g_playerNames` is the `+0x04` of, so four bytes lower than the name —
///    guarded by `+0x26 == 0` (a person), and
///    page 4's `FUN_00432FAB` is what wrote it.
/// 2. Walk realms **1 … 5 in realm order**, skipping humans and stopping when
///    `g_aiLordCount` lords have been handed out. Each AI takes **the lowest
///    shield nobody has taken**; a realm past the lord count gets no shield and
///    `strength = 0`.
/// 3. Then `lord = g_lordChoice[(g_scenarioIndex & 3) * 0x14 + shield * 4 + n]`
///    for `n` = 0 … 3, the first candidate no earlier realm has taken.
///
/// **This line used to read `let shield = realm`, under a doc comment that
/// stated that as the *mechanism*** — *"the colour slot is the realm id,
/// because `Game_SetupRealms` seeds `shieldIndex = i` and only a custom game's
/// colour picker permutes it."* The seed is real (`FUN_0049C995`) and the
/// conclusion drawn from it was not: the seed is what an *untouched* page 4
/// leaves, and `Realms_AssignLords` overwrites it for every AI on every run.
/// A sentence that explains a default as a rule is a sentence nobody re-reads,
/// so the line outlived four documents describing the real walk.
/// `docs/decisions.md` C130.
///
/// # The consequence a player will check
///
/// Take **yellow** and the Knight does not fall back to red: he becomes the
/// **black** lord and the **Baron** becomes the red one, because red's
/// candidate list names the Baron first and the walk reaches red before black.
/// `docs/rules.md` §7a has all five rows and
/// `crates/l2-game/tests/newgame.rs` drives the setup screen to each of them.
///
/// **`[D]` on the group.** The original picks the deterministic group 0 when
/// `DAT_0055302C == 1` and `(g_scenarioIndex & 3)` otherwise, and what
/// `DAT_0055302C` is has not been read. The scenario-varying path is taken here
/// because it is the one a single-player custom game reaches unless that flag
/// is set, and because it is the only reading under which the four groups exist
/// for a reason.
fn assign_lords(setup: &NewGame, lords: usize) -> Assignment {
    let group = setup.slot & 3;
    let human = setup.local_player as usize;
    // `g_aiLordCount` — `Setup_CommitOptions` keeps *Nobles* minus the people,
    // and this build has one person.
    let ai_lords = lords.saturating_sub(1);

    let mut a = Assignment { shield: [0; MAX_REALMS], lord: [0; MAX_REALMS] };
    // `acStack_14[6]` and `acStack_20[8]`, both zeroed at entry.
    let mut shield_taken = [false; 6];
    let mut lord_taken = [false; 8];

    // Step 1. The human's shield is taken before anybody walks.
    if human >= 1 && human < MAX_REALMS {
        a.shield[human] = setup.shield;
        shield_taken[setup.shield as usize] = true;
    }

    // Step 2. Realms 1 … 5 in realm order.
    let mut given = 0usize;
    for realm in 1..MAX_REALMS {
        if realm == human || given >= ai_lords {
            // A human keeps the shield he chose and takes no lord; a realm past
            // the lord count is the `strength = 0` limb and takes neither.
            continue;
        }
        given += 1;
        let Some(shield) = (1..=5u8).find(|s| !shield_taken[*s as usize]) else { continue };
        shield_taken[shield as usize] = true;
        a.shield[realm] = shield;
        // Step 3. And now the lord, out of that shield's four candidates.
        for n in 0..4usize {
            let at = group * 0x14 + shield as usize * 4 + n;
            let candidate = LORD_CHOICE.get(at).copied().unwrap_or(0);
            if candidate == 0 {
                continue;
            }
            if !lord_taken[candidate as usize & 7] {
                a.lord[realm] = candidate;
                lord_taken[candidate as usize & 7] = true;
                break;
            }
        }
    }
    a
}

// ------------------------------------------------------------- County_Reset

/// `County_Reset` (`0x00451150`) — the opening economy of **every** county,
/// owned or not.
///
/// It runs after `Map_InitScenario` and before the seating, and the option rows
/// then overwrite five of its numbers on every county
/// (`Game_SetupRealmsAndCounties`' first loop). What survives is the ration, the
/// split, the weather, the dryness, the labour shares and the industry share —
/// and `tax_rate`, which **nothing sets at all**: the county record was zeroed
/// wholesale by `FUN_0046EA28` in `Game_NewGame`'s preamble and no new-game
/// path writes a tax rate,
/// `[D]`, and it is the answer to a question `docs/kingdom.md` does not ask.
fn county_reset(id: usize) -> CountyState {
    CountyState {
        owner: 0,
        population: reset::POPULATION,
        population_last: reset::POPULATION,
        happiness: reset::HAPPINESS,
        happiness_last: reset::HAPPINESS,
        shown_tax: 0,
        shown_ration: 0,
        shown_health: 0,
        shown_events: 0,
        d_hap_ration: 0,
        // **Zero here for the same reason `tax_rate` is**, and stated rather
        // than assumed: `FUN_0046EA28` zeroes the whole county record in
        // `Game_NewGame`'s preamble, and no new-game path read here writes any
        // of the three. The season the new game immediately runs is what fills
        // them — `Panels_RefreshAll`'s `Tax_RecomputePreview` turns `+0x0F`
        // into 5 at rate 0, which is what every shipped save holds. `[D]`.
        d_hap_health: 0,
        d_hap_tax_local: 0,
        tax_shown: 0,
        health_meter: reset::HEALTH_METER,
        health_band: reset::HEALTH_BAND,
        unrest: 0,
        births: 0,
        deaths: 0,
        emigrants: 0,
        immigrants: 0,
        // `popBand = (population - 1) / 25 + 1`, which the original computes
// here.
        pop_band: (reset::POPULATION - 1) / 25 + 1,
        anchor: (0, 0),
        neighbours: Vec::new(),
        tax_rate: 0,
        tax_collected: 0,
        ration_wanted: reset::RATION_WANTED,
        ration_achieved: 0,
        ration_split: reset::RATION_SPLIT,
        grain_eaten: 0,
        herd_eaten: 0,
        castle_type: 0,
        castle_building: 0,
        castle_switch: false,
        industry: industry_reset(),
        fields_fallow: 0,
        fields_cattle: 0,
        fields_grain: 0,
        fertility: 0,
        weather: Weather::from_index(reset::WEATHER).expect("3 is Cloudy"),
        dryness: reset::DRYNESS,
        grain: reset::GRAIN,
        herd: reset::HERD,
        labour: [0; JOB_COUNT],
        labour_wanted: [0; JOB_COUNT],
        labour_useful: [0; JOB_COUNT],
        labour_share: {
            let mut s = [0i32; JOB_COUNT - 1];
            let n = s.len().min(reset::LABOUR_SHARE.len());
            s[..n].copy_from_slice(&reset::LABOUR_SHARE[..n]);
            s
        },
        industry_share: reset::INDUSTRY_SHARE,
        field_tiles: [0; MAX_FIELDS],
        // `field_0x1FE = county & 1`. **The save path did not import this
        // until now** and every loaded county farmed as style 0; see
        // `Scenario::from_save`.
        farm_style: (id & 1) as u8,
        // `County_Reset` zeroes the whole record, and no new-game path writes
        // any of these four: a fresh county has no money of its own and no
        // stall until `County_RecountMerchants` runs at the end of turn one.
        purse: 0,
        merchant_count: 0,
        merchant_unit: 0,
        merchant_visits: 0,
        // **C161.** Every one of these is `County::new()`'s value,
        // which is what a new game got before the save path learned to carry
        // them — so this constructor's world is unchanged. Zero is also what
        // `County_Reset`'s zeroing of the record gives, except the industry
        // ramp, which `industry` above carries at `Industry::new`'s base.
        herd_change_expected: 0,
        herd_births_expected: 0,
        herd_deaths_expected: 0,
        grain_weather_change: 0,
        grain_event_change: 0,
        herd_weather_change: 0,
        herd_event_change: 0,
        grain_change_expected: 0,
        grain_sown_expected: 0,
        grain_grown_expected: 0,
        reclaim_fields_finishing: 0,
        reclaim_seasons_to_next: 0,
        // `County_Reset` opens both on zero, which `l2_game::scenario` used to
        // have to write back over the save path's invented value.
        happiness_avg: 0,
        happiness_sum: 0,
        d_hap_tax: 0,
        shown_army: 0,
        tax_hap_other: 0,
        shown_ale: 0,
        ale_happiness_given: 0,
        unrest_warned: false,
        pop_change_pct: 0,
        army: 0,
        largest_inflow: 0,
        inflow_sources: [0; l2_kingdom::county::MAX_INFLOW_SOURCES],
        emigrant_destination: 0,
        largest_inflow_source: 0,
        change_reason: 0,
        event_fired: false,
        event_id: 0,
        event_population_pct: 0,
        event_population_swing: 0,
        event_grain_pct: 0,
        event_herd_pct: 0,
        tax_suppressed: false,
        field_progress: [0; MAX_FIELDS],
        friendly_troops: 0,
        enemy_troops: 0,
        levy_surcharge: 0,
        castle_degraded: 0,
        castle_ruined: false,
        castle_level_left: 0,
        castle_percent: 0,
        castle_work_left: 0,
        castle_work_total: 0,
        castle_stone_owed: 0,
        castle_stone_total: 0,
        castle_wood_owed: 0,
        castle_wood_total: 0,
        siege_scars: l2_kingdom::siege::SiegeScars::default(),
        crop: [0; 3],
        // `County_Reset` zeroes both field cursors.
        pasture_cursor: 0,
        blight_cursor: 0,
        fields_grain_sown: 0,
        fields_grain_standing: 0,
        sow_shortfall: false,
        weapon_type: 0,
        // `County_Reset` zeroes the record, and `Mercenary_AdvanceAll` — the
        // cache's only writer besides a hire — is turn phase 7's, which a new
        // game has not reached.
        mercenary_offer: 0,
    }
}

/// The four industry records a fresh county opens with: `Industry::new`'s
/// ramp base and nothing produced. The switches and the resource bytes are
/// filled from the map afterwards.
fn industry_reset() -> [IndustryState; 4] {
    let mut out = [IndustryState::default(); 4];
    for (slot, commodity) in [
        l2_kingdom::tables::Commodity::Wood,
        l2_kingdom::tables::Commodity::Iron,
        l2_kingdom::tables::Commodity::Weapons,
        l2_kingdom::tables::Commodity::Stone,
    ]
    .into_iter()
    .enumerate()
    {
        out[slot].efficiency = l2_kingdom::county::Industry::new(commodity).efficiency;
    }
    out
}

// ---------------------------------------------------------------- the merchants

/// `Merchant_SpawnAll` (`0x00427ED0`) — one type-3 merchant, owner 6, on a free
/// road tile near each start county, **stopping at the first zero**.
///
/// Owner 6 is nobody, which is what a merchant and a county's own levied
/// defence carry. The route cursor lives in the low byte of `yearFormed` and
/// opens at 1, not 0: the merchant's first destination is the *second* town on
/// its row, because it is standing in the first.
fn spawn_merchants(w: &MapWorld, map: &CampaignMap) -> Vec<(usize, Unit)> {
    let mut units = Units::new();
    let mut out = Vec::new();
    for (row, &county) in w.merchant_start.iter().enumerate() {
        if county == 0 {
            break;
        }
        let anchor = w.anchor.get(county as usize).copied().unwrap_or((0, 0));
        let Some((x, y)) = merchant::find_free_road_tile(map, &units, anchor)
            .or_else(|| merchant::find_free_open_tile(map, &units, anchor))
        else {
            continue;
        };
        let mut u = Unit::new(UnitKind::Merchant, 6, x, y);
        u.needs_destination = true;
        u.county = county;
        u.morale = 100;
        u.cargo_county = county;
        u.year_formed = 1;
        u.name_index = row as u8;
        if let Some(slot) = units.spawn(u.clone()) {
            out.push((slot, u));
        }
    }
    out
}

