#![allow(unused_imports)]
use super::*;
use super::job_popups::*;
use super::tile_panel::*;
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
    // `county.owner == g_localPlayer`
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

