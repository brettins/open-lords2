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

    game.kingdom.refresh_estimates(id);
    let c = &game.kingdom.counties[id];

    assert_eq!(
        c.labour_wanted[JOB_CATTLE_FARMING], 149,
        "break-even wants every person the search may ask for; he had {}",
        c.labour[JOB_CATTLE_FARMING]
    );

    assert_eq!(
        staffing(c, JOB_CATTLE_FARMING),
        Staffing::Short,
        "the milkmaid count is drawn red, not black"
    );

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

    assert!(c.labour[JOB_CATTLE_FARMING] < c.labour_wanted[JOB_CATTLE_FARMING]);
    assert!(
        c.labour_useful[JOB_CATTLE_FARMING] >= c.labour[JOB_CATTLE_FARMING],
        "and it is short rather than wasted, so the ringed frame does not win the test above it"
    );

    assert_eq!(c.minimap_bands().labour, 0, "the shortage band");

    assert!(
        c.herd_change_expected < 0,
        "the herd is shrinking under this staffing: {}",
        c.herd_change_expected
    );
}

/// **`Labour_SetIndustryShare` (`0x0043933B`) is one pass with a `Ration_Apply`
/// in it**, and ours was two passes with none.
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

    assert!(!game.kingdom.set_industry_share(id, 40), "no move, no work");
}
