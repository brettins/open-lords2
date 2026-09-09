//! Our own save format: round-trip, determinism, and the refusals.
//!
//! Needs no game install — the format is ours, and nothing here reads a file.
//!
//! # The obligation that matters, and how it is discharged
//!
//! `l2_kingdom::save` encodes through `l2_net::Canonical`, and
//! `docs/netcode.md` §6's per-tick digest is `Canonical::hash_of(kingdom)` over
//! **the same `Encode` impl**. So a field missing from this encoding is missing
//! from the lockstep checksum too: two peers can diverge on it and every
//! checksum they exchange reports agreement. That is the failure lockstep
//! exists to prevent, and it has now happened three times
//! (`docs/decisions.md` C30, C39).
//!
//! The property is **"every field reaches the bytes"**, and it is *not* the
//! same as "a field round-trips". A field that both `Encode` and `Decode`
//! ignore round-trips perfectly: the encoder drops it, the decoder leaves
//! `County::new`'s default, and if the source value also holds that default,
//! `PartialEq` is satisfied. All four C30 fields would have survived such a
//! test.
//!
//! So the property is split in two, and **neither half is a list of fields
//! somebody remembered**:
//!
//! * **[`furnished`] holds a value in every field that differs from the
//!   default**, and `a_furnished_kingdom_round_trips_field_for_field` compares
//!   the decoded kingdom against it with `#[derive(PartialEq)]` — the only
//!   exhaustive reader of a struct this project has. Over a saturated fixture
//!   that single `assert_eq!` *is* the completeness check: drop a field from
//!   `Encode` and the decoded value comes back holding the default, which the
//!   fixture does not hold.
//! * **[`every_field_of_the_state_is_furnished`] keeps it saturated.** It reads
//!   the source of `crates/l2-kingdom/src`, derives the fields of every struct
//!   reachable from `Kingdom`, and requires each one to be furnished below. A
//!   field added to `County` tomorrow fails this test by name, with nobody
//!   having had to remember anything.
//!
//! The predecessor was `every_part_of_the_state_reaches_the_bytes`, a
//! hand-written enumeration of ~130 mutations. It is deleted. It could only
//! ever check the fields somebody listed, it missed the four C30 fields, it
//! missed six more that were found by matching it against the struct, and it
//! missed the twelve of C39 — and leaving it beside a derived check is how the
//! derived one rots.
//!
//! The rest is unchanged: **determinism** (the same state, the same bytes, and
//! a fixed and known body length so that a change to the layout has to be a
//! deliberate one), **the version changelog** (checked, not merely written),
//! and **refusal** — an unknown version, a wrong ruleset, a flipped byte and a
//! truncated file each produce their own error rather than a kingdom.

use l2_kingdom::county::{ChangeReason, MAX_COUNTIES};
use l2_kingdom::realm::{Pair, MAX_REALMS};
use l2_kingdom::save::{
    checksum, decode, encode, ruleset_fingerprint, LoadError, HEADER_LEN, MAGIC, VERSION,
};
use l2_kingdom::tables::{Tables, Weather, JOB_COUNT};
use l2_kingdom::{Kingdom, Options};

/// **A kingdom with every field of its record holding something other than the
/// default.** That is the whole point of it, and
/// [`every_field_of_the_state_is_furnished`] is what keeps it true: the census
/// reads the struct definitions out of `crates/l2-kingdom/src` and requires
/// every field of every struct reachable from `Kingdom` to be furnished here.
///
/// Saturation is what makes `a_furnished_kingdom_round_trips_field_for_field`
/// a completeness check rather than a smoke test. Over a kingdom of zeros, a
/// field that the encoder drops round-trips perfectly — the decoder hands back
/// the constructor's default and the default is what went in. Over this one it
/// cannot: the default is the one value no field here holds.
///
/// So when adding a field to this fixture, **give it a value the constructor
/// would not** (`County::new` is not all zeros — `industry_share` is 25,
/// `ration_split` 100, `labour_share` a ladder), and prefer a value derived
/// from the record's index so that two records are never accidentally alike.
//
// --- the fixture: begin ---------------------------------------------------
// Everything between these two markers is what the census searches. Keep the
// markers, and keep helpers that furnish state inside them.
fn furnished(seed: u64) -> Kingdom {
    let mut k = Kingdom::new(seed);

    k.season = 2;
    k.season_next = 3;
    k.season_prev = 1;
    k.year = 1300;
    k.year_next = 1301;
    k.turn_count = 42;
    k.weather_county = 9;
    k.turn.phase = l2_kingdom::Phase::Merchants;
    k.turn.step = 5;
    k.rng.next_u32();
    k.options = Options {
        difficulty: 2,
        advanced_farming: true,
        armies_eat: true,
        fight_humans_only_byte: 1,
        exploration: true,
        time_limit: 120,
    };
    assert!(k.set_county_count(14));

    for id in 0..MAX_REALMS {
        let n = id as i32;
        let r = &mut k.realms[id];
        r.in_play = id != 4;
        r.strength = (3 * id + 1) as u8;
        r.is_human = id == 1;
        r.lord = (id + 1) as u8;
        r.shield_index = (id + 2) as u8;
        r.tax_hap_empire = -7 + n as i8;
        r.county_count = (id + 3) as u8;
        r.rank = (id % 5 + 1) as u8;
        r.score = 700 + n;
        r.wages = 80 + n;
        r.gold = 1000 + n * 37;
        r.iron = 40 + n;
        r.stone = 50 + n;
        r.wood = 60 + n;
        r.weapons = [1, 2, 3, 4, 5, 6].map(|w| w * (n + 1));
        r.bankrupt_stage = (id % 5 + 1) as u8;
        r.population_total = 4000 + n;
        r.population_last = 3900 + n;
        r.population_mean = 300 + n;
        r.mean_happiness = 40 + n;
        r.mean_health = 55 + n;
        r.share_of_map_pct = 10 + n;
        r.army_count = (id + 1) as u8;
        r.total_men = 900 + n;
        r.score_inputs = [1, 2, 3, 4, 5, 6].map(|v| v * (n + 1));
        r.ai_step = n + 1;
        r.weapon_rota = n + 1;

        // The diplomacy record. It reached neither the save nor the lockstep
        // digest until the census found it — `l2_kingdom::save::VERSION` 10.
        r.offer_pending = id % 2 == 0;
        r.ally_candidate = (id + 2) as u8;
        r.ally = (id + 3) as u8;
        r.target_county = (id + 4) as u8;
        r.taunt_timer = (id + 5) as u8;
        r.taunt_stage = (id % 2 + 1) as u8;
        r.war_target = (id + 6) as u8;
        r.offer_timer = -(n as i8) - 1;
        r.crowned_once = id % 3 == 0;
        r.voice_rotation = (id % 4 + 1) as u8;
        for other in 0..MAX_REALMS {
            let m = other as i32;
            r.pairs[other] = Pair {
                standing: (n * 7 + m) as i8 - 30,
                allied: (id + other) % 2 == 0,
                grudge: (id * 3 + other + 1) as u8,
                warnings_sent: ((id + other) % 3 + 1) as u8,
                at_war: (id + other) % 3 == 0,
                compliments_from: (other + 1) as u8,
                best_gift: 50 + n * 10 + m,
                has_mail: other % 2 == 1,
                help_price_multiple: (other + 2) as u8,
            };
        }
    }

    // Every slot, not only the fourteen in play: the encoding writes all
    // seventeen county records and all six realm records, so the fixture has to
    // furnish all of them or the unused ones are tested at their defaults.
    for id in 0..MAX_COUNTIES {
        let n = id as i32;
        let c = &mut k.counties[id];
        c.owner = match id {
            1 | 4 | 8 => 1,
            11 | 13 => 2,
            _ => 0,
        };

        c.event_fired = id % 2 == 0;
        c.event_id = id as u16 * 3 + 1;
        c.health_meter = 60 + n;
        c.health_band = l2_kingdom::tables::health_band(c.health_meter) as u8;
        c.happiness = 50 + n;
        c.happiness_last = 48 + n;
        c.d_hap_tax = -3 - n;
        c.d_hap_tax_local = -4 - n;
        c.d_hap_health = 5 + n;
        c.d_hap_ration = 6 + n;
        c.shown_tax = 7 + n;
        c.shown_ration = 8 + n;
        c.shown_health = 9 + n;
        c.shown_army = 10 + n;
        c.tax_hap_other = 11 + n;
        c.shown_events = 12 + n;
        c.happiness_avg = 13 + n;
        c.happiness_sum = 14 + n;
        c.shown_ale = 15 + n;
        c.ale_happiness_given = 16 + n;
        c.unrest = (id % 4 + 1) as u8;
        c.unrest_warned = id % 2 == 0;

        c.population = 400 + n * 11;
        c.pop_last = 390 + n * 11;
        c.pop_change_pct = 17 + n;
        c.births = 18 + n;
        c.deaths = 19 + n;
        c.army = 20 + n;
        c.emigrants = 21 + n;
        c.immigrants = 22 + n;
        c.largest_inflow = 23 + n;
        c.emigrant_destination = (id % 13 + 1) as u8;
        c.largest_inflow_source = (id % 11 + 1) as u8;
        for slot in 0..c.inflow_sources.len() {
            c.inflow_sources[slot] = ((slot + id) % 15 + 1) as u8;
        }
        for neighbour in 1..=14u8 {
            if neighbour as usize != id {
                c.add_neighbour(neighbour);
            }
        }
        c.change_reason = match id % 5 {
            0 => ChangeReason::None,
            1 => ChangeReason::Births,
            2 => ChangeReason::Deaths,
            3 => ChangeReason::Emigration,
            _ => ChangeReason::Immigration,
        };
        c.pop_band = 24 + n;
        c.anchor_x = (id + 1) as u8;
        c.anchor_y = (100 - id) as u8;

        c.tax_rate = id as i32 % 13 + 1;
        c.tax_collected = 25 + n;
        c.tax_shown = 26 + n;
        for job in 0..JOB_COUNT {
            c.labour[job] = 500 + n + job as i32;
            c.labour_wanted[job] = 300 + n + job as i32;
            c.labour_useful[job] = 200 + n + job as i32;
        }
        for job in 0..JOB_COUNT - 1 {
            c.labour_share[job] = 7 + n + job as i32;
        }
        c.industry_share = 61 + n;
        for slot in 0..c.field_progress.len() {
            c.field_progress[slot] = ((slot as u16 * 37) % 801) + 1;
        }

        c.ration_achieved = id as i32 % 4 + 4;
        c.ration_wanted = id as i32 % 3 + 4;
        c.ration_split = id as i32 * 7 % 101;
        c.grain_eaten = 27 + n;
        c.herd_eaten = 28 + n;
        c.grain_available = 29 + n;
        c.herd_available = 30 + n;
        c.friendly_troops = 31 + n;
        c.enemy_troops = 32 + n;

        c.mercenary_offer = (id % 12 + 1) as u8;
        c.garrison_unit = id + 3;
        c.levy_surcharge = (id as i32 % 4) * 5 + 5;

        c.castle_type = (id % 6) as u8;
        c.castle_building = ((id + 1) % 6) as u8;
        c.castle_degraded = (id % 3) as u8;
        c.castle_ruined = id % 2 == 1;
        c.castle_level_left = (id % 5) as u8;
        c.castle_switch = id % 3 == 1;
        c.castle_progress = n * 13 + 1;

        c.event_population_pct = 33 + n;
        c.event_grain_pct = 34 + n;
        c.event_herd_pct = 35 + n;

        for slot in 0..c.field_tiles.len() {
            c.field_tiles[slot] = ((slot as u16 * 11 + id as u16) % 4096) + 1;
        }
        c.tax_suppressed = id % 4 == 0;
        c.unrest_warned = id % 2 == 0;
        c.event_id = id as u16 * 3;
        c.weapon_type = id % 6;
        c.labour[0] = 500 + id as i32;
        c.labour[3] = 120;
        c.inflow_sources[0] = 9;
        c.inflow_sources[15] = 4;
        for slot in 0..c.field_progress.len() {
            c.field_progress[slot] = (slot as u16 * 37) % 801;
        }
        c.fields_fallow = 3;
        c.fields_cattle = 4;
        c.fields_grain = 6;
        c.fields_waste = 7;
        c.fields_reclaiming = 2;
        c.fertility = -5 - n;
        c.weather = Weather::ALL[id % 6];
        c.dryness = 30 + n;

        c.grain = 100 + n * 5;
        c.crop = [n + 1, n * 2 + 1, n * 3 + 1];
        c.fields_grain_sown = 5;
        c.sow_shortfall = id % 2 == 1;

        c.herd = 60 + n * 3;
        c.herd_crowding = 30 + n;
        c.herd_births_expected = 36 + n;
        c.herd_deaths_expected = 37 + n;
        c.herd_change_expected = -38 - n;

        for slot in 0..c.industry.len() {
            let s = slot as i32;
            c.industry[slot].output = 10 * (s + 1) + n;
            c.industry[slot].efficiency = 5 * (s + 1) + n;
            c.industry[slot].capacity = 25 * (s + 1);
            c.industry[slot].has_resource = slot != 3;
            c.industry[slot].enabled = slot != 2;
            c.industry[slot].disabled_seasons = s % 3 + 1;
            c.industry[slot].total = 100 * (s + 1) + n;
        }
        c.weapon_type = id % 6;
        c.farm_style = (id % 5 + 1) as u8;
        c.tax_suppressed = id % 4 == 0;
    }

    furnish_campaign(&mut k);

    // The ring, wrapped: `head`, `tail` and `len` all have to be non-zero and
    // unequal, and a reader that reconstructed them would get it wrong. Only
    // `History::record` can move them, so the census reaches them through it.
    //
    // **A different line every season**, so that all four hundred slots hold
    // four hundred different things. A ring recorded from one snapshot would
    // round-trip identically whether or not the encoder walked every slot, and
    // `no_record_slot_is_silenced` cannot reach in here to perturb one.
    let mut snapshot = k.counties.clone();
    for season in 0..(l2_kingdom::tables::HISTORY_SEASONS + 25) {
        snapshot[1].population = 1_000 + season as i32;
        snapshot[2].happiness = (season % 100) as i32 + 1;
        k.history.record(&snapshot);
    }

    k
}

/// The campaign half of a furnished kingdom: units of all four types, a map
/// with something in every plane, mercenary bands mid-walk, and name counters
/// that have been drawn from.
///
/// Same reasoning as [`furnished`] itself — a unit array tested only on empty
/// slots is a unit array tested only on empty slots.
fn furnish_campaign(k: &mut Kingdom) {
    use l2_kingdom::unit::{Mercenaries, TroopType, Unit, UnitKind};

    for i in 0..l2_kingdom::MAP_TILES {
        k.campaign.map.terrain[i] = (i % 24) as u8 + 1;
        k.campaign.map.flags[i] = (i % 7) as u8 + 1;
        k.campaign.map.county[i] = (i % 14) as u8 + 1;
    }

    let kinds = [UnitKind::Army, UnitKind::PeasantMob, UnitKind::Merchant, UnitKind::Transport];
    for (n, kind) in kinds.into_iter().enumerate() {
        let mut u = Unit::new(kind, 0, 0, 0);
        u.owner = (n % 5) as u8 + 1;
        u.x = 10 + n as u8;
        u.y = 20 + n as u8;
        u.owner_is_human = n % 2 == 0;
        u.shield = n as u8 + 1;
        u.player_driven = n % 2 == 1;
        u.facing = (n as u8 * 2) % 8 + 1;
        u.county = n as u8 + 1;
        u.home_county = n as u8 + 2;
        u.dest = (n % 2 == 0).then_some((30 + n as u8, 40));
        u.path = (0..n * 3 + 1).map(|s| (s as u8, (s * 2) as u8)).collect();
        u.moving = n % 2 == 0;
        u.on_road = n % 2 == 1;
        u.name_index = n as u8 + 3;
        u.needs_destination = n % 2 == 1;
        u.dest_county = n as u8 + 4;
        u.moves_used = n as i32 + 1;
        u.move_allowance = 15;
        u.starvation = (n as i32) % 5 + 1;
        u.wages = 40 + n as i32;
        u.year_formed = 1268 + n as i32;
        u.morale = 50 + n as i32;
        u.troops = core::array::from_fn(|t| (t as i32 + 1) * (n as i32 + 1));
        u.men = u.troops.iter().sum();
        u.mercenaries = (n != 3).then(|| Mercenaries {
            band: n as u8 + 1,
            troop: TroopType::from_index(n % 7).unwrap(),
            men: 150 + n as u8,
        });
        u.men += u.mercenaries.map_or(0, |m| m.men as i32);
        u.garrison_county = n as u8 + 3;
        u.besieging_county = n as u8 + 5;
        u.besieged_by = n as u8 + 2;
        u.cargo_county = n as u8 + 6;
        // `+0x167`, the county-defence mark — `docs/armies.md` §8.1, and the
        // second half of what `VERSION` 10 restored.
        u.defence_mark = (n % 2 + 1) as u8;
        // The siege build records and the countdown, saturated the same
        // way: every unit carries one rather than only the besieger, so
        // the round trip covers them on every slot it walks.
        for (e, record) in u.engines.iter_mut().enumerate() {
            record.ordered = (n + e) as i16 % 5;
            record.percent = ((n + e) as i16 * 7) % 101;
            record.work_done = ((n + e) as i16 * 43) % 201;
        }
        u.siege_seasons_left = (n % 4) as u8;
        // Slot 1 upward, but not contiguously: a gap is state too.
        k.campaign.units.put(n * 2 + 1, u);
    }

    k.campaign.mercenaries = l2_kingdom::MercenaryBands::init(14);
    let mut counties = k.counties.clone();
    for _ in 0..3 {
        k.campaign.mercenaries.advance(&mut counties, 14);
    }
    k.counties = counties;
    for band in 1..l2_kingdom::mercenary::BAND_SLOTS {
        let mut b = k.campaign.mercenaries.band_raw(band);
        b.hired_by = band as u16 + 1;
        b.offered_in = (band % 14 + 1) as u8;
        b.next_county = (band % 13 + 1) as u8;
        b.countdown = band as i8 + 1;
        b.reload = band as i8 + 2;
        k.campaign.mercenaries.set_band_raw(band, b);
    }
    k.campaign.mercenaries.set_in_play(4);

    for realm in 1..=5u8 {
        for _ in 0..realm {
            k.campaign.names.pick(realm);
        }
    }

    for route in 0..l2_kingdom::merchant::ROUTES {
        let mut row = [0u8; l2_kingdom::merchant::ROUTE_SLOTS];
        for (slot, county) in row.iter_mut().enumerate() {
            *county = ((route + slot) % 14 + 1) as u8;
        }
        k.campaign.routes.set_row(route, row);
    }
    k.campaign.mob_cursor = 7;
}
// --- the fixture: end -----------------------------------------------------

/// A furnished kingdom with several seasons behind it — a ring with entries in
/// it, a generator that has been drawn from, and a clock that has rolled a
/// year.
fn played(seasons: usize) -> Kingdom {
    let mut k = furnished(0xC0FFEE);
    k.start_new_game();
    for _ in 1..seasons {
        k.advance_season();
    }
    k
}

// --- round-trip ------------------------------------------------------------

#[test]
fn an_empty_kingdom_round_trips() {
    let k = Kingdom::new(7);
    let back = decode(&encode(&k), Tables::DEFAULT).expect("must decode");
    assert_eq!(back, k);
}

#[test]
fn a_furnished_kingdom_round_trips_field_for_field() {
    let k = furnished(1234);
    let back = decode(&encode(&k), Tables::DEFAULT).expect("must decode");
    assert_eq!(back, k);
}

/// The requirement that matters for "quit and resume": the state after a few
/// turns have run, not only a starting position.
#[test]
fn a_kingdom_with_turns_behind_it_round_trips() {
    for seasons in [1usize, 2, 5, 13] {
        let k = played(seasons);
        assert_eq!(k.turn_count as usize, seasons);
        let back = decode(&encode(&k), Tables::DEFAULT).expect("must decode");
        assert_eq!(back, k, "after {seasons} seasons");
    }
}

/// And a resumed kingdom keeps playing the same game: the generator, the clock
/// and the history ring all survive, so the next ten seasons are identical
/// whether or not the game was saved in between.
#[test]
fn resuming_from_a_save_plays_out_identically() {
    let mut original = played(6);
    let mut resumed = decode(&encode(&original), Tables::DEFAULT).expect("must decode");
    for season in 1..=10 {
        let a = original.advance_season();
        let b = resumed.advance_season();
        assert_eq!(a, b, "reports diverged at season {season}");
        assert_eq!(original, resumed, "state diverged at season {season}");
    }
}

/// A ring that has wrapped is the interesting case: `head`, `tail` and `len`
/// all matter, and a reader that reconstructed them would get it wrong.
#[test]
fn a_history_ring_that_has_wrapped_round_trips() {
    let mut k = furnished(5);
    k.start_new_game();
    for _ in 0..(l2_kingdom::tables::HISTORY_SEASONS + 25) {
        k.advance_season();
    }
    assert_eq!(k.history.len(), l2_kingdom::tables::HISTORY_SEASONS, "the ring is full");
    let back = decode(&encode(&k), Tables::DEFAULT).expect("must decode");
    assert_eq!(back, k);
    assert_eq!(back.history.county(1), k.history.county(1), "oldest first, still");
}

// --- determinism -----------------------------------------------------------

/// Same state in, identical bytes out — a hundred times, and across two
/// independently built kingdoms rather than one encoded twice.
#[test]
fn the_same_state_encodes_to_identical_bytes() {
    let first = encode(&played(4));
    for _ in 0..8 {
        assert_eq!(encode(&played(4)), first);
    }
    let a = furnished(99);
    let b = furnished(99);
    assert_eq!(a, b);
    assert_eq!(encode(&a), encode(&b));
    assert_eq!(checksum(&a), checksum(&b));
}

/// The checksum in the trailer is the checksum of the body, and it is the same
/// number [`checksum`] returns without writing anything — so a desync dump and
/// a save taken from the same state cannot disagree.
#[test]
fn the_trailer_is_the_state_checksum() {
    let k = played(3);
    let bytes = encode(&k);
    let trailer = u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().unwrap());
    assert_eq!(trailer, checksum(&k));
}


/// **The body is a fixed and known number of bytes, and that number is a
/// tripwire.** It moves whenever the state encoding does, which is exactly when
/// [`VERSION`] needs a thought — the same guard
/// `the_fingerprint_covers_a_fixed_and_known_number_of_bytes` puts on the
/// ruleset. Measured over `Kingdom::new`, not the fixture, so it is a fact
/// about the schema rather than about what this file happens to furnish.
#[test]
fn the_body_covers_a_fixed_and_known_number_of_bytes() {
    let mut c = l2_net::Canonical::hashing();
    l2_net::Encode::encode(&Kingdom::new(1), &mut c);
    // 56,566 at VERSION 11; +5 at 12 for `Options::exploration` (one byte) and
    // `Options::time_limit` (four).
    assert_eq!(c.finish().len, 56_571, "the state encoding changed - bump VERSION?");
}

/// **No record slot is silenced.** Every county, every realm, every unit slot,
/// every mercenary band and every merchant route is reached by the encoding,
/// and the loops that check it are over the array lengths rather than over a
/// list of slots.
///
/// The other half of the same property the census covers. A field can go
/// missing from a record; a whole *record* can go missing from the walk over
/// the array, and no amount of field checking sees that. The original game has
/// exactly this bug in its own sync checksum: `Sync_BuildDigest` (`0x00440231`)
/// fills eight per-block digest bytes and, as its last statement before summing
/// them, overwrites byte 7 — the battle-unit block — with the constant 1, so
/// that block's divergences are silenced in the shipped build. It is the same
/// failure as C30 in someone else's hand, and it costs six loops to make
/// impossible here.
#[test]
fn no_record_slot_is_silenced() {
    let base = furnished(11);
    let reference = encode(&base);

    let check = |what: String, mutate: &dyn Fn(&mut Kingdom)| {
        let mut changed = base.clone();
        mutate(&mut changed);
        assert_ne!(changed, base, "the {what} mutation changed no state");
        assert_ne!(encode(&changed), reference, "{what} does not reach the bytes");
        assert_eq!(decode(&encode(&changed), Tables::DEFAULT).unwrap(), changed, "{what}");
    };

    for slot in 0..MAX_COUNTIES {
        check(format!("county {slot}"), &move |k: &mut Kingdom| k.counties[slot].population += 1);
    }
    for slot in 0..MAX_REALMS {
        check(format!("realm {slot}"), &move |k: &mut Kingdom| k.realms[slot].gold += 1);
        for other in 0..MAX_REALMS {
            check(format!("realm {slot} pair {other}"), &move |k: &mut Kingdom| {
                k.realms[slot].pairs[other].standing += 1
            });
        }
    }
    for slot in 0..l2_kingdom::unit::MAX_UNITS {
        check(format!("unit slot {slot}"), &move |k: &mut Kingdom| match k
            .campaign
            .units
            .get_mut(slot)
        {
            Some(u) => u.men += 1,
            None => {
                k.campaign.units.put(slot, l2_kingdom::unit::Unit::new(
                    l2_kingdom::unit::UnitKind::Army,
                    1,
                    5,
                    6,
                ));
            }
        });
    }
    for band in 1..l2_kingdom::mercenary::BAND_SLOTS {
        check(format!("band {band}"), &move |k: &mut Kingdom| {
            let mut b = k.campaign.mercenaries.band_raw(band);
            b.next_county += 1;
            k.campaign.mercenaries.set_band_raw(band, b);
        });
    }
    for route in 0..l2_kingdom::merchant::ROUTES {
        check(format!("route {route}"), &move |k: &mut Kingdom| {
            let mut row = *k.campaign.routes.row(route);
            row[0] += 1;
            k.campaign.routes.set_row(route, row);
        });
    }
    for (plane, name) in ["terrain", "flags", "county"].into_iter().enumerate() {
        for tile in [0usize, 1, l2_kingdom::MAP_TILES / 2, l2_kingdom::MAP_TILES - 1] {
            check(format!("map {name} tile {tile}"), &move |k: &mut Kingdom| {
                let m = &mut k.campaign.map;
                let target = match plane {
                    0 => &mut m.terrain,
                    1 => &mut m.flags,
                    _ => &mut m.county,
                };
                target[tile] = target[tile].wrapping_add(1);
            });
        }
    }
    // The history ring's 400 seasons are not perturbed here — `History`'s
    // entries are `pub(crate)` and a test cannot write one. They are covered
    // instead by the fixture, which records a *different* line every season, so
    // a silenced slot is a slot the round trip finds holding another season's
    // numbers.
}

// --- the census ------------------------------------------------------------
//
// The two tests below are the mechanism `docs/decisions.md` C30 asked for and
// C39 delivered: the field list is *derived from the struct definitions*
// instead of retyped, so it cannot quietly cover fewer fields than exist.

/// Fields of the reachable state that the save body deliberately does not
/// carry. **Inclusion is the default and exclusion is the statement**, so a
/// field added tomorrow fails the census rather than slipping past it; a line
/// here is a claim, with its reason, that the field is not simulation state.
///
/// Reachability stops at these fields, so a whole subtree can be excused by its
/// root — which is what `Kingdom::tables` does for the ruleset.
const NOT_IN_THE_BODY: &[(&str, &str, &str)] = &[(
    "Kingdom",
    "tables",
    "the ruleset is fingerprinted into the header rather than written into the \
     body (docs/modding.md); every_sub_table_reaches_the_fingerprint covers it",
)];

/// Fields the fixture cannot name, and the call it must make instead.
///
/// A private field, or one only a constructor writes. **This table is the only
/// remembered part of the census and its length is the exact size of the hole
/// in it** — each line trades a by-name check for a by-call one, so keep it
/// short and keep the call specific.
const FURNISHED_BY_CALL: &[(&str, &str, &str)] = &[
    ("Units", "slots", "units.put("),
    ("ArmyNames", "counters", "names.pick("),
    ("MerchantRoutes", "rows", "routes.set_row("),
    ("MercenaryBands", "bands", "set_band_raw("),
    ("MercenaryBands", "in_play", "set_in_play("),
    ("History", "entries", "history.record("),
    ("History", "head", "history.record("),
    ("History", "tail", "history.record("),
    ("History", "len", "history.record("),
    ("HistoryEntry", "population", "history.record("),
    ("HistoryEntry", "happiness", "history.record("),
    ("County", "neighbour_count", "add_neighbour("),
    ("County", "neighbours", "add_neighbour("),
    ("Unit", "kind", "Unit::new(kind,"),
];

/// **Every field of the state is furnished, and the list of fields is read out
/// of the source rather than remembered.**
///
/// This is the guard `docs/decisions.md` C30 said was missing and C39 wrote.
/// The old one was an enumeration of mutations: it checked the fields somebody
/// had listed, so the four fields C30 is about were absent from the encoding
/// *and* from the list, and the list agreed with itself. Six more were found
/// later by matching it against `County` by hand, and twelve more — the whole
/// diplomacy record and the county-defence mark — were found by the first run
/// of this test.
///
/// The mechanism, in three steps:
///
/// 1. parse every `struct` in `crates/l2-kingdom/src` and its fields;
/// 2. walk the types from `Kingdom`, stopping at [`NOT_IN_THE_BODY`], which
///    gives the exact set of fields the save body has to carry;
/// 3. require each one to be furnished between the fixture markers above.
///
/// Saturation is what gives `a_furnished_kingdom_round_trips_field_for_field`
/// its power: `#[derive(PartialEq)]` reads every field exhaustively, and over a
/// value with no defaults in it, a field the encoder drops comes back as the
/// default and the comparison fails. This test and that one are one guard in
/// two halves; neither is worth much alone.
#[test]
fn every_field_of_the_state_is_furnished() {
    let structs = census::structs_of(&census::repo_root().join("crates/l2-kingdom/src"));

    // A parser that matched nothing would let everything through, so it is
    // checked against the shape of the tree before it is trusted.
    assert!(structs.len() >= 40, "only {} structs parsed - the parser broke", structs.len());
    assert!(
        structs["County"].len() >= 85,
        "County parsed as {} fields - the parser broke",
        structs["County"].len()
    );

    census::check_tables_are_live(&structs, NOT_IN_THE_BODY, "NOT_IN_THE_BODY");
    census::check_tables_are_live(&structs, FURNISHED_BY_CALL, "FURNISHED_BY_CALL");

    let reachable = census::reachable(&structs, "Kingdom", NOT_IN_THE_BODY);
    let fixture = census::fixture_region();

    let mut fields = 0usize;
    let mut missing: Vec<String> = Vec::new();
    for name in &reachable {
        for (field, _) in &structs[name] {
            if census::listed(NOT_IN_THE_BODY, name, field).is_some() {
                continue;
            }
            fields += 1;
            let found = match census::listed(FURNISHED_BY_CALL, name, field) {
                Some(call) => fixture.contains(call),
                None => census::mentions(&fixture, field),
            };
            if !found {
                missing.push(format!("  {name}::{field}"));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "{} field(s) of the state are not furnished, so the round trip cannot see them:\n{}\n\n\
         Give each a value `{}::new` would not, between the fixture markers in this file. \
         A field that is genuinely not simulation state goes in NOT_IN_THE_BODY with its \
         reason - and then out of `l2_kingdom::save`'s Encode impl too.",
        missing.len(),
        missing.join("\n"),
        "County"
    );

    // **A floor under the walk, not a target.** Everything above is a check
    // that each field *found* is furnished, and a walk that found nothing would
    // satisfy all of it. The bounds are deliberately far below the real numbers
    // so that adding a field never has to touch them, and far above what a
    // broken walk would return.
    assert!(reachable.len() >= 15, "only {} structs reached from Kingdom", reachable.len());
    assert!(fields >= 200, "only {fields} fields reached from Kingdom");

    // The exact counts are printed rather than asserted: a number to argue
    // with, in the spirit of `crates/l2-testkit/tests/census.rs`, without a
    // second place to update every time a field lands.
    println!(
        "{} structs reachable from Kingdom, {fields} fields, all furnished",
        reachable.len()
    );
}

/// **`VERSION` must be ahead of its own changelog, with no gap and no repeat.**
///
/// The version number is the one constant in `save.rs` that *every* branch
/// changing the layout has to touch and *no* branch can see the others touch.
/// It has now collided three times in a single day — 5 twice, then 6 twice,
/// then 7 twice — and each time the merge produced a number that already meant a
/// different layout in somebody else's save. All three were caught by an
/// integrator reading the doc comment, which is the same "a check nobody runs"
/// failure `tools/decisions/corrections.js` was written to end for correction
/// numbers.
///
/// This is that check. It reads `src/save.rs`, pulls the `* N —` entries out of
/// `VERSION`'s doc comment, and asserts three things:
///
/// * the entries are `1 … n` with no gap and no duplicate — a duplicate is
///   exactly what a two-branch collision leaves behind;
/// * `VERSION` equals the highest of them — so a merge that keeps one branch's
///   constant and both branches' entries goes red;
/// * every entry actually says something, which is the reason the changelog
///   exists: an older save's *absence* of a field is a question, and the answer
///   belongs here rather than in a commit message.
///
/// It cannot prevent two branches choosing the same number. It fails the moment
/// they meet, which is the earliest a machine can know.
#[test]
fn the_version_is_ahead_of_its_own_changelog() {
    let source = include_str!("../src/save.rs");
    let head = source.split("pub const VERSION").next().expect("VERSION is declared");
    let mut entries: Vec<u32> = Vec::new();
    for line in head.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("/// * ") else { continue };
        let Some((number, tail)) = rest.split_once(' ') else { continue };
        let Ok(n) = number.parse::<u32>() else { continue };
        assert!(
            tail.starts_with('\u{2014}') || tail.starts_with('-'),
            "changelog entry {n} has no description: {line}"
        );
        entries.push(n);
    }

    assert!(entries.len() > 1, "the changelog was not found; did the comment's shape change?");

    let expected: Vec<u32> = (1..=entries.len() as u32).collect();
    assert_eq!(
        entries, expected,
        "the save changelog is {entries:?}, which is not 1..={}. A repeated number is what \
         two branches bumping VERSION in parallel leaves behind: give the later layout the \
         next free number and say so in its entry.",
        entries.len()
    );

    assert_eq!(
        VERSION,
        *entries.last().unwrap(),
        "VERSION is {VERSION} and the changelog's last entry is {}. Every layout change \
         needs both, and a merge that takes one branch's constant with both branches' \
         entries is exactly what this catches.",
        entries.last().unwrap()
    );
}

// --- the header ------------------------------------------------------------

#[test]
fn the_header_is_a_magic_a_version_a_fingerprint_and_a_length() {
    let k = furnished(1);
    let bytes = encode(&k);
    assert_eq!(&bytes[..8], &MAGIC);
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), VERSION);
    assert_eq!(
        u64::from_le_bytes(bytes[12..20].try_into().unwrap()),
        ruleset_fingerprint(&Tables::DEFAULT)
    );
    let declared = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
    assert_eq!(declared, bytes.len() - HEADER_LEN - 8, "body, then an eight-byte checksum");
}

/// **An unknown version is refused, not guessed at.** The one requirement that
/// cannot be checked by round-tripping, because it is about a file this build
/// did not write.
#[test]
fn an_unknown_version_is_refused_rather_than_read() {
    let mut bytes = encode(&furnished(1));
    bytes[8..12].copy_from_slice(&(VERSION + 1).to_le_bytes());
    assert_eq!(
        decode(&bytes, Tables::DEFAULT),
        Err(LoadError::UnsupportedVersion { found: VERSION + 1, supported: VERSION })
    );

    // Including version 0, which is what a zero-filled file looks like.
    let mut zeroed = encode(&furnished(1));
    zeroed[8..12].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        decode(&zeroed, Tables::DEFAULT),
        Err(LoadError::UnsupportedVersion { found: 0, .. })
    ));
}

#[test]
fn something_that_is_not_a_save_is_refused() {
    assert_eq!(decode(b"", Tables::DEFAULT), Err(LoadError::NotASave));
    assert_eq!(decode(b"hello", Tables::DEFAULT), Err(LoadError::NotASave));
    assert_eq!(decode(&vec![0u8; 64], Tables::DEFAULT), Err(LoadError::NotASave));
    // The right length, the wrong magic.
    let mut bytes = encode(&Kingdom::new(1));
    bytes[0] = b'X';
    assert_eq!(decode(&bytes, Tables::DEFAULT), Err(LoadError::NotASave));
}

/// A save made under one ruleset will not load under another. The rules come
/// from the mod layer, and a silent mismatch is a campaign that quietly changes
/// its own economy halfway through.
#[test]
fn a_save_made_under_another_ruleset_is_refused() {
    let mut modded = Tables::DEFAULT;
    modded.grain.yield_per_sack = 24;
    let k = Kingdom::with_tables(1, modded);
    let bytes = encode(&k);

    match decode(&bytes, Tables::DEFAULT) {
        Err(LoadError::RulesetMismatch { save, supplied }) => {
            assert_eq!(save, ruleset_fingerprint(&modded));
            assert_eq!(supplied, ruleset_fingerprint(&Tables::DEFAULT));
            assert_ne!(save, supplied);
        }
        other => panic!("expected a ruleset mismatch, got {other:?}"),
    }
    // And it loads under the one it was written with.
    assert_eq!(decode(&bytes, modded).unwrap(), k);
}

/// Every sub-table reaches the fingerprint. Without this, a constant added to
/// `Tables` and forgotten in the encoder would leave two different rulesets
/// sharing a fingerprint — and the mismatch check above would pass a save it
/// should refuse.
#[test]
fn every_sub_table_reaches_the_fingerprint() {
    let base = ruleset_fingerprint(&Tables::DEFAULT);
    let mutations: Vec<(&str, Box<dyn Fn(&mut Tables)>)> = vec![
        ("food", Box::new(|t: &mut Tables| t.food.dairy_per_head += 1)),
        ("grain", Box::new(|t: &mut Tables| t.grain.yield_per_sack += 1)),
        ("field", Box::new(|t: &mut Tables| t.field.reclaim_per_season += 1)),
        ("event", Box::new(|t: &mut Tables| t.event.first_year += 1)),
        ("season", Box::new(|t: &mut Tables| t.season[4].death_rate += 1)),
        ("season dryness", Box::new(|t: &mut Tables| t.season[1].dryness += 1)),
        ("ration", Box::new(|t: &mut Tables| t.ration[3].divisor += 1)),
        ("health delta", Box::new(|t: &mut Tables| t.ration[3].health_delta[2] += 1)),
        ("ration happiness", Box::new(|t: &mut Tables| t.ration_happiness_slope += 1)),
        ("ration offset", Box::new(|t: &mut Tables| t.ration_happiness_offset += 1)),
        ("health", Box::new(|t: &mut Tables| t.health[3].death_rate += 1)),
        ("health ladder", Box::new(|t: &mut Tables| t.health_band_ladder[2].0 += 1)),
        ("birth ladder", Box::new(|t: &mut Tables| t.population.birth_rate_ladder[19].1 += 1)),
        ("happiness factor", Box::new(|t: &mut Tables| t.population.happiness_factor_ladder[4].1 += 1)),
        ("tax happiness", Box::new(|t: &mut Tables| t.tax_happiness_other[50] += 1)),
        ("weather", Box::new(|t: &mut Tables| t.weather[5].herd_pct += 1)),
        ("herd staffing", Box::new(|t: &mut Tables| t.herd.labour_per_head += 1)),
        ("herd staffing cap", Box::new(|t: &mut Tables| t.herd.staffing_max += 1)),
        ("herd understaffing", Box::new(|t: &mut Tables| t.herd.understaffing_divisor += 1)),
        ("herd crowding", Box::new(|t: &mut Tables| t.herd.crowding[3].birth_rate += 1)),
        ("herd crowding band", Box::new(|t: &mut Tables| t.herd.crowding[0].density_max += 1)),
        ("herd small bonus", Box::new(|t: &mut Tables| t.herd.small_bonus[2].1 += 1)),
        ("herd no pasture", Box::new(|t: &mut Tables| t.herd.no_pasture_divisor += 1)),
        ("herd no pasture floor", Box::new(|t: &mut Tables| t.herd.no_pasture_kill_all_below += 1)),
        ("herd no pasture density", Box::new(|t: &mut Tables| t.herd.no_pasture_density += 1)),
        ("herd calving", Box::new(|t: &mut Tables| t.herd.calving_season += 1)),
        ("herd culling", Box::new(|t: &mut Tables| t.herd.culling_season += 1)),
        ("herd season bonus", Box::new(|t: &mut Tables| t.herd.season_bonus.1 += 1)),
        ("cattle job", Box::new(|t: &mut Tables| t.job.cattle_farming += 1)),
        ("castle start", Box::new(|t: &mut Tables| t.castle.starting_type += 1)),
        ("castle tax", Box::new(|t: &mut Tables| t.castle.tax_base[5] += 1)),
        ("castle bonus", Box::new(|t: &mut Tables| t.castle.tax_bonus_pct[5] += 1)),
        ("castle cost", Box::new(|t: &mut Tables| t.castle.cost[4].1 += 1)),
        ("castle workforce", Box::new(|t: &mut Tables| t.castle.workforce[4].1 += 1)),
        ("garrison", Box::new(|t: &mut Tables| t.castle.garrison_cap[5] += 1)),
        ("free archers", Box::new(|t: &mut Tables| t.castle.free_archers[5] += 1)),
        ("commodity", Box::new(|t: &mut Tables| t.commodity[3].divisor += 1)),
        ("commodity job", Box::new(|t: &mut Tables| t.commodity[3].job += 1)),
        ("job", Box::new(|t: &mut Tables| t.job.castle_building += 1)),
        ("weapon", Box::new(|t: &mut Tables| t.weapon[5].iron += 1)),
        ("good", Box::new(|t: &mut Tables| t.good[14].sell_price += 1)),
        ("wages", Box::new(|t: &mut Tables| t.wages.divisor_ai[2] += 1)),
        ("bankrupt max", Box::new(|t: &mut Tables| t.wages.bankrupt_stage_max += 1)),
        ("efficiency", Box::new(|t: &mut Tables| t.efficiency.max += 1)),
        ("ale", Box::new(|t: &mut Tables| t.ale.max += 1)),
        ("army happiness", Box::new(|t: &mut Tables| t.army_happiness_cost[101] += 1)),
        ("gold grant", Box::new(|t: &mut Tables| t.ai.gold_grant[4][3] += 1)),
        ("grants", Box::new(|t: &mut Tables| t.ai.grant_min_grain += 1)),
        ("neutral ladder", Box::new(|t: &mut Tables| t.ai.tax_ladder_neutral[7].1 += 1)),
        ("ai ladders", Box::new(|t: &mut Tables| t.ai.tax_ladders[2][7].1 += 1)),
        ("personality style", Box::new(|t: &mut Tables| t.ai.personality[3].farm_style += 1)),
        ("personality ladder", Box::new(|t: &mut Tables| t.ai.personality[3].tax_ladder += 1)),
        ("gold brackets", Box::new(|t: &mut Tables| t.score.gold_brackets[3].1 += 1)),
        ("score weights", Box::new(|t: &mut Tables| t.score.weights[5].1 += 1)),
        ("score offsets", Box::new(|t: &mut Tables| t.score.input_offsets[5] += 1)),
    ];

    for (name, mutate) in &mutations {
        let mut t = Tables::DEFAULT;
        mutate(&mut t);
        assert_ne!(t, Tables::DEFAULT, "the {name} mutation changed nothing");
        assert_ne!(ruleset_fingerprint(&t), base, "changing {name} left the fingerprint alone");
    }
}

/// The fingerprint hashes a fixed number of bytes, and that number is a
/// tripwire: it moves whenever the ruleset encoding does, which is exactly when
/// the format version needs a thought.
#[test]
fn the_fingerprint_covers_a_fixed_and_known_number_of_bytes() {
    let mut c = l2_net::Canonical::hashing();
    l2_net::Encode::encode(&Tables::DEFAULT, &mut c);
    assert_eq!(c.finish().len, 2_168, "the ruleset encoding changed - bump VERSION?");
}

// --- corruption ------------------------------------------------------------

/// A flipped byte anywhere in the body is caught by the trailer, rather than
/// producing a kingdom with one wrong number in it.
#[test]
fn a_flipped_byte_is_caught_by_the_checksum() {
    let k = furnished(2);
    let bytes = encode(&k);
    // Walk the body in strides so the test stays quick but still covers
    // counties, realms and the history ring.
    let body = HEADER_LEN..bytes.len() - 8;
    let mut caught = 0;
    for at in body.step_by(997) {
        let mut spoiled = bytes.clone();
        spoiled[at] ^= 0x01;
        match decode(&spoiled, Tables::DEFAULT) {
            Err(LoadError::Corrupt { .. }) => caught += 1,
            Err(other) => panic!("byte {at}: {other}"),
            Ok(_) => panic!("byte {at} was flipped and the save still loaded"),
        }
    }
    assert!(caught > 20, "only {caught} strides checked");
}

#[test]
fn a_truncated_save_is_refused() {
    let bytes = encode(&furnished(2));
    for cut in [HEADER_LEN + 8, bytes.len() / 2, bytes.len() - 9, bytes.len() - 1] {
        let err = decode(&bytes[..cut], Tables::DEFAULT).expect_err("must refuse");
        assert!(
            matches!(err, LoadError::TruncatedBody { .. } | LoadError::NotASave),
            "cut at {cut}: {err}"
        );
    }
}

/// Trailing rubbish is an error too: it means the writer and the reader
/// disagree about the schema, and that is worth catching at the first save
/// rather than at the tenth.
#[test]
fn trailing_bytes_are_refused() {
    let mut bytes = encode(&furnished(2));
    bytes.push(0);
    assert!(matches!(
        decode(&bytes, Tables::DEFAULT),
        Err(LoadError::TruncatedBody { .. })
    ));
}

// ---------------------------------------------------------------------------
// The census: reading the struct definitions instead of retyping them
// ---------------------------------------------------------------------------

/// **`#[derive(PartialEq)]` is the only exhaustive reader of a struct we have,
/// and it can only read a value somebody built.** This module supplies the
/// other half: the *names* of the fields, taken from the source of
/// `crates/l2-kingdom/src` rather than from anyone's memory.
///
/// It is a text scan, not a parser of Rust, and that is a deliberate choice.
/// The alternative is a derive macro, which means `syn` — and `l2-kingdom` is
/// dependency-free on purpose (`docs/netcode.md` D-3: every third-party crate
/// is a place bit-identical behaviour can quietly break). Reading source in a
/// test is already how `crates/l2-testkit/tests/census.rs` counts install-gated
/// tests, so this is the house style rather than a new idea.
///
/// The scan is deliberately fragile in the safe direction: it asserts the shape
/// of what it found before it trusts it, so a scan that silently matched
/// nothing fails rather than passing everything.
mod census {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::{Path, PathBuf};

    /// Struct name → its fields, in declaration order, as `(name, type)`.
    pub type Structs = BTreeMap<String, Vec<(String, String)>>;

    /// The workspace root, from this crate's manifest.
    pub fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
    }

    /// Every `struct Name { .. }` in a directory of `.rs` files, with its
    /// fields. Tuple structs and unit structs have no named fields and are
    /// skipped; enums are not structs and are left out, which makes them leaves
    /// of the walk in [`reachable`] — correct, because an enum has no field a
    /// save can drop, only a tag it can misread.
    pub fn structs_of(dir: &Path) -> Structs {
        let mut out = Structs::new();
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "rs"))
            .collect();
        files.sort();
        for file in files {
            let src = std::fs::read_to_string(&file).expect("a source file");
            parse_into(&src, &mut out);
        }
        out
    }

    fn parse_into(src: &str, out: &mut Structs) {
        let mut lines = src.lines().peekable();
        while let Some(line) = lines.next() {
            let Some(name) = struct_header(line) else { continue };
            let mut fields = Vec::new();
            for body in lines.by_ref() {
                if body == "}" {
                    break;
                }
                if let Some(field) = field_line(body) {
                    fields.push(field);
                }
            }
            // A later definition never overwrites an earlier one: there is no
            // duplicate struct name in this crate, and if one appears the
            // shape assertions in the test are what notice.
            out.entry(name).or_insert(fields);
        }
    }

    /// `pub struct Name {`, at column zero and with a brace, so an `impl` or a
    /// doc comment mentioning the word cannot start a struct.
    fn struct_header(line: &str) -> Option<String> {
        let rest = line
            .strip_prefix("pub(crate) struct ")
            .or_else(|| line.strip_prefix("pub struct "))
            .or_else(|| line.strip_prefix("struct "))?;
        let (name, tail) = rest.split_once(' ')?;
        if tail.trim() != "{" || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return None;
        }
        Some(name.to_string())
    }

    /// `    pub name: Type,` — one level of indentation, a name, a colon.
    /// Doc comments, attributes and section comments do not match.
    fn field_line(line: &str) -> Option<(String, String)> {
        let body = line.strip_prefix("    ")?;
        if body.starts_with(' ') || body.starts_with("//") || body.starts_with('#') {
            return None;
        }
        let body = body
            .strip_prefix("pub(crate) ")
            .or_else(|| body.strip_prefix("pub "))
            .unwrap_or(body);
        let (name, ty) = body.split_once(": ")?;
        if name.is_empty() || !name.chars().all(|c| c.is_lowercase() || c.is_numeric() || c == '_') {
            return None;
        }
        Some((name.to_string(), ty.trim_end_matches(',').to_string()))
    }

    /// Every struct reachable from `root` by following field types, stopping at
    /// the fields named in `stop`.
    ///
    /// **Stopping at a field prunes its whole subtree**, which is what lets one
    /// line excuse the entire ruleset: `Kingdom::tables` is not in the save
    /// body, so nothing under `Tables` is either.
    pub fn reachable(structs: &Structs, root: &str, stop: &[(&str, &str, &str)]) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut queue = vec![root.to_string()];
        while let Some(name) = queue.pop() {
            let Some(fields) = structs.get(&name) else { continue };
            if !seen.insert(name.clone()) {
                continue;
            }
            for (field, ty) in fields {
                if listed(stop, &name, field).is_some() {
                    continue;
                }
                for token in identifiers(ty) {
                    if structs.contains_key(&token) {
                        queue.push(token);
                    }
                }
            }
        }
        seen
    }

    /// The identifiers in a type, so `[[Pair; MAX_REALMS]; N]` yields `Pair`.
    fn identifiers(ty: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut word = String::new();
        for c in ty.chars() {
            if c.is_alphanumeric() || c == '_' {
                word.push(c);
            } else if !word.is_empty() {
                out.push(std::mem::take(&mut word));
            }
        }
        if !word.is_empty() {
            out.push(word);
        }
        out
    }

    /// The third column of a `(struct, field, note)` table, if the pair is in
    /// it.
    pub fn listed<'a>(
        table: &'a [(&'a str, &'a str, &'a str)],
        name: &str,
        field: &str,
    ) -> Option<&'a str> {
        table.iter().find(|(s, f, _)| *s == name && *f == field).map(|(_, _, note)| *note)
    }

    /// **Every line of an exemption table names a field that exists.**
    ///
    /// A stale line is worse than no line: it looks like a considered decision
    /// and covers nothing, and the field it used to name is now checked by
    /// neither the table nor anybody's memory.
    pub fn check_tables_are_live(structs: &Structs, table: &[(&str, &str, &str)], which: &str) {
        for (name, field, _) in table {
            let fields = structs
                .get(*name)
                .unwrap_or_else(|| panic!("{which} names struct `{name}`, which does not exist"));
            assert!(
                fields.iter().any(|(f, _)| f == field),
                "{which} names `{name}::{field}`, which is not a field of `{name}` - \
                 a stale exemption covers nothing. Delete the line."
            );
        }
    }

    /// The text of the fixture, between the two markers in this file, with line
    /// comments stripped so that a field named only in prose does not count.
    pub fn fixture_region() -> String {
        let src = std::fs::read_to_string(
            repo_root().join("crates/l2-kingdom/tests/save.rs"),
        )
        .expect("this file");
        let begin = "// --- the fixture: begin ";
        let end = "// --- the fixture: end ";
        let from = src.find(begin).expect("the opening fixture marker");
        let to = src.find(end).expect("the closing fixture marker");
        assert!(to > from, "the fixture markers are the wrong way round");
        src[from..to]
            .lines()
            .map(|l| match l.find("//") {
                Some(at) => &l[..at],
                None => l,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Is `field` written in the fixture — as `.field` (an assignment or a
    /// read) or as `field:` (a struct literal)?
    ///
    /// `.field(` does not count: that is a method call, and `History::len` is
    /// both a field and a method. The distinction is the reason this is not a
    /// bare substring search.
    pub fn mentions(region: &str, field: &str) -> bool {
        let dotted = format!(".{field}");
        let mut from = 0;
        while let Some(rel) = region[from..].find(&dotted) {
            let at = from + rel;
            let after = region[at + dotted.len()..].chars().next();
            match after {
                None => return true,
                Some(c) if !c.is_alphanumeric() && c != '_' && c != '(' => return true,
                _ => {}
            }
            from = at + 1;
        }
        let literal = format!("{field}:");
        let mut from = 0;
        while let Some(rel) = region[from..].find(&literal) {
            let at = from + rel;
            let before = region[..at].chars().next_back();
            match before {
                None => return true,
                Some(c) if !c.is_alphanumeric() && c != '_' && c != '.' && c != ':' => return true,
                _ => {}
            }
            from = at + 1;
        }
        false
    }
}
