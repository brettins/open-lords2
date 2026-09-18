//! * [`tables`] holds every constant, each carrying the address it was read
//!   from. `docs/decisions.md` C11: no kingdom rule is loaded from a game data
//!   file, so our engine has to carry the whole ruleset itself.
//!
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
//! 4. **§7.3's `random/8` has no stated range** — *resolved, and this crate's
//!    guess was wrong.* It was the one constant here with no evidence behind
//!    it. The original's generator (`FUN_00404A46`) steps two 31-bit LFSRs and
//! masks their output with `0x7F`.
//!
//!    **0..=15** — twice this crate's guess, and enough to cancel Spring's `+8`
//!    and Autumn's `+12` outright, which the guess was chosen to prevent. The
//!    same call also picks the county that gets the local swing, by a flat
//!    `& 0xF` that ignores the map's size and falls through to *"the county
//!    after last season's"* when it overshoots. See
//!    [`weather::WEATHER_JITTER_BOUND`] and [`weather::chosen_county`]. §7.3's
//!    `localModifier` (`FUN_00449D6E`) is traced too now — a climate band cut
//!    out of the county's *index*, applied in Summer and Winter only, with a
//!    hole at band 3 that is `docs/bugs.md` B92. See
//!    [`weather::local_modifier`] and [`weather::climate_band`].
//!
//!    `docs/decisions.md` C90.
//!
//!    §3.4's call list says *"wood, iron, stone, weapons"* and §7.4's table
//!    indexes them wood, iron, weapons, stone. The driver (`FUN_0044E852`)
//!    runs **weapons over every county first**, in its own loop, and then iron,
//!    stone and wood per county. That is not cosmetic: the blacksmith spends
//!    the *previous* season's ore, because this season's has not been mined
//!    yet. See [`tables::INDUSTRY_ORDER`].
//!
//!    §4.3 was also wrong about the numbers: it said the owned counties store
//!    `+0x17C = 3`. They store 0; the 3 is `rationAchieved` at `+0x15D`. And
//!    there are five of them, not four.
//!
//!      — steps 4, 7, 9, 10 and 11 — in [`ai_army`]. One of the fourteen
//!      (step 8) is an *empty function* in the shipped binary; the two that
//!      remain are the diplomacy pair, which needs the inbox and its seven
//!      reply handlers;
//!    * the four `AI_SetTaxRates` ladders — [`tables::AiTable::tax_ladder_neutral`]
//!      and [`tables::AiTable::tax_ladders`];
//!    * the five bankruptcy stages — [`industry::BankruptcyAction`], each
//!      cross-checked against the `L2.eng` group its handler raises;
//! * the efficiency ramp — [`industry::efficiency_ramp`] — and the
//!      `resourceLimit` term — [`industry::resource_limit`];
//! * rows 1..3 of `g_aiGoldGrant`.
//!, with all 24 handlers in [`event`];
//!    * the history ring — [`kingdom::History`];
//!    * the ale and army happiness terms — [`happiness::buy_ale`] and
//!      [`happiness::raise_army`];
//!    * the job slot supplying `Grain_Sow`'s labour — slot 0, which is
//!      *"Grain farming"*, because the nine labour records are `L2.eng` group
//!      74's strings **1..9** and §7.4's whole job column is one too high.
//!
//!     * `localModifier` (`FUN_00449D6E`), the per-county weather swing;
//!     * the sixth score input, realm `+0x4C` — the other five are identified
//!       in [`tables::SCORE_INPUT_OFFSETS`], and it carries the heaviest weight
//!       of the six;
//!     * the two denominators the weapons `resourceLimit` divides by
//!       (`FUN_0044F15B`);
//!     * a fifth AI lord's personality record — see
//!       [`tables::AI_PERSONALITY_COUNT`], which is four;
//!     * personality `+0x2C` and `+0x6C`, the last two fields of that record
//!       with no reader — `docs/diplomacy.md` §8.4;
//!     * **`l2_kingdom::diplomacy` itself.** AI steps 1 and 2 are the only two
//!       of the fourteen this crate does not run, and four fields
//!       [`ai_army`] reads have no other writer — so the raid (step 10) and
//!       the *assist ally* mission are implemented, dispatched and
//!       **unreachable in a played game**. `docs/decisions.md` C62 states it,
//!       and `crates/l2-game/tests/ai_war/main.rs` holds a test written to go red
//!       the day it stops being true;
//!     * `Transport_Deliver`, and with it the goods evacuation in AI step 7's
//!       third pass — see [`ai_army::Evacuation`];
//!     * `Army_BeginSiege` from the mover: an army sent at somebody else's
//!       castle arrives and stops. The garrison half of the same tile is
//!       [`Kingdom::garrison_army`]; `docs/decisions.md` C61.
//!
//! 13. **`Score_RankRealms` is not in the `Season_Advance` pipeline** that
//! §3.4 lists it in, and realm `+0x04` is a strength count
//!     `inPlay` flag §2 calls it. See [`phase::SEASON_PIPELINE`] and
//!     [`ai::begin_realm_turn`].

pub mod ai;
pub mod ai_army;
pub mod ai_farm;
pub mod arrival;
pub mod battle;
pub mod conquest;
pub mod county;
pub mod diplomacy;
pub mod divide;
pub mod event;
pub mod explore;
pub mod field;
pub mod field_playlist;
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
pub mod mob;
pub mod movement;
pub mod phase;
pub mod population;
pub mod ration;
pub mod realm;
pub mod report;
pub mod save;
pub mod siege;
pub mod supply;
pub mod tables;
pub mod tax;
pub mod territory;
pub mod trade;
pub mod unit;
pub mod units_tick;
pub mod unrest;
pub mod victory;
pub mod weather;

pub use county::{ChangeReason, County, Industry, MAX_COUNTIES, MAX_COUNTY_ID, MAX_FIELDS};
pub use diplomacy::{Diplomacy, InboxSlot, Kind as DiploKind, Letter, INBOX_SLOTS};
pub use divide::{DisbandRefusal, SplitBasket, SplitInto, SplitRefusal};
pub use event::EventKind;
pub use kingdom::{Kingdom, Options};
pub use phase::{Pass, Phase, PhaseTick, PhaseWait, TurnMachine, PHASE_ORDER, SEASON_PIPELINE};
pub use levy::{Levy, LevyBasket, LevyRefusal};
pub use map::{CampaignMap, CostMap, MAP_DIM, MAP_TILES};
pub use mercenary::{Band, MercenaryBands, MERCENARY_BANDS};
pub use merchant::MerchantRoutes;
pub use movement::Routing;
pub use realm::{Realm, AI_STEP_DONE, MAX_REALMS};
pub use siege::{Engine, EngineBuild, SiegeCursor, SiegeRefusal};
pub use report::{Message, SeasonReport};
pub use victory::{Ending, Outcome, OutcomeStep, Ranking};
pub use tables::{Commodity, Season, Weather};
pub use unit::{Mercenaries, TroopType, Unit, UnitKind, Units, MAX_UNITS};

pub use l2_net::Pcg32;

/// `docs/decisions.md` C62.
pub use l2_net::{Quirk, Quirks};
