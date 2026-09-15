#![allow(unused_imports)]
use super::*;
use super::prompts_and_settling::*;
use super::battle_ai::*;
use super::tactical_controls::*;
use super::watched_battles::*;
use super::*;
use super::raising::*;
use super::marching::*;
use super::division::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::battlefield as bf;
use l2_game::screens::{armoury, army, battle, divide, info, map};
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

#[test]
fn a_loaded_game_offers_and_hires_the_band_its_save_has_standing_in_the_county() {
    use l2_game::screen::Screen;
    let save = l2_testkit::fixture!("siege-old_turn.sav");
    let mut g = l2_game::scenario::from_save(&save, l2_kingdom::tables::Tables::DEFAULT)
        .expect("the fixture loads");
    let a = Assets::placeholder();
    let county = 1u8;
    assert!(g.is_players(county), "the player holds county 1 in this save");
    assert_eq!(g.kingdom.counties[1].mercenary_offer, 2, "the file's +0x1AD: the Irish band");

    let mut screen = army::RaiseArmyScreen::new(county);
    {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        screen.update(&mut ctx);
        assert_eq!(screen.offer(&ctx), 2, "the raise-army screen offers it");
    }
    let yes = army::hire_yes(true);
    let (px, py) = (yes.x + yes.w / 2, yes.y + yes.h / 2);

    let press_the_tick = |g: &mut Game, screen: &mut army::RaiseArmyScreen| {
        let mut ctx = Ctx { game: g, assets: &a };
        screen.handle(Event::Click { x: px, y: py }, &mut ctx);
        for _ in 0..l2_game::press::DELAYED_FRAMES {
            screen.update(&mut ctx);
        }
    };
    assert!(g.gold() < 3_500, "the save's treasury cannot meet the Irish price");
    press_the_tick(&mut g, &mut screen);
    assert!(!g.levy.hire, "a band the treasury cannot meet cannot be ticked");

    let player = g.player as usize;
    g.kingdom.realms[player].gold = 10_000;
    press_the_tick(&mut g, &mut screen);
    assert!(g.levy.hire, "and once it can, the tick hires");

    let realm = g.kingdom.realms[player].clone();
    let basket = l2_kingdom::LevyBasket::seed(&realm, 50);
    let id = g.raise_army(county, &basket, 0, Some(2)).expect("the county raises an army with the band");
    let unit = g.kingdom.campaign.units.get(id).expect("the army");
    assert_eq!(
        unit.mercenaries,
        Some(l2_kingdom::Mercenaries { band: 2, troop: TroopType::Pikeman, men: 200 }),
        "the Irish band is on the army"
    );
    assert_eq!(g.kingdom.realms[player].gold, 10_000 - 3_500, "at the Irish price");
    let band = g.kingdom.campaign.mercenaries.get(2).expect("in play");
    assert_eq!(band.hired_by as usize, id, "the band records its hirer");
    assert_eq!(g.kingdom.counties[1].mercenary_offer, 0, "and the county's offer is gone");
}


#[test]
fn a_siege_laid_on_the_map_is_carried_to_its_assault_by_ending_the_turn() {
    let (mut g, a, mut m) = on_the_map();
    let (camp, keep) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    g.kingdom.counties[2].castle_type = 1; // a wooden palisade: no engines needed
    g.kingdom.counties[2].population = 10;

    let garrison = army_at(&mut g, 2, 2, 40, keep);
    g.kingdom.campaign.units.get_mut(garrison).unwrap().garrison_county = 2;
    g.kingdom.counties[2].garrison_unit = garrison;

    let besieger = army_at(&mut g, 1, 1, 800, camp);
    {
        let u = g.kingdom.campaign.units.get_mut(besieger).unwrap();
        u.besieging_county = 2;
    }
    g.kingdom.campaign.units.get_mut(garrison).unwrap().besieged_by = besieger as u8;

    press(&mut m, &mut g, &a, 'e');
    run_until(&mut m, &mut g, &a, "the siege prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });

    assert_eq!(
        m.top_id(),
        Some(ScreenId::BattlePrompt),
        "the assault should have raised the prompt",
    );
    assert!(
        g.kingdom.campaign.units.get(garrison).is_some()
            && g.kingdom.campaign.units.get(besieger).is_some(),
        "and nothing is resolved while the question is on the table",
    );

    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::DECLINE)));
    assert_eq!(m.top_id(), Some(ScreenId::BattleResult), "then the result screen");
    click(&mut m, &mut g, &a, on(battle::ok_rect()));
    for _ in 0..4 {
        tick(&mut m, &mut g, &a);
    }
    let letter = g.messages.open().copied();
    assert_eq!(m.top_id(), Some(ScreenId::Message), "the capture letter is up");
    assert!(
        letter.is_some_and(|r| r.category == l2_game::message::category::CAPTURE && r.county == 2),
        "a capture letter for county 2: {letter:?}",
    );
    send(&mut m, &mut g, &a, Event::RightClick { x: 320, y: 240 });
    for _ in 0..4 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the turn finished");

    assert!(
        g.kingdom.campaign.units.get(garrison).is_none()
            || g.kingdom.campaign.units.get(besieger).is_none(),
        "phase 2 launched the assault and one side of it is gone",
    );
    assert!(
        g.kingdom
            .campaign
            .units
            .get(besieger)
            .is_none_or(|u| u.besieging_county == 0),
        "and the siege link is broken either way",
    );
}


