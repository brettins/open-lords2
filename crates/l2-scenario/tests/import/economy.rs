#![allow(unused_imports)]
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

/// **The field counts we derive are the counts every reachable save stores.**
///
/// The importer no longer carries `+0x1FF`, `+0x200` and `+0x201` across: it
/// reads the twenty tiles in `g_countyFieldTiles`, applies
/// `County_RecountFields`' terrain ladder to the tile planes, and lets the
/// kingdom hold what that makes. `CountyState` still carries what the file
/// said, so the two are independent readings of the same thing and this diffs
/// them.
///
/// It runs over **every** save the machine can offer
/// named fixture, because `docs/decisions.md` C26 is what happens when a rule
/// is checked against one value of its input — and the battle saves are a
/// different map with four counties, which is exactly the second value.
#[test]
fn the_field_counts_are_derived_and_they_match_every_save_that_stores_them() {
    let saves = saves!();
    let mut counties_checked = 0;
    for SaveFile { name, save, .. } in &saves {
        let Ok(scenario) = Scenario::from_save(save) else { continue };
        let kingdom = scenario.kingdom(1);
        for id in scenario.county_ids() {
            let Some(stored) = &scenario.counties[id] else { continue };
            let ours = &kingdom.counties[id];
            counties_checked += 1;
            assert_eq!(
                (ours.fields_fallow, ours.fields_cattle, ours.fields_grain),
                (stored.fields_fallow, stored.fields_cattle, stored.fields_grain),
                "{name} county {id}: recounting its {} field tiles disagrees with the file",
                ours.field_slots_used()
            );
        }
    }
    assert!(counties_checked > 14, "only {counties_checked} counties reached");
}

// --- the campaign layer -----------------------------------------------------

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

/// `Pct` — `Tax_RecomputePreview`'s own rounding, which is truncation.
fn pct(v: i32, p: i32) -> i32 {
    v * p / 100
}

/// **`g_castleTaxBase`, written out.** The multiplier for
/// castle types 0 … 5, immediates in `Tax_CollectAll`'s instruction stream
/// (`docs/kingdom.md` §10). Spelled here so that the assertion below does not
/// compute its expected value from the table it is checking — the trap
/// `docs/agents.md` records as *ablating a constant while computing your probe
/// from that same constant*.
const CASTLE_TAX_BASE: [i32; 6] = [320, 480, 560, 640, 720, 800];

/// The one moment on this machine where `+0xC0` is **not** the current
/// population's answer, named with its reason.
///
/// It is the middle save of the battle triple
/// the explanation: `battle-during.sav` (the install calls the same game
/// `incombat.sav`) is taken with a battle open. County 2's population has
/// already fallen to 588 and the stored preview is still **245**
/// `Pct(Pct(638, 480), 8)` — the answer for the population the county had
/// before the fighting. `battle-after.sav` stores **225** for the same county,
/// which *is* `Pct(Pct(588, 480), 8)`.
///
/// recomputing it on load.** No recompute can produce 245; the original
/// restores a memory image, and `Tax_RecomputePreview` runs on a control or at
/// the end of a season, not on a load.
const PREVIEW_NOT_YET_REFRESHED: &[&str] = &["battle-during.sav", "incombat.sav"];

/// **`+0x0F` is `5 - taxRate` in every owned county of every save**, which is
/// `Tax_RecomputePreview` (`0x0044B80B`)'s second statement and is what says
/// the offset is the right one. It is a different field from `+0x0E`, which
/// carries the realm's empire term as well.
#[test]
fn the_local_tax_happiness_byte_is_five_minus_the_rate_in_every_save() {
    let mut checked = 0usize;
    for f in l2_testkit::saves!() {
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            let Ok(owner) = s.u8_at(base + 0x05) else { continue };
            if owner == 0 {
                continue;
            }
            let rate = s.u8_at(base + 0xB9).unwrap() as i32;
            let local = s.i8_at(base + 0x0F).unwrap() as i32;
            assert_eq!(
                local,
                5 - rate,
                "{}: county {id} stores +0x0F = {local} at rate {rate}",
                f.label()
            );
            checked += 1;
        }
    }
    assert!(checked >= 5, "only {checked} counties were reached");
}

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
/// `crates/l2-kingdom/tests/industry_forecast.rs`: its limit is a realm-wide
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

/// **The tax panel's *People pay* line, against the original's own answer.**
///
/// `+0xC0` is `Pct(Pct(population, castleBase), taxRate)` — the third statement
/// of `Tax_RecomputePreview` — and the saves on this machine carry rates 2, 3,
/// 6 and 8, so the arithmetic can be checked against a number the original
/// wrote. `docs/plan.md` §2.5 says every county in
/// every fixture sits at rate 0; that is true of the England fixture and false
/// of the turn pair and the six siege saves.
///
/// It is still true of any rate above 19, where `g_taxHappinessOther` starts to
/// bite — so this promotes the *preview*, not the empire term.
#[test]
fn the_tax_preview_byte_is_the_arithmetic_we_implement() {
    let mut agreed = 0usize;
    for f in l2_testkit::saves!() {
        if PREVIEW_NOT_YET_REFRESHED.contains(&f.name.as_str()) {
            continue;
        }
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            let Ok(owner) = s.u8_at(base + 0x05) else { continue };
            if owner == 0 {
                continue;
            }
            let rate = s.u8_at(base + 0xB9).unwrap() as i32;
            if rate == 0 {
                continue;
            }
            let pop = s.i32_at(base + 0x24).unwrap();
            let castle = s.u8_at(base + 0x1C0).unwrap() as usize;
            let shown = s.i32_at(base + 0xC0).unwrap();
            let base_mult = CASTLE_TAX_BASE[castle.min(5)];
            assert_eq!(
                pct(pct(pop, base_mult), rate),
                shown,
                "{}: county {id}, {pop} people at rate {rate} behind castle {castle}",
                f.label()
            );
            agreed += 1;
        }
    }
    if agreed == 0 {
        l2_testkit::skip!(
            "no reachable save carries a county at a non-zero tax rate, so there is \
             nothing to check the preview against"
        );
    }
}

// ------------------------------------ what docs/stored-fields.json found dropped

/// **County `+0x2F8` — the figure *Plague* and *Wedding fever*'s letters print —
/// reaches a loaded game**, measured on a patched copy of a real save because
/// the corpus cannot measure it.
///
/// Every county of every save on this machine stores **zero** here, so
/// `tests/stored_fields.rs` compares the row with zero and nothing else — and
/// `County::new()` is zero too, so deleting the importer's assignment leaves that
/// check green. Ablated: it did. The saves are not silent about events: county 3
/// holds Wedding fever's id `0x8E` in four consecutive siege saves, turns 12 to
/// 14, beside a zero figure. That id is **stale**, not this season's —
/// `Event_RollAll` (`0x00448819`) clears the three swing bytes and the tax gate
/// every season and never `eventId` — and the saved births reproduce with no swing
/// in them (`docs/decisions.md` C169).
///
/// So this writes a figure into one county's bytes, reopens the file through the
/// executable's own block table, and asks the import for it back.
#[test]
fn the_plague_letters_figure_survives_a_load() {
    let Some(exe) = l2_testkit::executable() else {
        l2_testkit::skip!("no Lords2.exe to take the save layout from");
    };
    let saves = l2_testkit::saves!();
    let f = saves.first().expect("saves! skips when there are none");
    let mut bytes = std::fs::read(&f.path).expect("the save that just opened");

    let county = 1usize;
    let va = COUNTY_BASE + (county * COUNTY_STRIDE) as u32 + 0x2F8;
    let stored = f.save.i32_at(va).unwrap();
    let figure = stored + 74; // a Winter plague on 1,000 people in band 2
    let at = f.save.layout().offset_of(va).expect("+0x2F8 is inside the county block");
    bytes[at..at + 4].copy_from_slice(&figure.to_le_bytes());

    let patched = l2_formats::save::Save::open(&exe, &bytes).expect("the patched save opens");
    assert_eq!(patched.i32_at(va).unwrap(), figure, "the patch landed on +0x2F8");
    let k = l2_scenario::Scenario::from_save(&patched)
        .unwrap_or_else(|e| panic!("{}: the import refused: {e}", f.label()))
        .kingdom(1);
    assert_eq!(
        k.counties[county].event_population_swing, figure,
        "{}: county {county}'s +0x2F8 holds {figure} and the loaded game does not carry it",
        f.label()
    );
}

/// **`+0x258` is `+0x268 − +0x26C − herdEaten` in every county of every save** —
/// the cattle row's *"Overall change"*, calf births expected, cow deaths expected,
/// and what the people ate (`docs/decisions.md` C128 for why the eating is in it).
///
/// Three fields that were dropped together by the importer, and one relation
/// that pins all three offsets at once: a wrong offset for any of them would have
/// to land on a word that happens to close this sum in every county of every
/// save. It is not vacuous — the England turn-one fixture's ten neutral counties
/// eat thirteen head each, so the eating term is exercised, and births differ
/// from deaths almost everywhere.
///
/// `tests/stored_fields.rs` is what holds the kingdom to these bytes; this is
/// what says the bytes are the fields.
#[test]
fn every_saved_cattle_forecast_is_births_less_deaths_less_what_was_eaten() {
    let (mut checked, mut eaten_counted) = (0usize, 0usize);
    for f in l2_testkit::saves!() {
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.i32_at(base + 0x24).unwrap() == 0 {
                continue;
            }
            let change = s.i32_at(base + 0x258).unwrap();
            let births = s.i32_at(base + 0x268).unwrap();
            let deaths = s.i32_at(base + 0x26C).unwrap();
            let eaten = s.i32_at(base + 0x17C).unwrap();
            assert_eq!(
                change,
                births - deaths - eaten,
                "{}: county {id} stores +0x258 {change}, and +0x268 {births} − +0x26C {deaths} − \
                 +0x17C {eaten} is {}",
                f.label(),
                births - deaths - eaten
            );
            checked += 1;
            eaten_counted += usize::from(eaten != 0);
        }
    }
    assert!(checked >= 14, "only {checked} counties reached");
    assert!(eaten_counted > 0, "no county anywhere ate cattle, so the eating term was never tested");
}

/// **`+0x22C` is `Grain_LabourEstimate`'s tail in every county of every save**:
/// `−sown − eaten` when the season is Spring, `harvest − eaten` in Winter,
/// `−eaten` otherwise, with `+0x230` the sowing and `crop[2]` the harvest.
///
/// **What the corpus can and cannot settle, said beside the assertion.** Every
/// save on this machine stores `+0x230 == 0` and `crop[2] == 0` in every county,
/// so the sowing and harvest arms are only checked at zero
/// `season` argument — `l2_kingdom::land::grain_preview` reads it as next season
/// — cannot be told from this season here. What *is* settled is that `+0x22C`
/// is minus the county's grain eaten wherever those arms are empty, in five
/// counties where that is not zero (`old_turn.sav` county 2 stores −73).
#[test]
fn every_saved_grain_forecast_is_its_seasons_tail() {
    let (mut checked, mut non_zero, mut arms) = (0usize, 0usize, 0usize);
    for f in l2_testkit::saves!() {
        let s = &f.save;
        let season_next = s.globals().unwrap().season_next;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.i32_at(base + 0x24).unwrap() == 0 {
                continue;
            }
            let change = s.i32_at(base + 0x22C).unwrap();
            let sown = s.i32_at(base + 0x230).unwrap();
            let harvest = s.i32_at(base + 0x248).unwrap();
            let eaten = s.i32_at(base + 0x178).unwrap();
            let tail = match season_next {
                1 => -sown - eaten,
                4 => harvest - eaten,
                _ => -eaten,
            };
            assert_eq!(
                change,
                tail,
                "{}: county {id} stores +0x22C {change}; next season {season_next}, sown {sown}, \
                 harvest {harvest}, eaten {eaten}",
                f.label()
            );
            checked += 1;
            non_zero += usize::from(change != 0);
            arms += usize::from(sown != 0 || harvest != 0);
        }
    }
    assert!(checked >= 14, "only {checked} counties reached");
    assert!(non_zero > 0, "every stored grain forecast is zero, so nothing was compared");
    eprintln!("{checked} grain forecasts, {non_zero} non-zero, {arms} with a sowing or a harvest");
}

/// **`+0x18` is `+0x1C / g_turnCount` in every county of every save** —
/// `Happiness_UpdateAll` (`0x0044BAEA`) banks the season's happiness into the sum
/// and divides by the turn count, into a signed byte.
///
/// The importer used to set both to this season's happiness, which this test
/// measures the cost of: it counts the counties where the stored average is not
/// the current happiness, which is every county past turn one whose mood has
/// moved — `siege-aftersie.sav` county 2 stores 54 and is at 95 today.
#[test]
fn the_happiness_average_is_the_running_sum_over_the_turn_count_in_every_save() {
    let (mut checked, mut differs) = (0usize, 0usize);
    for f in l2_testkit::saves!() {
        let s = &f.save;
        let turns = s.globals().unwrap().turn_count;
        assert!(turns > 0, "{}: turn count {turns}", f.label());
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.i32_at(base + 0x24).unwrap() == 0 {
                continue;
            }
            let avg = s.i8_at(base + 0x18).unwrap() as i32;
            let sum = s.i32_at(base + 0x1C).unwrap();
            assert_eq!(avg, (sum / turns) as i8 as i32, "{}: county {id}, sum {sum} over {turns} turns", f.label());
            checked += 1;
            differs += usize::from(avg != s.i8_at(base + 0x0C).unwrap() as i32);
        }
    }
    assert!(checked >= 14, "only {checked} counties reached");
    assert!(differs > 0, "the average equals the current happiness everywhere, so the old import passed too");
}

/// **`+0x5B` is set exactly where `+0x2C` reaches 6** — `Population_UpdateAll`
/// writes the change percentage and then attributes it only when it is at least
/// six (`docs/kingdom.md` §1.2). Both bytes are written in the same pass, so the
/// relation holds whatever happened to the population afterwards, so
/// this does not also check the percentage against the population.
///
/// The six is written as a literal
/// `l2_kingdom::county::CHANGE_REASON_MIN_PCT`: a probe computed from the constant
/// under test is `docs/agents.md`'s first way to ablate wrongly.
#[test]
fn a_population_change_reason_is_recorded_exactly_where_the_change_reaches_six_percent() {
    let (mut checked, mut reasons) = (0usize, 0usize);
    for f in l2_testkit::saves!() {
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.i32_at(base + 0x24).unwrap() == 0 {
                continue;
            }
            let pct = s.i32_at(base + 0x2C).unwrap();
            let reason = s.u8_at(base + 0x5B).unwrap();
            assert_eq!(reason != 0, pct >= 6, "{}: county {id}, change {pct}%, reason {reason}", f.label());
            assert!(reason <= 4, "{}: county {id}, reason {reason} names no L2.eng group 65 string", f.label());
            checked += 1;
            reasons += usize::from(reason != 0);
        }
    }
    assert!(checked >= 14 && reasons > 0, "{checked} counties, {reasons} with a reason");
}

/// **`+0x21` is set only in a county below thirty happiness**, which is what
/// identifies it as `Unrest_UpdateAll`'s warning latch (`0x0044AA41`: set when
/// happiness is under `0x1E` and the byte is clear) — the flag
/// `l2_kingdom::county::County::unrest_warned` described without an offset.
#[test]
fn the_unrest_warning_latch_is_set_only_in_a_county_below_thirty_happiness() {
    let mut set = 0usize;
    for f in l2_testkit::saves!() {
        let s = &f.save;
        for id in 1..17usize {
            let base = COUNTY_BASE + (id * COUNTY_STRIDE) as u32;
            if s.u8_at(base + 0x21).unwrap() == 0 {
                continue;
            }
            let happiness = s.i8_at(base + 0x0C).unwrap();
            assert!(happiness < 30, "{}: county {id} is warned at happiness {happiness}", f.label());
            set += 1;
        }
    }
    assert!(set > 0, "no save carries the latch, so nothing identified it");
}

