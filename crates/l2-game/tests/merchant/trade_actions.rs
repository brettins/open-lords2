#![allow(unused_imports)]
use super::*;
use super::merchant_ui::*;
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
fn a_player_can_buy_grain_at_the_merchant_and_sell_it_back() {
    let (mut game, assets) = world!();
    let (merchant, _county) = stall(&mut game);

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
    // **All six of `DAT_004DD838`'s records are `Widget_Test` kind 4**, so each
    // press is `Sound_RestartSlot(1)`. The panel answered raw clicks and played
    // nothing. Ablation, run: return 0 from `TradeScreen::take_clicks` and this
    // reads 0.
    assert_eq!(panel.take_clicks(), 11, "ten presses of the arrow and the thumb up, one click each");
    assert_eq!(panel.qty(), 10, "and the quantity the thumb agreed to was the ten presses");
    assert_eq!(game.gold(), gold - 10 * q.buy, "ten sacks at the buying price");

    let gold = game.gold();
    let mut panel = TradeScreen::new(merchant, Good::Grain.id() as u8);
    let down = down_button();
    for _ in 0..10 {
        send(&mut panel, &mut game, &assets, Event::Click { x: down.x + 2, y: down.y + 2 });
    }
    let t = send(&mut panel, &mut game, &assets, Event::Click { x: ok.x + 2, y: ok.y + 2 });
    assert_eq!(t, Transition::Pop);
    assert_eq!(game.gold(), gold + 10 * q.sell, "ten sacks at the selling price");

    let r = &game.kingdom.realms[game.player as usize];
    assert_eq!((r.trade_spent_a, r.trade_spent_b), (10 * q.buy, 10 * q.buy));
    assert_eq!((r.trade_received_a, r.trade_received_b), (10 * q.sell, 10 * q.sell));
}

/// Weapons are bought into the **realm's** armoury, not the county's store, and
/// into the slot `L2.eng` group 6 names.
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

    game.kingdom.realms[game.player as usize].gold = 1000;
    let mut panel = TradeScreen::new(merchant, Good::Ale.id() as u8);
    send(&mut panel, &mut game, &assets, Event::Click { x: max.x + 2, y: max.y + 2 });
    send(&mut panel, &mut game, &assets, Event::KeyDown(Key::Enter));
    assert_eq!(
        game.kingdom.counties[county as usize].happiness,
        after,
        "the season's five are spent",
    );

    l2_kingdom::happiness::update(&mut game.kingdom.counties[county as usize], true, 1);
    assert_eq!(game.kingdom.counties[county as usize].ale_happiness_given, 0);
}

#[test]
fn an_order_the_treasury_cannot_cover_is_refused_and_the_panel_stays() {
    let (mut game, assets) = world!();
    let (merchant, county) = stall(&mut game);
    let q = trade::quote(&game.kingdom.tables, Good::Grain, 100);

    game.kingdom.realms[game.player as usize].gold = 9 * q.buy;
    let mut panel = TradeScreen::new(merchant, Good::Grain.id() as u8);
    let up = up_button();
    for _ in 0..20 {
        send(&mut panel, &mut game, &assets, Event::Click { x: up.x + 2, y: up.y + 2 });
    }
    game.kingdom.realms[game.player as usize].gold = 1;
    let ok = confirm_button();
    let t = send(&mut panel, &mut game, &assets, Event::Click { x: ok.x + 2, y: ok.y + 2 });
    assert_eq!(t, Transition::Stay, "a refused trade leaves the panel up");
    assert_eq!(game.gold(), 1, "and takes nothing");

    let grain = game.kingdom.counties[county as usize].grain;
    let order = trade::Order::sell(Good::Grain, grain + 1, q, game.player as usize, county as usize);
    assert_eq!(
        l2_kingdom::trade::trade(&mut game.kingdom, order),
        Err(trade::Refusal::NotEnoughStock),
    );
    assert_eq!(game.kingdom.counties[county as usize].grain, grain, "a refusal moves nothing");
}

