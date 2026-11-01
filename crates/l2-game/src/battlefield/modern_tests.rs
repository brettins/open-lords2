//! C272, the Modern scheme's input and orders on a `LiveBattle`. Each test names the line whose
//! removal turns it red.

use super::*;
use crate::game::BattleControls;
use crate::input::Key;
use l2_net::Canonical;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::Troop;

const ME: u8 = 1;
const FOE: u8 = 2;

fn seat() -> LiveBattle {
    let field = l2_sim::terrain::build(&vec![0u8; l2_sim::terrain::CELLS], 1);
    let runner = BattleRunner::deploy_armies(
        field,
        0x5EED,
        Army { troops: &[(Troop::Swordsmen, 12u16), (Troop::Pikemen, 12u16)], owner: ME, human: true },
        Army { troops: &[(Troop::Peasants, 8u16)], owner: FOE, human: true },
    );
    let mut b = LiveBattle::new(runner, 0, 0, 0, None, ME, ME);
    b.latch_controls(BattleControls::Modern);
    b.pointer_in = true;
    b
}

fn units_of(b: &LiveBattle, realm: u8) -> Vec<u8> {
    (1..=80u8).filter(|&u| b.runner.units.get(u as usize).owner == realm).collect()
}

/// A man of the unit, brought into view; the pixel in the middle of his cell.
fn man_px(b: &mut LiveBattle, unit: u8) -> (i32, i32) {
    let f = (0..b.runner.fighters.len()).find(|&f| b.runner.unit_of(f) == unit as usize).expect("a man");
    let (x, y) = (b.runner.fighters[f].x, b.runner.fighters[f].y);
    b.look_at((x, y));
    cell_px(b, (x, y))
}

fn cell_px(b: &LiveBattle, c: (u8, u8)) -> (i32, i32) {
    (VIEW.x + (c.0 as i32 - b.cam.0) * TILE + TILE / 2, VIEW.y + (c.1 as i32 - b.cam.1) * TILE + TILE / 2)
}

fn click(b: &mut LiveBattle, p: (i32, i32), shift: bool) {
    b.pointer = p;
    b.update_hover();
    b.m_press(p.0, p.1, shift);
    b.m_release(p.0, p.1, shift);
}

fn men_of(b: &LiveBattle, unit: u8) -> usize {
    (0..b.runner.fighters.len())
        .filter(|&f| {
            let fig = &b.runner.sim.figures[b.runner.fighters[f].sim];
            fig.is_alive() && fig.unit == unit as u16
        })
        .count()
}

fn hash(b: &LiveBattle) -> u64 {
    Canonical::hash_of(&b.runner)
}

/// Ablate the unit lookup in `m_release` (a click on a man takes his figure only) or write
/// `selected` anywhere: the unit is not whole, or `figure.selected` is set, or the hash moves.
#[test]
fn a_click_picks_the_whole_unit_and_leaves_the_simulation_alone() {
    let mut b = seat();
    let (a, _) = (units_of(&b, ME)[0], 0);
    let before = (hash(&b), units_of(&b, ME));
    let p = man_px(&mut b, a);
    click(&mut b, p, false);
    assert_eq!(b.sel, vec![a]);
    assert_eq!(b.picked_fighters().len(), men_of(&b, a));
    assert!(b.runner.sim.figures.iter().all(|f| f.selected == 0), "figure.selected untouched");
    assert_eq!((hash(&b), units_of(&b, ME)), before, "no regroup, no new unit, same digest");
    let free = (0..VIEW_ROWS * VIEW_COLS)
        .map(|i| ((b.cam.0 + i % VIEW_COLS) as u8, (b.cam.1 + i / VIEW_COLS) as u8))
        .find(|&(x, y)| b.runner.occupant_of(x, y).is_none())
        .expect("an empty cell in view");
    let p = cell_px(&b, free);
    click(&mut b, p, false);
    assert!(b.sel.is_empty(), "empty ground deselects");
}

/// Ablate `!shift && !boxed` toggling in `m_release`: Shift replaces instead of adding or removing.
#[test]
fn shift_adds_and_removes_whole_units_and_a_box_takes_every_unit_with_a_man_in_it() {
    let mut b = seat();
    let (a, c) = (units_of(&b, ME)[0], units_of(&b, ME)[1]);
    let pa = man_px(&mut b, a);
    click(&mut b, pa, false);
    let pc = man_px(&mut b, c);
    click(&mut b, pc, true);
    assert_eq!(b.sel.len(), 2);
    let pa = man_px(&mut b, a);
    click(&mut b, pa, true);
    assert_eq!(b.sel, vec![c], "a second Shift-click drops it");
    b.sel.clear();
    let (x, y) = man_px(&mut b, a);
    b.pointer = (x - 15, y - 15);
    b.m_press(x - 15, y - 15, false);
    b.m_release(x + 14, y + 14, false);
    assert!(b.sel.contains(&a), "a box round one man takes his unit");
    assert!(b.runner.sim.figures.iter().all(|f| f.selected == 0));
}

/// Ablate the `prev == Some(slot)` arm: the second press never centres the camera.
#[test]
fn control_groups_set_select_and_a_second_press_centres_the_camera() {
    let mut b = seat();
    let a = units_of(&b, ME)[0];
    let p = man_px(&mut b, a);
    click(&mut b, p, false);
    assert!(b.m_key(Key::CtrlChar('1'), false));
    b.sel.clear();
    b.cam = (60, 60);
    assert!(b.m_key(Key::Char('1'), false));
    assert_eq!(b.sel, vec![a]);
    assert_eq!(b.cam, (60, 60), "once selects");
    assert!(b.m_key(Key::Char('1'), false));
    assert_ne!(b.cam, (60, 60), "twice looks");
    b.sel.clear();
    assert!(b.m_key(Key::Char('!'), true) || b.sel.is_empty(), "Shift+1 with nothing held adds nothing");
    assert!(b.runner.sim.figures.iter().all(|f| f.selected == 0));
}

/// Ablate `self.act_then(BattleOp::Command` in `issue`, or the `paused` test returning to `apply`.
#[test]
fn a_right_click_orders_the_selection_even_while_paused_without_regrouping() {
    let mut b = seat();
    assert!(b.paused, "a battle opens paused");
    let a = units_of(&b, ME)[0];
    let p = man_px(&mut b, a);
    click(&mut b, p, false);
    let dest = ((b.runner.units.get(a as usize).x as u8 + 6).min(78), b.runner.units.get(a as usize).y as u8);
    let (px, py) = cell_px(&b, dest);
    b.pointer = (px, py);
    b.update_hover();
    let before = units_of(&b, ME);
    assert!(b.m_right_click(px, py, false));
    assert_eq!((b.runner.units.get(a as usize).target_x, b.runner.units.get(a as usize).target_y), (dest.0 as i16, dest.1 as i16));
    assert_eq!(units_of(&b, ME), before, "no unit made or merged");
    assert!(b.runner.sim.figures.iter().all(|f| f.selected == 0));
}

/// Ablate the owner test in `BattleRunner::command`: another realm's unit is ordered.
#[test]
fn a_command_for_another_seats_unit_is_refused_and_changes_nothing() {
    let mut b = seat();
    let theirs = units_of(&b, FOE)[0];
    let before = hash(&b);
    let op = BattleOp::Command { units: vec![theirs], verb: Verb::Move, at: (3, 3), target: None, queue: false };
    assert!(!b.apply(ME, op));
    assert_eq!(hash(&b), before);
}

/// Ablate `armed` in `m_press`: the click selects instead of attack-moving.
#[test]
fn a_then_a_left_click_attack_moves_and_a_right_click_disarms() {
    let mut b = seat();
    let a = units_of(&b, ME)[0];
    let p = man_px(&mut b, a);
    click(&mut b, p, false);
    assert!(b.m_key(Key::Char('A'), false) && b.armed);
    let dest = cell_px(&b, ((b.cam.0 + 9) as u8, (b.cam.1 + 5) as u8));
    b.pointer = dest;
    b.update_hover();
    b.m_press(dest.0, dest.1, false);
    assert!(!b.armed);
    assert!(b.runner.modern.mode(a as usize).attack_move.is_some());
    b.m_key(Key::Char('A'), false);
    b.m_right_click(dest.0, dest.1, false);
    assert!(!b.armed, "right disarms and orders nothing");
}

/// Ablate `Verb::Merge` routing in `m_key`, or the `self.sel = vec![made]` in `apply`.
#[test]
fn m_merges_the_selection_into_one_unit_and_selects_it() {
    let mut b = seat();
    b.select_all();
    assert!(b.m_key(Key::Char('M'), false));
    assert_eq!(units_of(&b, ME).len(), 1);
    assert_eq!(b.sel, units_of(&b, ME));
}

/// Ablate `announce_modern`'s `is_modern`, or `latch_controls`'s latch.
#[test]
fn only_a_modern_seat_tells_the_sim_so_and_the_scheme_is_read_once() {
    let mut b = seat();
    b.announce_modern();
    assert!(b.runner.modern.plays(ME));
    let mut classic = seat();
    classic.controls = BattleControls::Classic;
    classic.latched = true;
    classic.announce_modern();
    classic.latch_controls(BattleControls::Modern);
    assert!(!classic.runner.modern.plays(ME) && !classic.is_modern());
}

/// Ablate `picked_count()` in `cursor`: an enemy under a Modern selection shows no sword.
#[test]
fn a_selected_modern_unit_shows_the_attack_cursor_over_an_enemy_and_a_right_click_attacks_it() {
    let mut b = seat();
    let a = units_of(&b, ME)[0];
    let p = man_px(&mut b, a);
    click(&mut b, p, false);
    let foe = units_of(&b, FOE)[0];
    let ff = (0..b.runner.fighters.len()).find(|&f| b.runner.unit_of(f) == foe as usize).expect("a man");
    let cell = (b.runner.fighters[ff].x, b.runner.fighters[ff].y);
    b.look_at(cell);
    let px = cell_px(&b, cell);
    b.pointer = px;
    b.update_hover();
    assert_eq!(b.cursor(), Cursor::Attack);
    let free = (0..VIEW_ROWS * VIEW_COLS)
        .map(|i| ((b.cam.0 + i % VIEW_COLS) as u8, (b.cam.1 + i / VIEW_COLS) as u8))
        .find(|&(x, y)| b.runner.occupant_of(x, y).is_none())
        .expect("an empty cell in view");
    b.pointer = cell_px(&b, free);
    b.update_hover();
    assert_eq!(b.cursor(), Cursor::Move, "a held unit over open ground: the walking cursor");
    b.pointer = px;
    b.update_hover();
    b.deferred = true;
    b.m_right_click(px.0, px.1, false);
    assert!(matches!(b.outbox.last(), Some(BattleOp::Command { target: Some(_), verb: Verb::Move, .. })));
}

/// Ablate the `deferred` refusal in `change_speed` (a shared world would run at one peer's speed), or
/// `is_modern()` in `ticks_this_frame` (Classic would double).
#[test]
fn plus_and_minus_set_half_one_and_double_speed_in_single_player_only() {
    let mut b = seat();
    assert_eq!(b.ticks_this_frame(), 1);
    assert!(b.m_key(Key::Char('+'), false));
    assert_eq!((b.ticks_this_frame(), b.ticks_this_frame()), (2, 2));
    b.m_key(Key::Char('-'), false);
    b.m_key(Key::Char('-'), false);
    let beats: Vec<u32> = (0..4).map(|_| b.ticks_this_frame()).collect();
    assert_eq!(beats.iter().sum::<u32>(), 2, "half speed steps every other frame: {beats:?}");
    let mut classic = seat();
    classic.controls = BattleControls::Classic;
    classic.speed = 1;
    assert_eq!(classic.ticks_this_frame(), 1);
    let mut shared = seat();
    shared.deferred = true;
    assert!(!shared.m_key(Key::Char('+'), false));
    assert_eq!(shared.ticks_this_frame(), 1);
}
