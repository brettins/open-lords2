#![allow(unused_imports)]
use super::*;
use super::economy::*;
use super::counties::*;
use super::victory::*;
use l2_kingdom::county::County;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::realm::Realm;
use l2_kingdom::tables::{Season, Tables, Weather};
use l2_kingdom::{Quirk, Quirks};

/// **The quirk set is inside the per-tick lockstep digest.**
///
/// `l2_kingdom::save::checksum` is `Canonical::hash_of(kingdom)` and is the
/// number two peers exchange every tick. If a quirk did not reach it, two
/// players with different settings would agree on every checksum they exchanged
/// while computing different games — which is the exact defect this engine
/// exists to replace, arrived at from the other direction. `docs/netcode.md` §6.
///
/// Asserted for **every** quirk, one at a time, because a bitfield that lost one
/// bit on the way out would still change the hash for the other thirteen.
#[test]
fn the_quirk_set_is_inside_the_lockstep_digest() {
    let base = furnished_kingdom(21);
    let faithful = l2_kingdom::save::checksum(&base);
    for q in Quirk::ALL {
        let mut k = base.clone();
        k.options.quirks.set_reproduced(*q, false);
        assert_ne!(
            l2_kingdom::save::checksum(&k),
            faithful,
            "flipping {} does not change the tick checksum — two peers that disagreed about it \
             would agree on every number they exchanged",
            q.name()
        );
    }
}

/// The `options` section is where it lands, so a desync dump names the right
/// subsystem.
#[test]
fn a_quirk_difference_shows_up_as_the_options_section() {
    let base = furnished_kingdom(22);
    let mut other = base.clone();
    other.options.quirks.set_all(false);

    let digest = |k: &Kingdom| {
        let mut c = l2_net::Canonical::hashing();
        l2_net::Encode::encode(k, &mut c);
        c.finish()
    };
    assert_eq!(
        digest(&base).sole_difference(&digest(&other)),
        Some("options"),
        "a quirk change must localise to `options` and nowhere else"
    );
}

/// **A saved game remembers which bugs it was played with.**
///
/// Not a formality: a save that dropped the field would reload a fixed game as a
/// faithful one, and the county figures would start drifting from the ones the
/// player left. `docs/decisions.md` C30's shape, for the sixth time.
#[test]
fn a_saved_game_remembers_which_bugs_it_was_played_with() {
    for quirks in [Quirks::FAITHFUL, Quirks::FIXED, mixed()] {
        let mut k = furnished_kingdom(23);
        k.options.quirks = quirks;
        let bytes = l2_kingdom::save::encode(&k);
        let back = l2_kingdom::save::decode(&bytes, Tables::DEFAULT).expect("round trip");
        assert_eq!(back.options.quirks, quirks);
        assert_eq!(back.options.quirks.group(), quirks.group());
    }
}

/// **Two kingdoms that differ only in their quirks really do play differently.**
///
/// The end-to-end claim, made once over the whole season pipeline
/// per rule: run the same world forward under both settings and the states
/// diverge. A switch that only changed a flag would pass every test above that
/// calls one rule directly and fail this one.
#[test]
fn the_same_world_played_faithfully_and_fixed_ends_up_in_two_different_states() {
    let mut faithful = furnished_kingdom(24);
    let mut fixed = faithful.clone();
    fixed.options.quirks.set_all(false);

    for _ in 0..8 {
        faithful.advance_season();
        fixed.advance_season();
    }
    assert_ne!(
        l2_kingdom::save::checksum(&faithful),
        l2_kingdom::save::checksum(&fixed),
        "two years of the same world under both settings must not land in the same state"
    );
}

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A quirk set with some of each, so the tri-state parent's middle is exercised
/// by the save round trip too.
fn mixed() -> Quirks {
    let mut q = Quirks::FAITHFUL;
    q.set_reproduced(Quirk::HarvestIgnoresLabourCap, false);
    q.set_reproduced(Quirk::MutualDestructionIsAWin, false);
    assert_eq!(q.group(), l2_net::Group::Mixed);
    q
}

/// A small kingdom with enough in it that a season does something.
///
/// **Nothing here asserts on a field this function writes.** It sets the world
/// up and the tests read what `advance_season` and the rule functions leave
/// behind — `docs/agents.md`'s rule about fixtures that check themselves.
fn furnished_kingdom(seed: u64) -> Kingdom {
    let mut k = Kingdom::new(seed);
    k.county_count = 8;
    k.year = 1300;
    k.year_next = 1301;
    k.season = 1;
    k.season_next = 2;
    for id in 1..=k.county_count {
        let c = &mut k.counties[id];
        c.owner = if id <= 4 { 1 } else { 2 };
        c.population = 400 + id as i32 * 10;
        c.happiness = 50;
        c.health_meter = 60;
        c.health_band = 2;
        c.grain = 2000;
        c.herd = 60;
        c.fields_grain = 6;
        c.fields_fallow = 2;
        c.tax_rate = 10;
        c.weather = Weather::Cloudy;
        c.labour[T.job.grain_farming] = 100;
        c.purse = 500;
    }
    for id in 1..=2 {
        let r = &mut k.realms[id];
        r.in_play = true;
        r.strength = 12;
        r.gold = 5000;
        r.lord = if id == 1 { 0 } else { 1 };
        r.is_human = id == 1;
    }
    k
}

