//! `docs/netcode.md` §5, §6: two copies of the game fed the same orders stay
//! identical, and a copy that is not is caught on the tick it stopped being.

mod script;
mod lobby_tcp;
mod seats;
mod two_seats;
mod live_tcp;
mod live_desync;
mod live_drop;
mod lobby_ux;
mod lobby_room;
mod loopback;
mod seat_view;
mod live_siege;
mod letter;
mod relay;
mod host_saves;
mod relay_play;
mod halt_box;

use l2_game::net::NetGame;
use l2_kingdom::tables::Tables;
use l2_net::{
    decode_all, state_hash, Advance, Canonical, Command, Config, Loopback, Message, PeerId,
    PlayerSlot, Session, Simulation, Tick, Transport,
};
use script::{stage, wire, Script};

const SEASONS: u32 = 6;
const MAX_FRAMES: u32 = 6_000;

fn copy(save: &l2_formats::save::Save) -> (NetGame, Script) {
    let mut game = l2_game::scenario::from_save(save, Tables::DEFAULT).expect("England loads");
    let script = stage(&mut game);
    (NetGame::new(game), script)
}

#[test]
fn two_copies_fed_the_same_orders_hash_alike_every_tick() {
    let save = l2_testkit::england!();
    let (mut a, mut script) = copy(&save);
    let (mut b, _) = copy(&save);
    let start = a.game.turns_played;
    let foe = a.game.kingdom.campaign.units.get(script.theirs).map(|u| u.owner).unwrap();
    let mut ai_done = false;
    let mut t = 0;
    while a.game.turns_played < start + SEASONS {
        assert!(t < MAX_FRAMES, "{SEASONS} seasons took more than {MAX_FRAMES} frames");
        let cmds = wire(&script.orders(&a.game, t));
        a.step(Tick(t), &cmds);
        b.step(Tick(t), &cmds);
        assert_eq!(state_hash(&a), state_hash(&b), "the copies diverged at tick {t}");
        ai_done |= a.game.kingdom.realms.iter().any(|r| {
            r.in_play && !r.is_human && r.ai_step >= l2_kingdom::AI_STEP_DONE
        });
        t += 1;
    }
    eprintln!("{SEASONS} seasons in {t} frames; asked {}, reports {}", script.asked, script.reports);
    assert_eq!(a.malformed, 0);
    assert!(ai_done, "no AI realm finished a turn");
    let county = &a.game.kingdom.counties[script.county as usize];
    assert_eq!(county.tax_rate, 3, "SetTaxRate never reached the kingdom");
    assert!(script.asked > 0, "the march into realm {foe}'s army raised no battle");
    assert!(script.reports > 0, "the declined battle left no report to dismiss");
    let units = &a.game.kingdom.campaign.units;
    assert!(
        script.theirs_at_report.is_none_or(|men| men < 60),
        "the autocalc left realm {foe}'s army whole"
    );
    assert!(script.ended >= SEASONS, "fewer EndTurns than seasons");
}

/// Tampers with one byte of the world before `at` is stepped.
struct Tampered {
    inner: NetGame,
    at: Option<Tick>,
}

impl Simulation for Tampered {
    fn step(&mut self, tick: Tick, commands: &[Command]) {
        if self.at == Some(tick) {
            self.inner.game.kingdom.counties[1].grain ^= 1;
        }
        self.inner.step(tick, commands);
    }
    fn encode_state(&self, out: &mut Canonical) {
        self.inner.encode_state(out);
    }
}

struct Peer {
    slot: PlayerSlot,
    session: Session,
    sim: Tampered,
    script: Script,
    net: l2_net::Endpoint,
    other: PeerId,
    hashes: Vec<(Tick, u64)>,
    issued: Option<Tick>,
}

fn pair(save: &l2_formats::save::Save, tamper: Option<Tick>) -> [Peer; 2] {
    let net = Loopback::with_peers(&[PeerId(0), PeerId(1)]);
    let slots = [PlayerSlot::new(0), PlayerSlot::new(1)];
    let make = |i: usize, at: Option<Tick>| {
        let (inner, script) = copy(save);
        let sim = Tampered { inner, at };
        let session = Session::new(Config::kingdom(), slots[i], &slots, 0x1025, &sim);
        let (me, other) = (PeerId(i as u32), PeerId(1 - i as u32));
        Peer {
            slot: slots[i],
            session,
            sim,
            script,
            net: net.endpoint(me),
            other,
            hashes: Vec::new(),
            issued: None,
        }
    };
    [make(0, None), make(1, tamper)]
}

fn run(peers: &mut [Peer; 2], target: Tick) {
    for _ in 0..(target.0 * 20) {
        if peers.iter().all(|p| p.session.tick() >= target || p.session.is_halted()) {
            return;
        }
        for p in peers.iter_mut() {
            let at = p.session.execution_tick();
            if p.issued.is_none_or(|last| at > last) {
                p.issued = Some(at);
                if p.slot == PlayerSlot::new(0) {
                    for c in p.script.orders(&p.sim.inner.game, at.0) {
                        p.session.issue(Canonical::bytes_of(&c));
                    }
                }
            }
            while let Some(packet) = p.session.next_packet() {
                let bytes = Canonical::bytes_of(&Message::Tick(packet));
                p.net.send(p.other, &bytes).unwrap();
            }
        }
        for p in peers.iter_mut() {
            while let Some((_, bytes)) = p.net.poll() {
                match decode_all::<Message>(&bytes).unwrap() {
                    Message::Tick(packet) => p.session.receive(packet).unwrap(),
                    other => panic!("unexpected {other:?}"),
                }
            }
            while let Advance::Stepped { tick, hash } = p.session.advance(&mut p.sim) {
                p.hashes.push((tick, hash));
            }
        }
    }
    panic!("stalled at {:?} / {:?}", peers[0].session.tick(), peers[1].session.tick());
}

#[test]
fn the_pair_stays_in_step_through_a_session_over_loopback() {
    let save = l2_testkit::england!();
    let mut peers = pair(&save, None);
    run(&mut peers, Tick(600));
    for p in &peers {
        assert!(!p.session.is_halted(), "{:?}", p.session.halt_reason());
    }
    assert_eq!(peers[0].hashes, peers[1].hashes);
    let k = &peers[1].sim.inner.game.kingdom;
    assert_eq!(k.counties[peers[1].script.county as usize].tax_rate, 3, "slot 0's order arrived");
}

#[test]
fn a_one_byte_tamper_is_a_divergence_at_its_tick() {
    let save = l2_testkit::england!();
    let at = Tick(57);
    let mut peers = pair(&save, Some(at));
    run(&mut peers, Tick(200));
    let found = peers.iter().find_map(|p| p.session.divergence().cloned());
    let d = found.expect("the tamper ran to completion undetected");
    assert_eq!(d.tick, at, "{d}");
    assert_eq!(d.last_agreed, Some(Tick(at.0 - 1)));
}
mod diplomacy;
