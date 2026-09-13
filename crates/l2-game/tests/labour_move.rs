//! **A drag in the village is `Labour_Move`
//! subtraction.** Five player reports, one mechanism and two beside it.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" LORDS2_FIXTURES="E:\dev\lords2-fixtures" \
//!   cargo test -p l2-game --test labour_move
//! ```
//!
//! > *"industry values don't seem to update, and for some reason mining started as off."*
//! > *"The labor slider seems to reset each turn so that I have to reassign peasants to
//! > wheat each turn."*
//! > *"wheat does not show the +value when it is about to be harvested, I'm noticing
//! > generally the industry numbers in the sidebar are inaccurate."*
//!
//! `Labour_Move` (`0x00439B52`) moves the workers and then switches the
//! destination industry on (`FUN_00439CC2`), re-runs the food pass and the
//! estimates twice with every blacksmith of the realm (`FUN_00448648`), and
//! rewrites the county's shares from where people now stand. Ours moved the
//! workers and stopped. So a drag changed no forecast, staffed a switched-off
//! mine that stayed switched off, and was dealt back to the old split by the
//! season's `Labour_Allocate`. `docs/decisions.md` C180.
//!
//! **Every test here drives the screens** — a band, a release and a press in the
//! village; End Turn from the keyboard; *Start* on the setup page; a press on a
//! smithy on the map — and reads the answer back where the player reads it
//! wherever a number is drawn. Where a number is typed it is typed from the
//! decompilation (the flat `80` of `Industry_EfficiencyRamp` with *Advanced
//! Farming* off, the site bases `10, 1, 7, 4` of `Industry_UpdateSiteTile`), not
//! from the constant under test; where only a save can settle it, the save is
//! read.

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::county as county_screen;
use l2_game::screens::map::MapScreen;
use l2_game::screens::setup::{
    SetupPage, SetupScreen, CUSTOM_BUTTONS, CUSTOM_BUTTON_Y, MAP_LIST_ROW, MAP_LIST_X, MAP_LIST_Y,
};
use l2_game::screens::village::VillageScreen;
use l2_game::Game;
use l2_kingdom::field::FieldType;
use l2_kingdom::tables::{Commodity, Tables};
use l2_mods::Platform;
use l2_view::village as vill;
use l2_view::{campaign, Canvas};

/// `Industry_EfficiencyRamp` (`FUN_0044F248`)'s first line:
/// `if (!g_optAdvancedFarming) return 80;`. Every fixture on this machine has
/// Advanced Farming off, and each test asserts that premise before using it.
const FLAT_EFFICIENCY: i32 = 80;

/// `Industry_LabourEstimate`'s "as many as you like" for wood, iron and stone.
const UNBOUNDED: i32 = 100_000;

/// `Industry_UpdateSiteTile` (`0x0044EDC2`)'s four bases, in commodity order —
/// wood 10, iron 1, weapons 7, stone 4 — and a working site is `base + 1`.
const SITE_BASE: [u8; 4] = [10, 1, 7, 4];

/// `colourPos` at all four industry rows' `Ui_DrawDelta` call sites.
const POS: u8 = 0xFA;

/// `Ui_DrawDelta(…, 0x22C, pitch*row + 0x139, …)` and the icon's `0x133`.
const DELTA_X: i32 = 0x22C;
const ICON_DY: i32 = 0x133;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

fn assets() -> Option<Assets> {
    let dir = install()?;
    let platform = Platform::builder().base(&dir).build().expect("the install mounts");
    Some(Assets::load(&platform.vfs).expect("assets load"))
}

macro_rules! assets {
    () => {{
        let Some(a) = assets() else {
            l2_testkit::skip!("no game install, so no village grid, fonts or map to drive");
        };
        a
    }};
}

/// `DAT_0053F04C`, the industry column's pitch, typed from `CountyStrip_Draw`.
fn pitch(rows: usize) -> i32 {
    if rows < 3 {
        0x3C
    } else if rows < 4 {
        0x2D
    } else {
        0x1E
    }
}

fn draw_stack(m: &mut Machine, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    m.draw(&ctx, &mut canvas);
    canvas
}

fn handle(m: &mut Machine, game: &mut Game, assets: &Assets, e: Event) {
    let mut ctx = Ctx { game, assets };
    m.handle(e, &mut ctx);
}

/// Every place `s` is drawn in `colour` in `font`, top-left corners.
fn all_text(canvas: &Canvas, font: &l2_game::shell::font::Font, s: &str, colour: u8) -> Vec<(i32, i32)> {
    let style = l2_game::shell::font::Style { colour: 1, shadow: None, caps: None };
    let (w, h) = (font.width(s).max(1), font.height(s).max(1));
    let mut probe = Canvas::new(w as usize, h as usize);
    font.draw(&mut probe, 0, 0, s, &style);
    let ink: Vec<(i32, i32)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| probe.at(x as usize, y as usize) == 1)
        .collect();
    let mut found = Vec::new();
    if ink.is_empty() {
        return found;
    }
    for oy in 0..(canvas.height as i32 - h) {
        for ox in 0..(canvas.width as i32 - w) {
            if ink.iter().all(|&(x, y)| canvas.at((ox + x) as usize, (oy + y) as usize) == colour) {
                found.push((ox, oy));
            }
        }
    }
    found
}

/// Whether `+value ` is drawn in the industry column's row `row` of `rows`.
fn drawn_in_row(canvas: &Canvas, assets: &Assets, value: i32, row: usize, rows: usize) -> bool {
    let f = assets.shell.ten.as_ref().expect("Font_10.pl8");
    let p = pitch(rows);
    all_text(canvas, f, &format!("+{value} "), POS).into_iter().any(|(x, y)| {
        x >= DELTA_X && y >= p * row as i32 + ICON_DY && y < p * row as i32 + ICON_DY + p
    })
}

/// A band round exactly one icon of `cluster`, a release, and a press on
/// `target` — the three screen ids of the original's gesture, `0x02 → 0x05 →
/// 0x06`, as events. Neighbouring icons are 16 px apart and rows 12 px, so a
/// band nine pixels wide and one tall catches one.
fn drag_one_icon(m: &mut Machine, game: &mut Game, assets: &Assets, county: u8, cluster: usize, target: usize) {
    let c = &game.kingdom.counties[county as usize];
    let icons = VillageScreen::icons(c);
    let slot = (0..vill::ICONS_PER_CLUSTER)
        .find(|&s| icons[cluster][s] != 0 && icons[cluster][s] != vill::ICON_SHORTFALL)
        .expect("the source cluster has a person to pick up");
    let (px, py) = vill::icon_hit_point(cluster, slot, vill::SCENE_Y);
    handle(m, game, assets, Event::Click { x: px - vill::DRAG_DEAD_ZONE, y: py });
    handle(m, game, assets, Event::Pointer { x: px, y: py });
    handle(m, game, assets, Event::Release { x: px, y: py });
    // Where the drop lands is `vill_gd8.pl8`, so the target point is read out
    // of the grid
    let art = assets.village.as_ref().expect("vill_gd8.pl8");
    let (dx, dy) = (vill::SCENE_Y..vill::SCENE_Y + 320)
        .step_by(4)
        .flat_map(|y| (vill::SCENE_X..vill::SCENE_X + 363).step_by(4).map(move |x| (x, y)))
        .find(|&(x, y)| art.cluster_at(x, y, vill::SCENE_Y) == target + 1)
        .expect("the target cluster is somewhere on the drop grid");
    handle(m, game, assets, Event::Click { x: dx, y: dy });
}

fn cluster_of(game: &Game, county: u8, slot: usize) -> usize {
    let slots = VillageScreen::slots(&game.kingdom.counties[county as usize]);
    (0..vill::CLUSTER_COUNT).find(|&i| slots[i] == slot).expect("the job has a cluster")
}

fn end_turn(m: &mut Machine, game: &mut Game, assets: &Assets) {
    let before = game.kingdom.turn_count;
    handle(m, game, assets, Event::KeyDown(Key::Char('E')));
    let mut done_at = None;
    for n in 1..2_000u32 {
        let mut ctx = Ctx { game: &mut *game, assets };
        m.update(&mut ctx);
        if done_at.is_none() && game.kingdom.turn_count > before {
            done_at = Some(n);
        }
        if done_at.is_some_and(|t| n >= t + l2_view::fade::PHASES as u32) {
            return;
        }
    }
    panic!("the turn never came round");
}

// ------------------------------------------------------------------- the drop

/// **One drag moves two sidebar numbers and throws a switch.**
///
/// England turn one's county 8 is the person's, cuts wood with 108 of its 435
/// and has an iron mine that `Game_SetupRealmsAndCounties` left **off** — the
/// loop takes the first of wood, iron, stone the county has and stops
/// save agrees. One icon of foresters dropped on the mine:
///
/// * **`FUN_00439CC2` switches the mine on**, so its site turns to working
///   (`1 + 1`) and its ceiling to 100,000 — *"mining started as off"*, and
///   putting men on it is how the original turns it on;
/// * **the wood row redraws** at `(108 − popBand) × 80%`
///   gone — *"industry values don't seem to update"*;
/// * **the iron row appears** under it with `popBand × 80%`.
///
/// **Ablations, both run.** Delete `self.switch_on_by_drop(county, to)` from
/// `Kingdom::move_labour`: red at the switch. Delete the `refresh_estimates` and
/// `refresh_blacksmiths` calls from its loop: red at the mine's ceiling, 0
/// against 100,000 — the first assertion that reads an estimate, and ahead of
/// the wood row, which would still draw `+86`.
#[test]
fn a_drop_on_a_switched_off_mine_switches_it_on_and_both_rows_redraw() {
    let assets = assets!();
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    game.prefs.tip_screens = false;
    assert!(!game.kingdom.options.advanced_farming, "the flat 80 below is Advanced Farming off");

    let county = 8u8;
    let id = county as usize;
    assert!(game.is_players(county), "England turn one seats the person in county 8");
    let (iron, wood) = (Commodity::Iron.index(), Commodity::Wood.index());
    {
        let c = &game.kingdom.counties[id];
        assert!(c.industry[iron].has_resource && !c.industry[3].has_resource, "a mine and no quarry");
        assert!(!c.industry[iron].enabled, "the file's mine starts switched off");
        assert!(c.industry[wood].enabled, "and its forest on");
    }
    let site = l2_kingdom::map::industry_site(&game.kingdom.campaign.map, county, Commodity::Iron)
        .expect("the mine has a site");
    assert_eq!(game.kingdom.campaign.map.terrain[site], SITE_BASE[iron], "an idle mine");

    game.select(county);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));

    let foresters = game.kingdom.counties[id].labour[6];
    let before = draw_stack(&mut m, &mut game, &assets);
    let file_forecast = foresters * FLAT_EFFICIENCY / 100;
    assert!(drawn_in_row(&before, &assets, file_forecast, 0, 1), "the file's +{file_forecast} on the wood row");

    let (from, to) = (cluster_of(&game, county, 6), cluster_of(&game, county, 4));
    drag_one_icon(&mut m, &mut game, &assets, county, from, to);

    let c = &game.kingdom.counties[id];
    let band = c.pop_band;
    assert_eq!(c.labour[4], band, "one icon is popBand people, and they are on the mine");
    assert_eq!(c.labour[6], foresters - band, "and out of the forest");

    // The switch, the ceiling and the picture — what the simulation uses and
    // what the map shows, which must be the same byte.
    assert!(c.industry[iron].enabled, "FUN_00439CC2: men on a site switch it on");
    assert_eq!(c.labour_useful[4], UNBOUNDED, "a switched-on mine takes as many as you like");
    assert_eq!(game.kingdom.campaign.map.terrain[site], SITE_BASE[iron] + 1, "the site is working");
    assert_eq!(county_screen::industry_rows(c), vec![6, 4], "the sidebar lists wood, then iron");

    let after = draw_stack(&mut m, &mut game, &assets);
    let wood_now = (foresters - band) * FLAT_EFFICIENCY / 100;
    let iron_now = band * FLAT_EFFICIENCY / 100;
    assert!(drawn_in_row(&after, &assets, wood_now, 0, 2), "the wood row redraws at +{wood_now}");
    assert!(
        !drawn_in_row(&after, &assets, file_forecast, 0, 2),
        "and the file's +{file_forecast} is gone from it"
    );
    assert!(drawn_in_row(&after, &assets, iron_now, 1, 2), "the iron row appears with +{iron_now}");
}

/// **The drag selection box is drawn with the debug overlay OFF**, because
/// `Village_DrawBand` (`0x00412795`) is the original's and not ours.
///
/// C173 put our outline behind Ctrl+D on the strength of not having found this
/// function
/// disappeared, it was probably a debug thing that you removed with other debug
/// boxes."* The assertion is the whole rectangle in the original's own colour —
/// `Ui_DrawRectOutline`'s literal `0x20` — read off the canvas at the four
/// edges, with the overlay left at its default.
#[test]
fn the_drag_selection_box_is_drawn_without_the_debug_overlay() {
    let assets = assets!();
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    game.prefs.tip_screens = false;
    assert!(!game.prefs.debug_overlay, "the default session, which is the point");

    let county = 8u8;
    game.select(county);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));

    // A press and a drag with no release: screen `0x05`, the band state.
    let top = vill::SCENE_Y;
    let (x0, y0) = (vill::SCENE_X + 20, top + 40);
    let (x1, y1) = (x0 + 60, y0 + 30);
    handle(&mut m, &mut game, &assets, Event::Click { x: x0, y: y0 });
    handle(&mut m, &mut game, &assets, Event::Pointer { x: x1, y: y1 });
    let canvas = draw_stack(&mut m, &mut game, &assets);

    // `FUN_00403cf4(x, y, w, h, 0x20)` — top, bottom, left and right.
    const BAND_INK: u8 = 0x20;
    let at = |x: i32, y: i32| canvas.at(x as usize, y as usize);
    for x in x0..=x1 {
        assert_eq!(at(x, y0), BAND_INK, "the band's top edge at x {x}");
        assert_eq!(at(x, y1), BAND_INK, "the band's bottom edge at x {x}");
    }
    for y in y0..=y1 {
        assert_eq!(at(x0, y), BAND_INK, "the band's left edge at y {y}");
        assert_eq!(at(x1, y), BAND_INK, "the band's right edge at y {y}");
    }

    // And it is the band that drew it, not the village: released, it is gone.
    handle(&mut m, &mut game, &assets, Event::Release { x: x1, y: y1 });
    let after = draw_stack(&mut m, &mut game, &assets);
    assert!(
        (x0..=x1).any(|x| after.at(x as usize, y0 as usize) != BAND_INK),
        "the outline goes with the release"
    );
}

/// **The clamp is `Village_DrawBand`'s own** — a band dragged below the
/// picture stops at `g_villageTopY + 0x178` and not at the pointer.
#[test]
fn the_drag_selection_box_stops_where_the_original_clamps_it() {
    let assets = assets!();
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    game.prefs.tip_screens = false;

    let county = 8u8;
    game.select(county);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));

    let top = vill::SCENE_Y;
    let (x0, y0) = (vill::SCENE_X + 20, top + 40);
    // Past the bottom of the band area, which `Village_BandStart` also refuses
    // to arm in: the pointer keeps going, the outline does not.
    let (x1, y1) = (x0 + 60, top + vill::BAND_H + 10);
    handle(&mut m, &mut game, &assets, Event::Click { x: x0, y: y0 });
    handle(&mut m, &mut game, &assets, Event::Pointer { x: x1, y: y1 });
    let canvas = draw_stack(&mut m, &mut game, &assets);

    const BAND_INK: u8 = 0x20;
    let bottom = top + vill::BAND_H - 1;
    assert_eq!(canvas.at(x0 as usize, bottom as usize), BAND_INK, "the clamped bottom edge");
    assert_ne!(
        canvas.at(x0 as usize, y1 as usize),
        BAND_INK,
        "and nothing at the pointer, ten rows past the band area"
    );
}

// ------------------------------------------------------------ across a season

/// **The split a player drags is the split the season deals him back.**
///
/// `Labour_Allocate` runs twice in every `Season_Advance` and deals the county
/// out from its eight shares; it never writes one. `Labour_Move` ends with
/// `Labour_RecomputeShares`, which does, from where people now stand — so in the
/// original the drag *is* the new split. Ours never rewrote the shares and the
/// season put everybody back: *"I have to reassign peasants to wheat each turn."*
///
/// **Staged, and why.** England turn one has **no grain in any county**, so no
/// field can be sown and the grain ceiling is zero whatever the split says — a
/// county the original would also empty. County 8 is given 2,000 sacks, and its
/// fallow fields are painted wheat through `Kingdom::paint_field`, the brush's
/// own road. Everything after that is the screens.
///
/// The share is typed from `Labour_RecomputeShares` (`FUN_00450000`): `PctOf`
/// of each farm job over the three, the remainder to the largest.
///
/// **Ablation, run:** delete `recompute_shares` (and its twin) from
/// `Kingdom::move_labour` and the shares after the drop are still the painted
/// county's, red at the first assertion below the drop.
#[test]
fn the_split_a_player_drags_is_the_split_the_season_deals_back() {
    let assets = assets!();
    let save = l2_testkit::england!();
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    game.prefs.tip_screens = false;
    let county = 8u8;
    let id = county as usize;
    game.kingdom.counties[id].grain = 2000;
    let fallow: Vec<usize> = game
        .kingdom
        .field_tiles(id)
        .into_iter()
        .filter(|&(_, t)| t == FieldType::Fallow)
        .map(|(t, _)| t)
        .collect();
    assert!(!fallow.is_empty(), "county 8 has fallow fields to sow");
    for t in fallow {
        game.kingdom.paint_field(id, t, FieldType::Grain).expect("a fallow field takes wheat");
    }
    let painted = game.kingdom.counties[id].labour_share;

    game.select(county);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Village(county));
    let (from, to) = (cluster_of(&game, county, 1), cluster_of(&game, county, 0));
    drag_one_icon(&mut m, &mut game, &assets, county, from, to);

    let c = &game.kingdom.counties[id];
    let farm = [c.labour[0], c.labour[1], c.labour[2]];
    let total: i32 = farm.iter().sum();
    let mut expected: Vec<i32> = farm.iter().map(|&w| w * 100 / total).collect();
    let short = 100 - expected.iter().sum::<i32>();
    let largest = (0..3).max_by_key(|&j| (expected[j], std::cmp::Reverse(j))).unwrap();
    expected[largest] += short;
    assert_eq!(
        &c.labour_share[..3],
        &expected[..],
        "the drop rewrites the farm shares from the workers ({farm:?}); they were {painted:?}"
    );
    assert_ne!(&c.labour_share[..3], &painted[..3], "and the drag moved them");
    let dragged = c.labour_share;
    let grain_after_drop = c.labour[0];

    // Out of the village and End Turn, from the keyboard.
    handle(&mut m, &mut game, &assets, Event::KeyDown(Key::Escape));
    assert_eq!(m.ids(), vec![ScreenId::Campaign]);
    end_turn(&mut m, &mut game, &assets);

    let c = &game.kingdom.counties[id];
    assert_eq!(game.kingdom.turn_count, 2, "one season ran");
    assert_eq!(c.labour_share, dragged, "the season deals from the player's split and does not rewrite it");
    assert!(
        c.labour[0] >= grain_after_drop.min(c.labour_useful[0]),
        "the wheat is still staffed after the season: {} workers against {} after the drop, ceiling {}",
        c.labour[0],
        grain_after_drop,
        c.labour_useful[0]
    );
}

// --------------------------------------------------------------- a new game

/// **A new England opens with its start counties staffed, forecasting, and
/// with the mines the original leaves off, off.**
///
/// Through the setup page: England, *Start*. Three claims, each against the
/// original.
///
/// 1. **The switches are `Game_SetupRealmsAndCounties`' loop** (`0x0049BD99`):
///    `for (i = 0; i < 4; i++) if (i != 2 && hasResource[i]) { enabled[i] = 1;
///    break; }` — one industry per start county, the first of wood, iron and
///    stone, never the smithy and never a mine beside a forest. The map shows
///    the same byte: every site is `base + enabled`.
/// 2. **The person's county is `england-turn1.sav`'s person's county**, job for
///    job, forecast for forecast. That save is the original's own turn one. Its
///    seat is rolled, so the counties differ; the start row does not, and every
///    number below came out equal. Before this change ours had **no foresters,
///    155 idle, and all four forecasts zero**, because nothing had computed an
///    industry ceiling before the opening season's `Labour_AllocateAll` —
/// `Industry_ProduceAll`'s estimates were not ported — and the person's
///    herd went into `Herd_SeasonTick` unminded, because
///    `Game_SetupRealmsAndCounties`' two allocation rounds were not either.
/// 3. **The wood row draws it** on the first frame of the campaign.
///
/// The herd *count* is not compared: the save's 101 against our 109, on a
/// different county. Not explained, and not claimed.
///
/// **Ablations, both run.** Delete the `settle_start_county` loop from
/// `Settings::apply_to`: red at claim 2 (280 cattle, 47 idle). Empty the
/// estimates in `Kingdom::industry`: red at claim 2 (no foresters).
#[test]
fn a_new_england_opens_staffed_and_forecasting_as_the_originals_turn_one() {
    let assets = assets!();
    let save = l2_testkit::england!();
    let oracle = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");

    let mut game = Game::new(scenario::SEED);
    let mut setup = SetupScreen::new(SetupPage::Custom);
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        setup.update(&mut ctx);
    }
    // England is row 0 of the map list.
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        setup.handle(Event::Click { x: MAP_LIST_X + 20, y: MAP_LIST_Y + MAP_LIST_ROW / 2 }, &mut ctx);
        assert_eq!(setup.map(), 0, "England");
        setup.handle(Event::Click { x: CUSTOM_BUTTONS[1].0 + 20, y: CUSTOM_BUTTON_Y }, &mut ctx);
    }
    game.prefs.tip_screens = false;
    assert_eq!(game.kingdom.turn_count, 1, "Start ran the opening season");
    assert_eq!(
        game.kingdom.options.advanced_farming, oracle.kingdom.options.advanced_farming,
        "the setup page's defaults are the fixture's"
    );
    assert!(!game.kingdom.options.advanced_farming);

    // 1 — the switches, the rule and the picture.
    let mut owned = 0;
    for id in game.kingdom.county_ids() {
        let c = &game.kingdom.counties[id];
        if c.owner == 0 {
            continue;
        }
        owned += 1;
        let first = [0usize, 1, 3].into_iter().find(|&i| c.industry[i].has_resource);
        for i in 0..4 {
            assert_eq!(c.industry[i].enabled, Some(i) == first, "county {id} industry {i}: the start switch");
            if let Some(site) = l2_kingdom::map::industry_site(&game.kingdom.campaign.map, id as u8, Commodity::ALL[i]) {
                assert_eq!(
                    game.kingdom.campaign.map.terrain[site],
                    SITE_BASE[i] + u8::from(c.industry[i].enabled),
                    "county {id} industry {i}: the map shows the switch the simulation reads"
                );
            }
        }
    }
    assert_eq!(owned, 5);

    // 2 — the person's county, against the original's person's county.
    let theirs = oracle
        .kingdom
        .county_ids()
        .find(|&id| oracle.kingdom.counties[id].owner == oracle.player)
        .expect("the save seats the person");
    let ours = game
        .kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].owner == game.player)
        .expect("the new game seats the person");
    let (t, o) = (&oracle.kingdom.counties[theirs], &game.kingdom.counties[ours]);
    assert_eq!(o.labour, t.labour, "the person's jobs: ours county {ours}, the original's county {theirs}");
    assert_eq!(o.industry_share, t.industry_share, "the farm/industry split");
    assert_eq!(o.labour_useful, t.labour_useful, "the ceilings");
    let next = |c: &l2_kingdom::county::County| c.industry.iter().map(|i| i.next_season).collect::<Vec<_>>();
    assert_eq!(next(o), next(t), "the four industry forecasts");
    assert_eq!(o.herd_change_expected, t.herd_change_expected, "the cattle forecast");
    assert_eq!(o.grain_change_expected, t.grain_change_expected, "the grain forecast");

    // 3 — drawn.
    let wood = t.industry[Commodity::Wood.index()].next_season;
    assert!(wood > 0, "the original's person forecasts wood on turn one");
    game.select(ours as u8);
    let mut m = Machine::new(ScreenId::Campaign);
    let canvas = draw_stack(&mut m, &mut game, &assets);
    assert!(drawn_in_row(&canvas, &assets, wood, 0, 1), "the wood row draws the original's +{wood}");
}

// ------------------------------------------------------------ the switch

/// **Switching a smithy on redraws every other smithy of the realm.**
///
/// `Industry_ToggleFromMap` (`0x0043D309`) ends with `FUN_00448648(owner)`,
/// `Industry_LabourEstimate(c, weapons)` over every county the realm holds —
/// the Readme's *"turning a blacksmith on will reduce the resources available
/// to other blacksmiths"*, visible the moment you click. Ours refreshed the
/// clicked county only, so the other smithy kept a ceiling and a forecast
/// computed from a stockpile it no longer had to itself.
///
/// `siege-lastturn.sav`: the person holds counties 1, 2 and 3 with every smithy
/// off and 50 iron. Press county 2's smithy on the map, then county 1's.
/// The claim is **idempotence**: after the second press, county 2's weapons
/// figures are already what a fresh `Industry_LabourEstimate` gives — and they
/// are not what they were after the first, so the claim is not vacuous.
///
/// **What moves on this save is the ceiling, not the forecast**, measured:
/// county 2's useful blacksmiths halve from 180 to 90 and its forecast is 4
/// either side, because four is what its own staffing makes before either
/// quota binds. The ceiling is what `FUN_004106C4` draws the row's ringed
/// frame from (`useful < workers`), and what the season's allocation obeys. The
/// forecast is still checked on the canvas.
///
/// **Ablation, run:** delete `self.refresh_blacksmiths(owner)` from
/// `Kingdom::toggle_industry` — red at the idempotence assertion, `(90, 4)`
/// fresh against `(180, 4)` stale.
#[test]
fn switching_one_smithy_on_redraws_the_realms_other_smithy() {
    let assets = assets!();
    let save = l2_testkit::fixture!("siege-lastturn.sav");
    let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
    game.prefs.tip_screens = false;
    let player = game.player;
    for id in [1usize, 2, 3] {
        assert_eq!(game.kingdom.counties[id].owner, player, "county {id} is the person's");
        assert!(!game.kingdom.counties[id].industry[2].enabled, "county {id}'s smithy starts off");
    }

    let press_smithy = |game: &mut Game, county: u8| {
        let tile = l2_kingdom::map::industry_site(&game.kingdom.campaign.map, county, Commodity::Weapons)
            .expect("every county has a smithy");
        let (tx, ty) = l2_kingdom::map::coords(tile);
        let mut screen = MapScreen::new();
        screen.centre_on_tile(tx as usize, ty as usize);
        let mut canvas = Canvas::screen();
        {
            let ctx = Ctx { game: &mut *game, assets: &assets };
            screen.draw(&ctx, &mut canvas);
        }
        let (row, col) = campaign::tile_to_cell(tx as usize, ty as usize);
        let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
        let z = screen.zoom();
        let (x, y) = (sx + z.tile_w / 2, sy + z.tile_h / 2);
        let mut ctx = Ctx { game: &mut *game, assets: &assets };
        screen.handle(Event::Click { x, y }, &mut ctx);
    };

    press_smithy(&mut game, 2);
    assert!(game.kingdom.counties[2].industry[2].enabled, "the press switched county 2's smithy on");
    let first = (game.kingdom.counties[2].labour_useful[7], game.kingdom.counties[2].industry[2].next_season);

    press_smithy(&mut game, 1);
    assert!(game.kingdom.counties[1].industry[2].enabled, "and county 1's");
    let c2 = &game.kingdom.counties[2];
    let now = (c2.labour_useful[7], c2.industry[2].next_season);

    let mut fresh = c2.clone();
    let k = &game.kingdom;
    let share = l2_kingdom::industry::weapon_shares(&k.tables, &k.counties, k.county_count, player);
    l2_kingdom::industry::refresh(
        &k.tables,
        &mut fresh,
        Commodity::Weapons,
        &k.realms[player as usize],
        share,
        k.options.advanced_farming,
    );
    assert_eq!(
        (fresh.labour_useful[7], fresh.industry[2].next_season),
        now,
        "county 2's smithy is not current after county 1's was switched on"
    );
    assert_ne!(now, first, "county 1's smithy took nothing from county 2's, so this proves nothing");

    // Drawn, on county 2's sidebar.
    game.select(2);
    let rows = county_screen::industry_rows(&game.kingdom.counties[2]);
    let row = rows.iter().position(|&s| s == 7).expect("the smithy row is listed");
    let mut m = Machine::new(ScreenId::Campaign);
    let canvas = draw_stack(&mut m, &mut game, &assets);
    if now.1 > 0 {
        assert!(drawn_in_row(&canvas, &assets, now.1, row, rows.len()), "the smithy row draws +{}", now.1);
    }
    if first.1 > 0 && first.1 != now.1 {
        assert!(!drawn_in_row(&canvas, &assets, first.1, row, rows.len()), "and not the stale +{}", first.1);
    }
}
