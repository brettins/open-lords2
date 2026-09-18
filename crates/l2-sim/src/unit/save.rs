//! `g_battleUnits` (`0x00566520`) is save block 9 — 4,212 bytes, `81 × 0x34`.
//! **[V]**

use l2_net::{Canonical, CodecError, Decode, Encode, Reader};

use super::{BattleUnit, Units, MAX_UNITS};

impl Encode for BattleUnit {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.owner);
        out.bool(self.human);
        out.u8(self.figures);
        out.u8(self.side);
        out.u16(self.first);
        out.u16(self.last);
        out.u8(self.category);
        out.u8(self.orientation);
        out.u16(self.last_attacker);
        out.u8(self.hit_memory);
        out.bool(self.in_melee);
        out.u8(self.order_lock);
        out.u8(self.firing);
        out.bool(self.withdrawing);
        out.bool(self.on_moat);
        out.u8(self.times_hit);
        out.bool(self.reform_gate);
        out.i16(self.reform);
        out.i16(self.orders);
        out.i16(self.think);
        out.i16(self.x);
        out.i16(self.y);
        out.i16(self.target_x);
        out.i16(self.target_y);
        out.i32(self.target_cell);
        out.bool(self.halted);
        out.u8(self.withdrawals);
        out.u8(self.shots_wasted);
    }
}

impl Decode for BattleUnit {
    fn decode(input: &mut Reader<'_>) -> Result<BattleUnit, CodecError> {
        Ok(BattleUnit {
            owner: input.u8()?,
            human: input.bool()?,
            figures: input.u8()?,
            side: input.u8()?,
            first: input.u16()?,
            last: input.u16()?,
            category: input.u8()?,
            orientation: input.u8()?,
            last_attacker: input.u16()?,
            hit_memory: input.u8()?,
            in_melee: input.bool()?,
            order_lock: input.u8()?,
            firing: input.u8()?,
            withdrawing: input.bool()?,
            on_moat: input.bool()?,
            times_hit: input.u8()?,
            reform_gate: input.bool()?,
            reform: input.i16()?,
            orders: input.i16()?,
            think: input.i16()?,
            x: input.i16()?,
            y: input.i16()?,
            target_x: input.i16()?,
            target_y: input.i16()?,
            target_cell: input.i32()?,
            halted: input.bool()?,
            withdrawals: input.u8()?,
            shots_wasted: input.u8()?,
        })
    }
}

impl Encode for Units {
    fn encode(&self, out: &mut Canonical) {
        out.section("units");
        out.seq(&self.slots, |c, u| u.encode(c));
        out.end_section();
    }
}

impl Decode for Units {
    fn decode(input: &mut Reader<'_>) -> Result<Units, CodecError> {
        let at = input.position();
        let slots = input.seq(BattleUnit::decode)?;
        if slots.len() != MAX_UNITS + 1 {
            return Err(CodecError::LengthOverrun {
                declared: slots.len(),
                remaining: MAX_UNITS + 1,
                at,
            });
        }
        Ok(Units { slots })
    }
}
