//! The missile array (`0x0057A100`) is save block 8 — 7,676 bytes,
//! `101 × 0x4C`. **[V]**

use l2_net::{Canonical, CodecError, Decode, Encode, Reader};

use super::{Missile, Missiles, MAX_MISSILES};

impl Encode for Missile {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.owner);
        out.u8(self.class);
        out.u16(self.shooter);
        out.i16(self.x);
        out.i16(self.y);
        out.i16(self.target_x);
        out.i16(self.target_y);
        out.i16(self.cell_x);
        out.i16(self.cell_y);
        out.i32(self.dx);
        out.i32(self.dy);
        out.i32(self.err);
        out.u8(self.major_axis);
        out.u8(self.dir);
        out.u8(self.launch_elevation);
        out.u8(self.blocked_ticks);
        out.i8(self.sub_steps);
        out.i16(self.ticks_flown);
        out.i16(self.range_ticks);
        out.bool(self.blocked);
        out.i16(self.ttl);
        out.u16(self.power);
        out.u8(self.saved_surface);
        out.i32(self.fire_arrow);
    }
}

impl Decode for Missile {
    fn decode(input: &mut Reader<'_>) -> Result<Missile, CodecError> {
        Ok(Missile {
            owner: input.u8()?,
            class: input.u8()?,
            shooter: input.u16()?,
            x: input.i16()?,
            y: input.i16()?,
            target_x: input.i16()?,
            target_y: input.i16()?,
            cell_x: input.i16()?,
            cell_y: input.i16()?,
            dx: input.i32()?,
            dy: input.i32()?,
            err: input.i32()?,
            major_axis: input.u8()?,
            dir: input.u8()?,
            launch_elevation: input.u8()?,
            blocked_ticks: input.u8()?,
            sub_steps: input.i8()?,
            ticks_flown: input.i16()?,
            range_ticks: input.i16()?,
            blocked: input.bool()?,
            ttl: input.i16()?,
            power: input.u16()?,
            saved_surface: input.u8()?,
            fire_arrow: input.i32()?,
        })
    }
}

impl Encode for Missiles {
    fn encode(&self, out: &mut Canonical) {
        out.section("missiles");
        out.seq(&self.slots, |c, m| m.encode(c));
        out.end_section();
    }
}

impl Decode for Missiles {
    fn decode(input: &mut Reader<'_>) -> Result<Missiles, CodecError> {
        let at = input.position();
        let slots = input.seq(Missile::decode)?;
        if slots.len() != MAX_MISSILES + 1 {
            return Err(CodecError::LengthOverrun {
                declared: slots.len(),
                remaining: MAX_MISSILES + 1,
                at,
            });
        }
        Ok(Missiles { slots })
    }
}
