#![allow(unused_imports)]
use super::*;
use super::furnished_state::*;
use super::campaign::*;
use super::coverage::*;
use super::header::*;
use super::census_part::*;
use l2_kingdom::county::{ChangeReason, MAX_COUNTIES};
use l2_kingdom::realm::{Pair, MAX_REALMS};
use l2_kingdom::save::{
    checksum, decode, encode, ruleset_fingerprint, LoadError, HEADER_LEN, MAGIC, VERSION,
};
use l2_kingdom::tables::{Tables, Weather, JOB_COUNT};
use l2_kingdom::{Kingdom, Options};

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

#[test]
fn a_kingdom_with_turns_behind_it_round_trips() {
    for seasons in [1usize, 2, 5, 13] {
        let k = played(seasons);
        assert_eq!(k.turn_count as usize, seasons);
        let back = decode(&encode(&k), Tables::DEFAULT).expect("must decode");
        assert_eq!(back, k, "after {seasons} seasons");
    }
}

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

#[test]
fn the_trailer_is_the_state_checksum() {
    let k = played(3);
    let bytes = encode(&k);
    let trailer = u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().unwrap());
    assert_eq!(trailer, checksum(&k));
}


#[test]
fn the_body_covers_a_fixed_and_known_number_of_bytes() {
    let mut c = l2_net::Canonical::hashing();
    l2_net::Encode::encode(&Kingdom::new(1), &mut c);
    // +272 at version 17 for `Industry::next_season`: four bytes on each of the
    // four industry records, over 17 county slots. It is the number the
    // sidebar's industry rows draw — county `+0x2A8 + c*0x18`.
    //
    // +272 at version 20 for last season's weather and event figures —
    // `grain_weather_change`, `grain_event_change`, `herd_weather_change` and
    // `herd_event_change`, four `i32`s over 17 county slots — which the grain and
    // cattle panels print under `L2.eng` group 77.
    //
    // +48 at version 21 for realm `+0xF4`/`+0xF8`, `Realm::tax_ledger` — two
    // `i32` a realm over 6 realm slots. `Tax_CollectAll` credits both beside the
    // treasury and no rule reads either.
    //
    // +68 at version 22 for `County::event_population_swing`, county `+0x2F8`:
    //
    // +6 at version 24 for realm `+0x2A`, `Realm::peak_counties` — one byte a
    // realm over 6 realm slots. `County_ChangeOwner` reads it to choose the
    // capture letter and raises it.
    //
    // +68 at version 25 for `County::fields_grain_standing` (`+0x206`), four
    // bytes over 17 county slots — the divisor of the wheat picture.
    //
    // +272 at version 26 for `Industry::last_efficiency`, county `+0x29C`: four
    // bytes a commodity over four commodities over 17 county slots. It is the
    // ramp's own input, and `Industry_Produce` is the only writer.
    //
    // +34 at version 27 for the two round-robin field cursors, county `+0x15A`
    // and `+0x15B`: one byte each over 17 county slots.
    //
    // +136 at version 30 for the ration shadow pair, county `+0x18C` and
    // `+0x190`: four bytes each over 17 county slots. `Ration_ApplyAll` writes
    // it and the two season ticks spend it.
    //
    // +52 at version 31 for the field-battle playlist: `DAT_0057CAE0`'s 48
    // bytes and `DAT_005653F8`'s four, the pair `FUN_00444A2F` net-syncs.
    //
    // +2 at version 32 for `g_battleApproachLane` and `g_battleRallyGroup`.
    //
    // +40 at version 33 for the marker tables, `DAT_00544050`/`70` and `g_rallyWaypoints`/`DAT_00522D60`.
    assert_eq!(c.finish().len, 67_664, "the state encoding changed - bump VERSION?");
}

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
    for (plane, name) in ["terrain", "flags", "bank", "county"].into_iter().enumerate() {
        for tile in [0usize, 1, l2_kingdom::MAP_TILES / 2, l2_kingdom::MAP_TILES - 1] {
            check(format!("map {name} tile {tile}"), &move |k: &mut Kingdom| {
                let m = &mut k.campaign.map;
                let target = match plane {
                    0 => &mut m.terrain,
                    1 => &mut m.flags,
                    2 => &mut m.bank,
                    _ => &mut m.county,
                };
                target[tile] = target[tile].wrapping_add(1);
            });
        }
    }
    for tile in [0usize, l2_kingdom::MAP_TILES / 2, l2_kingdom::MAP_TILES - 1] {
        for realm in 1..MAX_REALMS as u8 {
            check(format!("seen tile {tile} realm {realm}"), &move |k: &mut Kingdom| {
                let mut e = l2_kingdom::explore::Explored::new();
                e.copy_from_bytes(k.campaign.explored.as_bytes());
                if e.is_seen(realm, tile) {
                    e.clear();
                }
                e.set_seen(realm, tile);
                k.campaign.explored = e;
            });
        }
    }
}

// The two tests below are the mechanism `docs/decisions.md` C30 asked for and
// C39 delivered: the field list is *derived from the struct definitions*

