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
//! * [`save`] is **our own** save format: a whole campaign as deterministic,
//!   versioned bytes, so a player can quit and resume. It writes through
//!   `l2_net::Canonical` rather than inventing a second encoder, and it refuses
//!   an unknown version rather than guessing at the layout. It takes and
//!   returns a `Vec<u8>` and never touches a file — reading the *original's*
//!   `.sav` is `l2-formats`' job, and importing one is `l2-scenario`'s.
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
//! 1. **§4.1's empire tax term is a table, and inference got it wrong.**
//!    §4.1 gives `dHapTax = (5 - taxRate) + realm.taxHapEmpire` and §2 gives
//!    `taxHapEmpire = sum of every owned county's +0x16`. `+0x16` cannot be
//!    `5 - rate` — the England turn-one fixture, four owned counties all at rate 0, would
//!    then store `dHapTax = 25` where §9 says `+5`. This crate inferred
//!    `min(5 - rate, 0)` from that, which reproduces the save and matches the
//!    manual, **and is wrong at 45 of the 51 rates.** `Tax_RecomputePreview`
//!    (`0x0044B80B`) writes `5 - rate` to a *different* field, `+0x0F`, and
//!    `+0x16` from [`tables::TAX_HAPPINESS_OTHER`]. The save agrees with both
//!    readings because every rate in it is 0. See
//!    [`tax::empire_contribution`] and `docs/kingdom.md` §4.1.
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
//!    England turn-one save's `season 4, year 1268, turn 1`; the prose does not follow
//!    from it. See [`Kingdom::start_new_game`].
//!
//! 4. **§7.3's `random/8` has no stated range** — *resolved, and this crate's
//!    guess was wrong.* It was the one constant here with no evidence behind
//!    it. The original's generator (`FUN_00404A46`) steps two 31-bit LFSRs and
//!    masks their output with `0x7F`, so `random` is 0..=127 and the jitter is
//!    **0..=15** — twice this crate's guess, and enough to cancel Spring's `+8`
//!    and Autumn's `+12` outright, which the guess was chosen to prevent. The
//!    same call also picks the county that gets the local swing, by a flat
//!    `& 0xF` that ignores the map's size and falls through to *"the county
//!    after last season's"* when it overshoots. See
//!    [`weather::WEATHER_JITTER_BOUND`] and [`weather::chosen_county`]. §7.3's
//!    `localModifier` (`FUN_00449D6E`) is still named and never traced.
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
//! 7. **§3.4 and §7.4 disagree on the industry order, and both are wrong.**
//!    §3.4's call list says *"wood, iron, stone, weapons"* and §7.4's table
//!    indexes them wood, iron, weapons, stone. The driver (`FUN_0044E852`)
//!    runs **weapons over every county first**, in its own loop, and then iron,
//!    stone and wood per county. That is not cosmetic: the blacksmith spends
//!    the *previous* season's ore, because this season's has not been mined
//!    yet. See [`tables::INDUSTRY_ORDER`].
//!
//! 8. **§4.3's admission is discharged, and this crate's guess was right.**
//!    §4.3 said the food-split fields reproduced for the unowned counties and
//!    not for the owned ones, and that it *"did not untangle which write
//!    survives"*. Reading the save through `l2-scenario` untangles it, and
//!    **all fourteen counties reproduce** — see
//!    `tests/reproduction.rs::the_ration_preview_reproduces_every_stored_food_field`.
//!
//!    County 1 is what settles it. It stores `dHapRation = -2` and
//!    `shownRation = +1`, which are the two calls disagreeing: the display copy
//!    is taken while happiness is computed, so the *first* call fed it at
//!    Normal, and the *second* — next season's preview — says Half. Feeding it
//!    at Normal on an all-grain split costs `DivCeil(417 - 74*5, 6) = 8` sacks,
//!    and the county stores none, so **the first call debits the store**. That
//!    is [`ration::apply`] spending and [`ration::preview`] not, exactly as
//!    written here.
//!
//!    §4.3 was also wrong about the numbers: it said the owned counties store
//!    `+0x17C = 3`. They store 0; the 3 is `rationAchieved` at `+0x15D`. And
//!    there are five of them, not four.
//!
//! 9. **The rules §12 lists as unknown have been traced**, and are no longer
//!    stubs. Each is documented where it is implemented, with the address it
//!    came from and what second source confirms it:
//!
//!    * the fourteen AI turn handlers — all named with their addresses in
//!      [`ai::AiStep`]; **four are implemented here** and the other ten drive
//!      armies, merchants, diplomacy and map tiles, which this crate does not
//!      own. One of the fourteen (step 8) is an *empty function* in the
//!      shipped binary;
//!    * the four `AI_SetTaxRates` ladders — [`tables::AiTable::tax_ladder_neutral`]
//!      and [`tables::AiTable::tax_ladders`];
//!    * the five bankruptcy stages — [`industry::BankruptcyAction`], each
//!      cross-checked against the `L2.eng` group its handler raises;
//!    * the efficiency ramp — [`industry::efficiency_ramp`] — and the
//!      `resourceLimit` term — [`industry::resource_limit`];
//!    * rows 1..3 of `g_aiGoldGrant`, and the second, smaller table
//!      [`tables::AI_GOLD_GRANT_SMALL`];
//!    * the event table — [`event::EVENT_DECK`], which is a 256-slot deck
//!      rather than a 24-entry table, with all 24 handlers in [`event`];
//!    * the history ring — [`kingdom::History`];
//!    * the ale and army happiness terms — [`happiness::buy_ale`] and
//!      [`happiness::raise_army`];
//!    * the job slot supplying `Grain_Sow`'s labour — slot 0, which is
//!      *"Grain farming"*, because the nine labour records are `L2.eng` group
//!      74's strings **1..9** and §7.4's whole job column is one too high.
//!      See [`tables::JOB_COUNT`].
//!
//! 10. **What is still a stub, and why.** These are named in the binary and
//!     not reproduced, rather than invented:
//!
//!     * `localModifier` (`FUN_00449D6E`), the per-county weather swing;
//!     * the sixth score input, realm `+0x4C` — the other five are identified
//!       in [`tables::SCORE_INPUT_OFFSETS`], and it carries the heaviest weight
//!       of the six;
//!     * the two denominators the weapons `resourceLimit` divides by
//!       (`FUN_0044F15B`);
//!     * the three AI farming styles `AI_ManageFields` dispatches into, and the
//!       per-lord castle and weapon ladders steps 6 and 12 read;
//!     * a fifth AI lord's personality record — see
//!       [`tables::AI_PERSONALITY_COUNT`], which is four;
//!     * the ten AI handlers whose state lives outside this crate.
//!
//! 11. **§9's five-stage chain reproduces exactly**, and `tests/reproduction.rs`
//!     now asserts it against `lastturn.sav` rather than against §9's prose:
//!     ration → health meter → health band → happiness → birth rate →
//!     population, twenty-six stored fields across all fourteen counties.
//!     Nothing had to be adjusted to make it land.
//!
//!     The save reaches this crate through `l2-scenario`, which is the only
//!     crate allowed to know both a file format and a simulation. **Nothing in
//!     `src/` opens a file or knows one exists**, and `l2-formats` is a
//!     dev-dependency of the tests alone. §9 itself carried the same invented
//!     scenario the old test did — four counties owned by one realm — and both
//!     have been corrected to the five the file holds.
//!
//! 12. **Two more layout errors, found by reading the bytes.**
//!     `g_healthBandLadder` is five `{threshold, band}` pairs and §4.2's
//!     *"else 4"* is an explicit `(100, 4)` row; `g_castleWorkforce` is two
//!     ints per castle level rather than one. Both are confirmed by the
//!     addresses either side of them closing exactly. See
//!     [`tables::HEALTH_BAND_LADDER`] and [`tables::CASTLE_WORKFORCE`].
//!
//! 13. **`Score_RankRealms` is not in the `Season_Advance` pipeline** that
//!     §3.4 lists it in, and realm `+0x04` is a strength count rather than the
//!     `inPlay` flag §2 calls it. See [`phase::SEASON_PIPELINE`] and
//!     [`ai::begin_realm_turn`].

pub mod ai;
pub mod battle;
pub mod conquest;
pub mod county;
pub mod event;
pub mod field;
pub mod happiness;
pub mod health;
pub mod industry;
pub mod kingdom;
pub mod labour;
pub mod land;
pub mod levy;
pub mod map;
pub mod math;
pub mod merchant;
pub mod mercenary;
pub mod movement;
pub mod phase;
pub mod population;
pub mod ration;
pub mod realm;
pub mod report;
pub mod save;
pub mod tables;
pub mod tax;
pub mod territory;
pub mod unit;
pub mod units_tick;
pub mod unrest;
pub mod victory;
pub mod weather;

pub use county::{ChangeReason, County, Industry, MAX_COUNTIES, MAX_COUNTY_ID, MAX_FIELDS};
pub use event::EventKind;
pub use kingdom::{Kingdom, Options};
pub use phase::{Pass, Phase, PhaseTick, PhaseWait, TurnMachine, PHASE_ORDER, SEASON_PIPELINE};
pub use levy::{Levy, LevyBasket, LevyRefusal};
pub use map::{CampaignMap, CostMap, MAP_DIM, MAP_TILES};
pub use mercenary::{Band, MercenaryBands, MERCENARY_BANDS};
pub use movement::Routing;
pub use realm::{Realm, AI_STEP_DONE, MAX_REALMS};
pub use report::{Message, SeasonReport};
pub use victory::{Ending, Outcome, OutcomeStep, Ranking};
pub use tables::{Commodity, Season, Weather};
pub use unit::{Mercenaries, TroopType, Unit, UnitKind, Units, MAX_UNITS};

/// The generator this crate draws from, re-exported so a caller does not have
/// to depend on `l2-net` to seed a kingdom.
pub use l2_net::Pcg32;
