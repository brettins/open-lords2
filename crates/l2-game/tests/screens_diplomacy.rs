
#[macro_use]
mod common;

use common::*;

use l2_game::screen::Ctx;
use l2_game::screens::diplomacy::DiplomacyScreen;
use l2_game::shell::font;

/// **Every screen that names a lord reads the game's own two sources** —
/// `g_playerNames` first, `L2.eng` group 7 by the realm's **lord** behind it.
///
/// `docs/decisions.md` C189 put the court and the battle prompt on
/// `message::lord_name` and left three sites carrying a private copy that
/// stopped at the array and printed `REALM n`: `CountyStrip_Draw`'s third line,
/// `Diplo_DrawScreen`'s heading (`Ui_DrawText(&g_playerNames + g_diploTarget *
/// 0x2C, 0xD0, 0x3D, &g_fontHeading, 0x3F)`) and `Diplo_DrawLordCard`'s caption
/// (`… + realm * 0x2C, 0x20, slot * 100 + 0x83, &g_fontBody, 0x3F`). All five
/// are one draw in the original and are one function here.
#[test]
fn the_diplomacy_screen_names_a_lord_out_of_the_games_own_sources() {
    let (mut game, assets) = world!();
    if assets.shell.body.is_none() || assets.shell.heading.is_none() {
        l2_testkit::skip!("no Fntl2_14/22.pl8, so there is nothing to read a name off");
    }
    let mut screen = DiplomacyScreen::new();
    let (target, cards) = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        (screen.target(&ctx), DiplomacyScreen::cards(&ctx))
    };
    assert!(!cards.is_empty(), "the England fixture has rivals to draw cards for");
    let canvas = draw(&mut screen, &mut game, &assets);

    let name = l2_game::screens::message::lord_name(
        &Ctx { game: &mut game, assets: &assets },
        target,
    );
    assert!(!name.starts_with("REALM "), "group 7 names realm {target}'s lord, not {name:?}");
    assert_eq!(
        find_heading(&canvas, &assets, &name, font::TEXT),
        Some((0xD0, 0x3D)),
        "the heading is `Ui_DrawText(&g_playerNames + target * 0x2C, 0xD0, 0x3D, heading)`"
    );
    assert!(
        find_heading(&canvas, &assets, &format!("REALM {target}"), font::TEXT).is_none(),
        "and it is not a name of ours"
    );
    for (slot, realm) in cards.iter().enumerate() {
        let card = l2_game::screens::message::lord_name(
            &Ctx { game: &mut game, assets: &assets },
            *realm,
        );
        assert!(!card.starts_with("REALM "), "realm {realm}'s lord is named by group 7");
        assert_eq!(
            find_body(&canvas, &assets, &card, font::TEXT).map(|p| p.0),
            Some(0x20),
            "card {slot} names realm {realm} as {card:?} at x 0x20"
        );
    }
}
