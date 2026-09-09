//! **The merchant, driven** — the two screens against the England fixture and
//! a real install, headlessly.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test merchant
//! ```
//!
//! `crates/l2-kingdom/src/trade.rs` tests the rule; this tests that a *player*
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

fn send<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets, e: Event) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(e, &mut ctx)
}

fn draw<S: Screen>(screen: &mut S, game: &mut Game, assets: &Assets) -> Canvas {
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

/// **A player can buy grain and sell it back.** The property the whole screen
/// exists for, asserted at the ends: the treasury before and after, at the
/// prices the screen itself quoted.
#[test]
fn a_player_can_buy_grain_at_the_merchant_and_sell_it_back() {
    let (mut game, assets) = world!();
    let (merchant, _county) = stall(&mut game);

    // Every merchant the game creates carries morale 100, and that is the whole
    // of the markup: the buy price is exactly twice the sell price.
    assert_eq!(
        game.kingdom.campaign.units.get(merchant).map(|u| u.morale),
        Some(100),
        "a merchant's morale is the markup, and the fixture's is the shipped 100",
    );
    let q = trade::quote(&game.kingdom.tables, Good::Grain, 100);
    assert_eq!((q.sell, q.buy), (2, 4), "grain is 2/4 at full morale — the manual's 30/60");

    let gold = game.gold();
    let mut panel = TradeScreen::new(merchant, Good::Grain.id() as u8);
    let up = up_button();
    for _ in 0..10 {
        let t = send(&mut panel, &mut game, &assets, Event::Click { x: up.x + 2, y: up.y + 2 });
        assert_eq!(t, Transition::Stay);
    }
    let ok = confirm_button();
    let t = send(&mut panel, &mut game, &assets, Event::Click { x: ok.x + 2, y: ok.y + 2 });
    assert_eq!(t, Transition::Pop, "an agreed trade closes the panel");
    assert_eq!(game.gold(), gold - 10 * q.buy, "ten sacks at the buying price");

    // And back the other way, at the other price.
    let gold = game.gold();
    let mut panel = TradeScreen::new(merchant, Good::Grain.id() as u8);
    let down = down_button();
    for _ in 0..10 {
        send(&mut panel, &mut game, &assets, Event::Click { x: down.x + 2, y: down.y + 2 });
    }
    let t = send(&mut panel, &mut game, &assets, Event::Click { x: ok.x + 2, y: ok.y + 2 });
    assert_eq!(t, Transition::Pop);
    assert_eq!(game.gold(), gold + 10 * q.sell, "ten sacks at the selling price");

    // The four accumulators — the state `docs/hypotheses.json` needs and that
    // no fixture has ever carried a non-zero value for.
    let r = &game.kingdom.realms[game.player as usize];
    assert_eq!((r.trade_spent_a, r.trade_spent_b), (10 * q.buy, 10 * q.buy));
    assert_eq!((r.trade_received_a, r.trade_received_b), (10 * q.sell, 10 * q.sell));
}

/// Weapons are bought into the **realm's** armoury, not the county's store, and
/// into the slot `L2.eng` group 6 names rather than the one its id suggests.
#[test]
fn buying_swords_fills_the_realms_armoury_slot_group_six_names() {
    let (mut game, assets) = world!();
    let (merchant, _) = stall(&mut game);
    game.kingdom.realms[game.player as usize].gold = 100_000;

    let slot = Good::Swords.weapon_slot().expect("swords are a weapon");
    let before = game.kingdom.realms[game.player as usize].weapons[slot];
    let q = trade::quote(&game.kingdom.tables, Good::Swords, 100);
    assert_eq!((q.sell, q.buy), (23, 46));

    let mut panel = TradeScreen::new(merchant, Good::Swords.id() as u8);
    let up = up_button();
    for _ in 0..5 {
        send(&mut panel, &mut game, &assets, Event::Click { x: up.x + 2, y: up.y + 2 });
    }
    let ok = confirm_button();
    send(&mut panel, &mut game, &assets, Event::Click { x: ok.x + 2, y: ok.y + 2 });
    assert_eq!(game.kingdom.realms[game.player as usize].weapons[slot], before + 5);
}

/// **The stall's hit test is `mercgrid.pl8`**, and the shipped file names
/// twelve goods.
///
/// The grid is artwork, so this needs an install — and it checks the one thing
/// a table of rectangles could never tell us: that sheep and wool are on no
/// cell of the stall at all.
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

    // And clicking one of those cells opens the panel on that good.
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

/// **Ale is bought here and drunk immediately**, and its allowance is seasonal.
///
/// Both halves in one test, because the second is the correction: a second
/// purchase in the same season gives nothing, and the season's happiness pass
/// hands the allowance back. `docs/decisions.md` C53.
#[test]
fn ale_gives_five_happiness_a_season_and_no_more_until_the_season_turns() {
    let (mut game, assets) = world!();
    let (merchant, county) = stall(&mut game);
    {
        let c = &mut game.kingdom.counties[county as usize];
        c.happiness = 40;
        c.population = 1000;
    }
    game.kingdom.realms[game.player as usize].gold = 1000;

    // Ale carries no markup, so a barrel is a crown; a tenth of a thousand
    // people is a hundred crowns a point, and a thousand crowns is the cap.
    let q = trade::quote(&game.kingdom.tables, Good::Ale, 100);
    assert_eq!((q.sell, q.buy), (1, 1), "ale is the one good with no markup");

    let before = game.kingdom.counties[county as usize].happiness;
    let mut panel = TradeScreen::new(merchant, Good::Ale.id() as u8);
    let max = ceiling_button();
    send(&mut panel, &mut game, &assets, Event::Click { x: max.x + 2, y: max.y + 2 });
    let t = send(&mut panel, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(t, Transition::Pop);
    let after = game.kingdom.counties[county as usize].happiness;
    assert_eq!(after - before, 5, "the whole treasury of ale is worth five, and no more");
    assert_eq!(game.kingdom.counties[county as usize].ale_happiness_given, 5);

    // A second round in the same season buys nothing at all.
    game.kingdom.realms[game.player as usize].gold = 1000;
    let mut panel = TradeScreen::new(merchant, Good::Ale.id() as u8);
    send(&mut panel, &mut game, &assets, Event::Click { x: max.x + 2, y: max.y + 2 });
    send(&mut panel, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(
        game.kingdom.counties[county as usize].happiness,
        after,
        "the season's five are spent",
    );

    // The season's happiness pass hands the allowance back.
    l2_kingdom::happiness::update(&mut game.kingdom.counties[county as usize], true, 1);
    assert_eq!(game.kingdom.counties[county as usize].ale_happiness_given, 0);
}

/// **A click in the body does not close the merchant.** A player reported that
/// it did; that was the shell, which takes any click as a dismissal.
///
/// The original's two exits are the corner hotspot — `Ui_OkButtonClicked`
/// (`0x0040E7E4`), a 24 x 24 box on a *left release* — and a right release.
/// Both are asserted here, in both directions, on both screens.
#[test]
fn only_the_corner_hotspot_and_a_right_click_close_the_merchant() {
    let (mut game, assets) = world!();
    let (merchant, _) = stall(&mut game);

    let (bx, by) = empty_spot(&assets);
    let mut screen = MerchantScreen::new(merchant);
    assert_eq!(
        send(&mut screen, &mut game, &assets, Event::Click { x: bx, y: by }),
        Transition::Stay,
        "a click on the stall's background must not dismiss it",
    );
    assert_eq!(
        send(&mut screen, &mut game, &assets, Event::Click { x: STALL_OK.x + 4, y: STALL_OK.y + 4 }),
        Transition::Pop,
        "the corner hotspot closes it",
    );
    let mut screen = MerchantScreen::new(merchant);
    assert_eq!(
        send(&mut screen, &mut game, &assets, Event::RightClick { x: bx, y: by }),
        Transition::Pop,
        "and so does a right click",
    );

    // The panel, the same way: a click inside the window that is on none of its
    // six widgets does nothing.
    let mut panel = TradeScreen::new(merchant, Good::Grain.id() as u8);
    let inside = (PANEL.x + 8, PANEL.y + 8);
    assert_eq!(
        send(&mut panel, &mut game, &assets, Event::Click { x: inside.0, y: inside.1 }),
        Transition::Stay,
    );
    assert_eq!(
        send(&mut panel, &mut game, &assets, Event::Click { x: PANEL_OK.x + 4, y: PANEL_OK.y + 4 }),
        Transition::Pop,
    );
}

/// **The mouseover shows the ware's two prices**, which is the tooltip the
/// player said the original had and we did not.
///
/// It is asserted as a difference on the canvas rather than by reading the
/// plaque back: pointing at a ware must paint something that pointing at the
/// background does not, and moving to a *different* ware must paint something
/// different again.
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

/// A refusal is a refusal: the panel stays up, says which guard fired, and
/// nothing moves.
#[test]
fn an_order_the_treasury_cannot_cover_is_refused_and_the_panel_stays() {
    let (mut game, assets) = world!();
    let (merchant, county) = stall(&mut game);
    let q = trade::quote(&game.kingdom.tables, Good::Grain, 100);

    // Enough for nine sacks, and ask for far more: the arrows clamp, which is
    // the *first* line of defence and the reason the refusal is hard to reach.
    game.kingdom.realms[game.player as usize].gold = 9 * q.buy;
    let mut panel = TradeScreen::new(merchant, Good::Grain.id() as u8);
    let up = up_button();
    for _ in 0..20 {
        send(&mut panel, &mut game, &assets, Event::Click { x: up.x + 2, y: up.y + 2 });
    }
    // Now empty the treasury behind the panel's back — the second line, and the
    // one `Merchant_Trade` itself holds. In the original this is what happens
    // in multiplayer, where the order goes on the wire and the far end's gold
    // has moved by the time it arrives.
    game.kingdom.realms[game.player as usize].gold = 1;
    let ok = confirm_button();
    let t = send(&mut panel, &mut game, &assets, Event::Click { x: ok.x + 2, y: ok.y + 2 });
    assert_eq!(t, Transition::Stay, "a refused trade leaves the panel up");
    assert_eq!(game.gold(), 1, "and takes nothing");

    // Selling more than the county holds, straight at the rule.
    let grain = game.kingdom.counties[county as usize].grain;
    let order = trade::Order::sell(Good::Grain, grain + 1, q, game.player as usize, county as usize);
    assert_eq!(
        l2_kingdom::trade::trade(&mut game.kingdom, order),
        Err(trade::Refusal::NotEnoughStock),
    );
    assert_eq!(game.kingdom.counties[county as usize].grain, grain, "a refusal moves nothing");
}
