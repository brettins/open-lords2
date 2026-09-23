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
    if game.battle.is_some() {
        return;
    }
    if turn::wind(game) == turn::Wound::EndTurnDue {
        // The clock ends every seat's turn; an `EndTurn` would end only this peer's.
        let _ = command::time_up(game);
    }
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
    out.option(game.battle.as_deref(), |c, b| b.encode(c));
    out.end_section();
    game.kingdom.encode(out);
}

impl Simulation for NetGame {
    fn step(&mut self, _tick: Tick, commands: &[Command]) {
        let mut ordered = commands.to_vec();
        order_commands(&mut ordered);
        for c in &ordered {
            match decode_all::<GameCommand>(&c.payload) {
                Ok(cmd) => self.game.apply(c.slot, &cmd),
                Err(_) => self.malformed += 1,
            }
        }
        frame(&mut self.game);
    }

    fn encode_state(&self, out: &mut Canonical) {
        encode_state(&self.game, out);
        out.section("net");
        out.u32(self.malformed);
        out.end_section();
    }
}
