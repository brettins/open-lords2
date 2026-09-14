#![allow(unused_imports)]
use super::*;
use super::strip_quadrants_and_sidebar_buttons::*;
use super::strip_slider_and_pointer::*;
use super::*;
use super::panels::*;
use super::drawing_and_emboss::*;
use super::produce_and_pastures::*;
use super::layout_and_labels::*;
use common::*;
use l2_game::game::Assets;
use l2_game::game::MAX_TAX_RATE;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::{self as county};
use l2_game::screens::county::CountyScreen;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::Canvas;

/// **The right button has two jobs, and they are opposite ones.**
///
/// A player said *"right click would close a bunch of popups"*, and he is right:
/// `Screen_FrameInput` — the fourth and unnamed `g_screenId` dispatcher, and the one
/// that decides how every screen is *left* — has a right-release arm for almost
/// every screen id there is, and `L2.eng` group 12 index 0 is the game printing
/// *"Click Right to Exit"* on the value spinner.
///
/// On the campaign map it does the reverse: `if (onATile && rightReleased) {
/// g_screenId = 4; FUN_0043CAF4(); }` opens the information panel, which is the
/// pop-up the shipped `Readme.txt` errata describes on an army.
#[test]
fn the_right_button_closes_a_panel_and_opens_the_map_information_screen() {
    let (mut game, assets) = world!();
    let mut m = Machine::new(ScreenId::Campaign);
    game.select(8);

    // On the map: right-click on a tile opens screen 0x04.
    let screen = MapScreen::new();
    let (px, py) = (240, 240);
    assert!(screen.map_clip().contains(px, py), "that pixel is on the map");
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    m.handle(Event::RightClick { x: px, y: py }, &mut ctx);
    assert!(
        matches!(m.top_id(), Some(ScreenId::Info(_))),
        "the information panel, and it now knows what the click resolved to",
    );

    // And right-click again closes it, which is the same arm from the other
    // side: screen 0x04 has its own right-release branch back to the map.
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    m.handle(Event::RightClick { x: px, y: py }, &mut ctx);
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));

    // A county panel closes on the right button from anywhere, the strip
    // included: every guard in the original's chain tests a *left* press or a
    // left release, so none of them consumes a right one.
    for panel in county::PANELS {
        for (x, y) in [(240, 240), (500, 195), (620, 470)] {
            let mut m = Machine::new(ScreenId::County(8, panel));
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            m.handle(Event::RightClick { x, y }, &mut ctx);
            assert_eq!(m.depth(), 0, "{panel:?} closed by a right click at ({x}, {y})");
        }
    }

    // So does the village and so does the job popup.
    for id in [ScreenId::Village(8), ScreenId::Job(8, 0)] {
        let mut m = Machine::new(id);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::RightClick { x: 200, y: 200 }, &mut ctx);
        assert_eq!(m.depth(), 0, "{id:?} closed by a right click");
    }
}

/// The county strip shows what `CountyStrip_Draw` puts in the 162 × 94 plate,
/// at the coordinates it puts them: population at (508, 189), happiness ending
/// at 602 on the same line, and the tax rate at (506, 226).
///
/// The exact coordinates are the point. A panel *contains* the
/// right digits somewhere would pass a looser test and still be laid out
/// wrongly, which is the mistake this whole task exists to correct.
#[test]
fn the_county_strip_shows_the_saves_numbers_where_the_original_puts_them() {
    let (mut game, assets) = world!();
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);

    let c = &game.kingdom.counties[8];
    assert_eq!((c.population, c.happiness, c.ration_achieved), (435, 72, 3));

    // **The `x` in a call site is where the *string* starts, and the string
    // starts with a sign column.** `Ui_DrawNumber(value, lead, suffix, x, …)`
    // builds `lead + digits + suffix`: `Ui_NumberToBuffer(value, 1, 0)` writes
    // the digits from index **1** and the lead fills index 0. So a call at
    // `0x1FC` puts the *digits* at `0x1FC + SPACE_ADVANCE`.
    //
    // These three assertions used to name the call site's own `x` and were four
    // pixels short, all three, which a player saw: *"Happiness # and population
    // # in the sidebar are slightly left of where they should be."*
    //
    // **The offset is the same for the two-digit happiness and the three-digit
    // population, because a lead is one character whatever the value is.** That
    // is the fingerprint separating this from the right-anchoring cause, which
    // would have displaced the two by *different* amounts — and it could not
    // have applied here anyway, since `Ui_DrawNumber` has no anchoring
    // argument at all. `Ui_DrawNumberRight` is the one that centres.
    let lead = l2_game::shell::font::SPACE_ADVANCE;
    assert_eq!(lead, 4, "the sign column is four pixels wide");
    assert_eq!(
        find_strip(&canvas, &assets, "435", STRIP_INK),
        Some((0x1FC + lead, 189)),
        "the population, at Ui_DrawNumber(pop, ' ', …, 0x1FC, 0xBD) plus its sign column"
    );
    assert_eq!(
        find_strip(&canvas, &assets, "72", STRIP_INK),
        // **Left origins, not right-anchored.** `Ui_DrawNumber` has no
        // only in their value and their x. `docs/decisions.md` C42.
        Some((0x25A + lead, 189)),
        "the happiness, displaced by the same four pixels and not by more"
    );
    assert_eq!(
        find_strip(&canvas, &assets, "0%", STRIP_INK),
        Some((0x1FA + lead, 226)),
        "the tax rate at 0x1FA — its suffix is '%' and its lead is still a space"
    );
    // The county's name comes out of `L2.eng` group 100 at
    // `scenarioIndex * 20 + id` and is drawn in the **body** font, so it is
    // neither of the two the rest of the strip uses.
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 8);
    // Group 100 is twenty strings per map slot: index 0 is the *map's* name
    // ("Here Be Dragons!" for England), 1 … 14 are its fourteen counties and
    // 15 … 19 are the unused `CTY0` padding, so slot 1 starts at index 20 with
    // "The Normans". `scenarioIndex * 20 + countyId` lands on the county's own
    // name with no off-by-one, and county 8 of England is Dyfed.
    assert_eq!(name, "Dyfed", "L2.eng group 100, index map_slot * 20 + 8");
    // rationAchieved == rationWanted, so it is drawn plain.
    assert!(find_strip(&canvas, &assets, "Normal", STRIP_INK).is_some());
    assert!(find_strip(&canvas, &assets, "Normal", STRIP_BAD).is_none());

    // Near misses, one per number, so none of the three can match by accident.
    assert!(find_strip(&canvas, &assets, "436", STRIP_INK).is_none());
    assert!(find_strip(&canvas, &assets, "73", STRIP_INK).is_none());
    assert!(find_strip(&canvas, &assets, "Double", STRIP_INK).is_none());
}


