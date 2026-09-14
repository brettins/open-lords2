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

/// **A loaded game offers the band its save has standing in the county, refuses
/// it at the original's price, and hires it once the treasury can pay.**
///
/// `siege-old_turn.sav` is the one save on this machine where a band stands in a
/// county the player holds: the Irish — two hundred pikemen at 3,500 crowns — in
/// county 1, whose realm holds 2,354. Until `g_mercBands` was imported, no
/// loaded game offered any band: the kingdom had none in play and county
///
/// at nothing on every save.
///
/// Everything this reads was written by the import — the offer, the band, its
/// price — except the treasury, which the test tops up to reach the hiring
/// branch after watching the refusal.
///
/// **Ablation, run:** delete `k.campaign.mercenaries = self.mercenaries.clone()`
/// from `Scenario::skeleton` and the hire attaches no band; delete
/// `c.mercenary_offer = *mercenary_offer;` from `Scenario::kingdom_with_tables`
/// and the county offers nothing.
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

    // 2,354 crowns against 3,500 is 69/3's branch, where the tick is no button.
    // **`RaiseArmy_HireToggle` is kind 5**, so every press below is followed by
    // the twenty frames its record waits before the handler runs.
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

// ---------------------------------------------------------------------------
// Turn phase 2
// ---------------------------------------------------------------------------

/// **A siege laid on the map is now carried by the turn.**
///
/// `engagement::run_siege_phase` has been turn phase 2 end to end since it was
/// written and **nothing outside its own tests called it**: `turn::settled`
/// answered the phase-2 wait `true` with the comment *"sieges are out of
/// scope"*,
/// assaulted. It is called from `begin_phase` now, and this is the assertion
/// that says so — a palisade needs no engines
/// ([`l2_kingdom::siege::ENGINES_REQUIRED_FROM_LEVEL`] is 3), so the assault
/// launches on the first phase 2 the turn reaches.
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
    // Phase 2 is a few ticks into the turn now that a turn takes frames, so the
// prompt arrives some frames after the keystroke.
    run_until(&mut m, &mut g, &a, "the siege prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });

    // **The turn stops and asks**: the besieger is the human's, so
    // `battle::settlement` says `Prompt` and phase 2 parks its assault on
    // screen `0x12`
    // before the campaign moves again.
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

    // Decline — the autocalc, which is what this test always ran.
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::DECLINE)));
    assert_eq!(m.top_id(), Some(ScreenId::BattleResult), "then the result screen");
    click(&mut m, &mut g, &a, on(battle::ok_rect()));
    // The map's own `update` picks the suspended turn back up the moment the
    // result screen is gone.
    for _ in 0..4 {
        tick(&mut m, &mut g, &a);
    }
    // **And the player is written to.** The beaten garrison gave up the
    // county, and `Battle_ReturnToCampaign`'s `County_ChangeOwner` posts the
    // capture letter, category `0x0D`, which is up over the map. The turn is
    // wound by the map's `update`, so it waits for the letter to be closed.
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

// ---------------------------------------------------------------------------
// "Will you take the field?"
// ---------------------------------------------------------------------------

