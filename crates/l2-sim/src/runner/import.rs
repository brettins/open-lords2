//! The original's live battle laid over ours: `Save_Write` (`0x004ADE93`)
//! blocks 1 (`g_battlefield` `0x005440E0`, 80x80x8) and 7 (`g_battleMen`
//! `0x00554480`, 81 x `0x1B0`). Offsets: `docs/battle.md` §2, §3. [V]

use super::BattleRunner;
use crate::terrain::DIM;
use crate::State;

pub const FIELD_LEN: usize = DIM * DIM * 8;
pub const MAN_STRIDE: usize = 0x1B0;
pub const MEN_LEN: usize = 81 * MAN_STRIDE;

fn i16_at(b: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([b[at], b[at + 1]])
}

impl BattleRunner {
    /// Cells from block 1, then figure `k + 1` over our fighter `k`, where the
    /// troop agrees. [I] ours raises in `BattleMan_Alloc` order (index 0 is
    /// never allocated), so a fresh raise of the same two armies numbers them
    /// alike; the count returned is how many troop types agreed.
    pub fn overlay_saved(&mut self, field: &[u8], men: &[u8]) -> usize {
        if field.len() >= FIELD_LEN {
            for (c, cell) in self.field.cells.iter_mut().enumerate() {
                let r = &field[c * 8..c * 8 + 8];
                // +0 terrain, +1 flags, +2 flags2, +3 gfx, +4 elevation, +7 surface; +5/+6 are rebuilt.
                (cell.terrain, cell.flags, cell.flags2, cell.gfx) = (r[0], r[1], r[2], r[3]);
                (cell.elevation, cell.surface) = (r[4], r[7]);
            }
        }
        let mut agreed = 0;
        if men.len() >= MEN_LEN {
            for k in 0..self.fighters.len().min(80) {
                let r = &men[(k + 1) * MAN_STRIDE..(k + 2) * MAN_STRIDE];
                let f = &mut self.fighters[k];
                // +0x12 troopType
                if r[0x12] as usize != f.troop.index() {
                    continue;
                }
                agreed += 1;
                let fig = &mut self.sim.figures[f.sim];
                // +0x2C owner: 0 is a freed slot, a man who died.
                if r[0x2C] == 0 {
                    fig.men = 0;
                    fig.owner = 0;
                    fig.state = State::Dead;
                    continue;
                }
                // +0x20/+0x22 map x, y; +0x24/+0x26 tg x, y; +0x18 dirc; +0x19A hits; +0x1A0 men.
                (f.x, f.y) = (i16_at(r, 0x20) as u8, i16_at(r, 0x22) as u8);
                f.target = (i16_at(r, 0x24) as u8, i16_at(r, 0x26) as u8);
                f.facing = r[0x18] & 7;
                fig.hits = i16_at(r, 0x19A).max(0) as u16;
                fig.men = i16_at(r, 0x1A0).max(0) as u16;
            }
        }
        self.occupant.iter_mut().for_each(|o| *o = None);
        for (i, f) in self.fighters.iter().enumerate() {
            if self.sim.figures[f.sim].is_alive() {
                self.occupant[f.y as usize * DIM + f.x as usize] = Some(i as u16);
            }
        }
        self.sync_positions();
        self.rebuild_blocked();
        agreed
    }
}
