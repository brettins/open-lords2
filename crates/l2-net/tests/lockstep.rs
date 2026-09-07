//! The lockstep core, exercised as a whole session.
//!
//! This is the file `docs/netcode.md` §6 is really asking for: **two
//! simulations in one process, fed identical commands, must produce
//! identical checksums, and a deliberately perturbed one must be
//! caught.** Everything here runs with no network, no game install, no
//! clock and no threads, which is the only reason it will be run often
//! enough to be worth having.
//!
//! The harness below is a complete session — sealing, framing,
//! transport, reassembly, ordering, stepping, checksum exchange — with
//! the loopback standing in for sockets. Nothing is stubbed out except
//! the sockets themselves.

mod common;

use common::{Order, ToySim};
use l2_net::{
    decode_all, frame, Advance, Canonical, Config, Endpoint, FrameReader, HaltReason, Loopback,
    Message, PeerId, PlayerSlot, Fixed, Session, SessionError, Tick, Transport,
};

struct Peer {
    slot: PlayerSlot,
    id: PeerId,
    endpoint: Endpoint,
    reader: FrameReader,
    session: Session,
    sim: ToySim,
    /// Every checksum this peer computed, so two peers can be compared
    /// tick by tick rather than only at the end.
    hashes: Vec<(Tick, u64)>,
    errors: Vec<SessionError>,
    waited_for: Vec<PlayerSlot>,
    /// Every tick this peer sealed a packet for.
    sealed: Vec<Tick>,
    /// The last execution tick a scheduled command was issued for, so
    /// a schedule fires once per tick however many rounds that tick
    /// takes.
    issued_through: Option<Tick>,
}

struct Table {
    net: Loopback,
    peers: Vec<Peer>,
}

impl Table {
    /// `players` peers, all in one session, each with its own
    /// simulation seeded identically.
    fn new(config: Config, players: u8, seed: u64) -> Table {
        let slots: Vec<PlayerSlot> = (0..players).map(PlayerSlot::new).collect();
        let ids: Vec<PeerId> = (0..players as u32).map(PeerId).collect();
        let net = Loopback::with_peers(&ids);
        let peers = (0..players as usize)
            .map(|i| {
                let sim = ToySim::new(seed, 6);
                Peer {
                    slot: slots[i],
                    id: ids[i],
                    endpoint: net.endpoint(ids[i]),
                    reader: FrameReader::new(),
                    session: Session::new(config.clone(), slots[i], &slots, seed, &sim),
                    sim,
                    hashes: Vec::new(),
                    errors: Vec::new(),
                    waited_for: Vec::new(),
                    sealed: Vec::new(),
                    issued_through: None,
                }
            })
            .collect();
        Table { net, peers }
    }

    /// One turn of the loop every peer runs: seal, send, drain,
    /// advance.
    fn pump(&mut self) {
        let count = self.peers.len();

        for i in 0..count {
            let mut sealed = Vec::new();
            while let Some(packet) = self.peers[i].session.next_packet() {
                sealed.push(packet);
            }
            for packet in sealed {
                self.peers[i].sealed.push(packet.tick);
                let bytes = frame(&Canonical::bytes_of(&Message::Tick(packet))).unwrap();
                for j in 0..count {
                    if i != j {
                        let target = self.peers[j].id;
                        self.peers[i].endpoint.send(target, &bytes).unwrap();
                    }
                }
            }
        }

        for peer in &mut self.peers {
            while let Some((_from, bytes)) = peer.endpoint.poll() {
                peer.reader.feed(&bytes);
            }
            while let Some(message) = peer.reader.next_message().unwrap() {
                match decode_all::<Message>(&message).expect("a well-formed message") {
                    Message::Tick(packet) => {
                        if let Err(error) = peer.session.receive(packet) {
                            peer.errors.push(error);
                        }
                    }
                    other => panic!("unexpected {other:?}"),
                }
            }
        }

        for peer in &mut self.peers {
            loop {
                match peer.session.advance(&mut peer.sim) {
                    Advance::Stepped { tick, hash } => peer.hashes.push((tick, hash)),
                    Advance::Waiting { missing, .. } => {
                        peer.waited_for.extend(missing);
                        break;
                    }
                    Advance::Halted(_) => break,
                }
            }
        }
    }

    fn run(&mut self, rounds: usize) {
        for _ in 0..rounds {
            self.pump();
        }
    }

    /// Issue commands according to a schedule keyed on the **execution
    /// tick**, not on how many rounds of the loop have gone by.
    ///
    /// The distinction is the whole reason this method exists, and
    /// finding it out cost a failing test: a player types when they
    /// type, so under latency the same keystroke lands on a later tick
    /// — which changes the game, correctly and by design. A test that
    /// issued per round would therefore compare two runs that were
    /// given genuinely different input and call the difference a
    /// desync. Keying the schedule to the tick a command will execute
    /// on is what makes "the network's timing changed nothing" a
    /// statement about the lockstep core rather than about the test.
    fn run_scheduled(&mut self, rounds: usize, schedule: impl Fn(u8, Tick) -> Option<Order>) {
        for _ in 0..rounds {
            for peer in &mut self.peers {
                let at = peer.session.execution_tick();
                if peer.issued_through.is_none_or(|last| at > last) {
                    peer.issued_through = Some(at);
                    if let Some(order) = schedule(peer.slot.index(), at) {
                        peer.session.issue(order.payload());
                    }
                }
            }
            self.pump();
        }
    }

    fn all_agree(&self) -> bool {
        let first = l2_net::state_hash(&self.peers[0].sim);
        self.peers.iter().all(|p| l2_net::state_hash(&p.sim) == first)
    }
}

// --- the property everything else rests on ----------------------------

#[test]
fn two_sessions_fed_identical_commands_agree_on_every_tick() {
    let mut table = Table::new(Config::battle(), 2, 0xabc_def);

    for round in 0..60 {
        if round % 7 == 0 {
            table.peers[0].session.issue(Order::Attack { unit: 1 }.payload());
        }
        if round % 5 == 0 {
            let order =
                Order::MoveTo { unit: 2, x: Fixed::from_int(40), y: Fixed::from_int(12) };
            table.peers[1].session.issue(order.payload());
        }
        table.pump();
    }

    assert!(!table.peers[0].session.is_halted(), "{:?}", table.peers[0].session.halt_reason());
    assert!(table.peers[0].hashes.len() > 50, "the session barely advanced");
    assert_eq!(
        table.peers[0].hashes, table.peers[1].hashes,
        "two peers disagreed about some tick"
    );
    assert!(table.all_agree());
    assert_eq!(table.peers[0].errors, vec![]);
    assert_eq!(table.peers[0].sim.bad_commands, 0);
}

/// The other half of §6's claim. One peer's damage rolls are off by
/// one — a peer that is *almost* right, which is what a real desync
/// looks like.
#[test]
fn a_perturbed_peer_is_caught_and_both_sides_halt() {
    let mut table = Table::new(Config::battle(), 2, 7);
    table.peers[1].sim.bias = 1;

    // Nothing diverges until the biased code path actually runs, which
    // is the honest shape of the problem: a desync detector cannot see
    // a difference that has not been computed yet.
    table.run(4);
    assert!(!table.peers[0].session.is_halted(), "diverged before anything could differ");

    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    table.run(12);

    let divergence = table.peers[0].session.divergence().expect("peer 0 must notice");
    assert_eq!(
        table.peers[0].session.halt_reason(),
        Some(&HaltReason::Desync { tick: divergence.tick })
    );
    assert!(table.peers[1].session.is_halted(), "the other peer must notice too");
    assert_eq!(divergence.peer, PlayerSlot::new(1));
    assert_ne!(divergence.ours, divergence.theirs);
    assert_eq!(
        divergence.last_agreed,
        Some(divergence.tick.minus(1)),
        "with a checksum every tick the divergence is localised to one tick"
    );
}

/// §6: halt. A halted session must not step again no matter how much it
/// is pumped, because continuing turns a reproducible bug into an
/// unreproducible one.
#[test]
fn a_halted_session_never_steps_again() {
    let mut table = Table::new(Config::battle(), 2, 7);
    table.peers[1].sim.bias = 1;
    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    table.run(12);
    assert!(table.peers[0].session.is_halted());

    let frozen = table.peers[0].session.tick();
    let state = l2_net::state_hash(&table.peers[0].sim);
    table.run(20);
    assert_eq!(table.peers[0].session.tick(), frozen);
    assert_eq!(l2_net::state_hash(&table.peers[0].sim), state);
}

/// The first halt reason sticks: a session that desynced and was then
/// torn down must still say it desynced.
#[test]
fn the_first_halt_reason_wins() {
    let mut table = Table::new(Config::battle(), 2, 7);
    table.peers[1].sim.bias = 1;
    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    table.run(12);
    let reason = table.peers[0].session.halt_reason().cloned().unwrap();
    table.peers[0].session.halt(HaltReason::Left);
    assert_eq!(table.peers[0].session.halt_reason(), Some(&reason));
}

// --- the network's opinion must not matter -----------------------------

/// §4: commands are concatenated in a fixed order, never in arrival
/// order, because arrival order is the network's opinion and it differs
/// per peer. Latency and reordering must therefore change *nothing*
/// about the result — only how long it takes.
#[test]
fn latency_and_reordering_change_nothing_but_the_pace() {
    fn play(latency: u64, reorder: bool) -> Vec<(Tick, u64)> {
        let mut table = Table::new(Config::battle(), 2, 0x5eed);
        table.net.set_latency(PeerId(0), latency);
        table.net.set_reorder(PeerId(0), reorder);
        table.net.set_reorder(PeerId(1), reorder);
        table.run_scheduled(200, |slot, tick| match (slot, tick.0 % 12) {
            (0, 0) => Some(Order::Attack { unit: 1 }),
            (1, 5) => Some(Order::Attack { unit: 2 }),
            (1, 9) => {
                Some(Order::MoveTo { unit: 0, x: Fixed::from_int(8), y: Fixed::from_int(8) })
            }
            _ => None,
        });
        assert!(!table.peers[0].session.is_halted());
        assert_eq!(table.peers[0].hashes, table.peers[1].hashes);
        table.peers[0].hashes.clone()
    }

    let clean = play(0, false);
    assert!(clean.len() > 100);
    let delayed = play(3, false);
    let jumbled = play(0, true);
    let both = play(3, true);

    // A delayed peer gets fewer ticks done in the same number of
    // rounds, but every tick it did complete must be identical.
    for (name, run) in [("delayed", &delayed), ("jumbled", &jumbled), ("both", &both)] {
        assert!(!run.is_empty(), "{name} produced nothing");
        assert_eq!(
            run[..],
            clean[..run.len()],
            "{name} produced different checksums from the clean run"
        );
    }
}

// --- stalling ----------------------------------------------------------

/// §4: if a peer's packet has not arrived, the simulation blocks. It
/// does not extrapolate and it does not guess, because proceeding is a
/// desync and a desync is worse than a pause.
#[test]
fn a_stalled_link_pauses_the_session_and_it_resumes_intact() {
    let mut table = Table::new(Config::battle(), 2, 3);
    table.run(10);
    let reached = table.peers[0].session.tick();
    assert!(reached > Tick(4));

    // Peer 0 stops hearing from peer 1 — nothing lost, everything
    // late, which is what a quiet peer looks like over a reliable
    // transport.
    table.net.hold(PeerId(0), true);
    table.peers[0].waited_for.clear();
    table.run(20);

    assert_eq!(table.peers[0].session.tick(), reached, "the session must not advance on a guess");
    assert!(!table.peers[0].session.is_halted(), "a stall is not a halt: this crate has no clock");
    assert!(!table.peers[0].waited_for.is_empty(), "the stall must name who it is waiting for");
    assert!(
        table.peers[0].waited_for.iter().all(|s| *s == PlayerSlot::new(1)),
        "waiting for {:?}",
        table.peers[0].waited_for
    );

    // And it resumes, having missed nothing.
    table.net.hold(PeerId(0), false);
    table.run(20);
    assert!(table.peers[0].session.tick() > reached, "the backlog must let it catch up");
    let caught_up = table.peers[0].hashes.len();
    assert_eq!(table.peers[0].hashes, table.peers[1].hashes[..caught_up]);
}

/// The same experiment with a link that *discards* instead of holding.
///
/// The session never recovers, and that is correct rather than a
/// defect: tick `N` cannot be simulated without every peer's commands
/// for it, nothing in the design retransmits, and §4 says plainly that
/// proceeding without them is a desync. This is the concrete form of
/// §7's third requirement — reliable, ordered delivery — and the reason
/// the transport may not be a bare UDP socket.
#[test]
fn a_lossy_link_ends_the_session_which_is_why_the_transport_must_be_reliable() {
    let mut table = Table::new(Config::battle(), 2, 3);
    table.run(10);
    let reached = table.peers[0].session.tick();

    table.net.partition(PeerId(0), true);
    table.run(10);
    table.net.partition(PeerId(0), false);
    table.run(50);

    assert_eq!(
        table.peers[0].session.tick(),
        reached,
        "a dropped tick packet is unrecoverable: there is no retransmission and no skipping"
    );
    assert!(!table.peers[0].session.is_halted(), "it waits forever; the caller must time it out");
}

/// The timeout §4 asks for is the caller's to impose, because this
/// crate has no clock (D-5). What it offers is a way to say so.
#[test]
fn a_timeout_is_something_the_caller_declares() {
    let mut table = Table::new(Config::battle(), 2, 3);
    table.run(5);
    table.net.partition(PeerId(0), true);
    table.run(5);
    let peer = &mut table.peers[0];
    peer.session.halt(HaltReason::Timeout);
    assert!(matches!(
        peer.session.advance(&mut peer.sim),
        Advance::Halted(HaltReason::Timeout)
    ));
}

// --- input delay --------------------------------------------------------

/// §4: a command issued during tick `N` executes on tick `N + 2`, so
/// every peer has both peers' commands for a tick before it runs it.
#[test]
fn a_command_executes_after_the_input_delay() {
    let mut table = Table::new(Config::battle(), 2, 1);
    assert_eq!(table.peers[0].session.execution_tick(), Tick(2));

    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    assert_eq!(table.peers[0].session.staged(), 1);
    table.run(6);

    let replay = table.peers[0].session.replay().expect("recording");
    let commands: Vec<Tick> = replay.commands.iter().map(|(tick, _)| *tick).collect();
    assert_eq!(commands, vec![Tick(2)], "a command issued at tick 0 must run at tick 2");
    assert_eq!(table.peers[0].session.staged(), 0);
}

/// The priming packets. With a delay of two, ticks 0 and 1 must be
/// sealed before anything can run, and both are necessarily empty.
#[test]
fn the_first_packets_prime_the_pipeline() {
    let sim = ToySim::new(1, 2);
    let slots = [PlayerSlot::new(0), PlayerSlot::new(1)];
    let mut session = Session::new(Config::battle(), slots[0], &slots, 1, &sim);
    session.issue(Order::Attack { unit: 0 }.payload());

    let first = session.next_packet().expect("tick 0");
    let second = session.next_packet().expect("tick 1");
    let third = session.next_packet().expect("tick 2");
    assert_eq!(session.next_packet(), None, "one packet per tick and no more");

    assert_eq!(first.tick, Tick(0));
    assert!(first.commands.is_empty(), "nothing can execute before the delay has elapsed");
    assert_eq!(second.tick, Tick(1));
    assert!(second.commands.is_empty());
    assert_eq!(third.tick, Tick(2));
    assert_eq!(third.commands.len(), 1);
}

/// §4: every peer sends one packet per tick, **including when it has no
/// commands**, because without it "no input" and "player disconnected"
/// are indistinguishable.
#[test]
fn a_packet_is_sent_every_tick_even_with_nothing_to_say() {
    let mut table = Table::new(Config::battle(), 2, 1);
    table.run(12);

    let simulated = table.peers[0].hashes.len() as u32;
    assert!(simulated >= 9, "only {simulated} ticks ran");
    // One packet per tick, contiguous from zero, with no gaps — a gap
    // is a peer that said nothing, which the other side cannot
    // distinguish from a disconnection.
    let expected: Vec<Tick> = (0..table.peers[0].sealed.len() as u32).map(Tick).collect();
    assert_eq!(table.peers[0].sealed, expected);
    assert!(table.peers[0].sealed.len() as u32 >= simulated);

    // Drained fully, the peer has sealed exactly up to its current tick
    // plus the input delay — which is what "everyone has everyone's
    // commands before they are needed" means in one line.
    while let Some(packet) = table.peers[0].session.next_packet() {
        assert!(packet.commands.is_empty());
        table.peers[0].sealed.push(packet.tick);
    }
    assert_eq!(
        *table.peers[0].sealed.last().unwrap(),
        table.peers[0].session.tick().plus(Config::battle().input_delay)
    );
}

// --- the kingdom layer is the same machine ------------------------------

/// §5, run through the same code as §4 with `input_delay = 0`. Five
/// players, one step per turn, commands ordered by slot.
#[test]
fn the_kingdom_layer_is_the_same_mechanism_with_different_numbers() {
    let mut table = Table::new(Config::kingdom(), 5, 0x10ad);
    assert_eq!(table.peers[0].session.execution_tick(), Tick(0));

    for turn in 0..8 {
        for player in 0..5 {
            table.peers[player]
                .session
                .issue(Order::Attack { unit: turn as u32 % 4 }.payload());
        }
        table.pump();
    }

    let first = &table.peers[0].hashes;
    assert!(first.len() >= 7, "only {} turns resolved", first.len());
    for peer in &table.peers {
        assert_eq!(&peer.hashes, first, "{} disagreed", peer.slot);
        assert!(!peer.session.is_halted());
    }

    // Commands were applied in slot order, not arrival order.
    let replay = table.peers[0].session.replay().unwrap();
    let turn_one = replay.commands_at(Tick(1));
    assert_eq!(turn_one.len(), 5);
    let slots: Vec<u8> = turn_one.iter().map(|c| c.slot.index()).collect();
    assert_eq!(slots, vec![0, 1, 2, 3, 4]);
}

// --- checksum cadence ---------------------------------------------------

/// §6's fallback: keep hashing every tick, exchange less often. The
/// divergence is still caught, just later — and the history ring is
/// what makes "later" still useful.
#[test]
fn a_sparser_exchange_still_catches_a_divergence() {
    let mut config = Config::battle();
    config.exchange_every = 4;
    let mut table = Table::new(config, 2, 7);
    table.peers[1].sim.bias = 1;
    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    table.run(30);

    assert!(table.peers[0].session.is_halted());
    let divergence = table.peers[0].session.divergence().unwrap();
    assert_eq!(divergence.tick.0 % 4, 0, "only exchanged ticks can be compared");
    // Every tick is still hashed, so the ring can localise it.
    let history = table.peers[0].session.history();
    assert!(history.hash_at(divergence.tick.minus(1)).is_some());
}

#[test]
fn the_history_ring_holds_the_last_n_ticks_and_no_more() {
    let mut config = Config::battle();
    config.history = 8;
    let mut table = Table::new(config, 2, 2);
    table.run(40);

    let history = table.peers[0].session.history();
    assert_eq!(history.len(), 8);
    let last = history.last().unwrap().0;
    assert_eq!(history.earliest(), Some(last.minus(7)));
    assert!(history.hash_at(last).is_some());
    assert!(history.hash_at(last.minus(8)).is_none(), "older than the ring must be forgotten");
}

// --- refusing bad packets ------------------------------------------------

fn lone_session() -> (Session, ToySim, [PlayerSlot; 2]) {
    let slots = [PlayerSlot::new(0), PlayerSlot::new(1)];
    let sim = ToySim::new(1, 2);
    let session = Session::new(Config::battle(), slots[0], &slots, 1, &sim);
    (session, sim, slots)
}

#[test]
fn a_packet_claiming_to_be_from_us_is_refused() {
    let (mut session, _sim, slots) = lone_session();
    let packet = l2_net::TickPacket::empty(Tick(0), slots[0]);
    assert_eq!(session.receive(packet), Err(SessionError::OwnSlot(slots[0])));
}

#[test]
fn a_packet_from_a_stranger_is_refused() {
    let (mut session, _sim, _slots) = lone_session();
    let packet = l2_net::TickPacket::empty(Tick(0), PlayerSlot::new(3));
    assert_eq!(session.receive(packet), Err(SessionError::UnknownSlot(PlayerSlot::new(3))));
}

#[test]
fn a_second_packet_for_a_tick_is_refused() {
    let (mut session, _sim, slots) = lone_session();
    session.receive(l2_net::TickPacket::empty(Tick(0), slots[1])).unwrap();
    assert_eq!(
        session.receive(l2_net::TickPacket::empty(Tick(0), slots[1])),
        Err(SessionError::Duplicate { slot: slots[1], tick: Tick(0) })
    );
}

/// An integrity check rather than an anti-cheat measure: a packet whose
/// contents contradict its header is far more likely to be a relaying
/// bug in our own host code than an attack.
#[test]
fn a_packet_carrying_someone_elses_command_is_refused() {
    let (mut session, _sim, slots) = lone_session();
    let mut packet = l2_net::TickPacket::empty(Tick(0), slots[1]);
    packet.commands.push(l2_net::Command::new(slots[0], 0, Vec::new()));
    assert_eq!(
        session.receive(packet),
        Err(SessionError::ForgedCommand { packet_from: slots[1], command_from: slots[0] })
    );
}

#[test]
fn a_packet_far_beyond_the_horizon_is_refused() {
    let (mut session, _sim, slots) = lone_session();
    let packet = l2_net::TickPacket::empty(Tick(10_000), slots[1]);
    assert!(matches!(session.receive(packet), Err(SessionError::TooFarAhead { .. })));
}

#[test]
fn a_packet_for_a_tick_already_run_is_reported() {
    let mut table = Table::new(Config::battle(), 2, 4);
    table.run(6);
    let simulated = table.peers[0].session.tick().minus(1);
    let stale = l2_net::TickPacket::empty(simulated, PlayerSlot::new(1));
    assert_eq!(
        table.peers[0].session.receive(stale),
        Err(SessionError::AlreadySimulated { tick: simulated })
    );
}

#[test]
fn session_errors_explain_themselves() {
    assert_eq!(
        SessionError::UnknownSlot(PlayerSlot::new(2)).to_string(),
        "player 2 is not in this session"
    );
}

// --- session bookkeeping --------------------------------------------------

#[test]
fn a_session_reports_what_it_was_built_with() {
    let (session, _sim, slots) = lone_session();
    assert_eq!(session.local_slot(), slots[0]);
    assert_eq!(session.slots(), &slots);
    assert_eq!(session.seed(), 1);
    assert_eq!(session.tick(), Tick(0));
    assert_eq!(session.config().input_delay, 2);
}

#[test]
#[should_panic(expected = "must be one of the session's slots")]
fn a_session_whose_local_player_is_not_in_it_is_a_bug() {
    let sim = ToySim::new(1, 1);
    let _ = Session::new(Config::battle(), PlayerSlot::new(3), &[PlayerSlot::new(0)], 1, &sim);
}

#[test]
fn slots_are_sorted_and_deduplicated() {
    let sim = ToySim::new(1, 1);
    let given = [PlayerSlot::new(2), PlayerSlot::new(0), PlayerSlot::new(2)];
    let session = Session::new(Config::battle(), PlayerSlot::new(0), &given, 1, &sim);
    assert_eq!(session.slots(), &[PlayerSlot::new(0), PlayerSlot::new(2)]);
}

#[test]
fn recording_can_be_turned_off() {
    let mut config = Config::battle();
    config.record = false;
    let mut table = Table::new(config, 2, 1);
    table.run(5);
    assert!(table.peers[0].session.replay().is_none());
    assert!(table.peers[0].session.dump(&table.peers[0].sim).is_none());
}

// --- the dump -------------------------------------------------------------

/// §6's localisation, end to end: two dumps, compared, name the
/// subsystem that diverged.
#[test]
fn two_dumps_name_the_subsystem_that_diverged() {
    let mut table = Table::new(Config::battle(), 2, 7);
    table.peers[1].sim.bias = 1;
    table.peers[0].session.issue(Order::Attack { unit: 0 }.payload());
    table.run(12);
    assert!(table.peers[0].session.is_halted() && table.peers[1].session.is_halted());

    let ours = table.peers[0].session.dump(&table.peers[0].sim).expect("a dump");
    let theirs = table.peers[1].session.dump(&table.peers[1].sim).expect("a dump");

    assert_eq!(ours.author, PlayerSlot::new(0));
    assert!(!ours.state.is_empty(), "the diverged state is the one replaying cannot recover");
    assert_eq!(ours.replay.seed, theirs.replay.seed);

    let comparison = ours.compare(&theirs);
    assert!(comparison.same_seed);
    assert!(comparison.same_initial_state);
    assert!(
        comparison.same_command_stream,
        "the peers applied the same commands, so this is a simulation fault, not a network one"
    );
    assert_eq!(comparison.first_difference, Some(ours.divergence.tick));
    assert_eq!(
        comparison.sole_subsystem,
        Some("units"),
        "an off-by-one in a damage roll should localise to the unit array: {comparison}"
    );

    // And the dump encodes, for something with filesystem access to
    // write out.
    let bytes = Canonical::bytes_of(&ours);
    assert!(bytes.starts_with(b"L2DD"));
    assert!(bytes.len() > 100);
}

/// A divergence in the generator alone means somebody drew a random
/// number outside the simulation — §6 gives the PRNG its own slot for
/// exactly this reading.
#[test]
fn a_generator_that_drifts_alone_is_visible_as_such() {
    let mut table = Table::new(Config::battle(), 2, 11);
    // One extra draw on one peer, touching nothing else.
    table.peers[1].sim.rng.next_u32();
    table.run(8);

    assert!(table.peers[0].session.is_halted());
    let ours = table.peers[0].session.dump(&table.peers[0].sim).unwrap();
    let theirs = table.peers[1].session.dump(&table.peers[1].sim).unwrap();
    let comparison = ours.compare(&theirs);
    assert!(
        comparison.subsystems.contains(&"rng"),
        "the generator must be visible in the digest: {comparison}"
    );
}
