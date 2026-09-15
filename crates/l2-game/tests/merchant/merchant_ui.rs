#![allow(unused_imports)]
use super::*;
use super::trade_actions::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Screen, ScreenId, Transition};
use l2_game::screens::merchant::{
    ceiling_button, confirm_button, down_button, plaque, up_button, MerchantScreen, TradeScreen,
    PANEL, PANEL_OK, STALL_OK,
};
use l2_view::Canvas;
use l2_game::Game;
use l2_kingdom::tables::Tables;
use l2_kingdom::trade::{self, Good};
use l2_mods::Platform;

#[test]
fn the_stall_reads_mercgrid_and_it_names_twelve_goods() {
    let (mut game, assets) = world!();
    assert!(assets.shell.has_merchant_grid(), "mercgrid.pl8 is in the install");

    let mut seen: Vec<u8> = Vec::new();
    for y in (0..480).step_by(8) {
        for x in (0..640).step_by(8) {
            if let Some(g) = assets.shell.merchant_grid(x, y) {
                if !seen.contains(&g) {
                    seen.push(g);
                }
            }
        }
    }
    seen.sort_unstable();
    assert_eq!(
        seen,
        vec![1, 2, 4, 6, 7, 8, 9, 10, 11, 12, 13, 14],
        "sheep (3) and wool (5) are on no cell of the merchant's stall",
    );

    let (gx, gy) = (0..480)
        .step_by(8)
        .flat_map(|y| (0..640).step_by(8).map(move |x| (x, y)))
        .find(|&(x, y)| assets.shell.merchant_grid(x, y) == Some(1))
        .expect("grain is on the stall");
    let (merchant, _) = stall(&mut game);
    let mut screen = MerchantScreen::new(merchant);
    let t = send(&mut screen, &mut game, &assets, Event::Click { x: gx, y: gy });
    assert_eq!(t, Transition::Push(ScreenId::Trade(merchant, 1)), "grain opens the grain panel");
}

/// The original's two exits are the corner hotspot — `Ui_OkButtonClicked`
/// (`0x0040E7E4`), a 24 x 24 box on a *left release* — and a right release.
///
/// **And the corner is the RELEASE, which this test asserted in its own prose
/// and not in its code.** It sent an `Event::Click`, and so did both screens'
/// handlers; `Ui_OkButtonClicked`'s first statement is
/// `if (g_mouseLeftReleased == 0) return 0;`. `docs/arms.json`
/// `0x0042FF10/merchant-ok` and `0x0042FF10/trade-ok`.
///
/// **Ablation, run:** move either `Ok.contains` test back into the screen's
/// `Event::Click` arm and the matching press assertion below goes red.
#[test]
fn only_the_corner_hotspot_and_a_right_click_close_the_merchant() {
    let (mut game, assets) = world!();
    let (merchant, _) = stall(&mut game);

    let (bx, by) = empty_spot(&assets);
    let corner = (STALL_OK.x + 4, STALL_OK.y + 4);
    let mut screen = MerchantScreen::new(merchant);
    assert_eq!(
        send(&mut screen, &mut game, &assets, Event::Click { x: bx, y: by }),
        Transition::Stay,
        "a click on the stall's background must not dismiss it",
    );
    assert_eq!(
        send(&mut screen, &mut game, &assets, Event::Click { x: corner.0, y: corner.1 }),
        Transition::Stay,
        "and the PRESS on the corner does nothing: Ui_OkButtonClicked reads g_mouseLeftReleased",
    );
    assert_eq!(
        send(&mut screen, &mut game, &assets, Event::Release { x: bx, y: by }),
        Transition::Stay,
        "a release off the corner is not an exit either",
    );
    assert_eq!(
        send(&mut screen, &mut game, &assets, Event::Release { x: corner.0, y: corner.1 }),
        Transition::Pop,
        "the corner hotspot closes it, on the release",
    );
    let mut screen = MerchantScreen::new(merchant);
    assert_eq!(
        send(&mut screen, &mut game, &assets, Event::RightClick { x: bx, y: by }),
        Transition::Pop,
        "and so does a right click",
    );

    let mut panel = TradeScreen::new(merchant, Good::Grain.id() as u8);
    let inside = (PANEL.x + 8, PANEL.y + 8);
    let panel_corner = (PANEL_OK.x + 4, PANEL_OK.y + 4);
    assert_eq!(
        send(&mut panel, &mut game, &assets, Event::Click { x: inside.0, y: inside.1 }),
        Transition::Stay,
    );
    assert_eq!(
        send(&mut panel, &mut game, &assets, Event::Click { x: panel_corner.0, y: panel_corner.1 }),
        Transition::Stay,
        "the press on the panel's corner does nothing",
    );
    assert_eq!(
        send(&mut panel, &mut game, &assets, Event::Release { x: panel_corner.0, y: panel_corner.1 }),
        Transition::Pop,
    );
}

#[test]
fn hovering_a_ware_draws_its_price_plaque_and_the_background_draws_none() {
    let (mut game, assets) = world!();
    let (merchant, _) = stall(&mut game);
    let mut screen = MerchantScreen::new(merchant);

    let (bx, by) = empty_spot(&assets);
    send(&mut screen, &mut game, &assets, Event::Pointer { x: bx, y: by });
    let bare = draw(&mut screen, &mut game, &assets);

    let on_grain = (0..480)
        .step_by(8)
        .flat_map(|y| (0..640).step_by(8).map(move |x| (x, y)))
        .find(|&(x, y)| assets.shell.merchant_grid(x, y) == Some(1))
        .expect("grain is on the stall");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: on_grain.0, y: on_grain.1 });
    let grain = draw(&mut screen, &mut game, &assets);
    assert!(grain.diff_count(&bare) > 0, "hovering a ware drew nothing");

    let on_cattle = (0..480)
        .step_by(8)
        .flat_map(|y| (0..640).step_by(8).map(move |x| (x, y)))
        .find(|&(x, y)| assets.shell.merchant_grid(x, y) == Some(2))
        .expect("cattle are on the stall");
    send(&mut screen, &mut game, &assets, Event::Pointer { x: on_cattle.0, y: on_cattle.1 });
    let cattle = draw(&mut screen, &mut game, &assets);
    assert!(cattle.diff_count(&grain) > 0, "two different wares drew the same plaque");
}

