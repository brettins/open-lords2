//! Our own save format: round-trip, determinism, and the refusals.
//!
//! Needs no game install — the format is ours, and nothing here reads a file.
//!
//! Three obligations, and one test group each.
//!
//! * **Round-trip.** A kingdom, encoded and decoded, is the same kingdom. Not
//!   "equivalent": `Kingdom` is `PartialEq` over every field, and that is what
//!   is asserted, before any season has run and after several have.
//! * **Determinism.** The same state produces the same bytes, every time. And,
//!   more usefully, *different* states produce different bytes — a save format
//!   that ignored a field would round-trip nothing and pass the first test
//!   while silently losing the field, so every field is mutated in turn and the
//!   bytes have to move.
//! * **Refusal.** An unknown version, a wrong ruleset, a flipped byte and a
//!   truncated file each produce their own error rather than a kingdom.

use l2_kingdom::county::ChangeReason;
use l2_kingdom::save::{
    checksum, decode, encode, ruleset_fingerprint, LoadError, HEADER_LEN, MAGIC, VERSION,
};
use l2_kingdom::tables::{Tables, Weather};
use l2_kingdom::{Kingdom, Options};

/// A kingdom with something in every corner of the record: two realms, a mix of
/// owned and unowned counties, stock and industry, a ration split, a castle
/// under construction, and a history ring with entries in it.
///
/// Deliberately not `Kingdom::new`: a save format tested only on zeros is a
/// save format tested only on zeros.
fn furnished(seed: u64) -> Kingdom {
    let mut k = Kingdom::new(seed);
    k.options = Options { difficulty: 2, advanced_farming: true, armies_eat: true };
    assert!(k.set_county_count(14));

    for id in 1..=5 {
        let r = &mut k.realms[id];
        r.in_play = true;
        r.strength = (3 * id) as u8;
        r.lord = (id - 1) as u8;
        r.gold = 1000 + id as i32 * 37;
        r.iron = 40 + id as i32;
        r.stone = 50 + id as i32;
        r.wood = 60 + id as i32;
        r.weapons = [1, 2, 3, 4, 5, 6].map(|w| w * id as i32);
        r.bankrupt_stage = (id % 5) as u8;
        r.population_total = 4000 + id as i32;
        r.mean_happiness = 40 + id as i32;
        r.score_inputs = [1, 2, 3, 4, 5, 6].map(|v| v * id as i32);
        r.ai_step = id as i32;
    }
    k.realms[1].is_human = true;
    k.realms[3].tax_hap_empire = -7;

    for id in 1..=14usize {
        let c = &mut k.counties[id];
        c.owner = match id {
            1 | 4 | 8 => 1,
            11 | 13 => 2,
            _ => 0,
        };
        c.population = 400 + id as i32 * 11;
        c.pop_last = 390 + id as i32 * 11;
        c.happiness = 50 + id as i32;
        c.happiness_last = 48 + id as i32;
        c.health_meter = 60 + id as i32;
        c.health_band = l2_kingdom::tables::health_band(c.health_meter) as u8;
        c.herd = 60 + id as i32 * 3;
        c.grain = 100 + id as i32 * 5;
        c.crop = [id as i32, id as i32 * 2, id as i32 * 3];
        c.fields_fallow = 3;
        c.fields_grain = 6;
        c.fields_cattle = 4;
        c.ration_wanted = 3;
        c.ration_split = (id * 7 % 101) as i32;
        c.tax_rate = (id % 13) as i32;
        c.dryness = 30 + id as i32;
        c.weather = Weather::ALL[id % 6];
        c.anchor_x = id as u8;
        c.anchor_y = (100 - id) as u8;
        c.change_reason = match id % 5 {
            0 => ChangeReason::None,
            1 => ChangeReason::Births,
            2 => ChangeReason::Deaths,
            3 => ChangeReason::Emigration,
            _ => ChangeReason::Immigration,
        };
        c.castle_type = (id % 6) as u8;
        c.castle_building = ((id + 1) % 6) as u8;
        c.castle_progress = id as i32 * 13;
        c.castle_degraded = id % 3 == 0;
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
        for slot in 0..c.industry.len() {
            c.industry[slot].total = 100 * (slot as i32 + 1) + id as i32;
            c.industry[slot].capacity = 25 * (slot as i32 + 1);
            c.industry[slot].disabled_seasons = (slot as i32) % 3;
            c.industry[slot].enabled = slot != 2;
            c.industry[slot].has_resource = slot != 3;
        }
        for n in 1..=14u8 {
            if n as usize != id {
                c.add_neighbour(n);
            }
        }
    }
    k
}

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

/// **The test that a round-trip alone cannot do.** A field that is never
/// written round-trips perfectly and is silently lost; the only way to catch it
/// is to change the field and require the bytes to change. Every mutation below
/// is a different corner of the record.
#[test]
fn every_part_of_the_state_reaches_the_bytes() {
    let base = furnished(3);
    let reference = encode(&base);

    let mutations: Vec<(&str, Box<dyn Fn(&mut Kingdom)>)> = vec![
        ("season", Box::new(|k: &mut Kingdom| k.season = 2)),
        ("season_next", Box::new(|k: &mut Kingdom| k.season_next = 3)),
        ("season_prev", Box::new(|k: &mut Kingdom| k.season_prev = 1)),
        ("year", Box::new(|k: &mut Kingdom| k.year = 1300)),
        ("year_next", Box::new(|k: &mut Kingdom| k.year_next = 1301)),
        ("turn_count", Box::new(|k: &mut Kingdom| k.turn_count = 42)),
        ("county_count", Box::new(|k: &mut Kingdom| { k.set_county_count(9); })),
        ("turn phase", Box::new(|k: &mut Kingdom| k.turn.phase = l2_kingdom::Phase::Merchants)),
        ("turn step", Box::new(|k: &mut Kingdom| k.turn.step = 5)),
        ("weather_county", Box::new(|k: &mut Kingdom| k.weather_county = 9)),
        ("difficulty", Box::new(|k: &mut Kingdom| k.options.difficulty = 1)),
        ("advanced_farming", Box::new(|k: &mut Kingdom| k.options.advanced_farming = false)),
        ("armies_eat", Box::new(|k: &mut Kingdom| k.options.armies_eat = false)),
        ("rng", Box::new(|k: &mut Kingdom| { k.rng.next_u32(); })),
        ("owner", Box::new(|k: &mut Kingdom| k.counties[2].owner = 4)),
        ("event_fired", Box::new(|k: &mut Kingdom| k.counties[2].event_fired = true)),
        ("event_id", Box::new(|k: &mut Kingdom| k.counties[2].event_id = 999)),
        ("health_band", Box::new(|k: &mut Kingdom| k.counties[2].health_band = 4)),
        ("health_meter", Box::new(|k: &mut Kingdom| k.counties[2].health_meter = 12)),
        ("happiness", Box::new(|k: &mut Kingdom| k.counties[2].happiness = 12)),
        ("happiness_last", Box::new(|k: &mut Kingdom| k.counties[2].happiness_last = 12)),
        ("d_hap_tax", Box::new(|k: &mut Kingdom| k.counties[2].d_hap_tax = 12)),
        ("d_hap_tax_local", Box::new(|k: &mut Kingdom| k.counties[2].d_hap_tax_local = 12)),
        ("d_hap_health", Box::new(|k: &mut Kingdom| k.counties[2].d_hap_health = 12)),
        ("d_hap_ration", Box::new(|k: &mut Kingdom| k.counties[2].d_hap_ration = 12)),
        ("shown_tax", Box::new(|k: &mut Kingdom| k.counties[2].shown_tax = 12)),
        ("shown_ration", Box::new(|k: &mut Kingdom| k.counties[2].shown_ration = 12)),
        ("shown_health", Box::new(|k: &mut Kingdom| k.counties[2].shown_health = 12)),
        ("shown_army", Box::new(|k: &mut Kingdom| k.counties[2].shown_army = 12)),
        ("shown_events", Box::new(|k: &mut Kingdom| k.counties[2].shown_events = 12)),
        ("shown_ale", Box::new(|k: &mut Kingdom| k.counties[2].shown_ale = 12)),
        ("ale_given", Box::new(|k: &mut Kingdom| k.counties[2].ale_happiness_given = 3)),
        ("tax_hap_other", Box::new(|k: &mut Kingdom| k.counties[2].tax_hap_other = 3)),
        ("happiness_avg", Box::new(|k: &mut Kingdom| k.counties[2].happiness_avg = 3)),
        ("happiness_sum", Box::new(|k: &mut Kingdom| k.counties[2].happiness_sum = 3)),
        ("unrest", Box::new(|k: &mut Kingdom| k.counties[2].unrest = 3)),
        ("unrest_warned", Box::new(|k: &mut Kingdom| k.counties[2].unrest_warned = false)),
        ("population", Box::new(|k: &mut Kingdom| k.counties[2].population = 3)),
        ("pop_last", Box::new(|k: &mut Kingdom| k.counties[2].pop_last = 3)),
        ("pop_change_pct", Box::new(|k: &mut Kingdom| k.counties[2].pop_change_pct = 3)),
        ("births", Box::new(|k: &mut Kingdom| k.counties[2].births = 3)),
        ("deaths", Box::new(|k: &mut Kingdom| k.counties[2].deaths = 3)),
        ("army", Box::new(|k: &mut Kingdom| k.counties[2].army = 3)),
        ("emigrants", Box::new(|k: &mut Kingdom| k.counties[2].emigrants = 3)),
        ("immigrants", Box::new(|k: &mut Kingdom| k.counties[2].immigrants = 3)),
        ("largest_inflow", Box::new(|k: &mut Kingdom| k.counties[2].largest_inflow = 3)),
        ("emigrant_dest", Box::new(|k: &mut Kingdom| k.counties[2].emigrant_destination = 3)),
        ("inflow_source", Box::new(|k: &mut Kingdom| k.counties[2].largest_inflow_source = 3)),
        ("inflow_sources", Box::new(|k: &mut Kingdom| k.counties[2].inflow_sources[7] = 3)),
        ("neighbour_count", Box::new(|k: &mut Kingdom| k.counties[2].neighbour_count = 3)),
        ("neighbours", Box::new(|k: &mut Kingdom| k.counties[2].neighbours[2] = 9)),
        ("change_reason", Box::new(|k: &mut Kingdom| k.counties[2].change_reason = ChangeReason::Immigration)),
        ("pop_band", Box::new(|k: &mut Kingdom| k.counties[2].pop_band = 3)),
        ("anchor_x", Box::new(|k: &mut Kingdom| k.counties[2].anchor_x = 33)),
        ("anchor_y", Box::new(|k: &mut Kingdom| k.counties[2].anchor_y = 33)),
        ("tax_rate", Box::new(|k: &mut Kingdom| k.counties[2].tax_rate = 33)),
        ("tax_collected", Box::new(|k: &mut Kingdom| k.counties[2].tax_collected = 33)),
        ("tax_shown", Box::new(|k: &mut Kingdom| k.counties[2].tax_shown = 33)),
        ("labour", Box::new(|k: &mut Kingdom| k.counties[2].labour[8] = 33)),
        ("field_progress", Box::new(|k: &mut Kingdom| k.counties[2].field_progress[19] = 33)),
        ("ration_achieved", Box::new(|k: &mut Kingdom| k.counties[2].ration_achieved = 1)),
        ("ration_wanted", Box::new(|k: &mut Kingdom| k.counties[2].ration_wanted = 1)),
        ("ration_split", Box::new(|k: &mut Kingdom| k.counties[2].ration_split = 1)),
        ("grain_eaten", Box::new(|k: &mut Kingdom| k.counties[2].grain_eaten = 1)),
        ("herd_eaten", Box::new(|k: &mut Kingdom| k.counties[2].herd_eaten = 1)),
        ("grain_available", Box::new(|k: &mut Kingdom| k.counties[2].grain_available = 1)),
        ("herd_available", Box::new(|k: &mut Kingdom| k.counties[2].herd_available = 1)),
        ("friendly_troops", Box::new(|k: &mut Kingdom| k.counties[2].friendly_troops = 1)),
        ("enemy_troops", Box::new(|k: &mut Kingdom| k.counties[2].enemy_troops = 1)),
        ("castle_type", Box::new(|k: &mut Kingdom| k.counties[2].castle_type = 5)),
        ("castle_building", Box::new(|k: &mut Kingdom| k.counties[2].castle_building = 5)),
        ("castle_degraded", Box::new(|k: &mut Kingdom| k.counties[2].castle_degraded = true)),
        ("castle_progress", Box::new(|k: &mut Kingdom| k.counties[2].castle_progress = 5)),
        ("event_pop_pct", Box::new(|k: &mut Kingdom| k.counties[2].event_population_pct = 5)),
        ("event_grain_pct", Box::new(|k: &mut Kingdom| k.counties[2].event_grain_pct = 5)),
        ("event_herd_pct", Box::new(|k: &mut Kingdom| k.counties[2].event_herd_pct = 5)),
        ("fields_fallow", Box::new(|k: &mut Kingdom| k.counties[2].fields_fallow = 5)),
        ("fields_cattle", Box::new(|k: &mut Kingdom| k.counties[2].fields_cattle = 5)),
        ("fields_grain", Box::new(|k: &mut Kingdom| k.counties[2].fields_grain = 5)),
        ("fertility", Box::new(|k: &mut Kingdom| k.counties[2].fertility = -5)),
        ("weather", Box::new(|k: &mut Kingdom| k.counties[2].weather = Weather::Flooding)),
        ("dryness", Box::new(|k: &mut Kingdom| k.counties[2].dryness = -5)),
        ("grain", Box::new(|k: &mut Kingdom| k.counties[2].grain = 5)),
        ("crop", Box::new(|k: &mut Kingdom| k.counties[2].crop[2] = 5)),
        ("herd", Box::new(|k: &mut Kingdom| k.counties[2].herd = 5)),
        ("herd_crowding", Box::new(|k: &mut Kingdom| k.counties[2].herd_crowding = 30)),
        ("herd births due", Box::new(|k: &mut Kingdom| k.counties[2].herd_births_expected = 5)),
        ("herd deaths due", Box::new(|k: &mut Kingdom| k.counties[2].herd_deaths_expected = 5)),
        ("herd change due", Box::new(|k: &mut Kingdom| k.counties[2].herd_change_expected = -5)),
        ("industry output", Box::new(|k: &mut Kingdom| k.counties[2].industry[3].output = 5)),
        ("industry efficiency", Box::new(|k: &mut Kingdom| k.counties[2].industry[3].efficiency = 5)),
        ("industry capacity", Box::new(|k: &mut Kingdom| k.counties[2].industry[3].capacity = 5)),
        ("industry resource", Box::new(|k: &mut Kingdom| k.counties[2].industry[3].has_resource = true)),
        ("industry enabled", Box::new(|k: &mut Kingdom| k.counties[2].industry[3].enabled = false)),
        ("industry disabled", Box::new(|k: &mut Kingdom| k.counties[2].industry[3].disabled_seasons = 5)),
        ("industry total", Box::new(|k: &mut Kingdom| k.counties[2].industry[3].total = 5)),
        ("weapon_type", Box::new(|k: &mut Kingdom| k.counties[2].weapon_type = 5)),
        ("tax_suppressed", Box::new(|k: &mut Kingdom| k.counties[2].tax_suppressed = true)),
        ("the last county", Box::new(|k: &mut Kingdom| k.counties[16].population = 1)),
        ("realm ai_step", Box::new(|k: &mut Kingdom| k.realms[2].ai_step = 77)),
        ("realm in_play", Box::new(|k: &mut Kingdom| k.realms[2].in_play = false)),
        ("realm strength", Box::new(|k: &mut Kingdom| k.realms[2].strength = 77)),
        ("realm is_human", Box::new(|k: &mut Kingdom| k.realms[2].is_human = true)),
        ("realm lord", Box::new(|k: &mut Kingdom| k.realms[2].lord = 4)),
        ("realm tax_hap", Box::new(|k: &mut Kingdom| k.realms[2].tax_hap_empire = 9)),
        ("realm county_count", Box::new(|k: &mut Kingdom| k.realms[2].county_count = 9)),
        ("realm rank", Box::new(|k: &mut Kingdom| k.realms[2].rank = 3)),
        ("realm score", Box::new(|k: &mut Kingdom| k.realms[2].score = 3)),
        ("realm wages", Box::new(|k: &mut Kingdom| k.realms[2].wages = 3)),
        ("realm gold", Box::new(|k: &mut Kingdom| k.realms[2].gold = 3)),
        ("realm iron", Box::new(|k: &mut Kingdom| k.realms[2].iron = 3)),
        ("realm stone", Box::new(|k: &mut Kingdom| k.realms[2].stone = 3)),
        ("realm wood", Box::new(|k: &mut Kingdom| k.realms[2].wood = 3)),
        ("realm weapons", Box::new(|k: &mut Kingdom| k.realms[2].weapons[5] = 3)),
        ("realm bankrupt", Box::new(|k: &mut Kingdom| k.realms[2].bankrupt_stage = 4)),
        ("realm pop total", Box::new(|k: &mut Kingdom| k.realms[2].population_total = 3)),
        ("realm pop last", Box::new(|k: &mut Kingdom| k.realms[2].population_last = 3)),
        ("realm pop mean", Box::new(|k: &mut Kingdom| k.realms[2].population_mean = 3)),
        ("realm happiness", Box::new(|k: &mut Kingdom| k.realms[2].mean_happiness = 3)),
        ("realm health", Box::new(|k: &mut Kingdom| k.realms[2].mean_health = 3)),
        ("realm share", Box::new(|k: &mut Kingdom| k.realms[2].share_of_map_pct = 3)),
        ("realm armies", Box::new(|k: &mut Kingdom| k.realms[2].army_count = 3)),
        ("realm men", Box::new(|k: &mut Kingdom| k.realms[2].total_men = 3)),
        ("realm score input", Box::new(|k: &mut Kingdom| k.realms[2].score_inputs[5] = 3)),
        ("the last realm", Box::new(|k: &mut Kingdom| k.realms[0].gold = 1)),
        ("history", Box::new(|k: &mut Kingdom| k.history.record(&k.counties.clone()))),
    ];

    for (name, mutate) in &mutations {
        let mut changed = base.clone();
        mutate(&mut changed);
        assert_ne!(changed, base, "the {name} mutation did not change the kingdom");
        assert_ne!(encode(&changed), reference, "changing {name} did not change the bytes");
        // And it survives the trip, which is the other half: reaching the bytes
        // is no use if the reader ignores them.
        assert_eq!(decode(&encode(&changed), Tables::DEFAULT).unwrap(), changed, "{name}");
    }
    assert!(mutations.len() > 110, "the coverage list should not shrink quietly");
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
