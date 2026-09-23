//! `docs/netcode.md` §4, §6: two peers over real TCP on 127.0.0.1 play seasons
//! and a field battle fought live, every order through `Game::issue`.

use crate::seats::{assets, two_worlds};
use l2_game::battlefield::BattleOp;
use l2_game::command::{self as cmd, BattleOrder};
use l2_game::net::{Live, Stepper};
use l2_game::{turn, Game};
use l2_kingdom::unit::{Unit, UnitKind};
use l2_game::command::GameCommand;
use l2_net::{decode_all, state_hash, PeerId, PlayerSlot, TcpTransport, Transport};
use std::time::Duration;

const SEASONS: u32 = 3;
const MAX_PUMPS: u32 = 40_000;

pub struct Peer {
    pub game: Game,
    pub live: Live,
    realm: u8,
    army: usize,
    county: u8,
    target: (u8, u8),
    step: u32,
    /// An order waits out the input delay before the next is judged.
    wait_until: u32,
    idle: u32,
    patience: u32,
    pub fought: u32,
}

fn connect() -> (TcpTransport, TcpTransport) {
    let mut host = TcpTransport::listen("127.0.0.1:0").expect("a port");
    let addr = host.local_addr().expect("bound");
    let client = TcpTransport::connect_timeout(addr, PeerId(0), Duration::from_secs(3)).unwrap();
    for _ in 0..400 {
        let _ = host.poll();
        if !host.peers().is_empty() {
            return (host, client);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("the host never accepted");
}

fn army(game: &mut Game, realm: u8, at: (u8, u8), men: i32) -> usize {
    let mut u = Unit::new(UnitKind::Army, realm, at.0, at.1);
    u.men = men;
    u.troops[0] = men;
    u.county = game.kingdom.campaign.map.county_at(at.0, at.1);
    u.home_county = u.county;
    u.owner_is_human = true;
    game.kingdom.campaign.units.spawn(u).expect("a free slot")
}

/// Alike on both worlds: realm 1's army at its town, realm 2's one tile off.
fn stage(game: &mut Game) -> [(u8, usize, u8, (u8, u8)); 2] {
    let k = &game.kingdom;
    let county = k.county_ids().find(|&c| k.counties[c].owner == 1).expect("a county") as u8;
    let town = (game.anchor_x[county as usize], game.anchor_y[county as usize]);
    let map = &game.kingdom.campaign.map;
    let beside = [(1i16, 0i16), (0, 1), (-1, 0), (0, -1)]
        .iter()
        .map(|&(dx, dy)| ((town.0 as i16 + dx) as u8, (town.1 as i16 + dy) as u8))
        .find(|&(x, y)| map.county_at(x, y) != 0)
        .expect("land beside the town");
    let mine = army(game, 1, town, 240);
    let theirs = army(game, 2, beside, 160);
    let k = &game.kingdom;
    let home2 = k.county_ids().find(|&c| k.counties[c].owner == 2).expect("a county") as u8;
    [(1, mine, county, beside), (2, theirs, home2, town)]
}

pub fn pair() -> Option<[Peer; 2]> {
    let mut assets = assets()?;
    let (mut ga, mut gb) = two_worlds(&mut assets);
    let seats = stage(&mut ga);
    let _ = stage(&mut gb);
    let (ta, tb) = connect();
    let mut make = |mut game: Game, t: TcpTransport, i: usize, patience: u32| {
        game.net.me = Some(PlayerSlot::new(i as u8));
        let live = Live::new(t, &mut game, 0x1025).expect("a seat");
        let (realm, army, county, target) = seats[i];
        let step = 0;
        let (idle, fought, wait_until) = (0, 0, 0);
        Peer { game, live, realm, army, county, target, step, wait_until, idle, patience, fought }
    };
    Some([make(ga, ta, 0, 25), make(gb, tb, 1, 70)])
}

impl Peer {
    fn tick(&self) -> u32 {
        self.live.session.tick().0
    }

    /// Orders from what this peer sees now, issued as a screen issues them.
    fn play(&mut self) {
        let t = self.live.session.tick().0;
        if t < self.wait_until {
            return;
        }
        let before = self.game.net.outbox.len();
        self.decide(t);
        let queued = self.game.battle.as_deref().map_or(0, |b| b.outbox.len());
        if self.game.net.outbox.len() > before || queued > 0 {
            self.wait_until = t + 3;
        }
    }

    fn decide(&mut self, t: u32) {
        let g = &mut self.game;
        if t >= 2 && self.step == 0 {
            self.step = 1;
            g.issue(cmd::SetTaxRate { county: self.county, rate: 3 + self.realm as i32 });
            if self.realm == 1 {
                g.issue(cmd::MoveUnit { unit: self.army, dest: self.target });
            }
        }
        if g.battle.is_some() {
            self.fought += 1;
            self.fight();
            return;
        }
        let q = turn::pending_question(g);
        if q.is_some_and(|q| q.side_of(self.realm).is_some()) {
            g.issue(cmd::TakeTheField {});
        } else if turn::pending_report(g).is_some() {
            g.issue(cmd::DismissReport {});
        } else if !turn::turn_in_flight(g) && g.kingdom.realms[self.realm as usize].ai_step < l2_kingdom::AI_STEP_DONE {
            self.idle += 1;
            if self.idle >= self.patience {
                self.idle = 0;
                g.issue(cmd::EndTurn {});
            }
        }
    }

    /// Both sides give orders: unpause, pick everyone, march on the foe.
    fn fight(&mut self) {
        let realm = self.realm;
        let b = self.game.battle.as_deref_mut().expect("a battle");
        let foe = (0..b.runner.fighters.len()).find(|&f| {
            let fig = &b.runner.sim.figures[b.runner.fighters[f].sim];
            fig.owner != realm && fig.is_alive()
        });
        let op = match self.fought {
            1 if realm == 1 && b.paused => BattleOp::Pause,
            2 => BattleOp::Pick { lo: (0, 0), hi: (254, 254), commit: true },
            3 if realm == 2 => BattleOp::Charge,
            _ if b.mode == l2_game::battlefield::Mode::Outcome => {
                if b.outcome_ticks <= l2_game::battlefield::OUTCOME_FRAMES {
                    BattleOp::SkipOutcome
                } else {
                    self.game.issue(cmd::FinishBattle {});
                    return;
                }
            }
            n if n % 8 == 4 => match foe {
                Some(f) => {
                    let f = &b.runner.fighters[f];
                    BattleOp::Order { cell: (f.x, f.y), target: None, woodland: false }
                }
                None => return,
            },
            _ => return,
        };
        self.game.issue(BattleOrder { op });
    }
}

pub fn pump(peers: &mut [Peer; 2]) {
    for p in peers.iter_mut() {
        p.play();
        p.live.pump(&mut p.game);
    }
}

pub fn hash(p: &mut Peer) -> u64 {
    let mut m = 0;
    state_hash(&Stepper { game: &mut p.game, malformed: &mut m })
}

/// Ablations: `self.net.outbox.push` in `Game::issue` (applied at once: the
/// peers desync); `b.tick()` in `net::frame` (the battle never ends);
/// `owner` hashed in `encode_battle` (the seats hold different owners); `apply` in
/// `BattleOrder` (the battle never ends).
#[test]
fn two_peers_over_tcp_play_seasons_and_a_live_battle_in_step() {
    let Some(mut peers) = pair() else { l2_testkit::skip!("no install") };
    let start = peers[0].game.turns_played;
    let (mut battles, mut pumps) = (0u32, 0u32);
    let (mut was_live, mut decided) = (false, false);
    while peers[0].game.turns_played < start + SEASONS {
        assert!(pumps < MAX_PUMPS, "stuck at tick {} / {}", peers[0].tick(), peers[1].tick());
        pump(&mut peers);
        for p in &peers {
            assert!(!p.live.session.is_halted(), "{:?}", p.live.session.halt_reason());
        }
        let live = peers[0].game.battle.is_some();
        decided |= peers[0].game.battle.as_ref().is_some_and(|b| b.conclusion.is_some() && !b.autocalc);
        battles += (live && !was_live) as u32;
        was_live = live;
        pumps += 1;
    }
    let (a, b) = (&peers[0].live.session, &peers[1].live.session);
    let (ra, rb) = (a.replay().unwrap(), b.replay().unwrap());
    let n = ra.hashes.len().min(rb.hashes.len());
    assert!(n > 500, "only {n} ticks ran");
    assert_eq!(ra.hashes[..n], rb.hashes[..n], "the peers' hashes differ");
    assert!(battles >= 1, "no battle was fought live");
    assert!(decided, "the battle was not fought to a decision on the field");
    for slot in 0..2 {
        let orders = ra.commands.iter().filter(|(_, c)| c.slot == PlayerSlot::new(slot));
        let fought = orders.filter(|(_, c)| {
            matches!(decode_all::<GameCommand>(&c.payload), Ok(GameCommand::BattleOrder(_)))
        });
        assert!(fought.count() > 3, "seat {slot} gave no battle orders");
    }
    let k = &peers[1].game.kingdom;
    assert_eq!(k.counties[peers[0].county as usize].tax_rate, 4, "seat 0's order crossed");
    assert_eq!(k.counties[peers[1].county as usize].tax_rate, 5, "seat 1's order");
    eprintln!("{n} ticks, {pumps} pumps, {battles} battle(s)");
}
