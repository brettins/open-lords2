//! **The field counts, against the tiles they were counted from.**
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-kingdom --test fields
//! ```
//!
//! # What this is evidence about
//!
//! `l2_kingdom::field::classify` is a ladder of six terrain boundaries taken
//! off `County_RecountFields` (`FUN_00469B8D`, `0x00469B8D`) in the
//! decompiler. `docs/decisions.md` C3 is what happens when a plausible ladder
//! is believed because it is tidy, and C24 is what happens when a
//! hand-transcribed table is never read back.
//!
//! The England turn-one save closes that loop **without any of our rules being
//! involved**, because it stores both halves of the sum. `g_countyFieldTiles`
//! names the twenty tiles, `g_tiles` holds each one's terrain byte, and county
//! `+0x1FF`, `+0x200` and `+0x201` hold the three counts the game itself made
//! of them. Applying our ladder to the first and comparing against the third is
//! a check nothing in this tree can make come out right by agreeing with
//! itself: the file was written by the original.
//!
//! # And it is the reason the brush exists
//!
//! Every one of the fourteen counties has `fieldsGrain = 0`. That is not a
//! quirk of this position — it is what the start of a game *is*: you paint your
//! fields. Until [`l2_kingdom::field`] there was no code path in this tree that
//! could set that number for the human player at all, so the entire grain half
//! of the economy was finished, tested and unreachable in play.

use l2_kingdom::field::FieldType;
use l2_kingdom::{Kingdom, MAX_FIELDS};
use l2_scenario::Scenario;
use l2_testkit::england;

/// The ladder, against the save's own arithmetic, on every county.
///
/// The `[V]` this earns is precise: 168 field tiles across fourteen counties,
/// three counts each, every one of them the file's.
#[test]
fn the_terrain_ladder_reproduces_every_county_s_stored_field_counts() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let kingdom = scenario.kingdom(1);

    let mut tiles_seen = 0;
    for id in scenario.county_ids() {
        let stored = scenario.counties[id].as_ref().expect("a county");
        let ours = &kingdom.counties[id];
        tiles_seen += ours.field_slots_used();
        assert_eq!(ours.fields_fallow, stored.fields_fallow, "county {id} fallow");
        assert_eq!(ours.fields_cattle, stored.fields_cattle, "county {id} pasture");
        assert_eq!(ours.fields_grain, stored.fields_grain, "county {id} grain");
    }
    assert_eq!(tiles_seen, 168, "the England map's fourteen counties own 168 field tiles");
}

/// **The premise.** Nobody starts with a grain field, so the brush is the only
/// way the player's economy ever begins.
///
/// A scenario value, not an invariant (`docs/decisions.md` C23): it is a fact
/// about how this game opens, and it is asserted here so that a future position
/// which *does* ship sown fields fails this and gets read rather than silently
/// changing what the rest of the file means.
#[test]
fn no_county_of_the_england_position_has_a_single_grain_field() {
    let save = england!();
    let kingdom = Scenario::from_save(&save).expect("import").kingdom(1);
    for id in 1..=kingdom.county_count {
        assert_eq!(kingdom.counties[id].fields_grain, 0, "county {id}");
        assert!(kingdom.counties[id].field_slots_used() >= 8, "county {id} has fields to paint");
    }
}

/// Every field tile a county claims is inside its own county on the map, and no
/// two counties claim the same tile.
///
/// This is the *map's* half of the same reading: if the byte-offset-to-index
/// conversion in `l2-scenario` were out by a factor of eight, or the county
/// plane were being read at the wrong byte of the eight-byte tile record, the
/// tiles would land in other counties and this would say so. Neither could be
/// caught by the counts alone, because a wrong tile still has *a* terrain byte.
#[test]
fn every_field_tile_lies_in_its_own_county_and_belongs_to_nobody_else() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let kingdom = scenario.kingdom(1);
    let map = &kingdom.campaign.map;

    let mut owner = vec![0usize; map.terrain.len()];
    for id in scenario.county_ids() {
        for slot in 0..MAX_FIELDS {
            let Some(tile) = kingdom.counties[id].field_tile(slot) else { continue };
            assert_eq!(
                map.county[tile] as usize, id,
                "county {id}'s field {slot} is on tile {tile}, which the map gives to county {}",
                map.county[tile]
            );
            assert_eq!(owner[tile], 0, "tile {tile} is claimed by counties {} and {id}", owner[tile]);
            owner[tile] = id;
        }
    }
}

/// **The end-to-end one.** A human player paints a field to grain, ends four
/// turns, and the grain pipeline runs: sown, grown, harvested.
///
/// This is the first time this engine has driven its own economy from the
/// player's side. It is deliberately built on the real position rather than on
/// a fabricated county, because `docs/decisions.md` C26 is what happens when
/// the only fixture exercises one value of a rule's input.
#[test]
fn a_player_can_paint_a_field_to_grain_and_harvest_it_four_seasons_later() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut k = scenario.kingdom(1);

    // The human's county. `g_localPlayer` is realm 1 and which counties it
    // holds is rolled per game, so it is found rather than named (C23).
    let human = scenario.local_player;
    let mine = (1..=k.county_count)
        .find(|&id| k.counties[id].owner == human)
        .expect("the local player holds a county");

    // Seed grain, **before** painting. The five owned counties of this
    // position store none at all (C20 — the file records no *opening* store),
    // and the order matters for a reason worth stating: the grain ceiling the
    // brush computes is a search over `Grain_Sow`, which asks whether the
    // store can afford the seed. A county with an empty granary is told it has
    // no use for a farmer, and none is assigned.
    k.counties[mine].grain = 10_000;
    let before = k.counties[mine].grain;

    // Paint every fallow field this county has to grain.
    let painted = paint_all_fallow_to_grain(&mut k, mine);
    assert!(painted > 0, "county {mine} had fields to paint");
    assert_eq!(k.counties[mine].fields_grain, painted, "and they are grain now");
    assert!(
        k.counties[mine].labour[0] > 0,
        "and painting put farmers on them: {:?}",
        k.counties[mine].labour
    );

    let sown = k.advance_season();
    assert!(
        k.counties[mine].grain < before,
        "sowing debits the seed: {} -> {}",
        before,
        k.counties[mine].grain
    );
    let crop_after_sowing: i32 = k.counties[mine].crop.iter().sum();
    assert!(crop_after_sowing > 0, "and it puts a crop in the ground: {sown:?}");

    // Spring and Summer grow it; Autumn harvests it into the store.
    let mut low = k.counties[mine].grain;
    for _ in 0..3 {
        k.advance_season();
        low = low.min(k.counties[mine].grain);
    }
    assert_eq!(k.season, 4, "back to Winter");
    assert!(
        k.counties[mine].grain > low,
        "the harvest put grain back in the store: low {low}, now {}",
        k.counties[mine].grain
    );
    // **The crop words are not cleared by the harvest**, and this test used to
    // say they were. `crop` is *seed, standing crop, harvest* rather than three
    // growth stages: `Grain_SeasonTick` clears `crop[2]` at the top of every
    // season and fills it at the harvest, and `crop[0]` and `crop[1]` keep the
    // year's record until the next sowing overwrites them. See
    // [`l2_kingdom::land`].
    assert!(
        k.counties[mine].crop[2] > 0,
        "the harvest is the third word: {:?}",
        k.counties[mine].crop
    );
    assert_eq!(
        k.counties[mine].crop[0] * l2_kingdom::tables::GRAIN_YIELD_PER_SACK,
        k.counties[mine].crop[1],
        "and the first two are still the seed and the crop it became"
    );
}

/// The same painting, done twice, is the same kingdom — the brush is a rule and
/// rules are deterministic (`docs/netcode.md`).
#[test]
fn painting_is_deterministic() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let run = || {
        let mut k = scenario.kingdom(7);
        let mine = (1..=k.county_count)
            .find(|&id| k.counties[id].owner == scenario.local_player)
            .expect("a county");
        paint_all_fallow_to_grain(&mut k, mine);
        k.counties[mine].grain = 10_000;
        for _ in 0..4 {
            k.advance_season();
        }
        k
    };
    assert!(run() == run(), "two identical playthroughs diverged");
}

/// Paint every fallow field of one county to grain, the way the player does it:
/// one tile, one brush stroke. Returns how many strokes landed.
fn paint_all_fallow_to_grain(k: &mut Kingdom, county: usize) -> i32 {
    let tiles: Vec<usize> = k
        .field_tiles(county)
        .into_iter()
        .filter(|&(_, kind)| kind == FieldType::Fallow)
        .map(|(tile, _)| tile)
        .collect();
    let mut painted = 0;
    for tile in tiles {
        k.paint_field(county, tile, FieldType::Grain)
            .expect("a fallow field takes the grain brush");
        painted += 1;
    }
    painted
}

// ------------------------------------------------------- the cattle on the map

/// **The pasture picture is the herd count, and the save proves it.**
///
/// `Herd_UpdateCrowding` (`0x0044D913`) writes one of `0x13 … 0x16` onto every
/// pasture tile of a county, chosen by `herd / fieldsCattle` banded at 11 and
/// 21, and `FUN_004071A0` draws a different number of animals for each. So the
/// terrain byte on a pasture tile is not a graphic — it is the herd meter,
/// stored where the renderer can see it.
///
/// This is the same shape as the ladder test above and earns the same `[V]`:
/// **the England turn-one save carries both halves and neither is ours.** The
/// file holds `county.herd`, it holds the twenty field tiles' terrain bytes,
/// and `l2_kingdom::land::herd_graphic` has to reproduce the second from the
/// first. Nothing in this tree can make that come out right by agreeing with
/// itself.
///
/// The position exercises **three** of the four states — county 1 grazes 74
/// head on one field (`0x16`), the eight-field counties at 67 head sit at
/// density 8 (`0x14`) and the 93/101/110-head ones at 11 and 12 (`0x15`). The
/// empty-herd state `0x13` does not occur at turn one and is covered by
/// [`the_map_empties_when_the_herd_does`] instead.
#[test]
fn every_county_s_pasture_carries_the_picture_its_herd_calls_for() {
    let save = england!();
    let kingdom = Scenario::from_save(&save).expect("import").kingdom(1);
    let tables = l2_kingdom::tables::Tables::DEFAULT;

    let mut seen = std::collections::BTreeSet::new();
    let mut tiles = 0;
    for id in 1..=kingdom.county_count {
        let c = &kingdom.counties[id];
        let want = l2_kingdom::land::herd_graphic(&tables, c.herd, c.fields_cattle);
        for slot in 0..MAX_FIELDS {
            let Some(tile) = c.field_tile(slot) else { continue };
            let got = kingdom.campaign.map.terrain[tile];
            if l2_kingdom::field::classify(got) != FieldType::Pasture {
                continue;
            }
            assert_eq!(
                got, want,
                "county {id}: {} head on {} fields is density {}, so the game drew {want:#04x} \
                 and we would have drawn {got:#04x}",
                c.herd,
                c.fields_cattle,
                c.herd / c.fields_cattle.max(1),
            );
            seen.insert(got);
            tiles += 1;
        }
    }
    assert_eq!(tiles, 107, "the England position has 107 pasture tiles");
    assert_eq!(
        seen,
        [0x14u8, 0x15, 0x16].into_iter().collect(),
        "three of the four stocking states occur at turn one",
    );
}

/// **The graphic is not the crowding meter, and the position proves the
/// difference matters.**
///
/// `herd_crowding` bands at 11, 21 and 31 into 10/20/30/40; `herd_graphic`
/// bands at 11 and 21 only. County 1 grazes 74 head on one field — density 74,
/// which is meter band **40** and graphic `0x16`, the *third* of three. A
/// renderer that indexed three pictures by `herd_crowding / 10` would run off
/// the end of its table on the very first county of the shipped position.
#[test]
fn the_map_merges_the_top_two_crowding_bands_and_the_meter_does_not() {
    let save = england!();
    let kingdom = Scenario::from_save(&save).expect("import").kingdom(1);
    let tables = l2_kingdom::tables::Tables::DEFAULT;
    let c = &kingdom.counties[1];
    assert_eq!(c.fields_cattle, 1, "county 1 grazes everything on one field");
    assert_eq!(c.herd_crowding, 40, "which is the top meter band");
    assert_eq!(
        l2_kingdom::land::herd_graphic(&tables, c.herd, c.fields_cattle),
        0x16,
        "and the top *picture*, which is also band 30's",
    );
    // The two bands that share a picture, stated directly.
    assert_eq!(l2_kingdom::land::herd_crowding(&tables, 25, 1), 30);
    assert_eq!(l2_kingdom::land::herd_crowding(&tables, 250, 1), 40);
    assert_eq!(l2_kingdom::land::herd_graphic(&tables, 25, 1), 0x16);
    assert_eq!(l2_kingdom::land::herd_graphic(&tables, 250, 1), 0x16);
}

/// **A county that loses its herd loses its cattle**, and the season pass is
/// what does it.
///
/// `docs/agents.md`: *"a field is only tested if something a test reads was
/// written by something the game runs."* So this does not set a terrain byte
/// and read it back — it kills the herd and runs [`Kingdom::advance_season`], and the
/// pasture on the map has to follow. `Herd_UpdateCrowding` is called from
/// `Herd_SeasonTick`'s last line and that is the road being travelled.
#[test]
fn the_map_empties_when_the_herd_does() {
    let save = england!();
    let mut kingdom = Scenario::from_save(&save).expect("import").kingdom(1);
    let id = 2; // eight pasture fields, 67 head, drawn 0x14
    let pasture: Vec<usize> = (0..MAX_FIELDS)
        .filter_map(|s| kingdom.counties[id].field_tile(s))
        .filter(|&t| {
            l2_kingdom::field::classify(kingdom.campaign.map.terrain[t]) == FieldType::Pasture
        })
        .collect();
    assert_eq!(pasture.len(), 8, "county 2's eight pastures");
    assert!(pasture.iter().all(|&t| kingdom.campaign.map.terrain[t] == 0x14));

    kingdom.counties[id].herd = 0;
    kingdom.advance_season();

    for &t in &pasture {
        assert_eq!(
            kingdom.campaign.map.terrain[t], 0x13,
            "an empty herd leaves bare pasture, and l2_view draws no animals on it",
        );
    }
    // The other half of that claim — that `0x13` draws nothing — belongs to
    // `l2-view` and is asserted in `crates/l2-game/tests/screens.rs`, which is
    // the lowest crate that can see both sides. This one must not reach for
    // `l2-view`: nothing in the simulation may depend on the renderer.
}

/// **The cattle forecast moves when an input to it moves.**
///
/// A player: *"I right now have −11 cattle. If I move it so the people are
/// eating cattle, it still says −11 cattle in the sidebar."* The figure was
/// never wrong — `Herd_LabourEstimate`'s tail writes
/// `(births − deaths) − herdEaten`, so slaughter **is** in it, and `L2.eng`
/// group 77 index 28 calls it *"Overall change"*. What was wrong is *when*:
/// the tail ran only from `herd_season_tick`, so no control could move it.
///
/// The original calls `Herd_LabourEstimate` from **both** `Herd_SeasonTick`'s
/// last line and `County_RefreshEstimates`; ours now does too.
///
/// **The assertion is the player's own diagnostic** — change an input the
/// figure depends on and require the figure to change — which is the signal
/// that found this, the tax panel's stuck number and the ration panel's before
/// it. It deliberately does not assert a *value*: a test that pinned −11 would
/// pass just as well with the forecast frozen, which is the whole defect.
///
/// Ablating the `herd_preview` call in `field::refresh_estimates` fails it.
#[test]
fn the_cattle_forecast_follows_the_labour_it_depends_on() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut k = scenario.kingdom(1);
    let cattle = l2_kingdom::tables::JOB_CATTLE_FARMING;
    let county = k
        .county_ids()
        .find(|&id| k.counties[id].fields_cattle > 0 && k.counties[id].herd > 0)
        .expect("a county with a herd");

    // Staff the dairy fully and record the forecast.
    k.counties[county].labour[cattle] = k.counties[county].herd * 3;
    k.refresh_estimates(county);
    let staffed = k.counties[county].herd_change_expected;

    // Take every hand off it. Understaffing is added to the death rate — at
    // zero staffing the band's 1 becomes 1 + 33 — so the forecast must fall.
    k.counties[county].labour[cattle] = 0;
    k.refresh_estimates(county);
    let bare = k.counties[county].herd_change_expected;

    assert_ne!(
        staffed, bare,
        "the forecast did not move when the dairy was emptied: {staffed} both times",
    );
    assert!(
        bare < staffed,
        "an unstaffed herd should forecast worse than a fully staffed one: {staffed} -> {bare}",
    );
    eprintln!("county {county}: herd {} forecasts {staffed} staffed, {bare} bare",
        k.counties[county].herd);
}

/// **The reclamation row's two figures, and what the "+1" counts.**
///
/// A player: *"the figure is missing in the sidebar — it draws the serf
/// reclaiming, but not the +1 I'm used to."* `Field_ReclaimEstimate`
/// (`0x0044C278`) is a work-outstanding loop **plus a tail** that simulates the
/// coming season, and only the loop was ported. The tail's `+0x20C` counts
/// **fields that will be finished next season** — fields, not units of work,
/// which is the thing a small integer could plausibly have been either of.
///
/// Three claims, and each is a different line of the tail:
///
/// 1. **a field one season's work from done finishes** — one field, one gang;
/// 2. **the gang starts on the nearest-to-finished field**, so a field at 600
///    completes before a field at 0 gets touched;
/// 3. **a finished field hands its surplus on**, so a gang with enough labour
///    finishes two in a season — which is the only way the figure ever reads
///    more than 1, and the reason it is a simulation rather than a division.
#[test]
fn the_reclamation_forecast_counts_fields_finished_not_work_done() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut k = scenario.kingdom(1);
    let job = l2_kingdom::tables::JOB_FIELD_RECLAMATION;
    let per_season = k.tables.field.reclaim_per_season;
    let full = k.tables.field.progress_max;

    let county = k.county_ids().find(|&id| k.counties[id].field_slots_used() >= 2).expect("fields");
    // Two fields under reclamation: one nearly done, one untouched.
    let slots: Vec<usize> =
        (0..l2_kingdom::MAX_FIELDS).filter(|&s| k.counties[county].field_tile(s).is_some()).collect();
    let (near, far) = (slots[0], slots[1]);
    for &s in &[near, far] {
        let tile = k.counties[county].field_tile(s).expect("a tile");
        k.campaign.map.terrain[tile] = l2_kingdom::field::terrain::RECLAIM_FIRST;
    }
    k.counties[county].field_progress[near] = (full - per_season) as u16;
    k.counties[county].field_progress[far] = 0;

    // 1 and 2 — one gang's worth of labour finishes the near field only.
    k.counties[county].labour[job] = per_season;
    k.refresh_estimates(county);
    assert_eq!(
        k.counties[county].reclaim_fields_finishing, 1,
        "one season's work on the nearest-to-finished field completes it and nothing else",
    );
    assert_eq!(
        k.counties[county].reclaim_seasons_to_next, 1,
        "and it is one season away",
    );

    // 3 — enough for both, and the near field's surplus carries to the far one.
    // The far field needs a full 800, so this is deliberately generous: what is
    // being asserted is that the count can exceed 1 at all.
    k.counties[county].field_progress[far] = (full - per_season) as u16;
    k.counties[county].labour[job] = per_season * 2;
    k.refresh_estimates(county);
    assert_eq!(
        k.counties[county].reclaim_fields_finishing, 2,
        "two gangs' worth finishes two fields, which is why this is a count and not a flag",
    );

    // And nothing being reclaimed forecasts nothing, rather than keeping the
    // last answer — the original zeroes both before its guard.
    for &s in &[near, far] {
        let tile = k.counties[county].field_tile(s).expect("a tile");
        k.campaign.map.terrain[tile] = l2_kingdom::field::terrain::FALLOW;
    }
    k.refresh_estimates(county);
    assert_eq!(k.counties[county].reclaim_fields_finishing, 0);
    assert_eq!(k.counties[county].reclaim_seasons_to_next, 0);
}
