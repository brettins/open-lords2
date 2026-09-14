#![allow(unused_imports)]
use super::*;
use super::hires_and_sieges::*;
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

/// Put the player's army next to an enemy's and march it in, so the turn's unit
/// sweep raises a battle.
pub(crate) fn a_battle_is_about_to_happen() -> (Game, Assets, Machine, usize, usize) {
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

