//! The kingdom as an [`l2_net::Simulation`] — `docs/netcode.md` §5, D-11.
//! Headless: no screens, no assets.

use crate::command::{self, GameCommand};
use crate::game::Game;
use crate::turn;
use l2_net::{decode_all, order_commands, Canonical, Command, Encode, Simulation, Tick};

pub struct NetGame {
    pub game: Game,
    /// Payloads that did not decode; dropped on every peer alike.
    pub malformed: u32,
}

impl NetGame {
    pub fn new(game: Game) -> NetGame {
        NetGame { game, malformed: 0 }
    }
}

/// `Battle_Frame` (`0x004B99C0`)'s campaign half, in `Machine::update`'s order:
/// the turn clock, then `Turn_Tick` + `Units_Tick` unless `g_battlePhase != 0`.
/// `[I]` the clock's expiry closing a letter is the seat's, not the world's.
///
/// **The lockstep driver contract.** A network game calls this once per tick
/// whatever is on screen. `Machine::wind_turn` and `run_turn_clock` wind only
/// with the campaign on the stack and a fading map pauses them: that is the
/// single-player pacing, per seat, and must never drive a shared world.
/// `net_game` `the_world_winds_under_any_screen` holds it.
pub fn frame(game: &mut Game) {
    let clock = crate::turn_clock::Frame::of(game);
    let _ = game.turn_clock.tick(clock);
    // `Battle_Frame`'s `g_battlePhase == 2` arm: the battle's passes, not the turn's.
    if let Some(b) = game.battle.as_deref_mut() {
        b.tick();
        return;
    }
    let step = match turn::wind(game) {
        // The clock ends every seat's turn; an `EndTurn` would end only this peer's.
        turn::Wound::EndTurnDue => command::time_up(game),
        turn::Wound::Stepped(step) => step,
        _ => return,
    };
    if let turn::TurnStep::Done(outcome) = step {
        game.net.done = Some(outcome);
    }
}

/// [`NetGame::step`] on any holder of the world.
pub fn step_game(game: &mut Game, malformed: &mut u32, commands: &[Command]) {
    let mut ordered = commands.to_vec();
    order_commands(&mut ordered);
    for c in &ordered {
        match decode_all::<GameCommand>(&c.payload) {
            Ok(cmd) => game.apply(c.slot, &cmd),
            Err(_) => *malformed += 1,
        }
    }
    frame(game);
}

pub fn encode_net(game: &Game, malformed: u32, out: &mut Canonical) {
    encode_state(game, out);
    out.section("net");
    out.u32(malformed);
    out.end_section();
}

/// Everything two peers must agree on. Not the seat's view: `selected`,
/// `event_posted`, the letter ring and `campaign` are per seat (C210).
pub fn encode_state(game: &Game, out: &mut Canonical) {
    out.section("game");
    out.u32(game.turns_played);
    for gold in &game.gold_last {
        out.i32(*gold);
    }
    out.bool(game.ai_granted);
    out.i32(game.turn_clock.remaining());
    out.seq(&game.seats, |c, &(slot, realm)| {
        c.u8(slot.index());
        c.u8(realm);
    });
    out.end_section();
    turn::encode_progress(game, out);
    out.section("live-battle");
    out.option(game.battle.as_deref(), |c, b| encode_battle(b, c));
    out.end_section();
    game.kingdom.encode(out);
}

impl Simulation for NetGame {
    fn step(&mut self, _tick: Tick, commands: &[Command]) {
        step_game(&mut self.game, &mut self.malformed, commands);
    }

    fn encode_state(&self, out: &mut Canonical) {
        encode_net(&self.game, self.malformed, out);
    }
}

/// The battle both peers share. Out, per seat: `owner`, `cam`, `drag`, `hover`,
/// `pointer`, `groups`, `current_unit`, `scroll_wait`, `redraw`, `cries`, and
/// `mode` short of the outcome (C210's polarity).
fn encode_battle(b: &crate::battlefield::LiveBattle, out: &mut Canonical) {
    b.runner.encode(out);
    out.section("live-battle");
    out.len32(b.attacker);
    out.len32(b.defender);
    out.u8(b.county);
    out.option(b.castle_level.as_ref(), |c, v| c.u8(*v));
    out.u8(b.choice_owner);
    out.bool(b.paused);
    out.bool(b.mode == crate::battlefield::Mode::Outcome);
    out.bool(b.charged);
    out.bool(b.sallied);
    out.u32(b.outcome_ticks);
    out.bool(b.skirmish);
    out.option(b.conclusion.as_ref(), |c, v| v.encode(c));
    out.bool(b.autocalc);
    out.end_section();
}
