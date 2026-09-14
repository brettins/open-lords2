#![allow(unused_imports)]
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

/// **The four county panels are reachable from the map, and each from its own
/// quadrant.** `CountyStrip_Click` is the whole navigation into them; the map
/// screen used to carry one button of ours instead, which opened the county
/// screen on whichever panel it defaulted to — so a player could reach the tax
/// panel and no other.
#[test]
fn each_quadrant_of_the_strip_opens_its_own_panel_from_the_campaign_map() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    for panel in county::PANELS {
        let hot = panel.strip_hotspot();
        let t = send(
            &mut screen,
            &mut game,
            &assets,
            Event::Click { x: hot.centre_x(), y: hot.y + hot.h / 2 },
        );
        assert_eq!(t, Transition::Push(ScreenId::County(8, panel)), "{panel:?}'s own quadrant");
    }
    // The thermometer's dead band opens nothing — the gap exists for the bar.
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: 558, y: 200 });
    assert_eq!(t, Transition::Stay, "the health bar is deliberately not clickable");
}

/// **The five sidebar buttons.** `g_sidebarButtons` (`0x004DC680`) is a table of
/// five, and every one of them sets a `g_screenId` we can draw. They were under
/// a rectangle of ours that opened the county panel and wrote COUNTY PANEL
/// across their artwork.
#[test]
fn the_five_sidebar_buttons_each_open_the_screen_the_original_opens() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    assert!(game.is_players(8), "county 8 is the player's, so the gated three are allowed");
    for b in map::SIDEBAR_BUTTONS {
        let r = b.rect();
        let map::SidebarAction::Screen(id) = b.action;
        let t = send(
            &mut screen,
            &mut game,
            &assets,
            Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 },
        );
        // `map::sidebar_destination` is the one place a graduated screen is
        // Named, this asks it:
        // `0x17` is the raise-army screen now, and it takes the county.
        assert_eq!(
            t,
            Transition::Push(map::sidebar_destination(id, 8)),
            "{} opens {id:#04X}",
            b.name
        );
    }

    // Three of the five are gated on the county being yours.
    // `Sidebar_Button` gates them. County 1 belongs to realm 5.
    game.select(1);
    for b in map::SIDEBAR_BUTTONS {
        let r = b.rect();
        let map::SidebarAction::Screen(id) = b.action;
        let t = send(
            &mut screen,
            &mut game,
            &assets,
            Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 },
        );
        if matches!(id, 0x17 | 0x18 | 0x1B) {
            assert_eq!(t, Transition::Stay, "{} is refused on another realm's county", b.name);
        } else {
            assert_eq!(
                t,
                Transition::Push(map::sidebar_destination(id, game.selected)),
                "{} is not gated",
                b.name,
            );
            // The ungated two are the court (`0x09`, still a shell) and the
            // lords (`0x0B`, which has graduated) — so this asks
            // `sidebar_destination` too. The
            // ungating is the point: **the diplomacy screen is about realms,
            // not counties**, and `FUN_0043611B` has no county gate at all.
            assert_eq!(t, Transition::Push(map::sidebar_destination(id, 1)), "{} is not gated", b.name);
        }
    }
}

/// **The farm/industry split slider moves peasants.** `FUN_00439122` is the one
/// control on the campaign screen that reallocates labour in bulk, and it was
/// not wired at all — which is a fair part of *"I can't assign peasants"*.
#[test]
fn the_sidebar_split_slider_moves_the_countys_labour_between_farm_and_industry() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let before = game.kingdom.counties[8].industry_share;

    // x 533 on the track is ((533 - 531) * 2) & 0xFC = 4.
    send(&mut screen, &mut game, &assets, Event::Click { x: 533, y: 270 });
    assert_eq!(game.kingdom.counties[8].industry_share, 4);
    assert_ne!(4, before, "and that is not where it started");

    // The far end of the track is 100 per cent industry.
    send(&mut screen, &mut game, &assets, Event::Click { x: 581, y: 270 });
    assert_eq!(game.kingdom.counties[8].industry_share, 100);

    // The allocation follows it.
    // split: with everybody in the mines, the farm jobs empty.
    let farm: i32 = (0..3).map(|j| game.kingdom.counties[8].labour[j]).sum();
    send(&mut screen, &mut game, &assets, Event::Click { x: 531, y: 270 });
    assert_eq!(game.kingdom.counties[8].industry_share, 0);
    let farm_after: i32 = (0..3).map(|j| game.kingdom.counties[8].labour[j]).sum();
    assert!(farm_after > farm, "0% industry puts more people on the land than 100% did");
}

/// **The blue outline appears, and only when it should.**
///
/// Two separate signals with two separate tests, which is the thing worth
/// pinning down: the **slider's** thumb gains its ring on
/// `county.labour[8].workers != 0` — anybody idle at all — and each **produce
/// icon** gains one on `labour[slot].useful < labour[slot].workers` — too many
/// people on *that* job. A player described both and thought they were the same
/// signal; they are the same picture and different tests.
///
/// Measured by counting pixels of the ring's own three palette entries inside
/// the sidebar, so it is the artwork being asserted and not a description of
/// it.
#[test]
fn the_strip_draws_the_blue_ring_on_the_slider_and_on_the_overstaffed_job() {
    use l2_view::chrome::misc_cty::RING_COLOURS;
    let (mut game, assets) = world!();
    let idle = l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;
    let cattle = l2_kingdom::tables::JOB_CATTLE_FARMING;

    // Count the ring's colours in the sidebar column only.
    let ring_pixels = |canvas: &Canvas| -> usize {
        let mut n = 0;
        for y in 156..430usize {
            for x in 478..640usize {
                if RING_COLOURS.contains(&canvas.at(x, y)) {
                    n += 1;
                }
            }
        }
        n
    };

    // Nobody idle, and the dairy inside its ceiling.
    {
        let c = &mut game.kingdom.counties[8];
        c.labour[idle] = 0;
        c.labour[cattle] = 100;
        c.labour_wanted[cattle] = -1;
        c.labour_useful[cattle] = 200;
        c.herd = 400;
        c.fields_cattle = 4;
    }
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let quiet = ring_pixels(&draw(&mut screen, &mut game, &assets));

    // One idle townsman: the slider's thumb becomes frame 0x55.
    game.kingdom.counties[8].labour[idle] = 1;
    let with_slider = ring_pixels(&draw(&mut screen, &mut game, &assets));
    assert!(
        with_slider > quiet,
        "the slider's thumb gains its ring: {quiet} -> {with_slider} ring pixels"
    );

    // And more people milking than the herd can use: the cow gains one too.
    game.kingdom.counties[8].labour_useful[cattle] = 50;
    let with_both = ring_pixels(&draw(&mut screen, &mut game, &assets));
    assert!(
        with_both > with_slider,
        "the dairy icon gains its own ring: {with_slider} -> {with_both} ring pixels"
    );

    // Putting the ceiling back takes the cow's ring away again and leaves the
    // Slider's: two tests.
    game.kingdom.counties[8].labour_useful[cattle] = 200;
    assert_eq!(ring_pixels(&draw(&mut screen, &mut game, &assets)), with_slider);
}

/// **And it is a drag, not a click.** A player reported *"the peasant slider of
/// industry isn't draggable, should be"*, and `FUN_00439122` agrees: it acts
/// while `DAT_004E65CC` — the button's *level* — is set and `DAT_004EA4B0` says
/// the pointer moved, and does nothing at all on the release. So the value
/// follows the pointer for as long as the button is held, and stops the moment
/// it is let go.
///
/// The whole gesture as a sequence of values, which is the only way to test a
/// drag without an input queue.
#[test]
fn the_split_slider_tracks_the_pointer_while_the_button_is_held() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    game.select(8);
    let share = |g: &Game| g.kingdom.counties[8].industry_share;

    // Press on the track at x = 533, then travel along it without letting go.
    send(&mut screen, &mut game, &assets, Event::Click { x: 533, y: 270 });
    assert_eq!(share(&game), 4, "the press itself sets the value");
    for (x, want) in [(541, 20), (561, 60), (581, 100), (551, 40)] {
        send(&mut screen, &mut game, &assets, Event::Pointer { x, y: 270 });
        assert_eq!(share(&game), want, "held and moved to x = {x}");
    }

    // Off the sidebar entirely and the slider stops, without the drag ending —
// the original re-tests the rectangle every frame and skips.
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 200, y: 270 });
    assert_eq!(share(&game), 40, "outside the rectangle nothing moves");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 561, y: 270 });
    assert_eq!(share(&game), 60, "and coming back resumes the same drag");

    // Let go. Now the same movement does nothing.
    send(&mut screen, &mut game, &assets, Event::Release { x: 561, y: 270 });
    assert_eq!(share(&game), 60, "the release itself changes nothing");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 533, y: 270 });
    assert_eq!(share(&game), 60, "and a bare pointer move is not a drag");

    // Off the track, each move steps by four.
    send(&mut screen, &mut game, &assets, Event::Click { x: 600, y: 270 });
    assert_eq!(share(&game), 64, "right of the track: +4");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 601, y: 270 });
    assert_eq!(share(&game), 68, "and again on the next move");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: 500, y: 270 });
    assert_eq!(share(&game), 64, "left of the track: -4");
}

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

