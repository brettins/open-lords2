//! **The two constructors, held against each other.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" \
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-scenario --test newgame
//! ```
//!
//! `Scenario::from_save` reads a world the original built. `Scenario::from_map`
//! builds one. They are two independent readings of the *same* England, out of
//! two files that were authored separately — `L2_maps.dat` and a save — so
//! where they disagree, one of them is wrong; and where they agree by both
//! being silent, **both** may be wrong.
//!
//! That last case is the one this file exists for. `docs/agents.md`:
//!
//! > A field is only tested if something a test reads was written by something
//! > the game runs.
//!
//! A second constructor is the strongest available witness for the first, and
//! the trap it walks straight into is agreement-by-omission —
//! `CLAUDE.md`'s *"two of our own implementations agreeing proves only that we
//! ported our own misunderstanding faithfully"*, one level down. So the diff
//! below reports three verdicts, not two: **agree**, **differ, because…**, and
//! **both silent**, which is a finding rather than a pass. And
//! [`every_county_field_is_accounted_for`] reads the field list of
//! `CountyState` **out of the source**, so a field added tomorrow fails this
//! test until somebody says which verdict it takes.

use l2_formats::maps::{MapSet, Plane, PLANE_DIM, SLOT_LEN};
use l2_kingdom::map::MAP_TILES;
use l2_scenario::newgame::{self, MapError, NewGame};
use l2_scenario::{CountyState, Scenario};

/// `L2_maps.dat`, or skip. Its own gate rather than `l2_testkit::install!()`
/// because a partial install can have the executable and not the maps.
macro_rules! maps {
    () => {
        match l2_testkit::read_install("L2_maps.dat") {
            Some(b) => b,
            None => l2_testkit::skip!("no L2_maps.dat in the install"),
        }
    };
}

/// Slot 0 is England — `L2.eng` group 101 names it, and rendering it produces
/// England and Wales (`docs/formats/maps-layers.md` §0).
const ENGLAND: usize = 0;

/// The five England start counties. `docs/decisions.md` C23: the **set** is
/// fixed by the map file — the five castle blocks carrying a plane-4 marker —
/// and only the realm→county assignment is rolled, by `FUN_00497E65`.
const ENGLAND_STARTS: [u8; 5] = [1, 4, 8, 11, 13];

/// The seed both kingdoms are built on. Nothing in either construction path
/// draws from it except the start-table deal, which is compared by its result.
const SEED: u64 = 7;

// ------------------------------------------------------- the whole map corpus

/// **Every shipped map builds a world**, and each one closes on itself.
///
/// Running all 44 rather than England alone is `docs/plan.md`'s C26: a rule can
/// be wrong at 45 of its 51 inputs and stay invisible behind a fixture that
/// exercises one. Every claim here is a per-map invariant, so a map that breaks
/// one names itself.
#[test]
fn every_shipped_map_builds_a_world_that_closes_on_itself() {
    let bytes = maps!();
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    let used = set.used_slots();
    assert!(!used.is_empty(), "the shipped file has used slots");
    let mut checked = 0;
    let mut counties_seen = 0;
    let mut smithless = Vec::new();
    for slot_index in used {
        let slot = set.slot(slot_index).expect("a used slot");
        let setup = NewGame { slot: slot_index, lords: 1, ..NewGame::default() };
        let w = newgame::build(&slot, &setup)
            .unwrap_or_else(|e| panic!("slot {slot_index} does not build: {e}"));
        let label = format!("slot {slot_index}");

        assert_eq!(w.county_count, slot.county_count(), "{label}: county count");
        assert!(w.county_count >= 4, "{label}: every shipped map has at least four counties");

        for c in 1..=w.county_count {
            counties_seen += 1;
            assert_ne!(w.town_tile[c], 0, "{label} county {c}: no town tile");
            assert_ne!(w.castle_tile[c], 0, "{label} county {c}: no castle plot");
            assert_eq!(
                w.tiles.content[w.castle_tile[c]],
                0x14,
                "{label} county {c}: the castle plot is not stamped"
            );
            assert!(!w.neighbours[c].is_empty(), "{label} county {c}: an island county");
            if !w.has_resource[c][2] {
                smithless.push((slot_index, c));
            }
            // Exactly four `0x10` plots, which is what stops the dwelling array
            // running into `fieldProgress` — `maps-layers.md` §2.3 measured it
            // over the file and this measures it after the load.
            let plots = (0..MAP_TILES)
                .filter(|&t| w.tiles.county[t] as usize == c && w.tiles.flags[t] & 0x10 != 0)
                .count();
            assert_eq!(plots, 4, "{label} county {c}: {plots} dwelling plots");
            // The twenty-slot table is a bound, and the razing is what keeps it
            // one: after the load no county may have a farm tile with no slot.
            let fields = w.field_tiles[c].iter().filter(|&&t| t != 0).count();
            let farm_tiles = (0..MAP_TILES)
                .filter(|&t| w.tiles.county[t] as usize == c && w.tiles.flags[t] & 0x20 != 0)
                .count();
            assert_eq!(fields, farm_tiles, "{label} county {c}: fields vs surviving farm tiles");
            assert!(fields <= 20, "{label} county {c}: {fields} fields");
        }

        // The player-start table and `l2-formats`' own count of it agree — the
        // one counts writes and the other counts distinct markers, and no
        // shipped map repeats a marker.
        let starts: Vec<u8> = (1..6).map(|m| w.player_start[m]).filter(|&c| c != 0).collect();
        assert_eq!(
            w.player_start_count,
            slot.player_start_count(),
            "{label}: the counter and the table disagree, so a marker is repeated"
        );
        assert_eq!(starts.len(), w.player_start_count, "{label}: start table length");
        assert!(matches!(starts.len(), 2 | 4 | 5), "{label}: {} seats", starts.len());
        for (i, &c) in starts.iter().enumerate() {
            assert!(c as usize <= w.county_count, "{label}: start {i} names county {c}");
        }
        for row in 0..6 {
            for &c in w.routes.row(row).iter().filter(|&&c| c != 0) {
                assert!(c as usize <= w.county_count, "{label} route {row}: county {c}");
            }
        }
        checked += 1;
    }
    eprintln!("{checked} shipped maps built, {counties_seen} counties");
    assert_eq!(checked, 44, "the shipped file has 44 used slots");
    assert_eq!(counties_seen, 434, "434 counties over the 44 used slots — maps.md §2");

    // **A correction, and it is worth more than the assertion it replaces.**
    // `docs/symbols.json` says of `County_PlaceBlacksmith` that the weapons
    // site "is derived rather than authored, which is why every county has
    // one". 433 of 434 do. County 4 of slot 8 (Africa) has **no tile whose
    // flags byte is zero at all** — its 57 tiles are all road, boundary, rough,
    // plot, farmland, town or site — and the candidate test is `flags == 0`
    // exactly, so the original's `local_14` stays 0 and its guard refuses.
    // That county can never make a weapon.
    assert_eq!(smithless, vec![(8, 4)], "the set of counties with no blacksmith has changed");
}

/// **`PlayerStart_Compact`'s bubble is a slice, and this is why.**
///
/// The original drops table entries whose slot number is above the live realm
/// count by bubbling them down. `newgame`'s version takes the first `lords`
/// entries instead. The two agree exactly when every populated entry's slot
/// number is its own index and the populated entries are contiguous from 1 —
/// which is a property of the **maps**, and is what is checked here.
#[test]
fn the_player_start_markers_are_contiguous_from_one_on_every_shipped_map() {
    let bytes = maps!();
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    for slot_index in set.used_slots() {
        let slot = set.slot(slot_index).expect("a used slot");
        let mut markers = Vec::new();
        for y in 0..PLANE_DIM {
            for x in 0..PLANE_DIM {
                let m = slot.at(Plane::Marker, x, y);
                // The `0x80` arm — the castle — is the player start.
                if m != 0 && slot.flags_at(x, y) & 0x80 != 0 {
                    markers.push(m);
                }
            }
        }
        markers.sort_unstable();
        let n = markers.len();
        assert_eq!(
            markers,
            (1..=n as u8).collect::<Vec<u8>>(),
            "slot {slot_index}: the start markers are not 1..{n}"
        );
    }
}

/// A map that seats two cannot be started with five lords, and the refusal is a
/// refusal rather than a silently smaller game.
#[test]
fn a_two_seat_map_refuses_five_lords() {
    let bytes = maps!();
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    let two_seaters: Vec<usize> = set
        .used_slots()
        .into_iter()
        .filter(|&i| set.slot(i).unwrap().player_start_count() == 2)
        .collect();
    assert_eq!(two_seaters.len(), 8, "eight shipped maps seat two — maps-layers.md §5.1");
    let slot_index = two_seaters[0];
    let slot = set.slot(slot_index).expect("a used slot");
    let setup = NewGame { slot: slot_index, lords: 5, ..NewGame::default() };
    assert_eq!(
        Scenario::from_map(&slot, &setup),
        Err(MapError::TooManyLords { lords: 5, seats: 2 })
    );
}

/// **Which realm you play is rolled, and the seed is what rolls it.**
#[test]
fn the_start_table_is_dealt_and_the_deal_follows_the_seed() {
    let bytes = maps!();
    let set = MapSet::parse(&bytes).expect("L2_maps.dat parses");
    let slot = set.slot(ENGLAND).expect("slot 0");
    let owner_of = |seed: u64| {
        let setup = NewGame { slot: ENGLAND, lords: 5, local_player: 1, seed, ..NewGame::default() };
        let s = Scenario::from_map(&slot, &setup).expect("England builds");
        let mut v: Vec<(u8, u8)> = (1..=s.county_count)
            .filter_map(|id| s.counties[id].as_ref().map(|c| (id as u8, c.owner)))
            .filter(|&(_, o)| o != 0)
            .collect();
        v.sort_unstable();
        v
    };
    let a = owner_of(1);
    // The same seed twice is the same world — a lockstep peer's whole
    // requirement of this constructor.
    assert_eq!(a, owner_of(1), "the deal is a function of the seed");
    // The set of owned counties never moves; only who owns them.
    for seed in 0..24u64 {
        let deal = owner_of(seed);
        let counties: Vec<u8> = deal.iter().map(|&(c, _)| c).collect();
        assert_eq!(counties, ENGLAND_STARTS, "seed {seed}: the start counties moved");
        let mut realms: Vec<u8> = deal.iter().map(|&(_, r)| r).collect();
        realms.sort_unstable();
        assert_eq!(realms, vec![1, 2, 3, 4, 5], "seed {seed}: one county each");
    }
    let deals: std::collections::BTreeSet<Vec<(u8, u8)>> =
        (0..64u64).map(owner_of).collect();
    assert!(deals.len() > 1, "the deal never moves, so it is not a deal");
}

// ------------------------------------------------------------------ the diff

/// One field's verdict when the two constructors are asked the same question.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// The two paths produce the same value, and it is not the type's default —
    /// so something wrote it on both sides.
    Agree,
    /// They differ, and the reason is named.
    Differ(&'static str),
    /// **They agree on the type's default**, which is not evidence of anything.
    BothSilent(&'static str),
}

/// A difference nobody has explained. Written as a reason so that an
/// unlabelled call is impossible to miss in the report.
const UNEXPLAINED: &str = "UNEXPLAINED";

fn england_pair(save: &l2_formats::save::Save, maps: &[u8]) -> (Scenario, Scenario) {
    let from_save = Scenario::from_save(save).expect("the fixture imports");
    let set = MapSet::parse(maps).expect("L2_maps.dat parses");
    let slot = set.slot(ENGLAND).expect("slot 0");
    // The map path is told the **same** rule options the save recorded — the
    // difficulty in particular, which is the whole of the setting's effect on
    // the land — so the only thing left to disagree about is the world.
    let setup = NewGame {
        slot: ENGLAND,
        lords: 5,
        local_player: from_save.local_player,
        seed: SEED,
        options: from_save.options,
    };
    let from_map = Scenario::from_map(&slot, &setup).expect("England builds");
    (from_save, from_map)
}

/// **England out of `L2_maps.dat` and England out of `lastturn.sav`, field by
/// field.**
///
/// Two files authored separately, one reading each. Almost every legitimate
/// difference is of one kind: **the save is one season further on**, because
/// `Game_NewGame` runs an immediate `Season_Advance` and the autosave is
/// written after it. The exceptions are named individually.
#[test]
fn england_from_the_map_and_england_from_the_save_agree_field_by_field() {
    let save = l2_testkit::england!();
    let maps = maps!();
    let (a, b) = england_pair(&save, &maps);

    assert_eq!(a.county_count, b.county_count, "county count");
    assert_eq!(a.county_count, 14);
    assert_eq!(a.local_player, b.local_player);

    // ------------------------------------------------------- the three planes

    // The county plane is copied verbatim by both paths and nothing rewrites
    // it, so this one is exact over all 4,096 tiles.
    assert_eq!(a.map.county, b.map.county, "the county plane");

    // The flags plane differs on exactly the dwelling plots. Both paths leave
    // bit `0x10` set at load; `County_UpdateDwellings` clears it on every plot
    // beyond what the population supports, and England's turn-one population of
    // 417 supports **none** — so all fifty-six plots lose the bit in the save
    // and keep it here. `maps-layers.md` §5.4 counted the same difference from
    // the other side: 55 tiles `0x10 -> 0x00` and one `0x12 -> 0x02`.
    let flag_diffs: Vec<usize> =
        (0..MAP_TILES).filter(|&t| a.map.flags[t] != b.map.flags[t]).collect();
    for &t in &flag_diffs {
        assert_eq!(
            a.map.flags[t] | 0x10,
            b.map.flags[t],
            "tile {t}: flags differ by something other than the dwelling bit \
             ({:#04x} in the save, {:#04x} from the map)",
            a.map.flags[t],
            b.map.flags[t]
        );
    }
    assert_eq!(flag_diffs.len(), 14 * 4, "one dwelling-plot bit per plot, and nothing else");

    // **The blacksmith is a load-time edit that is in both.** It is the only
    // place either path adds `0x80` to a tile the file gave no flags at all,
    // and it lands on the same tile in all fourteen counties — which is what
    // says the nearest-plain-tile-to-a-field rule is read right.
    let file_set = MapSet::parse(&maps).unwrap();
    let file = file_set.slot(ENGLAND).unwrap();
    let smiths: Vec<usize> = (0..MAP_TILES)
        .filter(|&t| {
            let (x, y) = (t % PLANE_DIM, t / PLANE_DIM);
            file.flags_at(x, y) == 0 && b.map.flags[t] & 0x80 != 0
        })
        .collect();
    assert_eq!(smiths.len(), 14, "one blacksmith per county");
    for &t in &smiths {
        assert_eq!(a.map.flags[t] & 0x80, 0x80, "tile {t}: the save has no blacksmith here");
        assert_eq!(b.map.terrain[t], 7, "tile {t}: the blacksmith's terrain");
        assert_eq!(a.map.terrain[t], 7, "tile {t}: the save's blacksmith terrain");
    }

    // **The terrain plane, category by category.** This is the strongest single
    // assertion in the file: 4,096 tiles, 67 differences, and every one of them
    // in a class that names the pass that made it.
    let mut terrain_classes: std::collections::BTreeMap<(u8, u8, u8), usize> =
        std::collections::BTreeMap::new();
    for t in 0..MAP_TILES {
        if a.map.terrain[t] != b.map.terrain[t] {
            *terrain_classes.entry((a.map.terrain[t], b.map.terrain[t], a.map.flags[t])).or_default() +=
                1;
        }
    }
    let classes: Vec<((u8, u8, u8), usize)> = terrain_classes.into_iter().collect();
    assert_eq!(
        classes,
        vec![
            // A farm tile the season sowed: the map path laid pasture (0x14)
            // and the county's lord ploughed it.
            ((1, 20, 0x20), 7),
            // Five wood industries stepped from idle (10) to working (11) —
            // exactly the five owned counties, and exactly the five records
            // `Game_SetupRealmsAndCounties` switched on.
            ((11, 10, 0x80), 5),
            // Two fallow tiles the lord turned to pasture, and grown a stage.
            ((21, 1, 0x20), 2),
            // Pasture, one growth stage on: 0x14 -> 0x15.
            ((21, 20, 0x20), 32),
            ((22, 20, 0x20), 1),
            // **The castles.** Five 2x2 blocks stamped from the bare plot
            // (0x14) to a standing keep (0x17 = 23) by `FUN_0046826C`, which is
            // keyed on the castle level rather than on the map and is
            // deliberately not this constructor's — see `newgame`'s module
            // documentation.
            ((23, 20, 0x80), 20),
        ],
        "the terrain plane differs in a way nothing accounts for"
    );
    // …and the twenty castle tiles are the five start counties' castle blocks.
    let castle_diffs: Vec<u8> = (0..MAP_TILES)
        .filter(|&t| a.map.terrain[t] == 0x17 && b.map.terrain[t] == 0x14)
        .map(|t| a.map.county[t])
        .collect();
    let mut owners: Vec<u8> = castle_diffs.clone();
    owners.sort_unstable();
    owners.dedup();
    assert_eq!(owners, ENGLAND_STARTS, "the built castles are not the five start counties");
    assert_eq!(castle_diffs.len(), 20, "four tiles each");

    // ------------------------------------------------- per-county, exactly

    for id in 1..=a.county_count {
        let s = a.counties[id].as_ref().expect("the save has this county");
        let m = b.counties[id].as_ref().expect("the map has this county");
        assert_eq!(s.anchor, m.anchor, "county {id}: the town anchor");
        assert_eq!(s.neighbours, m.neighbours, "county {id}: the adjacency list");
        assert_eq!(s.field_tiles, m.field_tiles, "county {id}: the twenty field tiles");
        assert!(m.field_tiles.iter().any(|&t| t != 0), "county {id}: no fields at all");
        for c in 0..4 {
            assert_eq!(
                s.industry[c].has_resource, m.industry[c].has_resource,
                "county {id} industry {c}: the resource byte"
            );
            assert_eq!(
                s.industry[c].enabled, m.industry[c].enabled,
                "county {id} industry {c}: the switch"
            );
        }
        assert_eq!(s.castle_switch, m.castle_switch, "county {id}: the castle-building switch");
    }

    // The five owned counties are the map's five start markers, whatever the
    // roll made of the assignment.
    let owned = |sc: &Scenario| {
        let mut v: Vec<u8> = (1..=sc.county_count)
            .filter(|&id| sc.counties[id].as_ref().map(|c| c.owner).unwrap_or(0) != 0)
            .map(|id| id as u8)
            .collect();
        v.sort_unstable();
        v
    };
    assert_eq!(owned(&a), ENGLAND_STARTS, "the save's five owned counties");
    assert_eq!(owned(&b), ENGLAND_STARTS, "the map's five owned counties");

    // The merchants: six routes, six start counties, six units — both ways.
    assert_eq!(a.routes, b.routes, "the six merchant routes");
    assert_eq!(a.merchant_start, b.merchant_start, "the six merchant start counties");
    let merchants = |sc: &Scenario| {
        sc.units.iter().filter(|(_, u)| u.kind == l2_kingdom::unit::UnitKind::Merchant).count()
    };
    assert_eq!(merchants(&b), merchants(&a), "the merchant count");
    assert_eq!(merchants(&b), 6, "England seats six merchants");
    assert_eq!(a.units.len(), b.units.len(), "England turn one has nothing but merchants");

    // ------------------------------- the five derived counts, on the kingdoms

    // `fields_*` are the one group the *scenario* cannot be compared on: the
    // save carries the file's cache and the map path carries nothing, because
    // `Scenario::skeleton` derives all five from the field tiles with
    // `field::recount` on both paths. So they are compared where they are
    // actually written — on the kingdom.
    let ka = a.starting_kingdom(SEED);
    let kb = b.starting_kingdom(SEED);
    for id in 1..=a.county_count {
        let (x, y) = (&ka.counties[id], &kb.counties[id]);
        assert_eq!(x.fields_grain, 0, "county {id}: nobody has sown grain by turn one");
        assert_eq!(y.fields_grain, 0, "county {id}: and the map path agrees");
        assert_eq!(x.fields_waste, y.fields_waste, "county {id}: waste");
        assert_eq!(x.fields_reclaiming, y.fields_reclaiming, "county {id}: reclaiming");
        assert_eq!(
            x.fields_fallow + x.fields_cattle,
            y.fields_fallow + y.fields_cattle,
            "county {id}: the field total moved, so a field was created or lost"
        );
    }
    // **A field can reach `CountyState` and stop there, and a diff of two
    // `CountyState`s cannot see it.** Deleting the one line of
    // `Scenario::skeleton` that carries `farm_style` into the county left every
    // other assertion in this file green — which is exactly the shape of the
    // four defects `docs/agents.md` lists, one layer further out. So every
    // field this file calls *agreed* is checked again where the rules actually
    // read it: on the county, on both paths.
    for id in 1..=a.county_count {
        let s = a.counties[id].as_ref().unwrap();
        let m = b.counties[id].as_ref().unwrap();
        for (k, from, path) in [(&ka.counties[id], s, "save"), (&kb.counties[id], m, "map")] {
            assert_eq!(k.farm_style, from.farm_style, "{path} county {id}: farm_style");
            assert_eq!(k.field_tiles, from.field_tiles, "{path} county {id}: field_tiles");
            assert_eq!(k.anchor_x, from.anchor.0, "{path} county {id}: anchor_x");
            assert_eq!(k.anchor_y, from.anchor.1, "{path} county {id}: anchor_y");
            assert_eq!(k.weather, from.weather, "{path} county {id}: weather");
            assert_eq!(k.ration_wanted, from.ration_wanted, "{path} county {id}: ration_wanted");
            assert_eq!(k.ration_split, from.ration_split, "{path} county {id}: ration_split");
            assert_eq!(k.labour_share, from.labour_share, "{path} county {id}: labour_share");
            assert_eq!(k.castle_switch, from.castle_switch, "{path} county {id}: castle_switch");
            assert_eq!(k.industry_share, from.industry_share, "{path} county {id}: industry_share");
            assert_eq!(k.tax_rate, from.tax_rate, "{path} county {id}: tax_rate");
            assert_eq!(
                k.neighbour_count as usize,
                from.neighbours.len(),
                "{path} county {id}: neighbour_count"
            );
            for c in 0..4 {
                assert_eq!(
                    k.industry[c].has_resource, from.industry[c].has_resource,
                    "{path} county {id} industry {c}: has_resource"
                );
                assert_eq!(
                    k.industry[c].enabled, from.industry[c].enabled,
                    "{path} county {id} industry {c}: enabled"
                );
            }
        }
    }
    // …and none of that is worth anything if every value is the default, so
    // each of the four that *can* be zero everywhere is checked for content.
    for (k, path) in [(&ka, "save"), (&kb, "map")] {
        let ids = 1..=k.county_count;
        assert!(
            ids.clone().any(|id| k.counties[id].farm_style != 0),
            "{path}: every county farms as style 0, so farm_style reaches nothing"
        );
        assert!(
            ids.clone().any(|id| k.counties[id].industry.iter().any(|i| i.enabled)),
            "{path}: no industry is switched on anywhere"
        );
        assert!(
            ids.clone().any(|id| k.counties[id].castle_switch),
            "{path}: no county is building a castle"
        );
        assert!(
            ids.clone().all(|id| k.counties[id].neighbour_count > 0),
            "{path}: an island county"
        );
    }

    // Difficulty 0 lays the first eight fields of every county to pasture, so
    // the map path's pasture count is `min(fields, 8)` everywhere — the whole
    // of the difficulty setting's effect on the land, measured.
    for id in 1..=b.county_count {
        let fields = kb.counties[id].field_tiles.iter().filter(|&&t| t != 0).count() as i32;
        assert_eq!(kb.counties[id].fields_cattle, fields.min(8), "county {id}: pasture");
        assert_eq!(kb.counties[id].fields_fallow, (fields - 8).max(0), "county {id}: fallow");
    }

    // ---------------------------------------------- the field-by-field report

    let mut verdicts: Vec<(&'static str, Verdict)> = Vec::new();
    let column = |sc: &Scenario, f: fn(&CountyState) -> i64| -> Vec<i64> {
        (1..=sc.county_count).map(|i| f(sc.counties[i].as_ref().unwrap())).collect()
    };
    let season = "the save is one season on: Game_NewGame runs an immediate Season_Advance";
    let mut judge = |field: &'static str, f: fn(&CountyState) -> i64, why: &'static str| {
        let (sv, mv) = (column(&a, f), column(&b, f));
        let v = if sv != mv {
            Verdict::Differ(why)
        } else if sv.iter().all(|&x| x == 0) {
            Verdict::BothSilent(why)
        } else {
            Verdict::Agree
        };
        verdicts.push((field, v));
    };

    // Written by something the game runs, on both paths, and equal.
    judge("neighbours", |c| c.neighbours.len() as i64, UNEXPLAINED);
    judge("anchor", |c| c.anchor.0 as i64 * 64 + c.anchor.1 as i64, UNEXPLAINED);
    judge("field_tiles", |c| c.field_tiles.iter().map(|&t| t as i64).sum(), UNEXPLAINED);
    judge("industry", |c| c.industry.iter().filter(|i| i.has_resource).count() as i64, UNEXPLAINED);
    judge("weather", |c| c.weather as i64, UNEXPLAINED);
    judge("ration_wanted", |c| c.ration_wanted as i64, UNEXPLAINED);
    judge("labour_share", |c| c.labour_share.iter().map(|&n| n as i64).sum(), UNEXPLAINED);
    judge("castle_switch", |c| c.castle_switch as i64, UNEXPLAINED);

    // Written on both paths and legitimately different.
    for (field, f) in [
        ("population", (|c: &CountyState| c.population as i64) as fn(&CountyState) -> i64),
        ("population_last", |c| c.population_last as i64),
        ("happiness", |c| c.happiness as i64),
        ("happiness_last", |c| c.happiness_last as i64),
        ("health_meter", |c| c.health_meter as i64),
        ("health_band", |c| c.health_band as i64),
        ("grain", |c| c.grain as i64),
        ("herd", |c| c.herd as i64),
        ("dryness", |c| c.dryness as i64),
        ("pop_band", |c| c.pop_band as i64),
        ("industry_share", |c| c.industry_share as i64),
        ("ration_achieved", |c| c.ration_achieved as i64),
        ("herd_eaten", |c| c.herd_eaten as i64),
        ("births", |c| c.births as i64),
        ("deaths", |c| c.deaths as i64),
        ("shown_tax", |c| c.shown_tax as i64),
        ("shown_ration", |c| c.shown_ration as i64),
        ("shown_health", |c| c.shown_health as i64),
        ("shown_events", |c| c.shown_events as i64),
        ("d_hap_ration", |c| c.d_hap_ration as i64),
    ] {
        judge(field, f, season);
    }

    judge("owner", |c| c.owner as i64, "FUN_00497E65 deals the realm-to-county assignment");
    judge(
        "castle_type",
        |c| c.castle_type as i64,
        "the castle level is the options', not the map's: Settings::apply_to writes it",
    );
    judge(
        "farm_style",
        |c| c.farm_style as i64,
        "County_Reset seeds countyId & 1 and the AI overwrites its own counties' — 12 of 14 match",
    );
    judge(
        "ration_split",
        |c| c.ration_split as i64,
        "County_Reset opens every county at 100; the first ration pass moves an AI county's",
    );
    for (field, f) in [
        ("labour", (|c: &CountyState| c.labour.iter().map(|&n| n as i64).sum())
            as fn(&CountyState) -> i64),
        ("labour_wanted", |c| c.labour_wanted.iter().map(|&n| n as i64).sum()),
        ("labour_useful", |c| c.labour_useful.iter().map(|&n| n as i64).sum()),
    ] {
        judge(
            field,
            f,
            "Labour_Allocate has not run on the map path — County_Reset's own call is not \
             reproduced, so a new game's counties arrive unallocated",
        );
    }
    judge(
        "fields_fallow",
        |c| c.fields_fallow as i64,
        "not carried on the map path: Scenario::skeleton derives all five counts with \
         field::recount, and they are compared on the kingdom above",
    );
    judge(
        "fields_cattle",
        |c| c.fields_cattle as i64,
        "not carried on the map path — see fields_fallow",
    );
    judge(
        "fields_grain",
        |c| c.fields_grain as i64,
        "not carried on the map path — see fields_fallow; both are 0 because nobody has sown",
    );

    // **Both silent, and each one is a claim about the game rather than a
    // shrug.** These are zero in the England turn-one save *and* zero at new
    // game, and the save's zero is the original's own byte — so they are
    // corroborated, not merely unwritten.
    judge("tax_rate", |c| c.tax_rate as i64, "nothing sets a tax rate at new game: the county \
         record is zeroed by FUN_0046EA28 and County_Reset does not write one");
    judge("tax_collected", |c| c.tax_collected as i64, "no tax has been collected at rate 0");
    judge("grain_eaten", |c| c.grain_eaten as i64, "the save's own byte is 0 in all fourteen");
    judge("emigrants", |c| c.emigrants as i64, "nobody has moved by turn one");
    judge("immigrants", |c| c.immigrants as i64, "nobody has moved by turn one");
    judge("unrest", |c| c.unrest as i64, "no county is in unrest at turn one");
    judge("fertility", |c| c.fertility as i64, "fertility accumulates and has not");
    judge("castle_building", |c| c.castle_building as i64, "no castle is under construction");
    judge(
        "industry",
        |c| c.industry.iter().map(|i| i.disabled_seasons as i64).sum(),
        "no industry has been wrecked",
    );

    let mut silent = Vec::new();
    let mut unexplained = Vec::new();
    for (field, v) in &verdicts {
        match v {
            Verdict::Agree => eprintln!("  agree       {field}"),
            Verdict::BothSilent(why) => {
                eprintln!("  BOTH ZERO   {field} — {why}");
                silent.push(*field);
            }
            Verdict::Differ(why) if *why == UNEXPLAINED => {
                eprintln!("  UNEXPLAINED {field}");
                unexplained.push(*field);
            }
            Verdict::Differ(why) => eprintln!("  differ      {field} — {why}"),
        }
    }
    assert!(unexplained.is_empty(), "differences with no reason given: {unexplained:?}");

    // The list of fields neither constructor writes. Not an assertion of
    // health — an assertion that it has not grown silently.
    silent.sort_unstable();
    silent.dedup();
    assert_eq!(
        silent,
        [
            "castle_building",
            "emigrants",
            "fertility",
            "fields_grain",
            "grain_eaten",
            "immigrants",
            "industry",
            "tax_collected",
            "tax_rate",
            "unrest",
        ],
        "the set of fields neither constructor writes has changed"
    );
}

// ------------------------------------------------ the enumeration, from source

/// Every field of [`CountyState`] the diff above judges, by name.
const JUDGED: &[&str] = &[
    "owner",
    "population",
    "population_last",
    "happiness",
    "happiness_last",
    "shown_tax",
    "shown_ration",
    "shown_health",
    "shown_events",
    "d_hap_ration",
    "health_meter",
    "health_band",
    "unrest",
    "births",
    "deaths",
    "emigrants",
    "immigrants",
    "pop_band",
    "anchor",
    "neighbours",
    "tax_rate",
    "tax_collected",
    "ration_wanted",
    "ration_achieved",
    "ration_split",
    "grain_eaten",
    "herd_eaten",
    "castle_type",
    "castle_building",
    "castle_switch",
    "industry",
    "fields_fallow",
    "fields_cattle",
    "fields_grain",
    "fertility",
    "weather",
    "dryness",
    "grain",
    "herd",
    "labour",
    "labour_wanted",
    "labour_useful",
    "labour_share",
    "industry_share",
    "field_tiles",
    "farm_style",
];

/// **Enumerate the fields; do not spot-check them.**
///
/// Reads `pub struct CountyState`'s field list out of
/// `crates/l2-scenario/src/lib.rs` and fails if a field is not in [`JUDGED`].
/// A hand-written list goes stale the day somebody adds a field; a list checked
/// against the definition cannot.
///
/// It needs no game, so it runs on CI — which is the point, because the diff
/// itself is fixture-gated and this is the half of it that is not.
#[test]
fn every_county_field_is_accounted_for() {
    let src = include_str!("../src/lib.rs");
    let start = src.find("pub struct CountyState {").expect("the struct is still called that");
    let body = &src[start..];
    let end = body.find("\n}").expect("the struct closes");
    let mut fields = Vec::new();
    for line in body[..end].lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("pub ") else { continue };
        let Some(name) = rest.split(':').next() else { continue };
        if name.is_empty() || name.contains(' ') {
            continue;
        }
        fields.push(name.to_string());
    }
    assert!(fields.len() > 20, "the parser found only {} fields", fields.len());
    let missing: Vec<&String> = fields.iter().filter(|f| !JUDGED.contains(&f.as_str())).collect();
    assert!(
        missing.is_empty(),
        "CountyState fields the two-constructor diff does not judge: {missing:?}.\n\
         Add each to JUDGED with a verdict, or say in the test why it cannot be compared."
    );
    let stale: Vec<&&str> = JUDGED.iter().filter(|c| !fields.iter().any(|f| f == *c)).collect();
    assert!(stale.is_empty(), "JUDGED names fields CountyState no longer has: {stale:?}");
}

// ------------------------------------------------------------------ ablation

/// **Break it on purpose and watch the checks go red.**
///
/// `docs/agents.md`: this project has twice shipped a check that passed with
/// the bug still in. The diff is worth exactly what its sensitivity is worth,
/// so this reproduces the three shapes of failure it exists to catch — a pass
/// that does not run, a pass that runs in the wrong order, and a table that
/// silently truncates — on a synthetic map, so it needs no install.
#[test]
fn the_checks_are_sensitive_to_the_three_mistakes_they_exist_for() {
    let slot_bytes = synthetic_slot();
    let set = MapSet::parse(&slot_bytes).unwrap();
    let good =
        newgame::build(&set.slot(0).unwrap(), &NewGame { lords: 1, ..NewGame::default() }).unwrap();

    // 1. The blacksmith pass not running: the county claims no weapons
    //    resource, which is what the corpus test's `has_resource[2]` catches.
    assert!(good.has_resource[1][2], "the blacksmith pass ran");
    // …and it went on a tile the file gave no flags at all.
    let smith = good.industry_site[1][2];
    assert_eq!(slot_bytes[Plane::Flags as usize * 4096 + smith], 0);

    // 2. The site passes in the wrong order: were the castle search to run
    //    before the resource sites, the mine — a `0x80` tile with no terrain —
    //    would be swallowed into the castle block and stamped `0x14`.
    assert_eq!(good.tiles.content[mine_tile()], 1, "the mine is iron, not a castle plot");
    assert_ne!(good.castle_tile[1], mine_tile(), "the castle did not eat the mine");

    // 3. The twenty-slot table dropping rather than razing: a twenty-first
    //    field would stay farmland with no entry, and the corpus test's
    //    `fields == surviving farm tiles` is what fails.
    let many = many_fields_slot();
    let set = MapSet::parse(&many).unwrap();
    let w = newgame::build(&set.slot(0).unwrap(), &NewGame { lords: 1, ..NewGame::default() })
        .unwrap();
    let left = (0..MAP_TILES).filter(|&t| w.tiles.flags[t] & 0x20 != 0).count();
    assert_eq!(left, 20, "a razed field stops being farmland");
    let listed = w.field_tiles[1].iter().filter(|&&t| t != 0).count();
    assert_eq!(listed, left, "every surviving field is in the table");
}

fn put(buf: &mut [u8], plane: Plane, x: usize, y: usize, v: u8) {
    buf[plane as usize * 4096 + y * PLANE_DIM + x] = v;
}

fn mine_tile() -> usize {
    30 * PLANE_DIM + 30
}

fn synthetic_slot() -> Vec<u8> {
    let mut buf = vec![0u8; SLOT_LEN];
    for y in 0..PLANE_DIM {
        for x in 0..PLANE_DIM {
            put(&mut buf, Plane::County, x, y, 1);
        }
    }
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        put(&mut buf, Plane::Flags, 10 + dx, 10 + dy, 0x40);
        put(&mut buf, Plane::GfxBank, 10 + dx, 10 + dy, 0x0C);
        put(&mut buf, Plane::Flags, 20 + dx, 20 + dy, 0x80);
        put(&mut buf, Plane::GfxIndex, 20 + dx, 20 + dy, 6);
    }
    put(&mut buf, Plane::Marker, 20, 20, 1);
    // The mine: a Town-bank tile drawing frame 30, carrying the site bit.
    put(&mut buf, Plane::Flags, 30, 30, 0x80);
    put(&mut buf, Plane::GfxBank, 30, 30, 0x0C);
    put(&mut buf, Plane::GfxIndex, 30, 30, 30);
    for i in 0..4 {
        put(&mut buf, Plane::Flags, 40 + i, 40, 0x10);
    }
    for i in 0..2 {
        put(&mut buf, Plane::Flags, 12 + i, 12, 0x20);
        put(&mut buf, Plane::GfxBank, 12 + i, 12, 0x08);
        put(&mut buf, Plane::GfxIndex, 12 + i, 12, 80);
    }
    buf
}

fn many_fields_slot() -> Vec<u8> {
    let mut buf = synthetic_slot();
    for i in 0..25usize {
        let (x, y) = (i % 5 + 20, i / 5 + 40);
        put(&mut buf, Plane::Flags, x, y, 0x20);
        put(&mut buf, Plane::GfxBank, x, y, 0x08);
        put(&mut buf, Plane::GfxIndex, x, y, 80);
    }
    buf
}
