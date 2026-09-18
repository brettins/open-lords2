#![allow(unused_imports)]
use super::*;
use super::selection_and_orders::*;
use super::siege_and_garrison::*;
use super::merging::*;
use super::rendering::*;
use super::*;
use super::battle_part::*;
use super::raising::*;
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
fn an_army_ordered_from_the_map_takes_an_undefended_county_when_the_turn_ends() {
    let (mut g, a, mut m) = on_the_map();
    let (here, town) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    assert_eq!(g.kingdom.campaign.map.county_at(here.0, here.1), 1);
    assert_eq!(g.kingdom.campaign.map.county_at(town.0, town.1), 2);
    g.kingdom.campaign.map.set_flags(town.0, town.1, flags::CASTLE);
    g.kingdom.counties[2].population = 10; // below the defence floor
    let id = army_at(&mut g, 1, 1, 600, here);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(town.0, town.1).unwrap());
    assert!(
        g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving),
        "the order was placed from the map",
    );
    let before = g.kingdom.turn_count;
    press(&mut m, &mut g, &a, 'e');
    let mut letters = Vec::new();
    for _ in 0..2_000 {
        if g.kingdom.turn_count > before {
            break;
        }
        if m.top_id() == Some(ScreenId::Message) {
            letters.extend(g.messages.open().copied());
            send(&mut m, &mut g, &a, Event::RightClick { x: 320, y: 240 });
        }
        tick(&mut m, &mut g, &a);
    }
    assert!(g.kingdom.turn_count > before, "the turn happened");
    for _ in 0..=l2_view::fade::PHASES {
        tick(&mut m, &mut g, &a);
    }
    assert!(
        letters.iter().any(|r| r.category == l2_game::message::category::CAPTURE && r.county == 2),
        "the player was told he took county 2: {letters:?}",
    );
    assert_eq!(
        g.kingdom.counties[2].owner, 1,
        "the county changed hands without the player leaving the map",
    );
    assert_eq!(g.owned_by(1), 2, "and the realm's holding grew");
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Campaign),
        "the map is still what is on screen: raise, march, take, all from here",
    );
}


/// The defect a player reported as *"can't seem to move my army"* had two
/// halves and this is the structural one. `Units_Tick` has one call site in the
/// original and it is the **frame loop**, not a phase (`docs/decisions.md`
/// C35), so it runs whenever the game is up — including all the time the human
/// is looking at the map. Ours only ran it from inside a turn, so an order was
/// accepted,
/// not move until the turn was ended. From the player's chair that is
/// indistinguishable from the order having been ignored.
#[test]
fn an_army_ordered_from_the_map_walks_while_the_player_watches() {
    let (mut g, a, mut m) = on_the_map();
    let (here, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);

    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving), "ordered");
    assert_eq!(g.kingdom.campaign.units.get(id).map(|u| u.tile()), Some(here));

    let before = g.kingdom.turn_count;
    for _ in 0..8 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(g.kingdom.turn_count, before, "no turn was ended");
    assert_eq!(
        g.kingdom.campaign.units.get(id).map(|u| u.tile()),
        Some(there),
        "the army walked to the tile it was sent to, without the turn being ended",
    );
}

#[test]
fn watching_the_map_does_not_give_an_army_extra_movement() {
    let (mut g, a, mut m) = on_the_map();
    let (here, _) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 300, here);
    let far = (here.0, here.1.saturating_sub(30));
    assert!(here.1 - far.1 > 20, "the destination is well out of one season's reach");
    g.order_unit_move(id, far).expect("a path across open ground");
    let allowance = g.kingdom.campaign.units.get(id).map_or(0, |u| u.move_allowance);

    for _ in 0..1_000 {
        tick(&mut m, &mut g, &a);
    }
    let u = g.kingdom.campaign.units.get(id).expect("still there");
    assert!(!u.moving, "the army ran out of moves and stopped");
    assert!(
        u.moves_used <= allowance,
        "a thousand frames spent {} of an allowance of {allowance}",
        u.moves_used,
    );
    assert_ne!(u.tile(), far, "and it did not arrive: the season's budget is the bound");
}

/// A player: *"the merchants move and then it fades out then in which hides the
/// season change visuals just abruptly changing"*, and *"the screen doesn't go
/// dark"* when it did not. `FUN_004B0CB4`'s two call sites are `Turn_Tick`'s
/// phase 7 with `rawFlag = 1` right after `Season_Advance`, and `FUN_0049A3E6`
/// with `0` after the seasonal art is reloaded — so the order is: units walk,
/// season advances, fade down, art swaps in the dark, fade up.
#[test]
fn the_end_of_a_turn_fades_the_screen_down_and_back_up() {
    use l2_game::screen::Screen;
    let (mut g, a) = world();
    let mut s = map::MapScreen::new();

    assert_eq!(s.fade(), None, "no fade before the turn");
    let mut ctx = Ctx { game: &mut g, assets: &a };
    s.handle(Event::KeyDown(Key::letter('e')), &mut ctx);
    assert_eq!(s.fade(), None, "and none while the phase machine is still running");

    let mut seen: Vec<u8> = Vec::new();
    let mut season_at = None;
    let before = g.kingdom.turn_count;
    for n in 0..2_000u32 {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        s.wind_turn(&mut ctx);
        s.update(&mut ctx);
        if season_at.is_none() && g.kingdom.turn_count > before {
            season_at = Some(n);
        }
        match s.fade() {
            Some(p) => seen.push(p),
            None if season_at.is_some() && !seen.is_empty() => break,
            None => {}
        }
    }

    assert!(season_at.is_some_and(|n| n > 4), "the season advanced, and took frames doing it");
    assert_eq!(seen.first(), Some(&0), "the fade starts at full brightness");
    assert_eq!(
        seen.len(),
        l2_view::fade::PHASES as usize,
        "one phase per tick, all the way down and back up",
    );
    assert!(seen.windows(2).all(|w| w[1] == w[0] + 1), "and strictly in order");
    assert_eq!(
        seen.iter().filter(|&&p| l2_view::fade::is_darkest(p)).count(),
        1,
        "it bottoms out exactly once",
    );
    assert_eq!(s.fade(), None, "and the light is fully back afterwards");
}

