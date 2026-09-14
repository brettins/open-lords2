#![allow(unused_imports)]
use super::*;
use super::county::*;
use super::economy::*;
use l2_formats::save::{Layout, Save, COUNTY_BASE, COUNTY_STRIDE};
use l2_scenario::{ImportError, Scenario, STARTING_HEALTH_METER};
use l2_testkit::{saves, SaveFile};

/// **Every unit in every save survives the seam**, slot for slot and field for
/// field.
///
/// The importer used to read the counties, the realms and the map and stop
/// there
/// assertion that fails if the block goes back on the floor, and it runs over
/// every save because one file agreeing proves nothing: the England fixture is
/// six merchants
/// besieger and a levied defence.
#[test]
fn every_unit_in_every_save_is_imported_slot_for_slot() {
    let saves = saves!();
    let mut checked = 0;
    for SaveFile { name, save, .. } in &saves {
        let scenario = Scenario::from_save(save).expect("the save imports");
        let stored: Vec<_> =
            save.units().unwrap().into_iter().filter(|u| u.is_live()).collect();
        assert_eq!(
            scenario.units.len(),
            stored.len(),
            "{name}: {} units in the file, {} imported",
            stored.len(),
            scenario.units.len()
        );
        let kingdom = scenario.kingdom(1);
        for (n, f) in stored.iter().enumerate() {
            let (slot, ours) = &scenario.units[n];
            let at = format!("{name} unit {}", f.index);
            assert_eq!(*slot, f.index, "{at}: the slot moved");
            assert_eq!(ours.owner, f.owner, "{at}: owner");
            assert_eq!(ours.kind.byte(), f.kind, "{at}: kind");
            assert_eq!((ours.x, ours.y), (f.x, f.y), "{at}: tile");
            assert_eq!(ours.county, f.county, "{at}: county");
            assert_eq!(ours.home_county, f.home_county, "{at}: home county");
            assert_eq!(ours.men, f.men, "{at}: men");
            assert_eq!(ours.wages, f.wages, "{at}: wages");
            assert_eq!(ours.morale, f.morale as i32, "{at}: morale");
            assert_eq!(ours.name_index, f.name_index, "{at}: +0x14F");
            assert_eq!(ours.year_formed, f.year_formed as i32, "{at}: +0x164");
            // `+0x167` is one byte with two meanings, and `Unit` models it as
            // two fields: the defence mark on an army or a mob, the cargo county
            // on a transport — and, on a merchant, the county it was spawned in,
            // which lands in the same non-army field.
            let (mark, cargo) =
                if ours.kind.is_combatant() { (f.role, 0) } else { (0, f.role) };
            assert_eq!(ours.defence_mark, mark, "{at}: +0x167 as a defence mark");
            assert_eq!(ours.cargo_county, cargo, "{at}: +0x167 as a cargo county");
            assert_eq!(ours.garrison_county, f.garrison_county, "{at}: garrison");
            assert_eq!(ours.besieging_county, f.besieging_county, "{at}: besieging");
            assert_eq!(ours.needs_destination, f.needs_destination, "{at}: idle");
            assert_eq!(ours.dest_county, f.dest_county, "{at}: dest county");
            assert_eq!(ours.path.len(), f.path_len as usize, "{at}: path length");
            assert_eq!(ours.moves_used, f.moves_used as i32, "{at}: moves used");
            // **Not `kind.move_allowance()`.** The tick handler writes it, so
            // the file's zero is the truth for a unit that has not been ticked.
            assert_eq!(ours.move_allowance, f.move_allowance as i32, "{at}: allowance");
            for t in 0..7 {
                assert_eq!(ours.troops[t], f.troops[t] as i32, "{at}: troop {t}");
            }
            // And the kingdom got it in the same slot
            // `garrison_unit`, `besieged_by` and the merchant route index all
            // depend on.
            assert_eq!(kingdom.campaign.units.get(*slot), Some(ours), "{at}: in the kingdom");
            checked += 1;
        }
    }
    eprintln!("{checked} units imported across {} saves", saves.len());
    assert!(checked > 6, "only {checked} units reached; the battle saves add armies");
}

/// The route table reaches the kingdom unchanged, row for row.
///
/// `l2-kingdom` has had `MerchantRoutes` and `Merchant_AdvanceAll` since the
/// turn movers landed and **nothing but a test had ever filled the table**, so
/// a game loaded from a save arrived with six empty rows. This is the assertion
/// that they arrive full.
#[test]
fn the_merchant_routes_reach_the_kingdom() {
    let saves = saves!();
    for SaveFile { name, save, .. } in &saves {
        let scenario = Scenario::from_save(save).expect("imports");
        let k = scenario.kingdom(1);
        let stored = save.merchant_routes().unwrap();
        for route in 0..l2_kingdom::merchant::ROUTES {
            assert_eq!(k.campaign.routes.row(route), &stored[route], "{name}: route {route}");
        }
        assert_eq!(
            scenario.merchant_start,
            save.merchant_start_counties().unwrap(),
            "{name}: start counties"
        );
        // One merchant per start county **before the first zero** —
// `Merchant_SpawnAll` breaks, so a later non-zero
        // entry never spawns anything.
        let spawned = scenario.merchant_start.iter().take_while(|&&c| c != 0).count();
        assert_eq!(
            spawned as i32,
            save.globals().unwrap().merchant_count,
            "{name}: merchants against start counties"
        );
        assert_eq!(
            k.campaign.units.iter().filter(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant).count(),
            spawned,
            "{name}: and against the units that arrived"
        );
    }
}

/// The England position imports as six merchants owned by nobody, in the six
/// counties `docs/formats/plane4.md` predicted, each with the route it will
/// walk.
#[test]
fn the_england_fixture_imports_six_merchants_and_no_armies() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    assert_eq!(s.units.len(), 6);
    assert_eq!(s.merchant_start, [14, 5, 13, 11, 12, 4]);

    for (n, (slot, u)) in s.units.iter().enumerate() {
        assert_eq!(*slot, n + 1);
        assert_eq!(u.kind, l2_kingdom::UnitKind::Merchant);
        assert_eq!(u.owner, 6, "a merchant belongs to nobody");
        assert_eq!(u.county, s.merchant_start[n]);
        assert_eq!(u.cargo_county, s.merchant_start[n], "+0x167 is where it was born");
        assert_eq!(u.move_allowance, 0, "the file's zero, not the type's ten");
        assert_eq!(u.year_formed, 1, "the route cursor ships at 1, not 0");
        assert!(u.needs_destination);
        // Each merchant is standing in a county on its own route
        // it walks is its **slot** minus one — the coupling `Merchant_AdvanceAll`
        // rests on.
        let route = s.routes.row(slot - 1);
        assert!(
            route.contains(&u.county),
            "merchant {slot} started in county {}, which is not on route {route:?}",
            u.county,
        );
    }
    assert!(s.units.iter().all(|(_, u)| u.men == 0), "the England position has no troops on the map");
}

/// A unit whose `+0x0C` disagrees with its `x`/`y` is refused, because the two
/// can only disagree if the array is being read at the wrong stride.
#[test]
fn a_unit_whose_tile_offset_disagrees_with_its_tile_is_refused() {
    let exe = l2_testkit::executable!();
    for s in &saves!() {
        let bytes = std::fs::read(&s.path).expect("re-read");
        // Slot 1 exists in every save the machine has: the merchants take the
        // low slots. `+0x0A` is its x.
        let va = l2_formats::save::UNIT_BASE + l2_formats::save::UNIT_STRIDE as u32 + 0x0A;
        let x = s.save.u8_at(va).unwrap();
        let poked = poke(&exe, &bytes, va, x.wrapping_add(1));
        let save = Save::open(&exe, &poked).expect("still the right length");
        assert!(
            matches!(Scenario::from_save(&save), Err(ImportError::UnitTile { unit: 1, .. })),
            "{}: a moved unit was accepted",
            s.label()
        );
    }
}

/// A type byte naming no handler is refused.
#[test]
fn a_unit_type_that_names_no_handler_is_refused() {
    let exe = l2_testkit::executable!();
    for s in &saves!() {
        let bytes = std::fs::read(&s.path).expect("re-read");
        let va = l2_formats::save::UNIT_BASE + l2_formats::save::UNIT_STRIDE as u32 + 0x08;
        let poked = poke(&exe, &bytes, va, 5);
        let save = Save::open(&exe, &poked).expect("still the right length");
        assert_eq!(
            Scenario::from_save(&save),
            Err(ImportError::UnitKind { unit: 1, byte: 5 }),
            "{}",
            s.label()
        );
    }
}

/// **A castle garrison survives the import, and it does so on every save that
/// has one.**
///
/// The relation has two halves in the original — county `+0x1BC` names the unit,
/// unit `+0x198` names the county — and this importer reads only the unit's,
/// then derives the county's. It had not derived it at all: `County::new` seeds
/// `garrison_unit: 0`, nothing overwrote it, and **every loaded game arrived
/// with no castle garrisoned**. `conquest`'s ownership test, `divide`, `siege`
/// and the campaign map's castle flag are all downstream of that field
/// flag is what exposed it. C59.
///
/// Run over **every** save the machine can offer,
/// because the failure was silent on all of them: ten of the eleven in the tree
/// carry a garrison and the eleventh is England turn one
/// it is turn one. Asserting "both halves agree" on each is what makes this a
/// check of the derivation and not of one file.
#[test]
fn a_castle_garrison_reaches_the_county_it_is_standing_in() {
    let saves = saves!();
    let mut with_a_garrison = 0;
    for s in &saves {
        let scenario = Scenario::from_save(&s.save).expect("a save this code can read");
        let kingdom = scenario.kingdom(1);
        for (slot, unit) in &scenario.units {
            let county = unit.garrison_county as usize;
            if county == 0 {
                continue;
            }
            with_a_garrison += 1;
            assert_eq!(
                kingdom.counties[county].garrison_unit, *slot,
                "{}: unit {slot} says it garrisons county {county}, and the county does not \
                 say so back — the derivation in `skeleton` ran before the county loop \
                 overwrote it, or not at all",
                s.name
            );
            assert_ne!(
                kingdom.counties[county].castle_type, 0,
                "{}: county {county} holds a garrison and has no castle to hold it",
                s.name
            );
        }
    }
    assert!(
        with_a_garrison > 0,
        "no save on this machine has a garrison, so this test asserted nothing"
    );
}

/// `g_mercBands` (`0x00568DC0`), stride `0x14`, and `g_mercBandsInPlay`.
const MERC_BANDS: u32 = 0x0056_8DC0;
const MERC_STRIDE: u32 = 0x14;
const MERC_IN_PLAY: u32 = 0x0055_4030;

/// **Every mercenary band in every save reaches the kingdom holding the file's
/// walk, and every county's offer is `Mercenary_OfferInCounty` over it.**
///
/// A loaded game used to have no bands in play at all, so the raise-army screen
/// never offered one and the town square never showed one. The comparison is
/// the kingdom against the **bytes**, band by band, and two of the assertions
/// are the table checking itself:
///
/// * a band that offered itself this season has **just reloaded its countdown
///   and stepped one past the county it stands in**, with no wrap — the tail of
///   `Mercenary_AdvanceAll`'s offer branch — which ties `+0x03`, `+0x04`,
///   `+0x06` and `+0x07` together on every offering band on disk;
/// * county `+0x1AD` is the **lowest-numbered** unhired band offered there, and
///   `siege-old_turn.sav` puts bands 2 and 3 in county 1 at once.
///
/// **Ablation, run:** delete `k.campaign.mercenaries = self.mercenaries.clone()`
/// from `Scenario::skeleton` and the in-play assertion fails on the first save.
#[test]
fn every_mercenary_band_reaches_the_kingdom_and_every_offer_is_its_cache() {
    let saves = saves!();
    let (mut offers, mut offering, mut bands_seen) = (0usize, 0usize, 0usize);
    for s in &saves {
        let save = &s.save;
        let scenario = Scenario::from_save(save).unwrap_or_else(|e| panic!("{}: {e}", s.label()));
        let k = scenario.kingdom(1);
        let bands = &k.campaign.mercenaries;
        let in_play = save.i32_at(MERC_IN_PLAY).unwrap();
        assert_eq!(bands.in_play() as i32, in_play, "{}: g_mercBandsInPlay", s.label());
        assert_eq!(
            bands.in_play(),
            l2_kingdom::mercenary::bands_in_play(scenario.county_count),
            "{}: g_mercBandCount[g_countyCount]",
            s.label()
        );
        for slot in 0..l2_kingdom::mercenary::BAND_SLOTS {
            let at = MERC_BANDS + slot as u32 * MERC_STRIDE;
            if slot == 0 || slot as i32 > in_play {
                for off in 0..MERC_STRIDE {
                    assert_eq!(
                        save.u8_at(at + off).unwrap(),
                        0,
                        "{}: slot {slot} is not a band in play and holds a byte at +{off:#x}",
                        s.label()
                    );
                }
                continue;
            }
            bands_seen += 1;
            let b = bands.get(slot as u8).expect("in play");
            let what = |field: &str| format!("{}: band {slot} {field}", s.label());
            assert_eq!(b.hired_by as i16, save.i16_at(at).unwrap(), "{}", what("+0x00 hired by"));
            assert_eq!(b.offered_in, save.u8_at(at + 3).unwrap(), "{}", what("+0x03 offered in"));
            assert_eq!(b.next_county, save.u8_at(at + 4).unwrap(), "{}", what("+0x04 next county"));
            assert_eq!(b.countdown, save.i8_at(at + 6).unwrap(), "{}", what("+0x06 countdown"));
            assert_eq!(b.reload, save.i8_at(at + 7).unwrap(), "{}", what("+0x07 reload"));
            assert!((1..=b.reload).contains(&b.countdown), "{}", what("countdown outside 1..=reload"));
            if b.offered_in != 0 {
                offering += 1;
                assert_eq!(b.countdown, b.reload, "{}", what("offered, so the countdown just reloaded"));
                assert_eq!(b.next_county, b.offered_in + 1, "{}", what("offered, so the walk stepped past"));
            }
        }
        for id in scenario.county_ids() {
            let cached = k.counties[id].mercenary_offer;
            assert_eq!(
                cached,
                bands.offer_in(id as u8),
                "{}: county {id}'s +0x1AD is not Mercenary_OfferInCounty over the table",
                s.label()
            );
            offers += usize::from(cached != 0);
        }
    }
    eprintln!(
        "{} saves: {bands_seen} bands compared, {offering} of them offering, {offers} county offers",
        saves.len()
    );
}

/// The four one-End-Turn pairs on disk — `crates/l2-game/tests/differential.rs`
/// establishes which files are a turn apart, and it is not the pairs their names
/// suggest.
const TURN_PAIRS: [(&str, &str); 4] = [
    ("safeturn.sav", "old_turn.sav"),
    ("old_turn.sav", "battle-before.sav"),
    ("siege-safeturn.sav", "siege-old_turn.sav"),
    ("siege-old_turn.sav", "siege-lastturn.sav"),
];

/// **One season of `Mercenary_AdvanceAll` over a save lands on the next save's
/// band table and county offers, exactly.**
///
/// The strongest check the band import has, because nothing in it is ours
/// agreeing with ours: the *after* table was written by the original a turn
/// later. It covers all three walk branches on data — a band that only counts
/// down, one that wraps past the last county, and one that offers itself (four
/// offers across the pairs, including the two-band collision in county 1).
#[test]
fn one_season_of_the_mercenary_walk_lands_on_the_next_saves_table() {
    for (before, after) in TURN_PAIRS {
        let b = l2_testkit::fixture!(before);
        let a = l2_testkit::fixture!(after);
        let sb = Scenario::from_save(&b).expect("imports");
        let sa = Scenario::from_save(&a).expect("imports");
        assert_eq!(sa.clock.turn_count, sb.clock.turn_count + 1, "{before} -> {after} is one turn");
        let mut k = sb.kingdom(1);
        let (count, quirks) = (k.county_count, k.options.quirks);
        let l2_kingdom::Kingdom { counties, campaign, .. } = &mut k;
        campaign.mercenaries.advance(counties, count, quirks);
        let ka = sa.kingdom(1);
        assert_eq!(k.campaign.mercenaries, ka.campaign.mercenaries, "{before} -> {after}: the bands");
        for id in sa.county_ids() {
            assert_eq!(
                k.counties[id].mercenary_offer, ka.counties[id].mercenary_offer,
                "{before} -> {after}: county {id}'s offer"
            );
        }
    }
}

/// A band whose constant fields disagree with the roster is refused: the kingdom
/// keeps those in the roster, so importing the rest would price the band at a
/// number the file does not hold.
#[test]
fn a_mercenary_band_that_disagrees_with_the_roster_is_refused() {
    // Band 1's price, 1800 = 0x708: its low byte, one higher.
    refusal_over_every_save(
        |_| MERC_BANDS + MERC_STRIDE + 0x0C,
        0x09,
        |_| ImportError::MercenaryRoster { band: 1, field: "price", file: 1801, roster: 1800 },
    );
}

#[test]
fn a_mercenary_band_offered_off_the_map_is_refused() {
    refusal_over_every_save(
        |_| MERC_BANDS + MERC_STRIDE + 0x03,
        99,
        |_| ImportError::MercenaryState { band: 1, field: "offered county", value: 99 },
    );
}

/// Slot 1 is a merchant in every save on the machine — the merchants take the
/// low slots — and a merchant cannot carry a band.
#[test]
fn a_mercenary_band_hired_by_something_that_does_not_carry_it_is_refused() {
    refusal_over_every_save(
        |_| MERC_BANDS + MERC_STRIDE,
        1,
        |_| ImportError::MercenaryState { band: 1, field: "hiring unit", value: 1 },
    );
}

#[test]
fn a_band_count_past_twelve_is_refused() {
    refusal_over_every_save(|_| MERC_IN_PLAY, 13, |_| ImportError::MercenaryBandCount(13));
}

/// The raise-army screen indexes the roster with the county's byte.
#[test]
fn a_county_offering_a_band_that_is_not_in_play_is_refused() {
    refusal_over_every_save(
        |_| COUNTY_BASE + COUNTY_STRIDE as u32 + 0x1AD,
        13,
        |_| ImportError::MercenaryOffer { county: 1, band: 13 },
    );
}

/// **A hired band arrives hired, on the army that carries it** — the one half of
/// the band table no save on this machine exercises, because `+0x00` is zero in
/// every band of every save. So this writes a consistent hire into a copy of the
/// first save with an army in it: the army's `+0x195…+0x197` and the band's
/// hirer, and nothing else.
#[test]
fn a_hired_band_arrives_hired_on_the_army_that_carries_it() {
    let exe = l2_testkit::executable!();
    let saves = saves!();
    let Some((s, slot)) = saves.iter().find_map(|s| {
        s.save.units().ok()?.iter().find(|u| u.is_live() && u.kind == 1).map(|u| (s, u.index))
    }) else {
        l2_testkit::skip!("no save on this machine holds an army");
    };
    let unit = l2_formats::save::UNIT_BASE + (slot * l2_formats::save::UNIT_STRIDE) as u32;
    let bytes = std::fs::read(&s.path).expect("re-read");
    let mut poked = poke(&exe, &bytes, unit + 0x195, 4); // pikemen
    poked = poke(&exe, &poked, unit + 0x196, 100);
    poked = poke(&exe, &poked, unit + 0x197, 1); // the Scottish band
    poked = poke(&exe, &poked, MERC_BANDS + MERC_STRIDE, slot as u8);
    let save = Save::open(&exe, &poked).expect("still the right length");
    let scenario = Scenario::from_save(&save).unwrap_or_else(|e| panic!("{}: {e}", s.label()));
    let k = scenario.kingdom(1);
    let band = k.campaign.mercenaries.get(1).expect("in play");
    assert_eq!(band.hired_by as usize, slot, "{}", s.label());
    assert!(!band.is_available());
    assert_eq!(
        k.campaign.units.get(slot).and_then(|u| u.mercenaries).map(|m| (m.band, m.men)),
        Some((1, 100)),
        "{}: slot {slot} carries the band",
        s.label()
    );
    for id in scenario.county_ids() {
        assert_ne!(k.campaign.mercenaries.offer_in(id as u8), 1, "a hired band is offered nowhere");
    }
}

