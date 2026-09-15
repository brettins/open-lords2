#![allow(unused_imports)]
use super::*;
use super::garrison_and_mercenaries::*;
use super::zoom_and_battle::*;
use super::foraging_and_shoot::*;
use super::*;
use super::top_bar::*;
use super::panels::*;
use super::png_part::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Screen};
use l2_game::screens::map::MapScreen;
use l2_game::screens::setup::{SetupPage, SetupScreen};
use l2_game::shell::font::{self, Font, Style};
use l2_game::{scenario, Game};
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::Canvas;

/// **Every `&g_fontHeading` line of `UnitPanel_Draw` (`0x0041B19D`) is in the
/// heading face, where the call site puts it, inside the panel's own box — and
/// is not in the body face.**
///
/// ```c
/// Ui_DrawBox(8, R * 0x10 + 0x20, 0x1c, 0x1b - R);
/// /* transport */  Eng_DrawString(0x1f, 2, 0x18, R * 0x10 + 0x30, &g_fontHeading, 0x3f);
///                  Eng_DrawString(100, unit[+0x167] + scen * 0x14, g_penAdvance + 0x18, …);
/// /* merchant, peasants */ Eng_DrawString(0x1f, local_20, 0x28, R * 0x10 + 0x40, …);
/// /* army */       Eng_DrawString(owner + 0x5d, unit.nameIndex, 0x28, R * 0x10 + 0x30, …);
/// /* own army */   Eng_DrawString(0x10, 0, 0x38, R * 0x10 + 0x130, …);         /* no band */
///                  Ui_DrawNumber(mercMen, '@', &DAT_004d422c, 0x38, R * 0x10 + 0x130, …);
///                  Eng_DrawString(0x10, mercBand, g_penAdvance + 0x38, …);
///                  Ui_DrawUnitNoun(mercMen, mercTroop * 2 + 0x34, g_penAdvance + 0x38, …);
/// ```
///
/// Every `R` and every position below is a literal out of those lines and
/// `FUN_0041BEFE`'s ladder; the only computed parts are the widths of strings
/// measured in the face the call names.
#[test]
fn every_unit_panel_heading_is_in_the_heading_face_inside_its_box() {
    use l2_game::screens::info::{InfoScreen, Target};
    use l2_kingdom::unit::{Mercenaries, TroopType, Unit, UnitKind};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let heading = assets.shell.heading.as_ref().expect("Fntl2_22.pl8");
    let county = own_county(&game);
    let player = game.player;
    let enemy = if player == 1 { 2 } else { 1 };
    let slot = game.map_slot;
    let text = |g: usize, i: usize| assets.shell.text(g, i).to_string();

    let unit = |kind: UnitKind, owner: u8| {
        let mut u = Unit::new(kind, owner, 10, 10);
        u.men = 100;
        u.county = county;
        u.home_county = county;
        u.owner_is_human = owner == player;
        u
    };
    let mut transport = unit(UnitKind::Transport, player);
    transport.cargo_county = county;
    let mut named = unit(UnitKind::Army, player);
    named.name_index = 3;
    let mut hired = unit(UnitKind::Army, player);
    hired.name_index = 4;
    hired.mercenaries = Some(Mercenaries { band: 3, troop: TroopType::Archer, men: 40 });
    let mut foe = unit(UnitKind::Army, enemy);
    foe.name_index = 7;

    let cases: Vec<(&str, Unit, i32)> = vec![
        ("merchant", unit(UnitKind::Merchant, 0), 0x0F),
        ("peasants", unit(UnitKind::PeasantMob, 0), 0x0F),
        ("transport", transport, 0x0F),
        ("own army, no band", named, 2),
        ("own army, a band", hired, 2),
        ("enemy army", foe, 0x12),
    ];
    for (label, u, row) in cases {
        let kind = u.kind;
        let id = game.kingdom.campaign.units.spawn(u).expect("a free slot");
        let mut panel = InfoScreen::new(Target::Unit(id));
        let canvas = draw(&mut panel, &mut game, &assets);

        let lines: Vec<(String, i32, i32)> = match (label, kind) {
            (_, UnitKind::Merchant) => vec![(text(0x1F, 0), 0x28, row * 16 + 0x40)],
            (_, UnitKind::PeasantMob) => vec![(text(0x1F, 5), 0x28, row * 16 + 0x40)],
            (_, UnitKind::Transport) => {
                let y = row * 16 + 0x30;
                let t = text(0x1F, 2);
                let name_x = 0x18 + heading.width(&t) + TRAILER;
                vec![(t, 0x18, y), (text(100, slot * 20 + county as usize), name_x, y)]
            }
            ("own army, no band", _) => vec![
                (text(0x5D + player as usize, 3), 0x28, row * 16 + 0x30),
                (text(0x10, 0), 0x38, row * 16 + 0x130),
            ],
            ("own army, a band", _) => {
                let y = row * 16 + 0x130;
                let band = text(0x10, 3);
                let band_x = 0x38 + LEAD + heading.width("40") + TRAILER;
                let noun_x = band_x + heading.width(&band) + TRAILER;
                vec![
                    (text(0x5D + player as usize, 4), 0x28, row * 16 + 0x30),
                    ("40".to_string(), 0x38 + LEAD, y),
                    (band, band_x, y),
                    (text(8, 0x34 + 5 * 2 + 1), noun_x, y),
                ]
            }
            _ => vec![(text(0x5D + enemy as usize, 7), 0x28, row * 16 + 0x30)],
        };

        let (left, top) = (8, row * 16 + 0x20);
        let (right, bottom) = (left + 0x1C * 16, top + (0x1B - row) * 16);
        for (s, x, y) in &lines {
            assert!(!s.is_empty(), "{label}: an L2.eng line the painter draws is empty");
            assert_eq!(
                find_on_row(&canvas, heading, s, font::TEXT, *y),
                Some(*x),
                "{label}: {s:?} is not in Fntl2_22.pl8 at ({x}, {y})",
            );
            assert!(
                *x >= left && x + heading.width(s) <= right && *y >= top && y + heading.height(s) <= bottom,
                "{label}: {s:?} at ({x}, {y}) is outside the panel's box ({left}, {top})-({right}, {bottom})",
            );
            // On the call site's own row: the troop grid draws the same group 8
            // nouns in body further up, which is right and is not this line.
            if !s.chars().all(|c| c.is_ascii_digit()) {
                assert_eq!(
                    find_on_row(&canvas, body, s, font::TEXT, *y),
                    None,
                    "{label}: {s:?} is on row {y} in the body face",
                );
            }
        }
        if label == "enemy army" {
            let none = text(0x10, 0);
            assert_eq!(find_in(&canvas, heading, &none, font::TEXT), None, "{label}: {none:?}");
        }
    }
}

/// **The unit panel's *"Formed"* line is `Ui_DrawYear(…, style 0)`, which ends
/// in `L2.eng` 26/1 *"AD"* — and we drew the bare number.**
///
/// ```c
/// /* UnitPanel_Draw */
/// g_penAdvance = 0;
/// Eng_DrawString(0x1F, 0x14, 0x28, row * 0x10 + 0xA0, &g_fontBody, 0x3F);
/// Ui_DrawYear(unit.yearFormed, g_penAdvance + 0x28, row * 0x10 + 0xA0, 0);
/// /* Ui_DrawYear, 0x0041A900, style 0, year >= 0: */
/// Ui_DrawNumber(year, ' ', &DAT_004D41D8, x, y, &g_fontBody, 0x3F);
/// Eng_DrawString(0x1A, 1, x + g_penAdvance, y, &g_fontBody, 0x3F);
/// ```
///
/// `DAT_004D41D8` is one space, and the row is 2 for an army of the local
/// player's (`FUN_0041BEFE`). `CLAUDE.md` rule 6.
#[test]
fn the_unit_panel_says_the_year_an_army_was_formed_in_ad() {
    use l2_game::screens::info::{InfoScreen, Target};
    use l2_kingdom::unit::{Unit, UnitKind};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let county = own_county(&game);
    const FORMED: i32 = 1271;
    let mut u = Unit::new(UnitKind::Army, game.player, 10, 10);
    u.men = 100;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = true;
    u.year_formed = FORMED as _;
    let id = game.kingdom.campaign.units.spawn(u).expect("a free slot");
    let mut panel = InfoScreen::new(Target::Unit(id));
    let canvas = draw(&mut panel, &mut game, &assets);

    const ROW: i32 = 2;
    let y = ROW * 0x10 + 0xA0;
    let formed = assets.shell.text(0x1F, 0x14).to_string();
    let ad = assets.shell.text(0x1A, 1).to_string();
    assert_eq!(ad, "AD", "L2.eng 26/1");

    let year_x = 0x28 + body.width(&formed) + TRAILER;
    let digits = FORMED.to_string();
    assert_eq!(
        find_on_row(&canvas, body, &digits, font::TEXT, y),
        Some(year_x + LEAD),
        "the year is not one space right of g_penAdvance + 0x28"
    );
    assert_eq!(
        find_on_row_from(&canvas, body, &ad, font::TEXT, y, year_x),
        Some(year_x + LEAD + body.width(&digits) + SPACE + TRAILER),
        "{ad:?} is not after \" {digits} \" on the Formed line"
    );
}


