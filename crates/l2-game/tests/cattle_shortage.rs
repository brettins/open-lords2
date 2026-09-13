//! **The herd's distress signal, where the player reads it** — and the split
//! slider's writer.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" LORDS2_FIXTURES="E:\dev\lords2-fixtures" \
//!   cargo test -p l2-game --test cattle_shortage
//! ```
//!
//! > *"I don't know why 16 cows are being lost this season."*
//!
//! `crates/l2-kingdom/tests/cattle.rs` has the arithmetic and the oracle: the
//! floor at county `+0xD4` is `Herd_LabourEstimate`'s break-even staffing, we
//! never wrote it, and his county wanted 149 milkmaids and had 114. This file
//! is the other half — that the number now reaches **all four** places the
//! original spends it, because a floor computed and not drawn is the same
//! silence he reported.
//!
//! The second test is the split slider, which is the arm of the *other* cattle
//! report: *"it shows idle and +8 cows, then I click the slider towards
//! industry, then suddenly +12 cows."* The jump itself is the original's and
//! `docs/bugs.md` has it; what was ours is that
//! `Kingdom::set_industry_share` ran `Labour_Allocate; County_RefreshEstimates`
//! **twice** and never ran the `Ration_Apply` that `Labour_SetIndustryShare`
//! (`0x0043933B`) has between them.

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::screens::job::{staffing, Staffing};
use l2_game::screens::village::VillageScreen;
use l2_game::{scenario, Game};
use l2_kingdom::tables::{Tables, JOB_CATTLE_FARMING};
use l2_mods::Platform;
use l2_view::village as vill;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

fn assets() -> Option<Assets> {
    let dir = install()?;
    let platform = Platform::builder().base(&dir).build().expect("the install mounts");
    Some(Assets::load(&platform.vfs).expect("assets load"))
}

/// The county the player was looking at, as his save holds it: 80 head on eight
/// pastures, 114 milkmaids, 150 people, crowding 10.
///
/// Written onto a real imported position, so that
/// the pipeline, the estimate call sites and the screens all see a county the
/// rest of the game agrees exists.
fn his_county(game: &mut Game) -> usize {
    let id = game
        .kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].owner == game.player)
        .expect("the player holds a county");
    let c = &mut game.kingdom.counties[id];
    c.population = 150;
    c.pop_band = 6;
    c.herd = 80;
    c.herd_eaten = 0;
    c.fields_cattle = 8;
    c.herd_crowding = 10;
    c.labour = [0; l2_kingdom::tables::JOB_COUNT];
    c.labour[JOB_CATTLE_FARMING] = 114;
    c.labour[l2_kingdom::tables::JOB_IDLE_TOWNSFOLK] = 36;
    id
}

/// **All four readers of `+0xD4`, on the county that lost the sixteen cows.**
///
/// Each claim is a different piece of the original: `Panel_JobDetail`'s colour
/// ladder, `Village_RebuildIcons`' shortfall icons, the county strip's *short*
/// produce frame and the minimap's labour overlay. They are asserted together
/// because the defect was one missing number feeding all four, and a test that
/// checked one would have gone green on a fix that reached only that one.
///
/// **Ablation, run:** drop the `labour_wanted` write from
/// `l2_kingdom::field::refresh_estimates` — red at claim 1, `Right` instead of
/// `Short`. Drop it from `Kingdom::herd_season_tick` instead and the sibling
/// test in `crates/l2-kingdom/tests/cattle.rs` is red where this one is green,
///.
#[test]
fn the_understaffed_herd_is_drawn_short_everywhere_the_original_draws_it() {
    let Some(_assets) = assets() else {
        l2_testkit::skip!("no game install, so no imported position to put his county on");
    };
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    let id = his_county(&mut game);

    // The estimate pass the original runs on any county whose inputs moved.
    game.kingdom.refresh_estimates(id);
    let c = &game.kingdom.counties[id];

    assert_eq!(
        c.labour_wanted[JOB_CATTLE_FARMING], 149,
        "break-even wants every person the search may ask for; he had {}",
        c.labour[JOB_CATTLE_FARMING]
    );

    // 1 — `Panel_JobDetail`'s colour.
    assert_eq!(
        staffing(c, JOB_CATTLE_FARMING),
        Staffing::Short,
        "the milkmaid count is drawn red, not black"
    );

    // 2 — `Village_RebuildIcons`: the shortfall as extra, unselectable icons.
    let (main, other) = vill::icon_counts(
        c.labour[JOB_CATTLE_FARMING],
        c.labour_wanted[JOB_CATTLE_FARMING],
        c.labour_useful[JOB_CATTLE_FARMING],
        c.pop_band,
    );
    assert!(other < 0, "the cluster shows {other} shortfall icons beside its {main}");
    let icons = VillageScreen::icons(c);
    let cluster = VillageScreen::slots(c)
        .iter()
        .position(|&j| j == JOB_CATTLE_FARMING)
        .expect("cattle has a cluster");
    assert!(icons[cluster].iter().any(|&v| v != 0), "and the cattle cluster is not empty");

    // 3 — the county strip's produce row: `labour < wanted` is the *short*
    // frame, which is the branch `screens/county.rs` takes.
    assert!(c.labour[JOB_CATTLE_FARMING] < c.labour_wanted[JOB_CATTLE_FARMING]);
    assert!(
        c.labour_useful[JOB_CATTLE_FARMING] >= c.labour[JOB_CATTLE_FARMING],
        "and it is short rather than wasted, so the ringed frame does not win the test above it"
    );

    // 4 — the minimap's labour overlay.
    assert_eq!(c.minimap_bands().labour, 0, "the shortage band");

    // And the reason any of it matters: the herd really is dying. The fixture's
    // next season is Spring, whose calving bonus softens it to −7; his save was
    // entering Winter, where the same county forecasts the −16 he reported and
    // `crates/l2-kingdom/tests/cattle.rs` pins that season by name.
    assert!(
        c.herd_change_expected < 0,
        "the herd is shrinking under this staffing: {}",
        c.herd_change_expected
    );
}

/// **`Labour_SetIndustryShare` (`0x0043933B`) is one pass with a `Ration_Apply`
/// in it**, and ours was two passes with none.
///
/// The evidence the ration pass ran is a field only it writes: move the split
/// on a county whose herd cannot cover the requirement by dairy alone and
/// `herd_eaten` moves with the labour. The evidence it is `Ration_Apply` and
/// not `crate::ration::apply` is that the store is *not* debited — the original
/// records and does not spend outside the season, and a slider a player drags
/// a hundred times in one gesture would otherwise eat the county.
///
/// **Ablation, run:** restore the `for _ in 0..2 { allocate; refresh }` body —
/// red at the `herd_eaten` claim, which is 0 there because nothing recomputes
/// it.
#[test]
fn the_split_slider_reapplies_the_ration_without_spending_it() {
    let Some(_assets) = assets() else {
        l2_testkit::skip!("no game install, so no imported position to put his county on");
    };
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    let id = game
        .kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].owner == game.player)
        .expect("the player holds a county");

    // A county whose people outrun its dairy, so the ration pass has something
    // to say: `food_from_dairy` is five a head, so 20 head feed 100 of the 900.
    {
        let c = &mut game.kingdom.counties[id];
        c.population = 900;
        c.pop_band = 20;
        c.herd = 20;
        c.grain = 100_000;
        c.herd_eaten = 0;
        c.ration_wanted = 3;
        c.ration_split = 100;
        c.industry_share = 0;
    }
    let (herd_before, grain_before) =
        (game.kingdom.counties[id].herd, game.kingdom.counties[id].grain);

    assert!(game.kingdom.set_industry_share(id, 40), "the split moved");

    let c = &game.kingdom.counties[id];
    assert_eq!(c.industry_share, 40);
    assert!(
        c.herd_eaten > 0,
        "`Ration_Apply` ran between the allocation and the estimates: herd_eaten {}",
        c.herd_eaten
    );
    assert_eq!(c.herd, herd_before, "and it recorded rather than spent: the herd is untouched");
    assert_eq!(c.grain, grain_before, "likewise the granary");

    // Setting the same share again is the original's early return.
    assert!(!game.kingdom.set_industry_share(id, 40), "no move, no work");
}
