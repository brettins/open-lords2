#![allow(unused_imports)]
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

/// Put the player's army next to an enemy's and march it in, so the turn's unit
/// sweep raises a battle.
fn a_battle_is_about_to_happen() -> (Game, Assets, Machine, usize, usize) {
    let (mut g, a, m) = on_the_map();
    let (mine, theirs) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    let attacker = army_at(&mut g, 1, 1, 400, mine);
    let defender = army_at(&mut g, 2, 2, 200, theirs);
// The order is given through `l2-kingdom`:
    // the map screen's second click on an *enemy* army is a selection, not an
// attack order, and what is under test here is the prompt
    // route into it.
    let map = g.kingdom.campaign.map.clone();
    l2_kingdom::movement::order_move(
        &map,
        &mut g.kingdom.campaign.units,
        attacker,
        theirs,
        l2_kingdom::movement::Routing::Direct,
    )
    .expect("a path one tile long");
    (g, a, m, attacker, defender)
}

/// **The player is asked,
///
/// This is the gap the whole prompt closes: `end_turn` used to answer
/// `Answer::Decline` for him, because there was no screen to ask on. Ending the
/// turn now stops on screen `0x12` with both armies still standing.
#[test]
fn ending_a_turn_into_a_battle_asks_the_player_before_anything_is_decided() {
    let (mut g, a, mut m, attacker, defender) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');

    assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt), "the prompt should be up");
    assert!(
        g.kingdom.campaign.units.get(attacker).is_some()
            && g.kingdom.campaign.units.get(defender).is_some(),
        "neither army may be touched while the question is on the table",
    );
    // And the question knows who is asking and what each side is bringing.
    let q = l2_game::turn::pending_question(&g).expect("a question");
    assert_eq!(q.choice_owner, 1, "the local player holds the choice");
    assert_eq!(q.attacker_men, 400);
    assert_eq!(q.defender_men, 200);
    assert!(!q.is_siege);
}

/// **Both answers reach the simulation, and they are different battles.**
///
/// Taking the field raises the **battlefield** — `FUN_0043B593` with hotspot id
/// 1 calls `Battle_Start`, not the autocalc — and declining runs the autocalc.
/// The test that they are two paths is that the report says so.
#[test]
fn the_two_thumbs_reach_the_two_ways_a_battle_can_be_settled() {
    use l2_game::engagement::Resolution;

    for (thumb, want_fought) in [(battle::TAKE_THE_FIELD, true), (battle::DECLINE, false)] {
        let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
        press(&mut m, &mut g, &a, 'e');
        assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt));
        click(&mut m, &mut g, &a, on(battle::widget_rect(thumb)));

        if want_fought {
            // The battlefield, and it is **paused** — `Battle_Start` writes
            // `DAT_0053F238 = 0xFFFFFFFF` before it raises the screen.
            assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "the field, not the result");
            assert!(g.battle.as_ref().expect("a live battle").paused, "a battle starts paused");
            // Nothing happens while it is paused, however many frames pass.
            for _ in 0..200 {
                tick(&mut m, &mut g, &a);
            }
            assert_eq!(g.battle.as_ref().expect("still there").runner.tick, 0);
            // Unpause and let it run. A battle is thousands of ticks long, so
            // this is a longer loop than `run_until`'s — it is the same thing.
            click(&mut m, &mut g, &a, on(bf::Button::Pause.rect()));
            assert!(!g.battle.as_ref().expect("a live battle").paused);
            // **And the player attacks.** He has to: `Battle_Start` orders
            // nobody and `Battle_UpdateAllUnits` runs no handler for his units,
            // while the AI here is outnumbered two to one and holds its ground.
            // Two armies standing still is what the original does with a player
            // who never clicks,
            {
                let live = g.battle.as_mut().expect("a live battle");
                let enemy = live.runner.home(l2_sim::runner::other_side(l2_sim::SIDE_B));
                live.runner.order_side(l2_sim::SIDE_B, enemy.0, enemy.1);
            }
            for _ in 0..40_000 {
                if g.battle.as_ref().is_some_and(|b| b.conclusion.is_some()) {
                    break;
                }
                tick(&mut m, &mut g, &a);
            }
            assert!(
                g.battle.as_ref().is_some_and(|b| b.conclusion.is_some()),
                "the battle did not reach a conclusion",
            );
            // The banner is up, and it holds for five thousand frames unless a
            // right release skips it. Skip it, which is `0x2B`'s only arm.
            assert_eq!(g.battle.as_ref().map(|b| b.screen_id()), Some(0x2B));
            send(&mut m, &mut g, &a, Event::RightClick { x: 100, y: 300 });
            tick(&mut m, &mut g, &a);
        }

        assert_eq!(m.top_id(), Some(ScreenId::BattleResult), "the result screen follows");
        let r = l2_game::turn::pending_report(&g).expect("a settled battle");
        match (r.resolution, want_fought) {
            (Resolution::Fought { ticks, .. }, true) => assert!(ticks > 0),
            (Resolution::Autocalc, false) => {}
            (other, _) => panic!("{other:?} for thumb at {:?}", thumb.0),
        }
        // Whichever way it went, one army is gone and one is not.
        assert_ne!(
            r.attacker_men.1 == 0,
            r.defender_men.1 == 0,
            "exactly one side should have been destroyed: {:?} {:?}",
            r.attacker_men,
            r.defender_men
        );

        // The corner dismisses it and the turn carries on to its end — over as
        // many frames as the rest of the turn takes, which is what a turn
        // having a duration means.
        click(&mut m, &mut g, &a, on(battle::ok_rect()));
        run_until(&mut m, &mut g, &a, "the rest of the turn", |_, g| {
            !l2_game::turn::turn_in_flight(g)
        });
        assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the turn finished");
        assert!(!l2_game::turn::turn_in_flight(&g), "nothing left suspended");
    }
}

/// **A double click on a thumb answers the prompt.** `DAT_004DDBB0`'s two
/// records are kind 4, whose guard is `g_mouseLeftPressed ||
/// g_mouseLeftDoubleClick`, and Windows sends the double click
/// press —
///
/// `docs/input.md` counted this screen among six that dropped a double click.
/// It did not: `BattlePromptScreen::handle` hands every event to
/// `Press::event`. This is the test that says so.
///
/// **Ablation, run:** make `Press::event`'s `DoubleClick` arm answer
/// `Kind::Repeat` with `None` and this goes red.
#[test]
fn a_double_click_on_a_thumb_answers_the_prompt() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');
    assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt));
    let (x, y) = on(battle::widget_rect(battle::DECLINE));
    send(&mut m, &mut g, &a, Event::DoubleClick { x, y });
    let r = l2_game::turn::pending_report(&g).expect("the double click declined");
    assert_eq!(r.resolution, l2_game::engagement::Resolution::Autocalc);
    assert_eq!(m.clicks(), 1, "and it is Widget_Test's press, so it clicks");
}

/// **Screen `0x12` has exactly two exits and neither of them is a button on the
/// mouse or a key on the keyboard.**
///
/// `Screen_FrameInput`'s `0x12` arm is two `if`s, both multiplayer: the sync
/// latch, and `FUN_004BBEA7`, which returns 0 outright when `g_multiplayer` is
/// clear.
/// prompt waits for ever, which `docs/symbols.json` records of `Battle_Decline`
/// and which is correct.
///
/// We had right-click-to-Decline, Escape-to-Decline and Enter-to-fight here.
/// All three were ours; `docs/decisions.md` C67 counts them, and `docs/arms.json` is where they are
/// counted.
///
/// **Ablated**: putting any one of the three back turns this red.
#[test]
fn the_prompt_has_no_way_out_but_its_two_widgets() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');
    assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt));

    // A hundred ticks: no timeout.
    for _ in 0..100 {
        tick(&mut m, &mut g, &a);
    }
    // Every gesture the original does not have, inside the window and outside
    // it, and none of them may settle anything.
    let window = battle::window();
    for e in [
        Event::RightClick { x: 0, y: 0 },
        Event::RightClick { x: window.centre_x(), y: window.y + 8 },
        Event::KeyDown(Key::Escape),
        Event::KeyDown(Key::Enter),
        Event::KeyDown(Key::letter('Y')),
        // A left click on the window that is on neither widget.
        Event::Click { x: window.x + 8, y: window.y + 8 },
        Event::DoubleClick { x: window.centre_x(), y: window.y + 8 },
    ] {
        send(&mut m, &mut g, &a, e);
        assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt), "{e:?} left the prompt");
        assert!(l2_game::turn::pending_report(&g).is_none(), "{e:?} settled the battle");
        assert!(g.battle.is_none(), "{e:?} raised a battlefield");
    }
    // And the two that do work still work.
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::DECLINE)));
    let r = l2_game::turn::pending_report(&g).expect("the thumb down declines");
    assert_eq!(r.resolution, l2_game::engagement::Resolution::Autocalc);
}

/// **A battle a player watches and gives the headless path's own order in
/// produces exactly the battle nobody watches.**
///
/// This is the assertion that lets orders be added safely: a player's orders
/// change what a player *can do*, and must not change what the same inputs
/// produce. The two paths share [`l2_game::engagement::begin_fight`] and
/// [`l2_game::engagement::conclude_fight`]; what differs is who supplies the
/// ticks,
/// battle is over every hundredth tick and the played one every tick.
///
/// **The order is now the player's and is issued here.** It used to be inside
/// `begin_fight`, so "giving no orders" and "giving the headless order" were the
/// same thing and the test could not tell them apart. `Battle_Start` orders
/// nobody,
/// headless path uses for an absent player has to be issued explicitly on this
/// side for the two to be comparable at all.
#[test]
fn watching_a_battle_and_giving_the_same_order_reproduces_the_headless_verdict() {
    // The headless path: `end_turn`, whose policy answers Decline — so force
    // the fought path by resolving the same pair directly.
    let (mut g, _a, _m, attacker, defender) = a_battle_is_about_to_happen();
    let county = g.kingdom.campaign.units.get(defender).map_or(0, |u| u.county);
    let seed = 0x5EED_BEEF;
    let mut headless = g.kingdom.clone();
    let mut runner = l2_game::engagement::begin_fight(&mut headless, attacker, defender, None, seed)
        .expect("two armies");
    // `engagement::fight` supplies this for an absent player; this loop is
    // `fight`'s body written out, so it has to supply it too.
    {
        let enemy = runner.home(l2_sim::runner::other_side(l2_sim::SIDE_B));
        runner.order_side(l2_sim::SIDE_B, enemy.0, enemy.1);
    }
    while runner.tick < 12_000 && runner.conclusion().is_none() {
        runner.run(100);
    }
    let (want_verdict, _) =
        l2_game::engagement::conclude_fight(&mut headless, attacker, defender, runner);

    // The watched path: the same two armies, the same seed, one tick at a time.
    let mut watched = g.kingdom.clone();
    let mut runner =
        l2_game::engagement::begin_fight(&mut watched, attacker, defender, None, seed)
            .expect("two armies");
    // The click the headless path makes on the player's behalf, made here by
    // hand: every unit of the human side at the enemy's marker.
    {
        let enemy = runner.home(l2_sim::runner::other_side(l2_sim::SIDE_B));
        runner.order_side(l2_sim::SIDE_B, enemy.0, enemy.1);
    }
    let mut live = l2_game::battlefield::LiveBattle::new(runner, attacker, defender, county, None, 1, 1);
    live.paused = false;
    for _ in 0..12_000 {
        live.tick();
        if live.conclusion.is_some() {
            break;
        }
    }
    runner = live.runner;
    let (got_verdict, _) =
        l2_game::engagement::conclude_fight(&mut watched, attacker, defender, runner);

    assert_eq!(got_verdict, want_verdict, "a watched battle changed its own outcome");
    for id in [attacker, defender] {
        assert_eq!(
            headless.campaign.units.get(id).map(|u| u.troops),
            watched.campaign.units.get(id).map(|u| u.troops),
            "unit {id} came off the field with different men",
        );
    }
    let _ = &mut g;
}

/// **A raised battle orders nobody**, because `Battle_Start` (`0x004778A0`)
/// orders nobody.
///
/// The player's report: *"my men in battle started moving before I clicked"*,
/// and *"units don't advance on their own"*. Both are right,
/// the original's own arithmetic:
///
/// * `Battle_Start` runs `Battlefield_Build*`, `Battle_InitArmies`,
///   `FUN_00480F8B`, `Battle_UpdateAllMen` and
/// `Battle_UpdateStrengthAdvantage`.
///   `Battle_RaiseSide` (`0x0047FEA7`) → `BattleUnit_Create` (`0x00480662`)
///   gives each figure `tg x`/`tg y` equal to the cell it stands on.
/// * `Battle_UpdateAllUnits` (`0x00489401`) runs no order handler for a unit
///   whose `humanControlled` byte is set, so nothing writes a destination for
///   the player's units either.
/// * It is not that an unordered unit is inert: `BattleMan_FireMissile`
///   (`0x004804xx`, the state-5 arm calling `Missile_FindTarget`) carries **no**
///   human guard, so an unordered archer stands where it is and shoots whatever
///   comes inside its range. Standing is the behaviour; helplessness is not.
///
/// `Battle_Start` also seeds `DAT_0053F238 = 0xFFFFFFFF`,
/// paused —
/// men moved without him.
#[test]
fn a_raised_battle_gives_the_players_own_units_no_orders() {
    let (mut g, _a, _m, attacker, defender) = a_battle_is_about_to_happen();
    let mut runner =
        l2_game::engagement::begin_fight(&mut g.kingdom, attacker, defender, None, 0x5EED_BEEF)
            .expect("two armies");

    // The attacker is the human's and `Battle_InitArmies` raises it as army A,
    // side 4. Nothing may have written a destination for any of its units.
    let human: Vec<usize> = (1..=l2_sim::unit::MAX_UNITS)
        .filter(|&u| runner.units.get(u).is_live() && runner.units.get(u).human)
        .collect();
    assert!(!human.is_empty(), "the player's army must raise at least one unit");
    for u in &human {
        let unit = runner.units.get(*u);
        assert_eq!(
            (unit.target_x, unit.target_y),
            (unit.x, unit.y),
            "unit {u} was given a destination before anybody clicked",
        );
    }
    for (i, f) in runner.fighters.iter().enumerate() {
        if runner.sim.figures[f.sim].owner_is_human {
            assert_eq!(
                f.target,
                (f.x, f.y),
                "figure {i} was sent somewhere before anybody clicked",
            );
        }
    }

    // And it holds when the clock runs: two hundred ticks with no input leaves
    // every one of the player's men on the cell it deployed on.
    let before: Vec<(u8, u8)> = runner
        .fighters
        .iter()
        .map(|f| (f.x, f.y))
        .collect();
    runner.run(200);
    for (i, f) in runner.fighters.iter().enumerate() {
        if runner.sim.figures[f.sim].owner_is_human {
            assert_eq!(
                (f.x, f.y),
                before[i],
                "the player's figure {i} walked off on its own",
            );
        }
    }
}

/// The other half of the same claim, from the AI's side: the handler guard
/// selects **AI** units, not the player's.
///
/// `Battle_UpdateAllUnits`: `if (g_battleUnits[cur].humanControlled == 0)` — the
/// handler runs for everyone *except* the human. The player reported *"it might
/// be that enemy ai is getting applied to my units"*; this is the measurement
/// that says which side our dispatch picks.
#[test]
fn the_battle_ai_thinks_for_the_enemys_units_and_not_the_players() {
    let (mut g, _a, _m, attacker, defender) = a_battle_is_about_to_happen();
    let mut runner =
        l2_game::engagement::begin_fight(&mut g.kingdom, attacker, defender, None, 0x5EED_BEEF)
            .expect("two armies");
    runner.run(400);

    let mut ai_thought = 0;
    for u in 1..=l2_sim::unit::MAX_UNITS {
        let unit = runner.units.get(u);
        if !unit.is_live() {
            continue;
        }
        if unit.human {
            assert_eq!(unit.orders, 0, "a handler ran for the player's unit {u}");
            assert_eq!(unit.think, 0, "the player's unit {u} was given think time");
        } else if unit.orders > 0 {
            ai_thought += 1;
        }
    }
    assert!(ai_thought > 0, "no enemy unit thought at all in four hundred ticks");
}

/// **And an outnumbered AI holds its ground**, which is the other half of what
/// the player saw: *"I didn't see the enemy's units moving."*
///
/// He was right,
/// `Battle_UpdateStrengthAdvantage` (`0x0047FC01`) at roughly −50 against a
/// threshold of 5 (`g_aiAggressionThreshold`, `0x0057C8B4`), so every field
/// handler takes its **cautious** branch. With no attacker in `hit_memory` and
/// no enemy unit inside the charge radius — 8 cells for `UnitOrder_FieldFoot`,
/// 9 for `UnitOrder_FieldMelee`, against forty cells of field — the only
/// movement left in that branch is `Order_ToRallyWaypoint`.
///
/// And a rally waypoint is the side's **own** deployment marker. `[V]`:
/// `Battlefield_BuildRandom` writes all three of a side's waypoints from the
/// tile it just found the marker on — `g_rallyWaypoints[i] = g_foundTileX`,
/// both rally groups, in the terrain-`0x14` and terrain-`0x1E` arms.
/// cautious AI orders itself to stand where it is,
/// to it. That is the game, not a stall.
#[test]
fn an_outnumbered_ai_holds_its_ground_and_does_not_advance() {
    let (mut g, _a, _m, attacker, defender) = a_battle_is_about_to_happen();
    let mut runner =
        l2_game::engagement::begin_fight(&mut g.kingdom, attacker, defender, None, 0x5EED_BEEF)
            .expect("two armies");
    let ai: Vec<usize> = (1..=l2_sim::unit::MAX_UNITS)
        .filter(|&u| runner.units.get(u).is_live() && !runner.units.get(u).human)
        .collect();
    assert!(!ai.is_empty(), "the defence must raise at least one unit");
    // Its own marker, which is also all three of its rally waypoints.
    let (hx, hy) = runner.home(l2_sim::SIDE_A);
    let (px, py) = runner.home(l2_sim::SIDE_B);

    runner.run(4_000);
    assert!(runner.ai.strength_advantage < 5, "the AI must be the weaker side here");
    for u in ai {
        let unit = runner.units.get(u);
        assert!(unit.orders > 0, "AI unit {u} never thought");
        // It closes up **on its own marker** — the rally waypoint — and goes no
        // further. A unit that had marched on the enemy would be tens of cells
        // down the field, not six.
        assert!(
            (unit.x - i16::from(hx)).abs() <= 6 && (unit.y - i16::from(hy)).abs() <= 6,
            "AI unit {u} left its own marker ({hx}, {hy}) for ({}, {})",
            unit.x,
            unit.y,
        );
    }
    // And the player's end of the field is a long way off,
// marker" is a real claim.
    assert!(
        (i16::from(py) - i16::from(hy)).abs() > 30,
        "the two markers must be far enough apart for this to mean anything",
    );
    let _ = px;
}

/// Put the battle camera over the local player's own men.
///
/// `Battle_Start` seeds it at cell (0x20, 0x21) and the armies deploy at the two
/// markers, which may be nowhere near it — the original's player scrolls or
/// clicks the overview panel. A test that boxed the opening viewport and found
/// nothing would be asserting on its own emptiness, which is exactly the failure
/// `docs/agents.md` names.
fn look_at_the_players_men(g: &mut Game) {
    // The local realm is 1 and it owns the attacking army, which
    // `Battle_InitArmies` raises as army A — side 4, the one that is not
    // `SIDE_A` in `l2-sim`'s naming.
    let at = {
        let live = g.battle.as_ref().expect("a live battle");
        (0..live.runner.fighters.len())
            .find(|&i| live.runner.is_alive(i) && live.runner.fighters[i].side != l2_sim::SIDE_A)
            .map(|i| (live.runner.fighters[i].x, live.runner.fighters[i].y))
            .expect("the player has men on the field")
    };
    let live = g.battle.as_mut().expect("a live battle");
    live.cam = ((at.0 as i32 - 7).clamp(0, 80 - bf::VIEW_COLS), (at.1 as i32 - 7).clamp(0, 80 - bf::VIEW_ROWS));
}

/// **A player gives an order, and it reaches the simulation.**
///
/// The whole chain, driven with [`Event`] values and nothing else: press on the
/// field opens the drag (`g_screenId` `0x2A`), release commits the box
/// (`FUN_0043C247`), the men are selected, a release on empty ground is an order
/// (`FUN_0043C57D` → `BattleUnit_Order`),
/// that was clicked.
#[test]
fn a_box_selects_men_and_a_click_on_the_ground_orders_them() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::TAKE_THE_FIELD)));
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield));
    click(&mut m, &mut g, &a, on(bf::Button::Pause.rect()));

    look_at_the_players_men(&mut g);

    send(&mut m, &mut g, &a, Event::Pointer { x: bf::VIEW.x + 4, y: bf::VIEW.y + 4 });
    send(&mut m, &mut g, &a, Event::Click { x: bf::VIEW.x + 4, y: bf::VIEW.y + 4 });
    assert_eq!(
        g.battle.as_ref().map(|b| b.screen_id()),
        Some(0x2A),
        "a press on the field is the drag screen",
    );
    let far = (bf::VIEW.x + bf::VIEW.w - 4, bf::VIEW.y + bf::VIEW.h - 4);
    send(&mut m, &mut g, &a, Event::Pointer { x: far.0, y: far.1 });
    send(&mut m, &mut g, &a, Event::Release { x: far.0, y: far.1 });
    assert_eq!(g.battle.as_ref().map(|b| b.screen_id()), Some(0x29), "and the release ends it");

    let picked = {
        let live = g.battle.as_ref().expect("a live battle");
        live.runner.selected_count(1)
    };
    assert!(picked > 0, "the box picked nobody");

    // Now order them somewhere empty. Pick a cell in the viewport with no man
    // on it, and read the unit's destination back out of `l2-sim`.
    let (px, py, want) = {
        let live = g.battle.as_ref().expect("a live battle");
        let mut found = None;
        for row in 0..bf::VIEW_ROWS {
            for col in 0..bf::VIEW_COLS {
                let cell = (
                    (live.cam.0 + col).clamp(0, 79) as u8,
                    (live.cam.1 + row).clamp(0, 79) as u8,
                );
                if live.runner.occupant_of(cell.0, cell.1).is_none() {
                    found = Some((
                        bf::VIEW.x + col * bf::TILE + 4,
                        bf::VIEW.y + row * bf::TILE + 4,
                        cell,
                    ));
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        found.expect("some empty ground in view")
    };
    // **Press then release, the way a player does it**, because that is the
    // path that matters: the press opens the drag (`0x2A`), the release moves
    // nothing and hits nobody,
    // order arm behind it in the ladder gets the release. A test that released
    // without pressing would exercise a state the original never reaches.
    send(&mut m, &mut g, &a, Event::Pointer { x: px, y: py });
    send(&mut m, &mut g, &a, Event::Click { x: px, y: py });
    assert_eq!(g.battle.as_ref().map(|b| b.screen_id()), Some(0x2A));
    send(&mut m, &mut g, &a, Event::Release { x: px, y: py });

    let live = g.battle.as_ref().expect("a live battle");
    let unit = live.current_unit;
    assert_ne!(unit, 0, "the selection did not become a unit");
    let u = live.runner.units.get(unit);
    assert_eq!(
        (u.target_x as u8, u.target_y as u8),
        want,
        "the order did not reach the unit's destination",
    );
}

/// **A finished box does not also order at the corner it ended on.**
///
/// `Screen_FrameInput`'s `0x2A` arm is a ladder and `FUN_0043BF07` is above
/// `FUN_0043C57D`; when the drag guard consumes the release the dispatcher
/// `goto`s past the order guard. Ours ran both for a while, which is the sort of
/// thing that only shows up as *"my men wander off after I select them"*.
///
/// **Ablated**: making `release_field` return `true` for every case, or running
/// `order_at` unconditionally after it, turns this red.
#[test]
fn committing_a_selection_box_does_not_also_issue_an_order() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::TAKE_THE_FIELD)));
    click(&mut m, &mut g, &a, on(bf::Button::Pause.rect()));
    look_at_the_players_men(&mut g);

    // A destination nothing could have been ordered to before the box.
    let before: Vec<(i16, i16)> = {
        let live = g.battle.as_ref().expect("a live battle");
        (1..=80).map(|u| {
            let u = live.runner.units.get(u);
            (u.target_x, u.target_y)
        }).collect()
    };

    let start = (bf::VIEW.x + 4, bf::VIEW.y + 4);
    let end = (bf::VIEW.x + bf::VIEW.w - 4, bf::VIEW.y + bf::VIEW.h - 4);
    send(&mut m, &mut g, &a, Event::Pointer { x: start.0, y: start.1 });
    send(&mut m, &mut g, &a, Event::Click { x: start.0, y: start.1 });
    send(&mut m, &mut g, &a, Event::Pointer { x: end.0, y: end.1 });
    send(&mut m, &mut g, &a, Event::Release { x: end.0, y: end.1 });

    let live = g.battle.as_ref().expect("a live battle");
    assert!(live.runner.selected_count(1) > 0, "the box picked nobody");
    // The corner the box ended on, which is where a leaked order would land.
    let corner = (
        (live.cam.0 + (end.0 - bf::VIEW.x) / bf::TILE) as i16,
        (live.cam.1 + (end.1 - bf::VIEW.y) / bf::TILE) as i16,
    );
    for u in 1..=80usize {
        let unit = live.runner.units.get(u);
        if !unit.is_live() {
            continue;
        }
        let now = (unit.target_x, unit.target_y);
        if now == before[u - 1] {
            continue;
        }
        assert_ne!(now, corner, "unit {u} was ordered to the box's far corner");
    }
}

/// **The right button clears the selection; it does not leave the battle.**
///
/// `FUN_0043C2A9`'s right half is `FUN_0043C55C`, a deselect, and it is refused
/// only inside the overview panel. This is the arm the "right click exits"
/// habit would have replaced — `docs/decisions.md` C61's second pattern.
///
/// **Ablated**: making the right button pop the screen turns this red on the
/// first assertion.
#[test]
fn the_right_button_on_the_battlefield_clears_the_selection() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    press(&mut m, &mut g, &a, 'e');
    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::TAKE_THE_FIELD)));
    click(&mut m, &mut g, &a, on(bf::Button::Pause.rect()));
    look_at_the_players_men(&mut g);
    // Select everything in view.
    send(&mut m, &mut g, &a, Event::Pointer { x: bf::VIEW.x + 4, y: bf::VIEW.y + 4 });
    send(&mut m, &mut g, &a, Event::Click { x: bf::VIEW.x + 4, y: bf::VIEW.y + 4 });
    let far = (bf::VIEW.x + bf::VIEW.w - 4, bf::VIEW.y + bf::VIEW.h - 4);
    send(&mut m, &mut g, &a, Event::Pointer { x: far.0, y: far.1 });
    send(&mut m, &mut g, &a, Event::Release { x: far.0, y: far.1 });

    // A right release inside the overview panel is refused — and note the
    // off-by-one the original carries: the panel starts at 0x1E0 and the guard
    // tests 0x1E1, so its leftmost column deselects after all.
    let before = g.battle.as_ref().expect("a live battle").runner.selected_count(1);
    // **The fixture has to be non-empty or the last assertion checks nothing.**
    // `docs/agents.md`: a field is only tested if something a test reads was
    // written by something the game runs. Ablating the deselect passed this
    // test until this line existed.
    assert!(before > 0, "the box picked nobody, so clearing it proves nothing");
    send(&mut m, &mut g, &a, Event::RightClick { x: 0x1E8, y: 0x40 });
    assert_eq!(
        g.battle.as_ref().expect("a live battle").runner.selected_count(1),
        before,
        "the overview panel swallows the right button",
    );
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "and it is still a battle");

    // Anywhere else clears it, and still does not leave.
    send(&mut m, &mut g, &a, Event::RightClick { x: 100, y: 100 });
    assert_eq!(
        g.battle.as_ref().expect("a live battle").runner.selected_count(1),
        0,
        "the right button did not clear the selection",
    );
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "right-click must not leave the battle");
    assert!(g.battle.is_some());
}

/// The prompt draws inside its own window and nowhere else — the same
/// assertion the siege screen carries, for the same reason.
#[test]
fn the_prompt_paints_inside_its_window_and_over_nothing_above_it() {
    let (mut g, a, mut m, _, _) = a_battle_is_about_to_happen();
    let mut before = l2_view::Canvas::screen();
    {
        let ctx = Ctx { game: &mut g, assets: &a };
        m.draw(&ctx, &mut before);
    }
    press(&mut m, &mut g, &a, 'e');
    assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt));
    let mut after = l2_view::Canvas::screen();
    {
        let ctx = Ctx { game: &mut g, assets: &a };
        m.draw(&ctx, &mut after);
    }

    let window = battle::window();
    let (mut inside, mut above) = (0usize, 0usize);
    for y in 0..480usize {
        for x in 0..640usize {
            if before.at(x, y) == after.at(x, y) {
                continue;
            }
            let (px, py) = (x as i32, y as i32);
            if window.contains(px, py) {
                inside += 1;
            } else if py < window.y {
                above += 1;
            }
        }
    }
    assert!(inside > 500, "the prompt drew almost nothing: {inside} pixels");
    assert_eq!(above, 0, "it painted above its own window");
}

/// **A player: *"when I attacked and it asked me to decide it said I had 0 men,
/// I think it's because it was just mercenaries"*.**
///
/// `Mercenary_Hire` (`0x004AC7F3`) adds the band's men to `menTotal` (`+0x168`)
/// and never writes `+0x16C`, and `FUN_004224E7` — the roster painter both
/// battle screens share — draws `Ui_DrawCount(unit.menTotal, 0x48, …)` for the
/// total and folds the band into its own row, `if (unit.mercTroop == row) count
/// += unit.mercMen`, in **both** of its modes. Ours summed the seven rows for
/// the total and passed `Unit::troops` raw for the rows, so an army raised with
/// nothing but a hired band was seven zeroes under *"0 Total men"*, in the one
/// window that asks whether to fight with them.
///
/// The claim is that the two ways of holding two hundred pikemen draw the
/// **same** prompt, which is exactly what the fold and `menTotal` make true.
///
/// **Ablated, both halves.** Summing the rows for the total again: the band
/// army's total reads 0 against the other's 200. Passing `u.troops` in place of
/// `engagement::roster_of`: the pikeman row differs as well.
#[test]
fn a_mercenary_only_army_is_not_drawn_as_no_men_at_all() {
    let hired = the_prompt_over(true);
    let levied = the_prompt_over(false);
    let differing = (0..480usize)
        .flat_map(|y| (0..640usize).map(move |x| (x, y)))
        .filter(|&(x, y)| hired.at(x, y) != levied.at(x, y))
        .count();
    assert_eq!(differing, 0, "the band is not in the roster: {differing} pixels differ");
}

// ---------------------------------------------------------------------------
// …and the march that gets there **before** End Turn is pressed
// ---------------------------------------------------------------------------
//
// Every test above presses `e` in the same breath as the order, so the army
// always arrives inside the turn machine. A player does not: he gives the order
// and watches it walk, which is `turn::tick_units_only` on an ordinary frame.
// That door had no way to raise screen `0x12`, so his battle was settled by
// `Game::field_policy` — `Answer::Decline`, the autocalc — with nothing shown.
// A player reported it as *"it was me attacking a town and it just immediately
// resolved."*

/// Put the player's army one tile from **county 2's town** — plane-0 `0x40`,
///
/// (`0x004A6C68`) is reached from — and order the march. No turn is ended.
fn a_town_is_about_to_be_attacked() -> (Game, Assets, Machine, usize) {
    let (mut g, a, m) = on_the_map();
    let (mine, town) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    g.kingdom.campaign.map.set_flags(town.0, town.1, flags::CASTLE);
    g.kingdom.counties[2].anchor_x = town.0;
    g.kingdom.counties[2].anchor_y = town.1;
    let attacker = army_at(&mut g, 1, 1, 400, mine);
    g.order_unit_move(attacker, town).expect("a path one tile long");
    (g, a, m, attacker)
}

/// **The report: a town attacked from the map resolved with no prompt.**
///
/// `Battle_ChooseSettlement` (`0x004A6A30`) does not care which frame the army
/// arrived on — an army whose owner is a person is `Settlement::Prompt` and
/// screen `0x12` goes up. Ours only asked when the arrival happened inside the
/// turn machine.
#[test]
fn a_town_attacked_while_the_player_watches_asks_before_it_resolves() {
    let (mut g, a, mut m, attacker) = a_town_is_about_to_be_attacked();
    let before = g.kingdom.turn_count;

    run_until(&mut m, &mut g, &a, "the battle prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });

    assert_eq!(g.kingdom.turn_count, before, "no turn was ended: he was watching");
    assert!(
        g.kingdom.campaign.units.get(attacker).is_some(),
        "nothing may be resolved while the question is on the table",
    );
    let q = l2_game::turn::pending_question(&g).expect("a question");
    assert_eq!(q.choice_owner, 1, "the local player holds the choice");
    assert_eq!(q.attacker_men, 400);
    assert_eq!(q.county, 2, "the county fought over is the defender's");
    assert!(!q.is_siege);
}

/// The same door, the other way in: two armies meeting on an ordinary frame —
/// `Unit_EnterOccupiedTile` (`0x004658C1`).
#[test]
fn an_army_walked_into_while_the_player_watches_asks_too() {
    let (mut g, a, mut m, attacker, defender) = a_battle_is_about_to_happen();
    run_until(&mut m, &mut g, &a, "the battle prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });
    assert!(
        g.kingdom.campaign.units.get(attacker).is_some()
            && g.kingdom.campaign.units.get(defender).is_some(),
        "neither army may be touched while the question is on the table",
    );
}

/// **And the answer settles it and hands the map back**,
/// having been started. This is the half that would break if the suspension
/// were done by faking a turn: declining runs the autocalc, `0x13` shows the
/// result, and dismissing it must leave the campaign where it was
/// winding a season on.
#[test]
fn answering_a_watched_battle_settles_it_and_does_not_end_the_turn() {
    let (mut g, a, mut m, attacker) = a_town_is_about_to_be_attacked();
    let before = g.kingdom.turn_count;
    run_until(&mut m, &mut g, &a, "the battle prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });

    click(&mut m, &mut g, &a, on(battle::widget_rect(battle::DECLINE)));
    assert_eq!(m.top_id(), Some(ScreenId::BattleResult), "the result screen follows");
    let r = l2_game::turn::pending_report(&g).expect("a settled battle");
    assert_eq!(r.resolution, l2_game::engagement::Resolution::Autocalc);
    assert_eq!(r.attacker_men.0, 400);

    click(&mut m, &mut g, &a, on(battle::ok_rect()));
    run_until(&mut m, &mut g, &a, "the map to come back", |m, g| {
        m.top_id() == Some(ScreenId::Campaign) && !l2_game::turn::turn_in_flight(g)
    });
    assert_eq!(g.kingdom.turn_count, before, "answering a battle is not ending a turn");
    assert!(
        g.kingdom.campaign.units.get(attacker).is_none()
            || g.kingdom.counties[2].owner == 1,
        "the battle was actually fought: he lost the army or he took the county",
    );
}

/// **An AI's battle the player is not in still never opens a screen.**
///
/// `Battle_ChooseSettlement`'s first line — `if (!A.ownerIsHuman &&
/// !B.ownerIsHuman) return 0` —
/// prompt whenever a battle happens on a frame"*. Realm 2 marches on realm 3's
/// town while the human watches,
///
/// **Ablated**: dropping the `Settlement` test in `tick_units_only` and always
/// suspending turns this red.
#[test]
fn two_ai_armies_meeting_while_the_player_watches_open_nothing() {
    let (mut g, a, mut m) = on_the_map();
    g.kingdom.counties[3].owner = 3;
    g.kingdom.realms[3].in_play = true;
    g.kingdom.realms[3].strength = 5;
    let (mine, town) = adjacent_pair(|x| x as usize == BORDER_1_2 - 1);
    // County 2's town, attacked by realm 3 — neither realm is a person's.
    g.kingdom.campaign.map.set_flags(town.0, town.1, flags::CASTLE);
    g.kingdom.counties[2].anchor_x = town.0;
    g.kingdom.counties[2].anchor_y = town.1;
    let attacker = army_at(&mut g, 3, 3, 400, mine);
    // Not `Game::order_unit_move`: that door is the player's own, and this army
    // is not his.
    let map = g.kingdom.campaign.map.clone();
    l2_kingdom::movement::order_move(
        &map,
        &mut g.kingdom.campaign.units,
        attacker,
        town,
        l2_kingdom::movement::Routing::Direct,
    )
    .expect("a path one tile long");

    for _ in 0..8 {
        tick(&mut m, &mut g, &a);
    }
    // **No PROMPT, and nothing suspended.** This used to assert the campaign map
    // was on top, which was the same thing until the message window landed: a
    // battle between two other lords now raises a report, and a scroll over the
// map is the original's own behaviour. The
    // machine, so it is now made about those directly — the earlier form was a
    // stronger assertion than the test's own doc comment claims, and it went red
    // for a correct change.
    assert_ne!(
        m.top_id(),
        Some(ScreenId::BattlePrompt),
        "somebody else's war asked the player to take the field",
    );
    assert_ne!(
        m.top_id(),
        Some(ScreenId::Battlefield),
        "somebody else's war put the player on the field",
    );
    assert!(
        matches!(m.top_id(), Some(ScreenId::Campaign) | Some(ScreenId::Message)),
        "unexpected screen {:?} for somebody else's war",
        m.top_id(),
    );
    assert!(l2_game::turn::pending_question(&g).is_none());
    assert!(l2_game::turn::pending_report(&g).is_none());
    assert!(!l2_game::turn::turn_in_flight(&g), "and nothing left suspended");
}

