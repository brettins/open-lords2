//! **Invariants of the save format**, over every save this machine can reach.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" LORDS2_FIXTURES="E:\dev\lords2-fixtures" \
//!     cargo test -p l2-formats --test save
//! ```
//!
//! # Why this file was split
//!
//! It used to hold two different kinds of assertion under one roof, and the
//! difference only became visible when it broke. Half of it read properties that
//! must hold of *any* valid save — the block table accounting for the file
//! exactly, adjacency being symmetric, an owner byte naming a realm that exists.
//! The other half asserted the numbers of one particular saved game: five owned
//! counties, happiness 72 and 77, population 417 everywhere.
//!
//! Both halves pointed at `lastturn.sav` inside the game install, which is the
//! **rolling autosave** — the game rewrites it every turn a human plays. Ten
//! minutes of play replaced it, and nine tests went red at once with bare
//! assertion diffs that read like a broken reader. One of them was
//! `the_neighbour_lists_are_symmetric_and_name_only_real_counties`, whose name
//! promises an invariant; it failed on `assert_eq!(real.len(), 14)`, which is
//! not one. The invariant it is named after held perfectly.
//!
//! So: **scenario values live in `save_england_turn1.rs`**, behind a named
//! fixture with a fingerprint. This file asserts only what is true of every
//! save, and runs over every save it can find — the preserved fixtures in
//! `LORDS2_FIXTURES` *and* whatever volatile saves the install happens to hold.
//! One file agreeing proves nothing about a format (`docs/decisions.md` C1);
//! these run over as many as exist.

use l2_formats::save::{Save, SaveError, COUNTY_RECORDS, NEIGHBOUR_SLOTS, REALM_RECORDS};
use l2_testkit::{executable, saves, skip, SaveFile};

/// The arithmetic that validates the whole schema, and it is a property of the
/// executable rather than of any save: `Save_Write`'s table accounts for
/// 267,028 bytes of live memory, then sixteen 12,800-byte castle blocks, and
/// 267,028 + 16 × 12,800 = 471,828.
///
/// If any block length were misread this would not close, so it is the reason to
/// trust every other number read out of a save.
#[test]
fn the_block_table_accounts_for_a_save_exactly() {
    let exe = l2_testkit::executable!();
    let layout = l2_formats::save::Layout::from_executable(&exe).expect("block table");
    let blocks: u32 = layout.blocks().iter().map(|b| b.len).sum();

    assert_eq!(blocks as usize, 267_028, "the saved regions");
    assert_eq!(layout.expected_len(), 471_828, "plus castles.dat as 16 x 12,800");
    assert_eq!(267_028 + 16 * 12_800, 471_828, "and the arithmetic itself");

    // Blocks are contiguous in the file, in table order. A gap or an overlap
    // would put every addressed read at the wrong offset while still summing
    // to the right total.
    let mut at = 0usize;
    for b in layout.blocks() {
        assert_eq!(b.offset, at, "block at {:#010x} does not follow the previous one", b.va);
        at += b.len as usize;
    }
    assert_eq!(at, 267_028);
}

/// Every save that exists is the size the schema says a save is. Nothing else
/// in this file means anything if this does not hold.
#[test]
fn every_save_is_the_length_the_schema_predicts() {
    let saves = saves!();
    for s in &saves {
        let bytes = std::fs::read(&s.path).expect("re-read");
        assert_eq!(bytes.len(), 471_828, "{}", s.label());
        assert_eq!(s.save.layout().expected_len(), bytes.len(), "{}", s.label());
    }
    eprintln!("save length: {} files agree", saves.len());
}

/// A save whose size does not match the schema is refused rather than read from
/// the wrong offsets. The failure mode this guards against is silent: wrong
/// offsets still yield numbers.
#[test]
fn a_save_of_the_wrong_size_is_refused() {
    let exe = l2_testkit::executable!();
    let saves = saves!();
    for s in &saves {
        let mut bytes = std::fs::read(&s.path).expect("re-read");
        bytes.push(0);
        match Save::open(&exe, &bytes) {
            Err(SaveError::SizeMismatch { expected, actual }) => {
                assert_eq!(expected, 471_828, "{}", s.label());
                assert_eq!(actual, 471_829, "{}", s.label());
            }
            other => panic!("{}: expected a size mismatch, got {other:?}", s.label()),
        }
        bytes.truncate(100);
        assert!(matches!(Save::open(&exe, &bytes), Err(SaveError::SizeMismatch { .. })));
    }
}

/// The array shapes: seventeen county records, six realm records, and record 0
/// of each is a slot rather than a place. True of every save because it is the
/// shape of the game's `.data`, not of a scenario.
#[test]
fn the_arrays_are_the_shape_the_executable_gives_them() {
    let saves = saves!();
    for s in &saves {
        let counties = s.save.counties().expect("counties");
        let realms = s.save.realms().expect("realms");
        assert_eq!(counties.len(), COUNTY_RECORDS, "{}", s.label());
        assert_eq!(counties.len(), 17, "{}", s.label());
        assert_eq!(realms.len(), REALM_RECORDS, "{}", s.label());
        assert_eq!(realms.len(), 6, "{}", s.label());
        assert!(!realms[0].in_play(), "{}: realm 0 is an array slot", s.label());
        assert!(!counties[0].is_county(), "{}: record 0 is an array slot", s.label());
    }
}

/// `g_counties` counts exactly the records that read as counties, and the
/// counties are the low records with the slots above them.
///
/// This is the check that used to be spelled `assert_eq!(real.len(), 14)`. The
/// 14 was one map; the identity is every map.
#[test]
fn the_county_count_is_the_number_of_county_records() {
    let saves = saves!();
    for s in &saves {
        let g = s.save.globals().expect("globals");
        let counties = s.save.counties().expect("counties");
        let real = counties.iter().filter(|c| c.is_county()).count();
        assert_eq!(real as i32, g.county_count, "{}: g_counties", s.label());
        assert!(g.county_count >= 1, "{}: a map with no counties", s.label());
        assert!(g.county_count as usize <= COUNTY_RECORDS - 1, "{}", s.label());

        // Counties occupy records 1..=g_counties and nothing above.
        for c in counties.iter() {
            let expected = c.index >= 1 && c.index as i32 <= g.county_count;
            assert_eq!(c.is_county(), expected, "{}: record {}", s.label(), c.index);
        }
        eprintln!("{}: {} counties", s.label(), g.county_count);
    }
}

/// Owner bytes are in range, name a realm that is in play, and the realms'
/// own county tallies agree with them.
///
/// Two independent fields saying the same thing is the point: `+0x05` of a
/// county and `+0x29` of a realm are written by different code, so agreement is
/// evidence the offsets are right rather than evidence about one scenario.
#[test]
fn owner_bytes_and_realm_tallies_agree_in_every_save() {
    let saves = saves!();
    for s in &saves {
        let counties = s.save.counties().expect("counties");
        let realms = s.save.realms().expect("realms");
        let g = s.save.globals().expect("globals");

        for c in counties.iter().filter(|c| c.is_county()) {
            assert!(
                (c.owner as usize) < REALM_RECORDS,
                "{}: county {} owner {}",
                s.label(),
                c.index,
                c.owner
            );
            if c.is_owned() {
                assert!(
                    realms[c.owner as usize].in_play(),
                    "{}: county {} is owned by realm {}, which is not in play",
                    s.label(),
                    c.index,
                    c.owner
                );
            }
        }
        for r in realms.iter().skip(1) {
            let held =
                counties.iter().filter(|c| c.is_county() && c.owner as usize == r.index).count();
            assert_eq!(
                held, r.county_count as usize,
                "{}: realm {} says it holds {} counties, {held} name it",
                s.label(),
                r.index,
                r.county_count
            );
            if !r.in_play() {
                assert_eq!(held, 0, "{}: realm {} is out of play but holds land", s.label(), r.index);
            }
        }
        assert!(
            (1..REALM_RECORDS as i32).contains(&g.local_player),
            "{}: g_localPlayer {}",
            s.label(),
            g.local_player
        );
        assert!(
            realms[g.local_player as usize].in_play(),
            "{}: g_localPlayer names a realm that is not in play",
            s.label()
        );
    }
}

/// **Adjacency is symmetric.** Every neighbour list names a county that names it
/// back, names no county twice, never names itself, stays inside the map, and
/// leaves its unused slots zeroed.
///
/// This is a property of the data rather than of this reader, so it fails if the
/// ids are being read from the wrong offset — and it holds for every save,
/// which is what its name has always claimed. The version of this test that
/// broke also asserted a fourteen-county map and county 1's single neighbour;
/// those were scenario values wearing an invariant's name, and they now live in
/// `save_england_turn1.rs`.
#[test]
fn neighbour_lists_are_symmetric_in_every_save() {
    let saves = saves!();
    let mut edges = 0usize;
    for s in &saves {
        let g = s.save.globals().expect("globals");
        let counties = s.save.counties().expect("counties");
        let top = g.county_count as usize;

        for c in counties.iter().filter(|c| c.is_county()) {
            assert!(
                c.neighbour_count as usize <= NEIGHBOUR_SLOTS,
                "{}: county {} claims {} neighbours",
                s.label(),
                c.index,
                c.neighbour_count
            );
            assert!(
                !c.neighbours().is_empty(),
                "{}: county {} borders nobody, so the map is not connected",
                s.label(),
                c.index
            );
            let mut seen = Vec::new();
            for &n in c.neighbours() {
                let n = n as usize;
                assert!((1..=top).contains(&n), "{}: county {} names {n}", s.label(), c.index);
                assert_ne!(n, c.index, "{}: county {} borders itself", s.label(), c.index);
                assert!(!seen.contains(&n), "{}: county {} names {n} twice", s.label(), c.index);
                seen.push(n);
                assert!(
                    counties[n].neighbours().contains(&(c.index as u8)),
                    "{}: county {} names {n}, which does not name it back",
                    s.label(),
                    c.index
                );
                edges += 1;
            }
            for &spare in &c.neighbours[c.neighbour_count as usize..] {
                assert_eq!(
                    spare, 0,
                    "{}: county {} has a stale id past its count",
                    s.label(),
                    c.index
                );
            }
        }
        // A symmetric relation is counted twice, so the total is even.
        assert_eq!(edges % 2, 0, "{}: an odd number of directed edges", s.label());
    }
    eprintln!("adjacency: {edges} directed edges symmetric across {} saves", saves.len());
}

/// Every field the reader exposes as a bounded quantity is inside its bound, in
/// every save. A byte read from the wrong offset is far more likely to land
/// outside these than inside them, which is what makes a range check over a
/// whole record worth writing.
#[test]
fn every_bounded_field_is_inside_its_bound_in_every_save() {
    let saves = saves!();
    for s in &saves {
        for c in s.save.counties().expect("counties").iter().filter(|c| c.is_county()) {
            let at = |what: &str| format!("{}: county {} {what}", s.label(), c.index);
            assert!((0..=100).contains(&c.happiness), "{}", at("happiness"));
            assert!((0..=100).contains(&c.happiness_last), "{}", at("happinessLast"));
            assert!((0..=100).contains(&c.health_meter), "{}", at("healthMeter"));
            assert!((0..=4).contains(&c.health_band), "{}", at("healthBand"));
            assert!(c.unrest <= 4, "{}", at("unrest"));
            assert!((0..=5).contains(&c.weather), "{}", at("weather"));
            assert!((0..=5).contains(&c.ration_achieved), "{}", at("rationAchieved"));
            assert!((0..=5).contains(&c.ration_wanted), "{}", at("rationWanted"));
            assert!((0..=100).contains(&c.ration_split), "{}", at("rationSplit"));
            assert!((-100..=100).contains(&c.fertility), "{}", at("fertility"));
            assert!(c.tax_rate <= 100, "{}", at("taxRate"));
            assert!(c.population >= 0, "{}", at("population"));
            assert!(c.pop_last >= 0, "{}", at("popLast"));
            assert!(c.births >= 0 && c.deaths >= 0, "{}", at("births/deaths"));
            assert!(c.grain >= 0 && c.herd >= 0, "{}", at("stores"));
            assert!(c.grain_eaten >= 0 && c.herd_eaten >= 0, "{}", at("eaten"));
            assert!(c.grain_eaten <= c.grain_available, "{}", at("grain eaten beyond store"));
            assert!(c.herd_eaten <= c.herd_available, "{}", at("herd eaten beyond store"));
        }
        let g = s.save.globals().expect("globals");
        assert!((1..=4).contains(&g.season), "{}: g_season {}", s.label(), g.season);
        assert!((1..=4).contains(&g.season_next), "{}", s.label());
        assert!(g.year >= 1268, "{}: g_year {}", s.label(), g.year);
        assert!(g.turn_count >= 0, "{}", s.label());
        assert!((0..=2).contains(&g.opt_difficulty), "{}", s.label());
        assert!(
            (1..=g.county_count).contains(&g.weather_county),
            "{}: g_weatherCounty {} outside 1..={}",
            s.label(),
            g.weather_county,
            g.county_count
        );
    }
}

/// Migration conserves people: across a whole map, everyone who left arrived
/// somewhere. A one-sided migration would be a rule bug in our engine and a
/// misread offset here, and this cannot tell the two apart — which is exactly
/// why it is worth having it fail.
#[test]
fn migration_conserves_people_across_the_whole_map() {
    let saves = saves!();
    for s in &saves {
        let counties = s.save.counties().expect("counties");
        let out: i32 = counties.iter().filter(|c| c.is_county()).map(|c| c.emigrants).sum();
        let inn: i32 = counties.iter().filter(|c| c.is_county()).map(|c| c.immigrants).sum();
        assert_eq!(out, inn, "{}: {out} left and {inn} arrived", s.label());
    }
}

/// Addressed reads are the same reads the record parser makes. `Save::u8_at`
/// and the `County` struct come off the same bytes by two different routes —
/// one through the block table, one through the record stride — and a
/// disagreement means one of the two is wrong.
#[test]
fn the_addressed_read_and_the_record_read_agree() {
    let saves = saves!();
    for s in &saves {
        for c in s.save.counties().expect("counties").iter() {
            let base = l2_formats::save::COUNTY_BASE
                + (c.index * l2_formats::save::COUNTY_STRIDE) as u32;
            assert_eq!(s.save.u8_at(base + 0x05).unwrap(), c.owner, "{}", s.label());
            assert_eq!(s.save.i8_at(base + 0x0C).unwrap(), c.happiness, "{}", s.label());
            assert_eq!(s.save.i32_at(base + 0x24).unwrap(), c.population, "{}", s.label());
            assert_eq!(s.save.i32_at(base + 0x28).unwrap(), c.pop_last, "{}", s.label());
        }
    }
}

/// **A property that looked like an invariant and is not**, recorded here so
/// nobody re-derives it: `population == popLast + births - deaths + immigrants
/// - emigrants` closes on every turn-one save and **fails from turn two on**.
/// `battle-after.sav`'s county 2 reads 588 where the identity predicts 639.
///
/// Something else moves people — army recruitment and battle losses are the
/// obvious candidates and neither is read here yet — so the five fields are not
/// a closed system and asserting that they are would have been C12's shape
/// again: a rule that holds on the one file anybody looked at.
///
/// What *is* asserted is the half that survives contact with six saves: the
/// season's own bookkeeping never runs backwards.
#[test]
fn births_and_deaths_are_bounded_by_the_population_they_moved() {
    let saves = saves!();
    for s in &saves {
        for c in s.save.counties().expect("counties").iter().filter(|c| c.is_county()) {
            assert!(c.deaths <= c.pop_last + c.births, "{}: county {}", s.label(), c.index);
            assert!(c.emigrants <= c.pop_last + c.births, "{}: county {}", s.label(), c.index);
        }
    }
}

/// A save is a pure function of its bytes: opening the same file twice yields
/// the same records. Trivial, and the thing that would catch a reader that
/// grew a cache or a `static mut`.
#[test]
fn opening_the_same_bytes_twice_reads_the_same_save() {
    let Some(exe) = executable() else {
        skip!("no Lords2.exe to read the block table from");
    };
    let saves = saves!();
    for s in &saves {
        let bytes = std::fs::read(&s.path).expect("re-read");
        let again = Save::open(&exe, &bytes).expect("opens");
        assert_eq!(again.counties().unwrap(), s.save.counties().unwrap(), "{}", s.label());
        assert_eq!(again.globals().unwrap(), s.save.globals().unwrap(), "{}", s.label());
    }
}

/// A cheap census of what the machine actually offered, so a run that asserted
/// almost nothing says so out loud instead of printing thirteen `ok`s.
#[test]
fn the_suite_reports_which_saves_it_ran_over() {
    let found: Vec<SaveFile> = l2_testkit::every_available_save();
    if found.is_empty() {
        skip!("no saves reachable; every invariant in this file asserted nothing");
    }
    for s in &found {
        let g = s.save.globals().unwrap();
        eprintln!(
            "  {} - {} counties, turn {}, season {}, year {}",
            s.label(),
            g.county_count,
            g.turn_count,
            g.season,
            g.year
        );
    }
    assert!(found.iter().any(|s| s.origin == l2_testkit::Origin::Fixture)
        || found.iter().any(|s| s.origin == l2_testkit::Origin::Install));
}

// --- the unit array ---------------------------------------------------------
//
// `g_units` is one array holding four kinds of thing, and everything below is
// true of all four. The scenario-specific half — that the England turn-one
// position holds six merchants and nothing else — lives in
// `save_england_turn1.rs`, behind the fingerprinted fixture, for the reason the
// module documentation gives.

/// **The self-checking invariant of the whole record.**
///
/// `+0x0C` is `(y * 64 + x) * 8`, which is redundant with `+0x0A`/`+0x0B` — so
/// the only way it can agree on every occupied slot of every save is if the base
/// and the `0x1A4` stride are both right. A wrong stride would shift the pair
/// and the offset by different amounts and the equality would fail on the first
/// unit of the first file.
#[test]
fn every_units_tile_offset_agrees_with_its_coordinates_in_every_save() {
    let saves = saves!();
    let mut checked = 0;
    for s in &saves {
        for u in s.save.units().unwrap().iter().filter(|u| u.is_live()) {
            assert!(
                u.tile_offset_agrees(),
                "{}: unit {} is at ({}, {}) but +0x0C is {:#x}",
                s.label(),
                u.index,
                u.x,
                u.y,
                u.tile_offset
            );
            checked += 1;
        }
    }
    eprintln!("tile offsets agreed on {checked} units across {} saves", saves.len());
    assert!(checked > 0, "no save offered a single unit");
}

/// Every live unit names one of the four handlers in `g_unitTickTable`, is owned
/// by a realm or by nobody, and stands on the map.
///
/// Slot 5 of that table is NULL while the dispatcher accepts types up to 5, so a
/// type-5 unit would call address 0. Nothing spawns one, and this says so over
/// every save rather than only over the one somebody looked at.
#[test]
fn every_live_unit_is_one_of_the_four_types_and_owned_by_somebody() {
    let saves = saves!();
    for s in &saves {
        let county_count = s.save.globals().unwrap().county_count;
        for u in s.save.units().unwrap().iter().filter(|u| u.is_live()) {
            let at = format!("{} unit {}", s.label(), u.index);
            assert!((1..=4).contains(&u.kind), "{at}: type byte {}", u.kind);
            // 1..=5 are realms; 6 is nobody, which merchants and a county's own
            // levied defence carry.
            assert!((1..=6).contains(&u.owner), "{at}: owner {}", u.owner);
            assert!(u.x < 64 && u.y < 64, "{at}: at ({}, {})", u.x, u.y);
            assert!(u.path_len as usize <= l2_formats::save::UNIT_PATH_STEPS, "{at}: path");
            assert!(u.county as i32 <= county_count, "{at}: county {}", u.county);
            assert!(u.dest_county as i32 <= county_count, "{at}: dest {}", u.dest_county);
            assert!(u.garrison_county as i32 <= county_count, "{at}: garrison");
            assert!(u.besieging_county as i32 <= county_count, "{at}: besieging");
            assert!((0..=5).contains(&u.starvation), "{at}: starvation {}", u.starvation);
        }
    }
}

/// **Two counts of the same thing.** `g_merchantCount` records how many
/// merchants `Merchant_SpawnAll` placed; counting type-3 units in the array is
/// the other way to ask, and the two were written by different code at different
/// times.
///
/// It also pins the coupling `Merchant_AdvanceAll` depends on: the merchants
/// occupy slots **1 … n**, contiguously and from 1, because they are spawned
/// before anything else exists. That routine indexes the route table by *slot
/// minus one*, so a merchant anywhere else would walk somebody else's route.
#[test]
fn the_merchants_are_the_first_slots_and_there_are_as_many_as_the_counter_says() {
    let saves = saves!();
    for s in &saves {
        let counted = s.save.globals().unwrap().merchant_count;
        let units = s.save.units().unwrap();
        let merchants: Vec<usize> =
            units.iter().filter(|u| u.is_live() && u.kind == 3).map(|u| u.index).collect();
        assert_eq!(merchants.len() as i32, counted, "{}: g_merchantCount", s.label());
        let expected: Vec<usize> = (1..=merchants.len()).collect();
        assert_eq!(merchants, expected, "{}: merchants are not slots 1..n", s.label());
    }
}

/// The route table is what plane 4 of the map's castle tiles builds, and three
/// of its properties hold over every shipped map — `docs/formats/plane4.md` §1.1
/// proves them from `L2_maps.dat`; this proves them again out of the **saves**,
/// which is a different file written by a different piece of code.
#[test]
fn the_merchant_route_table_is_well_formed_in_every_save() {
    let saves = saves!();
    for s in &saves {
        let county_count = s.save.globals().unwrap().county_count as u8;
        let rows = s.save.merchant_routes().unwrap();
        let start = s.save.merchant_start_counties().unwrap();
        for (r, row) in rows.iter().enumerate() {
            let live: Vec<u8> = row.iter().copied().take_while(|&c| c != 0).collect();
            let at = format!("{} route {}", s.label(), r + 1);
            assert_eq!(
                row[live.len()..].iter().filter(|&&c| c != 0).count(),
                0,
                "{at}: a zero with counties after it"
            );
            for &c in &live {
                assert!(c <= county_count, "{at}: county {c}, and the map has {county_count}");
            }
            let mut sorted = live.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), live.len(), "{at}: a county appears twice");
        }
        // A start county is either zero or a real county, and — this is the
        // half `Merchant_SpawnAll`'s `break` makes load-bearing — the non-zero
        // ones come first, so counting up to the first zero is counting the
        // merchants.
        let spawned = start.iter().take_while(|&&c| c != 0).count();
        for &c in &start[..spawned] {
            assert!(c <= county_count, "{}: start county {c}", s.label());
        }
        assert_eq!(
            spawned as i32,
            s.save.globals().unwrap().merchant_count,
            "{}: a merchant per start county before the first zero",
            s.label()
        );
    }
}

/// **`+0x167` on a merchant is where it was born, not where it is.**
///
/// `Merchant_SpawnAll` writes `g_merchantStartCounty[i]` into it once and
/// nothing updates it afterwards, so merchant *n* carries start county *n* for
/// the life of the game while its `+0x10` follows it around the map. Worth an
/// assertion because the same byte is an army's county-defence mark, and reading
/// a merchant's birthplace as a defence mark would make every merchant look like
/// a levy.
#[test]
fn a_merchants_0x167_is_its_start_county_however_far_it_has_walked() {
    let saves = saves!();
    let mut moved = 0;
    for s in &saves {
        let start = s.save.merchant_start_counties().unwrap();
        for u in s.save.units().unwrap().iter().filter(|u| u.is_live() && u.kind == 3) {
            assert_eq!(
                u.role,
                start[u.index - 1],
                "{}: merchant {} carries {} and started in {}",
                s.label(),
                u.index,
                u.role,
                start[u.index - 1]
            );
            if u.county != u.role {
                moved += 1;
            }
        }
    }
    // The claim is only worth making because it is *observably* not the
    // county the merchant is standing in.
    assert!(moved > 0, "no merchant in any save had left its start county");
    eprintln!("{moved} merchants stood outside the county +0x167 names");
}

/// **The three minimap rating bytes, `+0x01`, `+0x02` and `+0x03`, named from
/// the saved games rather than from a decompiler.**
///
/// `FUN_00451BBA` recomputes them on every minimap draw and `Minimap_DrawOverlay`
/// reads them. `docs/screens.md` §3.2 used to call them `+0x0B1`, `+0x0B2` and
/// `+0x0B3`, which is the literal `0x0053F9B3` in the disassembly mistaken for an
/// offset; they are three of the five bytes `Sync_CompareState` skips, so they
/// are interface state and a save holds whatever the last draw left there.
///
/// Three claims, each falsifiable against the user's own saves:
///
/// * **`+0x02` is the food rating and it is binary** — 0 when the county did not
///   achieve the ration it was asked for and 6, off the end of the six-entry
///   ramp, when it did. Never anything between.
/// * **`+0x03` is the labour rating and it has three values** — 0 short of farm
///   workers, 5 carrying slack, 6 neither.
/// * **`+0x01` is `happiness / 20`.** Checked only in saves where the bands have
///   been computed at all: a game whose minimap overlay was never opened has all
///   three bytes zero, which the England turn-one fixture is.
#[test]
fn the_minimap_rating_bytes_are_food_labour_and_happiness_over_twenty() {
    use l2_formats::save::{COUNTY_BASE, COUNTY_STRIDE};
    let saves = saves!();
    let mut checked = 0;
    let mut computed = 0;
    for s in &saves {
        let mut any = false;
        let mut happiness_bands = Vec::new();
        for i in 0..COUNTY_RECORDS {
            let county = s.save.county(i).unwrap();
            if !county.is_county() {
                continue;
            }
            let base = COUNTY_BASE + (i * COUNTY_STRIDE) as u32;
            let (b1, b2, b3) = (
                s.save.u8_at(base + 1).unwrap(),
                s.save.u8_at(base + 2).unwrap(),
                s.save.u8_at(base + 3).unwrap(),
            );
            assert!(
                matches!(b2, 0 | 6),
                "{}: county {i} food band {b2} is neither 0 nor 6",
                s.label()
            );
            assert!(
                matches!(b3, 0 | 5 | 6),
                "{}: county {i} labour band {b3} is not 0, 5 or 6",
                s.label()
            );
            any |= b1 != 0 || b2 != 0 || b3 != 0;
            happiness_bands.push((i, b1, county.happiness));
            checked += 1;
        }
        if !any {
            // Nothing has ever drawn this game's minimap overlay.
            continue;
        }
        computed += 1;
        for (i, band, happiness) in happiness_bands {
            assert_eq!(
                band as i32,
                happiness as i32 / 20,
                "{}: county {i} band {band} against happiness {happiness}",
                s.label()
            );
        }
    }
    assert!(computed >= 2, "at least two saves must have the bands computed");
    eprintln!("minimap bands: {checked} counties, {computed} saves with the bands computed");
}
