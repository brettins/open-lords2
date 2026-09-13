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
/// which *does* ship sown fields fails this and gets read
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
/// player's side. It is deliberately built on the real position
/// a fabricated county, because `docs/decisions.md` C26 is what happens when
/// the only fixture exercises one value of a rule's input.
#[test]
fn a_player_can_paint_a_field_to_grain_and_harvest_it_four_seasons_later() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut k = scenario.kingdom(1);

    // The human's county. `g_localPlayer` is realm 1 and which counties it
// holds is rolled per game, so it is found (C23).
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
// say they were. `crop` is *seed, standing crop, harvest*
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
/// terrain byte on a pasture tile is the herd meter,
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
///    more than 1, and the reason it is a simulation.
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

// And nothing being reclaimed forecasts nothing,
    // last answer — the original zeroes both before its guard.
    for &s in &[near, far] {
        let tile = k.counties[county].field_tile(s).expect("a tile");
        k.campaign.map.terrain[tile] = l2_kingdom::field::terrain::FALLOW;
    }
    k.refresh_estimates(county);
    assert_eq!(k.counties[county].reclaim_fields_finishing, 0);
    assert_eq!(k.counties[county].reclaim_seasons_to_next, 0);
}

/// **The grain row's forecast, driven the way a player produces it.**
///
/// A player: *"Sidebar doesn't show grain being planted as a negative
/// number."* `Grain_LabourEstimate` (`0x0044D374`) is a search loop **plus a
/// tail**, and the tail is what writes county `+0x22C`; only the loop was
/// ported, and `docs/decisions.md` C123 is the repair.
///
/// **The repair landed and had no test.** Its two siblings each got one —
/// [`the_cattle_forecast_follows_the_labour_it_depends_on`] and
/// [`the_reclamation_forecast_counts_fields_finished_not_work_done`] — and the
/// one the player reported did not, so this exists.
///
/// It is deliberately driven from the **brush and the season**
/// writing the county's fields: `docs/agents.md`'s *a field is only tested if
/// something a test reads was written by something the game runs*. Every input
/// here is something a player does.
///
/// Three claims, one per arm of the tail's ladder (`season` is `g_seasonNext`):
///
/// ```c
/// if      (season == 1) county.field_0x22C = -county.field_0x230 - county.grainEaten;
/// else if (season == 4) county.field_0x22C =  county.crop[2]     - county.grainEaten;
/// else                  county.field_0x22C = -county.grainEaten;
/// ```
///
/// 1. **Facing Spring the row is a loss and cannot be anything else** — sowing
///    spends the store, so the forecast for the season about to begin is
///    negative. That is the player's sentence with the arithmetic under it.
/// 2. **It is exactly `-sown - eaten`**, and `sown` is the tail's own
///    `Grain_Sow(county, staff, grain)`
///    `Grain_Sow(county, workers, grain - grainEaten)` — a different third
///    argument, so the number cannot be recovered from the ceiling.
/// 3. **Facing Winter it is the harvest less the eating**, so the same row
///    turns positive once there is a crop to bring in. A test that only ever
///    looked at Spring would pass with the other two arms deleted.
///
/// Ablation, run: deleting the `crate::land::grain_preview` call from
/// `field::refresh_estimates` fails claims 1 and 3, at `0` both times.
#[test]
fn the_grain_forecast_is_the_sowing_loss_the_player_reported() {
    let save = england!();
    let scenario = Scenario::from_save(&save).expect("import");
    let mut k = scenario.kingdom(1);
    let mine = (1..=k.county_count)
        .find(|&id| k.counties[id].owner == scenario.local_player)
        .expect("the local player holds a county");

    // Seed the granary before painting, for the reason
    // `a_player_can_paint_a_field_to_grain_and_harvest_it_four_seasons_later`
    // states: an empty store is told it has no use for a farmer.
    k.counties[mine].grain = 10_000;
    assert!(paint_all_fallow_to_grain(&mut k, mine) > 0, "county {mine} had fields to paint");

    // The fixture opens facing Spring, which is the sowing turn and the one the
    // player was looking at.
    assert_eq!(k.season_next, 1, "the England position faces Spring");
    k.refresh_estimates(mine);
    let c = &k.counties[mine];
    let (sown, eaten, shown) = (c.grain_sown_expected, c.grain_eaten, c.grain_change_expected);
    assert!(sown > 0, "farmers on grain fields forecast a sowing: {sown}");
    // 1 — the sign, which is the whole report.
    assert!(shown < 0, "facing Spring the grain row is a loss, not {shown}");
    // 2 — and it is the tail's arithmetic, not something that resembles it.
    assert_eq!(shown, -sown - eaten, "Spring is `-sown - eaten`");

    // 3 — the other end of the year. Run the crop round to Winter and the same
    // row becomes the harvest less the eating.
    for _ in 0..3 {
        k.advance_season();
    }
    assert_eq!(k.season_next, 4, "facing Winter, the harvest turn");
    k.refresh_estimates(mine);
    let c = &k.counties[mine];
    assert!(c.crop[2] > 0, "there is a harvest to forecast: {:?}", c.crop);
    assert_eq!(
        c.grain_change_expected,
        c.crop[2] - c.grain_eaten,
        "facing Winter the row is `harvest - eaten`",
    );
    assert!(
        c.grain_change_expected > 0,
        "and a county with a crop in the ground forecasts a gain, not {}",
        c.grain_change_expected,
    );
}

/// **Why more milkmaids stop helping**, which is the half of the player's
/// cattle question that is not about the sidebar at all.
///
/// > *"I had lots of milk maids with low herd crowding and we were only getting
/// > 1 cow, and if I added more milk maids they were idle."*
///
/// Both halves are `Herd_LabourEstimate` (`0x0044DD4D`), whose search loop is
/// the dairy's ceiling: the **fewest** workers that reach the best
/// `births - deaths`, because the test inside it is a strict `<`. The sidebar
/// rings the cow the moment `labour > useful`, so *idle* is that ceiling being
/// hit.
///
/// `l2_kingdom::land::herd_labour_estimate` documents the closed form as
/// *"about `6 * herd`"* and marks it **`[I]`**. This measures it, and finds the
/// inference true as a **bound** and wrong as an estimate for exactly the case
/// the player was in:
///
/// 1. **The ceiling never exceeds six a head** — twice
///    [`l2_kingdom::tables::HERD_LABOUR_PER_HEAD`], which is where
///    `PctOf(labour, herd * 3)` reaches its 200% cap and births stop rising.
/// 2. **For a small herd it is three a head, not six.** A herd of five gets
///    `+5000` on its birth rate the moment staffing reaches 100%, and
///    `herd * birthRate / 10000` then rounds to the same integer at 100% as at
///    200% — so the argmax is the *first* of the two, and every milkmaid past
///    three a head is idle. That is the player's county.
/// 3. **The season moves the answer**, which is the question he asked outright:
///    Spring multiplies the births by `3/2` and Winter the deaths.
///
/// Nothing here is asserted against a typed constant: the counts come out of
/// [`l2_kingdom::land::herd_growth`], which `docs/kingdom.md` §13 reproduces
/// instruction for instruction.
///
/// Two ablations, both run, and the first one's *predicted* failure was wrong
/// in a way worth keeping:
///
/// * Relaxing the search's `best < net` to `best <= net` makes it take the
///   **last** argmax instead of the first. Claim 2 was expected to fail at six
///   a head; what fails is **claim 1**, at `ceiling 9999` for a herd
///   of one — the last argmax is the end of the scan, not `6 * herd`. The
///   ablation found the right defect for a reason one step away from the one
///   written down, so the note says what happened
///   was expected.
/// * Disabling the `staffing >= 100` small-herd bonus fails **claim 2**
///   directly: the herd of five's ceiling drops from 15 to 7.
#[test]
fn the_dairy_ceiling_is_the_fewest_milkmaids_that_reach_the_best_herd() {
    use l2_kingdom::land::{herd_crowding, herd_growth, herd_labour_estimate};
    let t = &l2_kingdom::tables::Tables::DEFAULT;
    let per_head = l2_kingdom::tables::HERD_LABOUR_PER_HEAD;
    let fields = 8;
    let mut c = l2_kingdom::county::County::new();
    c.population = 10_000;
    c.pop_band = 1;
    c.fields_cattle = fields;

    // 1 — the bound, over every herd size a county plausibly holds.
    for herd in 1..=400 {
        c.herd = herd;
        c.herd_crowding = herd_crowding(t, herd, fields);
        for season in 1..=4u8 {
            let ceiling = herd_labour_estimate(t, &c, season).useful;
            assert!(
                ceiling <= herd * per_head * 2,
                "herd {herd} season {season}: ceiling {ceiling} is more than six a head",
            );
            // And it is a real argmax: nobody past it does any good.
            let at = herd_growth(t, herd, fields, ceiling, c.herd_crowding, season);
            let past = herd_growth(t, herd, fields, ceiling + 500, c.herd_crowding, season);
            assert!(
                past.net() <= at.net(),
                "herd {herd} season {season}: 500 more hands beat the ceiling",
            );
        }
    }

    // 2 — the player's case. A herd of five tops out at three a head, so the
    // sixteenth milkmaid is idle and so is the twentieth.
    c.herd = 5;
    c.herd_crowding = herd_crowding(t, 5, fields);
    let small = herd_labour_estimate(t, &c, 2).useful;
    assert_eq!(small, 5 * per_head, "a herd of five uses three milkmaids a head, not six");
    assert_eq!(
        herd_growth(t, 5, fields, small, c.herd_crowding, 2).net(),
        herd_growth(t, 5, fields, small * 2, c.herd_crowding, 2).net(),
        "and doubling the dairy buys exactly nothing",
    );

    // 3 — and the season is one of the inputs, which is what he asked.
    c.herd = 74;
    c.herd_crowding = herd_crowding(t, 74, fields);
    let hands = herd_labour_estimate(t, &c, 1).useful;
    let spring = herd_growth(t, 74, fields, hands, c.herd_crowding, 1);
    let summer = herd_growth(t, 74, fields, hands, c.herd_crowding, 2);
    assert_eq!(
        spring.births,
        summer.births * 3 / 2,
        "Spring is half again as many calves: {spring:?} against {summer:?}",
    );
    // Winter's half is on the deaths, so it needs a herd that has any: a
    // fully-staffed low-crowding herd loses none at all and `x * 3 / 2` on zero
    // would pass with the multiplier deleted.
    let packed = herd_crowding(t, 400, fields);
    let winter = herd_growth(t, 400, fields, 2_400, packed, 4);
    let autumn = herd_growth(t, 400, fields, 2_400, packed, 3);
    assert!(autumn.deaths > 0, "the herd this claim is about has deaths to multiply");
    assert_eq!(
        winter.deaths,
        autumn.deaths * 3 / 2,
        "and Winter half again as many deaths: {winter:?} against {autumn:?}",
    );
}

/// **The small-herd bonus steps down harder than the herd steps up**, so a
/// smaller herd outbreeds a larger one at all three of its boundaries.
/// `docs/bugs.md` B98.
///
/// The bonus in `FUN_0044DA99` is `+10000` below 5 head, `+5000` below 10 and
/// `+2000` below 25, per ten thousand, added to the birth rate once staffing
/// reaches 100 %. Each band is sensible on its own — a county reduced to three
/// cows has to be able to come back — and the three together are not monotone:
/// the animal that crosses a boundary costs more births than it brings.
///
/// **Reproduced on purpose**, and pinned here so that a ruleset which smooths
/// the ladder has to say so. The staffing is the one
/// `Herd_LabourEstimate` would assign, not a number chosen to make the point.
///
/// Ablation, run: flattening [`l2_kingdom::tables::HERD_SMALL_BONUS`] to three
/// equal bonuses makes every pair monotone and fails all three.
#[test]
fn a_smaller_herd_outbreeds_a_larger_one_at_each_bonus_step() {
    use l2_kingdom::land::{herd_crowding, herd_growth, herd_labour_estimate};
    let t = &l2_kingdom::tables::Tables::DEFAULT;
    let fields = 8;
    let spring = 1u8;

    // Fully staffed means "at the ceiling the game itself would assign", which
    // is what a player who has filled the dairy is looking at.
    let calves = |herd: i32| {
        let mut c = l2_kingdom::county::County::new();
        c.population = 10_000;
        c.pop_band = 1;
        c.herd = herd;
        c.fields_cattle = fields;
        c.herd_crowding = herd_crowding(t, herd, fields);
        let hands = herd_labour_estimate(t, &c, spring).useful;
        herd_growth(t, herd, fields, hands, c.herd_crowding, spring).births
    };

// The three boundaries, with the numbers —
    // an inequality alone would survive the whole ladder being scaled away.
    for &(small, big, fewer, more) in &[(4, 5, 7, 4), (9, 10, 10, 6), (24, 25, 16, 10)] {
        assert_eq!(calves(small), fewer, "a herd of {small} in spring");
        assert_eq!(calves(big), more, "a herd of {big} in spring");
        assert!(
            calves(small) > calves(big),
            "the bonus step at {big} head is supposed to cost more than the cow is worth",
        );
    }

    // And it is a boundary effect, not a trend: inside a band the larger herd
    // does breed faster, which is the half that says the ladder is otherwise
    // working.
    assert!(calves(8) > calves(6), "inside a band, more cows means more calves");
}
