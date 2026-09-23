//! A battle click as a command: the runner changes on its tick, on every peer.
//! `docs/netcode.md` Â§4.

use super::codec::Wire;
use super::*;
use crate::battlefield::BattleOp;
use l2_net::{Canonical, CodecError, Reader};
use l2_sim::runner::Formation;

impl Wire for Vec<usize> {
    fn put(&self, out: &mut Canonical) {
        out.seq(self, |c, i| i.put(c));
    }
    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        input.seq(|r| usize::get(r))
    }
}

const FORMATIONS: [Formation; 3] = [Formation::Keep, Formation::Line, Formation::Column];

impl Wire for BattleOp {
    fn put(&self, out: &mut Canonical) {
        match self {
            BattleOp::Pick { lo, hi, commit } => {
                out.u8(0);
                lo.put(out);
                hi.put(out);
                commit.put(out);
            }
            BattleOp::Drop { figure } => {
                out.u8(1);
                figure.put(out);
            }
            BattleOp::Clear => out.u8(2),
            BattleOp::Recall { figures } => {
                out.u8(3);
                figures.put(out);
            }
            BattleOp::Order { cell, target, woodland } => {
                out.u8(4);
                cell.put(out);
                target.put(out);
                woodland.put(out);
            }
            BattleOp::Formation { unit, formation } => {
                out.u8(5);
                unit.put(out);
                out.u8(FORMATIONS.iter().position(|f| f == formation).unwrap_or(0) as u8);
            }
            BattleOp::Pause => out.u8(6),
            BattleOp::Charge => out.u8(7),
            BattleOp::Sally => out.u8(8),
            BattleOp::Autocalc => out.u8(9),
            BattleOp::SkipOutcome => out.u8(10),
        }
    }

    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let at = input.position();
        let bad = |tag, what| CodecError::BadTag { tag, expected: what, at };
        Ok(match input.u8()? {
            0 => BattleOp::Pick {
                lo: Wire::get(input)?,
                hi: Wire::get(input)?,
                commit: Wire::get(input)?,
            },
            1 => BattleOp::Drop { figure: Wire::get(input)? },
            2 => BattleOp::Clear,
            3 => BattleOp::Recall { figures: Wire::get(input)? },
            4 => BattleOp::Order {
                cell: Wire::get(input)?,
                target: Wire::get(input)?,
                woodland: Wire::get(input)?,
            },
            5 => {
                let unit = Wire::get(input)?;
                let tag = input.u8()?;
                let formation = *FORMATIONS.get(tag as usize).ok_or(bad(tag, "formation"))?;
                BattleOp::Formation { unit, formation }
            }
            6 => BattleOp::Pause,
            7 => BattleOp::Charge,
            8 => BattleOp::Sally,
            9 => BattleOp::Autocalc,
            10 => BattleOp::SkipOutcome,
            tag => return Err(bad(tag, "battle order")),
        })
    }
}

/// The realms on the two sides of the live battle.
pub(crate) fn battle_sides(game: &Game) -> Option<(u8, u8)> {
    let b = game.battle.as_ref()?;
    let owner = |u: usize| game.kingdom.campaign.units.get(u).map_or(b.owner, |u| u.owner);
    Some((owner(b.attacker), owner(b.defender)))
}

/// Seated, only a side's seat orders its men. `[I]` ours: the original has one seat.
impl Order for BattleOrder {
    type Reply = bool;
    fn run(self, game: &mut Game) -> bool {
        if !turn::concerns(game, battle_sides(game)) {
            return false;
        }
        let realm = game.player;
        game.battle.as_deref_mut().is_some_and(|b| b.apply(realm, self.op))
    }
}
