//! The siege counters `Save_Write` stores one word at a time:
//! `g_wallHitsRampart` (`0x00554034`, block 128), `g_wallHitsGate`
//! (`0x00568DA4`, block 127), `_DAT_00569588` the gate's one-shot (block 129)
//! and `_DAT_0055307C` the breached count (block 130). **[V]**

use l2_net::{Canonical, CodecError, Decode, Encode, Reader};

use super::SiegeState;

impl Encode for SiegeState {
    fn encode(&self, out: &mut Canonical) {
        out.section("siege");
        out.bool(self.is_siege);
        out.u8(self.castle_level);
        out.u32(self.rampart_hits);
        out.u32(self.gate_hits);
        out.u32(self.ramparts_breached);
        out.bool(self.gate_breached);
        out.bool(self.broke_in);
        out.bool(self.drawbridge_down);
        out.u16(self.moat_filled);
        out.u16(self.wall_damage);
        out.end_section();
    }
}

impl Decode for SiegeState {
    fn decode(input: &mut Reader<'_>) -> Result<SiegeState, CodecError> {
        Ok(SiegeState {
            is_siege: input.bool()?,
            castle_level: input.u8()?,
            rampart_hits: input.u32()?,
            gate_hits: input.u32()?,
            ramparts_breached: input.u32()?,
            gate_breached: input.bool()?,
            broke_in: input.bool()?,
            drawbridge_down: input.bool()?,
            moat_filled: input.u16()?,
            wall_damage: input.u16()?,
        })
    }
}
