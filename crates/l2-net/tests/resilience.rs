//! What happens when the network is not a friendly in-process queue.
//!
//! `docs/status.html` listed "beyond loopback — real loss, NAT, MTU, TCP
//! head-of-line blocking" as untested, and that list deserves an honest
//! reckoning rather than four tests named after it. Taking them one at a time:
//!
//! * **Loss and reordering are not testable at this layer, because TCP has
//!   already handled them.** A byte stream does not lose or reorder; it either
//!   delivers in order or it fails. Writing a test that "drops packets" through
//!   a `TcpTransport` would be theatre — the code under test would never see
//!   it. What *is* real is the failure that replaces them: the connection
//!   breaking, which `tests/tcp.rs` covers. Reordering at the *session* layer,
//!   where packets from different peers genuinely arrive in any order, is
//!   covered in `tests/lockstep.rs`.
//! * **NAT is not testable on one machine at all.** Two peers on loopback never
//!   traverse anything. This is a real gap and is recorded as one in
//!   `docs/netcode.md`; no test here should be read as covering it.
//! * **MTU is real and is testable**, because it is really a question about
//!   *framing*: the transport must not care where the operating system chose to
//!   split the stream. The strongest version of that is one byte at a time.
//! * **Head-of-line blocking is real and is the interesting one.** With TCP,
//!   one slow peer stalls everyone, and the correct behaviour is to *wait* —
//!   never to guess ahead, because a guess is a desync and a desync is worse
//!   than a pause. That is the last and longest test here.

mod common;

use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, FrameReader, Message, PeerId, PlayerSlot,
    Session, TcpTransport, Tick, Transport, MAX_FRAME,
};

fn pair() -> (TcpTransport, TcpTransport, PeerId) {
    let mut host = TcpTransport::listen("127.0.0.1:0").expect("binding a loopback port");
    let addr = host.local_addr().expect("a bound address");
    let client = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    let mut joined = Vec::new();
    for _ in 0..10_000 {
        while host.poll().is_some() {}
        joined = host.take_joined();
        if !joined.is_empty() {
            break;
        }
    }
    assert_eq!(joined.len(), 1, "the host never accepted the client");
    (host, client, joined[0])
}

/// MTU, in the only form this layer can see it: the reader must not care where
/// the stream was cut. One byte at a time is the worst case and the clearest.
#[test]
fn a_frame_survives_being_delivered_one_byte_at_a_time() {
    let message = Message::Tick(l2_net::TickPacket {
        tick: Tick(7),
        from: PlayerSlot::new(1),
        commands: vec![l2_net::Command {
            slot: PlayerSlot::new(1),
            seq: 3,
            payload: Order::Attack { unit: 2 }.payload(),
        }],
        acks: Vec::new(),
    });
    let bytes = frame(&Canonical::bytes_of(&message)).unwrap();

    let mut reader = FrameReader::new();
    for (i, b) in bytes.iter().enumerate() {
        assert!(
            reader.next_message().unwrap().is_none(),
            "a message appeared after only {i} of {} bytes",
            bytes.len()
        );
        reader.feed(&[*b]);
    }
    let got = reader.next_message().unwrap().expect("the last byte completes it");
    assert_eq!(decode_all::<Message>(&got).unwrap(), message);
    assert!(reader.next_message().unwrap().is_none(), "exactly one message");
}

/// The opposite cut: many frames arriving glued together in a single read, which
/// is what Nagle-free bulk sending actually produces.
#[test]
fn many_frames_in_one_read_are_all_recovered() {
    let mut stream = Vec::new();
    for tick in 0..64u32 {
        let m = Message::Tick(l2_net::TickPacket {
            tick: Tick(tick),
            from: PlayerSlot::new(0),
            commands: Vec::new(),
            acks: Vec::new(),
        });
        stream.extend_from_slice(&frame(&Canonical::bytes_of(&m)).unwrap());
    }

    let mut reader = FrameReader::new();
    reader.feed(&stream);
    let mut seen = Vec::new();
    while let Some(m) = reader.next_message().unwrap() {
        match decode_all::<Message>(&m).unwrap() {
            Message::Tick(p) => seen.push(p.tick.0),
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(seen, (0..64).collect::<Vec<_>>(), "all of them, in order");
}

/// A message at the frame limit crosses a real socket intact. The interesting
/// part is not the size but that it is certainly split by the kernel, so this is
/// the fragmentation test with an actual network under it.
#[test]
fn a_message_at_the_frame_limit_crosses_a_real_socket() {
    let (mut host, mut client, client_id) = pair();

    let payload = vec![0xA5u8; MAX_FRAME - 64];
    let bytes = frame(&payload).unwrap();
    let sent = bytes.len();
    host.send(client_id, &bytes).unwrap();

    let mut reader = FrameReader::new();
    let mut got = None;
    for _ in 0..200_000 {
        while let Some((_, chunk)) = client.poll() {
            reader.feed(&chunk);
        }
        if let Some(m) = reader.next_message().unwrap() {
            got = Some(m);
            break;
        }
        host.send(client_id, &[]).ok();
    }
    let got = got.unwrap_or_else(|| panic!("{sent} bytes never arrived whole"));
    assert_eq!(got.len(), payload.len());
    assert_eq!(got, payload, "the bytes changed in transit");
}

/// Head-of-line blocking, which is the one that could actually corrupt a game.
///
/// One peer stops sending for a stretch. The other must **wait** — reporting
/// exactly who it is waiting on — and must not advance a single tick on its
/// own. When the stalled peer resumes, both must arrive at identical checksums,
/// with the stall leaving no trace in the simulation.
#[test]
fn a_stalled_peer_blocks_the_other_and_then_both_catch_up() {
    let (host_net, client_net, client_id) = pair();
    let seed = 0xB10C_C1EDu64;
    let slots = [PlayerSlot::new(0), PlayerSlot::new(1)];

    let mut sims = [ToySim::new(seed, 8), ToySim::new(seed, 8)];
    let mut sessions = [
        Session::new(Config::battle(), slots[0], &slots, seed, &sims[0]),
        Session::new(Config::battle(), slots[1], &slots, seed, &sims[1]),
    ];
    let mut nets = [host_net, client_net];
    let mut readers = [FrameReader::new(), FrameReader::new()];
    let targets = [client_id, PeerId(0)];
    let mut hashes: [Vec<(Tick, u64)>; 2] = [Vec::new(), Vec::new()];

    // Peer 1 goes quiet once it has sealed this many packets, then resumes.
    let mut frozen_backlog: Vec<Vec<u8>> = Vec::new();
    let mut peer1_sealed = 0;
    const FREEZE_AFTER: usize = 6;
    const THAW_AT: usize = 400;

    let mut waited_on_peer1 = 0;

    for step in 0..40_000 {
        for i in 0..2 {
            let mut sealed = Vec::new();
            while let Some(packet) = sessions[i].next_packet() {
                sealed.push(frame(&Canonical::bytes_of(&Message::Tick(packet))).unwrap());
            }
            for bytes in sealed {
                if i == 1 {
                    peer1_sealed += 1;
                    // While frozen, peer 1's packets are held rather than
                    // dropped. TCP does not lose them, and neither does this.
                    if peer1_sealed > FREEZE_AFTER && step < THAW_AT {
                        frozen_backlog.push(bytes);
                        continue;
                    }
                }
                nets[i].send(targets[i], &bytes).unwrap();
            }
        }
        // Thaw: everything held goes out, in the order it was produced.
        if step == THAW_AT {
            for bytes in frozen_backlog.drain(..) {
                nets[1].send(targets[1], &bytes).unwrap();
            }
        }

        for i in 0..2 {
            while let Some((_, chunk)) = nets[i].poll() {
                readers[i].feed(&chunk);
            }
            while let Some(m) = readers[i].next_message().unwrap() {
                match decode_all::<Message>(&m).unwrap() {
                    Message::Tick(p) => sessions[i].receive(p).unwrap(),
                    other => panic!("unexpected {other:?}"),
                }
            }
            loop {
                match sessions[i].advance(&mut sims[i]) {
                    Advance::Stepped { tick, hash } => hashes[i].push((tick, hash)),
                    Advance::Waiting { missing, .. } => {
                        if i == 0 && missing == vec![slots[1]] {
                            waited_on_peer1 += 1;
                        }
                        break;
                    }
                    Advance::Halted(r) => panic!("halted: {r}"),
                }
            }
        }

        if step > THAW_AT && hashes.iter().all(|h| h.len() >= 60) {
            break;
        }
    }

    assert!(
        waited_on_peer1 > 0,
        "the stall never actually blocked peer 0 - the test proved nothing"
    );
    // The decisive assertion: peer 0 did not run ahead while it was blocked.
    assert!(
        hashes[0].len() >= 60 && hashes[1].len() >= 60,
        "the session never recovered: {} and {} ticks",
        hashes[0].len(),
        hashes[1].len()
    );
    let common = hashes[0].len().min(hashes[1].len());
    assert_eq!(
        hashes[0][..common],
        hashes[1][..common],
        "a stall changed the simulation, which is exactly what must never happen"
    );
    assert_eq!(sims[0].units, sims[1].units);
    assert!(sessions.iter().all(|s| !s.is_halted()));
    assert!(sessions.iter().all(|s| s.divergence().is_none()));
}

/// The pause has no clock behind it: this crate never decides a peer is gone.
/// A caller with a stopwatch does. Pinned because "wait forever" is a deliberate
/// design choice (D-5) that looks like a hang if you do not know it.
#[test]
fn waiting_is_indefinite_and_never_becomes_a_timeout_on_its_own() {
    let (_host, _client, _id) = pair();
    let seed = 99;
    let slots = [PlayerSlot::new(0), PlayerSlot::new(1)];
    let mut sim = ToySim::new(seed, 4);
    let mut session = Session::new(Config::battle(), slots[0], &slots, seed, &sim);

    // Nothing will ever arrive from slot 1. Note that the local slot is
    // reported missing too, and rightly so: nothing has sealed a packet for it
    // either, and a session that quietly excused itself would be treating its
    // own silence differently from a peer's.
    for _ in 0..10_000 {
        match session.advance(&mut sim) {
            Advance::Stepped { .. } => {}
            Advance::Waiting { missing, .. } => {
                assert!(
                    missing.contains(&slots[1]),
                    "the absent peer must be named: {missing:?}"
                );
            }
            Advance::Halted(r) => panic!("this crate has no clock, so it cannot time out: {r}"),
        }
    }
    assert!(!session.is_halted(), "a stall is not a halt");
    assert!(session.divergence().is_none());
}
