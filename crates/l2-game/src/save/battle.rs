//! What `Save_Write` (`0x004ADE93`) stores of a battle in flight, block by
//! block: the camera `DAT_0055CE70` / `DAT_005678A0` (blocks 61, 62), the nine
//! groups `DAT_00553400` (block 182, 240 = 10 × 0x18), the charge latch
//! `DAT_0055322C` (block 200), the sally `DAT_0052AF9C` (block 126), the
//! skirmish flag `DAT_0057A0F0` (block 201), the outcome counter `DAT_00568470`
//! (block 47) and the two army indices `g_battleArmyA` / `g_battleArmyB`
//! (`0x00568210`, `0x00568218`, blocks 33 and 34). **[V]** — the block table at
//! `0x004DE960`, and blocks 33 and 34 are zero in `battle-before.sav` and set in
//! `noorders-start.sav`.

use l2_net::{Canonical, CodecError, Decode, Encode, Reader};

use crate::battlefield::{Cry, Drag, Hover, LiveBattle, Mode, GROUPS};

fn mode_tag(m: Mode) -> u8 {
    match m {
        Mode::Field => 0,
        Mode::Drag => 1,
        Mode::Outcome => 2,
    }
}

fn mode_of(tag: u8, at: usize) -> Result<Mode, CodecError> {
    Ok(match tag {
        0 => Mode::Field,
        1 => Mode::Drag,
        2 => Mode::Outcome,
        _ => return Err(CodecError::BadTag { tag, expected: "battle mode", at }),
    })
}

fn put_point(out: &mut Canonical, p: (i32, i32)) {
    out.i32(p.0);
    out.i32(p.1);
}

fn get_point(input: &mut Reader<'_>) -> Result<(i32, i32), CodecError> {
    Ok((input.i32()?, input.i32()?))
}

impl Encode for Cry {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.troop);
        out.u8(self.class);
    }
}

impl Decode for Cry {
    fn decode(input: &mut Reader<'_>) -> Result<Cry, CodecError> {
        Ok(Cry { troop: input.u8()?, class: input.u8()? })
    }
}

impl Encode for Drag {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.anchor_cell.0);
        out.u8(self.anchor_cell.1);
        put_point(out, self.anchor_px);
        put_point(out, self.px);
    }
}

impl Decode for Drag {
    fn decode(input: &mut Reader<'_>) -> Result<Drag, CodecError> {
        Ok(Drag {
            anchor_cell: (input.u8()?, input.u8()?),
            anchor_px: get_point(input)?,
            px: get_point(input)?,
        })
    }
}

impl Encode for Hover {
    fn encode(&self, out: &mut Canonical) {
        out.bool(self.on_field);
        out.u8(self.cell.0);
        out.u8(self.cell.1);
        out.u8(self.surface);
        out.bool(self.woodland);
        out.option(self.friendly.as_ref(), |c, i| c.len32(*i));
        out.option(self.friendly_picked.as_ref(), |c, i| c.len32(*i));
        out.option(self.enemy.as_ref(), |c, i| c.len32(*i));
    }
}

impl Decode for Hover {
    fn decode(input: &mut Reader<'_>) -> Result<Hover, CodecError> {
        Ok(Hover {
            on_field: input.bool()?,
            cell: (input.u8()?, input.u8()?),
            surface: input.u8()?,
            woodland: input.bool()?,
            friendly: input.option(|r| r.len32())?,
            friendly_picked: input.option(|r| r.len32())?,
            enemy: input.option(|r| r.len32())?,
        })
    }
}

impl Encode for LiveBattle {
    fn encode(&self, out: &mut Canonical) {
        self.runner.encode(out);
        out.section("live-battle");
        out.len32(self.attacker);
        out.len32(self.defender);
        out.u8(self.county);
        out.option(self.castle_level.as_ref(), |c, v| c.u8(*v));
        out.u8(self.owner);
        out.u8(self.choice_owner);
        out.bool(self.paused);
        put_point(out, self.cam);
        out.u8(mode_tag(self.mode));
        out.option(self.drag.as_ref(), |c, d| d.encode(c));
        self.hover.encode(out);
        put_point(out, self.pointer);
        out.bool(self.pointer_in);
        for g in &self.groups {
            out.option(g.as_ref(), |c, picked| c.seq(picked, |c, i| c.len32(*i)));
        }
        out.u8(self.charged);
        out.bool(self.sallied);
        out.len32(self.current_unit);
        out.u32(self.outcome_ticks);
        out.bool(self.skirmish);
        out.option(self.conclusion.as_ref(), |c, v| v.encode(c));
        out.bool(self.autocalc);
        out.u32(self.scroll_wait);
        out.bool(self.redraw);
        out.seq(&self.cries, |c, cry| cry.encode(c));
        out.end_section();
    }
}

impl Decode for LiveBattle {
    fn decode(input: &mut Reader<'_>) -> Result<LiveBattle, CodecError> {
        let runner = l2_sim::BattleRunner::decode(input)?;
        let attacker = input.len32()?;
        let defender = input.len32()?;
        let county = input.u8()?;
        let castle_level = input.option(|r| r.u8())?;
        let owner = input.u8()?;
        let choice_owner = input.u8()?;
        let paused = input.bool()?;
        let cam = get_point(input)?;
        let at = input.position();
        let mode = mode_of(input.u8()?, at)?;
        let drag = input.option(Drag::decode)?;
        let hover = Hover::decode(input)?;
        let pointer = get_point(input)?;
        let pointer_in = input.bool()?;
        let mut groups: [Option<Vec<usize>>; GROUPS] = Default::default();
        for g in &mut groups {
            *g = input.option(|r| r.seq(|r| r.len32()))?;
        }
        Ok(LiveBattle {
            runner,
            attacker,
            defender,
            county,
            castle_level,
            owner,
            choice_owner,
            paused,
            cam,
            mode,
            drag,
            hover,
            pointer,
            pointer_in,
            groups,
            charged: input.u8()?,
            sallied: input.bool()?,
            current_unit: input.len32()?,
            outcome_ticks: input.u32()?,
            skirmish: input.bool()?,
            conclusion: input.option(l2_sim::Conclusion::decode)?,
            autocalc: input.bool()?,
            scroll_wait: input.u32()?,
            redraw: input.bool()?,
            cries: input.seq(Cry::decode)?,
            deferred: false,
            outbox: Vec::new(),
            awaiting: Default::default(),
        })
    }
}
