//! Deterministic kingdom simulation for Lords of the Realm II — counties,
//! seasons, population, food, happiness, taxation, crops, livestock, industry,
//! castles and wages.
//!
//! Reimplemented from `docs/kingdom.md`, which reads the model out of the
//! original binary. Where `l2-sim` covers what happens when two armies meet,
//! this covers everything that decides *which* armies exist. This crate is our
//! own code implementing documented behaviour; nothing is copied from the
//! original.
//!
//! # Where to start
//!
//! * [`Kingdom`] is the whole state and the season driver.
//! * [`phase::SEASON_PIPELINE`] is the end-of-season order, as data.
//!   `docs/kingdom.md` §3.4 is explicit that **the order is the rule**, so it
//!   is an array a test can assert against rather than a sequence of calls.
//! * [`tables`] holds every constant, each carrying the address it was read
//!   from. `docs/decisions.md` C11: no kingdom rule is loaded from a game data
//!   file, so our engine has to carry the whole ruleset itself.
//!
//! # Determinism
//!
//! Lockstep networking requires two machines running the same commands to reach
//! bit-identical state (`docs/netcode.md`). Four rules follow, and this crate
//! holds to all four:
//!
//! * **No floating point anywhere.** Integer arithmetic only, through
//!   [`math::pct`] and [`math::div_ceil`], which round exactly the way the
//!   original's C rounds.
//! * **No iteration in hash order.** Counties and realms are fixed arrays
//!   walked by ascending index; there is no `HashMap` in the crate.
//! * **No dependence on addresses or allocation.** Nothing branches on a
//!   pointer.
//! * **One generator, and it is frozen in-tree.** The two rules that draw a
//!   random number — the weather accumulator and the random-event roll — use
//!   `l2_net::Pcg32`, and both draw the *same number of times* regardless of
//!   how many counties there are or who owns them, so the stream cannot
//!   diverge on a difference the two peers already disagree about.
//!
//! # Errata: where `docs/kingdom.md` is wrong, ambiguous, or unimplementable
//!
//! Implementing a document is the only way to find out whether it is true.
//! These are the places this crate could not simply follow it. Each is repeated
//! at the code that deals with it.
//!
//! 1. **§4.1's empire tax term cannot be read literally.** §4.1 gives
//!    `dHapTax = (5 - taxRate) + realm.taxHapEmpire` and §2 gives
//!    `taxHapEmpire = sum of every owned county's +0x16`. If `+0x16` were the
//!    obvious `5 - rate`, the shipped save — four owned counties, every rate 0
//!    — would give `taxHapEmpire = 20` and `dHapTax = 25`. §9 says
//!    `shownTax = +5`. Only the **negative part** of `5 - rate` reproduces the
//!    save, and it is also the only reading that matches the manual's
//!    description of what the term is for. See [`tax::empire_contribution`].
//!
//! 2. **§4.3's ration loop cannot start where it says.** *"descends from
//!    `rationWanted + 1`"* would let §4.3's own worked example be fed at
//!    Double — which its herd affords, and which would slaughter 58 head where
//!    the save stores 13. The loop must be *entered* at `wanted + 1` and first
//!    *evaluated* at `wanted`, which is what a `do { level--; } while` does.
//!    See [`ration::choose`].
//!
//! 3. **§3.3's prose contradicts §3.3's own code.** The code rolls the year
//!    when `ended == 4`, i.e. in the same `Season_Advance` call that begins
//!    Winter, so the year label runs **Winter, Spring, Summer, Autumn**. The
//!    prose says *"the year runs Spring → Winter"*. The code reproduces the
//!    shipped save's `season 4, year 1268, turn 1`; the prose does not follow
//!    from it. See [`Kingdom::start_new_game`].
//!
//! 4. **§7.3's `random/8` has no stated range**, and the term is meaningless
//!    without one — C's `rand()` would give a jitter of 0..4095 against a
//!    seasonal push of 8..24. [`weather::WEATHER_JITTER_BOUND`] is this
//!    crate's own choice and the one constant here that is an invention.
//!    §7.3's `localModifier` is likewise named and never traced.
//!
//! 5. **§6's AI unrest ladder has a hole.** *"happiness >= 41 resets it to 0;
//!    11 … 40 walks it down; below 1 walks it up"* says nothing about 1..=10.
//!    Reproduced literally, as a dead band, in [`unrest`].
//!
//! 6. **§5's `deaths = pop` on a county that dies out stores a negative death
//!    count.** The expression is quoted from decompiled C and is almost
//!    certainly a lost negation or a `popLast`; it is reproduced as written and
//!    flagged rather than silently corrected. See [`population::update_one`].
//!
//! 7. **§3.4 and §7.4 disagree on the industry order.** §3.4's call list says
//!    *"wood, iron, stone, weapons"*; §7.4's table indexes them wood, iron,
//!    weapons, stone. Nothing resolves it. It changes no documented outcome —
//!    wood and iron precede weapons either way — and this crate uses §7.4's
//!    order.
//!
//! 8. **§4.3's own admission stands.** The food-split fields reproduce for the
//!    ten unowned counties and not for the four owned ones, because
//!    `Ration_Apply` runs twice and the surviving write is next season's
//!    preview. This crate models that as [`ration::apply`] (spends) and
//!    [`ration::preview`] (does not), which reproduces the unowned case
//!    exactly and leaves the owned case exactly as unexplained as §4.3 left it.
//!
//! 9. **Several rules are named but not specified**, and are implemented as
//!    honest stubs rather than invented: the fourteen AI turn handlers (§3.2,
//!    §12), the four `AI_SetTaxRates` ladders (§8.2), the five bankruptcy
//!    stages (§2, `+0x158`), the `Industry_Produce` efficiency ramp (§7.4,
//!    §12), the `resourceLimit` term of the industry formula (§7.4), rows 1..3
//!    of `g_aiGoldGrant` (§8.2), the contents of the 24-entry event table
//!    (§8.1), the history ring (§3.4), the ale and army happiness terms (§12),
//!    and the job slot that supplies `Grain_Sow`'s labour (§7.1).
//!
//! 10. **§9's five-stage chain reproduces exactly**, and is asserted in
//!     `tests/reproduction.rs`: ration → health meter → health band →
//!     happiness → birth rate → population, on live data from a real game with
//!     no free parameters. Nothing had to be adjusted to make it land.

pub mod ai;
pub mod county;
pub mod event;
pub mod happiness;
pub mod health;
pub mod industry;
pub mod kingdom;
pub mod land;
pub mod math;
pub mod phase;
pub mod population;
pub mod ration;
pub mod realm;
pub mod report;
pub mod tables;
pub mod tax;
pub mod unrest;
pub mod weather;

pub use county::{ChangeReason, County, Industry, MAX_COUNTIES, MAX_COUNTY_ID, MAX_FIELDS};
pub use event::EventKind;
pub use kingdom::{Kingdom, Options};
pub use phase::{Pass, Phase, PhaseTick, PhaseWait, TurnMachine, PHASE_ORDER, SEASON_PIPELINE};
pub use realm::{Realm, AI_STEP_DONE, MAX_REALMS};
pub use report::{Message, SeasonReport};
pub use tables::{Commodity, Season, Weather};

/// The generator this crate draws from, re-exported so a caller does not have
/// to depend on `l2-net` to seed a kingdom.
pub use l2_net::Pcg32;
