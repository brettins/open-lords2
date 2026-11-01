//! `Modern` in the battle's save and digest: `u16` realms, then the units that differ from the default.

use super::modern::{Modern, Queued, UnitMode, Verb};
use l2_net::{Canonical, CodecError, Decode, Encode, Reader};

impl Encode for Modern {
    fn encode(&self, out: &mut Canonical) {
        out.u16(self.realms);
        let used: Vec<usize> =
            (0..self.modes.len()).filter(|&i| self.modes[i] != UnitMode::default()).collect();
        out.seq(&used, |c, &i| {
            let m = &self.modes[i];
            c.u8(i as u8);
            c.bool(m.passive);
            c.option(m.attack_move.as_ref(), |c, p| {
                c.u8(p.0);
                c.u8(p.1);
            });
            c.bool(m.engaged);
            c.seq(&m.queue, |c, q| {
                c.u8(q.verb.tag());
                c.u8(q.at.0);
                c.u8(q.at.1);
                c.option(q.target.as_ref(), |c, t| c.len32(*t));
            });
        });
    }
}

impl Decode for Modern {
    fn decode(input: &mut Reader<'_>) -> Result<Modern, CodecError> {
        let mut m = Modern { realms: input.u16()?, ..Modern::default() };
        let used = input.seq(|r| {
            let at = r.position();
            let i = r.u8()? as usize;
            let passive = r.bool()?;
            let attack_move = r.option(|r| Ok((r.u8()?, r.u8()?)))?;
            let engaged = r.bool()?;
            let queue = r.seq(|r| {
                let at = r.position();
                let tag = r.u8()?;
                let verb = Verb::from_tag(tag).ok_or(CodecError::BadTag { tag, expected: "verb", at })?;
                Ok(Queued { verb, at: (r.u8()?, r.u8()?), target: r.option(|r| r.len32())? })
            })?;
            Ok((at, i, UnitMode { passive, attack_move, engaged, queue }))
        })?;
        for (at, i, mode) in used {
            let slot = m.modes.get_mut(i).ok_or(CodecError::BadTag { tag: i as u8, expected: "unit", at })?;
            *slot = mode;
        }
        Ok(m)
    }
}
