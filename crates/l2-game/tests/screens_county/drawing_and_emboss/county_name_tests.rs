#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::panel_tests::*;
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

