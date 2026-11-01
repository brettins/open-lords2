//! C272, the simulation half of the modern battle controls. Each test names the line whose removal
//! turns it red.

use super::*;
use l2_net::{Canonical, Decode, Encode, Reader};

const ME: u8 = 2;
const FOE: u8 = 1;

/// Both armies human: no AI handler thinks, so the only mover is the order under test.
fn duel(mine: &[(Troop, u16)], foe: &[(Troop, u16)]) -> BattleRunner {
    let mut r = BattleRunner::deploy_armies(
        blank_field(),
        DEFAULT_SEED,
        Army { troops: mine, owner: ME, human: true },
        Army { troops: foe, owner: FOE, human: true },
    );
    r.settle();
    r
}

fn unit_of_troop(r: &BattleRunner, troop: Troop) -> usize {
    r.unit_of(r.fighters.iter().position(|f| f.troop == troop).expect("troop deployed"))
}

/// The unit's men in a block four wide with its corner at `(x, y)`.
fn put(r: &mut BattleRunner, unit: usize, x: u8, y: u8) {
    for (k, m) in r.members(unit).into_iter().enumerate() {
        let old = r.fighters[m].y as usize * DIM + r.fighters[m].x as usize;
        if r.occupant[old] == Some(m as u16) {
            r.occupant[old] = None;
        }
        let (nx, ny) = (x + (k % 4) as u8, y + (k / 4) as u8);
        let f = &mut r.fighters[m];
        (f.x, f.y, f.target) = (nx, ny, (nx, ny));
        f.path.clear();
        f.progress = Progress::default();
        r.occupant[ny as usize * DIM + nx as usize] = Some(m as u16);
    }
    r.settle();
}

fn announce(r: &mut BattleRunner) {
    assert!(r.command(ME, &[], Verb::Stop, (0, 0), None, false).is_some());
}

fn bytes(r: &BattleRunner) -> Vec<u8> {
    let mut out = Canonical::recording();
    r.encode(&mut out);
    out.finish().bytes.expect("a recording encoder keeps its bytes")
}

fn states(r: &BattleRunner, unit: usize) -> Vec<State> {
    r.members(unit).into_iter().map(|m| r.sim.figures[r.fighters[m].sim].state).collect()
}

/// Ablate `all(own)` in `command` and the foe's unit is ordered by realm 2.
#[test]
fn a_command_naming_another_realms_unit_is_refused_whole() {
    let mut r = duel(&[(Troop::Swordsmen, 4)], &[(Troop::Pikemen, 4)]);
    let (mine, theirs) = (unit_of_troop(&r, Troop::Swordsmen), unit_of_troop(&r, Troop::Pikemen));
    let before = (bytes(&r), r.units.get(mine).target_x);
    assert_eq!(r.command(ME, &[mine as u8, theirs as u8], Verb::Move, (60, 60), None, false), None);
    assert_eq!(r.command(ME, &[0], Verb::Move, (60, 60), None, false), None, "slot 0 is no unit");
    assert_eq!(r.command(ME, &[mine as u8], Verb::Move, (60, 60), Some(0), false), None, "own man as target");
    assert_eq!((bytes(&r), r.units.get(mine).target_x), before, "nothing changed, not even the flag");
    assert!(!r.modern.plays(ME));
}

/// Ablate the `inside` arm and the units pile on one cell; ablate the offset and they all go to `at`.
#[test]
fn a_group_move_keeps_the_units_relative_positions_unless_it_lands_inside_them() {
    let mut r = duel(&[(Troop::Swordsmen, 4), (Troop::Pikemen, 4)], &[(Troop::Peasants, 1)]);
    let (a, b) = (unit_of_troop(&r, Troop::Swordsmen), unit_of_troop(&r, Troop::Pikemen));
    put(&mut r, a, 10, 20);
    put(&mut r, b, 10, 30);
    let (ay, by) = (r.units.get(a).y as i32, r.units.get(b).y as i32);
    r.command(ME, &[a as u8, b as u8], Verb::Move, (50, 50), None, false);
    let (ta, tb) = (r.units.get(a).target_y as i32, r.units.get(b).target_y as i32);
    assert_eq!(tb - ta, by - ay, "ten cells apart before, ten after");
    assert_eq!((ta + tb) / 2, 50, "centred on the click");
    r.command(ME, &[a as u8, b as u8], Verb::Move, (11, 25), None, false);
    let t = |u: usize| (r.units.get(u).target_x, r.units.get(u).target_y);
    assert_eq!((t(a), t(b)), ((11, 25), (11, 25)), "a click between them gathers them");
}

/// Ablate `self.engages(unit)` in `join_melee`'s caller `engage` and the sword unit stands.
#[test]
fn an_idle_modern_unit_charges_an_enemy_unit_within_nine_cells_and_a_classic_one_stands() {
    for (modern, gap, expect) in [(true, 8, true), (true, 12, false), (false, 8, false)] {
        let mut r = duel(&[(Troop::Swordsmen, 4)], &[(Troop::Peasants, 4)]);
        let (mine, foe) = (unit_of_troop(&r, Troop::Swordsmen), unit_of_troop(&r, Troop::Peasants));
        put(&mut r, mine, 20, 40);
        put(&mut r, foe, 20 + gap, 40);
        if modern {
            announce(&mut r);
        }
        r.step();
        let charging = states(&r, mine).iter().all(|&s| s == State::Chasing);
        assert_eq!(charging, expect, "modern {modern}, {gap} cells");
        assert_eq!(r.units.get(mine).halted, expect);
    }
}

/// Ablate `mode.passive = verb == Verb::Move` and the moved unit charges what it walks past.
#[test]
fn move_and_hold_ignore_a_near_enemy_and_stop_and_attack_move_engage() {
    for (verb, engages) in [(Verb::Move, false), (Verb::Hold, false), (Verb::Stop, true), (Verb::AttackMove, true)] {
        let mut r = duel(&[(Troop::Swordsmen, 4)], &[(Troop::Peasants, 4)]);
        let (mine, foe) = (unit_of_troop(&r, Troop::Swordsmen), unit_of_troop(&r, Troop::Peasants));
        put(&mut r, mine, 20, 40);
        put(&mut r, foe, 25, 40);
        r.command(ME, &[mine as u8], verb, (20, 40), None, false);
        r.step();
        assert_eq!(r.units.get(mine).halted, engages, "{verb:?}");
    }
}

/// Ablate the `engaged && clear` release and the unit chases for ever (`Melee_ChooseChaseTarget`).
#[test]
fn an_engaged_unit_stops_and_reforms_when_no_enemy_is_left_within_the_radius() {
    let mut r = duel(&[(Troop::Swordsmen, 4)], &[(Troop::Peasants, 4)]);
    let (mine, foe) = (unit_of_troop(&r, Troop::Swordsmen), unit_of_troop(&r, Troop::Peasants));
    put(&mut r, mine, 20, 40);
    put(&mut r, foe, 28, 40);
    announce(&mut r);
    r.step();
    assert!(r.modern.mode(mine).engaged && r.units.get(mine).halted);
    for m in r.members(foe) {
        r.sim.figures[r.fighters[m].sim].owner = 0;
    }
    r.run(3);
    assert!(!r.modern.mode(mine).engaged && !r.units.get(mine).halted, "released");
    assert!(states(&r, mine).iter().all(|&s| s != State::Chasing));
}

/// Ablate `f.straffed = true` in `engage` and the archers walk to their range.
#[test]
fn a_modern_bow_unit_shoots_inside_its_range_and_does_not_walk() {
    let mut r = duel(&[(Troop::Archers, 4)], &[(Troop::Peasants, 4)]);
    let (mine, foe) = (unit_of_troop(&r, Troop::Archers), unit_of_troop(&r, Troop::Peasants));
    put(&mut r, mine, 20, 40);
    put(&mut r, foe, 31, 40);
    announce(&mut r);
    let at = |r: &BattleRunner| -> Vec<_> {
        r.members(mine).iter().map(|&m| (r.fighters[m].x, r.fighters[m].y)).collect()
    };
    let start = at(&r);
    r.run(40);
    assert!(states(&r, mine).contains(&State::Shooting));
    assert!(r.members(mine).iter().all(|&m| r.sim.figures[r.fighters[m].sim].straffed), "held where they stand");
    assert_eq!(at(&r), start, "shot from where it stood");
}

/// Ablate the `queue && moves && busy` arm and the second order replaces the first.
#[test]
fn a_shift_order_waits_for_the_men_to_stand_at_the_first_target() {
    let mut r = duel(&[(Troop::Swordsmen, 4)], &[(Troop::Peasants, 1)]);
    let mine = unit_of_troop(&r, Troop::Swordsmen);
    put(&mut r, mine, 20, 40);
    r.command(ME, &[mine as u8], Verb::Move, (26, 40), None, false);
    r.command(ME, &[mine as u8], Verb::Move, (26, 46), None, true);
    assert_eq!(r.modern.mode(mine).queue.len(), 1);
    assert_eq!(r.units.get(mine).target_y, 40, "still bound for the first");
    let mut ticks = 0;
    while !r.modern.mode(mine).queue.is_empty() && ticks < 600 {
        r.step();
        ticks += 1;
    }
    assert!(ticks > 5 && ticks < 600, "taken once the men arrived: {ticks}");
    assert_eq!(r.units.get(mine).target_y, 46);
}

/// Ablate the `f.selected = s` restore in `merge_units` and `selected` stays set.
#[test]
fn merge_makes_one_unit_and_leaves_figure_selected_alone() {
    let mut r = duel(&[(Troop::Swordsmen, 4), (Troop::Pikemen, 4)], &[(Troop::Peasants, 1)]);
    let (a, b) = (unit_of_troop(&r, Troop::Swordsmen), unit_of_troop(&r, Troop::Pikemen));
    let merged = r.command(ME, &[a as u8, b as u8], Verb::Merge, (0, 0), None, false).expect("own units");
    assert!(merged != 0 && merged != a && merged != b);
    r.settle();
    assert_eq!(r.members(merged).len(), 8);
    assert!(r.sim.figures.iter().all(|f| f.selected == 0));
}

/// Classic's frame leaves the block empty, and the block round-trips.
#[test]
fn the_modern_block_is_empty_for_classic_and_round_trips_with_a_queue() {
    let mut r = duel(&[(Troop::Swordsmen, 4)], &[(Troop::Peasants, 1)]);
    r.run(5);
    assert_eq!(r.modern, Modern::default(), "no seat commanded: the sweep touched nothing");
    let mine = unit_of_troop(&r, Troop::Swordsmen);
    r.command(ME, &[mine as u8], Verb::AttackMove, (30, 40), None, false);
    r.command(ME, &[mine as u8], Verb::Move, (30, 50), None, true);
    r.run(3);
    let back = BattleRunner::decode(&mut Reader::new(&bytes(&r))).expect("our own bytes");
    assert_eq!(back.modern, r.modern);
    assert_eq!(bytes(&back), bytes(&r));
}

/// Ablate `!self.engages(unit)` in `join_melee`: a touched modern unit stands as the original's
/// player unit does; a unit that was moved (passive) still does.
#[test]
fn a_touched_modern_unit_joins_the_melee_like_an_ai_unit_and_a_moved_one_does_not() {
    for (verb, joins) in [(Verb::Stop, true), (Verb::Move, false)] {
        let mut r = duel(&[(Troop::Swordsmen, 4)], &[(Troop::Peasants, 4)]);
        let (mine, foe) = (unit_of_troop(&r, Troop::Swordsmen), unit_of_troop(&r, Troop::Peasants));
        put(&mut r, mine, 5, 5);
        put(&mut r, foe, 70, 70);
        r.command(ME, &[mine as u8], verb, (5, 5), None, false);
        r.join_melee(mine);
        assert_eq!(states(&r, mine).iter().all(|&s| s == State::Chasing), joins, "{verb:?}");
        r.join_melee(foe);
        assert!(states(&r, foe).iter().all(|&s| s != State::Chasing), "the foe's seat is Classic");
    }
}

/// Same state, same ops: the same bytes, run to run.
#[test]
fn the_same_commands_give_the_same_state() {
    let run = || {
        let mut r = duel(&[(Troop::Swordsmen, 4), (Troop::Archers, 4)], &[(Troop::Peasants, 4)]);
        let (a, b) = (unit_of_troop(&r, Troop::Swordsmen), unit_of_troop(&r, Troop::Archers));
        announce(&mut r);
        r.command(ME, &[a as u8, b as u8], Verb::AttackMove, (40, 40), None, false);
        r.run(60);
        r.command(ME, &[b as u8], Verb::Hold, (0, 0), None, false);
        r.run(20);
        bytes(&r)
    };
    assert_eq!(run(), run());
}

/// Ablate `modern` in `save.rs`'s flag (`MODERN_BIT` always set): a Classic battle grows a block.
#[test]
fn a_classic_battle_encodes_without_the_modern_block_and_a_commanded_one_with_it() {
    let mut r = duel(&[(Troop::Swordsmen, 4)], &[(Troop::Peasants, 1)]);
    let classic = bytes(&r);
    let mut same = r.clone();
    same.modern = Modern::default();
    assert_eq!(bytes(&same), classic);
    announce(&mut r);
    let flagged = bytes(&r);
    assert!(flagged.len() > classic.len());
    let back = BattleRunner::decode(&mut Reader::new(&classic)).expect("a classic encoding");
    assert_eq!(back.modern, Modern::default());
    assert_eq!(back.size_class, r.size_class, "the flag bit never reaches size_class");
    assert_eq!(BattleRunner::decode(&mut Reader::new(&flagged)).expect("flagged").size_class, r.size_class);
}
