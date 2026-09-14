#![allow(unused_imports)]
use super::*;
use super::strip_and_sidebar::*;
use super::panels::*;
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

// ---------------------------------------------------- the county-strip emboss

/// The glyph mask of `s` in the body font, as offsets from the string's origin.
fn body_mask(assets: &Assets, s: &str) -> Vec<(i32, i32)> {
    let font = assets.shell.body.as_ref().expect("the body font");
    let style = l2_game::shell::font::Style { colour: 1, shadow: None, caps: None };
    let (w, h) = (font.width(s).max(1), font.height(s).max(1));
    let mut probe = Canvas::new(w as usize, h as usize);
    font.draw(&mut probe, 0, 0, s, &style);
    (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| probe.at(x as usize, y as usize) == 1)
        .collect()
}

/// **The emboss pair a line was drawn with, read back off the canvas.**
///
/// `Ui_DrawText` draws each glyph three times, in this order: at `y - 1` in the
/// *up* colour, at `y + 1` in the *down* colour, then at `y` in its own. So the
/// final colour of a pixel is decided by which of the three masks it is in,
/// later passes winning:
///
/// * in the glyph mask → the text colour;
/// * else in the mask shifted **down** one → the *down* shadow;
/// * else in the mask shifted **up** one → the *up* shadow.
///
/// Reading those two sets back is exact;
/// every pixel of each set has to agree or this returns `None`. The two shadow
/// colours come out as palette indices, which is the form the binary states
/// them in.
fn emboss_at(canvas: &Canvas, assets: &Assets, s: &str, colour: u8) -> Option<(u8, u8)> {
    let mask = body_mask(assets, s);
    let (ox, oy) = find_body(canvas, assets, s, colour)?;
    let inside = |dx: i32, dy: i32| mask.contains(&(dx, dy));
    let mut up: Option<u8> = None;
    let mut down: Option<u8> = None;
    for &(mx, my) in &mask {
        // One row below a glyph pixel, and not itself a glyph pixel: the
        // *down* shadow, drawn second and never overpainted.
        if !inside(mx, my + 1) {
            let got = canvas.at((ox + mx) as usize, (oy + my + 1) as usize);
            if *down.get_or_insert(got) != got {
                return None;
            }
        }
        // One row above, in neither the glyph mask nor the down mask: the *up*
        // shadow, which is drawn first and so loses both overlaps.
        if !inside(mx, my - 1) && !inside(mx, my - 2) {
            let got = canvas.at((ox + mx) as usize, (oy + my - 1) as usize);
            if *up.get_or_insert(got) != got {
                return None;
            }
        }
    }
    Some((up?, down?))
}

/// **Two different emboss colours in one plate, and one of them is the
/// original's own bug.**
///
/// A player reported both halves and was right about both:
///
/// > *"There's a bug in the original where the town name's embossing against
/// > the cloudy background … still has the emboss colour of the parchment that
/// > you see on a town you own, that blends it in with the parchment. But the
/// > OG correctly has the 'sovereign land of the baron' properly tinged in
/// > grey."*
///
/// `CountyStrip_Draw` (`0x0040F7D3`) is the function, and it draws the name
/// with `DAT_0058FE9C` clear and the three *Sovereign land of …* lines with it
/// set — the only thing in the whole plate that changes it:
///
/// ```c
/// Pl8_DrawFrameHere(g_miscCtySheet, 0x3a, 0x1de, 0x9c);            /* the cloudy plate */
/// Ui_DrawCentred(100, …, 0x1e0, 0xb4, 0xa0, &g_fontBody, 0x3f);    /* the name */
/// if (owner != 0) {
///   DAT_0058fe9c = 1;
///   Ui_DrawCentred(0xf, 0, …);  Ui_DrawCentred(0xf, 1, …);  FUN_004025d7(name, …);
///   DAT_0058fe9c = 0;
/// }
/// ```
///
/// and `Ui_DrawText`'s two branches give `0x10`/`0x1F` — `rgb(81,73,53)` over
/// `rgb(247,223,134)`, the parchment — and `0x3F`/`0x26` —
/// `rgb(0,0,0)` over `rgb(202,202,202)`, the grey. Both pairs are palette
/// indices out of the executable; neither was chosen to look right.
///
/// **The fixture used for each case.** `england-turn1.sav`, whose five owned
/// counties are 1, 4, 8, 11 and 13 with one realm each: county 8 is the
/// player's, county 1 belongs to realm 5, and county 2 belongs to nobody. There
/// is no fixture on this project in which one realm holds two counties, so the
/// owned case is the player's own county and nothing else.
#[test]
fn the_county_name_keeps_the_parchment_emboss_and_the_sovereign_lines_do_not() {
    let (mut game, assets) = world!();
    if assets.shell.body.is_none() {
        l2_testkit::skip!("no Fntl2_14.pl8, so nothing is embossed");
    }
    let parchment = l2_game::shell::font::SHADOW;
    let grey = l2_game::shell::font::SHADOW_GREY;
    assert_ne!(parchment, grey, "the two pairs are different, which is the whole point");

    // --- a county another lord holds: name in parchment, banner in grey.
    assert_eq!(game.kingdom.counties[1].owner, 5);
    let mut screen = CountyScreen::new(1, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 1);

    assert_eq!(
        emboss_at(&canvas, &assets, &name, STRIP_INK),
        Some(parchment),
        "the county's name is embossed in the parchment pair — the original's own bug"
    );
    let realm5 = l2_view::chrome::realm_pen(game.kingdom.realms[5].shield_index)
        .expect("realm 5 flies a shield");
    let banner = assets.shell.text(15, 0).to_string();
    let banner = if banner.is_empty() { "SOVEREIGN LAND".to_string() } else { banner };
    assert_eq!(
        emboss_at(&canvas, &assets, &banner, realm5),
        Some(grey),
        "the Sovereign land line is embossed in the grey pair"
    );

    // --- the player's own county: the same parchment emboss, on the plate it
    // was designed for, and no banner at all.
    let mut screen = CountyScreen::new(8, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 8);
    assert_eq!(
        emboss_at(&canvas, &assets, &name, STRIP_INK),
        Some(parchment),
        "and on the owned plate the same pair is correct"
    );
    assert!(
        find_body(&canvas, &assets, &banner, realm5).is_none(),
        "your own county carries no Sovereign land line"
    );
}

/// **Unclaimed land has no *Sovereign land of* line — a third case, not a
/// second.** **[V]**
///
/// `CountyStrip_Draw`'s else-arm draws the cloudy plate and the name for any
/// county that is not yours, and guards the three extra lines with
/// `owner != 0`. `L2.eng` group 15 holds exactly two strings, `"Sovereign
/// land"` and `"of"`, and the third line is a lord's name out of
/// Wording in the file holds a county nobody owns,
/// because the original never needs one.
///
/// Ours drew `SOVEREIGN LAND / OF / UNCLAIMED`, a sentence the original cannot
/// produce. The player reported it in one line: *"Unclaimed lands have no
/// 'sovereign land of'."*
#[test]
fn an_unclaimed_county_shows_its_name_and_nothing_else() {
    let (mut game, assets) = world!();
    let unclaimed = game
        .kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].owner == 0)
        .expect("England turn one has counties nobody holds");

    let mut screen = CountyScreen::new(unclaimed as u8, Panel::Tax);
    let canvas = draw(&mut screen, &mut game, &assets);
    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, unclaimed as u8);

    // The name is there, on the cloudy plate's lower line…
    assert_eq!(
        find_body(&canvas, &assets, &name, STRIP_INK).map(|p| p.1),
        Some(180),
        "an unclaimed county still gets its name at 0xB4"
    );
    // …and nothing under it. Every colour the banner could be drawn in is
    // checked, so this cannot pass by looking for the wrong one.
    let banner = assets.shell.text(15, 0).to_string();
    let banner = if banner.is_empty() { "SOVEREIGN LAND".to_string() } else { banner };
    for colour in assets.ink.realm.iter().copied().chain([STRIP_INK, assets.ink.text]) {
        assert!(
            find_body(&canvas, &assets, &banner, colour).is_none(),
            "an unclaimed county must not claim a sovereign, and one was drawn in {colour}"
        );
    }
    assert!(
        find_body(&canvas, &assets, "UNCLAIMED", assets.ink.text).is_none(),
        "and it must not invent a lord called UNCLAIMED"
    );
}

/// **The quirk switch turns the county name's emboss grey, and only that.**
///
/// Default off — the original's behaviour is what ships — and it lives on
/// [`l2_game::game::Quirks`], which is display state that never reaches the
/// simulation. See `docs/bugs.md` B64.
#[test]
fn the_grey_county_name_quirk_changes_the_emboss_and_nothing_else() {
    let (mut game, mut assets) = world!();
    if assets.shell.body.is_none() {
        l2_testkit::skip!("no Fntl2_14.pl8, so nothing is embossed");
    }
    assert!(!assets.quirks.grey_county_name, "the original's behaviour is the default");

    let mut screen = CountyScreen::new(1, Panel::Tax);
    let plain = draw(&mut screen, &mut game, &assets);
    assets.quirks.grey_county_name = true;
    let fixed = draw(&mut screen, &mut game, &assets);

    let name = county::county_name(&Ctx { game: &mut game, assets: &assets }, 1);
    assert_eq!(
        emboss_at(&plain, &assets, &name, STRIP_INK),
        Some(l2_game::shell::font::SHADOW),
        "off: the parchment pair"
    );
    assert_eq!(
        emboss_at(&fixed, &assets, &name, STRIP_INK),
        Some(l2_game::shell::font::SHADOW_GREY),
        "on: the grey pair the Sovereign lines already use"
    );

    // It moves the name's emboss and leaves everything else alone: the
    // difference is a few hundred pixels around one line, not a redrawn panel.
    let moved = plain.diff_count(&fixed);
    assert!(moved > 0, "the switch does something");
    assert!(moved < 4_000, "and only around the name: {moved} pixels");

    // And on the player's own county it changes nothing at all — the defect is
    // the parchment emboss over the *cloudy* plate, and the owned plate really
    // is parchment.
    let mut screen = CountyScreen::new(8, Panel::Tax);
    assets.quirks.grey_county_name = false;
    let plain = draw(&mut screen, &mut game, &assets);
    assets.quirks.grey_county_name = true;
    let fixed = draw(&mut screen, &mut game, &assets);
    assert_eq!(plain.diff_count(&fixed), 0, "your own county's name is right as it is");
}

// ===========================================================================
// The input arms of the right-hand column, the menu bar and the county panels
//
// `docs/arms.json`, groups `right-column`, `menu-bar`, `county-panels`,
// `village` and `management-screens`. Every test below names the arm it is
// about; the point of them is that an arm can only be shown to be live from the
// screen a player has on top; half of these arms are answered by a
// screen that is not the top one.
// ===========================================================================

/// **The whole right-hand column is live under an open county panel**, and each
/// of its five controls is checked separately because they are five separate
/// functions in `Screen_FrameInput`'s ladder.
///
/// `docs/arms.json` `0x0042FF10/inset-runs-the-sidebar-guards`. This is the arm
/// that was missing: `CountyScreen::handle` tested the four strip quadrants
/// itself and returned `Stay` for everything else in the column, so with the tax
/// panel open a player could not touch the minimap, the five sidebar buttons,
/// the farm/industry slider, the produce rows or End Turn.
#[test]
fn a_county_panel_leaves_the_whole_sidebar_live_underneath_it() {
    let (mut game, assets) = world!();
    game.select(8);
    assert!(game.is_players(8), "the fixture's county 8 is the local player's");

    // 1. A sidebar button - the court, which is ungated.
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    let court = map::SIDEBAR_BUTTONS[1].rect();
    send_stack(&mut m, &mut game, &assets, Event::Click { x: court.centre_x(), y: court.y + 4 });
    assert_eq!(m.top_id(), Some(ScreenId::Court), "sidebar button 2 opens the court");
    assert_eq!(m.depth(), 2, "and it replaced the panel rather than stacking on it");

    // 2. A minimap mode icon. The panel stays open; what changes is the map.
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    let mode = map::MINIMAP_MODE_BUTTONS[0];
    send_stack(&mut m, &mut game, &assets, Event::Click { x: mode.centre_x(), y: mode.y + 4 });
    assert_eq!(m.top_id(), Some(ScreenId::County(8, Panel::Tax)), "the panel is still open");

    // 3. The farm/industry split slider, which moves a real number.
    let mut m = over_the_map(ScreenId::County(8, Panel::Ration));
    game.kingdom.counties[8].industry_share = 40;
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 531, y: 270 });
    assert_eq!(
        game.kingdom.counties[8].industry_share, 0,
        "the press lands on the track's left edge, which is share 0"
    );

    // 4. A produce row, which opens the job popup for that row's labour slot.
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    let rows = county::farm_rows(&game.kingdom.counties[8]);
    assert!(!rows.is_empty(), "the fixture's county 8 farms something");
    let pitch = county::farm_pitch(rows.len());
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 500, y: 0x12E + pitch / 2 });
    assert_eq!(m.top_id(), Some(ScreenId::Job(8, rows[0])), "the first farm row's job popup");

    // 5. End Turn, which is record 5 of the same hotspot table and whose whole
    //    rectangle a BACK TO MAP button of ours used to sit on.
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 550, y: 470 });
    assert_ne!(
        m.top_id(),
        Some(ScreenId::County(8, Panel::Tax)),
        "End Turn is reachable through the panel"
    );
}

/// **The panel keeps its own two ways out**, so the pass above cannot be
/// "everything falls through", and a click on the panel itself is neither.
#[test]
fn a_county_panel_still_closes_on_its_corner_and_on_the_right_button() {
    let (mut game, assets) = world!();
    game.select(8);

    // **On the release.** `Ui_OkButtonClicked` (`0x0040E7E4`) opens
    // `if (g_mouseLeftReleased == 0) return 0;`, and this test used to drive a
    // press — which passed, because ours answered on the press too. It is the
    // shape a player reported from the other side: *"the game waited on
    // mouse-up."*
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    let ok = Panel::Tax.ok_button();
    let (okx, oky) = (ok.centre_x(), ok.y + 4);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: okx, y: oky });
    assert_eq!(
        m.top_id(),
        Some(ScreenId::County(8, Panel::Tax)),
        "the PRESS on the corner does nothing — the original tests the release",
    );
    send_stack(&mut m, &mut game, &assets, Event::Release { x: okx, y: oky });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "Ui_OkButtonClicked's 24 x 24 corner");

    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    send_stack(&mut m, &mut game, &assets, Event::RightClick { x: 300, y: 200 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "a right release anywhere");

    // The ablation: a click on the middle of the panel does nothing at all. Both
    // assertions above would still pass if `handle` closed on any click.
    let mut m = over_the_map(ScreenId::County(8, Panel::Tax));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 200, y: 200 });
    assert_eq!(
        m.top_id(),
        Some(ScreenId::County(8, Panel::Tax)),
        "a click on the panel is not an exit"
    );
}

/// **`Labour_SplitSliderDrag`'s three zones are half-open, and x = 594 steps
/// up.** The one column that used to fall on the track is the whole test.
#[test]
fn the_split_slider_steps_up_at_594_and_refuses_a_county_you_do_not_hold() {
    // Pure arithmetic, so this half needs no install.
    assert_eq!(map::split_from_click(593, 40), 100, "593 is still the track, and the track clamps at 100");
    assert_eq!(map::split_from_click(594, 40), 44, "594 is the first pixel of the up zone");
    assert_eq!(map::split_from_click(530, 40), 36, "530 is the last pixel of the down zone");
    assert_eq!(map::split_from_click(531, 40), 0, "531 is the first pixel of the track");
    assert_eq!(map::split_from_click(639, 100), 100, "clamped at 100");
    assert_eq!(map::split_from_click(0, 0), 0, "and at 0");

    let (mut game, assets) = world!();
    // The ownership gate, which `Labour_SplitSliderDrag` tests on its second
    // line and ours did not test at all.
    let other = (1..game.kingdom.counties.len() as u8)
        .find(|&id| game.kingdom.counties[id as usize].owner != game.player);
    let Some(other) = other else { return };
    game.select(other);
    let before = game.kingdom.counties[other as usize].industry_share;
    let mut m = Machine::new(ScreenId::Campaign);
    send_stack(&mut m, &mut game, &assets, Event::Click { x: 560, y: 270 });
    assert_eq!(
        game.kingdom.counties[other as usize].industry_share, before,
        "another lord's peasants do not move"
    );
}

