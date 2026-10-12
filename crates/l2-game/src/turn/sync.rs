//! [`TurnProgress`] as bytes, for C256's resync. `encode_progress` hashes it through
//! `Debug`, which nothing decodes; this is the decodable twin (`sync_tests`).

use super::{Question, Stage, Tail, TurnProgress};
use crate::engagement::sync::{bad, get_attack, get_report, get_roster, put_attack, put_report, put_roster};
use crate::engagement::SiegePhase;
use l2_kingdom::conquest::CastleArrival;
use l2_kingdom::siege::{Assault, SiegeRefusal};
use l2_kingdom::units_tick::{Contact, Encounter};
use l2_net::{Canonical, CodecError, Reader};

pub fn encode(out: &mut Canonical, p: &TurnProgress) {
    out.bool(p.idle);
    out.bool(p.opening);
    out.u32(p.ticks);
    out.len32(p.steps);
    out.seq(&p.contacts, put_contact);
    out.seq(&p.pending_battles, put_encounter);
    out.bool(p.granted);
    out.u8(p.stage as u8);
    out.option(p.siege.as_ref(), |o, s| s.encode(o));
    out.option(p.question.as_ref(), put_question);
    out.option(p.pending_assault.as_ref(), put_assault);
    out.option(p.tail.as_ref(), |o, t| {
        o.seq(&t.contacts, put_contact);
        o.option(t.report.as_ref(), crate::save::encode_report);
        o.option(t.battle.as_ref(), put_encounter);
    });
    out.seq(&p.reports, put_report);
    out.option(p.unseen.as_ref(), put_report);
}

pub fn decode(input: &mut Reader<'_>) -> Result<TurnProgress, CodecError> {
    let idle = input.bool()?;
    let opening = input.bool()?;
    let ticks = input.u32()?;
    let steps = input.len32()?;
    let contacts = input.seq(get_contact)?;
    let pending_battles = input.seq(get_encounter)?;
    let granted = input.bool()?;
    let at = input.position();
    let stage = match input.u8()? {
        0 => Stage::Begin,
        1 => Stage::Phase,
        2 => Stage::Tail,
        t => return Err(bad(t, "turn stage", at)),
    };
    let siege = input.option(SiegePhase::decode)?;
    let question = input.option(get_question)?;
    let pending_assault = input.option(get_assault)?;
    let tail = input.option(|r| {
        let contacts = r.seq(get_contact)?;
        let at = r.position();
        let report = r.option(|r| {
            crate::save::decode_report(r)?.map_err(|_| bad(0, "season report", at))
        })?;
        Ok(Tail { contacts, report, battle: r.option(get_encounter)? })
    })?;
    let reports = input.seq(get_report)?;
    let unseen = input.option(get_report)?;
    Ok(TurnProgress {
        idle,
        opening,
        ticks,
        steps,
        contacts,
        pending_battles,
        granted,
        stage,
        siege,
        question,
        pending_assault,
        tail,
        reports,
        unseen,
    })
}

fn put_encounter(out: &mut Canonical, e: &Encounter) {
    out.len32(e.mover);
    out.len32(e.occupant);
    out.u8(e.county);
}

fn get_encounter(input: &mut Reader<'_>) -> Result<Encounter, CodecError> {
    Ok(Encounter { mover: input.len32()?, occupant: input.len32()?, county: input.u8()? })
}

fn put_contact(out: &mut Canonical, c: &Contact) {
    match *c {
        Contact::Battle(e) => {
            out.u8(0);
            put_encounter(out, &e);
        }
        Contact::Castle { unit, county, outcome } => {
            out.u8(1);
            out.len32(unit);
            out.u8(county);
            put_attack(out, &outcome);
        }
        Contact::CastleBuilding { unit, county, arrival } => {
            out.u8(2);
            out.len32(unit);
            out.u8(county);
            match arrival {
                CastleArrival::Garrisoned(u) => {
                    out.u8(0);
                    out.len32(u);
                }
                CastleArrival::GarrisonFull => out.u8(1),
                CastleArrival::Siege(Ok(())) => out.u8(2),
                CastleArrival::Siege(Err(r)) => {
                    out.u8(3);
                    out.u8(r as u8);
                }
                CastleArrival::NotAnArmy => out.u8(4),
            }
        }
        Contact::Merged { mover, into } => {
            out.u8(3);
            out.len32(mover);
            out.len32(into);
        }
        Contact::Blocked { mover, occupant } => {
            out.u8(4);
            out.len32(mover);
            out.len32(occupant);
        }
    }
}

fn get_contact(input: &mut Reader<'_>) -> Result<Contact, CodecError> {
    let at = input.position();
    Ok(match input.u8()? {
        0 => Contact::Battle(get_encounter(input)?),
        1 => Contact::Castle { unit: input.len32()?, county: input.u8()?, outcome: get_attack(input)? },
        2 => {
            let (unit, county) = (input.len32()?, input.u8()?);
            let at = input.position();
            let arrival = match input.u8()? {
                0 => CastleArrival::Garrisoned(input.len32()?),
                1 => CastleArrival::GarrisonFull,
                2 => CastleArrival::Siege(Ok(())),
                3 => {
                    let at = input.position();
                    CastleArrival::Siege(Err(match input.u8()? {
                        0 => SiegeRefusal::NotAnArmy,
                        1 => SiegeRefusal::NoGarrison,
                        2 => SiegeRefusal::CastleRuined,
                        3 => SiegeRefusal::CastleUnderConstruction,
                        4 => SiegeRefusal::AlreadyBesieged,
                        t => return Err(bad(t, "siege refusal", at)),
                    }))
                }
                4 => CastleArrival::NotAnArmy,
                t => return Err(bad(t, "castle arrival", at)),
            };
            Contact::CastleBuilding { unit, county, arrival }
        }
        3 => Contact::Merged { mover: input.len32()?, into: input.len32()? },
        4 => Contact::Blocked { mover: input.len32()?, occupant: input.len32()? },
        t => return Err(bad(t, "contact", at)),
    })
}

fn put_question(out: &mut Canonical, q: &Question) {
    out.len32(q.attacker);
    out.len32(q.defender);
    out.u8(q.county);
    out.bool(q.is_siege);
    out.u8(q.attacker_owner);
    out.u8(q.defender_owner);
    out.i32(q.attacker_men);
    out.i32(q.defender_men);
    put_roster(out, &q.attacker_roster);
    put_roster(out, &q.defender_roster);
    out.u8(q.choice_owner);
    out.option(q.castle_level.as_ref(), |o, l| o.u8(*l));
}

fn get_question(input: &mut Reader<'_>) -> Result<Question, CodecError> {
    Ok(Question {
        attacker: input.len32()?,
        defender: input.len32()?,
        county: input.u8()?,
        is_siege: input.bool()?,
        attacker_owner: input.u8()?,
        defender_owner: input.u8()?,
        attacker_men: input.i32()?,
        defender_men: input.i32()?,
        attacker_roster: get_roster(input)?,
        defender_roster: get_roster(input)?,
        choice_owner: input.u8()?,
        castle_level: input.option(|r| r.u8())?,
    })
}

fn put_assault(out: &mut Canonical, a: &Assault) {
    match *a {
        Assault::Battle { attacker, defender, castle_level } => {
            out.u8(0);
            out.len32(attacker);
            out.len32(defender);
            out.u8(castle_level);
        }
        Assault::NoEngines => out.u8(1),
        Assault::NoSiege => out.u8(2),
    }
}

fn get_assault(input: &mut Reader<'_>) -> Result<Assault, CodecError> {
    let at = input.position();
    Ok(match input.u8()? {
        0 => Assault::Battle { attacker: input.len32()?, defender: input.len32()?, castle_level: input.u8()? },
        1 => Assault::NoEngines,
        2 => Assault::NoSiege,
        t => return Err(bad(t, "assault", at)),
    })
}
