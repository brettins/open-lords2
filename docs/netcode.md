# Netcode

**Status: design. Nothing in `crates/` implements any of this yet.**

This document is written *before* the network layer exists, and deliberately so.
Almost everything in it is a constraint on the **simulation**, not on the network
code — and the simulation is being written now. Determinism costs very little if
it is designed in and a great deal if it is retrofitted, because retrofitting it
means auditing every arithmetic expression and every iteration order in the
engine at once.

**If you are writing simulation code today and read nothing else, read
[§3, the determinism contract](#3-the-determinism-contract).** In one line: no
floats, no `HashMap` iteration, no clocks, no ambient randomness, and `step()` is
a pure function of `(state, commands)`. Those five cost nothing to honour now.

The original game's networking was investigated first and is **not** the basis
for any of this. We will never play against the 1996 binary, so its wire format
is something we would learn and then discard. What was found is kept as
[Appendix A](#appendix-a--what-the-original-did) because it explains why we are
replacing the transport rather than wrapping it.

---

## 1. Two layers, two very different problems

Lords of the Realm II is two games stacked on each other, and they have almost
nothing in common as networking problems.

| | Kingdom layer | Battle layer |
|---|---|---|
| Model | Turn-based, phase machine | Continuous, real-time |
| Participants | up to 5 | 2 |
| Latency budget | seconds | ~100–200 ms |
| Sync frequency | once per turn | ~10 per second |
| State size | whole world | one 80×80 field |
| Entity count | ~150 units, 1–17 counties | up to thousands of soldiers |

Where those numbers come from:

- **5 players** — *verified.* `g_playerStartCount` is 5, 4 or 2 across the
  shipped maps (`docs/symbols.md`), and the original opens its DirectPlay
  session with `dwMaxPlayers = 5` (observed live; Appendix A).
- **2 sides per battle** — *verified.* A `.skr` battle map carries exactly two
  army records, `base + mapIndex * 0x58` with armies at `+0x00` and `+0x2C`
  (`docs/formats/skr.md`).
- **80×80 battlefield, ~150 units, 1–17 counties** — *verified*, from
  `docs/formats/skr.md` and `docs/symbols.md` (`g_units` is 150 records,
  `g_countyCount` caps at 17).
- Tick rate and latency budget are **ours to choose**. We have not measured the
  original's battle tick rate and do not need to.

The two layers get the *same mechanism* (deterministic lockstep) with very
different parameters. One mechanism is worth a lot: one determinism contract,
one desync detector, one replay format, one test harness.

---

## 2. Decision: deterministic lockstep, not state synchronisation

**Lockstep**: every peer runs the identical simulation, and the only thing that
crosses the wire is the *player commands*. **State sync**: one authority runs the
simulation and ships the resulting world state to everyone else.

We choose lockstep, for four reasons.

**It is the only one whose cost does not scale with the world.** A battle can
have thousands of individual soldiers on an 80×80 field. State-syncing that many
entities several times a second is the expensive design; shipping "player 2
ordered unit 7 to move to (34, 12)" is a handful of bytes regardless of how many
soldiers are on the field. The kingdom layer is even more lopsided: an entire
turn is a few dozen commands against a world state of hundreds of kilobytes.

**We control the simulation, so determinism is actually available to us.**
This is the reason lockstep is usually rejected and the reason it is right here.
A team wrapping someone else's physics engine cannot guarantee bit-identical
results; we are writing every line of the simulation, in Rust, against a design
(`docs/decisions.md` D4) that already insists on owning a plain `Vec<u8>`
framebuffer rather than hiding behind an engine. The same instinct applies to
the simulation.

**Replays come free, and this project needs replays more than most.** A lockstep
session is fully described by `(initial state, seed, ordered command stream)`.
That is a complete, tiny, replayable recording — and `docs/decisions.md` D7 is
already emphatic that two implementations agreeing proves little, while a
property of the data itself is a real check. A recorded battle that must replay
to a bit-identical final state hash is exactly that kind of check, and it runs in
CI on one machine with no network at all.

**The turn layer is lockstep whether we call it that or not.** A turn *is* a
batch of commands applied in an agreed order. Building the kingdom layer as
state sync would mean inventing a second mechanism to do something the first one
already does.

### What lockstep costs, stated plainly

- **Every peer simulates everything.** No thin clients, no spectators-on-a-phone.
  Fine here: the simulation is a 1996 game's worth of work.
- **The slowest peer sets the pace.** If one player's packet is late, everybody
  waits. This is the "Waiting for player…" banner every 90s RTS had. It is a real
  user-visible cost and the honest alternative is not "no wait" but "rollback",
  which is far more complexity than this genre earns.
- **One divergence anywhere breaks the whole game**, and the symptom appears
  arbitrarily far from the cause. This is the genuine risk of the design, and
  §6 exists entirely to manage it.
- **Late join needs a full state snapshot**, not a command stream. Cheap on the
  kingdom layer (once per turn), not attempted mid-battle.
- **Every peer knows everything**, so a modified client can see through fog of
  war. See §8.

---

## 3. The determinism contract

These are requirements on simulation code. They are listed as numbered rules so
that a code review or a CI lint can cite one.

A "simulation crate" means any crate whose code runs inside `step()`. The intent
is that these live in one crate (`l2-sim`, not yet created) whose dependency list
makes most of the rules structurally impossible to break.

### D-1 — No floating point in the simulation

No `f32`, no `f64`, anywhere a simulation result depends on it.

Rationale: floats are *mostly* deterministic across machines under IEEE-754, and
"mostly" is what makes them dangerous. `x87` versus SSE, fused multiply-add,
compiler reassociation under optimisation, and library functions (`sin`, `sqrt`,
`powf`) that carry no cross-platform bit-exactness guarantee all reintroduce
divergence — and they do it silently, on one player's machine, an hour into a
game.

Note this is not a sacrifice. The original is a 1996 32-bit integer game; its
map is a 64×64 byte grid, its units are 150 fixed records, its blitters copy
bytes. There is no floating-point requirement anywhere in the design we are
reimplementing.

### D-2 — Fixed-point where fractions are genuinely needed

Provide a single `Fixed` type — Q16.16 in an `i32` is the obvious default, with
`i64` intermediates for multiply and divide — in a leaf crate, and use it for
positions, velocities, and combat fractions. All operations must be explicit
about rounding; no "whatever the hardware does".

### D-3 — One seeded PRNG, owned by the simulation state, algorithm frozen in-tree

The generator must be a field of the simulation state, advanced only by
simulation code, and serialised with the state.

**Write the generator ourselves.** Thirty lines of PCG32 or xoshiro256++ in our
own crate. This is not not-invented-here: the value stream must be frozen
*forever*, because a change to it is a silent desync between a player who
updated and one who did not, and between a replay and its recording.

The obvious alternative is `rand_pcg` or `rand_xoshiro`, and it was checked
rather than dismissed. The rust-random project's own reproducibility policy
says, verbatim:

> A change is considered **value-breaking** if it is not API-breaking yet would
> result in changed output values of a deterministic stochastic process using
> only unchanged parts of the `rust-random` API. Value-breaking changes are
> permitted in minor versions.

That is a well-run project being honest about a promise it declines to make —
and it is exactly the promise we need. The practical risk is small: PCG and
xoshiro are externally specified algorithms whose streams have not moved, and
both crates test against published reference vectors. But it is not our risk to
accept, because the failure lands in someone else's game months later, as a
desync. (`rand_xoshiro` additionally carries no reproducibility statement in its
own crate docs at all — only the project-wide policy above applies.) If we ever
do depend on one, pin an exact `=x.y.z`, never a caret range.

A frozen algorithm with test vectors in the repo is strictly safer, and it
matches how `l2-formats` and `l2-mods` are already built (both deliberately
dependency-free — see their `Cargo.toml`).

Corollary: **no ambient randomness.** No `rand::thread_rng()`, no
`getrandom`, no time-seeded anything inside `step()`.

### D-4 — No hash-ordered iteration

Never iterate a `HashMap` or `HashSet` in simulation code. Rust's default hasher
is randomly seeded per process specifically so that iteration order differs
between runs — which is exactly the bug we cannot afford.

Use `Vec`, `BTreeMap`/`BTreeSet`, or an index-ordered arena. The original's own
data layout is already the right shape: `g_units` is a flat 150-entry array
indexed 1..149, and code walks it in index order.

### D-5 — No wall clock, no scheduler

No `SystemTime`, no `Instant`, no frame delta time, no `thread::sleep`-derived
values, no work whose result depends on how many threads finished first. **Time
inside the simulation is the tick counter and nothing else.** Render-side
interpolation may use real time; it must not feed back into `step()`.

### D-6 — No addresses, no allocation order

Nothing derived from a pointer value, an allocation address, a `HashMap`
capacity, or the order the allocator happened to hand out memory may reach the
simulation.

This is not hypothetical. The original game demonstrates the bug: the
`guidApplication` it puts in its DirectPlay session description was **different
on every run** (observed: `{A09FFA29-DF90-71AC-…}`, `{7192BCDB-DF90-71AC-…}`,
`{2BC84424-DF90-70C8-…}`), with a byte pattern that looks like a module address
repeated into the field. Whatever the exact cause, a value that is supposed to
identify "this game" and instead identifies "this process" is precisely the
class of defect this rule forbids. *(The observation is verified; the
address-derivation explanation is inferred.)*

### D-7 — Total orderings, with an id as the final tiebreak

Any sort inside the simulation must use a comparator that is a total order, with
a unique stable id as the last key. `sort_unstable_by` on a key with ties is a
desync waiting for a different allocation pattern.

### D-8 — Deterministic entity allocation

Ids are assigned by a deterministic rule, not by whatever slot the allocator
returns. First-free-slot scanning — which is exactly what the original's
`Unit_Spawn` does over `g_units` (`docs/symbols.md`, verified) — is a good rule:
it is total, it is reproducible, and it survives serialisation.

### D-9 — No parallelism inside a tick

No `rayon`, no work-stealing, no "spawn a thread per county". If a tick ever
becomes slow enough to need parallelism, the parallel section must have an
order-independent reduction and must be justified in `docs/decisions.md`. Until
then the rule is simply: don't.

### D-10 — Canonical command encoding

Commands are serialised by hand-written code with fixed widths and fixed
endianness (little-endian, to match the file formats we already read). No
derive-based format whose layout could shift with a dependency version. A
command must round-trip byte-identically.

### D-11 — `step` is a pure function of state and commands

```
fn step(state: &mut State, commands: &[Command])
```

No file I/O, no logging that reads back into the simulation, no globals, no
`static mut`. Everything the tick may read is reachable from `state` and
`commands`. This rule is what makes the replay harness in §6 possible, and it is
the one worth defending hardest in review.

### D-12 — The ruleset is part of the version

Two peers must agree on: protocol version, engine version, **and the mod set and
load order**. `l2-mods` exists to let mods change the simulation, so a mod
mismatch is a guaranteed desync with a confusing symptom. The session handshake
must exchange a hash over the resolved ruleset — `l2-mods` already tracks which
mod and line set each value, so the material for that hash exists.

### Enforcing this

Rust cannot `#![forbid(float)]`, so enforcement is structural plus mechanical:

1. **Dependency structure.** The simulation crate does not depend on `rand`,
   `rayon`, or anything needing `std::time`. Most of D-3, D-5 and D-9 then
   cannot be broken without an obvious `Cargo.toml` change that shows up in
   review.
2. **A CI grep** over the simulation crate for `f32`, `f64`, `HashMap`,
   `HashSet`, `Instant`, `SystemTime`, `thread_rng`, `sort_unstable_by`, with an
   explicit allow-list of reviewed exceptions. Crude, and crude is fine — it
   catches the accidental cases, which are the ones that happen.
3. **The replay test** (§6). It is the only check that catches a violation
   nobody thought to grep for.

---

## 4. Battle layer

### Tick model

Two clocks, deliberately separate:

- a **command tick** at a fixed 10 Hz — the unit of simulation and of network
  exchange;
- a **render frame** at whatever the display does, interpolating between the
  last two simulation states.

10 Hz is a starting recommendation, not a measurement. It is the rate at which
90s RTS games of this scale ran, it makes a 200 ms input delay two ticks, and it
keeps the exchanged volume trivially small. If play testing wants 15 or 20 Hz,
the number is a constant; nothing else in the design changes.

### Input delay, not rollback

A command issued during tick `N` executes on tick `N + 2`. Every peer therefore
has both peers' commands for tick `N + 2` before it needs to run it, and no peer
ever has to un-run anything.

The cost is a fixed ~200 ms between click and response. The alternative —
rollback with speculative execution and re-simulation — buys responsiveness at
the price of making every piece of simulation state rewindable, which is a large
tax on every future feature. For a game whose battle units are formations of
soldiers rather than fighting-game frames, input delay is the right trade. **We
do not build rollback.**

### The tick packet

Every peer sends one packet per tick, **including when it has no commands**. An
empty packet is what tells the other side it may advance; without it "no input"
and "player disconnected" are indistinguishable.

```
tick_packet {
  u32  tick                 // the tick these commands execute on
  u16  command_count
  [Command; command_count]  // canonical encoding, D-10
  u32  ack_tick             // last tick this peer has fully simulated
  u64  state_hash           // hash of state after ack_tick (§6)
}
```

### Ordering within a tick

Commands from both peers are concatenated in a fixed order — by player slot,
then by the per-player sequence number — before `step()` sees them. Never in
arrival order. Arrival order is the network's opinion and it differs per peer.

### Stalling

If tick `N`'s packet from the other peer has not arrived when it is needed, the
simulation blocks and the UI says so after a short grace period. It does not
extrapolate, and it does not proceed with a guess: proceeding is a desync, and a
desync is worse than a pause. A peer that stays silent past a timeout ends the
battle.

---

## 5. Kingdom layer

The turn layer needs no per-frame anything. Its shape:

1. Each player plans a whole turn locally. Nothing is sent while planning, and
   nothing local is applied — planning builds a command list, it does not mutate
   the world.
2. On "end turn", the player's command list is sent to the host.
3. When the host has every player's list (or a player's timeout expires), it
   concatenates them **in player-slot order** and broadcasts the resulting turn
   packet.
4. Every peer, host included, applies that identical list and then runs the turn
   resolution phase machine.

Latency is irrelevant here: one round trip per turn, against a turn that takes a
human minutes.

### The phase machine is part of the contract

Turn resolution must be an explicit, ordered sequence of phases, not "whatever
order the systems happen to run in". The original works this way already —
`g_turnPhase` is switched on by a state machine, and `Merchant_AdvanceAll` is
documented as *phase 6, step 1* (`docs/symbols.md`, verified). Ours should be at
least as explicit, because the phase list *is* the definition of the simulation's
order of operations, and D-7 depends on it being fixed.

### Snapshots

A whole-world snapshot once per turn is cheap at this scale and buys three
things at once: late join, crash recovery, and a place to stand when a desync
has to be diagnosed. Take it after resolution, alongside the turn's state hash.

---

## 6. Desync detection

In a lockstep design, this is the part that decides whether the whole approach
was a good idea. A desync detector that is added later will be added after the
first unexplained multi-hour debugging session.

### What we hash

A **canonical serialisation** of the simulation state — the same byte stream the
snapshot uses — never the in-memory image. A memory image includes struct
padding, `Vec` spare capacity and allocator-dependent layout, all of which differ
legitimately between peers and would produce false divergence reports. False
alarms are how a desync detector gets switched off.

### Which hash

**xxHash64**, either vendored into our own crate with the reference test vectors
or taken from `twox-hash` — which is MIT, has no build script, no mandatory
dependencies, and is the one crate in this space that *explicitly documents* the
property we need:

> The xxHash algorithms produce consistent output given consistent input. […]
> The output does not depend on the platform; 32- and 64-bit systems produce the
> same output, as do little- and big-endian systems. The Rust implementation is
> verified against the reference C implementation.

(That paragraph is new — it was added in `twox-hash` 2.1.4, whose changelog entry
is literally "Documentation added about the stability of the hashing
algorithms". Check it is still there before depending on it.)

FNV-1a is tempting because it is ten lines, and it was the first choice here, but
its avalanche behaviour is poor on exactly the kind of input we would feed it —
long runs of structured, mostly-zero records — and its own crate's README warns
it "performs badly on larger inputs". For a checksum whose whole job is that any
one-bit difference changes the output, that is the wrong trade for a few saved
lines.

**Three things to avoid, for three different reasons:**

- `std::collections::hash_map::DefaultHasher`. Its documentation says: *"The
  internal algorithm is not specified, and so it and its hashes should not be
  relied upon over releases."* Note the trap this is *not*:
  `DefaultHasher::new()` is deterministic within and across runs of the same
  binary — it is only unstable across **Rust versions**. That is what makes it
  dangerous, because it will pass every test you write and then diverge between
  two players on different toolchains.
- `ahash`. Its own README could not be plainer: *"aHash is not intended for
  network use or in applications which persist hashed values."* It is randomly
  seeded and deliberately free to improve its output over time.
- **Routing state through `#[derive(Hash)]` at all**, whichever hasher is
  underneath. `core`'s `Hash` documentation states that *"data fed by `Hash` to a
  `Hasher` should not be considered portable across platforms"* and *"the data
  passed by most standard library types should not be considered stable between
  compiler versions"* — `Vec` and `BTreeMap` hash their lengths in a
  platform-dependent way. So: hash **our canonical byte stream** with a oneshot
  API, and never let the `Hash` trait near a checksum.

None of this is a defect in those tools. They are built for hash tables, where
instability is a feature; for state checksums it is precisely inverted.

### Cadence

- **Battle:** compute a hash every tick. Exchange it every tick — it is 8 bytes
  in a packet we are already sending. Keep the last 256 in a ring buffer on each
  peer.
- **Kingdom:** compute and exchange once per turn, next to the snapshot.

Hashing full state at 10 Hz for a simulation of this size is affordable. If
profiling ever says otherwise, the fallback is to keep hashing every tick but
exchange every fourth — the ring buffer still localises the divergence to a
window.

### When it diverges

**Halt.** Do not try to resynchronise, do not let one peer "catch up" from the
other's state. Once the two simulations differ, everything either of them says
about the game is suspect, and continuing turns a reproducible bug into an
unreproducible one.

Both peers then write a **desync dump**:

- initial state and seed,
- the full command stream from the start of the session,
- the last 256 state hashes,
- the canonical state at the last agreed tick and at the diverged tick,
- per-subsystem hashes for the diverged tick (see below).

Two dumps plus the command stream are enough to reproduce the divergence offline,
on one machine, with no network.

### Localising it

In debug builds, hash subsystems separately — units, terrain, counties, PRNG
state, command queue — and include the vector rather than one number. The first
tick where exactly one subsystem hash differs names the subsystem, which converts
"the game desynced" into "the unit array diverged at tick 4,112" before anyone
opens a debugger. The PRNG state deserves its own slot: if it is the *only* thing
that differs, someone drew a random number outside the simulation, and if it
differs *along with* everything else, it is a consequence rather than the cause.

### The replay harness — the part that pays for itself

Because `step()` is pure (D-11), a session is fully described by
`(initial state, seed, command stream)`. So:

- Every session records that triple. It is small.
- A CI test replays a corpus of recorded sessions and asserts the final state
  hash matches the recorded one.
- Any refactor that introduces nondeterminism fails that test **on a single
  machine, with no second player**, which is the only reason it will actually be
  run often enough to help.

This is the same argument `docs/decisions.md` D7 makes about the retired
differential harness: the check that is a property of the data, rather than of
two implementations by the same author, is the one worth keeping. A replay is
that kind of check.

---

## 7. Transport

### Requirements

1. MIT-compatible licence (`docs/decisions.md` D5).
2. **No native build dependencies.** `cargo build` on
   `x86_64-pc-windows-msvc` must not need a C compiler, cmake, an assembler or
   perl. This is a standing property of the project — `l2-formats` and `l2-mods`
   are dependency-free by policy, with the reasoning recorded in their
   `Cargo.toml` and in `docs/modding.md`.
3. Reliable, ordered delivery of small messages. Lockstep has no use for an
   unreliable channel: a dropped command packet cannot be skipped, it must be
   waited for.
4. Up to 5 peers on the kingdom layer, 2 on the battle layer.
5. Reachable by two people on ordinary home connections.

### Recommendation: `std::net::TcpStream`, behind a `Transport` trait

For v1, plain TCP from the standard library. Not a placeholder — a considered
choice.

**Head-of-line blocking, TCP's usual disqualifier for games, costs us nothing
here.** The standard argument against TCP is that a lost packet stalls later
packets that the game could otherwise have used. In lockstep there *are* no later
packets it could have used: tick `N + 1` cannot be simulated before tick `N`
anyway. The stall TCP imposes is the stall the design already has. A UDP layer
would spend its effort delivering something out of order that we would then have
to put back in order and wait on.

**The volume is trivial.** A few commands per tick at 10 Hz between two peers.
Any transport built in the last thirty years handles this.

**Zero dependencies, zero build friction**, which is requirement 2 satisfied by
construction rather than by audit, and consistent with how the rest of this
project is built.

**What it costs, honestly:**

- **No NAT traversal.** The host must be reachable — port forwarding, or a LAN,
  or a VPN overlay. This is precisely what the original required too, and it is
  the single biggest usability gap in the design. No transport crate makes this
  problem disappear; UDP-based libraries make hole punching *possible*, but hole
  punching still needs a rendezvous server that somebody has to run and pay for.
  An MIT hobby project has no Steam Datagram Relay to fall back on.
- **Star topology.** With 5 players the host holds 4 connections and relays. That
  is more work for the host and one extra hop of latency for peer-to-peer
  traffic — irrelevant on the turn layer, and the battle layer is 2 players, so
  it never applies where it would matter.
- **Connection setup is manual**: an address and a port typed by a human.

**Design for replacement.** All of it sits behind one narrow trait:

```
trait Transport {
    fn send(&mut self, peer: PeerId, bytes: &[u8]) -> Result<()>;
    fn poll(&mut self) -> Option<(PeerId, Vec<u8>)>;
}
```

Message framing (a length prefix) belongs above the trait so that a datagram
implementation and a stream implementation present the same interface. Nothing
in §4–§6 may know which one it is talking to.

Two implementation details that are easy to get wrong and expensive to diagnose:

- **`set_nodelay(true)` on every socket.** Nagle's algorithm buffers small writes
  waiting for an ACK, which is precisely the wrong behaviour for one small
  packet every 100 ms — it can add most of a round trip to every tick and
  present as "the network is slow" when it is the local stack holding the data.
- **Sockets are read on their own thread**, handing complete messages to the
  simulation through a queue. The simulation must drain that queue at a fixed
  point in the tick and never ask "has anything arrived yet?" mid-tick: how much
  has arrived by any given instant is exactly the sort of scheduler-dependent
  value D-5 forbids from reaching `step()`.

### The alternatives, and why none of them wins today

The obvious crates were checked against requirements 1 and 2 rather than waved
at. Findings as of this writing — re-check before adopting anything, because
`docs/decisions.md` C8 is exactly about prior art being a lead rather than an
authority.

**`renet` — clean on the constraint, wrong on the topology.** MIT OR Apache-2.0,
actively maintained, and genuinely free of native build steps: no build script,
and its resolved dependency closure (26 crates) contains no `cc`, `cmake`,
`bindgen` or `nasm-rs` — its crypto is RustCrypto's pure-Rust
`chacha20poly1305`. Its channels (`Unreliable`, `ReliableOrdered`,
`ReliableUnordered`) map neatly onto what §4 needs.

The problem is its security model, not its code: `renet` inherits netcode 1.02's
assumptions, so a server declares `public_addresses` and clients present a
`ConnectToken` minted by **a separate trusted backend sharing a private key with
the game server**. That is a design for a publicly routable dedicated server plus
a token service — precisely the two things §8 says we are not running. Adopting
it would mean either standing up that infrastructure or bypassing the part that
justifies the dependency. It remains the best candidate *if* the project ever
grows hosting, and it is worth revisiting then. (Minor: it pulls `octets`, which
is BSD-2-Clause — permissive, but a third licence to attribute. And avoid
`renet_steam`, which links the native Steamworks SDK.)

**`laminar` — dead.** Last release May 2021, last commit 2023, built for the
now-archived Amethyst engine. Its published licence field is also the legacy
`MIT/Apache-2.0` slash form rather than a valid SPDX expression, which trips
licence tooling. Closed off.

**`message-io` — Apache-2.0 only**, with no MIT option, and semi-dormant. Not
disqualifying for an MIT project, but it drags Apache's attribution terms into
distribution for no compensating benefit.

**QUIC (`quinn`) — blocked, and more firmly than expected.** Multiplexed,
encrypted, handles connection migration; actively maintained; MIT OR Apache-2.0.
The QUIC crates themselves are clean — their build scripts do nothing but
`cfg_aliases!`. The blocker is the crypto provider:

- The default feature set pulls `ring`, which **requires a C compiler on Windows
  MSVC** (`cc` is a build-dependency and it compiles 17 shipped `.c` files).
  Worth correcting a widely repeated claim: it does *not* need NASM or perl any
  more — the crate ships pregenerated Windows objects.
- `aws-lc-rs` also requires a C compiler; NASM is avoidable via its
  `prebuilt-nasm` feature, which `quinn-proto` already enables.
- **No pure-Rust provider can drive QUIC at all right now.** Both
  `rustls-rustcrypto` (still 0.0.2-alpha, and its README opens with "DO NOT USE
  THIS IN PRODUCTION") and `rustls-graviola` declare `quic: None` on every TLS
  1.3 suite, and quinn requires that field.

So choosing QUIC means accepting a C compiler on every build machine — a direct
collision with requirement 2 — and the alternative is implementing
`quinn_proto::crypto::Session` against a pure-Rust stack ourselves, which is a
security-sensitive project rather than a feature flag. `rustls-graviola` is the
one to watch: it is genuinely native-build-free, using Rust inline assembly
rather than C, so if it ever grows QUIC support this paragraph should be
rewritten.

**Evidence basis for the above:** published `Cargo.toml`s, `build.rs` sources,
the fully resolved `Cargo.lock` shipped inside each `.crate` tarball, and the
maintainers' own requirement documents. A crate needing a C toolchain cannot
hide it — `cc` would appear in that lock. What was *not* done is an actual
`cargo build` on `x86_64-pc-windows-msvc`, so these are strong static findings
rather than a compiled result. Verify by building before committing to any of
them.

**Port:** pick our own and document it. Do not reuse DirectPlay's 2300–2400 or
47624; nothing good comes of colliding with a service some players may still
have enabled.

---

## 8. What we deliberately do not do

- **No compatibility with the original's DirectPlay protocol.** We will never
  play against the 1996 binary. Every hour spent matching its wire format buys
  nothing. (This is the reason Appendix A is an appendix.)
- **No DirectPlay.** It is deprecated, it is an optional Windows feature that
  must be turned on by hand (`dism /online /Enable-Feature /FeatureName:DirectPlay
  /All`), and it does not exist off Windows at all. On this machine it still
  works once enabled — Appendix A gets a live session out of it — so the reason
  to drop it is obsolescence and portability, not breakage.
- **No rollback netcode.** §4.
- **No client-side prediction.** Input delay is the mechanism; prediction would
  require the rewindable state that rollback needs.
- **No dedicated server.** Peer-hosted, like the original. A dedicated server
  implies someone running it.
- **No interest management / area-of-interest culling.** The world is a 64×64
  grid and battles are 80×80. Everyone can afford to know everything.
- **No anti-cheat, and no attempt at hidden information integrity.** Lockstep
  puts full state on every peer, so a modified client can see through fog of war.
  This is a game played with people you chose to play with. Saying so plainly is
  better than implying a protection that does not exist.
- **No encryption or authentication in v1.** Sessions are between people who
  exchanged an address out of band. If this ever ships to strangers, it becomes
  a `Transport` concern and is solved there, not by sprinkling it through the
  simulation.
- **No matchmaking or lobby service.** The original had one; it is dead (Appendix
  A), which is a reasonably strong argument about the lifespan of such things.

---

## Appendix A — what the original did

Kept because it explains the decision to replace rather than wrap, and so nobody
re-investigates it. Everything here was measured on the GOG Windows build
(1,031,680 bytes, `ImageBase = 0x400000`, no ASLR) using the proxy in
`native/dplay-proxy/` and the scripts in `tools/net/`.

### The entry surface — verified

`Lords2.exe` imports exactly two functions from `DPLAYX.dll`, **by ordinal**,
with one call site each:

| IAT slot | Ordinal | Function | Call site |
|---|---|---|---|
| `0x005CF364` | 1 | `DirectPlayCreate` | `0x004B826E` (via thunk `0x004BF614`) |
| `0x005CF368` | 2 | `DirectPlayEnumerateA` | `0x004B7FCE` (via thunk `0x004BF60E`) |

Reproduce with `node tools/net/dpimports.js "<install>/Lords2.exe"`.

### The interface it asks for — verified

`0x004B8243` calls `DirectPlayCreate`, then `QueryInterface` for the IID stored
at `0x004D0058`, then releases the version-1 interface. That IID is
`{2B74F7C0-9154-11CF-A9CD-00AA006886E3}` = **`IID_IDirectPlay2`** — the *wide*
variant, despite the game being an ANSI application that calls
`DirectPlayEnumerateA`. It is the only DirectPlay IID referenced anywhere in
`.text`; `IID_IDirectPlay2A` and `IID_IDirectPlay` are present in `.rdata` but
dead. The resulting interface is stored at `0x004E2A2C`.

That combination looks like a bug in the original. `IDirectPlay2` and
`IDirectPlay2A` have identical vtables and differ *only* in whether the strings
in `DPSESSIONDESC2` and `DPNAME` are `WCHAR` or `char`. The game asks for the
wide interface and then passes ANSI bytes through it: the session name it handed
to `Open` read back correctly as the ANSI string `"l2probe"` (verified — the
proxy dumps it), which is not what a `WCHAR*` would contain. What DirectPlay
then makes of that name — and whether it is part of why session discovery is
unreliable — is **inferred, not established**; we never got two peers into one
session to find out.

`IDirectPlay2` has exactly 32 vtable slots. That is not taken from a header: it
was measured on this machine by walking `dplayx.dll`'s vtables and checking that
they abut (`tools/net/dptest.cpp`), giving 25 / 32 / 47 / 53 slots for
`IDirectPlay` / `2` / `3` / `4` — which then matched the headers exactly.

### The setup path — verified

The entire multiplayer setup UI is **standard Win32 dialogs**, not the game's
DirectDraw UI. `Net_MultiplayerSetup` at `0x004B7585` runs them in sequence
(`node tools/net/dlgdump.js` recovers the templates):

| Dialog | Caption | Controls |
|---|---|---|
| 129 | Connection method | listbox 1001, OK 1, Cancel 2 |
| 108 | Connect or Create a game? | Connect 1002, Create 1004, Cancel 2 |
| 116 | Enter a name for this session… | edit 1000, OK 1 |
| 130 | Select Session | listbox 1024, OK 1, Cancel 2 |

That is why a capture was possible at all: `docs/decisions.md` D8 records that a
fullscreen DirectDraw game cannot be seen or clicked from outside, but ordinary
dialogs accept posted messages.

### The observed call sequence — verified

Captured live from inside the process. Host side, TCP/IP provider:

```
DirectPlayEnumerateA(cb=0x004B82BF)          -> 3 providers
DirectPlayCreate(DPSPGUID_TCPIP)             -> hr=0, IDirectPlay
  QueryInterface(IID_IDirectPlay2)           -> hr=0
  Release  (the v1 interface)
IDirectPlay2::GetCaps(DPGETCAPS_GUARANTEED)
    -> dwFlags=0x00010060, maxBufferSize=1048547, maxPlayers=64,
       latency=500ms, headerLength=20, timeout=5000ms
IDirectPlay2::Open(DPOPEN_CREATE)
    -> dwFlags=0x00000044 (MIGRATEHOST|KEEPALIVE), dwMaxPlayers=5
IDirectPlay2::CreatePlayer(short="Nobleman", event=NULL, flags=0)
```

Joining peer: `GetCaps`, then `EnumSessions(DPENUMSESSIONS_AVAILABLE)`, then
`Open(DPOPEN_JOIN)`.

Three things in that are worth carrying forward:

- **`CreatePlayer` passes a NULL event handle**, so the original *polls*
  `Receive` rather than being woken. Our design polls too, but on a tick clock
  we chose rather than on whatever the render loop happened to do.
- **It asks for guaranteed-delivery caps**, which is consistent with a turn-based
  game that cannot tolerate a lost move — the same conclusion §7 reaches from
  first principles.
- `dwMaxPlayers = 5` corroborates the player count from the map data.

### What was *not* captured

**No `Send` or `Receive` traffic.** Reaching the point where the game exchanges
gameplay messages needs two peers in one session *and* the game driven through
its own DirectDraw new-game screens, which D8 blocks. A joining peer got as far
as `Open(DPOPEN_JOIN)` returning `DPERR_NOSESSIONS`. So the original's actual
**wire content is unknown**, and nothing in §1–§8 depends on knowing it.

### The `SNWValid.dll` gate — verified, and narrower than it first appeared

The GOG release ships neither `SNWValid.dll` nor `SierraNW.dll`, and neither is
in the import table. `0x004B64D8` `LoadLibraryA("SNWValid.dll")` then
`GetProcAddress("SNWValidate")`, and on failure raises the modal:

> **File load error** — SNWValid.dll not found in Windows system folder.

It is reached **only** when dialog 129 returns `1003`, which happens only when
the selected row's `LB_ITEMDATA` is `0` — and the only such row is the
`"Sierra Internet Gaming System"` entry the game appends after enumeration.

**Correction worth stating clearly: this does not gate DirectPlay.** Selecting
any of the three real service providers reaches `DirectPlayCreate` and a working
session, with `SNWValid.dll` never loaded — verified by capture above. What makes
it look like a hard gate is that the dialog's `WM_INITDIALOG` *pre-selects* the
Sierra row, so a player who opens the connection dialog and presses OK without
first clicking a provider lands on the dead matchmaking path every time.

Two consequences:

- For anyone trying to play the GOG release today: pick a connection method
  explicitly. The default is broken and always will be.
- For us: the original's DirectPlay path is not dead, merely deprecated,
  fragile, and dependent on an optional Windows component. That is still an
  ample reason to replace it — but the reason is obsolescence, not breakage.

### Tools left behind

| Path | What it does |
|---|---|
| `native/dplay-proxy/` | proxy `dplayx.dll`; forwards both ordinals and wraps the returned interface to log every vtable call |
| `tools/net/dpimports.js` | dumps a PE's `DPLAYX` imports and their call sites |
| `tools/net/enclosing.js` | finds the function containing an address, without Ghidra |
| `tools/net/dlgdump.js` | dumps `RT_DIALOG` resources and control IDs |
| `tools/net/dptest.cpp/.ps1` | exercises DirectPlay directly; measures vtable lengths |
| `tools/net/session.ps1` | launches the game and drives the setup dialogs by posted messages |

None of it is needed for the design above. It is kept because it builds cleanly
and because re-deriving it would cost a day.

> One caveat on the whole appendix: the captures were obtained by calling
> `Net_MultiplayerSetup` directly from an injected thread rather than by walking
> the game's menus. Anything the menu path would have initialised first was not
> initialised. The call sequence and the arguments are what the function does;
> whether a menu-driven run would differ is **not established**.
