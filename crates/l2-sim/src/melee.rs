//! The melee duel.
//!
//! Reimplemented from `docs/battle.md` §6.1, which reads it out of the
//! original's per-figure tick. The three properties that fall out of it, all of
//! which the printed manual's rankings agree with:
//!
//! * damage per blow is the **attacker's** melee attack for its strength band;
//! * the interval between blows is the **defender's own recovery**, so recovery
//!   is the only melee defence in the game;
//! * the heavy blow lands once per exchange and is large.
//!
//! Everything here is integer arithmetic evaluated in a fixed order. Nothing
//! branches on address, pointer value or hash iteration order, so two runs on
//! different machines produce identical state — the requirement lockstep
//! imposes on the simulation (`docs/netcode.md`).

use crate::figure::{Figure, Role, State};

/// Borrow two distinct figures mutably. Panics only on a caller bug (equal
/// indices), which is a programming error rather than a data condition.
fn pair_mut(figs: &mut [Figure], a: usize, b: usize) -> (&mut Figure, &mut Figure) {
    assert_ne!(a, b, "a figure cannot duel itself");
    if a < b {
        let (l, r) = figs.split_at_mut(b);
        (&mut l[a], &mut r[0])
    } else {
        let (l, r) = figs.split_at_mut(a);
        (&mut r[0], &mut l[b])
    }
}

/// Pair two figures into a duel. One is flagged attacker, the other defender.
pub fn engage(figs: &mut [Figure], a: usize, b: usize) {
    let (fa, fb) = pair_mut(figs, a, b);
    if !fa.is_alive() || !fb.is_alive() {
        return;
    }
    // Siege engines neither seek nor are chosen as melee targets.
    if fa.troop.is_siege() || fb.troop.is_siege() {
        return;
    }
    fa.state = State::Melee;
    fb.state = State::Melee;
    fa.opponent = Some(b);
    fb.opponent = Some(a);
    fa.role = Role::Attacking;
    fb.role = Role::Defending;
    fa.exchange = fa.stats().exchange as i32;
    fb.exchange = fb.stats().exchange as i32;
}

/// Advance one figure's melee by a tick.
///
/// Deliberately mirrors the original's order of operations: a figure takes its
/// damage first, then dies or acts. Reordering these changes who wins a duel
/// decided on the final tick.
pub fn tick(figs: &mut [Figure], idx: usize) {
    let Some(opp) = figs[idx].opponent else { return };
    if idx == opp || opp >= figs.len() {
        return;
    }
    if figs[idx].state != State::Melee || !figs[opp].is_alive() {
        return;
    }

    let (me, other) = pair_mut(figs, idx, opp);

    // 1. Take a blow, if recovered. The damage is the opponent's attack; the
    //    interval is our own recovery.
    if me.recovery_counter <= 0 {
        let band = 0; // strength band; single-band until unit strength lands
        let incoming = other.stats().attack(band);
        me.take_hits(incoming);
        me.recovery_counter += me.stats().recovery as i32;
    } else {
        me.recovery_counter -= 1;
    }

    if !me.is_alive() {
        me.state = State::Dead;
        other.opponent = None;
        other.state = State::Idle;
        return;
    }

    // 2. If we are the one swinging, land the heavy blow and press the opponent.
    if me.role == Role::Attacking {
        if !me.blow_used {
            let heavy = me.stats().heavy_blow;
            if heavy > 0 {
                other.take_hits(heavy);
            }
            // Set and never cleared, exactly as the original appears to behave.
            me.blow_used = true;
        }
        // Pressing the opponent shortens *their* recovery, so an attacker
        // effectively raises the rate at which the defender is struck.
        other.recovery_counter -= 1;

        me.exchange -= 1;
        if me.exchange < 1 {
            me.role = Role::Defending;
            other.role = Role::Attacking;
            me.exchange = me.stats().exchange as i32;
            other.exchange = other.stats().exchange as i32;
        }
    }

    if !other.is_alive() {
        other.state = State::Dead;
        me.opponent = None;
        me.state = State::Idle;
    }
}
