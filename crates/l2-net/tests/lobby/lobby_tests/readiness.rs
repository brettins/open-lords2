#![allow(unused_imports)]
use super::*;
use super::joins_and_roster::*;
use super::compatibility::*;
use super::codec_and_transport::*;
use super::*;
use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, FrameReader, Hello, Lobby, LobbyError,
    LobbyEvent, Message, Mismatch, PeerId, PlayerSlot, Role, Roster, Session, TcpTransport, Tick,
    Transport, PROTOCOL_VERSION,
};

#[test]
fn readiness_travels_and_gates_the_start() {
    let mut t = Table::new("Richard");
    t.join(1, hello(1), "Matilda").unwrap();

    assert!(!t.host.roster().all_ready());
    assert!(t.host.start().is_err(), "starting on an unready lobby");

    t.clients[0].1.set_ready(true).unwrap();
    t.pump();

    assert!(t.host.roster().all_ready());
    assert!(
        t.client(0).roster().all_ready(),
        "the client must see its own readiness reflected back by the host"
    );

    let start = t.host.start().expect("everyone is ready");
    assert_eq!(start.seed, SEED);
    assert_eq!(start.roster.slots(), vec![PlayerSlot::new(0), PlayerSlot::new(1)]);

    t.pump();
    assert_eq!(t.client(0).started().map(|s| s.seed), Some(SEED));
    assert_eq!(t.client(0).started().unwrap().roster, start.roster);
}

#[test]
fn un_readying_takes_the_start_away_again() {
    let mut t = Table::new("Richard");
    t.join(1, hello(1), "Matilda").unwrap();
    t.clients[0].1.set_ready(true).unwrap();
    t.pump();
    assert!(t.host.roster().all_ready());

    t.clients[0].1.set_ready(false).unwrap();
    t.pump();
    assert!(!t.host.roster().all_ready());
    assert!(t.host.start().is_err());
}

