#![allow(unused_imports)]
use super::*;
use super::basic::*;
use super::ai_battle::*;
use super::runner_tests::*;
use super::hashing::*;
use l2_net::{
    decode_all, frame, Advance, Canonical, Command, Config, FrameReader, Message, PeerId,
    PlayerSlot, Session, Simulation, TcpTransport, Tick, Transport,
};
use l2_sim::{Battle, Troop, SIDE_A, SIDE_B};
use l2_sim::ai::{self, Ai, AiField};
use l2_sim::unit::{Units, CATEGORY_OF_TROOP};
use l2_sim::{Figure, State};
use l2_sim::runner::{Army, BattleRunner};


pub(super) const OP_ORDER: u8 = 2;

pub(crate) struct RunnerNetBattle {
    pub(crate) runner: BattleRunner,
    pub(super) nudge_at: Option<Tick>,
}

pub(super) fn close_field() -> l2_sim::Battlefield {
    let mut layer = vec![0u8; l2_sim::terrain::CELLS];
    layer[30 * l2_sim::terrain::DIM + 40] = 0x04;
    layer[50 * l2_sim::terrain::DIM + 40] = 0x0F;
    l2_sim::terrain::build(&layer, 1)
}

pub(super) const RUNNER_SEED: u64 = 0x0B47_71E5;

impl RunnerNetBattle {
    pub(crate) fn new() -> Self {
        let ai = [(Troop::Peasants, 12)];
        let player = [(Troop::Pikemen, 4)];
        RunnerNetBattle {
            runner: BattleRunner::deploy_armies(
                close_field(),
                RUNNER_SEED,
                Army { troops: &ai, owner: 1, human: false },
                Army { troops: &player, owner: 2, human: true },
            ),
            nudge_at: None,
        }
    }
}

impl Simulation for RunnerNetBattle {
    fn step(&mut self, tick: Tick, commands: &[Command]) {
        if self.nudge_at == Some(tick) {
            self.runner.fighters[0].x = self.runner.fighters[0].x.wrapping_add(1);
        }
        for cmd in commands {
            if let [OP_ORDER, unit, x, y] = cmd.payload[..] {
                self.runner.order_unit(unit as usize, x, y);
            }
        }
        self.runner.step();
    }

    fn encode_state(&self, out: &mut Canonical) {
        out.section("runner");
        out.u32(self.runner.tick);
        out.len32(self.runner.fighters.len());
        for f in &self.runner.fighters {
            out.u8(f.x);
            out.u8(f.y);
            out.u8(f.target.0);
            out.u8(f.target.1);
            out.u8(f.facing);
            // **A simulation read depends on it**: `simulation.rs:438`
            // `anim == Motion::Dying` gates corpse clearing, `:518` gates
            // clearing `occupant[cell]`. `phase` is NOT_HASHED below.
            out.u8(f.anim as u8);
            out.len32(f.path.len());
            out.u8(f.barred);
            out.u8(f.hold);
            out.u16(f.reroutes);
            out.u32(f.progress.tick_counter);
            out.u32(f.progress.substep);
            out.bool(f.progress.free);
            out.u32(f.moat_cell.unwrap_or(u32::MAX));
            out.u8(f.moat_load);
            out.u8(f.polar);
            // **How long the body has lain here** — `+0x173`, stepped by the
            // corpse state's own tick. Simulation state, not a picture:
            //
            // `BattleRunner::corpse_gone` is the slot's end, and in the
            // original a pot of oil still inside its eighty frames is a spent
            // pot that cannot pour. Two peers that disagreed would clear
            // different bodies. Added at merge — C209's branch left it out and
            // this check named it.
            out.u16(f.corpse);
            // **The frames a swapped man stands still** — `+0x172`, written by
            // `BattleMan_Step`'s `BattleMen_SwapPlaces` arm (`0x0049005F`) as
            // `(other & 1) + 1`. Two peers that disagreed would disagree about
            // his cell a frame later.
            out.u8(f.delay);
        }
        out.end_section();

        out.section("fire");
        out.bool(self.runner.wood_fire);
        out.u8(self.runner.battle_size_class());
        out.end_section();

        out.section("siege");
        let s = &self.runner.siege;
        out.u8(s.is_siege as u8);
        out.u8(s.castle_level);
        out.u32(s.rampart_hits);
        out.u32(s.gate_hits);
        out.u32(s.ramparts_breached);
        out.u8(s.gate_breached as u8);
        out.u8(s.broke_in as u8);
        out.u8(s.drawbridge_down as u8);
        out.u16(s.moat_filled);
        out.u16(s.wall_damage);
        out.u8(s.skirmish as u8);
        let mut fold: u64 = 0xcbf2_9ce4_8422_2325;
        for c in &self.runner.field.cells {
            for b in [c.terrain, c.flags, c.gfx, c.elevation, c.surface] {
                fold = (fold ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        out.u64(fold);
        out.end_section();

        out.section("figures");
        out.len32(self.runner.sim.figures.len());
        for f in &self.runner.sim.figures {
            out.u16(f.men);
            out.u16(f.hits);
            out.u8(f.state as u8);
            out.u16(f.unit);
            out.u8(f.targeted);
            out.option(f.opponent.as_ref(), |c, i| c.len32(*i));
            out.u16(f.reload_counter);
            // **State 17.** Whether a closing archer walks or stands, whether
            // its AI unit's reform is held, and when a dead target is let go
            // are each decided by one of these — and the target itself is who
            // it walks at and shoots.
            out.bool(f.engaged);
            out.bool(f.straffed);
            out.option(f.target.as_ref(), |c, i| c.len32(*i));
        }
        out.end_section();

        // **The arrows.** A missile in flight is simulation state: it carries
        // damage, it is a tick away from killing somebody, and two peers that
        // disagree about where one is disagree about the battle. The original
        // agrees — its own sync digest copies all hundred records, `0x1DB0`
        // bytes, one `Sync_RecordDigest` per slot.
        //
        // **Every slot, not every live one.** A record that has just been freed
        // must hash differently from one, and a walk over
        // only the live ones is the second half of `docs/decisions.md` C39: a
        // whole *record* can go missing from the sweep and no amount of field
        // checking sees it.
        out.section("missiles");
        for slot in 1..=l2_sim::missile::MAX_MISSILES {
            let m = self.runner.missiles.get(slot);
            out.u8(m.owner);
            out.u8(m.class);
            out.u16(m.shooter);
            out.i32(m.x as i32);
            out.i32(m.y as i32);
            out.i32(m.target_x as i32);
            out.i32(m.target_y as i32);
            out.i32(m.cell_x as i32);
            out.i32(m.cell_y as i32);
            out.i32(m.dx);
            out.i32(m.dy);
            out.i32(m.err);
            out.u8(m.major_axis);
            out.u8(m.dir);
            out.u8(m.launch_elevation);
            out.u8(m.blocked_ticks);
            out.i32(m.sub_steps as i32);
            out.i32(m.ticks_flown as i32);
            out.i32(m.range_ticks as i32);
            out.bool(m.blocked);
            out.i32(m.ttl as i32);
            out.u16(m.power);
            out.u8(m.saved_surface);
            out.i32(m.fire_arrow);
        }
        out.end_section();

        out.section("units");
        for i in 1..=l2_sim::MAX_UNITS {
            let u = self.runner.units.get(i);
            out.u8(u.owner);
            out.u8(u.figures);
            out.u8(u.category);
            out.i32(u.orders as i32);
            out.i32(u.think as i32);
            out.i32(u.x as i32);
            out.i32(u.y as i32);
            out.i32(u.target_x as i32);
            out.i32(u.target_y as i32);
            out.i32(u.target_cell);
            out.bool(u.halted);
        }
        out.end_section();

        out.section("ai");
        let (state, increment) = self.runner.ai.rng.parts();
        out.u64(state);
        out.u64(increment);
        out.i32(self.runner.ai.strength_advantage);
        out.i32(self.runner.ai.advantage_timer);
        out.i32(self.runner.ai.approach_score);
        out.i32(self.runner.ai.breach_score);
        out.i32(self.runner.ai.ramparts_breached);
        for post in self.runner.ai_field.defence_posts {
            out.u32(post as u32);
        }
        out.end_section();
    }
}

