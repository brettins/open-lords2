#![allow(unused_imports)]
use super::*;
use super::joins_and_roster::*;
use super::compatibility::*;
use super::readiness::*;
use super::*;
use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, FrameReader, Hello, Lobby, LobbyError,
    LobbyEvent, Message, Mismatch, PeerId, PlayerSlot, Role, Roster, Session, TcpTransport, Tick,
    Transport, PROTOCOL_VERSION,
};

/// A roster is the one message whose *order* is part of its meaning,
/// claiming an out-of-order one is refused
#[test]
fn an_out_of_order_roster_is_rejected_by_the_decoder() {
    let good = Roster {
        players: vec![
            l2_net::Player {
                slot: PlayerSlot::new(0),
                name: "a".into(),
                ready: true,
                is_host: true,
            },
            l2_net::Player {
                slot: PlayerSlot::new(2),
                name: "b".into(),
                ready: true,
                is_host: false,
            },
        ],
    };
    let bytes = Canonical::bytes_of(&Message::Roster(good.clone()));
    assert_eq!(decode_all::<Message>(&bytes).unwrap(), Message::Roster(good.clone()));

    let mut backwards = good.clone();
    backwards.players.reverse();
    let bytes = Canonical::bytes_of(&Message::Roster(backwards));
    assert!(
        decode_all::<Message>(&bytes).is_err(),
        "a descending roster would give one machine a different slot list"
    );

    let mut duplicated = good;
    duplicated.players[1].slot = PlayerSlot::new(0);
    let bytes = Canonical::bytes_of(&Message::Roster(duplicated));
    assert!(decode_all::<Message>(&bytes).is_err(), "two players in one slot");
}

#[test]
fn every_lobby_message_round_trips() {
    let roster = Roster {
        players: vec![l2_net::Player {
            slot: PlayerSlot::new(0),
            name: "Richard".into(),
            ready: true,
            is_host: true,
        }],
    };
    let messages = vec![
        Message::Join(l2_net::Join { hello: hello(2), name: "Matilda".into() }),
        Message::Ready(true),
        Message::Ready(false),
        Message::Roster(roster.clone()),
        Message::Start(l2_net::Start { seed: SEED, roster }),
        Message::Refused(vec![
            Mismatch::Protocol { ours: 1, theirs: 2 },
            Mismatch::Engine { ours: "a".into(), theirs: "b".into() },
            Mismatch::Ruleset { ours: 7, theirs: 8 },
            Mismatch::Seed { ours: 9, theirs: 10 },
            Mismatch::SameSlot(PlayerSlot::new(3)),
        ]),
    ];
    for message in messages {
        let bytes = Canonical::bytes_of(&message);
        assert_eq!(decode_all::<Message>(&bytes).unwrap(), message, "round trip");
    }
}

/// The whole point, end to end: a lobby over a real socket produces a `Start`,
/// the `Start` produces two `Session`s, and the two sessions agree on every
/// tick. If the roster order were wrong this is where it would surface.
#[test]
fn a_lobby_over_a_real_socket_starts_a_session_that_agrees() {
    let mut host_net = TcpTransport::listen("127.0.0.1:0").expect("binding a loopback port");
    let addr = host_net.local_addr().expect("a bound address");
    let mut client_net = TcpTransport::connect(addr, PeerId(0)).expect("connecting");

    let mut joined = Vec::new();
    for _ in 0..10_000 {
        while host_net.poll().is_some() {}
        joined = host_net.take_joined();
        if !joined.is_empty() {
            break;
        }
    }
    let client_peer = joined[0];

    let mut host = Lobby::host(hello(0), "Richard").unwrap();
    let mut client = Lobby::join(hello(1), "Matilda").unwrap();
    client.greet().unwrap();
    client.set_ready(true).unwrap();

    let mut host_reader = FrameReader::new();
    let mut client_reader = FrameReader::new();

    // Run the lobby until the host is able to start, then start it.
    let mut started = None;
    for _ in 0..10_000 {
        for out in std::iter::from_fn(|| client.next_outgoing()).collect::<Vec<_>>() {
            let bytes = frame(&Canonical::bytes_of(&out.message)).unwrap();
            client_net.send(PeerId(0), &bytes).unwrap();
        }
        for out in std::iter::from_fn(|| host.next_outgoing()).collect::<Vec<_>>() {
            let bytes = frame(&Canonical::bytes_of(&out.message)).unwrap();
            host_net.send(client_peer, &bytes).unwrap();
        }
        while let Some((_, bytes)) = host_net.poll() {
            host_reader.feed(&bytes);
        }
        while let Some(m) = host_reader.next_message().unwrap() {
            host.receive(client_peer, decode_all::<Message>(&m).unwrap()).unwrap();
        }
        while let Some((_, bytes)) = client_net.poll() {
            client_reader.feed(&bytes);
        }
        while let Some(m) = client_reader.next_message().unwrap() {
            client.receive(PeerId(0), decode_all::<Message>(&m).unwrap()).unwrap();
        }

        if started.is_none() && host.roster().all_ready() {
            started = Some(host.start().unwrap());
        }
        if started.is_some() && client.started().is_some() {
            break;
        }
    }

    let start = started.expect("the host never reached a startable lobby");
    let client_start = client.started().expect("the client never saw the start").clone();
    assert_eq!(start, client_start, "the two sides must start the same game");

    // Hand the lobby's output to the sessions - this is the seam under test.
    let slots = start.roster.slots();
    assert_eq!(slots, vec![PlayerSlot::new(0), PlayerSlot::new(1)]);

    let mut sims = [ToySim::new(start.seed, 8), ToySim::new(start.seed, 8)];
    let mut sessions = [
        Session::new(Config::battle(), slots[0], &slots, start.seed, &sims[0]),
        Session::new(Config::battle(), slots[1], &slots, start.seed, &sims[1]),
    ];
    let mut hashes: [Vec<(Tick, u64)>; 2] = [Vec::new(), Vec::new()];
    let mut nets = [host_net, client_net];
    let mut readers = [host_reader, client_reader];
    let targets = [client_peer, PeerId(0)];

    for _ in 0..20_000 {
        if hashes.iter().all(|h| h.len() >= 40) {
            break;
        }
        for i in 0..2 {
            let at = sessions[i].execution_tick();
            if at.0 == 3 {
                sessions[i].issue(Order::Attack { unit: i as u32 }.payload());
            }
            let mut sealed = Vec::new();
            while let Some(packet) = sessions[i].next_packet() {
                sealed.push(frame(&Canonical::bytes_of(&Message::Tick(packet))).unwrap());
            }
            for bytes in sealed {
                nets[i].send(targets[i], &bytes).unwrap();
            }
        }
        for i in 0..2 {
            while let Some((_, bytes)) = nets[i].poll() {
                readers[i].feed(&bytes);
            }
            while let Some(m) = readers[i].next_message().unwrap() {
                match decode_all::<Message>(&m).unwrap() {
                    Message::Tick(p) => sessions[i].receive(p).unwrap(),
                    other => panic!("unexpected {other:?}"),
                }
            }
            while let Advance::Stepped { tick, hash } = sessions[i].advance(&mut sims[i]) {
                hashes[i].push((tick, hash));
            }
        }
    }

    let common = hashes[0].len().min(hashes[1].len());
    assert!(common >= 40, "only {common} ticks ran");
    assert_eq!(hashes[0][..common], hashes[1][..common], "the sessions diverged");
    assert!(sessions.iter().all(|s| !s.is_halted()));
    assert_eq!(sims[0].units, sims[1].units, "state differs despite equal hashes");
}

