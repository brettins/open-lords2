//! **The battlefield, heard.**
//!
//! ```text
//! cargo test -p l2-game --test audio_battle
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test audio_battle
//! ```
//!
//! `docs/audio.json` had 31 of the game's sound triggers on the battlefield
//! and none of them sounded: 25 `blocked` on *"per-man events"* and the six
//! troop cries `missing`. The block was the **event stream, not the sounds** —
//! `l2-sim` resolved figure and unit state and recorded nothing a listener
//! could hear — and `l2_sim::cue` is that stream. These tests are what it and
//! the two listeners on it are held to:
//!
//! * the right file for the right event and troop, **with the file names pinned
//! as literals read out of `Lords2.exe`**
//!   under test;
//! * the original's two throttles, which are drop-if-busy and nothing else;
//! * and that **a battle played with sound is the same battle as one played
//!   without**, tick for tick.
//!
//! Everything a player does here is an [`Event`] through [`Machine::handle`], and
//! every counter a sound is decided from was written by the running battle.
//! Five tests read the install or the executable and skip without them.

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

// ------------------------------------------------------------------ the world

/// Two deployment markers eight rows apart, so both armies fit one screen.
fn near_field() -> l2_sim::Battlefield {
    let mut layer = vec![0u8; l2_sim::terrain::CELLS];
    layer[36 * l2_sim::terrain::DIM + 40] = 0x04;
    layer[44 * l2_sim::terrain::DIM + 40] = 0x0F;
    l2_sim::terrain::build(&layer, 1)
}

/// A live battle between a human (realm 1, side 0) and an AI (realm 2,
/// side 4), unpaused and looking at the middle of the two markers.
fn battle(human: &[(Troop, u16)], ai: &[(Troop, u16)]) -> LiveBattle {
    let runner = BattleRunner::deploy_armies(
        near_field(),
        0x5EED,
        Army { troops: ai, owner: 2, human: false },
        Army { troops: human, owner: 1, human: true },
    );
    let mut live = LiveBattle::new(runner, 0, 0, 0, None, 1, 1);
    live.paused = false;
    live.cam = (33, 33);
    live
}

fn staged(human: &[(Troop, u16)], ai: &[(Troop, u16)]) -> (Game, Machine) {
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.player = 1;
    g.battle = Some(Box::new(battle(human, ai)));
    (g, Machine::new(ScreenId::Battlefield))
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn live(g: &Game) -> &LiveBattle {
    g.battle.as_deref().expect("a live battle")
}

/// The centre of a cell on screen.
fn pixel(live: &LiveBattle, cell: (u8, u8)) -> (i32, i32) {
    let (cx, cy) = (cell.0 as i32 - live.cam.0, cell.1 as i32 - live.cam.1);
    assert!(
        (0..bf::VIEW_COLS).contains(&cx) && (0..bf::VIEW_ROWS).contains(&cy),
        "cell {cell:?} is off screen with the camera at {:?}",
        live.cam
    );
    (bf::VIEW.x + cx * bf::TILE + bf::TILE / 2, bf::VIEW.y + cy * bf::TILE + bf::TILE / 2)
}

/// The cells of one side's living figures.
fn cells_of(live: &LiveBattle, side: u8) -> Vec<(u8, u8)> {
    live.runner
        .fighters
        .iter()
        .enumerate()
        .filter(|(i, f)| f.side == side && live.runner.is_alive(*i))
        .map(|(_, f)| (f.x, f.y))
        .collect()
}

/// A click that does not move: pointer, press, release.
fn click_at(m: &mut Machine, g: &mut Game, a: &Assets, (x, y): (i32, i32)) {
    send(m, g, a, Event::Pointer { x, y });
    send(m, g, a, Event::Click { x, y });
    send(m, g, a, Event::Release { x, y });
}

/// A box drawn round every living figure of the human side.
fn box_the_army(m: &mut Machine, g: &mut Game, a: &Assets) {
    let cells = cells_of(live(g), SIDE_A);
    let lo = (cells.iter().map(|c| c.0).min().unwrap(), cells.iter().map(|c| c.1).min().unwrap());
    let hi = (cells.iter().map(|c| c.0).max().unwrap(), cells.iter().map(|c| c.1).max().unwrap());
    let (lx, ly) = pixel(live(g), lo);
    let (hx, hy) = pixel(live(g), hi);
    // A quarter-tile inside each outer edge, which both of the box's rounding
    // rules keep.
    let from = (lx - bf::TILE / 2 + 2, ly - bf::TILE / 2 + 2);
    let to = (hx + bf::TILE / 2 - 2, hy + bf::TILE / 2 - 2);
    send(m, g, a, Event::Pointer { x: from.0, y: from.1 });
    send(m, g, a, Event::Click { x: from.0, y: from.1 });
    send(m, g, a, Event::Pointer { x: to.0, y: to.1 });
    send(m, g, a, Event::Release { x: to.0, y: to.1 });
}

/// A cell on screen with nobody on it.
fn empty_cell(live: &LiveBattle) -> (u8, u8) {
    let (cx, cy) = (live.cam.0 as u8, live.cam.1 as u8);
    (cx + 1, cy + 1)
}

fn file_of(r: Request) -> &'static str {
    match r {
        Request::Slot(n) => names::slot(names::Bank::Battle, n).expect("a battle slot that plays"),
        Request::File(f) => f,
    }
}

/// Play a battle by ticking it the way the screen does, and collect every
/// file the ladder asked for from the cues the battle itself wrote.
///
/// `advance` sends the human side at the enemy's marker every 300 ticks, which
/// is what a melee needs and what a line of archers must **not** be given: a
/// shooter only looses standing at its destination, so marching it forward
/// turns an archery test into a brawl.
fn fight(
    human: &[(Troop, u16)],
    ai: &[(Troop, u16)],
    ticks: u32,
    advance: bool,
) -> (BTreeSet<&'static str>, LiveBattle) {
    let mut live = battle(human, ai);
    let mut asked = BTreeSet::new();
    for t in 0..ticks {
        if advance && t % 300 == 0 {
            live.runner.order_side(SIDE_A, 40, 44);
        }
        let was = live.runner.sim.cues;
        live.tick();
        for r in audio::battle_requests(&was, &live.runner.sim.cues) {
            asked.insert(file_of(r));
        }
        if live.conclusion.is_some() {
            break;
        }
    }
    (asked, live)
}

// ------------------------------------------------------------- the troop cries

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
/// `DAT_0053F238`, which `Battle_OrderClicked` tests and these do not. So a
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

/// **Every call the battlefield's ladder can make, pinned.** Twenty-two sites,
/// seventeen files, and the verb each one uses: every bank slot is
/// drop-if-busy on its own buffer, and the wall coming down and the bridge
/// catching are the one-shot buffer.
///
/// A new arm makes this red with the request it added — which is how the six
/// that fire, oil, the tower and the high rampart added arrived here.
#[test]
fn the_battle_ladder_is_twenty_two_calls_and_seventeen_files() {
    let every = Cues::of_every_occasion();
    let asked = audio::battle_requests(&Cues::default(), &every);
    assert_eq!(
        asked,
        [
            Request::Slot(4),
            Request::Slot(5),
            Request::Slot(5),
            Request::Slot(6),
            Request::Slot(0xb),
            Request::Slot(0xc),
            Request::Slot(0xb),
            Request::Slot(0xc),
            Request::Slot(0xf),
            Request::Slot(0x10),
            Request::Slot(10),
            Request::Slot(8),
            Request::Slot(10),
            Request::Slot(8),
            Request::Slot(0xd),
            Request::Slot(9),
            Request::Slot(7),
            Request::Slot(0xe),
            Request::File("bathit2.wav"),
            Request::Slot(3),
            Request::Slot(0x11),
            Request::File("dest_ind.wav"),
        ]
    );
    let files: BTreeSet<&str> = asked.into_iter().map(file_of).collect();
    assert_eq!(
        files.into_iter().collect::<Vec<_>>(),
        [
            "bathit2.wav",
            "bow_hit.wav",
            "bowmen1.wav",
            "catfire.wav",
            "cathit.wav",
            "catmiss.wav",
            "cros_hit.wav",
            "crossbow.wav",
            "deadguy2.wav",
            "deadguy3.wav",
            "deadguy4.wav",
            "dest_ind.wav",
            "pouroil.wav",
            "siegedoc.wav",
            "sword2.wav",
            "sword3.wav",
            "sword5.wav",
        ]
    );
    // And a tick in which nothing happened asks for nothing.
    assert!(audio::battle_requests(&every, &every).is_empty());
}

/// **The sword is the striker's**: knights and swordsmen `sword2.wav`, macemen
/// `sword5.wav`, everybody else `sword3.wav` — from three real battles, each
/// fought by one troop type on both sides, so only one rung can be reached.
///
/// Ablation: swap `Troop::Macemen` and `Troop::Knights` in the ladder and the
/// knights' battle asks for `sword5.wav`.
#[test]
fn the_sword_a_man_falls_to_is_chosen_by_the_troop_that_struck_him() {
    for (troop, sword, not) in [
        (Troop::Knights, "sword2.wav", ["sword3.wav", "sword5.wav"]),
        (Troop::Macemen, "sword5.wav", ["sword2.wav", "sword3.wav"]),
        (Troop::Peasants, "sword3.wav", ["sword2.wav", "sword5.wav"]),
    ] {
        let (asked, live) = fight(&[(troop, 3)], &[(troop, 3)], 12_000, true);
        assert!(asked.contains(sword), "{troop:?} asked for {asked:?}");
        for n in not {
            assert!(!asked.contains(n), "{troop:?} asked for {n}: {asked:?}");
        }
        // The death cry is the dying figure's side: side 0 `deadguy2`, side 4
        // `deadguy3`.
        let lost = |side| cells_of(&live, side).len() < 3;
        assert_eq!(asked.contains("deadguy2.wav"), lost(SIDE_A), "{troop:?}: {asked:?}");
        assert_eq!(asked.contains("deadguy3.wav"), lost(SIDE_B), "{troop:?}: {asked:?}");
        assert!(lost(SIDE_A) || lost(SIDE_B), "{troop:?}: nobody died in 12,000 ticks");
    }
}

/// **A bow is a bow and a crossbow is a crossbow**, at the loose and at the
/// hit, and an arrow's kill is `deadguy4.wav`.
#[test]
fn a_bow_and_a_crossbow_are_heard_as_themselves() {
    for (troop, loose, hit, other) in [
        (Troop::Archers, "bowmen1.wav", "bow_hit.wav", ["crossbow.wav", "cros_hit.wav"]),
        (Troop::Crossbowmen, "crossbow.wav", "cros_hit.wav", ["bowmen1.wav", "bow_hit.wav"]),
    ] {
        let (asked, _) = fight(&[(troop, 6)], &[(Troop::Peasants, 1)], 6_000, false);
        assert!(asked.contains(loose), "{troop:?}: {asked:?}");
        assert!(asked.contains(hit), "{troop:?}: {asked:?}");
        assert!(asked.contains("deadguy4.wav"), "{troop:?} shot nobody dead: {asked:?}");
        for n in other {
            assert!(!asked.contains(n), "{troop:?} asked for {n}: {asked:?}");
        }
        assert!(!asked.contains("catfire.wav"));
    }
}

// ---------------------------------------------------------------- determinism

/// One battle, played twice from the same events: once with a
/// [`audio::Director`] listening after every tick, once with nothing listening.
/// Returns the two games.
fn play_it_twice(sound: &mut Audio) -> (Game, Game) {
    let a = Assets::placeholder();
    let armies = (
        [(Troop::Swordsmen, 3), (Troop::Archers, 3)],
        [(Troop::Crossbowmen, 3), (Troop::Macemen, 3)],
    );
    let (mut heard, mut hm) = staged(&armies.0, &armies.1);
    let (mut silent, mut sm) = staged(&armies.0, &armies.1);
    let mut director = audio::Director::new();

    for (g, m) in [(&mut heard, &mut hm), (&mut silent, &mut sm)] {
        box_the_army(m, g, &a);
        let enemy = *cells_of(live(g), SIDE_B).first().unwrap();
        let at = pixel(live(g), enemy);
        click_at(m, g, &a, at);
        // Park the pointer mid-field so the edge scroll never moves either
        // camera.
        send(m, g, &a, Event::Pointer { x: 240, y: 240 });
    }
    assert_eq!(heard.battle, silent.battle, "the same events left the two games different");

    for t in 0..4_000 {
        tick(&mut hm, &mut heard, &a);
        director.listen(sound, &hm, &heard);
        tick(&mut sm, &mut silent, &a);
        assert!(heard.battle == silent.battle, "sound changed the battle at tick {t}");
        if t % 500 == 250 {
            // An order mid-battle, to both, so a cry lands while men are dying.
            for (g, m) in [(&mut heard, &mut hm), (&mut silent, &mut sm)] {
                send(m, g, &a, Event::KeyDown(Key::letter('h')));
            }
        }
    }
    (heard, silent)
}

/// **Sound does not change the battle, tick for tick** — with the silent
/// layer, so it runs everywhere.
///
/// Compared by the whole `LiveBattle`'s derived equality at every tick — every
/// field of the runner, the figures, the missiles, the cues and the cries — and
/// by the saved game's bytes and trailing hash at the end. That is strictly more
/// than the lockstep digest, which is a hand-written projection of the same
/// runner (`crates/l2-sim/tests/lockstep.rs`).
///
/// **A green ablation, and it is the finding.**
/// `Director` whose deletion turns this red, because `Director::listen` holds
/// `&Game` and cannot write to it: the guarantee is the type, and this test is
/// the tripwire for the day somebody changes the signature. Ablated the other
/// way instead — an extra `runner.step()` on one copy at tick 100 — it goes red
/// at tick 100.
#[test]
fn sound_does_not_change_the_battle() {
    let mut silent_layer = Audio::silent();
    let (heard, silent) = play_it_twice(&mut silent_layer);
    assert_eq!(l2_game::save::encode(&heard), l2_game::save::encode(&silent));
    // The listener had something to listen to, or the comparison is empty.
    let b = live(&heard);
    assert!(b.cries.len() >= 2, "cries: {:?}", b.cries);
    assert!(b.runner.sim.cues.loosed(l2_sim::WeaponClass::Bow) > 0, "{:?}", b.runner.sim.cues);
    let melee: u32 = l2_sim::ALL_TROOPS.iter().map(|&t| b.runner.sim.cues.melee_casualties(t)).sum();
    assert!(melee > 0, "{:?}", b.runner.sim.cues);
}

/// **The proving ground as a live battle**: `l2_sim::proving`'s siege, unpaused,
/// in a game on the battlefield screen.
fn staged_siege() -> (Game, Machine) {
    let runner = l2_sim::proving::deploy();
    let mut live = LiveBattle::new(runner, 0, 0, 0, Some(l2_sim::proving::LEVEL), 1, 1);
    live.paused = false;
    live.cam = (20, 25);
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.player = 1;
    g.battle = Some(Box::new(live));
    (g, Machine::new(ScreenId::Battlefield))
}

/// One siege that pours oil, docks a tower, burns a bridge and the men on it
/// and bounces catapult shots off a wall four high, played twice from the same
/// timetable: once with a [`audio::Director`] listening after every tick, once
/// with nothing listening.
fn play_a_siege_twice(sound: &mut Audio) -> (Game, Game) {
    let a = Assets::placeholder();
    let (mut heard, mut hm) = staged_siege();
    let (mut silent, mut sm) = staged_siege();
    let mut director = audio::Director::new();
    for (g, m) in [(&mut heard, &mut hm), (&mut silent, &mut sm)] {
        send(m, g, &a, Event::Pointer { x: 240, y: 240 });
    }
    for t in 0..2_000 {
        for g in [&mut heard, &mut silent] {
            l2_sim::proving::orders(&mut g.battle.as_deref_mut().unwrap().runner);
        }
        tick(&mut hm, &mut heard, &a);
        director.listen(sound, &hm, &heard);
        tick(&mut sm, &mut silent, &a);
        assert!(heard.battle == silent.battle, "sound changed the siege at tick {t}");
    }
    (heard, silent)
}

/// **Sound does not change a siege that burns, tick for tick** — C166's proof,
/// extended to the six sites fire, oil, the tower and the high rampart added.
///
/// The same comparison as [`sound_does_not_change_the_battle`]: the whole
/// `LiveBattle` at every tick — every field of the runner, the missile array
/// with its fires and its stream, the battlefield with its burning cells and
/// its ramp — and the saved game's bytes at the end. The second half of the
/// assertion is what keeps the first from being about a quiet siege: every one
/// of the six occasions happened, so the listener was asked for all six.
#[test]
fn sound_does_not_change_a_siege_that_burns() {
    let mut silent_layer = Audio::silent();
    let (heard, silent) = play_a_siege_twice(&mut silent_layer);
    assert_eq!(l2_game::save::encode(&heard), l2_game::save::encode(&silent));
    let c = live(&heard).runner.sim.cues;
    assert_eq!(c.oil_poured(), 1, "{c:?}");
    assert_eq!(c.towers_docked(), 1, "{c:?}");
    assert!(c.bridges_fired() >= 1, "{c:?}");
    assert!(c.walls_missed() >= 1, "{c:?}");
    assert!(c.burn_deaths(SIDE_B) >= 1, "{c:?}");
    let asked = audio::battle_requests(&Cues::default(), &c);
    for want in [Request::Slot(3), Request::Slot(0x11), Request::File("dest_ind.wav"), Request::Slot(0x10), Request::Slot(0xc)] {
        assert!(asked.contains(&want), "{want:?} was never asked for: {asked:?}");
    }
    assert_eq!(live(&heard).conclusion, None, "the siege was still being fought");
}

// ------------------------------------------------------ against the install

/// The install's case-insensitive lookup, as `tests/audio_install.rs` does it.
fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    let want = name.to_ascii_lowercase();
    std::fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).find_map(|e| {
        let p = e.path();
        let n = p.file_name()?.to_str()?.to_ascii_lowercase();
        (n == want).then_some(p)
    })
}

/// **The same, with sound** — decoded and mixed from the
/// install, and the assertion that sound was made is what keeps the equality
/// from being about a silence.
#[test]
fn sound_that_plays_does_not_change_the_battle() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so nothing to play");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut sound = Audio::headless(&platform.vfs);
    let (heard, silent) = play_it_twice(&mut sound);
    assert_eq!(l2_game::save::encode(&heard), l2_game::save::encode(&silent));
    let played = sound.heard();
    for want in ["swor_u2.wav", "bowmen1.wav", "crossbow.wav"] {
        assert!(played.contains(&want), "{want} was never played: {played:?}");
    }
}

/// **The same siege with sound** — and the four files the
/// siege added are among what was opened: the pour, the dock, the bridge and
/// the shot off the high wall, with a burning man's death cry beside them.
#[test]
fn sound_that_plays_does_not_change_a_siege_that_burns() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so nothing to play");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut sound = Audio::headless(&platform.vfs);
    let (heard, silent) = play_a_siege_twice(&mut sound);
    assert_eq!(l2_game::save::encode(&heard), l2_game::save::encode(&silent));
    let played = sound.heard();
    for want in ["pouroil.wav", "siegedoc.wav", "dest_ind.wav", "catmiss.wav", "deadguy3.wav"] {
        assert!(played.contains(&want), "{want} was never played: {played:?}");
    }
}

/// **Every cry a player can hear ships, and the ones D34 says cannot be asked
/// for are the ones that do not** — including two the bug list said were real.
#[test]
fn every_reachable_cry_ships_and_nine_unreachable_names_do_not() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install");
    };
    let mut cries = TroopCries::default();
    let mut reachable = BTreeSet::new();
    for troop in 0..11u8 {
        for class in 0..4u8 {
            for _ in 0..4 {
                if let Some(n) = cries.cry(troop, class) {
                    reachable.insert(n);
                }
            }
        }
    }
    assert_eq!(reachable.len(), 66, "the reachable cries moved");
    for n in &reachable {
        assert!(find(&dir, n).is_some(), "{n} is reachable and does not ship");
    }
    // The three cells of every `_M` row that class 3 can never select. Seven
    // are `_F1` and two are cell 14, which `docs/bugs.md` D34 used to say always
    // named a real file.
    for n in [
        "Peas_F1.wav", "Cros_F1.wav", "Mace_F1.wav", "Swor_F1.wav", "Pike_F1.wav", "Arch_F1.wav",
        "Knig_F1.wav", "Swor_U3.wav", "Arch_U3.wav",
    ] {
        assert!(!reachable.contains(n), "{n} should be unreachable");
        assert!(find(&dir, n).is_none(), "{n} ships after all");
    }
}

/// **`TROOP_CRIES` is the executable's table**, cell for cell.
#[test]
fn the_troop_cry_table_is_the_one_at_0x004db0d0() {
    let exe = l2_testkit::executable!();
    let base = l2_testkit::pe::va_to_offset(&exe, 0x004D_B0D0).expect("the table is in the image");
    for (t, troop) in names::TROOP_CRIES.iter().enumerate() {
        for (c, class) in troop.iter().enumerate() {
            for (k, name) in class.iter().enumerate() {
                let at = base + t * 0x100 + c * 0x40 + k * 0x10;
                let cell = &exe[at..at + 16];
                let end = cell.iter().position(|&b| b == 0).unwrap_or(16);
                assert_eq!(
                    std::str::from_utf8(&cell[..end]).unwrap(),
                    *name,
                    "g_troopSounds[{t}][{c}][{k}]"
                );
            }
        }
    }
}

/// **Eighty-three more files**: the ladder's seventeen and the sixty-six cries,
/// asked for through the verbs the director uses and counted by what the layer
/// opened. With `tests/audio_wiring.rs`' 530 and 30 that is **643 of
/// 771**. It was seventy-nine until the siege could pour oil, dock a tower,
/// burn a bridge and bounce a shot off a wall four high.
#[test]
fn the_battlefield_is_eighty_three_more_files() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so nothing to open");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut sound = Audio::headless(&platform.vfs);
    for r in audio::battle_requests(&Cues::default(), &Cues::of_every_occasion()) {
        // `play_effect`
        // loses a file to a busy buffer is counting the timing, not the reach.
        sound.play_effect(file_of(r));
    }
    let mut cries = TroopCries::default();
    for troop in 0..11u8 {
        for class in 0..4u8 {
            for _ in 0..4 {
                if let Some(n) = cries.cry(troop, class) {
                    // The same, for the cries: `play_file` would drop all but
                    // the first, since nothing here mixes.
                    sound.play_effect(n);
                }
            }
        }
    }
    let heard = sound.heard();
    let ladder = [
        "bathit2.wav", "bow_hit.wav", "bowmen1.wav", "cathit.wav", "catfire.wav", "catmiss.wav",
        "cros_hit.wav", "crossbow.wav", "deadguy2.wav", "deadguy3.wav", "deadguy4.wav",
        "dest_ind.wav", "pouroil.wav", "siegedoc.wav", "sword2.wav", "sword3.wav", "sword5.wav",
    ];
    for f in ladder {
        assert!(heard.contains(&f), "{f} was not opened: {heard:?}");
    }
    assert_eq!(heard.len() - ladder.len(), 66, "the cries: {heard:?}");
    assert_eq!(heard.len(), 83);
}

/// **The throttle is the original's and nothing else.** `Sound_PlayFile` has
/// one buffer: a cry over a cry is dropped, a cry over the narrator is dropped,
/// and **the take a dropped cry would have played is spent** — because the
/// counter steps before the drop.
///
/// Played through the director, from orders given as events. Ablation: move
/// the busy test in `Audio::play_file` after the load and `peas_p3.wav` is heard.
#[test]
fn a_cry_over_a_cry_is_dropped_and_its_take_is_spent() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so nothing to drop");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut sound = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    let a = Assets::placeholder();
    let (mut g, mut m) = staged(&[(Troop::Peasants, 4)], &[(Troop::Knights, 1)]);
    let drain = |sound: &mut Audio, name: &str| {
        let mut buf = vec![0f32; 4096];
        for _ in 0..2_000 {
            if !sound.is_playing(name) {
                return;
            }
            sound.mix(&mut buf);
        }
        panic!("{name} never finished");
    };

    box_the_army(&mut m, &mut g, &a);
    director.listen(&mut sound, &m, &g);
    assert!(sound.heard().contains(&"peas_u2.wav"), "the selection's cry: {:?}", sound.heard());
    drain(&mut sound, "peas_u2.wav");

    // Two orders inside one tick. The first is take 1; the second is take 2,
    // asked for while take 1 is sounding, and dropped.
    let ground = pixel(live(&g), empty_cell(live(&g)));
    click_at(&mut m, &mut g, &a, ground);
    click_at(&mut m, &mut g, &a, ground);
    director.listen(&mut sound, &m, &g);
    assert!(sound.heard().contains(&"peas_p2.wav"), "{:?}", sound.heard());
    assert!(!sound.heard().contains(&"peas_p3.wav"), "a cry over a cry was played");
    drain(&mut sound, "peas_p2.wav");

    // The next order is take 3, not take 2: the dropped cry spent its take.
    click_at(&mut m, &mut g, &a, ground);
    director.listen(&mut sound, &m, &g);
    assert!(sound.heard().contains(&"peas_p4.wav"), "{:?}", sound.heard());
    assert!(!sound.heard().contains(&"peas_p3.wav"), "the dropped take came back");

    // And the narrator holds the same buffer.
    drain(&mut sound, "peas_p4.wav");
    assert!(sound.play_file("S021_01.wav", true), "the narrator, into an idle buffer");
    assert!(!sound.play_file("Knig_E2.wav", true), "a cry played over the narrator");
}
