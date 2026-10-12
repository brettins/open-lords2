//! C260's harness: seats through the real lobby over TCP on 127.0.0.1, then their
//! sessions, pumped in one thread the way the app loop pumps them (`app_frame.rs`
//! `pump_net`, `Live::pump`, `net::end_session`).

use crate::seats::setup;
use l2_game::command::{self as cmd, Answer};
use l2_game::game::Assets;
use l2_game::net::{self, Driver, Identity, InstallId, Live, Phase, Request};
use l2_game::{turn, Game};
use l2_kingdom::tables::Tables;
use l2_net::Tick;
use std::time::{Duration, Instant};

const PATIENCE: Duration = Duration::from_secs(30);

pub fn install(n: u8) -> InstallId {
    InstallId([0xA0 + n; 16])
}

pub struct Seat {
    pub game: Game,
    pub driver: Option<Driver>,
    pub live: Option<Live>,
    idle: u32,
    patience: u32,
}

impl Seat {
    fn new(name: &str, install: InstallId, first: Request, patience: u32) -> Seat {
        let mut game = Game::new(1);
        game.prefs.install = install;
        game.net.identity = Some(Identity { name: name.into(), shield: 2, setup: setup() });
        game.net.requests.push(first);
        Seat { game, driver: None, live: None, idle: 0, patience }
    }

    pub fn joiner(name: &str, n: u8, port: u16) -> Seat {
        Seat::new(name, install(n), Request::Join { address: format!("127.0.0.1:{port}") }, 40 + 15 * n as u32)
    }

    pub fn live(&self) -> &Live {
        self.live.as_ref().expect("a session")
    }

    pub fn tick(&self) -> u32 {
        self.live.as_ref().map_or(0, |l| l.session.tick().0)
    }

    pub fn hash_at(&self, t: Tick) -> Option<u64> {
        self.live().session.hash_at(t)
    }

    /// One app-loop pass: the lobby until a world, then the session.
    pub fn pump(&mut self, assets: &Assets) {
        if let Some(live) = self.live.as_mut() {
            live.pump(&mut self.game);
            net::end_session(&mut self.live, &mut self.game);
            return;
        }
        let Some(world) = net::service(&mut self.driver, &mut self.game, assets, Tables::DEFAULT) else {
            return;
        };
        self.game = world;
        let (t, relay) = self.driver.take().and_then(|d| d.into_session()).expect("a transport");
        let mut live = match self.game.net.rejoin.take() {
            Some(r) => Live::rejoin(t, &mut self.game, &r).expect("the host's world loads"),
            None => Live::new(t, &mut self.game, 0xC260).expect("a seat"),
        };
        live.keep_relay(relay);
        self.live = Some(live);
    }

    /// Ends its turn when idle and answers its own questions, as a player would.
    pub fn play(&mut self) {
        let Some(live) = self.live.as_ref() else { return };
        if live.session.is_halted() || !self.game.net.outbox.is_empty() {
            return;
        }
        let g = &mut self.game;
        let me = g.player;
        if turn::pending_question(g).is_some_and(|q| q.side_of(me).is_some()) {
            g.issue(cmd::AnswerBattle { answer: Answer::Decline });
        } else if turn::pending_report(g).is_some() {
            g.issue(cmd::DismissReport {});
        } else if !turn::turn_in_flight(g) && g.kingdom.realms[me as usize].ai_step < l2_kingdom::AI_STEP_DONE {
            self.idle += 1;
            if self.idle >= self.patience {
                self.idle = 0;
                g.issue(cmd::EndTurn {});
            }
        }
    }
}

/// The host and its port, once its listener is up.
pub fn host(assets: &Assets) -> (Seat, u16) {
    let mut h = Seat::new("Richard", install(0), Request::Host { addr: crate::loopback::ANY_PORT }, 30);
    let t0 = Instant::now();
    while t0.elapsed() < PATIENCE {
        h.pump(assets);
        if let Some(port) = h.driver.as_ref().and_then(|d| d.local_addr()).map(|a| a.port()) {
            return (h, port);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    panic!("the host never listened: {:?}", h.game.net.phase);
}

/// Pumps every seat until each has a session; joiners ready, the host starts with `n`.
pub fn start(seats: &mut [Seat], assets: &Assets, n: usize) {
    let t0 = Instant::now();
    let mut readied = vec![false; seats.len()];
    let mut started = false;
    while seats.iter().any(|s| s.live.is_none()) {
        assert!(t0.elapsed() < PATIENCE, "no start: {:?}", seats.iter().map(|s| s.game.net.phase.clone()).collect::<Vec<_>>());
        for (i, s) in seats.iter_mut().enumerate() {
            s.pump(assets);
            if let Phase::Failed(why) = &s.game.net.phase {
                panic!("seat {i} failed: {why}");
            }
            let link = &mut s.game.net;
            if s.live.is_none() && link.phase == Phase::Lobby && !link.hosting && !readied[i] && link.me.is_some() {
                link.requests.push(Request::Ready(true));
                readied[i] = true;
            }
        }
        let link = &mut seats[0].game.net;
        let ready = link.roster.len() >= n && link.roster.players.iter().all(|p| p.ready);
        if seats[0].live.is_none() && link.phase == Phase::Lobby && ready && !started {
            link.requests.push(Request::Start);
            started = true;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// A host and `n - 1` joiners in a game that has run a while.
pub fn game(assets: &Assets, n: u8) -> (Vec<Seat>, u16) {
    let (host, port) = host(assets);
    let mut seats = vec![host];
    let names = ["Matilda", "John", "Geoffrey"];
    seats.extend((1..n).map(|i| Seat::joiner(names[i as usize - 1], i, port)));
    start(&mut seats, assets, n as usize);
    run(&mut seats, assets, 80);
    assert!(seats[0].tick() > 30, "the game never ran");
    (seats, port)
}

/// Plays and pumps every seat, `passes` times.
pub fn run(seats: &mut [Seat], assets: &Assets, passes: u32) {
    for _ in 0..passes {
        for s in seats.iter_mut() {
            s.play();
            s.pump(assets);
        }
    }
}

/// Runs until `done` or a few seconds, sleeping a little between passes.
pub fn until(seats: &mut [Seat], assets: &Assets, what: &str, done: impl Fn(&[Seat]) -> bool) {
    let t0 = Instant::now();
    while !done(seats) {
        assert!(t0.elapsed() < PATIENCE, "never: {what}; ticks {:?}", seats.iter().map(Seat::tick).collect::<Vec<_>>());
        run(seats, assets, 1);
        std::thread::sleep(Duration::from_millis(1));
    }
}

/// The last tick every listed seat has run and still holds a hash for.
pub fn common_tick(seats: &[Seat]) -> Tick {
    Tick(seats.iter().map(Seat::tick).min().unwrap_or(0).saturating_sub(2))
}

/// A save's decode needs more than a test thread's 2 MB in a debug build; the app's
/// main thread has 8.
pub fn big_stack(body: impl FnOnce() + Send + 'static) {
    let t = std::thread::Builder::new().stack_size(32 << 20).spawn(body).expect("a thread");
    if let Err(e) = t.join() {
        std::panic::resume_unwind(e);
    }
}

/// Saves, dumps and replays go under a scratch directory, never the player's (thread-local).
pub fn scope(tag: &str) -> l2_game::saves::ScopedDir {
    let dir = std::env::temp_dir().join(format!("l2-c260-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    l2_game::saves::scoped_dir(dir.join("saves"))
}
