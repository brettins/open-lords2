//! C257: in a network game the host's save is the game's, and a seat leaves from
//! the menu. Two seats over TCP on 127.0.0.1, pumped as the app pumps them:
//! `Live::pump`, then `net::end_session` between ticks.

use crate::live_tcp::connect;
use crate::seats::{assets, two_worlds};
use l2_game::command::{self as cmd, Answer};
use l2_game::net::{self, InstallId, Live};
use l2_game::screens::leave::Leaving;
use l2_game::{saves, turn, Game};
use l2_kingdom::tables::Tables;
use l2_net::{HaltReason, PlayerSlot};

const HOST: InstallId = InstallId([0xA1; 16]);
const JOINER: InstallId = InstallId([0xB2; 16]);
const MAX_PASSES: u32 = 30_000;

struct Seat {
    game: Game,
    live: Option<Live>,
    idle: u32,
}

fn seats() -> Option<[Seat; 2]> {
    let mut assets = assets()?;
    let (mut ga, mut gb) = two_worlds(&mut assets);
    ga.net.hosting = true;
    (ga.prefs.install, gb.prefs.install) = (HOST, JOINER);
    let (ta, tb) = connect();
    let make = |mut game: Game, t, i: u8| {
        game.net.me = Some(PlayerSlot::new(i));
        let live = Live::new(t, &mut game, 0xC257).expect("a seat");
        net::own_seat(&mut game);
        Seat { game, live: Some(live), idle: 0 }
    };
    Some([make(ga, ta, 0), make(gb, tb, 1)])
}

impl Seat {
    fn play(&mut self) {
        let g = &mut self.game;
        let me = g.player;
        if !g.net.outbox.is_empty() || g.net.halted.is_some() {
            return;
        }
        if turn::pending_question(g).is_some_and(|q| q.side_of(me).is_some()) {
            g.issue(cmd::AnswerBattle { answer: Answer::Decline });
        } else if turn::pending_report(g).is_some() {
            g.issue(cmd::DismissReport {});
        } else if !turn::turn_in_flight(g) && g.kingdom.realms[me as usize].ai_step < l2_kingdom::AI_STEP_DONE {
            self.idle += 1;
            if self.idle >= 30 + 25 * me as u32 {
                self.idle = 0;
                g.issue(cmd::EndTurn {});
            }
        }
    }

    fn pass(&mut self) -> bool {
        self.play();
        if let Some(live) = self.live.as_mut() {
            live.pump(&mut self.game);
        }
        let joiner_held = !self.game.net.hosting && self.game.season_save.is_some();
        net::end_session(&mut self.live, &mut self.game);
        joiner_held
    }
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("l2-c257-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Ablations: `game.net.hosting` in `net::frame`'s season save (the joiner holds
/// one too); `write_season` in `end_session` (nothing on disk); `own_seat` (the save
/// names no install); `alone` in `saves::read_path` (realm 2 waits for nobody).
#[test]
fn only_the_host_writes_the_seasons_autosave_and_it_records_the_hosts_install() {
    let Some(mut seats) = seats() else { l2_testkit::skip!("no install") };
    let dir = scratch("autosave");
    let _scope = saves::scoped_dir(&dir);
    let start = seats[0].game.turns_played;
    let mut passes = 0;
    while seats[0].game.turns_played == start {
        assert!(passes < MAX_PASSES, "no season ended");
        for s in seats.iter_mut() {
            assert!(!s.pass(), "the joiner encoded a season save");
        }
        passes += 1;
    }
    for _ in 0..5 {
        seats.iter_mut().for_each(|s| assert!(!s.pass()));
    }
    let names: Vec<String> = saves::list().into_iter().map(|e| e.name).collect();
    assert_eq!(names, vec![saves::NET_AUTOSAVES[0].to_string()], "one file, the host's");
    let path = saves::path_for(saves::NET_AUTOSAVES[0]).unwrap();
    let saved = saves::read_path(&path, Tables::DEFAULT).expect("the save loads");
    assert_eq!(saved.turns_played, start + 1, "written at the season's end");
    assert_eq!(saved.owners, vec![(1, HOST)]);
    assert!(saved.kingdom.realms[1].is_human && !saved.kingdom.realms[2].is_human, "loaded alone");
    let _ = std::fs::remove_dir_all(&dir);
}

/// *Leave game*: the joiner's session closes and the host's world gives the realm to
/// the computer (C240's halt, `Live::leave`). Ablation: the `game.leaving` check in
/// `end_session` (nothing leaves).
#[test]
fn leave_game_closes_the_joiners_session_and_the_computer_takes_its_realm() {
    let Some(mut seats) = seats() else { l2_testkit::skip!("no install") };
    for _ in 0..60 {
        seats.iter_mut().for_each(|s| {
            s.pass();
        });
    }
    seats[1].game.leaving = Some(Leaving::Leave);
    seats[1].pass();
    assert!(seats[1].live.is_none() && !seats[1].game.net.lockstep, "the joiner is out of the game");
    for _ in 0..2_000 {
        seats[0].pass();
        if seats[0].game.net.halted.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let host = &seats[0];
    assert_eq!(host.live.as_ref().unwrap().session.halt_reason(), Some(&HaltReason::Left));
    assert!(!host.game.kingdom.realms[2].is_human, "the computer plays the leaver's realm");
}

/// *End game for everyone*: the host's session closes and the joiner's stops.
/// Ablation: `live.take()` in `end_session` left as `as_mut` without `leave`.
#[test]
fn end_game_for_everyone_stops_the_joiner() {
    let Some(mut seats) = seats() else { l2_testkit::skip!("no install") };
    for _ in 0..60 {
        seats.iter_mut().for_each(|s| {
            s.pass();
        });
    }
    seats[0].game.leaving = Some(Leaving::EndForEveryone);
    seats[0].pass();
    assert!(seats[0].live.is_none(), "the host's session closed");
    for _ in 0..2_000 {
        seats[1].pass();
        if seats[1].game.net.halted.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(seats[1].game.net.halted.is_some(), "the joiner was not told");
}
