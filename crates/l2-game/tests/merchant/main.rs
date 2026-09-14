//! **The merchant, driven** — the two screens against the England fixture and
//! a real install, headlessly.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test merchant
//! ```
//!
//! `crates/l2-kingdom/src/trade/mod.rs` tests the rule; this tests that a *player*
//! can reach it — the clicks land on the original's own widget rectangles, and
//! what changes is the county's store and the realm's treasury.


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

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there is no stall to trade at");
        };
        let platform = Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        (game, assets)
    }};
}

mod trade_actions;
pub use trade_actions::*;
mod merchant_ui;
pub use merchant_ui::*;

fn send<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets, e: Event) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(e, &mut ctx)
}

pub(crate) fn draw<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game, assets };
    screen.draw(&ctx, &mut canvas);
    canvas
}

/// A screen pixel that is on **no** ware and on no corner button — the "body"
/// a click must not dismiss.
fn empty_spot(assets: &Assets) -> (i32, i32) {
    (0..480)
        .step_by(8)
        .flat_map(|y| (0..640).step_by(8).map(move |x| (x, y)))
        .find(|&(x, y)| {
            assets.shell.merchant_grid(x, y).is_none()
                && !STALL_OK.contains(x, y)
                && !trade::ALL_GOODS.iter().any(|&g| plaque(g).contains(x, y))
        })
        .expect("the stall has some empty background")
}

/// The first merchant in the fixture, moved into a county the player owns —
/// which is the guard `Map_Click` applies (`docs/decisions.md` C50).
fn stall(game: &mut Game) -> (usize, u8) {
    let merchant = game
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
        .map(|(id, _)| id)
        .expect("the fixture ships six merchants");
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player owns counties");
    game.kingdom.campaign.units.get_mut(merchant).expect("the merchant").county = county;
    game.select(county);
    (merchant, county)
}

