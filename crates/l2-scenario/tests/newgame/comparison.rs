#![allow(unused_imports)]
use super::*;
use super::build::*;
use l2_formats::maps::{MapSet, Plane, PLANE_DIM, SLOT_LEN};
use l2_kingdom::map::MAP_TILES;
use l2_scenario::newgame::{self, MapError, NewGame};
use l2_scenario::{CountyState, Scenario};

/// **A new game's mercenary bands, after its opening season, are England turn
/// one's, band for band.**
///
/// `Game_NewGame` is `Mercenary_Init(); … Season_Advance();` with no
/// `Mercenary_AdvanceAll` between them, and the save written after it still
/// holds every band at its start county with its countdown full. Our
/// `Kingdom::start_new_game` walks the same pipeline `Season_Advance` does, which
/// carries the phase-7 walk; this is the test that says it must not run there.
///
/// **Ablation, run:** take the `MercenaryAdvance` skip out of `start_new_game`
/// and the Saxon band, period 1, has already offered itself in county 14.
#[test]
fn a_new_games_bands_after_its_opening_season_are_england_turn_ones() {
    let save = l2_testkit::england!();
    let maps = maps!();
    let (from_save, from_map) = england_pair(&save, &maps);
    let mut k = from_map.kingdom(SEED);
    k.start_new_game();
    let saved = from_save.kingdom(SEED);
    assert_eq!(k.campaign.mercenaries.in_play(), 12);
    assert_eq!(k.campaign.mercenaries, saved.campaign.mercenaries, "the bands after the opening season");
    for id in 1..=k.county_count {
        assert_eq!(k.counties[id].mercenary_offer, saved.counties[id].mercenary_offer, "county {id}'s offer");
    }
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

    // --------------------------------------------- the colours and the lords
    //
    // **`Realms_AssignLords` against a game the original itself set up.** The
    // walk is otherwise checked only against itself — the reference
    // implementation in `l2-view`'s install tests reads the same tables and
    // does the same arithmetic — so this is the one place a save the *original
    // program wrote* says what the answer is. It can only settle the default
    // row of `docs/rules.md` §7a, because every `.sav` this project keeps has
    // the human on shield 1 or 5; the middle colours have no fixture and say
    // so at the table.
    for id in 1..l2_kingdom::realm::MAX_REALMS {
        assert_eq!(
            a.realms[id].shield_index, b.realms[id].shield_index,
            "realm {id} flies a different colour when the world is built from the map"
        );
        assert_eq!(
            a.realms[id].lord, b.realms[id].lord,
            "realm {id} has a different lord when the world is built from the map"
        );
    }

    // ------------------------------------------------------- the three planes

    // The county plane is copied verbatim by both paths and nothing rewrites
    // it, so this one is exact over all 4,096 tiles.
    assert_eq!(a.map.county, b.map.county, "the county plane");

    // **The bank plane, on the one question a rule asks it.**
    // `Map_ResolvePick` (`0x0046D5FE`) reads `(tile.bank & 0x1C) == 4` and
    // `TileInfo_Draw` (`0x0041C208`) has nothing else to tell a mountain from a
    // wood. Both paths agree tile for tile, and every mountain is rough.
    let mountains_save: Vec<usize> = (0..MAP_TILES).filter(|&t| a.map.is_mountain(t)).collect();
    let mountains_map: Vec<usize> = (0..MAP_TILES).filter(|&t| b.map.is_mountain(t)).collect();
    assert_eq!(mountains_save, mountains_map, "the Mtns bank");
    assert!(!mountains_save.is_empty(), "England has mountains");
    for &t in &mountains_save {
        assert_eq!(
            a.map.flags[t] & l2_kingdom::map::flags::ROUGH,
            l2_kingdom::map::flags::ROUGH,
            "tile {t}: the Mtns bank on a tile the flags do not call rough"
        );
    }

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
            // keyed on the castle level and is
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

    // **The mercenary bands, exactly.** `Game_NewGame` runs `Mercenary_Init` and
    // then `Season_Advance`, which does not walk them, so the save written after
    // it holds `Mercenary_Init`'s table untouched: twelve bands on a
    // fourteen-county map, each at its start county with its countdown equal to
    // its reload — including the Spanish and Angevin bands, whose start
    // counties 16 and 17 are not on this map at all. One reading of the file
    // and one of the roster, and they agree on all sixty fields.
    assert_eq!(b.mercenaries.in_play(), 12, "England gets all twelve bands");
    assert_eq!(a.mercenaries, b.mercenaries, "the band table is Mercenary_Init's");

    // ------------------------------- the five derived counts, on the kingdoms

    // `fields_*` are the one group the *scenario* cannot be compared on: the
    // save carries the file's cache and the map path carries nothing, because
    // `Scenario::skeleton` derives all five from the field tiles with
    // `field::recount` on both paths. So they are compared where they are
    // — on the kingdom.
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
    // field this file calls *agreed* is checked again where the rules
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
        // `+0x0F` and `+0x10`: the save carries the season's answers — `+0x0F`
        // is `5 - taxRate`, so 5 in all fourteen at rate 0 — and the map path
        // has not run a season yet. `docs/decisions.md` C142.
        ("d_hap_health", |c| c.d_hap_health as i64),
        ("d_hap_tax_local", |c| c.d_hap_tax_local as i64),
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
    // **The purse and the stall.** `County_Reset` zeroes all four and no
    // new-game path writes any of them, so a fresh county has no money and no
    // merchant; a played save has both, and they are the two things
    // `Ai_BuyGood` reads before it will let an unowned county buy food.
    judge("purse", |c| c.purse as i64, "Tax_CollectAll fills it only once a season has run");
    judge(
        "merchant_count",
        |c| c.merchant_count as i64,
        "County_RecountMerchants is season pass 22, so turn one has no stall anywhere",
    );
    judge("merchant_unit", |c| c.merchant_unit as i64, "written by the same pass");
    judge("merchant_visits", |c| c.merchant_visits as i64, "the same pass's lifetime counter");
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

    // **Both silent, and each one is a claim about the game
    // shrug.** These are zero in the England turn-one save *and* zero at new
    // game, and the save's zero is the original's own byte — so they are
    // corroborated.
    judge("tax_rate", |c| c.tax_rate as i64, "nothing sets a tax rate at new game: the county \
         record is zeroed by FUN_0046EA28 and County_Reset does not write one");
    judge("tax_collected", |c| c.tax_collected as i64, "no tax has been collected at rate 0");
    judge(
        "tax_shown",
        |c| c.tax_shown as i64,
        "`Pct(Pct(population, castleBase), 0)` is zero however large the county: the \
         preview is only non-zero once somebody sets a rate, and the turn-pair and siege \
         saves — which do carry rates 2, 3, 6 and 8 — are where it is checked instead",
    );
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

    // **C161**: what the save path now carries and a new game opens
    // at `County::new()`'s value. Grouped by the pass that writes them, because
    // that is the whole of why the two sides differ where they do.
    let forecast = "a forecast the estimate round writes at a season's end, which the save has \
                    had and a new game has not; the grain and reclamation rows are zero in the \
                    save as well, because nobody has sown or reclaimed by turn one";
    for (field, f) in [
        ("herd_change_expected", (|c: &CountyState| c.herd_change_expected as i64) as fn(&CountyState) -> i64),
        ("herd_births_expected", |c| c.herd_births_expected as i64),
        ("herd_deaths_expected", |c| c.herd_deaths_expected as i64),
        ("grain_change_expected", |c| c.grain_change_expected as i64),
        ("grain_sown_expected", |c| c.grain_sown_expected as i64),
        ("grain_grown_expected", |c| c.grain_grown_expected as i64),
        ("reclaim_fields_finishing", |c| c.reclaim_fields_finishing as i64),
        ("reclaim_seasons_to_next", |c| c.reclaim_seasons_to_next as i64),
    ] {
        judge(field, f, forecast);
    }
    let figures = "what last season's weather and event did to the grain and the herd; a new game \
                   has had no season, and England turn one's first stood in Cloudy with no event";
    for (field, f) in [
        ("grain_weather_change", (|c: &CountyState| c.grain_weather_change as i64) as fn(&CountyState) -> i64),
        ("grain_event_change", |c| c.grain_event_change as i64),
        ("herd_weather_change", |c| c.herd_weather_change as i64),
        ("herd_event_change", |c| c.herd_event_change as i64),
    ] {
        judge(field, f, figures);
    }
    let mood = "the happiness pass's and the tax preview's outputs, which the save's season wrote \
                and a new game's has not; the army, ale, other-counties and warning terms are \
                zero in the save too, because nothing raised, bought or taxed past 19% by turn one";
    for (field, f) in [
        ("happiness_avg", (|c: &CountyState| c.happiness_avg as i64) as fn(&CountyState) -> i64),
        ("happiness_sum", |c| c.happiness_sum as i64),
        ("d_hap_tax", |c| c.d_hap_tax as i64),
        ("shown_army", |c| c.shown_army as i64),
        ("tax_hap_other", |c| c.tax_hap_other as i64),
        ("shown_ale", |c| c.shown_ale as i64),
        ("ale_happiness_given", |c| c.ale_happiness_given as i64),
        ("unrest_warned", |c| c.unrest_warned as i64),
    ] {
        judge(field, f, mood);
    }
    let people = "Population_UpdateAll's and Migration_UpdateAll's outputs, which the save's \
                  season wrote; nobody has moved or been levied by turn one, so the migration \
                  and army bytes are zero on both sides";
    for (field, f) in [
        ("pop_change_pct", (|c: &CountyState| c.pop_change_pct as i64) as fn(&CountyState) -> i64),
        ("change_reason", |c| c.change_reason as i64),
        ("army", |c| c.army as i64),
        ("largest_inflow", |c| c.largest_inflow as i64),
        ("inflow_sources", |c| c.inflow_sources.iter().map(|&n| n as i64).sum()),
        ("emigrant_destination", |c| c.emigrant_destination as i64),
        ("largest_inflow_source", |c| c.largest_inflow_source as i64),
    ] {
        judge(field, f, people);
    }
    for (field, f) in [
        ("event_fired", (|c: &CountyState| c.event_fired as i64) as fn(&CountyState) -> i64),
        ("event_id", |c| c.event_id as i64),
        ("event_population_pct", |c| c.event_population_pct as i64),
        ("event_population_swing", |c| c.event_population_swing as i64),
        ("event_grain_pct", |c| c.event_grain_pct as i64),
        ("event_herd_pct", |c| c.event_herd_pct as i64),
        ("tax_suppressed", |c| c.tax_suppressed as i64),
    ] {
        judge(field, f, "no random event has fired by turn one");
    }
    for (field, f) in [
        ("field_progress", (|c: &CountyState| c.field_progress.iter().map(|&n| n as i64).sum())
            as fn(&CountyState) -> i64),
        ("crop", |c| c.crop.iter().map(|&n| n as i64).sum()),
        ("fields_grain_sown", |c| c.fields_grain_sown as i64),
        ("fields_grain_standing", |c| c.fields_grain_standing as i64),
        ("sow_shortfall", |c| c.sow_shortfall as i64),
        // The two field cursors: `County_Reset` zeroes both and no sweep has
        // run by turn one.
        ("pasture_cursor", |c| c.pasture_cursor as i64),
        ("blight_cursor", |c| c.blight_cursor as i64),
    ] {
        judge(field, f, "nobody has reclaimed a field or sown grain by turn one");
    }
    judge("friendly_troops", |c| c.friendly_troops as i64, "no army stands in a county at turn one");
    judge("enemy_troops", |c| c.enemy_troops as i64, "no army stands in a county at turn one");
    judge("levy_surcharge", |c| c.levy_surcharge as i64, "no levy has been raised by turn one");
    for (field, f) in [
        ("castle_degraded", (|c: &CountyState| c.castle_degraded as i64) as fn(&CountyState) -> i64),
        ("castle_ruined", |c| c.castle_ruined as i64),
        ("castle_level_left", |c| c.castle_level_left as i64),
        ("castle_percent", |c| c.castle_percent as i64),
        ("castle_work_left", |c| c.castle_work_left as i64),
        ("castle_work_total", |c| c.castle_work_total as i64),
        ("castle_stone_owed", |c| c.castle_stone_owed as i64),
        ("castle_stone_total", |c| c.castle_stone_total as i64),
        ("castle_wood_owed", |c| c.castle_wood_owed as i64),
        ("castle_wood_total", |c| c.castle_wood_total as i64),
        ("siege_scars", |c| {
            let s = &c.siege_scars;
            s.moat_filled as i64 + s.wall_damage as i64 + s.breach_score as i64
                + s.approach_score as i64 + s.ramparts_breached as i64 + s.gate_open as i64
        }),
    ] {
        judge(field, f, "no castle is being built, repaired or besieged at turn one");
    }
    judge(
        "weapon_type",
        |c| c.weapon_type as i64,
        "the save's counties each carry the weapon their blacksmith makes; a new game opens at \
         County::new's 0, and which pass first chooses it is not traced here",
    );
    judge(
        "mercenary_offer",
        |c| c.mercenary_offer as i64,
        "Mercenary_AdvanceAll is turn phase 7's, so a band cannot be on offer before turn one ends",
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
            // **And this row is the whole of how the neutral counties came to
            // starve.** `purse` is zero on both sides *of turn one* — nothing
            // has been banked yet, because `Tax_CollectAll` runs at the end of
            // a season and turn one has not had one. `County::purse`'s own
            // comment generalised exactly this observation to *"it is 0 in
            // every fixture"*, and the turn pair carries 186 … 436. A field
            // that is inert in the only save a check reads is not an inert
            // field; it is an unread one.
            "purse",
            "tax_collected",
            "tax_rate",
            "tax_shown",
            "unrest",
            // **C161**, and these are the rows
            // `tests/stored_fields.rs` also names as measured only against
            // zero: nothing in England turn one has built, besieged, sown,
            // reclaimed, levied, bought ale, drawn an event or moved house.
            "ale_happiness_given",
            "army",
            // Neither field cursor has been stepped: no cattle bought, no
            // county blighted.
            "pasture_cursor",
            "blight_cursor",
            "castle_degraded",
            "castle_level_left",
            "castle_percent",
            "castle_ruined",
            "castle_stone_owed",
            "castle_stone_total",
            "castle_wood_owed",
            "castle_wood_total",
            "castle_work_left",
            "castle_work_total",
            "crop",
            "emigrant_destination",
            "enemy_troops",
            "event_fired",
            "event_grain_pct",
            "event_herd_pct",
            "event_id",
            "event_population_pct",
            "event_population_swing",
            "field_progress",
            "fields_grain_sown",
            "fields_grain_standing",
            "friendly_troops",
            "grain_change_expected",
            "grain_grown_expected",
            "grain_sown_expected",
            "inflow_sources",
            "largest_inflow",
            "largest_inflow_source",
            "levy_surcharge",
            "reclaim_fields_finishing",
            "reclaim_seasons_to_next",
            "shown_ale",
            "shown_army",
            "siege_scars",
            "sow_shortfall",
            "tax_hap_other",
            "tax_suppressed",
            "unrest_warned",
            // No band has walked by turn one, and no weather or event has
            // touched a crop or a herd in England turn one's first season.
            "mercenary_offer",
            "grain_weather_change",
            "grain_event_change",
            "herd_weather_change",
            "herd_event_change",
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>(),
        "the set of fields neither constructor writes has changed"
    );
}

// ------------------------------------------------ the enumeration, from source

