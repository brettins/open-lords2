//! `docs/netcode.md` §5: two seats in one world, each ordering its own realm.

use crate::seats::{assets, two_worlds};
use l2_game::command::{self as cmd, Answer, GameCommand};
use l2_game::net::NetGame;
use l2_game::{turn, Game};
use l2_kingdom::unit::{Unit, UnitKind};
use l2_net::{state_hash, Canonical, Command, PlayerSlot, Simulation, Tick};

const SEASONS: u32 = 3;
const MAX_FRAMES: u32 = 8_000;

struct Seat {
    slot: PlayerSlot,
    realm: u8,
    county: u8,
    army: usize,
    target: (u8, u8),
    patience: u32,
    idle: u32,
}

/// World-building done alike on both copies: one army per seat at its town.
fn seat(game: &mut Game, slot: u8, realm: u8, patience: u32) -> Seat {
    let k = &game.kingdom;
    let county = k.county_ids().find(|&c| k.counties[c].owner == realm).expect("a county") as u8;
    let (x, y) = (game.anchor_x[county as usize], game.anchor_y[county as usize]);
    let mut u = Unit::new(UnitKind::Army, realm, x, y);
    u.men = 200;
    u.troops[0] = 200;
    u.county = game.kingdom.campaign.map.county_at(x, y);
    u.home_county = u.county;
    u.owner_is_human = true;
    let army = game.kingdom.campaign.units.spawn(u).expect("a free slot");
    let campaign = &game.kingdom.campaign;
    let routable = |dest: (u8, u8)| {
        let mut units = campaign.units.clone();
        let direct = l2_kingdom::movement::Routing::Direct;
        l2_kingdom::movement::order_move(&campaign.map, &mut units, army, dest, direct).is_some()
    };
    let target = [(2i16, 0i16), (0, 2), (-2, 0), (0, -2), (3, 0), (0, 3), (-3, 0), (0, -3)]
        .iter()
        .map(|&(dx, dy)| ((x as i16 + dx) as u8, (y as i16 + dy) as u8))
        .find(|&t| campaign.map.county_at(t.0, t.1) != 0 && routable(t))
        .expect("a routable tile beside the town");
    let slot = PlayerSlot::new(slot);
    Seat { slot, realm, county, army, target, patience, idle: 0 }
}

impl Seat {
    fn orders(&mut self, game: &Game, t: u32) -> Vec<GameCommand> {
        let mut out: Vec<GameCommand> = Vec::new();
        match t {
            2 => out.push(cmd::SetTaxRate { county: self.county, rate: 3 + self.realm as i32 }.into()),
            3 => out.push(cmd::MoveUnit { unit: self.army, dest: self.target }.into()),
            _ => {}
        }
        let q = turn::pending_question(game);
        if q.is_some_and(|q| q.side_of(self.realm).is_some()) {
            out.push(cmd::AnswerBattle { answer: Answer::Decline }.into());
        } else if turn::pending_report(game).is_some() && self.slot == PlayerSlot::new(0) {
            out.push(cmd::DismissReport {}.into());
        } else if !turn::turn_in_flight(game) && !ended(game, self.realm) {
            self.idle += 1;
            if self.idle >= self.patience {
                self.idle = 0;
                out.push(cmd::EndTurn {}.into());
            }
        }
        out
    }
}

fn ended(game: &Game, realm: u8) -> bool {
    game.kingdom.realms[realm as usize].ai_step >= l2_kingdom::AI_STEP_DONE
}

fn wire(slot: PlayerSlot, orders: &[GameCommand]) -> Vec<Command> {
    let each = orders.iter().enumerate();
    each.map(|(i, o)| Command::new(slot, i as u32, Canonical::bytes_of(o))).collect()
}

/// Ablations: `realm_of`'s seat lookup (seat 1's tax never lands);
/// `every_seat_ready` in `EndTurn` (the turn starts on seat 0's press).
#[test]
fn two_seats_order_and_end_turn_and_stay_identical() {
    let Some(mut assets) = assets() else { l2_testkit::skip!("no install") };
    let (mut ga, mut gb) = two_worlds(&mut assets);
    let mut seats = [seat(&mut ga, 0, 1, 30), seat(&mut ga, 1, 2, 90)];
    let _ = [seat(&mut gb, 0, 1, 30), seat(&mut gb, 1, 2, 90)];
    let (mut a, mut b) = (NetGame::new(ga), NetGame::new(gb));
    let start = a.game.turns_played;
    let (mut t, mut waited) = (0u32, 0u32);
    while a.game.turns_played < start + SEASONS {
        assert!(t < MAX_FRAMES, "{SEASONS} seasons took more than {MAX_FRAMES} frames");
        let mut cmds = Vec::new();
        for s in seats.iter_mut() {
            cmds.extend(wire(s.slot, &s.orders(&a.game, t)));
        }
        let idle = !turn::turn_in_flight(&a.game);
        a.step(Tick(t), &cmds);
        b.step(Tick(t), &cmds);
        assert_eq!(state_hash(&a), state_hash(&b), "the copies diverged at tick {t}");
        if idle && ended(&a.game, 1) && !turn::turn_in_flight(&a.game) {
            waited += 1;
        }
        if t == 3 {
            for s in &seats {
                let dest = a.game.kingdom.campaign.units.get(s.army).and_then(|u| u.dest);
                assert_eq!(dest, Some(s.target), "realm {}'s march was refused", s.realm);
            }
        }
        t += 1;
    }
    let k = &a.game.kingdom;
    assert_eq!(k.counties[seats[0].county as usize].tax_rate, 4, "seat 0's tax");
    assert_eq!(k.counties[seats[1].county as usize].tax_rate, 5, "seat 1's tax");
    assert!(waited >= 60, "the world did not wait for seat 1: {waited} idle ticks");
}

/// The lockstep driver contract on `net::frame`. Ablation: `turn::wind` in it.
#[test]
fn the_world_winds_under_any_screen() {
    let save = l2_testkit::england!();
    let game = l2_game::scenario::from_save(&save, l2_kingdom::tables::Tables::DEFAULT).unwrap();
    let mut sim = NetGame::new(game);
    let start = sim.game.turns_played;
    let end = wire(PlayerSlot::new(0), &[cmd::EndTurn {}.into()]);
    sim.step(Tick(0), &end);
    for t in 1..4_000 {
        if sim.game.turns_played > start {
            return;
        }
        if turn::pending_question(&sim.game).is_some() {
            let no = cmd::AnswerBattle { answer: Answer::Decline }.into();
            sim.step(Tick(t), &wire(PlayerSlot::new(0), &[no]));
        } else if turn::pending_report(&sim.game).is_some() {
            sim.step(Tick(t), &wire(PlayerSlot::new(0), &[cmd::DismissReport {}.into()]));
        } else {
            sim.step(Tick(t), &[]);
        }
    }
    panic!("frame alone never finished the turn");
}
