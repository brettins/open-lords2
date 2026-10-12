//! A turn's battle values as bytes, for C256's resync: the host's [`super::BattleReport`]s
//! and siege cursor reach a joiner whose own diverged. Never hashed; `encode_progress` is.

use super::{BattleReport, Resolution, Roster, SiegePhase};
use l2_kingdom::battle::{Aftermath, Settlement, Verdict};
use l2_kingdom::conquest::{Attack, Capture, LeftCastle, Refusal};
use l2_net::{Canonical, CodecError, Reader};
use l2_sim::End;

pub(crate) fn bad(tag: u8, expected: &'static str, at: usize) -> CodecError {
    CodecError::BadTag { tag, expected, at }
}

pub(crate) fn put_roster(out: &mut Canonical, r: &Roster) {
    r.iter().for_each(|&n| out.i32(n));
}

pub(crate) fn get_roster(input: &mut Reader<'_>) -> Result<Roster, CodecError> {
    let mut r = Roster::default();
    for n in &mut r {
        *n = input.i32()?;
    }
    Ok(r)
}

impl SiegePhase {
    pub(crate) fn decode(input: &mut Reader<'_>) -> Result<SiegePhase, CodecError> {
        let at = input.len32()?;
        let count = input.u32()?;
        let round = input.len32()?;
        Ok(SiegePhase { cursor: l2_kingdom::siege::SiegeCursor { at, count }, round })
    }
}

pub(crate) fn put_capture(out: &mut Canonical, c: &Capture) {
    for b in [c.new_owner, c.old_owner, c.county, c.held_before, c.peak_before] {
        out.u8(b);
    }
    out.bool(c.governable);
    out.i32(c.penalty);
    out.option(c.garrison.as_ref(), |o, g| match *g {
        LeftCastle::NotAGarrison => o.u8(0),
        LeftCastle::Marched { tile, sortie } => {
            o.u8(1);
            o.u8(tile.0);
            o.u8(tile.1);
            o.option(sortie.as_ref(), |o, s| o.len32(*s));
        }
        LeftCastle::Destroyed => o.u8(2),
    });
}

pub(crate) fn get_capture(input: &mut Reader<'_>) -> Result<Capture, CodecError> {
    let (new_owner, old_owner, county) = (input.u8()?, input.u8()?, input.u8()?);
    let (held_before, peak_before) = (input.u8()?, input.u8()?);
    let governable = input.bool()?;
    let penalty = input.i32()?;
    let garrison = input.option(|r| {
        let at = r.position();
        Ok(match r.u8()? {
            0 => LeftCastle::NotAGarrison,
            1 => LeftCastle::Marched { tile: (r.u8()?, r.u8()?), sortie: r.option(|r| r.len32())? },
            2 => LeftCastle::Destroyed,
            t => return Err(bad(t, "left castle", at)),
        })
    })?;
    Ok(Capture { new_owner, old_owner, county, held_before, peak_before, governable, penalty, garrison })
}

pub(crate) fn put_attack(out: &mut Canonical, a: &Attack) {
    match *a {
        Attack::Refused(r) => {
            out.u8(0);
            out.u8(r as u8);
        }
        Attack::Captured(c) => {
            out.u8(1);
            put_capture(out, &c);
        }
        Attack::Battle { attacker, defender } => {
            out.u8(2);
            out.len32(attacker);
            out.len32(defender);
        }
    }
}

pub(crate) fn get_attack(input: &mut Reader<'_>) -> Result<Attack, CodecError> {
    let at = input.position();
    Ok(match input.u8()? {
        0 => {
            let at = input.position();
            Attack::Refused(match input.u8()? {
                0 => Refusal::NotAnArmy,
                1 => Refusal::Ownerless,
                2 => Refusal::AlreadyYours,
                3 => Refusal::Garrisoned,
                t => return Err(bad(t, "attack refusal", at)),
            })
        }
        1 => Attack::Captured(get_capture(input)?),
        2 => Attack::Battle { attacker: input.len32()?, defender: input.len32()? },
        t => return Err(bad(t, "attack", at)),
    })
}

fn end_tag(e: End) -> u8 {
    match e {
        End::Annihilation => 0,
        End::Withdrawal => 1,
        End::BrokeIn => 2,
        End::AssaultFailed => 3,
    }
}

fn end_of(input: &mut Reader<'_>) -> Result<End, CodecError> {
    let at = input.position();
    Ok(match input.u8()? {
        0 => End::Annihilation,
        1 => End::Withdrawal,
        2 => End::BrokeIn,
        3 => End::AssaultFailed,
        t => return Err(bad(t, "battle end", at)),
    })
}

fn put_pair(out: &mut Canonical, p: (i32, i32)) {
    out.i32(p.0);
    out.i32(p.1);
}

fn get_pair(input: &mut Reader<'_>) -> Result<(i32, i32), CodecError> {
    Ok((input.i32()?, input.i32()?))
}

pub(crate) fn put_report(out: &mut Canonical, r: &BattleReport) {
    out.u8(r.settlement as u8);
    match r.resolution {
        Resolution::Autocalc => out.u8(0),
        Resolution::Fought { ticks, cause } => {
            out.u8(1);
            out.u32(ticks);
            out.u8(end_tag(cause));
        }
        Resolution::Stalled { ticks } => {
            out.u8(2);
            out.u32(ticks);
        }
    }
    let v = &r.verdict;
    out.len32(v.attacker);
    out.len32(v.defender);
    out.bool(v.attacker_won);
    out.option(v.survival_percent.as_ref(), |o, p| o.i32(*p));
    let a = &r.aftermath;
    out.option(a.county_taken_by.as_ref(), |o, c| o.u8(*c));
    out.bool(a.loser_destroyed);
    out.bool(a.loser_siege_lifted);
    out.option(a.offence.as_ref(), |o, p| {
        o.u8(p.0);
        o.u8(p.1);
    });
    out.i32(a.winner_moves_used);
    out.option(a.withdrawal_casualties.as_ref(), |o, n| o.i32(*n));
    out.u8(a.loser_owner);
    a.captures.iter().for_each(|c| out.option(c.as_ref(), put_capture));
    out.i32(r.defenders_returned);
    put_pair(out, r.attacker_men);
    put_pair(out, r.defender_men);
    out.u8(r.county);
    out.bool(r.is_siege);
    out.u8(r.attacker_owner);
    out.u8(r.defender_owner);
    for roster in [&r.attacker_roster.0, &r.attacker_roster.1, &r.defender_roster.0, &r.defender_roster.1] {
        put_roster(out, roster);
    }
    let d = &r.castle_damage;
    out.u16(d.moat_filled);
    out.u16(d.wall_damage);
    out.i32(d.breach_score);
    out.i32(d.approach_score);
    out.u8(d.ramparts_breached);
    out.bool(d.gate_open);
}

pub(crate) fn get_report(input: &mut Reader<'_>) -> Result<BattleReport, CodecError> {
    let at = input.position();
    let settlement = match input.u8()? {
        0 => Settlement::Silently,
        1 => Settlement::Reported,
        2 => Settlement::Prompt,
        t => return Err(bad(t, "settlement", at)),
    };
    let at = input.position();
    let resolution = match input.u8()? {
        0 => Resolution::Autocalc,
        1 => Resolution::Fought { ticks: input.u32()?, cause: end_of(input)? },
        2 => Resolution::Stalled { ticks: input.u32()? },
        t => return Err(bad(t, "resolution", at)),
    };
    let verdict = Verdict {
        attacker: input.len32()?,
        defender: input.len32()?,
        attacker_won: input.bool()?,
        survival_percent: input.option(|r| r.i32())?,
    };
    let aftermath = Aftermath {
        county_taken_by: input.option(|r| r.u8())?,
        loser_destroyed: input.bool()?,
        loser_siege_lifted: input.bool()?,
        offence: input.option(|r| Ok((r.u8()?, r.u8()?)))?,
        winner_moves_used: input.i32()?,
        withdrawal_casualties: input.option(|r| r.i32())?,
        loser_owner: input.u8()?,
        captures: [input.option(get_capture)?, input.option(get_capture)?],
    };
    Ok(BattleReport {
        settlement,
        resolution,
        verdict,
        aftermath,
        defenders_returned: input.i32()?,
        attacker_men: get_pair(input)?,
        defender_men: get_pair(input)?,
        county: input.u8()?,
        is_siege: input.bool()?,
        attacker_owner: input.u8()?,
        defender_owner: input.u8()?,
        attacker_roster: (get_roster(input)?, get_roster(input)?),
        defender_roster: (get_roster(input)?, get_roster(input)?),
        castle_damage: l2_sim::CastleDamage {
            moat_filled: input.u16()?,
            wall_damage: input.u16()?,
            breach_score: input.i32()?,
            approach_score: input.i32()?,
            ramparts_breached: input.u8()?,
            gate_open: input.bool()?,
        },
    })
}
