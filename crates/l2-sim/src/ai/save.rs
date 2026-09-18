//! The AI globals `Save_Write`'s table does hold: `g_rallyWaypoints`
//! (`0x00522D30`, block 91, 96 bytes), `g_battleApproachLane` (`0x0056D6B0`,
//! block 92), `g_battleRallyGroup` (`0x0053F654`, block 93), `g_aiMenMissile`
//! (`0x0056D6AC`, 184), `g_aiMenTotal` (`0x00553F74`, 185), `g_aiCommitCounter`
//! (`0x00553070`, 186), `g_aiRallyX` / `g_aiRallyY` (`0x00522F70`,
//! `0x00522F74`, 187 and 188), `g_aiRallyRequest` (`0x0056D89C`, 189),
//! `g_aiEngagementCount` (`0x00553F28`, 192) and `g_aiMenKnight`
//! (`0x0057C980`, 199). **[V]** — each looked up in the table at `0x004DE960`.

use l2_net::{Canonical, CodecError, Decode, Encode, Pcg32, Reader};

use super::types::{Action, Ai, AiField};
use crate::unit::MAX_UNITS;

fn put_pairs(out: &mut Canonical, ps: &[(i16, i16)]) {
    for (x, y) in ps {
        out.i16(*x);
        out.i16(*y);
    }
}

fn get_pairs(input: &mut Reader<'_>, ps: &mut [(i16, i16)]) -> Result<(), CodecError> {
    for p in ps {
        *p = (input.i16()?, input.i16()?);
    }
    Ok(())
}

impl Encode for AiField {
    fn encode(&self, out: &mut Canonical) {
        out.section("ai-field");
        for side in &self.rally {
            for group in side {
                put_pairs(out, group);
            }
        }
        for side in &self.enemy_end {
            put_pairs(out, side);
        }
        for lane in &self.castle_approach {
            put_pairs(out, lane);
        }
        put_pairs(out, &[self.castle_ref]);
        put_pairs(out, &self.staging);
        for ring in &self.wall_slot {
            put_pairs(out, ring);
        }
        for i in self.castle_objective {
            out.len32(i);
        }
        for i in self.defence_posts {
            out.len32(i);
        }
        out.u8(self.layout);
        out.u8(self.orientation);
        out.bool(self.castle_layout_flag);
        out.u8(self.castle_index);
        out.bytes(&self.surface);
        out.bytes(&self.elevation);
        out.end_section();
    }
}

impl Decode for AiField {
    fn decode(input: &mut Reader<'_>) -> Result<AiField, CodecError> {
        let mut f = AiField::field((0, 0), (0, 0));
        for side in &mut f.rally {
            for group in side {
                get_pairs(input, group)?;
            }
        }
        for side in &mut f.enemy_end {
            get_pairs(input, side)?;
        }
        for lane in &mut f.castle_approach {
            get_pairs(input, lane)?;
        }
        let mut one = [(0i16, 0i16); 1];
        get_pairs(input, &mut one)?;
        f.castle_ref = one[0];
        get_pairs(input, &mut f.staging)?;
        for ring in &mut f.wall_slot {
            get_pairs(input, ring)?;
        }
        for i in &mut f.castle_objective {
            *i = input.len32()?;
        }
        for i in &mut f.defence_posts {
            *i = input.len32()?;
        }
        f.layout = input.u8()?;
        f.orientation = input.u8()?;
        f.castle_layout_flag = input.bool()?;
        f.castle_index = input.u8()?;
        f.surface = input.bytes()?.to_vec();
        f.elevation = input.bytes()?.to_vec();
        Ok(f)
    }
}

impl Encode for Ai {
    fn encode(&self, out: &mut Canonical) {
        out.section("ai");
        let (state, increment) = self.rng.parts();
        out.u64(state);
        out.u64(increment);
        // `[D]`: `g_aiStrengthAdvantage` (`0x004EEA98`) and `g_aiAdvantageTimer`
        // (`0x004D9C70`) are in no block of the table — the original recomputes
        // both within 101 frames (`Battle_UpdateStrengthAdvantage`
        // `0x0047FC01`). Storing them is ours, because a load that waited for
        // the recompute would run up to 101 frames of handlers off a zeroed
        // advantage, which `a_battle_saved_and_reloaded_runs_the_same_hundred_ticks`
        // catches as a split digest.
        out.i32(self.strength_advantage);
        out.i32(self.advantage_timer);
        out.i32(self.commit_counter);
        out.i32(self.engagement_count);
        out.i32(self.engagement_budget);
        out.bool(self.rally_request);
        out.i32(self.rally_x);
        out.i32(self.rally_y);
        out.i32(self.men_total);
        out.i32(self.men_missile);
        out.i32(self.men_knight);
        out.u8(self.season);
        out.len32(self.approach_lane);
        out.len32(self.rally_group);
        out.bool(self.is_siege);
        out.bool(self.multiplayer);
        out.i32(self.approach_score);
        out.i32(self.breach_score);
        out.i32(self.ramparts_breached);
        out.i32(self.attackers_on_wall);
        out.i32(self.siege_engine_count);
        out.bool(self.drawbridge_down);
        out.option(self.withdrawal.as_ref(), |c, s| c.u8(*s));
        out.i32(self.rot_missile3);
        out.i32(self.rot_missile16);
        out.i32(self.rot_missile_objective);
        out.i32(self.tick_missile);
        out.i32(self.rot_foot3);
        out.i32(self.tick_foot);
        out.i32(self.rot_melee6);
        out.i32(self.rot_knight3);
        out.len32(self.wall_slot_cursor);
        out.len32(self.inner_slot_cursor);
        for o in self.defence_post_owner {
            out.u16(o);
        }
        out.end_section();
    }
}

impl Decode for Ai {
    fn decode(input: &mut Reader<'_>) -> Result<Ai, CodecError> {
        let state = input.u64()?;
        let increment = input.u64()?;
        let mut a = Ai::new(0);
        a.rng = Pcg32::from_parts(state, increment);
        a.strength_advantage = input.i32()?;
        a.advantage_timer = input.i32()?;
        a.commit_counter = input.i32()?;
        a.engagement_count = input.i32()?;
        a.engagement_budget = input.i32()?;
        a.rally_request = input.bool()?;
        a.rally_x = input.i32()?;
        a.rally_y = input.i32()?;
        a.men_total = input.i32()?;
        a.men_missile = input.i32()?;
        a.men_knight = input.i32()?;
        a.season = input.u8()?;
        a.approach_lane = input.len32()?;
        a.rally_group = input.len32()?;
        a.is_siege = input.bool()?;
        a.multiplayer = input.bool()?;
        a.approach_score = input.i32()?;
        a.breach_score = input.i32()?;
        a.ramparts_breached = input.i32()?;
        a.attackers_on_wall = input.i32()?;
        a.siege_engine_count = input.i32()?;
        a.drawbridge_down = input.bool()?;
        a.withdrawal = input.option(|r| r.u8())?;
        a.rot_missile3 = input.i32()?;
        a.rot_missile16 = input.i32()?;
        a.rot_missile_objective = input.i32()?;
        a.tick_missile = input.i32()?;
        a.rot_foot3 = input.i32()?;
        a.tick_foot = input.i32()?;
        a.rot_melee6 = input.i32()?;
        a.rot_knight3 = input.i32()?;
        a.wall_slot_cursor = input.len32()?;
        a.inner_slot_cursor = input.len32()?;
        for o in &mut a.defence_post_owner {
            *o = input.u16()?;
        }
        a.last_action = vec![Action::NoThink; MAX_UNITS + 1];
        Ok(a)
    }
}
