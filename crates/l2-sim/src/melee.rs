
use crate::figure::{Figure, Role, State};

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

pub fn engage(figs: &mut [Figure], a: usize, b: usize) {
    let (fa, fb) = pair_mut(figs, a, b);
    if !fa.is_alive() || !fb.is_alive() {
        return;
    }
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

pub fn tick(figs: &mut [Figure], idx: usize) {
    let Some(opp) = figs[idx].opponent else { return };
    if idx == opp || opp >= figs.len() {
        return;
    }
    if figs[idx].state != State::Melee || !figs[opp].is_alive() {
        return;
    }

    let (me, other) = pair_mut(figs, idx, opp);

    if me.recovery_counter <= 0 {
        let band = 0; // strength band; single-band until unit strength lands
        let incoming = other.stats().attack(band);
        me.take_hits_from(incoming, opp);
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

    if me.role == Role::Attacking {
        if !me.blow_used {
            let heavy = me.stats().heavy_blow;
            if heavy > 0 {
                other.take_hits_from(heavy, idx);
            }
            me.blow_used = true;
        }
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
