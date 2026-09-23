//! `Save_Write` (`0x004ADE93`) writes the battle with the rest of the world:
//! its block table at `0x004DE960` carries `g_battlefield` (`0x005440E0`,
//! 51,200), `g_battleMen` (`0x00554480`, 34,992 = 81 × `0x1B0`), the missiles
//! (`0x0057A100`, 7,676 = 101 × `0x4C`) and `g_battleUnits` (`0x00566520`,
//! 4,212 = 81 × `0x34`) unconditionally, zeroed when no battle is running.
//! **[V]** — those four blocks are all-zero in `battle-before.sav`, saved on the
//! map, and non-zero in `noorders-start.sav` and `fought-start.sav`, saved with
//! the battle screen open.

use l2_net::{Canonical, CodecError, Decode, Encode, Reader};

use crate::battle::Battle;
use crate::cue::Cues;
use crate::figure::{Figure, Motion, Role, State};
use crate::movement::Progress;
use crate::pathfind::Pos;
use crate::troop::{Troop, TroopStats, TroopTable, ALL_TROOPS};

pub(crate) fn troop_tag(t: Troop) -> u8 {
    t.index() as u8
}

pub(crate) fn troop_of(tag: u8, at: usize) -> Result<Troop, CodecError> {
    ALL_TROOPS
        .get(tag as usize)
        .copied()
        .ok_or(CodecError::BadTag { tag, expected: "troop", at })
}

fn state_tag(s: State) -> u8 {
    match s {
        State::Idle => 0,
        State::Melee => 1,
        State::Dead => 2,
        State::Chasing => 3,
        State::Shooting => 4,
        State::FillingMoat => 5,
    }
}

fn state_of(tag: u8, at: usize) -> Result<State, CodecError> {
    Ok(match tag {
        0 => State::Idle,
        1 => State::Melee,
        2 => State::Dead,
        3 => State::Chasing,
        4 => State::Shooting,
        5 => State::FillingMoat,
        _ => return Err(CodecError::BadTag { tag, expected: "figure state", at }),
    })
}

pub(crate) fn motion_tag(m: Motion) -> u8 {
    match m {
        Motion::Idle => 0,
        Motion::Walking => 1,
        Motion::Attacking => 2,
        Motion::Shooting => 3,
        Motion::Dying => 4,
        Motion::Shovelling => 5,
    }
}

pub(crate) fn motion_of(tag: u8, at: usize) -> Result<Motion, CodecError> {
    Ok(match tag {
        0 => Motion::Idle,
        1 => Motion::Walking,
        2 => Motion::Attacking,
        3 => Motion::Shooting,
        4 => Motion::Dying,
        5 => Motion::Shovelling,
        _ => return Err(CodecError::BadTag { tag, expected: "motion", at }),
    })
}

fn role_tag(r: Role) -> u8 {
    match r {
        Role::Attacking => 0,
        Role::Defending => 1,
    }
}

fn role_of(tag: u8, at: usize) -> Result<Role, CodecError> {
    Ok(match tag {
        0 => Role::Attacking,
        1 => Role::Defending,
        _ => return Err(CodecError::BadTag { tag, expected: "figure role", at }),
    })
}

/// `out.option` over a `usize` index, the shape `tests/lockstep.rs` hashes
/// `Figure::opponent` with.
pub(crate) fn put_index(out: &mut Canonical, v: Option<usize>) {
    out.option(v.as_ref(), |c, i| c.len32(*i));
}

pub(crate) fn get_index(input: &mut Reader<'_>) -> Result<Option<usize>, CodecError> {
    input.option(|r| r.len32())
}

impl Encode for Pos {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.x);
        out.u8(self.y);
    }
}

impl Decode for Pos {
    fn decode(input: &mut Reader<'_>) -> Result<Pos, CodecError> {
        let x = input.u8()?;
        let y = input.u8()?;
        Ok(Pos { x, y })
    }
}

impl Encode for Progress {
    fn encode(&self, out: &mut Canonical) {
        out.u32(self.tick_counter);
        out.u32(self.substep);
        out.bool(self.free);
    }
}

impl Decode for Progress {
    fn decode(input: &mut Reader<'_>) -> Result<Progress, CodecError> {
        Ok(Progress {
            tick_counter: input.u32()?,
            substep: input.u32()?,
            free: input.bool()?,
        })
    }
}

impl Encode for TroopStats {
    fn encode(&self, out: &mut Canonical) {
        for a in self.melee_attack {
            out.u16(a);
        }
        out.u16(self.recovery);
        out.u16(self.heavy_blow);
        out.u16(self.armour);
        out.u16(self.exchange);
    }
}

impl Decode for TroopStats {
    fn decode(input: &mut Reader<'_>) -> Result<TroopStats, CodecError> {
        let mut melee_attack = [0u16; 4];
        for a in &mut melee_attack {
            *a = input.u16()?;
        }
        Ok(TroopStats {
            melee_attack,
            recovery: input.u16()?,
            heavy_blow: input.u16()?,
            armour: input.u16()?,
            exchange: input.u16()?,
        })
    }
}

impl Encode for TroopTable {
    fn encode(&self, out: &mut Canonical) {
        for s in &self.stats {
            s.encode(out);
        }
        for h in self.hits_per_casualty {
            out.u16(h);
        }
    }
}

impl Decode for TroopTable {
    fn decode(input: &mut Reader<'_>) -> Result<TroopTable, CodecError> {
        let mut stats = TroopTable::DEFAULT.stats;
        for s in &mut stats {
            *s = TroopStats::decode(input)?;
        }
        let mut hits_per_casualty = [0u16; ALL_TROOPS.len()];
        for h in &mut hits_per_casualty {
            *h = input.u16()?;
        }
        Ok(TroopTable { stats, hits_per_casualty })
    }
}

impl Encode for Figure {
    fn encode(&self, out: &mut Canonical) {
        out.u8(troop_tag(self.troop));
        out.u8(self.side);
        out.u16(self.men);
        out.u16(self.hits);
        out.i32(self.recovery_counter);
        out.u8(state_tag(self.state));
        out.u8(role_tag(self.role));
        put_index(out, self.opponent);
        out.i32(self.exchange);
        out.bool(self.blow_used);
        out.bool(self.owner_is_human);
        out.u8(self.owner);
        out.u16(self.unit);
        out.bool(self.was_hit);
        put_index(out, self.hit_by);
        put_index(out, self.target);
        out.u8(self.targeted);
        self.stats.encode(out);
        out.u16(self.hits_per_casualty);
        out.u16(self.reload_counter);
        out.u8(self.firing);
        out.bool(self.engaged);
        out.bool(self.straffed);
        out.u16(self.full_men);
        out.u8(self.selected);
        out.bool(self.mercenary);
    }
}

impl Decode for Figure {
    fn decode(input: &mut Reader<'_>) -> Result<Figure, CodecError> {
        let at = input.position();
        let troop = troop_of(input.u8()?, at)?;
        let side = input.u8()?;
        let men = input.u16()?;
        let hits = input.u16()?;
        let recovery_counter = input.i32()?;
        let at = input.position();
        let state = state_of(input.u8()?, at)?;
        let at = input.position();
        let role = role_of(input.u8()?, at)?;
        let opponent = get_index(input)?;
        Ok(Figure {
            troop,
            side,
            men,
            hits,
            recovery_counter,
            state,
            role,
            opponent,
            exchange: input.i32()?,
            blow_used: input.bool()?,
            owner_is_human: input.bool()?,
            owner: input.u8()?,
            unit: input.u16()?,
            was_hit: input.bool()?,
            hit_by: get_index(input)?,
            target: get_index(input)?,
            targeted: input.u8()?,
            stats: TroopStats::decode(input)?,
            hits_per_casualty: input.u16()?,
            reload_counter: input.u16()?,
            firing: input.u8()?,
            engaged: input.bool()?,
            straffed: input.bool()?,
            full_men: input.u16()?,
            selected: input.u8()?,
            mercenary: input.bool()?,
        })
    }
}

impl Encode for Battle {
    fn encode(&self, out: &mut Canonical) {
        out.section("battle");
        out.u32(self.tick);
        self.troops.encode(out);
        out.seq(&self.figures, |c, f| f.encode(c));
        self.cues.encode(out);
        out.end_section();
    }
}

impl Decode for Battle {
    fn decode(input: &mut Reader<'_>) -> Result<Battle, CodecError> {
        let tick = input.u32()?;
        let troops = TroopTable::decode(input)?;
        let figures = input.seq(Figure::decode)?;
        let cues = Cues::decode(input)?;
        Ok(Battle { figures, tick, troops, cues })
    }
}
