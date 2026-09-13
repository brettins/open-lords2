//! **The job popup's five bodies, the county-event letter and the tile
//! panel's three, drawn with the game's own fonts and words.**
//!
//! The tile panel's castle and field arms draw `L2.eng` groups 71 and 77 out
//! of a county exactly as the job popup does — `TileInfo_DrawCastle` shares
//! `Castle_DrawStatusBlock` with `FUN_00414220` — so they are measured with
//! these helpers rather than a second copy of them.
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-game --test job_bodies
//! ```
//!
//! C164 carried the figures these painters draw and left the painters as stubs.
//! Every assertion below is **one figure or one word, in its own box, at the
//! painter's own coordinates** — never a whole-canvas diff, which can pass when a
//! panel's height changes too. Every coordinate that starts a line is a literal
//! out of the decompilation; a piece chained after it is placed by the font's
//! own measure and `Ui_DrawText`'s two rules (C155): a character with no glyph
//! advances four, and every call adds a four-pixel trailer. Nothing here is
//! computed from a constant in `screens/job.rs` or `screens/message.rs`.
//!
//! **What the fixtures hold, and so what is weak.** Advanced farming is off in
//! every save on this machine, and the grain sown, grown and harvested, field
//! reclamation, every castle under construction and all four event and weather
//! figures are zero in every save. Where a figure is zero on disk the test takes
//! the road the game takes to make it non-zero — painting fields and advancing
//! seasons, ordering a castle, firing an event and running the season's tick —
//! and the few that stay zero say so beside the assertion.

use l2_game::game::Assets;
use l2_game::message::category;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::job::JobScreen;
use l2_game::shell::font::{Font, Style};
use l2_game::{scenario, Game};
use l2_kingdom::event::{EventKind, RealmPurse};
use l2_kingdom::field::FieldType;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::tables::{
    Commodity, Season, Tables, Weather, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING,
    JOB_FIELD_RECLAMATION, JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_STONE_QUARRYING,
    JOB_WOOD_CUTTING,
};
use l2_mods::Platform;
use l2_view::Canvas;

// ---------------------------------------------------------------------- setup

macro_rules! install_assets {
    () => {{
        let Some(dir) = l2_testkit::install_dir() else {
            l2_testkit::skip!("no game install, so there are no fonts and no L2.eng");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        Assets::load(&platform.vfs).expect("assets load")
    }};
}

macro_rules! england_world {
    () => {{
        let assets = install_assets!();
        let save = l2_testkit::england!();
        let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        game.prefs.tip_screens = false;
        (game, assets)
    }};
}

macro_rules! fixture_world {
    ($name:expr) => {{
        let assets = install_assets!();
        let save = l2_testkit::fixture!($name);
        let mut game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        game.prefs.tip_screens = false;
        (game, assets)
    }};
}

/// The popup for one job of one county, drawn on its own.
fn draw_job(game: &mut Game, assets: &Assets, county: usize, job: usize) -> Canvas {
    let mut screen = JobScreen::new(county as u8, job);
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

/// The painters' ink, `0x3F`, and `Ui_DrawDelta`'s `colourNeg`, `0xF9`.
const INK: u8 = 0x3F;
const NEG: u8 = 0xF9;

fn body(a: &Assets) -> &Font {
    a.shell.body.as_ref().expect("Fntl2_14.pl8 is in the install")
}

fn heading(a: &Assets) -> &Font {
    a.shell.heading.as_ref().expect("Fntl2_22.pl8 is in the install")
}

/// One `L2.eng` string out of the install, which must be there.
#[track_caller]
fn eng(a: &Assets, group: usize, index: usize) -> String {
    let s = a.shell.text(group, index).to_string();
    assert!(!s.is_empty(), "setup: L2.eng {group}/{index} is empty");
    s
}

/// Whether `s`, drawn in `f`, sits on the canvas with its origin at exactly
/// `(x, y)`: every set pixel of a probe rendered in the same face is `colour`.
fn is_at(canvas: &Canvas, f: &Font, s: &str, colour: u8, x: i32, y: i32) -> bool {
    let (w, h) = (f.width(s).max(1), f.height(s).max(1));
    let mut probe = Canvas::new(w as usize, h as usize);
    f.draw(&mut probe, 0, 0, s, &Style { colour: 1, shadow: None, caps: None });
    let mut any = false;
    for py in 0..h {
        for px in 0..w {
            if probe.at(px as usize, py as usize) != 1 {
                continue;
            }
            any = true;
            let (cx, cy) = (x + px, y + py);
            if cx < 0 || cy < 0 || cx >= canvas.width as i32 || cy >= canvas.height as i32 {
                return false;
            }
            if canvas.at(cx as usize, cy as usize) != colour {
                return false;
            }
        }
    }
    any
}

/// Every x on row `y` where `s` sits — the failure message's answer to *then
/// where is it?*
fn xs_on_row(canvas: &Canvas, f: &Font, s: &str, colour: u8, y: i32) -> Vec<i32> {
    (0..canvas.width as i32).filter(|&x| is_at(canvas, f, s, colour, x, y)).collect()
}

#[track_caller]
fn expect_at(canvas: &Canvas, f: &Font, s: &str, colour: u8, x: i32, y: i32) {
    assert!(
        is_at(canvas, f, s, colour, x, y),
        "{s:?} in {colour:#04x} is not at ({x:#x}, {y:#x}); on that row it is at {:?}",
        xs_on_row(canvas, f, s, colour, y)
    );
}

/// `Ui_DrawText`'s advance over `s`: four for a character with no glyph, the
/// measure for the rest, and the call's four-pixel trailer. C155.
fn advance(f: &Font, s: &str) -> i32 {
    s.chars()
        .map(|ch| match f.width(&ch.to_string()) {
            0 => 4,
            w => w,
        })
        .sum::<i32>()
        + 4
}

/// `Eng_DrawString(group, index, x, y)`: the word at `x`. Returns where the
/// next piece starts.
#[track_caller]
fn word_at(canvas: &Canvas, a: &Assets, group: usize, index: usize, x: i32, y: i32) -> i32 {
    let s = eng(a, group, index);
    expect_at(canvas, body(a), &s, INK, x, y);
    x + advance(body(a), &s)
}

/// `Ui_DrawCount(value, noun, x, y)`: the digits one blank sign column right of
/// `x`, and group 8's singular or plural one trailer after them. Returns where
/// the next piece starts.
#[track_caller]
fn count_at(canvas: &Canvas, a: &Assets, value: i32, noun: usize, x: i32, y: i32) -> i32 {
    let f = body(a);
    let digits = value.to_string();
    expect_at(canvas, f, &digits, INK, x + 4, y);
    let noun = eng(a, 8, if value.abs() == 1 { noun } else { noun + 1 });
    let noun_x = x + 4 + advance(f, &digits);
    expect_at(canvas, f, &noun, INK, noun_x, y);
    noun_x + advance(f, &noun)
}

/// A signed row's value: `Ui_DrawDelta(v, 0, " ", " ", 0x128, y)` puts the sign
/// and digits at `0x130`, in `0xF9` when negative; a zero row is
/// `Ui_DrawNumber(0, '@', " ", 0x130, y)`, whose digit is one column further.
#[track_caller]
fn signed_at(canvas: &Canvas, a: &Assets, value: i32, y: i32) {
    let f = body(a);
    match value {
        0 => expect_at(canvas, f, "0", INK, 0x130 + 4, y),
        v if v < 0 => expect_at(canvas, f, &format!("-{}", -v), NEG, 0x130, y),
        v => expect_at(canvas, f, &format!("+{v}"), INK, 0x130, y),
    }
}

/// The first county a predicate picks, or a setup failure naming what was
/// wanted. The fixtures' realm assignment is rolled per game, so a test names
/// the property it needs rather than a county number.
#[track_caller]
fn county_where(k: &Kingdom, what: &str, pick: impl Fn(&l2_kingdom::county::County) -> bool) -> usize {
    (1..=k.county_count)
        .find(|&id| pick(&k.counties[id]))
        .unwrap_or_else(|| panic!("setup: no county in this fixture is {what}"))
}

/// Paint every fallow field of one county to grain, one brush stroke a tile —
/// `Field_SetType`, which is the only way a player makes a county sow.
fn paint_all_fallow_to_grain(k: &mut Kingdom, county: usize) -> i32 {
    let tiles: Vec<usize> = k
        .field_tiles(county)
        .into_iter()
        .filter(|&(_, kind)| kind == FieldType::Fallow)
        .map(|(tile, _)| tile)
        .collect();
    for &tile in &tiles {
        k.paint_field(county, tile, FieldType::Grain).expect("a fallow field takes the grain brush");
    }
    tiles.len() as i32
}

// ------------------------------------------------------------------- grain

/// **`Panel_JobGrain`'s two signed rows and its growing branch, on stored
/// numbers.** `safeturn.sav` faces Autumn, so the painter takes the `else` arm:
/// `Ui_DrawCount(+0x2FC, 2, 0x40, 0xD8)` + 77/3 + `Ui_DrawCount(2, 0x42, …)`,
/// then 77/0 + `crop[0]` + 77/4 on `0xE8`.
///
/// Non-zero on disk: the store, grain eaten, the overall change, and the season
/// count (Autumn next gives 2). **Only ever zero here:** `+0x2FC` and `crop[0]`
/// (zero in every save; the rule-driven test below makes them move) and
/// `+0x278` (the no-event line is what is asserted).
///
/// Ablations, run: the `delta(…grain_eaten…)` line deleted → `"-135"` not at
/// `(0x130, 0x108)`; the season mapping's `3 => 2` → `3 => 3` → `"2"` not at
/// `(0xF5, 0xD8)`.
#[test]
fn the_grain_popup_draws_the_store_the_eating_and_the_overall_change() {
    let (mut game, assets) = fixture_world!("safeturn.sav");
    let k = &game.kingdom;
    assert_eq!(k.season_next, 3, "setup: safeturn.sav faces Autumn");
    assert!(!k.options.advanced_farming, "setup: advanced farming is off in this save");
    let id = county_where(k, "eating grain with a store", |c| c.grain > 0 && c.grain_eaten > 0);
    let c = k.counties[id].clone();
    assert!(c.grain_change_expected != 0, "setup: county {id}'s overall change is non-zero");
    assert_eq!(c.grain_event_change, 0, "setup: no event touched the store");

    let canvas = draw_job(&mut game, &assets, id, JOB_GRAIN_FARMING);

    // `Ui_DrawCount(grain, 2, 0x130, 0x88)`.
    count_at(&canvas, &assets, c.grain, 2, 0x130, 0x88);
    // `+0x278 == 0`: 77/0x18 at (0x40, 0xB0).
    word_at(&canvas, &assets, 77, 0x18, 0x40, 0xB0);
    // Advanced farming off: no fertility line, no weather line.
    assert!(!is_at(&canvas, body(&assets), &eng(&assets, 22, 3), INK, 0x80, 0x98));
    assert!(!is_at(&canvas, body(&assets), &eng(&assets, 77, 0x12), INK, 0x40, 0xC0));

    // Facing Autumn: `+0x2FC` harvested in 2 Seasons.
    let at = count_at(&canvas, &assets, c.grain_grown_expected, 2, 0x40, 0xD8);
    let at = word_at(&canvas, &assets, 77, 3, at, 0xD8);
    count_at(&canvas, &assets, 2, 0x42, at, 0xD8);
    // From `crop[0]` Sacks sown in spring.
    let at = word_at(&canvas, &assets, 77, 0, 0x40, 0xE8);
    let at = count_at(&canvas, &assets, c.crop[0], 2, at, 0xE8);
    word_at(&canvas, &assets, 77, 4, at, 0xE8);

    // The two signed rows.
    word_at(&canvas, &assets, 77, 0x1B, 0x40, 0x108);
    signed_at(&canvas, &assets, -c.grain_eaten, 0x108);
    word_at(&canvas, &assets, 77, 0x1C, 0x40, 0x118);
    signed_at(&canvas, &assets, c.grain_change_expected, 0x118);
}

/// **The grain year, as the rule walks it: sown, growing, harvested.**
///
/// No save on this machine has grain in the ground, so the fields are painted
/// with the player's brush and the seasons advanced, exactly as
/// `crates/l2-kingdom/tests/fields.rs` does. Each stage then has a non-zero
/// figure in its own box:
///
/// * facing Spring, `Ui_DrawCount(+0x230, 2, 0x40, 0xD8)` + 77/1 and
///   `Ui_DrawCount(+0x230 * g_grainYieldPerSack, 2, 0x40, 0xE8)` + 77/2;
/// * facing Summer, `+0x2FC` + 77/3 + **3** Seasons, and `crop[0]` after 77/0;
/// * facing Winter, `crop[2]` + 77/3 + **1** Season, singular.
///
/// And last, with advanced farming on and a *Sunny* band, `Grain_Grow`'s own
/// weather swing on `0xC0`, and the fertility phrase on `0x98`. The band is the
/// one hand-set input: `Weather_UpdateAll` rolls it, and this names one.
///
/// Ablations, run: `wrapping_mul(yield_per_sack)` → `wrapping_mul(1)` → `"240"`
/// not at `(0x44, 0xE8)`; `weather_line` deleted from `grain` → `"120"` not at
/// `(0x44, 0xC0)`.
#[test]
fn the_grain_popup_follows_the_crop_the_rule_sows_grows_and_harvests() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    let k = &mut game.kingdom;
    assert_eq!(k.season_next, 1, "setup: the England position faces Spring");
    let id = county_where(k, "the player's", |c| c.owner == player);
    k.counties[id].grain = 10_000;
    assert!(paint_all_fallow_to_grain(k, id) > 0, "setup: county {id} had fields to paint");
    k.refresh_estimates(id);
    let sown = k.counties[id].grain_sown_expected;
    assert!(sown > 0, "setup: painted grain fields forecast a sowing, not {sown}");
    let yield_per_sack = k.tables.grain.yield_per_sack;

    // Facing Spring.
    let canvas = draw_job(&mut game, &assets, id, JOB_GRAIN_FARMING);
    let at = count_at(&canvas, &assets, sown, 2, 0x40, 0xD8);
    word_at(&canvas, &assets, 77, 1, at, 0xD8);
    let at = count_at(&canvas, &assets, sown * yield_per_sack, 2, 0x40, 0xE8);
    word_at(&canvas, &assets, 77, 2, at, 0xE8);

    // Facing Summer: the crop is in the ground.
    game.kingdom.advance_season();
    game.kingdom.refresh_estimates(id);
    assert_eq!(game.kingdom.season_next, 2, "setup: one season on faces Summer");
    let c = game.kingdom.counties[id].clone();
    assert!(c.grain_grown_expected > 0 && c.crop[0] > 0, "setup: a crop is growing: {c:?}");
    let canvas = draw_job(&mut game, &assets, id, JOB_GRAIN_FARMING);
    let at = count_at(&canvas, &assets, c.grain_grown_expected, 2, 0x40, 0xD8);
    let at = word_at(&canvas, &assets, 77, 3, at, 0xD8);
    count_at(&canvas, &assets, 3, 0x42, at, 0xD8);
    let at = word_at(&canvas, &assets, 77, 0, 0x40, 0xE8);
    let at = count_at(&canvas, &assets, c.crop[0], 2, at, 0xE8);
    word_at(&canvas, &assets, 77, 4, at, 0xE8);

    // Facing Winter: the harvest, one Season off.
    game.kingdom.advance_season();
    game.kingdom.advance_season();
    game.kingdom.refresh_estimates(id);
    assert_eq!(game.kingdom.season_next, 4, "setup: facing Winter");
    let c = game.kingdom.counties[id].clone();
    assert!(c.crop[2] > 0, "setup: there is a harvest to forecast: {:?}", c.crop);
    let canvas = draw_job(&mut game, &assets, id, JOB_GRAIN_FARMING);
    let at = count_at(&canvas, &assets, c.crop[2], 2, 0x40, 0xD8);
    let at = word_at(&canvas, &assets, 77, 3, at, 0xD8);
    count_at(&canvas, &assets, 1, 0x42, at, 0xD8);

    // Advanced farming: the fertility phrase and the weather's swing.
    game.kingdom.options.advanced_farming = true;
    {
        let t = game.kingdom.tables;
        let c = &mut game.kingdom.counties[id];
        c.weather = Weather::Sunny;
        l2_kingdom::land::grow(&t, c, true);
    }
    let c = game.kingdom.counties[id].clone();
    assert!(c.grain_weather_change != 0, "setup: a Sunny season moves the growing crop");
    let canvas = draw_job(&mut game, &assets, id, JOB_GRAIN_FARMING);
    let band = ((c.fertility + 100) / 29) as usize;
    word_at(&canvas, &assets, 22, band, 0x80, 0x98);
    let (shown, index) = if c.grain_weather_change > 0 {
        (c.grain_weather_change, 0x10)
    } else {
        (-c.grain_weather_change, 0x11)
    };
    let at = count_at(&canvas, &assets, shown, 2, 0x40, 0xC0);
    word_at(&canvas, &assets, 77, index, at, 0xC0);
}

// ------------------------------------------------------------------ cattle

/// **`Panel_JobCattle`, on the England position's stored herds**, which carry
/// every row this painter has in both signs: births, deaths, slaughter and the
/// overall change are all non-zero, a herd eaten by nobody draws the zero, and
/// the crowding bands 10, 20 and 40 are each some county's.
///
/// Only ever zero here: `+0x274` (the no-event line is asserted; the event test
/// below moves it).
///
/// Ablations, run: the crowding arm `20 => 9` → `20 => 11` → *"Average herd
/// crowding."* not at `(0x40, 0xA0)`; `wrapping_sub` → `wrapping_add` in the
/// farming row → `"+6"` not at `(0x130, 0xF8)`.
#[test]
fn the_cattle_popup_draws_the_herd_its_crowding_and_three_signed_rows() {
    let (mut game, assets) = england_world!();
    let k = &game.kingdom;
    let crowded = county_where(k, "a herd at crowding 20 that no one eats", |c| {
        c.herd_crowding == 20 && c.herd_eaten == 0 && c.herd_births_expected > c.herd_deaths_expected
    });
    let eaten = county_where(k, "a herd being eaten", |c| c.herd_eaten > 0 && c.herd_crowding == 10);
    let shrinking =
        county_where(k, "a massively overcrowded herd", |c| c.herd_crowding == 40 && c.herd_change_expected < 0);

    for (id, band) in [(crowded, 9), (eaten, 8), (shrinking, 11)] {
        let c = game.kingdom.counties[id].clone();
        let canvas = draw_job(&mut game, &assets, id, JOB_CATTLE_FARMING);
        count_at(&canvas, &assets, c.herd, 4, 0x130, 0x88);
        word_at(&canvas, &assets, 77, band, 0x40, 0xA0);
        word_at(&canvas, &assets, 77, 0x13, 0x40, 0xB0);
        word_at(&canvas, &assets, 77, 5, 0x40, 0xD8);
        count_at(&canvas, &assets, c.herd_births_expected, 4, 0x130, 0xD8);
        word_at(&canvas, &assets, 77, 6, 0x40, 0xE8);
        count_at(&canvas, &assets, c.herd_deaths_expected, 4, 0x130, 0xE8);
        word_at(&canvas, &assets, 77, 7, 0x40, 0xF8);
        signed_at(&canvas, &assets, c.herd_births_expected - c.herd_deaths_expected, 0xF8);
        word_at(&canvas, &assets, 77, 0x1B, 0x40, 0x108);
        signed_at(&canvas, &assets, -c.herd_eaten, 0x108);
        word_at(&canvas, &assets, 77, 0x1C, 0x40, 0x118);
        signed_at(&canvas, &assets, c.herd_change_expected, 0x118);
    }
}

/// **The weather's line, both signs and none**, with advanced farming on and
/// `Herd_SeasonTick` writing `+0x270` from a *Sunny*, a *Frost* and a *Cloudy*
/// band. The band is the hand-set input; the figure is the rule's.
///
/// Ablation, run: the `v < 0` arm of `weather_line` given `0x10` → 77/0x11 not
/// at the chained x on `0xC0`.
#[test]
fn the_cattle_popup_says_what_the_weather_did_to_the_herd() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    game.kingdom.options.advanced_farming = true;
    let id = county_where(&game.kingdom, "the player's, with a herd", |c| c.owner == player && c.herd > 40);

    for (weather, index) in [(Weather::Sunny, 0x10), (Weather::Frost, 0x11), (Weather::Cloudy, 0x12)] {
        {
            let t = game.kingdom.tables;
            let c = &mut game.kingdom.counties[id];
            c.weather = weather;
            l2_kingdom::land::herd_season_tick(&t, c, 1, 2);
        }
        let v = game.kingdom.counties[id].herd_weather_change;
        let canvas = draw_job(&mut game, &assets, id, JOB_CATTLE_FARMING);
        match index {
            0x12 => {
                assert_eq!(v, 0, "setup: Cloudy leaves the herd alone");
                word_at(&canvas, &assets, 77, 0x12, 0x40, 0xC0);
            }
            _ => {
                assert!(v != 0, "setup: {weather:?} moves the herd");
                let at = count_at(&canvas, &assets, v.abs(), 4, 0x40, 0xC0);
                word_at(&canvas, &assets, 77, index, at, 0xC0);
            }
        }
    }
}

// ------------------------------------------------------------------ events

/// Run the machine until the message scroll is up.
fn open_the_scroll(m: &mut Machine, game: &mut Game, assets: &Assets) {
    for _ in 0..8 {
        let mut ctx = Ctx { game: &mut *game, assets };
        m.update(&mut ctx);
        if m.top_id() == Some(ScreenId::Message) {
            return;
        }
    }
    panic!("Msg_Pump never raised the window; the screen is {:?}", m.top_id());
}

/// **The letter, posted the way the game posts it** — select the county and let
/// the frame driver run. `Machine::update` calls `l2_game::message::post_event`,
/// the port of `FUN_00448D7E`, exactly where `Battle_Frame` calls
/// `FUN_00448d7e(g_selectedCounty)`; nothing here enqueues by hand.
fn letter(game: &mut Game, assets: &Assets, county: usize) -> Canvas {
    assert!(game.kingdom.counties[county].event_fired, "setup: the county has a letter waiting");
    assert!(game.select(county as u8), "setup: the county can be selected");
    let mut m = Machine::new(ScreenId::Campaign);
    open_the_scroll(&mut m, game, assets);
    let record = game.messages.open().copied().expect("the scroll is up");
    assert_eq!(record.category, category::EVENT, "and it is the event letter");
    assert_eq!(record.county as usize, county);
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    m.draw(&ctx, &mut canvas);
    canvas
}

/// **An event's toll is a number and a word, on the popup and in the letter.**
///
/// *Rats* on a county with grain and *Mad cows* on one with a herd, each fired
/// by `l2_kingdom::event::fire` and turned into a figure by the season's own
/// tick — `Grain_SeasonTick` writes `+0x278` and `Herd_SeasonTick` `+0x274` —
/// so no figure here is typed. Then, for each:
///
/// * the job popup's `0xB0` row: `Ui_DrawCount(figure, noun, 0x40, 0xB0)` and
///   77/0x19 *"eaten by rats."* or 77/0x14 *"died of disease."*;
/// * the letter, `Msg_DrawWindow`'s category `0x0F` arm in a window at
///   `(0x20, 0xA0)`: the group's label centred in `(0x30, 0x180)` on `0xC0`, and
///   the same count and word at `(0x40, 0x130)`.
///
/// Ablations, run: `draw_event`'s `pen.eng(…word…)` deleted → *"eaten by
/// rats."* not at `(0x94, 0x130)`; the popup's `0x87` word deleted → the same
/// not at `(0x94, 0xB0)`; `Shape::Event => draw_event` removed, so
/// `draw_notice` draws the letter → *"Rats!!"* not at `(0xC7, 0xC0)`, because
/// the notice puts the county's name there.
#[test]
fn a_random_event_s_toll_is_drawn_on_the_popup_and_in_its_letter() {
    let (mut game, assets) = england_world!();
    let quirks = game.kingdom.options.quirks;

    // Rats. The player's own county: the letter is posted only when
    // `county.owner == g_localPlayer`, so a rival's rats reach nobody.
    let me = game.player;
    let rats = county_where(&game.kingdom, "the player's", |c| c.owner == me);
    // Stocking the barn is setup; the figure below is still the rule's.
    game.kingdom.counties[rats].grain = 1_000;
    {
        let t = game.kingdom.tables;
        let c = &mut game.kingdom.counties[rats];
        let mut purse = RealmPurse::default();
        assert!(l2_kingdom::event::fire(c, rats, &mut purse, EventKind::Rats, Season::Summer, quirks));
        l2_kingdom::land::grain_season_tick(&t, c, Season::Summer, false, quirks);
    }
    let c = game.kingdom.counties[rats].clone();
    assert_eq!(c.event_id, 0x87, "setup: the county's event is Rats");
    assert!(c.grain_event_change > 0, "setup: the rats ate something");
    let canvas = draw_job(&mut game, &assets, rats, JOB_GRAIN_FARMING);
    let at = count_at(&canvas, &assets, c.grain_event_change, 2, 0x40, 0xB0);
    word_at(&canvas, &assets, 77, 0x19, at, 0xB0);

    let canvas = letter(&mut game, &assets, rats);
    let label = eng(&assets, 0x87, 0);
    let h = heading(&assets);
    let x = 0x30 + ((0x180 - h.width(&label)) / 2).max(0);
    expect_at(&canvas, h, &label, INK, x, 0xC0);
    let at = count_at(&canvas, &assets, c.grain_event_change, 2, 0x40, 0x130);
    word_at(&canvas, &assets, 77, 0x19, at, 0x130);

    // Mad cows, on a fresh position so the first letter is not still up.
    let (mut game, assets) = england_world!();
    let me = game.player;
    let cows = county_where(&game.kingdom, "the player's and grazing 40 head", |c| {
        c.owner == me && c.herd >= 40
    });
    assert!(game.kingdom.counties[cows].herd >= 40, "setup: a herd the malady can thin");
    {
        let t = game.kingdom.tables;
        let c = &mut game.kingdom.counties[cows];
        let mut purse = RealmPurse::default();
        assert!(l2_kingdom::event::fire(c, cows, &mut purse, EventKind::MadCows, Season::Spring, quirks));
        l2_kingdom::land::herd_season_tick(&t, c, 1, 2);
    }
    let c = game.kingdom.counties[cows].clone();
    assert_eq!(c.event_id, 0x88, "setup: the county's event is Mad cows");
    assert!(c.herd_event_change > 0, "setup: the malady killed something");
    let canvas = draw_job(&mut game, &assets, cows, JOB_CATTLE_FARMING);
    let at = count_at(&canvas, &assets, c.herd_event_change, 4, 0x40, 0xB0);
    word_at(&canvas, &assets, 77, 0x14, at, 0xB0);

    let canvas = letter(&mut game, &assets, cows);
    let at = count_at(&canvas, &assets, c.herd_event_change, 4, 0x40, 0x130);
    word_at(&canvas, &assets, 77, 0x14, at, 0x130);
}

/// **Wedding fever's number line, which used to be the one the painter left
/// blank.**
///
/// `siege-aftersie.sav` holds a county whose stored event is `0x8E` and whose
/// `eventFired` is still set, because nobody ever selected it —
/// `Event_RollAll` does not clear the latch and `FUN_00448D7E` is the only thing
/// that does. So the letter this fixture produces is one the *original's own
/// player* never read.
///
/// The line is `Ui_DrawNumber(+0x2F8, '@', " ", 0x40, 0x130)` and 77/`0x1E`
/// *"extra births."* after it. `+0x2F8` is `County::event_population_swing`,
/// imported (`docs/stored-fields.json`, `County+0x2F8`), so the figure is the
/// save's own.
///
/// Ablations, run: the `0x8E` arm's `Line::Number` deleted → *"extra births."*
/// not on row `0x130`; `Shape::Event => draw_event` removed → *"Wedding
/// fever."* not at `(0x93, 0xC0)`, because `draw_notice` puts the county's name
/// there.
#[test]
fn the_two_population_events_draw_the_swing_the_season_computed() {
    for (kind, id, word, season) in [
        (EventKind::WeddingFever, 0x8Eusize, 0x1E, Season::Spring),
        (EventKind::Plague, 0x8A, 0x1D, Season::Winter),
    ] {
        let (mut game, assets) = england_world!();
        let quirks = game.kingdom.options.quirks;
        let me = game.player;
        let county = county_where(&game.kingdom, "the player's", |c| c.owner == me);
        {
            let t = game.kingdom.tables;
            let c = &mut game.kingdom.counties[county];
            // Both guards want 100 people, and Wedding fever wants happiness 30.
            c.population = c.population.max(600);
            c.happiness = c.happiness.max(40);
            let mut purse = RealmPurse::default();
            assert!(l2_kingdom::event::fire(c, county, &mut purse, kind, season, quirks));
            // `Population_UpdateAll` is what turns the percentage into `+0x2F8`.
            l2_kingdom::population::update_one(&t, c, season, quirks);
        }
        let swing = game.kingdom.counties[county].event_population_swing;
        assert!(swing > 0, "setup: {kind:?} moved the births or the deaths");

        let canvas = letter(&mut game, &assets, county);
        let label = eng(&assets, id, 0);
        let h = heading(&assets);
        expect_at(&canvas, h, &label, INK, 0x30 + ((0x180 - h.width(&label)) / 2).max(0), 0xC0);
        // `Ui_DrawNumber(v, '@', " ", x, y)`: the blank lead puts the digits one
        // column right of `x`, and the one-space suffix (`DAT_004D7050` /
        // `DAT_004D7054`, both `" "`) carries the pen to the word.
        let f = body(&assets);
        expect_at(&canvas, f, &swing.to_string(), INK, 0x40 + 4, 0x130);
        // One `Ui_DrawText` over lead + digits + suffix, so one trailer.
        let x = 0x40 + advance(f, &format!("@{swing} "));
        word_at(&canvas, &assets, 77, word, x, 0x130);
    }
}

/// **The stale letter the original's own player never read.**
///
/// `siege-aftersie.sav` holds a county whose stored `eventId` is `0x8E` and
/// whose `eventFired` is *still set*, several seasons after the Wedding fever:
/// `Event_RollAll` does not clear the latch and `FUN_00448D7E` is the only thing
/// that does, so the letter waited for a click that never came.
///
/// Its figure is gone, though. `Population_UpdateAll` zeroes `+0x2F8` in every
/// county at the top of every season and only rewrites it when `+0x1FB` is
/// non-zero — and `Event_RollAll` *does* clear `+0x1FB`. So the letter, opened
/// now, reads **`0 extra births.`**: a real letter about a real event, with a
/// figure a later season took away. That is the original's behaviour and it is
/// the shape of the whole bug the latch produces.
#[test]
fn a_letter_left_waiting_for_seasons_prints_a_figure_its_season_has_zeroed() {
    let (mut game, assets) = fixture_world!("siege-aftersie.sav");
    let me = game.player;
    let id = county_where(&game.kingdom, "the player's and under Wedding fever", |c| {
        c.owner == me && c.event_id == 0x8E && c.event_fired
    });
    assert_eq!(
        game.kingdom.counties[id].event_population_swing, 0,
        "the save's own figure: the season that cleared +0x1FB took +0x2F8 with it"
    );
    let canvas = letter(&mut game, &assets, id);
    let label = eng(&assets, 0x8E, 0);
    let h = heading(&assets);
    expect_at(&canvas, h, &label, INK, 0x30 + ((0x180 - h.width(&label)) / 2).max(0), 0xC0);
    let f = body(&assets);
    expect_at(&canvas, f, "0", INK, 0x40 + 4, 0x130);
    word_at(&canvas, &assets, 77, 0x1E, 0x40 + advance(f, "@0 "), 0x130);
}

// ---------------------------------------------------------------- industry

/// **`Panel_JobIndustry` for iron, wood and stone, on a save whose blacksmiths
/// are working.** `battle-during.sav` has a county making weapons from both
/// wood and iron, so `+0x280` and `+0x284` — the two figures C164 found drawn
/// by this painter and read by nothing of ours — are non-zero, and so are the
/// outputs. Advanced farming is switched on, as a player does from the options
/// screen, for the efficiency line.
///
/// **The county is chosen so the two smiths' figures differ** (42 and 21).
/// `siege-old_turn.sav`'s county is 56 and 56, where swapping them in the
/// painter stays green — an ablation that cannot fail is a test that cannot.
///
/// Only ever zero here: stone's output and `+0x288`/`+0x28C` (no castle is
/// being built; the castle test below makes the stone figure move). **And the
/// output and efficiency cannot tell wood from iron on any save here**: every
/// county that has both produces the same amount of each at the same 80%.
///
/// Ablations, run: `smiths_iron` → `smiths_wood` in the iron arm → `"21"` not at
/// `(0x44, 0xC0)`; the efficiency's `"%"` → `""` → `"80%"` not at
/// `(0x137, 0xA0)`. **And one stayed green, which is the finding above:**
/// `JOB_IRON_MINING => (Commodity::Wood, …)`, the iron popup reading the wood
/// record, passes — 31 and 31, 80 and 80.
#[test]
fn the_industry_popup_counts_output_and_what_the_blacksmiths_will_use() {
    let (mut game, assets) = fixture_world!("battle-during.sav");
    let t = game.kingdom.tables;
    let id = county_where(&game.kingdom, "smithing unequal wood and iron", |c| {
        let f = l2_kingdom::industry::panel_figures(&t, c);
        f[0] > 0 && f[1] > 0 && f[0] != f[1] && c.industry[Commodity::Iron.index()].next_season > 0
    });
    game.kingdom.options.advanced_farming = true;
    let c = game.kingdom.counties[id].clone();
    let [smiths_wood, smiths_iron, castle_wood, castle_stone] =
        l2_kingdom::industry::panel_figures(&t, &c);

    for (job, record, noun) in [
        (JOB_IRON_MINING, Commodity::Iron, 0x0C),
        (JOB_WOOD_CUTTING, Commodity::Wood, 0x10),
        (JOB_STONE_QUARRYING, Commodity::Stone, 0x0E),
    ] {
        let r = c.industry[record.index()];
        let canvas = draw_job(&mut game, &assets, id, job);
        // `Eng_DrawString(76, 0, 0x40, 0xA0)` + `Ui_DrawNumber(eff, '@', "%", pen + 0x40)`.
        let at = word_at(&canvas, &assets, 76, 0, 0x40, 0xA0);
        expect_at(&canvas, body(&assets), &format!("{}%", r.efficiency), INK, at + 4, 0xA0);
        let at = count_at(&canvas, &assets, r.next_season, noun, 0x40, 0xB0);
        word_at(&canvas, &assets, 76, 1, at, 0xB0);
        match job {
            JOB_IRON_MINING => {
                let at = count_at(&canvas, &assets, smiths_iron, noun, 0x40, 0xC0);
                word_at(&canvas, &assets, 76, 2, at, 0xC0);
            }
            JOB_WOOD_CUTTING => {
                let at = count_at(&canvas, &assets, smiths_wood, noun, 0x40, 0xC0);
                word_at(&canvas, &assets, 76, 2, at, 0xC0);
                let at = count_at(&canvas, &assets, castle_wood, noun, 0x40, 0xD0);
                word_at(&canvas, &assets, 76, 3, at, 0xD0);
            }
            _ => {
                let at = count_at(&canvas, &assets, castle_stone, noun, 0x40, 0xC0);
                word_at(&canvas, &assets, 76, 3, at, 0xC0);
            }
        }
    }
}

// ------------------------------------------------------------------ castle

/// **`Castle_DrawStatusBlock` via `FUN_00414220`, for a castle the rule is
/// building.** A county of the player's orders the next castle up with an empty
/// store (`industry::order_castle`, the build screen's order), so both materials
/// are owed and `Castle_BuildEstimate` answers a hundred seasons. Every figure
/// on the block is then non-zero except one.
///
/// The column is `x + 0x60 = 0x40` and the rows `y + 0x68 …` = `0xA8`, `0xB8`,
/// `0xD0`, `0xE0`, `0xF0`. The stone quarry's popup is checked too while the
/// stone is owed, because `+0x28C` is that figure.
///
/// **Only ever zero here: the builders.** Delivering the materials opens the
/// castle's ceiling (`labour_useful` 1500) and even an industry split of 100
/// staffs nobody, because castle building's share at `+0x130 + 3*4` is 0 after
/// `order_castle` — wood cutting takes all 435. So a finite estimate is not
/// drawn by any test either. Measured and not chased: whether `Castle_Order`
/// leaves that share alone too was not read.
///
/// Ablations, run: `CASTLE_TAX_BONUS_BASE + type` → `+ 1 + type` → `"125"` not
/// at `(0x106, 0xA8)`; `seasons == 0` inverted → the builders' `"0"` not at
/// `(0x44, 0xF0)`.
#[test]
fn a_castle_under_construction_reports_materials_builders_and_seasons() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    let k = &mut game.kingdom;
    let id = county_where(k, "the player's, with a keep", |c| c.owner == player && c.castle_type == 3);
    let owner = k.counties[id].owner as usize;
    k.realms[owner].stone = 0;
    k.realms[owner].wood = 0;
    let t = k.tables;
    assert!(
        l2_kingdom::industry::order_castle(&t, &mut k.counties[id], &mut k.realms[owner], 4),
        "setup: a stone castle may be ordered over a keep"
    );
    let c = k.counties[id].clone();
    assert!(c.castle_stone_owed > 0 && c.castle_wood_owed > 0, "setup: both owed: {c:?}");
    assert_eq!(l2_kingdom::industry::castle_seasons_left(&t, &c), 100, "setup: nothing delivered");

    let canvas = draw_job(&mut game, &assets, id, JOB_CASTLE_BUILDING);
    let f = body(&assets);
    // 71/0x10 + `Ui_DrawNumber(125, ' ', " %", …)` — a stone castle's bonus.
    let at = word_at(&canvas, &assets, 71, 0x10, 0x40, 0xA8);
    expect_at(&canvas, f, "125", INK, at + 4, 0xA8);
    // 71/0xB + `Ui_DrawNumber(400, ' ', " ", …)` + 71/0xC.
    let at = word_at(&canvas, &assets, 71, 0x0B, 0x40, 0xB8);
    expect_at(&canvas, f, "400", INK, at + 4, 0xB8);
    word_at(&canvas, &assets, 71, 0x0C, at + advance(f, " 400 "), 0xB8);
    let at = count_at(&canvas, &assets, c.castle_stone_owed, 0x0E, 0x40, 0xD0);
    word_at(&canvas, &assets, 71, 6, at, 0xD0);
    let at = count_at(&canvas, &assets, c.castle_wood_owed, 0x10, 0x40, 0xE0);
    word_at(&canvas, &assets, 71, 7, at, 0xE0);
    let at = count_at(&canvas, &assets, c.labour[JOB_CASTLE_BUILDING], 0x26, 0x40, 0xF0);
    let at = word_at(&canvas, &assets, 71, 8, at, 0xF0);
    count_at(&canvas, &assets, 100, 0x42, at, 0xF0);

    // The quarry popup's second line is the stone still owed.
    let canvas = draw_job(&mut game, &assets, id, JOB_STONE_QUARRYING);
    let at = count_at(&canvas, &assets, c.castle_stone_owed, 0x0E, 0x40, 0xC0);
    word_at(&canvas, &assets, 76, 3, at, 0xC0);

}

/// **A county with no castle reads its barracks from the neighbouring table.**
///
/// `siege-aftersie.sav` has a county of the player's with `castleType` 0, which
/// is stored and not staged. `Castle_DrawStatusBlock` indexes
/// `&DAT_004D8A0C + type * 4`, so type 0 reads the last word of
/// `CASTLE_WORKFORCE`, **2500**, and `&DAT_004D8A24 + 0` reads the garrison
/// table's trailing **0**. `[V]` on the bytes and the painter; `[I]` that a
/// player sees it — the original was not run for this.
///
/// Only ever zero here: both materials, and `+0x1A6` (the no-build line is what
/// is asserted).
///
/// Ablations, run: `CASTLE_BARRACKS_BASE` → `(0x004D_8A10 - 0x004D_89E8) / 4`
/// → `"2500"` not at `(0xB5, 0xB8)`; `seasons == 0` inverted → *"No castle
/// building in progress."* not at `(0x40, 0xF0)`.
#[test]
fn a_county_with_no_castle_reads_barracks_for_2500_from_the_next_table() {
    let (mut game, assets) = fixture_world!("siege-aftersie.sav");
    let player = game.player;
    let id = county_where(&game.kingdom, "the player's with no castle", |c| {
        c.owner == player && c.castle_type == 0 && c.castle_degraded == 0
    });
    let canvas = draw_job(&mut game, &assets, id, JOB_CASTLE_BUILDING);
    let f = body(&assets);
    let at = word_at(&canvas, &assets, 71, 0x10, 0x40, 0xA8);
    expect_at(&canvas, f, "0", INK, at + 4, 0xA8);
    let at = word_at(&canvas, &assets, 71, 0x0B, 0x40, 0xB8);
    expect_at(&canvas, f, "2500", INK, at + 4, 0xB8);
    word_at(&canvas, &assets, 71, 0x0C, at + advance(f, " 2500 "), 0xB8);
    let at = count_at(&canvas, &assets, 0, 0x0E, 0x40, 0xD0);
    word_at(&canvas, &assets, 71, 6, at, 0xD0);
    let at = count_at(&canvas, &assets, 0, 0x10, 0x40, 0xE0);
    word_at(&canvas, &assets, 71, 7, at, 0xE0);
    word_at(&canvas, &assets, 71, 0x11, 0x40, 0xF0);
}

// ------------------------------------------- the tile panel's same two groups
//
// `TileInfo_DrawCastle` (`0x0041DA2F`) and `TileInfo_DrawGrain`/`…Herd` draw
// groups 71 and 77 out of a county exactly as the job popup does, at a y built
// from `DAT_00553D2C` instead of a literal. The helpers above measure both, so
// the tile panel's arms are checked here rather than in a second copy of them.

/// The tile half of screen `0x04` for one tile, drawn on its own.
fn draw_tile_panel(game: &mut Game, assets: &Assets, tile: usize) -> Canvas {
    let mut screen = l2_game::screens::info::InfoScreen::new(
        l2_game::screens::info::Target::Tile(tile),
    );
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

/// A tile of `county` carrying a standing castle — terrain `0x15 … 0x19`,
/// `Castle_StampTile`'s own range. Found from the map and not from the
/// predicate under test.
#[track_caller]
fn a_castle_tile(k: &Kingdom, county: usize) -> usize {
    let map = &k.campaign.map;
    (0..map.terrain.len())
        .find(|&t| {
            map.county[t] as usize == county
                && (l2_kingdom::map::terrain::CASTLE_FROM..=l2_kingdom::map::terrain::CASTLE_TO)
                    .contains(&map.terrain[t])
        })
        .unwrap_or_else(|| panic!("setup: county {county} has no castle tile"))
}

/// **`TileInfo_DrawCastle`'s intact arm**, which was not drawn at all: a
/// player right-clicking his own castle got a heading and an empty box.
///
/// `FUN_0041BEFE` gives an intact, unruined castle row `0x0E`, so every y is
/// `0x0E * 0x10 + k`. The tax and barracks words are the same two table reads
/// `Castle_DrawStatusBlock` makes, one word low, so a Norman keep (type 3)
/// prints `CASTLE_TAX_BONUS_PCT[2]` = 100 and `CASTLE_GARRISON_CAP[2]` = 200 —
/// the shift `a_county_with_no_castle_reads_barracks_for_2500_from_the_next_table`
/// pins at the other end of the run.
///
/// The garrison line has **no ownership gate on the button**: 71/13 after the
/// men for your own, 71/19 for somebody else's, and 71/14 with the widget
/// under either.
///
/// Ablations, run: the heading arm's `_ => CASTLE_HEADING` → `0x0E` → 30/8 not
/// at `(0x28, 0x120)`; the `CASTLE_TROOPS` line deleted → 71/12 not at its
/// chained x on `0x178`.
#[test]
fn the_tile_panel_draws_an_intact_castles_tax_bonus_barracks_and_garrison() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    let id = county_where(&game.kingdom, "the player's, with a keep", |c| {
        c.owner == player && c.castle_type == 3 && c.castle_degraded == 0 && !c.castle_ruined
    });
    let tile = a_castle_tile(&game.kingdom, id);
    assert_eq!(game.kingdom.counties[id].garrison_unit, 0, "setup: nobody is barracked yet");

    const R: i32 = 0x0E * 0x10;
    let canvas = draw_tile_panel(&mut game, &assets, tile);
    let f = body(&assets);
    // The heading, 30/8 *"Castle."*, in `&g_fontHeading`.
    expect_at(&canvas, heading(&assets), &eng(&assets, 30, 8), INK, 0x28, R + 0x40);
    // 71/16 + `Ui_DrawNumber(100, ' ', " %", …)`.
    let at = word_at(&canvas, &assets, 71, 0x10, 0x68, R + 0x88);
    expect_at(&canvas, f, "100", INK, at + 4, R + 0x88);
    // 71/11 + `Ui_DrawNumber(200, ' ', " ", …)` + 71/12.
    let at = word_at(&canvas, &assets, 71, 0x0B, 0x68, R + 0x98);
    expect_at(&canvas, f, "200", INK, at + 4, R + 0x98);
    word_at(&canvas, &assets, 71, 0x0C, at + advance(f, " 200 "), R + 0x98);
    // `Widget_Draw`'s count is `garrisonUnit != 0`, so there is no button.
    assert!(
        !is_at(&canvas, f, &eng(&assets, 71, 0x0E), INK, 0x68, R + 0xC4),
        "an empty castle offers no \"View these troops?\""
    );

    // **A garrison, and then the other owner's.** The fixture ships merchants
    // and no army; the painter reads only `owner` and `+0x168`, and the branch
    // it picks is the owner test.
    let unit = game
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
        .map(|(id, _)| id)
        .expect("the fixture ships six merchants");
    game.kingdom.counties[id].garrison_unit = unit;
    {
        let u = game.kingdom.campaign.units.get_mut(unit).expect("the merchant");
        u.owner = player;
        u.men = 40;
    }
    let canvas = draw_tile_panel(&mut game, &assets, tile);
    expect_at(&canvas, f, "40", INK, 0x68 + 4, R + 0xA8);
    word_at(&canvas, &assets, 71, 0x0D, 0x68 + advance(f, " 40 "), R + 0xA8);
    word_at(&canvas, &assets, 71, 0x0E, 0x68, R + 0xC4);

    let others = (1..=5u8).find(|&r| r != player).expect("another realm");
    game.kingdom.campaign.units.get_mut(unit).expect("the merchant").owner = others;
    let canvas = draw_tile_panel(&mut game, &assets, tile);
    word_at(&canvas, &assets, 71, 0x13, 0x68, R + 0xA8);
    word_at(&canvas, &assets, 71, 0x0E, 0x68, R + 0xC4);
}

/// **`TileInfo_DrawCastle`'s degraded arm** — the other half, and the one the
/// panel shares with the job page: `Castle_DrawStatusBlock(county, 8, 0x30,
/// DAT_00553D2C)` under `Ui_DrawCount(labour[3], 0x26, 0x68, R + 0x88)`.
///
/// `FUN_0041BEFE` gives it row `0x0A`, the tallest tile layout in the game, and
/// only to the county's owner. So the block's column is `8 + 0x60 = 0x68` and
/// its rows start at `0x0A * 0x10 + 0x30 + 0x68`.
///
/// Ablations, run: the layout's `Some(_) if mine => 0x0A` → `0x0E` → 30/14 not
/// at `(0x28, 0xE0)`; `castle_status_block(…, 8, 0x30, …)` → `(-0x20, 0x40, …)`,
/// the job page's origin → 71/16 not at `(0x68, 0x138)`.
#[test]
fn the_tile_panel_of_a_castle_under_construction_is_the_status_block() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    let id = county_where(&game.kingdom, "the player's, with a keep", |c| {
        c.owner == player && c.castle_type == 3
    });
    let tile = a_castle_tile(&game.kingdom, id);
    let k = &mut game.kingdom;
    let owner = k.counties[id].owner as usize;
    k.realms[owner].stone = 0;
    k.realms[owner].wood = 0;
    let t = k.tables;
    assert!(
        l2_kingdom::industry::order_castle(&t, &mut k.counties[id], &mut k.realms[owner], 4),
        "setup: a stone castle may be ordered over a keep"
    );
    let c = k.counties[id].clone();
    assert_eq!(c.castle_degraded, 1, "setup: work is under way");
    assert!(c.castle_stone_owed > 0 && c.castle_wood_owed > 0, "setup: both owed");

    const R: i32 = 0x0A * 0x10;
    // `Castle_DrawStatusBlock`'s `row * 0x10 + y` with `y = 0x30`.
    const TOP: i32 = R + 0x30;
    let canvas = draw_tile_panel(&mut game, &assets, tile);
    let f = body(&assets);
    // 30/14 *"Castle under construction."* — the heading the degraded byte picks.
    expect_at(&canvas, heading(&assets), &eng(&assets, 30, 0x0E), INK, 0x28, R + 0x40);
    count_at(&canvas, &assets, c.labour[JOB_CASTLE_BUILDING], 0x26, 0x68, R + 0x88);
    let at = word_at(&canvas, &assets, 71, 0x10, 0x68, TOP + 0x68);
    expect_at(&canvas, f, "125", INK, at + 4, TOP + 0x68);
    let at = word_at(&canvas, &assets, 71, 0x0B, 0x68, TOP + 0x78);
    expect_at(&canvas, f, "400", INK, at + 4, TOP + 0x78);
    word_at(&canvas, &assets, 71, 0x0C, at + advance(f, " 400 "), TOP + 0x78);
    let at = count_at(&canvas, &assets, c.castle_stone_owed, 0x0E, 0x68, TOP + 0x90);
    word_at(&canvas, &assets, 71, 6, at, TOP + 0x90);
    let at = count_at(&canvas, &assets, c.castle_wood_owed, 0x10, 0x68, TOP + 0xA0);
    word_at(&canvas, &assets, 71, 7, at, TOP + 0xA0);
}

/// **The field panel's four weather and event figures**, which the module docs
/// in `screens/info.rs` said were not carried — `docs/stored-fields.json` has
/// all four *imported*, and `Panel_JobGrain` has been drawing them since C164.
///
/// `TileInfo_DrawGrain` and `TileInfo_DrawHerd` put them at `(0x28, R + 0x94)`
/// and `(0x28, R + 0xA4)` where the job popup uses `(0x40, 0xB0)` and
/// `(0x40, 0xC0)`; `FUN_0041BEFE` gives a real field of the player's `R = 5`.
/// Both signs of the weather and one event of each pair.
///
/// Ablations, run: the grain weather line's `0xA4` → `0x94` → the event's
/// `"-25"` not at `(0x2C, 0xE4)`, the weather having painted over it; the herd
/// event's `0x89 => Some(0x15)` → `Some(0x14)` → *"taken by wolves."* not at
/// `(0x8C, 0xE4)`.
#[test]
fn the_field_panel_says_what_the_weather_and_the_events_did() {
    use l2_game::screens::info::{mode, FARM_TILE_INFO};
    let (mut game, assets) = england_world!();
    let player = game.player;
    game.kingdom.options.advanced_farming = true;
    // A field of the player's in one of the two modes with a report, and the
    // county it belongs to — the realm's counties are rolled per game and no
    // one of them need carry both.
    let field = |k: &Kingdom, want: usize| {
        let map = &k.campaign.map;
        (0..map.terrain.len())
            .find(|&t| {
                let c = map.county[t] as usize;
                c != 0
                    && c <= k.county_count
                    && k.counties[c].owner == player
                    && map.flags[t] & l2_kingdom::map::flags::FARMLAND != 0
                    && FARM_TILE_INFO
                        .get(map.terrain[t] as usize)
                        .is_some_and(|row| row[3] == want)
            })
            .map(|t| (map.county[t] as usize, t))
            .unwrap_or_else(|| panic!("setup: the player has no field in mode {want:#x}"))
    };
    let (id, pasture) = field(&game.kingdom, mode::CATTLE);
    assert!(game.kingdom.counties[id].herd > 0, "setup: county {id} has a herd to move");
    const R: i32 = 5 * 0x10;

    // **The herd's weather**, both signs and none, through `Herd_SeasonTick`.
    for (weather, index) in
        [(Weather::Sunny, 0x10), (Weather::Frost, 0x11), (Weather::Cloudy, 0x12)]
    {
        {
            let t = game.kingdom.tables;
            let c = &mut game.kingdom.counties[id];
            c.weather = weather;
            l2_kingdom::land::herd_season_tick(&t, c, 1, 2);
        }
        let v = game.kingdom.counties[id].herd_weather_change;
        let canvas = draw_tile_panel(&mut game, &assets, pasture);
        if index == 0x12 {
            assert_eq!(v, 0, "setup: Cloudy leaves the herd alone");
            word_at(&canvas, &assets, 77, 0x12, 0x28, R + 0xA4);
        } else {
            assert!(v != 0, "setup: {weather:?} moves the herd");
            let at = count_at(&canvas, &assets, v.abs(), 4, 0x28, R + 0xA4);
            word_at(&canvas, &assets, 77, index, at, R + 0xA4);
        }
    }

    // **The herd's event** — *Wolves* (`0x89`) takes animals, and 77/21 is the
    // sentence after the figure.
    {
        let c = &mut game.kingdom.counties[id];
        c.event_id = 0x89;
        c.herd_event_change = -7;
    }
    let canvas = draw_tile_panel(&mut game, &assets, pasture);
    let at = count_at(&canvas, &assets, -7, 4, 0x28, R + 0x94);
    word_at(&canvas, &assets, 77, 0x15, at, R + 0x94);

    // **And the grain's pair.** Turn one has no wheat anywhere — every field
    // is fallow or pasture — so the county's fallow fields take the grain
    // brush first, which is how a player makes one.
    assert!(
        paint_all_fallow_to_grain(&mut game.kingdom, id) > 0,
        "setup: county {id} has fallow fields to sow"
    );
    let (grain_id, wheat) = field(&game.kingdom, mode::WHEAT);
    {
        let c = &mut game.kingdom.counties[grain_id];
        c.event_id = 0x87;
        c.grain_event_change = -25;
        c.grain_weather_change = 12;
    }
    let canvas = draw_tile_panel(&mut game, &assets, wheat);
    let at = count_at(&canvas, &assets, -25, 2, 0x28, R + 0x94);
    word_at(&canvas, &assets, 77, 0x19, at, R + 0x94);
    let at = count_at(&canvas, &assets, 12, 2, 0x28, R + 0xA4);
    word_at(&canvas, &assets, 77, 0x10, at, R + 0xA4);
}

// ------------------------------------------------------------- reclamation

/// **`Panel_JobReclamation`, on the stored zeros.** Every save on this machine
/// has no field under reclamation, so this is **only ever zero**: *"0 fields
/// being reclaimed"* on `0xB8` and 77/0xF on `200`. It pins the two lines'
/// places and the plural at zero, which is `Ui_DrawNumber` and not
/// `Ui_DrawCount` here — `(byte) +0x204 == 1` picks 0xC, anything else 0xD.
///
/// Ablation, run: the `'@'` lead → `' '` has no effect on the pixels (both are
/// glyph-less); the `fields == 1` test inverted → 77/0xD not at the chained x.
#[test]
fn the_reclamation_popup_draws_its_two_lines_on_a_county_reclaiming_nothing() {
    let (mut game, assets) = england_world!();
    let player = game.player;
    let id = county_where(&game.kingdom, "the player's", |c| c.owner == player);
    let c = game.kingdom.counties[id].clone();
    assert_eq!((c.fields_reclaiming, c.reclaim_seasons_to_next), (0, 0), "setup: nothing reclaimed");
    let canvas = draw_job(&mut game, &assets, id, JOB_FIELD_RECLAMATION);
    let f = body(&assets);
    expect_at(&canvas, f, "0", INK, 0x40 + 4, 0xB8);
    word_at(&canvas, &assets, 77, 0x0D, 0x40 + 4 + advance(f, "0"), 0xB8);
    word_at(&canvas, &assets, 77, 0x0F, 0x40, 200);
}

// -------------------------------------------------------------- blacksmith

/// Our zero-based job 7, the blacksmith — `g_jobPanelJob == 8`.
const JOB_BLACKSMITH: usize = 7;
/// `g_weaponCost[5]` — armour, **4 wood and 18 iron**, the only row whose two
/// costs differ enough to tell a transposed pair apart at a glance.
const ARMOUR: usize = 5;
/// `DAT_004D29C8[5]` — group 8's singular index for armour, whose plural at 29
/// is *"Armour"* again.
const ARMOUR_NOUN: usize = 28;

/// **`Panel_JobBlacksmith` (`0x00413155`) — its words, its two figures and its
/// two costs**, on a county whose smithy is staffed.
///
/// The line that matters most is `Ui_DrawCentred(75, 0, 0, 0x1CC, 0x1CC)` —
/// *"Click on a weapon to change production."* **`Panel_JobBlacksmith` is group
/// 75's only consumer in the whole binary**, so this is the page's own
/// vocabulary rather than a naming lead (rule 6), and it is the sentence a
/// player reported missing by reporting the control: *"I can't choose what type
/// of weapon my blacksmiths are making."*
///
/// The centred line is asserted for **its row and its ink** and not its x,
/// because `Ui_DrawCentred`'s x is `(width - measure) / 2` over the whole string
/// and re-deriving it here would be re-deriving `FUN_004025D7`. Every other
/// piece is at a literal out of the painter or chained from one.
///
/// Advanced Farming is off in every save on this machine, so the `76/4` + `76/5`
/// branch is **only ever unexercised** and `76/8` — *"Smiths working."* — is the
/// one that runs. Said here rather than left to be discovered.
///
/// Ablations, run: the two cost numbers swapped → `"18"` is at 440 and not at
/// `0x16A + 4`; 76/6's `SMITHY_ROW_OUTPUT` → `SMITHY_ROW_WORKERS` → the smiths
/// figure goes red first, because the moved word is drawn over it;
/// `WEAPON_NOUN[weapon]` → a constant `0x12` (Pike) → *"Armour"* is on no row at
/// all.
#[test]
fn the_blacksmith_page_draws_group_75_and_the_weapon_it_forges() {
    let (mut game, assets) = fixture_world!("siege-lastturn.sav");
    let player = game.player;
    let id = county_where(&game.kingdom, "the player's with a smithy site", |c| {
        c.owner == player && c.industry[Commodity::Weapons.index()].has_resource && c.pop_band > 0
    });
    // **No save on this machine has a staffed smithy**, so the state is reached
    // by the two roads a player reaches it by: the map click that switches the
    // site on (`Industry_ToggleFromMap`) and the village drag that staffs it
    // (`Labour_Move`). A hand-set `labour[7]` would have drawn a page whose
    // forecast nothing computed.
    if !game.kingdom.counties[id].industry[Commodity::Weapons.index()].enabled {
        game.kingdom.toggle_industry(id, l2_kingdom::industry::MapToggle::Industry(Commodity::Weapons));
    }
    let spare = game.kingdom.counties[id].labour[JOB_WOOD_CUTTING] / 2;
    game.kingdom.move_labour(id, JOB_WOOD_CUTTING, JOB_BLACKSMITH, spare);
    assert!(game.kingdom.set_weapon_type(id, ARMOUR), "the click the page answers");
    assert!(game.kingdom.counties[id].labour[JOB_BLACKSMITH] > 0, "setup: the smithy is staffed");
    assert!(
        !game.kingdom.options.advanced_farming,
        "setup: this fixture has Advanced Farming off, so 76/8 is the workers line"
    );
    let c = game.kingdom.counties[id].clone();

    let canvas = draw_job(&mut game, &assets, id, JOB_BLACKSMITH);
    let f = body(&assets);
    let h = heading(&assets);

    // `Eng_DrawString(74, 8, 0x10, 0x186, &g_fontHeading, 0x3F)` — the page's
    // title, in the heading face and not the body's.
    expect_at(&canvas, h, &eng(&assets, 74, 8), INK, 0x10, 0x186);

    // `Ui_DrawCentred(75, 0, 0, 0x1CC, 0x1CC, &g_fontBody, 0x3F)`.
    let say = eng(&assets, 75, 0);
    assert!(
        !xs_on_row(&canvas, f, &say, INK, 0x1CC).is_empty(),
        "{say:?} — the only string of group 75 anything draws — is not on row 0x1CC"
    );

    // `Ui_DrawNumber(labour[7], '@', " ", 0x10, 0x1A4)` then 76/8. The `'@'`
    // lead has no glyph and advances four; the `" "` suffix is a space.
    let smiths = c.labour[JOB_BLACKSMITH].to_string();
    expect_at(&canvas, f, &smiths, INK, 0x10 + 4, 0x1A4);
    word_at(&canvas, &assets, 76, 8, 0x10 + advance(f, &format!("@{smiths} ")), 0x1A4);

    // 76/6, the count in the weapon's own noun, 76/7.
    let at = word_at(&canvas, &assets, 76, 6, 0x10, 0x1B4);
    let made = c.industry[Commodity::Weapons.index()].next_season;
    let at = count_at(&canvas, &assets, made, ARMOUR_NOUN, at, 0x1B4);
    word_at(&canvas, &assets, 76, 7, at, 0x1B4);

    // The cost well. **Iron is drawn first**, at 0x16A, and it is `g_weaponCost`'s
    // *second* word: armour is 4 wood and 18 iron, so these two are the check
    // that the pair is not read the wrong way round.
    expect_at(&canvas, f, "18", INK, 0x16A + 4, 0x18E);
    expect_at(&canvas, f, "4", INK, 0x1B4 + 4, 0x18E);
}

/// **The six weapon hotspots are the player's own exe's**, `DAT_004DCA10` read
/// at a 24-byte stride, plus `Hotspot_Test`'s two offsets.
///
/// `Hotspot_Test(0, 0x18, &DAT_004DCA10, 6)` **adds** its first two arguments to
/// every record before the test, and `0x18` is exactly where
/// `Sprite_WGenSprite(0, 0, 0x18)` puts the smithy picture — so the table is in
/// the picture's coordinates. Read as screen coordinates every weapon sits 24
/// pixels high, and a click a player aims at the pike lands on the bow. That is
/// the defect this test exists to make impossible.
///
/// The record is `{x0, y0, x1, y1}` as `i16`s at `+0x00`, `+0x02`, `+0x04`,
/// `+0x06`, and `Hotspot_Test` is **half-open** on both axes: `x0 <= mx < x1`.
/// `docs/input.md` §1.
///
/// Ablation, run: `HOTSPOT_ORIGIN` → `(0, 0)` turns the two corner claims red.
#[test]
fn the_weapon_hotspots_are_the_exes_own_table_at_the_pictures_origin() {
    use l2_game::screens::job::{weapon_at, HOTSPOT_ORIGIN, WEAPON_HOTSPOTS};

    let exe = l2_testkit::executable!();
    let table = l2_testkit::pe::Table::at(&exe, 0x004D_CA10);
    let (dx, dy) = HOTSPOT_ORIGIN;
    assert_eq!((dx, dy), (0, 0x18), "Hotspot_Test's two offsets, and 0x18 is the picture's y");

    for (w, &(x0, y0, x1, y1)) in WEAPON_HOTSPOTS.iter().enumerate() {
        // Twelve `i16`s a record.
        let at = |field: usize| i32::from(table.u16_at(w * 12 + field) as i16);
        assert_eq!((at(0), at(1), at(2), at(3)), (x0, y0, x1, y1), "weapon {w}'s rectangle");
        // The kind byte at `+0x0F` — 1, the down edge, for all six.
        assert_eq!(table.u8_at(w * 24 + 0x0F), 1, "weapon {w} is kind 1");
        // And `g_uiHotspotId` at `+0x10` is the weapon type itself, which is
        // what makes table order weapon order.
        assert_eq!(table.i32_at(w * 6 + 4), w as i32, "weapon {w}'s g_uiHotspotId");

        // Inside the rectangle, in screen coordinates, is this weapon; the far
        // corner itself is not, because the test is half-open.
        assert_eq!(weapon_at(x0 + dx, y0 + dy), Some(w), "weapon {w}'s near corner");
        assert_eq!(weapon_at(x1 + dx - 1, y1 + dy - 1), Some(w), "weapon {w}'s last pixel");
        assert_ne!(weapon_at(x1 + dx, y1 + dy), Some(w), "weapon {w}'s far corner is past it");
    }
    // The corner picture at (448, 448) is below every one of them, so the OK
    // button and the weapons cannot collide.
    assert_eq!(weapon_at(0x1C0, 0x1C0), None, "the OK button is on no weapon");
}
