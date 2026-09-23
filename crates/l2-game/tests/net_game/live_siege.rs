//! `docs/netcode.md` §4, §5: a siege laid and assaulted over real TCP on
//! 127.0.0.1, seat 0 besieging seat 1's castle, orders from both seats.

use crate::live_tcp::connect;
use crate::seats::{assets, two_worlds};
use l2_game::battlefield::{BattleOp, Mode, OUTCOME_FRAMES};
use l2_game::command::{self as cmd, BattleOrder, GameCommand};
use l2_game::net::{Live, NetGame};
use l2_game::{turn, Game};
use l2_kingdom::map::{castle_plot_tiles, stamp_castle_terrain, MAP_DIM};
use l2_kingdom::unit::{Unit, UnitKind};
use l2_net::{decode_all, PlayerSlot, Simulation, TcpTransport, Tick};

const MAX_PUMPS: u32 = 60_000;

struct Stage {
    county: u8,
    keep: (u8, u8),
    besieger: usize,
}

fn army(game: &mut Game, realm: u8, at: (u8, u8), men: i32) -> usize {
    let mut u = Unit::new(UnitKind::Army, realm, at.0, at.1);
    (u.men, u.troops[0]) = (men, men);
    u.county = game.kingdom.campaign.map.county_at(at.0, at.1);
    u.home_county = u.county;
    u.owner_is_human = true;
    game.kingdom.campaign.units.spawn(u).expect("a free slot")
}

fn routable(game: &Game, unit: usize, to: (u8, u8)) -> bool {
    let c = &game.kingdom.campaign;
    let mut units = c.units.clone();
    let direct = l2_kingdom::movement::Routing::Direct;
    l2_kingdom::movement::order_move(&c.map, &mut units, unit, to, direct).is_some()
}

/// Alike on both worlds: a level-2 castle in realm 2's county, garrisoned by
/// a march run through `net::frame`, and realm 1's army three tiles off.
fn stage(game: Game) -> (Game, Stage) {
    let mut game = game;
    let k = &game.kingdom;
    let county = k.county_ids().find(|&c| k.counties[c].owner == 2).expect("a county") as u8;
    game.kingdom.counties[county as usize].castle_type = 2;
    assert!(stamp_castle_terrain(&mut game.kingdom.campaign.map, county, 2) > 0, "a castle plot");
    let corner = castle_plot_tiles(&game.kingdom.campaign.map, county)[0];
    let keep = ((corner % MAP_DIM) as u8, (corner / MAP_DIM) as u8);
    let near = |d: i16| [(-d, 0i16), (0, d), (d, 0), (0, -d), (-d, d), (-d, -d), (d, -d), (d, d)]
        .map(|(dx, dy)| ((keep.0 as i16 + dx) as u8, (keep.1 as i16 + dy) as u8));
    let land = |g: &Game, t: (u8, u8)| g.kingdom.campaign.map.county_at(t.0, t.1) != 0;
    let spot = near(3).into_iter().find(|&t| land(&game, t)).expect("land by the castle");
    let besieger = army(&mut game, 1, spot, 400);
    assert!(routable(&game, besieger, keep), "the march onto the castle routes");
    if game.kingdom.counties[county as usize].garrison_unit == 0 {
        let door = near(1).into_iter().find(|&t| land(&game, t)).expect("a tile by the keep");
        let garrison = army(&mut game, 2, door, 80);
        let c = &mut game.kingdom.campaign;
        let direct = l2_kingdom::movement::Routing::Direct;
        let _ = l2_kingdom::movement::order_move(&c.map.clone(), &mut c.units, garrison, keep, direct);
        let mut sim = NetGame::new(game);
        for t in 0..600 {
            if sim.game.kingdom.counties[county as usize].garrison_unit == garrison {
                break;
            }
            sim.step(Tick(t), &[]);
        }
        game = sim.game;
        assert_eq!(game.kingdom.counties[county as usize].garrison_unit, garrison, "garrisoned");
    }
    (game, Stage { county, keep, besieger })
}

struct Peer {
    game: Game,
    live: Live,
    realm: u8,
    idle: u32,
    asked: u32,
    fought: u32,
    wait_until: u32,
    moved: bool,
}

impl Peer {
    fn new(game: Game, t: TcpTransport, i: u8) -> Peer {
        let mut game = game;
        game.net.me = Some(PlayerSlot::new(i));
        let live = Live::new(t, &mut game, 0x5E1E).expect("a seat");
        Peer { game, live, realm: i + 1, idle: 0, asked: 0, fought: 0, wait_until: 0, moved: false }
    }

    fn play(&mut self, s: &Stage) {
        let t = self.live.session.tick().0;
        if t < self.wait_until {
            return;
        }
        let before = self.game.net.outbox.len();
        self.decide(s, t);
        let queued = self.game.battle.as_deref().map_or(0, |b| b.outbox.len());
        if self.game.net.outbox.len() > before || queued > 0 {
            self.wait_until = t + 3;
        }
    }

    fn decide(&mut self, s: &Stage, t: u32) {
        let g = &mut self.game;
        if t >= 2 && self.realm == 1 && !self.moved {
            self.moved = true;
            // MoveOrder_ConfirmSiege 0x004A960F `:3968`: the yes of 10/8 issues the move.
            g.issue(cmd::MoveUnit { unit: s.besieger, dest: s.keep });
        }
        if g.battle.is_some() {
            self.fought += 1;
            return self.fight();
        }
        let q = turn::pending_question(g).filter(|q| q.side_of(self.realm).is_some());
        if q.is_some() {
            // Seat 1 presses at once and is refused (`choice_for` 2); seat 0 after a wait.
            self.asked += 1;
            if self.realm == 2 || self.asked > 20 {
                g.issue(cmd::TakeTheField {});
            }
        } else if turn::pending_report(g).is_some() {
            g.issue(cmd::DismissReport {});
        } else if !turn::turn_in_flight(g)
            && g.kingdom.realms[self.realm as usize].ai_step < l2_kingdom::AI_STEP_DONE
        {
            self.idle += 1;
            if self.idle >= 20 + 30 * self.realm as u32 {
                self.idle = 0;
                g.issue(cmd::EndTurn {});
            }
        }
    }

    fn fight(&mut self) {
        let (realm, n) = (self.realm, self.fought);
        let b = self.game.battle.as_deref().expect("a battle");
        let foe = b.runner.fighters.iter().find(|f| {
            let fig = &b.runner.sim.figures[f.sim];
            fig.owner != realm && fig.is_alive()
        });
        let op = match n {
            _ if b.mode == Mode::Outcome && b.outcome_ticks <= OUTCOME_FRAMES => BattleOp::SkipOutcome,
            _ if b.mode == Mode::Outcome || b.autocalc => {
                self.game.issue(cmd::FinishBattle {});
                return;
            }
            1 if realm == 1 && b.paused => BattleOp::Pause,
            2 => BattleOp::Pick { lo: (0, 0), hi: (254, 254), commit: true },
            n if n > 1_200 && realm == 1 => BattleOp::Autocalc,
            n if n % 8 == 4 => match foe {
                Some(f) => BattleOp::Order { cell: (f.x, f.y), target: None, woodland: false },
                None => return,
            },
            _ => return,
        };
        self.game.issue(BattleOrder { op });
    }
}

/// Ablations: `q.choice_for(game, realm) != 1` in `take_the_field` (seat 1's
/// press starts the assault); `self.net.outbox.push` in `Game::issue`.
#[test]
fn a_siege_is_laid_and_assaulted_over_tcp_with_both_seats_in_step() {
    let Some(mut assets) = assets() else { l2_testkit::skip!("no install") };
    let (ga, gb) = two_worlds(&mut assets);
    let ((ga, s), (gb, _)) = (stage(ga), stage(gb));
    let (ta, tb) = connect();
    let mut peers = [Peer::new(ga, ta, 0), Peer::new(gb, tb, 1)];
    let (mut pumps, mut besieged, mut assault, mut after) = (0u32, false, None, 0u32);
    let mut seat0_took = None;
    while after < 2 {
        assert!(pumps < MAX_PUMPS, "stuck at tick {}", peers[0].live.session.tick().0);
        let (pre, asked) = (peers[0].live.session.tick().0, peers[0].asked);
        for p in peers.iter_mut() {
            p.play(&s);
            p.live.pump(&mut p.game);
            assert!(!p.live.session.is_halted(), "{:?}", p.live.session.halt_reason());
        }
        let g = &peers[0].game;
        if asked <= 20 && peers[0].asked > 20 {
            seat0_took = Some(pre);
        }
        besieged |= g.kingdom.campaign.units.get(s.besieger).is_some_and(|u| u.besieging_county == s.county);
        assert!(besieged || g.turns_played < 4, "no siege laid in four seasons");
        assert!(assault.is_some() || g.turns_played < 8, "no assault in eight seasons");
        if assault.is_none() && g.battle.as_ref().is_some_and(|b| b.is_siege()) {
            assault = Some(peers[0].live.session.tick().0);
            let q = turn::pending_question(&peers[1].game).expect("seat 1 is asked too");
            assert_eq!(q.choice_for(&peers[1].game, 2), 2, "the defender's view");
        }
        if assault.is_some() && g.battle.is_none() && peers[1].game.battle.is_none() {
            after += (turn::pending_report(g).is_none() && !turn::turn_in_flight(g)) as u32;
        }
        pumps += 1;
    }
    assert!(besieged, "the march never laid the siege");
    let (at, took) = (assault.expect("no assault was fought live"), seat0_took.expect("seat 1's refused press started the assault"));
    assert!(at > took, "the assault began at {at}, before seat 0 pressed at {took}");
    let (ra, rb) = (peers[0].live.session.replay().unwrap(), peers[1].live.session.replay().unwrap());
    let n = ra.hashes.len().min(rb.hashes.len());
    assert_eq!(ra.hashes[..n], rb.hashes[..n], "the peers' hashes differ");
    for slot in 0..2 {
        let orders = ra.commands.iter().filter(|(_, c)| c.slot == PlayerSlot::new(slot));
        let fought = orders.filter(|(_, c)| {
            matches!(decode_all::<GameCommand>(&c.payload), Ok(GameCommand::BattleOrder(_)))
        });
        assert!(fought.count() > 3, "seat {slot} gave no battle orders");
    }
    eprintln!("{n} ticks, {pumps} pumps, assault at {at}");
}
