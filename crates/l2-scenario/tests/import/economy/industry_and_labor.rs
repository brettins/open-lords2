#![allow(unused_imports)]
use super::*;
use super::tax_and_happiness::*;
use super::forecasts_and_events::*;
use super::*;
use super::county::*;
use super::units::*;
use l2_formats::save::{Layout, Save, COUNTY_BASE, COUNTY_STRIDE};
use l2_scenario::{ImportError, Scenario, STARTING_HEALTH_METER};
use l2_testkit::{saves, SaveFile};

/// **The other two words of the labour record**, which this crate read as
/// nothing until the village screen needed them.
///
/// The worker count is word 0 of a twelve-byte record; words 1 and 2 are a
/// *wanted floor* and a *useful ceiling*. Nothing but the right offsets
/// produces the pattern below
///
/// * **Seven of the nine floors are −1 in all fourteen counties.** Only the
/// cattle estimate (`FUN_0044DD4D`) and the grain estimate (`FUN_0044D374`)
/// ever write a real floor
///   grain sown, so cattle is the only one with a number in it. A misread
///   offset does not produce ninety-eight −1s.
/// * **Wood's ceiling is exactly 100,000 in every owned county and exactly 0
///   in every unowned one** — `FUN_0044F318` writes `LABOUR_UNBOUNDED` for an
///   industry the county has and 0 for one it does not.
/// * And that single byte explains the save's whole labour split: the
///   allocator fills each job up to its ceiling and drops the remainder into
///   *Idle townsfolk*, so an owned county has 217 foresters and nobody idle
///   while an unowned one has no forester and 133 idle.
#[test]
fn the_labour_records_other_two_words_are_a_wanted_floor_and_a_useful_ceiling() {
    use l2_kingdom::county::{LABOUR_NO_FLOOR, LABOUR_UNBOUNDED};
    use l2_kingdom::tables::{
        JOB_CATTLE_FARMING, JOB_COUNT, JOB_IDLE_TOWNSFOLK, JOB_WOOD_CUTTING,
    };

    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();

    let mut floors = 0;
    for id in s.county_ids() {
        let Some(c) = s.counties[id].as_ref() else { continue };

        for job in 0..JOB_COUNT {
            if job == JOB_CATTLE_FARMING || job == JOB_IDLE_TOWNSFOLK {
                continue;
            }
            assert_eq!(
                c.labour_wanted[job], LABOUR_NO_FLOOR,
                "county {id} job {job}: only grain and cattle ever ask for a floor"
            );
            floors += 1;
        }
        // Idle townsfolk is the one slot no estimate ever touches, so both its
        // spare words are still the zero `FUN_00451150` cleared them to.
        assert_eq!(c.labour_wanted[JOB_IDLE_TOWNSFOLK], 0, "county {id}");
        assert_eq!(c.labour_useful[JOB_IDLE_TOWNSFOLK], 0, "county {id}");

        // Break-even staffing is never above growth-maximising staffing, and
// both are real counts a county could field.
        let (want, useful) = (c.labour_wanted[JOB_CATTLE_FARMING], c.labour_useful[1]);
        assert!(want > 0 && want <= useful, "county {id}: cattle {want} .. {useful}");
        assert!(useful <= c.population, "county {id}: {useful} tenders of {} people", c.population);

        // The ceiling is what decides whether the county's spare people work.
        let owned = c.owner != 0;
        assert_eq!(
            c.labour_useful[JOB_WOOD_CUTTING],
            if owned { LABOUR_UNBOUNDED } else { 0 },
            "county {id} owner {}", c.owner
        );
        assert_eq!(
            c.labour[JOB_WOOD_CUTTING] > 0,
            owned,
            "county {id}: an unbounded ceiling is why anyone cuts wood"
        );
        assert_eq!(
            c.labour[JOB_IDLE_TOWNSFOLK] > 0,
            !owned,
            "county {id}: and a ceiling of zero is why the rest stand idle"
        );
    }
    assert_eq!(floors, 14 * 7, "fourteen counties, seven floorless jobs each");
}

/// **The realm's treasury and its armoury arrive**, both of them field for
/// field out of the file.
///
/// This test exists because deleting the importer's `realm.weapons = r.weapons`
/// broke **nothing in the workspace**. Every weapon assertion in the tree was
/// downstream of a fixture the test had written itself: `military.rs` sets
/// `weapons = [200; 6]` before it equips anyone, `merchant.rs` reads a stock
/// back after buying it, and `setup.rs` checks the *new-game* table. Nothing
/// read a stock that came off a disk — which is `docs/agents.md`'s rule to the
/// letter: *a field is only tested if something a test reads was written by
/// something the game runs.*
///
/// It matters now because the armoury is the screen that spends them. A levy
/// whose realm imports with an empty armoury is a levy that can only ever be
/// peasants
/// only when the realm owns one — so the failure would have been visible and
/// unexplained.
///
/// Asserted against the file, for the reason the
/// weather-county correction above records: a regenerated fixture is a
/// different game, and `docs/kingdom.md`'s *"50 swords, 50 pikes and 50 bows in
/// all five realms"* is one roll of `g_startArmoury`, not a law.
#[test]
fn every_imported_realm_holds_the_stocks_the_file_holds() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    let k = s.kingdom(1);

    let mut checked = 0;
    let mut armed = 0;
    for r in save.realms().unwrap().iter() {
        let ours = &k.realms[r.index];
        assert_eq!(ours.gold, r.gold, "realm {}", r.index);
        assert_eq!(ours.iron, r.iron, "realm {}", r.index);
        assert_eq!(ours.stone, r.stone, "realm {}", r.index);
        assert_eq!(ours.wood, r.wood, "realm {}", r.index);
        assert_eq!(
            ours.weapons.as_slice(),
            r.weapons.as_slice(),
            "realm {}'s armoury did not survive the seam",
            r.index,
        );
        checked += 1;
        if r.weapons.iter().any(|&w| w > 0) {
            armed += 1;
        }
    }
    assert!(checked >= 5, "only {checked} realms were compared");
    // Not a vacuous agreement: a fixture whose realms all had empty armouries
    // would pass the loop above while proving nothing about the field.
    assert!(armed > 0, "every realm in the fixture has an empty armoury: the test proves nothing");
}

/// **A county's four `hasResource` bytes agree with the map, 56 times out of
/// 56** — and until this test existed, none of them was read at all.
///
/// `County_PlaceResourceSites` (`0x00468E61`) writes `+0x295 + c*0x18` at load
/// from the county's `Town`-bank tiles, and `County_PlaceBlacksmith` does the
/// weapons record. So the byte and the map are two recordings of one fact and
/// have to match: an industry has its resource exactly when the county owns a
/// settlement tile (plane-0 bit `0x80`) whose terrain falls in that industry's
/// rung of `Map_Click`'s ladder — 0…3 iron, 4…6 stone, 7…9 weapons, 10…12 wood.
///
/// On the England fixture the answer is *false*
/// for 15 of the 56, iron and stone are complementary in thirteen of the
/// fourteen counties
/// is what the importer used to supply, fails this fifteen times.
#[test]
fn every_industrys_resource_byte_agrees_with_the_tiles_the_map_puts_it_on() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    let mut checked = 0;
    let mut without = 0;
    for id in s.county_ids() {
        let Some(c) = &s.counties[id] else { continue };
        // What the map says: the terrain of every settlement tile of this
        // county, run through the same ladder the map click uses.
        let mut from_map = [false; 4];
        for tile in 0..l2_kingdom::map::MAP_TILES {
            if s.map.county[tile] != id as u8
                || s.map.flags[tile] & l2_kingdom::map::flags::SETTLEMENT == 0
            {
                continue;
            }
            if let Some(l2_kingdom::industry::MapToggle::Industry(what)) =
                l2_kingdom::industry::map_toggle_for_graphic(s.map.terrain[tile])
            {
                from_map[what.index()] = true;
            }
        }
        for slot in 0..4 {
            assert_eq!(
                c.industry[slot].has_resource, from_map[slot],
                "county {id} industry {slot}: the record says {} and the map says {}",
                c.industry[slot].has_resource, from_map[slot]
            );
            checked += 1;
            without += usize::from(!from_map[slot]);
        }
    }
    assert_eq!(checked, 56, "fourteen counties, four industries each");
    assert_eq!(without, 15, "and fifteen of the fifty-six have no resource at all");
}

/// **Turn one has exactly five industries switched on: the wood cutting of the
/// five counties that start owned.** Every other switch of all fourteen
/// counties is off.
///
/// This is the byte `Industry_ToggleFromMap` XORs and the one
/// `Labour_Allocate` gates each mining job on, so importing it as a defaulted
/// `true` — which is what happened until C57 — starts the player with four
/// industries running in every county he owns and, worse, puts every one of the
/// map's toggles in the opposite position to the one the player sees.
#[test]
fn only_wood_is_switched_on_at_the_start_and_only_in_an_owned_county() {
    let save = l2_testkit::england!();
    let s = Scenario::from_save(&save).unwrap();
    let mut on = Vec::new();
    for id in s.county_ids() {
        let Some(c) = &s.counties[id] else { continue };
        for slot in 0..4 {
            if c.industry[slot].enabled {
                on.push((id, slot, c.owner));
            }
            assert_eq!(
                c.industry[slot].disabled_seasons, 0,
                "nothing is out of action on turn one"
            );
        }
    }
    assert!(
        on.iter().all(|&(_, slot, owner)| slot == 0 && owner != 0),
        "every switch that is on is wood, in an owned county: {on:?}"
    );
    assert_eq!(on.len(), 5, "five counties start owned and each has its forestry running");
}

// ------------------------------- the three numbers the county panels read back

/// **The industry row's forecast is its own record's `+0x14`, in every save** —
/// and that is what says the `Industry` array starts at county `+0x294`.
///
/// `docs/records.json` had the array at `+0x290`. Under that base the word the
/// sidebar draws for commodity `c`, county `+0x2A8 + c*0x18`, is the head of
/// record `c + 1`, and stone's `+0x2F0` is past the end. Under `+0x294` it is
/// the last field of record `c` itself. **The two readings predict different
/// numbers**
///
/// `Industry_LabourEstimate` (`0x0044F318`) writes the word as
/// `min(limit, Pct(workers / divisor, ramp))`, zeroed first, and only when the
/// county is owned, has a `popBand`, and its resource limit is positive — which
/// for wood, iron and stone is the switch, the seam and a zero countdown. With
/// *Advanced Farming* off the ramp is a flat 80 and the limit is 999. So this
/// computes, from bytes of **record `c`** and the job record `c` draws on, the
/// number the word must hold, and requires the file to hold exactly it.
///
/// **It is not vacuous between the two bases.** Where record `c + 1`'s guards
/// and workers would give a different number, the file sides with record `c` —
/// siege-lastturn county 4 stores 74 at `+0x2A8`, which is 93 woodcutters × 80%,
/// not iron's 92 × 80% = 73 — and the test counts those cases and requires
/// some. Stone is switched off in every save on this machine, so its word is
/// only ever checked at zero; that is a limit of the corpus and is said here.
///
/// Weapons is excluded from the equality, as in
/// `crates/l2-kingdom/tests/industry_forecast/main.rs`: its limit is a realm-wide
/// share of wood and iron. It is still checked to be zero when a guard fails
/// and never above the unlimited figure.
///
/// Every offset and constant below is a literal, so ablating one in the
/// importer cannot move this expectation with it. **Ablations, both run:**
/// putting `INDUSTRY_BASE` back to `0x290` (offsets unchanged) turns this red
/// at the import half and `every_industrys_resource_byte_agrees…` red with it;
/// dropping the `next_season` line from `Scenario::kingdom` turns the
/// import-reaches-the-kingdom half red.
#[test]
fn every_saved_industry_forecast_is_what_its_own_records_workers_make() {
    // Wood, iron, weapons, stone: the job slot each draws on and
    // `Industry_Produce`'s divisor. `County_RefreshEstimates`' four calls.
    const JOB: [u32; 4] = [6, 4, 7, 5];
    const DIVISOR: [i32; 4] = [1, 1, 4, 2];
    const WEAPONS: u32 = 2;

    let mut checked = 0usize;
    let mut non_zero = 0usize;
    let mut discriminating = 0usize;
    for f in l2_testkit::saves!() {
        let s = &f.save;
        assert_eq!(
            s.globals().unwrap().opt_advanced_farming,
            0,
            "{}: Advanced Farming is on, so the flat 80 below is not the rule",
            f.label()
        );
        let scenario = Scenario::from_save(s).expect("import");
        let kingdom = scenario.kingdom(1);
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            let Ok(owner) = s.u8_at(base + 0x05) else { continue };
            let band = s.u8_at(base + 0xB8).unwrap();
            // What `Industry_LabourEstimate` writes for record `r`, from record
            // `r`'s bytes. `None` for a record that does not exist.
            let made = |r: u32| -> Option<i32> {
                if r > 3 {
                    return None;
                }
                let rec = base + 0x294 + r * 0x18;
                let has = s.u8_at(rec + 0x01).unwrap() != 0;
                let countdown = s.u8_at(rec + 0x02).unwrap();
                let on = s.u8_at(rec + 0x03).unwrap() != 0;
                let workers = s.i32_at(base + 0xC4 + JOB[r as usize] * 0x0C).unwrap();
                Some(if owner == 0 || band == 0 || !has || !on || countdown != 0 {
                    0
                } else {
                    ((workers / DIVISOR[r as usize]) * 80 / 100).min(999)
                })
            };
            for c in 0..4u32 {
                let stored = s.i32_at(base + 0x2A8 + c * 0x18).unwrap();
                let own = made(c).unwrap();
                if c == WEAPONS {
                    assert!(
                        (own == 0 && stored == 0) || (own != 0 && (0..=own).contains(&stored)),
                        "{}: county {id} weapons stores {stored}, its own record allows {own}",
                        f.label()
                    );
                } else {
                    assert_eq!(
                        stored,
                        own,
                        "{}: county {id} commodity {c} stores {stored} at +{:#X}, and record \
                         {c}'s own workers make {own}",
                        f.label(),
                        0x2A8 + c * 0x18
                    );
                    if made(c + 1) != Some(own) {
                        discriminating += 1;
                    }
                }
                // The importer carries it
// receives it.
                if let Some(state) = &scenario.counties[id] {
                    assert_eq!(state.industry[c as usize].next_season, stored, "{}", f.label());
                    assert_eq!(
                        kingdom.counties[id].industry[c as usize].next_season,
                        stored,
                        "{}: county {id} commodity {c} was imported and did not reach the kingdom",
                        f.label()
                    );
                }
                checked += 1;
                non_zero += usize::from(stored != 0);
            }
        }
    }
    assert!(checked >= 4 * 14, "only {checked} forecasts were reached");
    assert!(non_zero > 0, "every stored forecast is zero, so nothing was compared");
    assert!(
        discriminating > 0,
        "no county anywhere tells record c from record c + 1, so this cannot tell the bases apart"
    );
    eprintln!("{checked} forecasts, {non_zero} non-zero, {discriminating} discriminating");
}

