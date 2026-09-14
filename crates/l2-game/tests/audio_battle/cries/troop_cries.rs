#![allow(unused_imports)]
use super::*;
use super::audio_coverage::*;
use super::*;
use super::events::*;
use super::determinism::*;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use l2_game::audio::{self, names, Audio, Request, TroopCries};
use l2_game::battlefield::{self as bf, cry, Cry, LiveBattle};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::{Cues, Troop, SIDE_A, SIDE_B};

/// **`Sound_PlayTroopCry` is a round robin, not a dice roll** — and the first
/// cry of every pair is take **1**.
///
/// The literals are cells of `g_troopSounds` (`0x004DB0D0`) as the bytes hold
/// them. Ablation: read the counter before stepping it and the first assertion
/// gets `Peas_U1.wav`; drop the `class == 3` clause and the `_M` row walks into
/// `Peas_U5.wav`.
#[test]
fn a_troop_cry_is_a_round_robin_that_starts_at_take_one() {
    let mut cries = TroopCries::default();
    let peasants_selected: Vec<_> =
        (0..5).map(|_| cries.cry(Troop::Peasants.index() as u8, cry::SELECTED)).collect();
    assert_eq!(
        peasants_selected,
        [Some("Peas_U2.wav"), Some("Peas_U3.wav"), Some("Peas_U4.wav"), Some("Peas_U1.wav"), Some("Peas_U2.wav")]
    );
    // Class 3 is always take 0, however often it is asked for — docs/bugs.md D34.
    for _ in 0..4 {
        assert_eq!(cries.cry(Troop::Peasants.index() as u8, cry::MOAT), Some("Peas_M1.wav"));
    }
    // Each (troop, class) pair has a counter of its own.
    assert_eq!(cries.cry(Troop::Peasants.index() as u8, cry::ORDERED), Some("Peas_P2.wav"));
    assert_eq!(cries.cry(Troop::Knights.index() as u8, cry::ATTACK), Some("Knig_E2.wav"));
    // Two `_M` rows borrow another troop's voice.
    assert_eq!(cries.cry(Troop::Swordsmen.index() as u8, cry::MOAT), Some("Pike_M1.wav"));
    // A siege engine has a voice only when told to move, and its counter steps
    // either way.
    assert_eq!(cries.cry(Troop::Catapults.index() as u8, cry::SELECTED), None);
    assert_eq!(cries.cry(Troop::Catapults.index() as u8, cry::ORDERED), Some("movcat.wav"));
}

/// **The six cry arms, played.** A pick, a box, an order onto open ground, an
/// order onto an enemy and the `H` key — each through the event the player
/// would send — and the troop each cry reads is the one most of the selection
/// belongs to.
///
/// Ablation: delete `self.cry(cry::ATTACK)` in `LiveBattle::order_at` and the
/// fourth cry is missing; delete `self.cry(cry::SELECTED)` under the pick arm
/// and the first is.
#[test]
fn the_men_answer_a_pick_a_box_an_order_an_attack_and_the_formation_key() {
    let a = Assets::placeholder();
    let (mut g, mut m) = staged(&[(Troop::Pikemen, 4), (Troop::Archers, 2)], &[(Troop::Knights, 3)]);

    // A click on one archer picks him — `Battle_DragSelect#2`.
    let archer = live(&g)
        .runner
        .fighters
        .iter()
        .enumerate()
        .find(|(i, f)| f.troop == Troop::Archers && live(&g).runner.is_alive(*i))
        .map(|(_, f)| (f.x, f.y))
        .expect("an archer");
    let at = pixel(live(&g), archer);
    click_at(&mut m, &mut g, &a, at);
    assert_eq!(live(&g).cries, [Cry { troop: 5, class: cry::SELECTED }], "the pick");
    assert_eq!(live(&g).runner.selected_count(1), 1);

    // A box round the whole army: six figures, four of them pikemen — `#1`.
    box_the_army(&mut m, &mut g, &a);
    assert_eq!(live(&g).runner.selected_count(1), 6);
    assert_eq!(live(&g).cries[1], Cry { troop: 4, class: cry::SELECTED }, "the box");

    // A click on nobody, with men held, is an order — `Battle_OrderSelection#2`.
    let ground = pixel(live(&g), empty_cell(live(&g)));
    click_at(&mut m, &mut g, &a, ground);
    assert_eq!(live(&g).cries[2], Cry { troop: 4, class: cry::ORDERED }, "the order");

    // A click on a knight is an attack — `#3`.
    let knight = *cells_of(live(&g), SIDE_B).first().expect("a knight");
    let at = pixel(live(&g), knight);
    click_at(&mut m, &mut g, &a, at);
    assert_eq!(live(&g).cries.len(), 4, "the attack did not cry: {:?}", live(&g).cries);
    assert_eq!(live(&g).cries[3], Cry { troop: 4, class: cry::ATTACK }, "the attack");

    // `H` — `Battle_FormationKey#1`.
    send(&mut m, &mut g, &a, Event::KeyDown(Key::letter('h')));
    assert_eq!(live(&g).cries[4], Cry { troop: 4, class: cry::ORDERED }, "the formation key");
    assert_eq!(live(&g).cries.len(), 5, "and nothing else cried: {:?}", live(&g).cries);
}

/// **`H` cries even when there is nothing to turn and the battle is paused**,
/// because `FUN_0043C77A` calls `Sound_PlayTroopCry(1)` before it looks.
#[test]
fn the_formation_key_cries_with_nothing_held_and_the_battle_paused() {
    let a = Assets::placeholder();
    let (mut g, mut m) = staged(&[(Troop::Swordsmen, 2)], &[(Troop::Knights, 1)]);
    g.battle.as_deref_mut().unwrap().paused = true;
    send(&mut m, &mut g, &a, Event::KeyDown(Key::letter('v')));
    assert_eq!(live(&g).cries, [Cry { troop: 0, class: cry::ORDERED }]);
}

/// **`H` and `V` turn the unit while the battle is paused**, because nothing
/// between the key and the order asks about the pause. The window procedure's
/// `WM_CHAR` arm tests `g_battlePhase == 2 && DAT_0057A0CC == 0`, and
/// `FUN_0043C77A` tests `DAT_00553C6C == 0 && g_appPhase == 3`; the pause word is
/// `DAT_0053F238`, which `Battle_OrderClicked` tests.
/// player can put a paused battle's men in a column before a blow is struck.
/// `[V]`
///
/// Ablation: put `self.paused ||` back in front of `key_formation`'s unit check
/// and the unit is still a line after `V`.
#[test]
fn the_formation_key_turns_the_unit_while_the_battle_is_paused() {
    let a = Assets::placeholder();
    let (mut g, mut m) = staged(&[(Troop::Swordsmen, 4)], &[(Troop::Knights, 1)]);
    box_the_army(&mut m, &mut g, &a);
    let unit = live(&g).current_unit;
    assert_ne!(unit, 0, "the box made no unit to turn");
    assert_eq!(live(&g).runner.units.get(unit).orientation, 0, "a line to start with");

    g.battle.as_deref_mut().unwrap().paused = true;
    send(&mut m, &mut g, &a, Event::KeyDown(Key::letter('v')));
    assert_eq!(live(&g).runner.units.get(unit).orientation, 1, "V did not form a column while paused");
    assert!(live(&g).paused, "and the key is not an unpause");
    send(&mut m, &mut g, &a, Event::KeyDown(Key::letter('h')));
    assert_eq!(live(&g).runner.units.get(unit).orientation, 0, "H did not form a line while paused");
}

/// **An order onto the moat is class 3**, from a real castle's surface 2 under
/// the pointer.
#[test]
fn an_order_onto_the_moat_is_the_fourth_cry() {
    let a = Assets::placeholder();
    let level = 2; // the first moated level
    let runner = BattleRunner::deploy_siege(
        l2_sim::siege::our_castle(level),
        9,
        l2_sim::Muster { troops: &[(Troop::Knights, 40u32)], owner: 1, human: true },
        l2_sim::Muster { troops: &[(Troop::Archers, 8u32)], owner: 2, human: false },
        level,
    );
    let mut live_battle = LiveBattle::new(runner, 0, 0, 0, Some(level), 1, 1);
    live_battle.paused = false;
    // Look at the besieger first, to select him.
    let knights: Vec<(u8, u8)> = cells_of(&live_battle, SIDE_B);
    let (kx, ky) = knights[0];
    live_battle.cam = ((kx as i32 - 7).clamp(0, 80 - bf::VIEW_COLS), (ky as i32 - 7).clamp(0, 80 - bf::VIEW_ROWS));
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.player = 1;
    g.battle = Some(Box::new(live_battle));
    let mut m = Machine::new(ScreenId::Battlefield);
    let at = pixel(live(&g), (kx, ky));
    click_at(&mut m, &mut g, &a, at);
    assert_eq!(live(&g).runner.selected_count(1), 1, "a knight is held");

    // Now look at a stretch of moat with nobody on it, and order onto it.
    let field = &live(&g).runner.field;
    let moat = (0..80u8)
        .flat_map(|y| (0..80u8).map(move |x| (x, y)))
        .find(|&(x, y)| {
            field.at(x as usize, y as usize).surface == cry::MOAT_SURFACE
                && live(&g).runner.occupant_of(x, y).is_none()
        })
        .expect("a moated castle has surface 2");
    g.battle.as_deref_mut().unwrap().cam = (
        (moat.0 as i32 - 7).clamp(0, 80 - bf::VIEW_COLS),
        (moat.1 as i32 - 7).clamp(0, 80 - bf::VIEW_ROWS),
    );
    let at = pixel(live(&g), moat);
    click_at(&mut m, &mut g, &a, at);
    assert_eq!(live(&g).cries.last(), Some(&Cry { troop: Troop::Knights.index() as u8, class: cry::MOAT }));
}

// ---------------------------------------------------------------- the fighting

