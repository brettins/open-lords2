
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
    if figs[idx].state != State::Melee {
        return;
    }
    // 0x004831D8 00480000.c:1386-1402: owner 0, state 2 or not state 4 -> state 8 [V]
    let opp = figs[idx].opponent.filter(|&o| o != idx && o < figs.len());
    let Some(opp) = opp.filter(|&o| figs[o].is_alive() && figs[o].state == State::Melee) else {
        figs[idx].state = State::Chasing;
        return;
    };

    let (me, other) = pair_mut(figs, idx, opp);

    if me.recovery_counter <= 0 {
        let band = 0; // strength band; single-band until unit strength lands
        let incoming = other.stats().attack(band);
        me.take_hits_from(incoming, opp);
        // Melee_ResetRecovery 0x00494DF9; no else: only the attacker steps it down, :1832 [V]
        me.recovery_counter += me.stats().recovery as i32;
    }
    // 00490000.c:1801: `if (99 < me.hits)` each tick, so a heavy blow's hits land here [V]
    if me.hits >= me.hits_per_casualty {
        me.take_hits(0);
    }

    // Melee_Tick 0x00494908 00490000.c:1817-1825: only cur dies, `state = 2` [V]
    if !me.is_alive() {
        me.state = State::Dead;
        return;
    }

    if me.role == Role::Attacking {
        if !me.blow_used {
            let heavy = me.stats().heavy_blow;
            if heavy > 0 {
                // 00490000.c:1829: `other.hits += heavyBlow`; other's own tick takes the man [V]
                other.hits = other.hits.saturating_add(heavy);
                other.was_hit = true;
                other.hit_by = Some(idx);
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

}
