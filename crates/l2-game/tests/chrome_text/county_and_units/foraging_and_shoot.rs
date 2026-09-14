#![allow(unused_imports)]
use super::*;
use super::garrison_and_mercenaries::*;
use super::unit_panels::*;
use super::zoom_and_battle::*;
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

/// **The fourth thing *Army foraging* gates is the unit panel's own lines**, and
/// ours drew none of them — not even the army's body line.
///
/// `UnitPanel_Draw` (`0x0041B19D`), the `kind == 1` arm: with `g_optArmiesEat`
/// off the body is one line at `row * 0x10 + 0x70`; with it on the body moves to
/// `+0x5E` and the supply state (31/23…26) and the starvation band
/// (31/27 + `+0x155`, red once the counter leaves zero) follow at `+0x72` and
/// `+0x86`. `docs/armies.md` §3.4 tabulates the supply strings.
///
/// Ablated: dropping the `armies_eat` branch leaves the body at `+0x70` with the
/// option on — the supply line is `None` at `+0x72`.
#[test]
fn army_foraging_moves_the_unit_panels_body_line_and_adds_two() {
    use l2_game::screens::info::{InfoScreen, Target};
    use l2_kingdom::unit::{Unit, UnitKind};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let county = own_county(&game);
    let mut u = Unit::new(UnitKind::Army, game.player, 10, 10);
    u.men = 100;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = true;
    u.starvation = 2;
    let id = game.kingdom.campaign.units.spawn(u).expect("a free slot");

    // `FUN_0041BEFE` puts a local player's army on row 2.
    const ROW: i32 = 2;
    let line = |i: usize| assets.shell.text(0x1F, i).to_string();
    // 31/16 is an own army's body, 31/23 "Fed in your county.", 31/29 the
    // third starvation band.
    let (text, fed, starving) = (line(0x10), line(0x17), line(0x1B + 2));
    assert!(!fed.is_empty() && !starving.is_empty(), "L2.eng 31/23 and 31/29");

    game.kingdom.options.armies_eat = false;
    let canvas = draw(&mut InfoScreen::new(Target::Unit(id)), &mut game, &assets);
    assert_eq!(
        find_on_row(&canvas, body, &text, font::TEXT, ROW * 0x10 + 0x70),
        Some(0x68),
        "with foraging off the army's body line is not at row * 0x10 + 0x70"
    );
    assert_eq!(
        find_on_row(&canvas, body, &fed, font::TEXT, ROW * 0x10 + 0x72),
        None,
        "the supply line is drawn with foraging off"
    );

    game.kingdom.options.armies_eat = true;
    let canvas = draw(&mut InfoScreen::new(Target::Unit(id)), &mut game, &assets);
    assert_eq!(
        find_on_row(&canvas, body, &text, font::TEXT, ROW * 0x10 + 0x5E),
        Some(0x68),
        "with foraging on the body line has not moved up to row * 0x10 + 0x5E"
    );
    assert_eq!(
        find_on_row(&canvas, body, &fed, font::TEXT, ROW * 0x10 + 0x72),
        Some(0x68),
        "{fed:?} is not the supply line at row * 0x10 + 0x72"
    );
    assert_eq!(
        find_on_row(&canvas, body, &starving, font::TEXT, ROW * 0x10 + 0x86),
        None,
        "the starvation line is drawn at 0x3F with the counter at 2"
    );
    assert_eq!(
        find_on_row(&canvas, body, &starving, 0xF9, ROW * 0x10 + 0x86),
        Some(0x68),
        "{starving:?} is not the starvation line in 0xF9 at row * 0x10 + 0x86"
    );
}

#[test]
#[ignore]
fn shoot() {
    let (mut game, assets) = world!();

    let mut screen = MapScreen::new();
    let canvas = draw(&mut screen, &mut game, &assets);
    save(&canvas, &assets.palette, "menubar");

    // The three drop-downs, over the map, for the row-pitch measurement.
    for menu in 0..3usize {
        let mut m = l2_game::screen::Machine::new(l2_game::screen::ScreenId::Campaign);
        m.push(l2_game::screen::ScreenId::MenuBar(menu));
        let mut canvas = Canvas::screen();
        let ctx = Ctx {
            game: &mut game,
            assets: &assets,
        };
        m.draw(&ctx, &mut canvas);
        save(&canvas, &assets.palette, &format!("dropdown_{menu}"));
    }

    for page in [SetupPage::Title, SetupPage::Options] {
        let mut screen = SetupScreen::new(page);
        let canvas = draw(&mut screen, &mut game, &assets);
        let p = assets
            .shell
            .palette("Gateway.256")
            .unwrap_or(&assets.palette);
        save(&canvas, p, &format!("setup_{}", page.number()));
    }
}

fn save(canvas: &Canvas, palette: &l2_formats::Palette, name: &str) {
    let mut rgba = vec![0u8; 640 * 480 * 4];
    canvas.to_rgba(palette, &mut rgba);
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let dir = std::env::var("L2_SHOT_DIR").unwrap_or_else(|_| "out".to_string());
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(format!("{dir}/{name}.png"), png::encode(640, 480, &rgb)).unwrap();
}


