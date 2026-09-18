//! `g_battleMen` (`0x00554480`) is save block 7 — 34,992 bytes, `81 × 0x1B0` —
//! and `Save_Write` writes the whole record, path array and animation counters
//! with it. **[V]**

use l2_net::{Canonical, CodecError, Decode, Encode, Reader};

use super::{BattleRunner, Conclusion, End, Fighter};
use crate::ai::{Ai, AiField};
use crate::battle::Battle;
use crate::missile::Missiles;
use crate::pathfind::Pos;
use crate::save::{motion_of, motion_tag, troop_of, troop_tag};
use crate::siege::SiegeState;
use crate::terrain::{Battlefield, DIM};
use crate::unit::Units;

const NO_OCCUPANT: u16 = u16::MAX;

fn end_tag(e: End) -> u8 {
    match e {
        End::Annihilation => 0,
        End::Withdrawal => 1,
        End::BrokeIn => 2,
        End::AssaultFailed => 3,
    }
}

fn end_of(tag: u8, at: usize) -> Result<End, CodecError> {
    Ok(match tag {
        0 => End::Annihilation,
        1 => End::Withdrawal,
        2 => End::BrokeIn,
        3 => End::AssaultFailed,
        _ => return Err(CodecError::BadTag { tag, expected: "battle end", at }),
    })
}

impl Encode for Conclusion {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.winner);
        out.u8(end_tag(self.cause));
    }
}

impl Decode for Conclusion {
    fn decode(input: &mut Reader<'_>) -> Result<Conclusion, CodecError> {
        let winner = input.u8()?;
        let at = input.position();
        Ok(Conclusion { winner, cause: end_of(input.u8()?, at)? })
    }
}

impl Encode for Fighter {
    fn encode(&self, out: &mut Canonical) {
        out.len32(self.sim);
        out.u8(troop_tag(self.troop));
        out.u8(self.side);
        out.u8(self.x);
        out.u8(self.y);
        out.u8(self.target.0);
        out.u8(self.target.1);
        out.u8(self.facing);
        self.progress.encode(out);
        out.u8(motion_tag(self.anim));
        out.u8(self.phase);
        out.u8(self.facing_drawn);
        out.u8(self.fidget);
        out.u8(self.fidget_period);
        out.u8(self.horse_frame);
        out.seq(&self.path, |c, p| p.encode(c));
        out.u8(self.barred);
        out.u8(self.hold);
        out.u16(self.reroutes);
        out.option(self.moat_cell.as_ref(), |c, v| c.u32(*v));
        out.u8(self.moat_load);
        out.u8(self.polar);
        out.u16(self.corpse);
        out.u8(self.delay);
    }
}

impl Decode for Fighter {
    fn decode(input: &mut Reader<'_>) -> Result<Fighter, CodecError> {
        let sim = input.len32()?;
        let at = input.position();
        let troop = troop_of(input.u8()?, at)?;
        let side = input.u8()?;
        let x = input.u8()?;
        let y = input.u8()?;
        let target = (input.u8()?, input.u8()?);
        let facing = input.u8()?;
        let progress = crate::movement::Progress::decode(input)?;
        let at = input.position();
        let anim = motion_of(input.u8()?, at)?;
        Ok(Fighter {
            sim,
            troop,
            side,
            x,
            y,
            target,
            facing,
            progress,
            anim,
            phase: input.u8()?,
            facing_drawn: input.u8()?,
            fidget: input.u8()?,
            fidget_period: input.u8()?,
            horse_frame: input.u8()?,
            path: input.seq(Pos::decode)?,
            barred: input.u8()?,
            hold: input.u8()?,
            reroutes: input.u16()?,
            moat_cell: input.option(|r| r.u32())?,
            moat_load: input.u8()?,
            polar: input.u8()?,
            corpse: input.u16()?,
            delay: input.u8()?,
        })
    }
}

impl Encode for BattleRunner {
    fn encode(&self, out: &mut Canonical) {
        self.sim.encode(out);
        self.field.encode(out);
        out.section("fighters");
        out.seq(&self.fighters, |c, f| f.encode(c));
        out.end_section();
        self.units.encode(out);
        self.ai.encode(out);
        self.ai_field.encode(out);
        self.missiles.encode(out);
        self.siege.encode(out);
        out.section("runner");
        out.seq(&self.occupant, |c, o| c.u16(o.unwrap_or(NO_OCCUPANT)));
        out.u8(self.wall_missile_latch);
        out.option(self.withdrawn.as_ref(), |c, s| c.u8(*s));
        out.u16(self.men_per_figure[0]);
        out.u16(self.men_per_figure[1]);
        out.u8(self.size_class);
        out.bool(self.wood_fire);
        out.u32(self.humans_in_woods);
        out.u32(self.tick);
        out.end_section();
    }
}

impl Decode for BattleRunner {
    fn decode(input: &mut Reader<'_>) -> Result<BattleRunner, CodecError> {
        let sim = Battle::decode(input)?;
        let field = Battlefield::decode(input)?;
        let fighters = input.seq(Fighter::decode)?;
        let units = Units::decode(input)?;
        let ai = Ai::decode(input)?;
        let ai_field = AiField::decode(input)?;
        let missiles = Missiles::decode(input)?;
        let siege = SiegeState::decode(input)?;
        let at = input.position();
        let occupant: Vec<Option<u16>> =
            input.seq(|r| r.u16().map(|v| (v != NO_OCCUPANT).then_some(v)))?;
        if occupant.len() != DIM * DIM {
            return Err(CodecError::LengthOverrun {
                declared: occupant.len(),
                remaining: DIM * DIM,
                at,
            });
        }
        // `sync_positions` and `rebuild_blocked` are the only writers of the two
        // below, and both read only state decoded above.
        let positions = fighters.iter().map(|f| (f.x, f.y)).collect();
        let blocked = field.cells.iter().map(|c| c.impassable()).collect();
        Ok(BattleRunner {
            sim,
            field,
            fighters,
            units,
            ai,
            ai_field,
            positions,
            occupant,
            missiles,
            blocked,
            siege,
            wall_missile_latch: input.u8()?,
            withdrawn: input.option(|r| r.u8())?,
            men_per_figure: [input.u16()?, input.u16()?],
            size_class: input.u8()?,
            wood_fire: input.bool()?,
            humans_in_woods: input.u32()?,
            tick: input.u32()?,
        })
    }
}
