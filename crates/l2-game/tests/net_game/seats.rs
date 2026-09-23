//! `docs/netcode.md` D-3a, D-3b, D-5, D-12: two lobbies, one world.

use l2_game::game::Assets;
use l2_game::net::{self, NetGame};
use l2_game::setup::{SetupOptions, Settings};
use l2_game::Game;
use l2_kingdom::tables::Tables;
use l2_net::{
    decode_all, state_hash, Canonical, Lobby, LobbyError, Loopback, Message, Mismatch, PeerId,
    PlayerSlot, Quirks, Start, Transport,
};

pub const ENGLAND: usize = 0;
const WORLD_SEED: u64 = 0x1025_0417;

pub fn assets() -> Option<Assets> {
    let dir = l2_testkit::install_dir()?;
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    Some(Assets::load(&platform.vfs).expect("assets load"))
}

pub fn settings(quirks: Quirks) -> Settings {
    Settings { ai_lords: 1, ..SetupOptions::new().commit(2, quirks) }
}

/// The host and one client over `Loopback`, each lobby fed only bytes.
fn lobbies(client_quirks: Quirks) -> (Result<Start, LobbyError>, Option<Start>) {
    let tables = Tables::DEFAULT;
    let net = Loopback::with_peers(&[PeerId(0), PeerId(1)]);
    let (mut a, mut b) = (net.endpoint(PeerId(0)), net.endpoint(PeerId(1)));
    let (s0, s1) = (PlayerSlot::new(0), PlayerSlot::new(1));
    let h0 = net::hello(&settings(Quirks::FAITHFUL), &tables, WORLD_SEED, s0);
    let h1 = net::hello(&settings(client_quirks), &tables, WORLD_SEED, s1);
    let host = Lobby::host(h0, "Richard").unwrap().with_shield(4).with_unseated_gold(333);
    let mut host = host;
    let mut client = Lobby::join(h1, "Matilda").unwrap().with_shield(4);
    client.greet().unwrap();
    let mut refused = None;
    for _ in 0..8 {
        while let Some(out) = client.next_outgoing() {
            a_send(&mut b, PeerId(0), &out.message);
        }
        while let Some((peer, bytes)) = a.poll() {
            if let Err(e) = host.receive(peer, decode_all::<Message>(&bytes).unwrap()) {
                refused = Some(e);
            }
        }
        if host.roster().len() == 2 && !host.roster().all_ready() {
            client.set_ready(true).unwrap();
        }
        while let Some(out) = host.next_outgoing() {
            a_send(&mut a, PeerId(1), &out.message);
        }
        while let Some((peer, bytes)) = b.poll() {
            let _ = client.receive(peer, decode_all::<Message>(&bytes).unwrap());
        }
        if host.roster().all_ready() && host.started().is_none() {
            host.start().unwrap();
        }
    }
    let theirs = client.started().cloned();
    match refused {
        Some(e) => (Err(e), theirs),
        None => (host.started().cloned().ok_or(LobbyError::WrongRole), theirs),
    }
}

fn a_send(from: &mut l2_net::Endpoint, to: PeerId, m: &Message) {
    from.send(to, &Canonical::bytes_of(m)).unwrap();
}

/// Each peer builds from its own copy of `Start`, under its own wall clock.
pub fn two_worlds(assets: &mut Assets) -> (Game, Game) {
    let (host, client) = lobbies(Quirks::FAITHFUL);
    let (host, client) = (host.expect("the lobby starts"), client.expect("Start arrived"));
    assert_eq!(host, client);
    let s = settings(Quirks::FAITHFUL);
    let mut build = |start: &Start, me: u8, clock: i64| {
        assets.wall_clock = Some(clock);
        let slot = PlayerSlot::new(me);
        net::new_game(assets, ENGLAND, &s, start, slot, Tables::DEFAULT).expect("England builds")
    };
    (build(&host, 0, 1_000), build(&client, 1, 9_999_999))
}

/// Ablations: `new_game_seed`'s Start arm; `carry_chosen_gold`'s
/// `start.unseated_gold`; `humans: *people` in `scenario::build`.
#[test]
fn two_lobbies_over_loopback_build_one_world() {
    let Some(mut assets) = assets() else { l2_testkit::skip!("no install") };
    let (a, b) = two_worlds(&mut assets);
    let (a, b) = (NetGame::new(a), NetGame::new(b));
    assert_eq!(state_hash(&a), state_hash(&b), "turn 0 differs between the peers");
    assert_eq!((a.game.player, b.game.player), (1, 2), "each peer drives its own seat");
    let realms = &a.game.kingdom.realms;
    assert!(realms[1].is_human && realms[2].is_human && !realms[3].is_human);
    assert_eq!((realms[1].shield_index, realms[2].shield_index), (4, 1), "D-3b");
    assert!(!realms[4].in_play);
    assert_eq!(realms[4].gold, 333, "an unseated realm holds the host's carried gold");
}

/// D-12. Ablation: `quirks` in `net::hello`.
#[test]
fn a_quirks_mismatch_is_refused() {
    let (host, client) = lobbies(Quirks::FIXED);
    let Err(LobbyError::Incompatible(why)) = host else { panic!("started: {host:?}") };
    assert!(matches!(why[..], [Mismatch::Quirks { .. }]), "{why:?}");
    assert!(client.is_none());
}
