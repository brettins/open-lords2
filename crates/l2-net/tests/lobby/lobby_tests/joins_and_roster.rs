#![allow(unused_imports)]
use super::*;
use super::compatibility::*;
use super::readiness::*;
use super::codec_and_transport::*;
use super::*;
use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, FrameReader, Hello, Lobby, LobbyError,
    LobbyEvent, Message, Mismatch, PeerId, PlayerSlot, Role, Roster, Session, TcpTransport, Tick,
    Transport, PROTOCOL_VERSION,
};

#[test]
fn a_host_occupies_its_own_slot_and_is_ready_by_definition() {
    let lobby = Lobby::host(hello(0), "Richard").unwrap();
    assert_eq!(lobby.role(), Role::Host);
    assert_eq!(lobby.roster().len(), 1);
    let host = &lobby.roster().players[0];
    assert_eq!(host.slot, PlayerSlot::new(0));
    assert_eq!(host.name, "Richard");
    assert!(host.is_host);
    assert!(host.ready, "a host waiting on itself would never start");
    assert!(!lobby.roster().all_ready(), "one player is not a game");
}

#[test]
fn a_client_that_joins_appears_on_both_sides() {
    let mut t = Table::new("Richard");
    assert_eq!(t.join(1, hello(1), "Matilda"), Ok(LobbyEvent::Joined(PlayerSlot::new(1))));

    assert_eq!(t.host.roster().len(), 2);
    assert_eq!(
        t.host.roster(),
        t.client(0).roster(),
        "the host's view is the only view, so both must match exactly"
    );
    let joiner = t.host.roster().get(PlayerSlot::new(1)).unwrap();
    assert_eq!(joiner.name, "Matilda");
    assert!(!joiner.ready, "a joiner is not ready until it says so");
    assert!(!joiner.is_host);
}

#[test]
fn the_roster_is_slot_ordered_whatever_order_players_arrived_in() {
    let mut t = Table::new("Richard");
    t.join(1, hello(4), "Eleanor").unwrap();
    t.join(2, hello(3), "Geoffrey").unwrap();
    t.join(3, hello(1), "Matilda").unwrap();

    let slots = t.host.roster().slots();
    assert_eq!(
        slots,
        vec![
            PlayerSlot::new(0),
            PlayerSlot::new(1),
            PlayerSlot::new(3),
            PlayerSlot::new(4)
        ],
        "arrival order must not survive into the slot list"
    );
    let mut sorted = slots.clone();
    sorted.sort_by_key(|s| s.index());
    assert_eq!(slots, sorted);

    for i in 0..3 {
        assert_eq!(t.client(i).roster().slots(), slots);
    }
}

#[test]
fn two_players_cannot_share_a_name_or_a_slot() {
    let mut t = Table::new("Richard");
    t.join(1, hello(1), "Matilda").unwrap();

    assert!(matches!(
        t.join(2, hello(2), "Matilda"),
        Err(LobbyError::BadName(_))
    ));
    assert_eq!(t.host.roster().len(), 2);

    assert_eq!(t.join(3, hello(1), "Eleanor"), Ok(LobbyEvent::Joined(PlayerSlot::new(2))));
    assert_eq!(t.host.roster().slots().len(), 3);
}

#[test]
fn a_full_game_refuses_the_next_arrival() {
    let mut t = Table::new("Richard");
    for i in 1..l2_net::MAX_PLAYERS as u8 {
        t.join(i as u32, hello(i), &format!("player{i}")).unwrap();
    }
    assert_eq!(t.host.roster().len(), l2_net::MAX_PLAYERS);
    assert_eq!(t.join(99, hello(0), "latecomer"), Err(LobbyError::Full));
    assert_eq!(t.host.roster().len(), l2_net::MAX_PLAYERS);
}

#[test]
fn claiming_the_hosts_slot_reseats_rather_than_refuses() {
    let mut t = Table::new("Richard");
    assert_eq!(
        t.join(1, hello(0), "Matilda"),
        Ok(LobbyEvent::Joined(PlayerSlot::new(1))),
        "the host holds slot 0, so the joiner takes the next free one"
    );
    assert_eq!(t.host.roster().slots(), vec![PlayerSlot::new(0), PlayerSlot::new(1)]);
    assert_eq!(t.host.roster().get(PlayerSlot::new(0)).unwrap().name, "Richard");
    assert_eq!(t.host.roster().get(PlayerSlot::new(1)).unwrap().name, "Matilda");

    let mut bad = hello(0);
    bad.ruleset_hash = 999;
    assert!(matches!(t.join(2, bad, "Eleanor"), Err(LobbyError::Incompatible(_))));
}

#[test]
fn a_player_who_drops_leaves_the_roster() {
    let mut t = Table::new("Richard");
    t.join(1, hello(1), "Matilda").unwrap();
    t.join(2, hello(2), "Eleanor").unwrap();
    assert_eq!(t.host.roster().len(), 3);

    assert_eq!(t.host.peer_left(PeerId(1)), LobbyEvent::Left(PlayerSlot::new(1)));
    assert_eq!(t.host.roster().slots(), vec![PlayerSlot::new(0), PlayerSlot::new(2)]);
    assert_eq!(
        t.host.peer_left(PeerId(1)),
        LobbyEvent::Nothing,
        "leaving twice is not an event"
    );
}

