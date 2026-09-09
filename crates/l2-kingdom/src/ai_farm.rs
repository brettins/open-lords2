//! The AI's **five farming styles** — how an AI lord lays out a county's twenty
//! fields, sets its rations, and splits its workforce.
//!
//! # The correction this module exists to carry
//!
//! `crates/l2-kingdom/src/ai.rs` used to say that AI turn step 5 is
//! `AI_ManageFields` at `0x0049DD01`, dispatching into *"one of three labour
//! allocators"*. **Two names for two different functions had been collapsed into
//! one.** `[V]`, from the corpus:
//!
//! | address | function | its only callers | dispatches on | into |
//! |---|---|---|---|---|
//! | `0x0049DD01` | `Ai_ManageCountyFarms(realm)` | AI turn **step 5**, and `Ai_ManageFarmsAll` at the head of `Season_Advance` | county `+0x1FE`, styles **0, 1, 9** | `0x004A4052`, `0x004A42E3`, `0x004A440F` |
//! | `0x0049DFC6` | `AI_ManageFields(realm)` | **one** caller: `AI_ManageFields(0)`, turn phase 1 step 2 — the **unowned** counties | county `+0x1FE`, styles **0, 1** | `0x004A3C67`, `0x004A3ED3` |
//!
//! So there are **five** allocators, not three; `FUN_004A3C67` is the *neutral*
//! counties' grain style and **no AI realm ever reaches it**; and the two
//! outer functions differ in more than their name — `AI_ManageFields` also
//! zeroes county `+0x1B0` on the way in, which `Ai_ManageCountyFarms` does not.
//!
//! All five are implemented here, plus `FUN_004A4694` (the cattle-field nudge)
//! and `FUN_004A4782` (the AI's ration setter), and [`manage_county_farms`] and
//! [`manage_neutral_fields`] are the two outer passes.
//!
//! # What the five actually do differently
//!
//! Every style runs the same skeleton — *top up the larder, set the industry
//! share, take the default labour shares, set rations, re-lay the fields,
//! re-estimate* — and differs in six places. Laid side by side:
//!
//! | | `0x004A3C67` neutral 0 | `0x004A3ED3` neutral 1 | `0x004A4052` realm 0 | `0x004A42E3` realm 1 | `0x004A440F` realm 9 |
//! |---|---|---|---|---|---|
//! | sells its surplus first | no | no | **yes** | **yes** | **yes** |
//! | buys cattle when | — | herd < 11 | — | herd < **41** | herd < 11 |
//! | buys grain below | 100 | 100 | **600** | 100 | **300** |
//! | grain lots offered | 400/200/100/50 | 400/200/100/50 | 400/200/100 then 50/25 below 100 | **400 only** | 800/400/200/100/50/25 |
//! | industry share | **0** | **0** | **50** | **20** | **40** |
//! | ration split search | herd ≥ 101 ? high : low | low | herd ≥ 101 ? high : low | low | low |
//! | field layout | grain on **half**, one pasture | pasture on **all but one** | grain on **half**, one pasture | pasture on **all but one** | grain on a **third**, pasture up to a **third** |
//!
//! Read as behaviour: **style 0 is an arable lord**, style 1 is a **grazier**,
//! and style 9 splits the county in three and is the only one that farms both.
//! `AI_PERSONALITY_FARM_STYLE` is `[1, 1, 0, 9]`, so two of the four lords
//! graze, one ploughs and one mixes — and the mixed lord is also the one with
//! the most aggressive shopping list.
//!
//! # Five findings that fall out of reading them
//!
//! 1. **The fertility ladder's middle rung is unreachable.** Styles 0 and 9 size
//!    the Winter grain quota with
//!    `if (fertility < -20) n/2 - 1; else if (fertility < -50) n/2 - 2; else n/2`.
//!    Anything below −50 is already below −20, so the `-2` branch **never
//!    runs**: a ruined county gets the same one-field discount as a merely tired
//!    one. Reproduced rather than tidied — see [`winter_grain_quota`]. `[V]`,
//!    from the branch order in the decompilation of all three functions that
//!    carry it.
//! 2. **Turning *Advanced Farming* off makes the AI plant far more grain, not
//!    less.** The option's `else` limb overwrites the whole ladder: neutral 0
//!    plants `total - 3`, realm 0 plants `total - 1` (nearly every field), and
//!    style 9 plants `total / 2` instead of `total / 3`. `[V]`.
//! 3. **`Labour_DefaultSharesBuilt` (`0x0045158B`) is the AI's setter.**
//!    `docs/symbols.md` records of the two default-share functions that *"which
//!    of the two setters a county gets is not traced"*. All five styles call the
//!    *Built* one — farm 33/50/17, industry 40/15/15/15/15 with castle building
//!    favoured — and none calls `Labour_DefaultShares`. `[V]`.
//! 4. **The AI's ration ladder punishes a cattle county.** Of its two dairy
//!    rungs the Triple one can never change an answer, and the Double one only
//!    ever *lowers* the level: a county fed on cattle gets Double where a county
//!    fed on the same quantity of food as grain gets Triple. The arithmetic is
//!    in [`ration_wanted`]. `[V]`.
//! 5. **"Add a field" is a reclamation order, and one already running eats the
//!    quota.** `crate::tables::AI_FIELD_LADDER` reads as *"give the county
//!    another field"*; `FUN_0044C6C4` paints
//!    [`crate::field::terrain::RECLAIM_FIRST`] onto a **wasteland** tile, and a
//!    tile already reclaiming spends a place in the quota without anything
//!    happening — so a county told to add one while one is already under way
//!    adds nothing. This crate used to add one to `County::fields_fallow`
//!    instead, which `crate::field::recount` overwrites from the map on the very
//!    next pass: **the AI's field expansion has never actually happened.** See
//!    [`crate::field::order_reclamation`]. `[V]`.
//!
//! # The seams
//!
//! Two things these functions do are not this crate's state, and both are
//! **named seams rather than silent omissions**:
//!
//! * **The merchant.** Every style opens by buying food (`FUN_004A4B12` →
//!   `Merchant_Trade`), and the buys are *interleaved* with the tests that
//!   trigger them — `if (grain < 100) buy 400; if (grain < 100) buy 200;` — so
//!   whether a lot arrives changes whether the next one is even attempted. That
//!   cannot be a list of requests returned at the end; it has to be a callback,
//!   which is [`Market`]. [`NoMarket`] refuses everything and is what a caller
//!   with no stall yet passes.
//! * **`FUN_0049E39B`, the AI's selling pass**, which the three *realm* styles
//!   run first: it sells wood, iron and stone down to the lord's reserves at
//!   personality `+0x84`/`+0x88`/`+0x8C` and buys weapons of the county's type
//!   when gold clears `+0x78`. It is not implemented — it needs the goods
//!   prices, the six-weapon market ids and the realm's resource *wants* — and
//!   [`FarmStyle::sells_first`] records which styles want it so a caller that
//!   grows a market can hang it on the right three.

use crate::county::County;
use crate::field::{self, FieldType};
use crate::labour;
use crate::map::CampaignMap;
use crate::ration;
use crate::realm::Realm;
use crate::tables::{Season, Tables};

/// A good the AI's farming pass can buy, by `Merchant_Trade`'s good id.
///
/// The ids are `L2.eng` group 6 and the two the styles use are the two that
/// live in the county. See `docs/symbols.md` on `0x004284CE` for the full list
/// — 3 *Sheep* and 5 *Wool* have no branch in either direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Good {
    /// Good 1 — sacks, into [`County::grain`].
    Grain = 1,
    /// Good 2 — head, into [`County::herd`].
    Cattle = 2,
}

impl Good {
    pub fn id(self) -> u8 {
        self as u8
    }
}

/// The merchant seam — `FUN_004A4B12(county, qty, good)`.
///
/// The original checks the county has a stall (`+0x1A4`), prices the good from
/// the live stall marked up by the merchant's morale, and refuses the trade
/// unless the buyer's purse covers it: the realm's treasury for an owned
/// county, and county `+0x1F4` for an unowned one trading with itself.
///
/// An implementation must **move both sides** — credit [`County::grain`] or
/// [`County::herd`] and debit whichever purse — because the caller re-reads the
/// county's stock immediately afterwards to decide whether to try the next lot
/// down.
pub trait Market {
    /// Buy `qty` of `good` for this county. Returns whether the goods moved.
    fn buy(&mut self, county: &mut County, qty: i32, good: Good) -> bool;
}

/// A market that refuses every trade — what a caller with no merchant stall
/// passes. Every style still lays out its fields, sets its rations and splits
/// its labour; it just farms what it already has.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoMarket;

impl Market for NoMarket {
    fn buy(&mut self, _county: &mut County, _qty: i32, _good: Good) -> bool {
        false
    }
}

/// Everything the styles read that is neither the county nor the map.
#[derive(Debug, Clone, Copy)]
pub struct FarmEnv {
    /// `g_season`. Only Winter re-sows; see [`FarmStyle::sows_in_winter`].
    pub season: Season,
    /// `g_seasonNext`, which is what `County_RefreshEstimates` forecasts for.
    pub season_next: Season,
    /// `g_optAdvancedFarming`. **Off makes the AI plant more, not less** —
    /// finding 2 in the module documentation.
    pub advanced_farming: bool,
    /// `g_optArmiesEat`, which moves the ration ladder's denominator.
    pub armies_eat: bool,
}

impl FarmEnv {
    /// Both options on, which is the game's default.
    pub fn new(season: Season, season_next: Season) -> FarmEnv {
        FarmEnv { season, season_next, advanced_farming: true, armies_eat: true }
    }
}

/// The five allocators, named for what they are rather than for their number.
///
/// The stored style byte is county `+0x1FE`; [`FarmStyle::from_county_byte`]
/// resolves it the way each of the two outer passes does, which is **not the
/// same mapping**. Style 9 dispatches nowhere in the neutral pass — an unowned
/// county that used to belong to the Knight keeps his style byte and is then
/// left entirely alone by phase 1, which is a real behaviour and not a gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FarmStyle {
    /// `0x004A3C67` — style 0 in the **unowned** counties' pass.
    NeutralArable,
    /// `0x004A3ED3` — style 1 in the **unowned** counties' pass.
    NeutralGrazing,
    /// `0x004A4052` — style 0 for an **AI realm**.
    RealmArable,
    /// `0x004A42E3` — style 1 for an **AI realm**.
    RealmGrazing,
    /// `0x004A440F` — style 9 for an **AI realm**. Nothing reaches it in the
    /// neutral pass.
    RealmMixed,
}

impl FarmStyle {
    pub const ALL: [FarmStyle; 5] = [
        FarmStyle::NeutralArable,
        FarmStyle::NeutralGrazing,
        FarmStyle::RealmArable,
        FarmStyle::RealmGrazing,
        FarmStyle::RealmMixed,
    ];

    /// The address of the allocator in `Lords2.exe`.
    pub fn address(self) -> u32 {
        match self {
            FarmStyle::NeutralArable => 0x004A3C67,
            FarmStyle::NeutralGrazing => 0x004A3ED3,
            FarmStyle::RealmArable => 0x004A4052,
            FarmStyle::RealmGrazing => 0x004A42E3,
            FarmStyle::RealmMixed => 0x004A440F,
        }
    }

    /// The style byte `Ai_ManageCountyFarms` (`0x0049DD01`) dispatches on, for
    /// a county an **AI realm** holds. Styles other than 0, 1 and 9 fall
    /// through to no allocator at all.
    pub fn for_realm(style: u8) -> Option<FarmStyle> {
        match style {
            0 => Some(FarmStyle::RealmArable),
            1 => Some(FarmStyle::RealmGrazing),
            9 => Some(FarmStyle::RealmMixed),
            _ => None,
        }
    }

    /// The same byte in `AI_ManageFields(0)` (`0x0049DFC6`), the **unowned**
    /// counties' pass. Only 0 and 1 are dispatched there.
    pub fn for_neutral(style: u8) -> Option<FarmStyle> {
        match style {
            0 => Some(FarmStyle::NeutralArable),
            1 => Some(FarmStyle::NeutralGrazing),
            _ => None,
        }
    }

    /// Resolve `+0x1FE` for whichever of the two passes is running.
    pub fn from_county_byte(style: u8, owned_by_ai: bool) -> Option<FarmStyle> {
        if owned_by_ai {
            FarmStyle::for_realm(style)
        } else {
            FarmStyle::for_neutral(style)
        }
    }

    /// Whether this style runs `FUN_0049E39B` — the surplus sale and the weapon
    /// purchase — before it farms. The three realm styles do; the two neutral
    /// ones do not, because an unowned county has no realm to sell for.
    ///
    /// **Not implemented.** See the module documentation's second seam.
    pub fn sells_first(self) -> bool {
        matches!(self, FarmStyle::RealmArable | FarmStyle::RealmGrazing | FarmStyle::RealmMixed)
    }

    /// The percentage of the county's people this style hands to industry
    /// rather than to the farm — [`County::industry_share`], written before the
    /// labour shares are reset.
    ///
    /// This is the single largest difference between the styles, and it goes
    /// the way you would not guess: **the two arable styles put the most people
    /// into industry**, not into the fields, because grain needs a sower for a
    /// season and nothing after.
    pub fn industry_share(self) -> i32 {
        match self {
            FarmStyle::NeutralArable | FarmStyle::NeutralGrazing => 0,
            FarmStyle::RealmArable => 50,
            FarmStyle::RealmGrazing => 20,
            FarmStyle::RealmMixed => 40,
        }
    }

    /// Whether the style re-lays its grain in Winter. All but the two grazing
    /// styles do — they plant no grain at all.
    pub fn sows_in_winter(self) -> bool {
        matches!(self, FarmStyle::NeutralArable | FarmStyle::RealmArable | FarmStyle::RealmMixed)
    }
}

// ---------------------------------------------------------------------------
// The shopping list
// ---------------------------------------------------------------------------

/// One `if (stock < floor) buy(lot)` line of a style's opening cascade.
///
/// The cascade is **not** "buy the biggest lot you can afford". Each line
/// re-reads the stock, so a lot that arrives satisfies the floor and every
/// later line is skipped; a lot the purse refuses leaves the floor unmet and
/// the next, smaller lot is tried. The floors are per line, not per style:
/// [`FarmStyle::RealmArable`] tops up to **600** sacks with the three big lots
/// and only to **100** with the two small ones, so a lord who cannot afford 100
/// sacks still tries for 50 and 25.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyLine {
    /// The good, and therefore which stock the floor is read from.
    pub good: Good,
    /// Buy only while the stock is strictly below this.
    pub floor: i32,
    /// How much to ask for.
    pub lot: i32,
}

const fn grain(floor: i32, lot: i32) -> BuyLine {
    BuyLine { good: Good::Grain, floor, lot }
}

const fn cattle(floor: i32, lot: i32) -> BuyLine {
    BuyLine { good: Good::Cattle, floor, lot }
}

/// `0x004A3C67`'s cascade. The **unowned** county is handed 100 crowns of its
/// own money first (`+0x1F4 += 100`) so that it has something to trade with —
/// see [`neutral_purse_top_up`].
const BUYS_NEUTRAL_ARABLE: [BuyLine; 4] =
    [grain(100, 400), grain(100, 200), grain(100, 100), grain(100, 50)];

/// `0x004A3ED3`'s cascade: one herd top-up, then the same four grain lots.
const BUYS_NEUTRAL_GRAZING: [BuyLine; 5] =
    [cattle(11, 10), grain(100, 400), grain(100, 200), grain(100, 100), grain(100, 50)];

/// `0x004A4052`'s cascade — the two floors, 600 then 100.
const BUYS_REALM_ARABLE: [BuyLine; 5] =
    [grain(600, 400), grain(600, 200), grain(600, 100), grain(100, 50), grain(100, 25)];

/// `0x004A42E3`'s cascade — the shortest of the five, and the only one that
/// tops its herd up to **41** rather than 11.
const BUYS_REALM_GRAZING: [BuyLine; 2] = [cattle(41, 10), grain(100, 400)];

/// `0x004A440F`'s cascade — the longest, and the only one that offers a lot of
/// 800.
const BUYS_REALM_MIXED: [BuyLine; 7] = [
    cattle(11, 10),
    grain(300, 800),
    grain(300, 400),
    grain(300, 200),
    grain(300, 100),
    grain(300, 50),
    grain(300, 25),
];

impl FarmStyle {
    /// The style's opening cascade, in the order the original tries it.
    pub fn buys(self) -> &'static [BuyLine] {
        match self {
            FarmStyle::NeutralArable => &BUYS_NEUTRAL_ARABLE,
            FarmStyle::NeutralGrazing => &BUYS_NEUTRAL_GRAZING,
            FarmStyle::RealmArable => &BUYS_REALM_ARABLE,
            FarmStyle::RealmGrazing => &BUYS_REALM_GRAZING,
            FarmStyle::RealmMixed => &BUYS_REALM_MIXED,
        }
    }
}

/// `0x004A3C67`'s opening line: `if (grain < 100) county.+0x1F4 += 100`.
///
/// County `+0x1F4` is the **unowned county's own purse** — the thing
/// `Merchant_Trade` pays out of when the realm is 0. So a starving neutral
/// county is quietly given a hundred crowns each pass so that the cascade below
/// it has something to spend. Only [`FarmStyle::NeutralArable`] does it, which
/// is odd company for the one style whose lord does not exist.
///
/// This crate has no field for that purse — it is [`Market`]'s business — so
/// this reports the top-up rather than applying it, and an implementation with
/// a purse credits it before running the cascade.
pub const NEUTRAL_PURSE_TOP_UP: i32 = 100;

/// Whether this pass hands the county [`NEUTRAL_PURSE_TOP_UP`] before shopping.
pub fn neutral_purse_top_up(style: FarmStyle, county: &County) -> i32 {
    if style == FarmStyle::NeutralArable && county.grain < 100 {
        NEUTRAL_PURSE_TOP_UP
    } else {
        0
    }
}

fn stock(county: &County, good: Good) -> i32 {
    match good {
        Good::Grain => county.grain,
        Good::Cattle => county.herd,
    }
}

/// Run one style's cascade against a market. Returns how many lots moved.
pub fn run_buys(style: FarmStyle, county: &mut County, market: &mut dyn Market) -> i32 {
    let mut bought = 0;
    for line in style.buys() {
        if stock(county, line.good) < line.floor && market.buy(county, line.lot, line.good) {
            bought += 1;
        }
    }
    bought
}

// ---------------------------------------------------------------------------
// FUN_004A4782 — the AI's ration setter
// ---------------------------------------------------------------------------

/// `FUN_0044E6A3` — **not** `Food_Available`, and the difference is the whole
/// point of `crate::ration::food_available`'s warning.
///
/// ```c
/// f = 0;
/// if (herd  > 0) f  = herd  * g_dairyPerHead;   /* 5  */
/// if (herd  > 0) f += herd  * g_foodPerHead;    /* 10 */
/// if (grain > 0) f += grain * g_foodPerSack;    /* 6  */
/// ```
///
/// Three functions in the binary compute a superficially similar sum from
/// different fields: this one from the **stores** (`herd`, `grain`),
/// `Food_Available` (`0x0044E7B4`) from the per-season *caps*
/// `herdAvailable`/`grainAvailable`, and `FUN_0044E741` from the stores with no
/// slaughter term at all. Picking the wrong one is `docs/decisions.md` C3
/// waiting to happen; the AI's ration ladder reads **this** one.
pub fn food_in_store(t: &Tables, county: &County) -> i32 {
    let mut food = 0i64;
    if county.herd > 0 {
        food += county.herd as i64 * t.food.dairy_per_head as i64;
        food += county.herd as i64 * t.food.food_per_head as i64;
    }
    if county.grain > 0 {
        food += county.grain as i64 * t.food.food_per_sack as i64;
    }
    food.clamp(i32::MIN as i64, i32::MAX as i64) as i32
}

/// How the style searches for a ration split — the `param_2` of `FUN_004A4782`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitSearch {
    /// `0` — walk 0 … 100 upward keeping the best, so ties go to the **lowest**
    /// split: prefer grain, spare the herd.
    PreferGrain,
    /// `100` — walk 100 … 0 downward keeping the best, so ties go to the
    /// **highest** split: eat the herd.
    PreferHerd,
    /// Anything else — take the number as the split, search nothing.
    Fixed(i32),
}

/// Which search a style asks for. Only the two **arable** styles ever ask for
/// [`SplitSearch::PreferHerd`], and only when the county already has a hundred
/// head to spare: `herd < 101 ? 0 : 100`.
pub fn split_search(style: FarmStyle, county: &County) -> SplitSearch {
    match style {
        FarmStyle::NeutralArable | FarmStyle::RealmArable => {
            if county.herd < 101 {
                SplitSearch::PreferGrain
            } else {
                SplitSearch::PreferHerd
            }
        }
        _ => SplitSearch::PreferGrain,
    }
}

/// The ration level the AI *asks* for, from what is in store —
/// `FUN_004A4782`'s first half.
///
/// ```c
/// store = food_in_store(county);  dairy = herd * 5;  people = pop (+ garrisons);
/// if      (store < people / 2) 0        /* None    */
/// else if (store < people)     1        /* Quarter */
/// else if (store < people * 2) 2        /* Half    */
/// else if (dairy > people * 3) 5        /* Triple  — on cheese alone */
/// else if (dairy > people * 2) 4        /* Double  */
/// else if (store > people * 8) 5
/// else if (store > people * 6) 4
/// else                         3        /* Normal  */
/// ```
///
/// **The two `dairy` rungs sit above the two large `store` rungs, and reading
/// them carefully turns the obvious story inside out.** `store` counts the herd
/// twice, at 5 and at 10, so `store >= 3 * dairy` always. Therefore:
///
/// * `dairy > 3p` implies `store > 9p > 8p`, so the **Triple rung is
///   redundant** — wherever it fires, the `store > 8p` rung below would have
///   said 5 anyway. It never changes an answer.
/// * `dairy > 2p` implies only `store > 6p`. When the store is also over `8p`
///   the earlier rung wins with **4** where the store rung would have given
///   **5**. So the surviving effect of the dairy rungs is a *downgrade*: a
///   county whose food is mostly cattle is fed **Double** where a county with
///   the same quantity of food as grain is fed **Triple**.
///
/// `[V]` on the arithmetic, `[D]` on calling it a bug. It is the only place in
/// the ration rules where the *composition* of the larder changes the level
/// rather than only the price, and it costs the grazing lords a ration point.
pub fn ration_wanted(t: &Tables, county: &County, armies_eat: bool) -> i32 {
    let store = food_in_store(t, county);
    let dairy = ration::food_from_dairy(t, county.herd);
    let people = ration::people_to_feed(county, armies_eat);
    if store < people / 2 {
        0
    } else if store < people {
        1
    } else if store < people * 2 {
        2
    } else if dairy > people * 3 {
        5
    } else if dairy > people * 2 {
        4
    } else if store > people * 8 {
        5
    } else if store > people * 6 {
        4
    } else {
        3
    }
}

/// `FUN_004A4782` — set `rationWanted`, then find the `rationSplit` that feeds
/// the county best.
///
/// The search is a brute-force sweep of all 101 splits, scoring each by the
/// level `Ration_Apply` manages to achieve, and it keeps a strict improvement
/// only (`best < achieved`). That is what makes the direction matter: sweeping
/// upward keeps the **first** split that reaches the best level and sweeping
/// downward keeps the **last**, so the two modes are "the least herd that will
/// do" and "the most".
///
/// **A transcription note.** In the downward limb the original writes the
/// running best back into `rationSplit` *inside* the loop rather than after it.
/// It makes no difference — the next iteration overwrites it, and the last
/// iteration's write is the one that survives — so this is written the
/// straightforward way rather than reproducing a redundant store. `[D]`, and
/// the reason it is safe to tidy is that `Ration_Apply` reads `rationSplit`
/// only at the top of each iteration, after it has already been set to the
/// candidate.
///
/// The scan calls the crate's [`ration::choose`], which is pure, so nothing is
/// eaten while the AI is thinking. The original's `Ration_Apply` writes display
/// fields on every one of the 101 calls and this does not; the surviving state
/// is identical because the last call is repeated at the end either way.
pub fn set_rations(t: &Tables, county: &mut County, search: SplitSearch, armies_eat: bool) {
    county.ration_wanted = ration_wanted(t, county, armies_eat);
    let score = |county: &County, split: i32| -> i32 {
        let mut probe = county.clone();
        probe.ration_split = split;
        ration::choose(t, &probe, armies_eat).level
    };
    let chosen = match search {
        SplitSearch::Fixed(split) => split,
        SplitSearch::PreferGrain => {
            let mut best = 0;
            let mut chosen = 0;
            for split in 0..=100 {
                let achieved = score(county, split);
                if best < achieved {
                    best = achieved;
                    chosen = split;
                }
            }
            chosen
        }
        SplitSearch::PreferHerd => {
            let mut best = 0;
            let mut chosen = 100;
            for split in (0..=100).rev() {
                let achieved = score(county, split);
                if best < achieved {
                    best = achieved;
                    chosen = split;
                }
            }
            chosen
        }
    };
    county.ration_split = chosen;
    ration::preview(t, county, armies_eat);
}

// ---------------------------------------------------------------------------
// The field layout
// ---------------------------------------------------------------------------

/// The Winter grain quota, **including the unreachable rung**.
///
/// ```c
/// if      (fertility < -20) q = base - 1;
/// else if (fertility < -50) q = base - 2;   /* dead code */
/// else                      q = base;
/// ```
///
/// Finding 1 in the module documentation. `base` is `total / 2` for the two
/// arable styles and `total / 3` for the mixed one. The subtraction is done in
/// `unsigned` in the original and the result is passed to a signed parameter,
/// so a tiny county underflows to a negative quota and every grain field is
/// returned to fallow — which `i32` here reproduces exactly.
pub fn winter_grain_quota(fertility: i32, base: i32) -> i32 {
    if fertility < -20 {
        base - 1
    } else if fertility < -50 {
        base - 2
    } else {
        base
    }
}

/// `Labour_DefaultSharesBuilt` (`0x0045158B`) — the setter every farming style
/// uses. Farm 33/50/17, industry 40/15/15/15/15.
///
/// Written here rather than in [`crate::labour`] because it is the AI's choice
/// of the two, and because the shares are two independent groups each summing
/// to 100 — an invariant [`crate::county::County::labour_share`] documents and
/// this upholds.
pub fn default_shares_built(county: &mut County) {
    use crate::tables::{
        JOB_BLACKSMITH, JOB_CASTLE_BUILDING, JOB_CATTLE_FARMING, JOB_FIELD_RECLAMATION,
        JOB_GRAIN_FARMING, JOB_IRON_MINING, JOB_STONE_QUARRYING, JOB_WOOD_CUTTING,
    };
    county.labour_share[JOB_GRAIN_FARMING] = 33;
    county.labour_share[JOB_CATTLE_FARMING] = 50;
    county.labour_share[JOB_FIELD_RECLAMATION] = 17;
    county.labour_share[JOB_CASTLE_BUILDING] = 40;
    county.labour_share[JOB_IRON_MINING] = 15;
    county.labour_share[JOB_STONE_QUARRYING] = 15;
    county.labour_share[JOB_WOOD_CUTTING] = 15;
    county.labour_share[JOB_BLACKSMITH] = 15;
}

/// `FUN_004A4694` — nudge the pasture count by one, and only by one.
///
/// ```c
/// if (herdCrowding < 11) {
///     if (fieldsCattle > 2 && herd < 10) set pasture count to fieldsCattle - 1;
/// } else if (fieldsCattle < cap) {
///     set pasture count to fieldsCattle + 1;
/// }
/// recount; update crowding;
/// ```
///
/// So the herd's own crowding meter is the controller: **under 11 the county
/// gives a field back**, but only if it has more than two and fewer than ten
/// animals; at 11 or over it takes one more, up to `cap`. Both limbs clear
/// every pasture first and re-lay the whole count, which matters because
/// `set_count` fills in **slot order** — the pasture the county keeps after a
/// shrink is not necessarily the one it had.
///
/// The original's `else if` re-tests `10 < herdCrowding`, which is already
/// implied. Redundant, not a second condition; dropped here.
pub fn fit_cattle_fields(t: &Tables, county: &mut County, map: &mut CampaignMap, cap: i32) {
    if county.herd_crowding < 11 {
        if county.fields_cattle > 2 && county.herd < 10 {
            field::clear_type(county, map, FieldType::Pasture);
            field::set_count(county, map, FieldType::Pasture, county.fields_cattle - 1);
        }
    } else if county.fields_cattle < cap {
        field::clear_type(county, map, FieldType::Pasture);
        field::set_count(county, map, FieldType::Pasture, county.fields_cattle + 1);
    }
    field::recount(county, map);
    field::herd_update_crowding(t, county, map);
}

/// One farming style, start to finish — the body of one of the five.
///
/// The order is the rule and is reproduced exactly:
///
/// 1. the shopping cascade ([`run_buys`]);
/// 2. `total` is read **here**, from the county's cached field counts, before
///    anything is re-laid — so the quota below is sized on last pass's layout;
/// 3. the industry share, the default labour shares, and one allocation;
/// 4. rations;
/// 5. the layout, which differs per style;
/// 6. estimate / allocate / estimate.
///
/// Step 6's double round is not a fixpoint — see `crate::field`'s note on
/// `Field_SetType`. Style 9 is the odd one out and ends on
/// *estimate / allocate*, with **no closing estimate**; that is reproduced, not
/// an oversight, and it means a style-9 county's panel forecast is one
/// allocation stale.
///
/// # Why this takes the whole county array
///
/// It would read better taking one `&mut County`, and it cannot.
/// `County_RefreshEstimates` computes the **blacksmith's ceiling**, which is a
/// share of the realm's wood and iron split across *every staffed smithy the
/// realm owns* ([`crate::industry::weapon_shares`]) — so the estimate needs the
/// sibling counties, and it needs the owning realm's stockpile. And the share
/// moves **between** the two passes of step 6, because the allocation in the
/// middle is what staffs the smiths.
///
/// So this takes `counties` and an index, exactly as
/// [`crate::field::set_type`] does, and recomputes the share on each pass. The
/// tempting shortcut — hand the estimate a default [`crate::realm::Realm`] —
/// would zero every industry ceiling in every AI county, because
/// `Industry_LabourEstimate` tests the owner first and a fresh record owns
/// nothing. That is silently wrong in the direction nothing here tests for.
#[allow(clippy::too_many_arguments)]
pub fn lay_out(
    t: &Tables,
    style: FarmStyle,
    counties: &mut [County],
    county_count: usize,
    id: usize,
    map: &mut CampaignMap,
    realms: &[Realm],
    market: &mut dyn Market,
    env: &FarmEnv,
) {
    run_buys(style, &mut counties[id], market);

    let total = counties[id].field_total();

    counties[id].industry_share = style.industry_share();
    default_shares_built(&mut counties[id]);
    labour::allocate(&mut counties[id]);

    let search = split_search(style, &counties[id]);
    set_rations(t, &mut counties[id], search, env.armies_eat);

    let winter = env.season == Season::Winter;
    match style {
        FarmStyle::NeutralArable | FarmStyle::RealmArable => {
            // The arable pair clear every pasture up front, then plant grain on
            // half the county in Winter and keep exactly one pasture if there
            // is a herd worth pasturing.
            let county = &mut counties[id];
            field::clear_type(county, map, FieldType::Pasture);
            if winter {
                field::clear_type(county, map, FieldType::Grain);
                let quota = if env.advanced_farming {
                    winter_grain_quota(county.fertility, total / 2)
                } else if style == FarmStyle::NeutralArable {
                    total - 3
                } else {
                    total - 1
                };
                field::set_count(county, map, FieldType::Grain, quota);
            }
            if county.herd > 10 {
                field::set_count(county, map, FieldType::Pasture, 1);
                field::recount(county, map);
                field::herd_update_crowding(t, county, map);
            }
            refresh(t, counties, county_count, id, map, realms, env);
            labour::allocate(&mut counties[id]);
            refresh(t, counties, county_count, id, map, realms, env);
        }
        FarmStyle::NeutralGrazing | FarmStyle::RealmGrazing => {
            // The grazing pair plant no grain at all, ever: they clear it
            // whatever the season and hand the whole county but one field to
            // the herd, one field per pass.
            field::clear_type(&counties[id], map, FieldType::Grain);
            fit_cattle_fields(t, &mut counties[id], map, total - 1);
            refresh(t, counties, county_count, id, map, realms, env);
            labour::allocate(&mut counties[id]);
            refresh(t, counties, county_count, id, map, realms, env);
        }
        FarmStyle::RealmMixed => {
            if winter {
                let county = &mut counties[id];
                field::clear_type(county, map, FieldType::Grain);
                let quota = if env.advanced_farming {
                    winter_grain_quota(county.fertility, total / 3)
                } else {
                    total / 2
                };
                field::set_count(county, map, FieldType::Grain, quota);
            }
            fit_cattle_fields(t, &mut counties[id], map, total / 3);
            refresh(t, counties, county_count, id, map, realms, env);
            labour::allocate(&mut counties[id]);
        }
    }
}

/// One `County_RefreshEstimates` call, with the two things it reads that are
/// not the county: the **owning realm** and the realm's **weapon share**.
///
/// Both are recomputed at every call site rather than hoisted, because the
/// share depends on which smiths are staffed and [`lay_out`] re-allocates
/// between its two passes. `crate::field::set_type` recomputes it inside its
/// loop for the same reason.
///
/// An unowned county falls back to a default [`Realm`], which is correct
/// *only* for realm 0: `Industry_LabourEstimate` tests the owner first and
/// gives a county nobody owns a ceiling of 0 on all four industries, whatever
/// the record holds. That is why this looks the realm up rather than being
/// handed one.
fn refresh(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    id: usize,
    map: &CampaignMap,
    realms: &[Realm],
    env: &FarmEnv,
) {
    let owner = counties[id].owner;
    let share = crate::industry::weapon_shares(t, counties, county_count, owner);
    let neutral = Realm::new();
    let realm = realms.get(owner as usize).unwrap_or(&neutral);
    field::refresh_estimates(
        &mut counties[id],
        map,
        env.season_next,
        t,
        env.advanced_farming,
        realm,
        share,
    );
}

// ---------------------------------------------------------------------------
// The two outer passes
// ---------------------------------------------------------------------------

/// The field-reclamation ladder both outer passes open with —
/// `crate::tables::AI_FIELD_LADDER`, then [`field::order_reclamation`].
///
/// The ladder is an `if`/`else if` chain, so the **first** row whose *both*
/// conditions hold wins and the rest are skipped — which is why a two-field
/// county of 150 people falls through every row and gains nothing.
///
/// Returns how many wasteland tiles were actually started, which is **not** the
/// number the ladder asked for: the county may have no wasteland left, and a
/// field already under reclamation eats a place in the quota. See
/// [`field::order_reclamation`].
pub fn order_fields(county: &County, map: &mut CampaignMap) -> i32 {
    let total = county.field_total();
    for &(fields_below, population_above, n) in crate::tables::AI_FIELD_LADDER.iter() {
        if total < fields_below && county.population > population_above {
            return field::order_reclamation(county, map, n);
        }
    }
    0
}

/// `Ai_ManageCountyFarms` (`0x0049DD01`) — **AI turn step 5**, and also the
/// first thing `Season_Advance` does, through `Ai_ManageFarmsAll`
/// (`0x0049A990`), for every realm in play.
///
/// That second caller matters more than the first: it runs **ahead of the whole
/// economy**, so an AI's fields and labour split are already this season's when
/// tax, rations and industry read them. The human's are not.
///
/// For each county the realm holds: order fields, copy the lord's `farmStyle`
/// into the county, dispatch, and re-allocate labour. Returns the number of
/// fields ordered across the realm.
#[allow(clippy::too_many_arguments)]
pub fn manage_county_farms(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    map: &mut CampaignMap,
    realms: &[Realm],
    realm: u8,
    lord: u8,
    market: &mut dyn Market,
    env: &FarmEnv,
) -> i32 {
    let Some(style_byte) = t.ai_farm_style(lord) else { return 0 };
    let mut ordered = 0;
    for id in 1..=county_count.min(counties.len().saturating_sub(1)) {
        if counties[id].owner != realm {
            continue;
        }
        ordered += order_fields(&counties[id], map);
        counties[id].farm_style = style_byte;
        if let Some(style) = FarmStyle::for_realm(style_byte) {
            lay_out(t, style, counties, county_count, id, map, realms, market, env);
        }
        labour::allocate(&mut counties[id]);
    }
    ordered
}

/// `AI_ManageFields(0)` (`0x0049DFC6`) — **turn phase 1, step 2**: the same
/// treatment for every county nobody owns.
///
/// Two differences from [`manage_county_farms`] beyond the two-style dispatch:
/// it zeroes county `+0x1B0` on the way in, and it never *writes* the style
/// byte. So an unowned county farms itself according to whichever lord held it
/// last, and one that has always been free farms as style 0.
///
/// `+0x1B0` is not modelled in this crate — `docs/kingdom.md` has it as an
/// untraced flag that AI step 12 sets back to 1 — so the zeroing is recorded
/// here and not performed.
#[allow(clippy::too_many_arguments)]
pub fn manage_neutral_fields(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    map: &mut CampaignMap,
    realms: &[Realm],
    market: &mut dyn Market,
    env: &FarmEnv,
) -> i32 {
    let mut ordered = 0;
    for id in 1..=county_count.min(counties.len().saturating_sub(1)) {
        if counties[id].owner != 0 {
            continue;
        }
        ordered += order_fields(&counties[id], map);
        if let Some(style) = FarmStyle::for_neutral(counties[id].farm_style) {
            lay_out(t, style, counties, county_count, id, map, realms, market, env);
        }
        labour::allocate(&mut counties[id]);
    }
    ordered
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::terrain;
    use crate::map::CampaignMap;

    const T: &Tables = &Tables::DEFAULT;

    fn env() -> FarmEnv {
        FarmEnv::new(Season::Winter, Season::Spring)
    }

    /// Five realms, all in play, all AI. The counties below are given to realm
    /// 2 wherever the owner matters; the rest of the array exists because
    /// [`lay_out`] looks its county's owner up in it.
    fn realms() -> Vec<Realm> {
        let mut realms = vec![Realm::new(); crate::realm::MAX_REALMS];
        for (i, r) in realms.iter_mut().enumerate().skip(1) {
            r.in_play = true;
            r.lord = ((i - 1) % crate::tables::AI_PERSONALITY_COUNT + 1) as u8;
        }
        realms
    }

    /// [`lay_out`] over a **one-county world**, which is what every style test
    /// below wants: the county lands at index 1 of a two-slot array so the
    /// weapon share has a realm to sum over, and is copied back out.
    ///
    /// Passing the county alone is exactly what the signature no longer allows,
    /// and for a reason worth restating here rather than only at [`lay_out`]:
    /// the blacksmith's ceiling is a share of the realm's stockpile divided
    /// across its staffed smithies, so it is not a one-county quantity.
    fn lay(style: FarmStyle, c: &mut County, map: &mut CampaignMap, e: &FarmEnv) {
        let mut counties = vec![County::new(), c.clone()];
        lay_out(T, style, &mut counties, 1, 1, map, &realms(), &mut NoMarket, e);
        *c = counties.remove(1);
    }

    /// A county with `n` field tiles laid along one row, all fallow.
    fn county_with(n: usize) -> (County, CampaignMap) {
        let mut c = County::new();
        let mut map = CampaignMap::empty();
        for i in 0..n {
            let tile = crate::map::index(i as u8, 8);
            c.set_field_tile(i, Some(tile));
            map.terrain[tile] = terrain::FALLOW;
        }
        c.population = 500;
        crate::field::recount(&mut c, &map);
        (c, map)
    }

    fn counts(county: &County, map: &CampaignMap) -> (i32, i32, i32) {
        let mut probe = county.clone();
        crate::field::recount(&mut probe, map);
        (probe.fields_grain, probe.fields_cattle, probe.fields_fallow)
    }

    /// A market that records every request and grants the ones it is told to.
    struct Ledger {
        asked: Vec<(i32, Good)>,
        grant: bool,
    }

    impl Market for Ledger {
        fn buy(&mut self, county: &mut County, qty: i32, good: Good) -> bool {
            self.asked.push((qty, good));
            if !self.grant {
                return false;
            }
            match good {
                Good::Grain => county.grain += qty,
                Good::Cattle => county.herd += qty,
            }
            true
        }
    }

    fn ledger(grant: bool) -> Ledger {
        Ledger { asked: Vec::new(), grant }
    }

    // --- the five, and that they really are five ---------------------------

    /// **The correction this module was written for.** Five allocators, two
    /// outer passes, and the neutral pass reaches neither the realm styles nor
    /// style 9.
    #[test]
    fn there_are_five_allocators_and_the_two_passes_reach_different_ones() {
        assert_eq!(FarmStyle::ALL.len(), 5);
        // Every one has a distinct address in the binary.
        let mut addrs: Vec<u32> = FarmStyle::ALL.iter().map(|s| s.address()).collect();
        addrs.sort_unstable();
        addrs.dedup();
        assert_eq!(addrs.len(), 5, "five distinct allocators");

        assert_eq!(FarmStyle::for_realm(0), Some(FarmStyle::RealmArable));
        assert_eq!(FarmStyle::for_realm(1), Some(FarmStyle::RealmGrazing));
        assert_eq!(FarmStyle::for_realm(9), Some(FarmStyle::RealmMixed));
        assert_eq!(FarmStyle::for_realm(2), None, "no other byte dispatches");

        assert_eq!(FarmStyle::for_neutral(0), Some(FarmStyle::NeutralArable));
        assert_eq!(FarmStyle::for_neutral(1), Some(FarmStyle::NeutralGrazing));
        assert_eq!(
            FarmStyle::for_neutral(9),
            None,
            "an ex-Knight county is left alone by the neutral pass"
        );
    }

    /// `FUN_004A3C67` is the **neutral** pass's style 0 and no AI realm can
    /// reach it. This is the briefing error, asserted.
    #[test]
    fn the_neutral_arable_style_is_unreachable_from_an_ai_realm() {
        assert_eq!(FarmStyle::NeutralArable.address(), 0x004A3C67);
        for byte in 0..=255u8 {
            assert_ne!(
                FarmStyle::for_realm(byte),
                Some(FarmStyle::NeutralArable),
                "byte {byte}"
            );
        }
    }

    /// Every one of the four lords' styles resolves, and between them they use
    /// all three realm allocators.
    #[test]
    fn the_four_lords_between_them_use_every_realm_allocator() {
        let mut seen = Vec::new();
        for lord in 1..=crate::tables::AI_PERSONALITY_COUNT as u8 {
            let byte = T.ai_farm_style(lord).expect("lord {lord} has a record");
            let style = FarmStyle::for_realm(byte).expect("and a live allocator");
            seen.push(style);
        }
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(
            seen,
            vec![FarmStyle::RealmArable, FarmStyle::RealmGrazing, FarmStyle::RealmMixed]
        );
        assert_eq!(T.ai_farm_style(crate::realm::LORD_HUMAN), None);
    }

    // --- the shopping cascade ----------------------------------------------

    /// The cascade is **not** "the biggest affordable lot". A refused lot lets
    /// the next one be tried; an accepted one ends the run.
    #[test]
    fn a_lot_that_arrives_stops_the_cascade_and_one_refused_does_not() {
        let (mut c, _) = county_with(4);
        c.grain = 0;
        let mut m = ledger(true);
        run_buys(FarmStyle::NeutralArable, &mut c, &mut m);
        assert_eq!(m.asked, vec![(400, Good::Grain)], "the first lot satisfied the floor");
        assert_eq!(c.grain, 400);

        let (mut c, _) = county_with(4);
        c.grain = 0;
        let mut m = ledger(false);
        run_buys(FarmStyle::NeutralArable, &mut c, &mut m);
        assert_eq!(
            m.asked,
            vec![(400, Good::Grain), (200, Good::Grain), (100, Good::Grain), (50, Good::Grain)],
            "every lot is offered when the purse refuses"
        );
    }

    /// **The arable realm style has two floors**, 600 for the big lots and 100
    /// for the small ones. A lord with 300 sacks keeps shopping; one with 150
    /// stops after the three big lots are offered.
    #[test]
    fn the_arable_realm_style_tops_up_to_six_hundred_then_to_one_hundred() {
        let (mut c, _) = county_with(4);
        c.grain = 150;
        let mut m = ledger(false);
        run_buys(FarmStyle::RealmArable, &mut c, &mut m);
        assert_eq!(
            m.asked,
            vec![(400, Good::Grain), (200, Good::Grain), (100, Good::Grain)],
            "150 sacks is under 600 but not under 100"
        );

        let (mut c, _) = county_with(4);
        c.grain = 50;
        let mut m = ledger(false);
        run_buys(FarmStyle::RealmArable, &mut c, &mut m);
        assert_eq!(m.asked.len(), 5, "under 100, the two small lots are tried too");
    }

    /// The grazing realm style is the only one that stocks a herd of forty.
    #[test]
    fn only_the_grazing_realm_style_tops_its_herd_up_to_forty() {
        for (style, floor) in [
            (FarmStyle::NeutralGrazing, 11),
            (FarmStyle::RealmGrazing, 41),
            (FarmStyle::RealmMixed, 11),
        ] {
            let line = style.buys()[0];
            assert_eq!((line.good, line.floor, line.lot), (Good::Cattle, floor, 10), "{style:?}");
        }
        // The two arable styles never buy an animal.
        for style in [FarmStyle::NeutralArable, FarmStyle::RealmArable] {
            assert!(style.buys().iter().all(|l| l.good == Good::Grain), "{style:?}");
        }
    }

    /// Only the neutral arable style hands the county a purse, and only when it
    /// is short of grain.
    #[test]
    fn the_unowned_county_is_given_a_hundred_crowns_to_shop_with() {
        let (mut c, _) = county_with(4);
        c.grain = 0;
        assert_eq!(neutral_purse_top_up(FarmStyle::NeutralArable, &c), 100);
        c.grain = 100;
        assert_eq!(neutral_purse_top_up(FarmStyle::NeutralArable, &c), 0);
        c.grain = 0;
        for style in [FarmStyle::NeutralGrazing, FarmStyle::RealmArable, FarmStyle::RealmMixed] {
            assert_eq!(neutral_purse_top_up(style, &c), 0, "{style:?}");
        }
    }

    /// With no merchant the styles still run: they farm what is already there.
    #[test]
    fn a_style_runs_to_completion_against_a_market_that_refuses_everything() {
        let (mut c, mut map) = county_with(8);
        c.owner = 2;
        c.herd = 40;
        lay(FarmStyle::RealmArable, &mut c, &mut map, &env());
        assert_eq!(c.industry_share, 50);
        assert!(counts(&c, &map).0 > 0, "it still planted");
    }

    // --- the layouts --------------------------------------------------------

    /// **The arable styles plant half the county and keep one pasture.**
    #[test]
    fn the_arable_style_plants_half_the_county_in_winter() {
        let (mut c, mut map) = county_with(8);
        c.herd = 50;
        c.fertility = 0;
        lay(FarmStyle::RealmArable, &mut c, &mut map, &env());
        let (g, p, _) = counts(&c, &map);
        assert_eq!(g, 4, "half of eight");
        assert_eq!(p, 1, "and exactly one pasture, because the herd is over ten");
    }

    /// A county with ten head or fewer gets **no** pasture at all from an
    /// arable lord — the herd is left to graze nothing.
    #[test]
    fn an_arable_lord_gives_a_small_herd_no_pasture_whatever() {
        let (mut c, mut map) = county_with(8);
        c.herd = 10;
        // Start it with a pasture, so the test shows the clear rather than an
        // absence.
        map.terrain[c.field_tile(0).unwrap()] = terrain::PASTURE;
        crate::field::recount(&mut c, &map);
        assert_eq!(c.fields_cattle, 1);
        lay(FarmStyle::RealmArable, &mut c, &mut map, &env());
        assert_eq!(counts(&c, &map).1, 0, "the pasture was cleared and not replaced");
    }

    /// **The grazing styles never plant grain**, whatever the season.
    #[test]
    fn a_grazing_lord_clears_grain_in_every_season() {
        for season in Season::ALL {
            let (mut c, mut map) = county_with(6);
            for slot in 0..3 {
                map.terrain[c.field_tile(slot).unwrap()] = terrain::GRAIN;
            }
            crate::field::recount(&mut c, &map);
            assert_eq!(c.fields_grain, 3);
            c.herd = 200;
            c.herd_crowding = 50;
            let mut e = env();
            e.season = season;
            lay(FarmStyle::RealmGrazing, &mut c, &mut map, &e);
            assert_eq!(counts(&c, &map).0, 0, "no grain in {}", season.name());
        }
    }

    /// The grazing styles grow their pasture **one field a pass**, up to all
    /// but one of the county.
    #[test]
    fn a_grazing_lord_adds_one_pasture_a_pass_and_stops_one_short() {
        let (mut c, mut map) = county_with(5);
        c.owner = 2;
        c.herd = 400;
        let mut seen = Vec::new();
        for _ in 0..8 {
            c.herd_crowding = crate::land::herd_crowding(T, c.herd, c.fields_cattle);
            lay(FarmStyle::RealmGrazing, &mut c, &mut map, &env());
            seen.push(counts(&c, &map).1);
        }
        assert_eq!(seen, vec![1, 2, 3, 4, 4, 4, 4, 4], "one a pass, capped at total - 1");
    }

    /// **Style 9 splits the county in three** — a third grain, a third pasture.
    #[test]
    fn the_mixed_style_gives_a_third_to_each() {
        let (mut c, mut map) = county_with(9);
        c.owner = 2;
        c.herd = 400;
        c.fertility = 0;
        for _ in 0..5 {
            c.herd_crowding = crate::land::herd_crowding(T, c.herd, c.fields_cattle);
            lay(FarmStyle::RealmMixed, &mut c, &mut map, &env());
        }
        let (g, p, _) = counts(&c, &map);
        assert_eq!(g, 3, "a third of nine in grain");
        assert_eq!(p, 3, "and a third in pasture");
    }

    /// **Finding 1, asserted: the `-2` rung is dead code.** No fertility
    /// anywhere on the scale produces `base - 2`.
    #[test]
    fn the_fertility_ladders_middle_rung_can_never_be_reached() {
        for fertility in -100..=100 {
            let q = winter_grain_quota(fertility, 10);
            assert_ne!(q, 8, "fertility {fertility} reached the unreachable rung");
            assert_eq!(q, if fertility < -20 { 9 } else { 10 });
        }
    }

    /// **Finding 2, asserted: turning Advanced Farming off plants more.**
    #[test]
    fn switching_advanced_farming_off_makes_the_ai_plant_far_more_grain() {
        let plant = |style: FarmStyle, advanced: bool| {
            let (mut c, mut map) = county_with(9);
            c.herd = 0;
            c.fertility = 0;
            let mut e = env();
            e.advanced_farming = advanced;
            lay(style, &mut c, &mut map, &e);
            counts(&c, &map).0
        };
        assert_eq!((plant(FarmStyle::RealmArable, true), plant(FarmStyle::RealmArable, false)), (4, 8));
        assert_eq!((plant(FarmStyle::RealmMixed, true), plant(FarmStyle::RealmMixed, false)), (3, 4));
        assert_eq!(
            (plant(FarmStyle::NeutralArable, true), plant(FarmStyle::NeutralArable, false)),
            (4, 6),
            "the neutral pass keeps three fields back where the realm one keeps one"
        );
    }

    /// Outside Winter the arable styles re-lay nothing: the standing crop is
    /// left where it is. Only the pasture clear runs.
    #[test]
    fn an_arable_lord_leaves_a_standing_crop_alone_outside_winter() {
        for season in [Season::Spring, Season::Summer, Season::Autumn] {
            let (mut c, mut map) = county_with(8);
            for slot in 0..6 {
                map.terrain[c.field_tile(slot).unwrap()] = terrain::GRAIN + 3;
            }
            crate::field::recount(&mut c, &map);
            c.herd = 0;
            let mut e = env();
            e.season = season;
            lay(FarmStyle::RealmArable, &mut c, &mut map, &e);
            assert_eq!(counts(&c, &map).0, 6, "{} left the crop standing", season.name());
        }
    }

    // --- rations ------------------------------------------------------------

    /// **The dairy rungs are a downgrade, not an upgrade.** A cattle county is
    /// fed Double where a grain county with the same amount of food is fed
    /// Triple — finding 4.
    #[test]
    fn a_herd_county_is_fed_worse_than_a_grain_county_with_the_same_food() {
        let mut c = County::new();
        c.population = 100;
        // 57 head: store 855 (> 8 * 100, which alone would say Triple) but
        // dairy 285, which is over 2 * 100 and not over 3 * 100.
        c.herd = 57;
        c.grain = 0;
        assert_eq!(food_in_store(T, &c), 855);
        assert_eq!(ration_wanted(T, &c, false), 4, "Double: the dairy rung got there first");

        // The same feeding capacity, entirely as grain.
        c.herd = 0;
        c.grain = 143;
        assert_eq!(food_in_store(T, &c), 858);
        assert_eq!(ration_wanted(T, &c, false), 5, "Triple, on the same food as bread");
    }

    /// And the **Triple dairy rung can never change an answer**: `store` counts
    /// the herd twice, so it is always at least three times `dairy`.
    #[test]
    fn the_triple_dairy_rung_is_redundant_at_every_herd_and_population() {
        let with_rung = |c: &County| ration_wanted(T, c, false) == 5;
        for people in [1i32, 7, 100, 999, 5_000] {
            for herd in [1i32, 10, 61, 400, 3_000] {
                let mut c = County::new();
                c.population = people;
                c.herd = herd;
                let dairy = ration::food_from_dairy(T, herd);
                if dairy > people * 3 {
                    assert!(
                        with_rung(&c),
                        "the rung fired at people {people}, herd {herd}"
                    );
                    assert!(
                        food_in_store(T, &c) > people * 8,
                        "and the store rung below would have said 5 anyway"
                    );
                }
            }
        }
    }

    /// The bottom of the ladder, at each boundary.
    #[test]
    fn the_ration_ladder_descends_with_the_larder() {
        let want = |grain: i32| {
            let mut c = County::new();
            c.population = 600;
            c.grain = grain;
            ration_wanted(T, &c, false)
        };
        assert_eq!(want(0), 0, "nothing at all");
        assert_eq!(want(49), 0, "294 < 300");
        assert_eq!(want(50), 1, "300 is not under 300");
        assert_eq!(want(100), 2, "600 clears the population");
        assert_eq!(want(200), 3, "1200 clears twice it");
        assert_eq!(want(601), 4, "3606 > 3600");
        assert_eq!(want(801), 5, "4806 > 4800");
    }

    /// **The two search directions differ only on ties**, and the tie is the
    /// normal case: many splits reach the same level.
    #[test]
    fn the_two_split_searches_take_the_opposite_end_of_a_tie() {
        let mut base = County::new();
        base.population = 200;
        base.herd = 300;
        base.grain = 300;

        let mut low = base.clone();
        set_rations(T, &mut low, SplitSearch::PreferGrain, false);
        let mut high = base.clone();
        set_rations(T, &mut high, SplitSearch::PreferHerd, false);

        assert_eq!(low.ration_wanted, high.ration_wanted);
        assert!(low.ration_split < high.ration_split, "{} < {}", low.ration_split, high.ration_split);
        assert_eq!(low.ration_achieved, high.ration_achieved, "and both feed the county the same");
    }

    /// A fixed split is taken as given and nothing is searched.
    #[test]
    fn a_fixed_split_is_used_as_it_stands() {
        let mut c = County::new();
        c.population = 100;
        c.grain = 500;
        set_rations(T, &mut c, SplitSearch::Fixed(37), false);
        assert_eq!(c.ration_split, 37);
    }

    /// **Which search a style asks for**, including the hundred-head hinge.
    #[test]
    fn only_an_arable_lord_with_a_hundred_head_spare_eats_the_herd_first() {
        let mut c = County::new();
        c.herd = 100;
        assert_eq!(split_search(FarmStyle::RealmArable, &c), SplitSearch::PreferGrain);
        c.herd = 101;
        assert_eq!(split_search(FarmStyle::RealmArable, &c), SplitSearch::PreferHerd);
        assert_eq!(split_search(FarmStyle::NeutralArable, &c), SplitSearch::PreferHerd);
        for style in [FarmStyle::RealmGrazing, FarmStyle::NeutralGrazing, FarmStyle::RealmMixed] {
            assert_eq!(split_search(style, &c), SplitSearch::PreferGrain, "{style:?}");
        }
    }

    /// The AI's food total counts the herd **twice** and reads the stores, not
    /// the season's caps. Distinct from `ration::food_available`.
    #[test]
    fn the_ai_food_total_counts_the_herd_twice_and_ignores_the_season_caps() {
        let mut c = County::new();
        c.herd = 10;
        c.grain = 10;
        c.herd_available = 0;
        c.grain_available = 0;
        assert_eq!(food_in_store(T, &c), 10 * 5 + 10 * 10 + 10 * 6);
        assert_eq!(
            crate::ration::food_available(T, &c),
            10 * 5,
            "the sibling reads the caps and sees only the dairy"
        );
    }

    // --- the shares ---------------------------------------------------------

    /// **Finding 3, asserted**: the AI takes the *Built* defaults, and both
    /// halves close on 100.
    #[test]
    fn every_style_takes_the_castle_favouring_default_shares() {
        use crate::tables::{JOB_BLACKSMITH, JOB_CASTLE_BUILDING, JOB_FIELD_RECLAMATION};
        let mut c = County::new();
        default_shares_built(&mut c);
        assert_eq!(&c.labour_share[0..3], &[33, 50, 17]);
        assert_eq!(&c.labour_share[3..8], &[40, 15, 15, 15, 15]);
        assert_eq!(c.labour_share[0..=JOB_FIELD_RECLAMATION].iter().sum::<i32>(), 100);
        assert_eq!(c.labour_share[JOB_CASTLE_BUILDING..=JOB_BLACKSMITH].iter().sum::<i32>(), 100);
    }

    /// The industry share is the styles' loudest difference, and the arable
    /// lord — not the grazier — is the one who industrialises.
    #[test]
    fn the_arable_realm_style_puts_half_the_county_into_industry() {
        assert_eq!(FarmStyle::RealmArable.industry_share(), 50);
        assert_eq!(FarmStyle::RealmMixed.industry_share(), 40);
        assert_eq!(FarmStyle::RealmGrazing.industry_share(), 20);
        assert_eq!(FarmStyle::NeutralArable.industry_share(), 0);
        assert_eq!(FarmStyle::NeutralGrazing.industry_share(), 0);
    }

    // --- the outer passes ---------------------------------------------------

    /// Step 5 stamps the lord's style on every county he holds and farms it.
    #[test]
    fn step_five_stamps_the_lords_style_on_every_county_he_holds() {
        let mut counties = vec![County::new(); 4];
        let mut map = CampaignMap::empty();
        for id in 1..=3 {
            for i in 0..6 {
                let tile = crate::map::index(i as u8, id as u8);
                counties[id].set_field_tile(i, Some(tile));
                map.terrain[tile] = terrain::FALLOW;
            }
            counties[id].population = 500;
            crate::field::recount(&mut counties[id], &map);
        }
        counties[1].owner = 2;
        counties[2].owner = 3;
        counties[3].owner = 2;

        // Lord 4 farms style 9.
        manage_county_farms(T, &mut counties, 3, &mut map, &realms(), 2, 4, &mut NoMarket, &env());
        assert_eq!(counties[1].farm_style, 9);
        assert_eq!(counties[3].farm_style, 9);
        assert_eq!(counties[2].farm_style, 0, "another realm's county is untouched");
        assert_eq!(counties[1].industry_share, FarmStyle::RealmMixed.industry_share());
        assert_eq!(counties[2].industry_share, 25, "a fresh county's default");
    }

    /// The neutral pass **reads** the style byte and never writes it, so a
    /// county that has changed hands keeps the old lord's habits — and one that
    /// kept a style-9 lord's byte is left entirely alone.
    #[test]
    fn an_unowned_county_farms_by_whichever_lord_held_it_last() {
        let mut counties = vec![County::new(); 3];
        let mut map = CampaignMap::empty();
        for id in 1..=2 {
            for i in 0..6 {
                let tile = crate::map::index(i as u8, id as u8);
                counties[id].set_field_tile(i, Some(tile));
                map.terrain[tile] = terrain::FALLOW;
            }
            counties[id].population = 500;
            counties[id].herd = 200;
            crate::field::recount(&mut counties[id], &map);
            counties[id].herd_crowding =
                crate::land::herd_crowding(T, counties[id].herd, counties[id].fields_cattle);
        }
        counties[1].farm_style = 1; // an ex-grazier's county
        counties[2].farm_style = 9; // an ex-Knight's

        manage_neutral_fields(T, &mut counties, 2, &mut map, &realms(), &mut NoMarket, &env());
        assert_eq!(counties[1].farm_style, 1, "never rewritten");
        assert!(counts(&counties[1], &map).1 > 0, "it grazed");
        assert_eq!(
            counts(&counties[2], &map),
            (0, 0, 6),
            "style 9 dispatches nowhere in the neutral pass"
        );
    }

    /// **The "add a field" ladder starts a reclamation on a wasteland tile.**
    /// It does not conjure a fallow field, which is what this crate used to do —
    /// and [`crate::field::recount`] would have wiped that on the next pass.
    #[test]
    fn ordering_a_field_puts_a_wasteland_tile_under_reclamation() {
        let mut counties = vec![County::new(); 2];
        let mut map = CampaignMap::empty();
        counties[1].owner = 2;
        counties[1].population = 10;
        for i in 0..4 {
            let tile = crate::map::index(i as u8, 3);
            counties[1].set_field_tile(i, Some(tile));
            map.terrain[tile] = terrain::WASTE;
        }
        crate::field::recount(&mut counties[1], &map);
        assert_eq!(counties[1].fields_waste, 4);

        assert_eq!(
            manage_county_farms(T, &mut counties, 1, &mut map, &realms(), 2, 4, &mut NoMarket, &env()),
            1,
            "an empty county always gets its first field"
        );
        crate::field::recount(&mut counties[1], &map);
        assert_eq!(counties[1].fields_reclaiming, 1);
        assert_eq!(counties[1].fields_waste, 3);
    }

    /// **The ladder itself**, at every boundary the original branches on. It is
    /// an `if`/`else if` chain, so a county that clears no row's *pair* of
    /// conditions gains nothing even though it is small.
    #[test]
    fn the_field_ladder_orders_a_field_only_when_both_conditions_hold() {
        // Twenty wasteland tiles, so the quota is never the binding constraint,
        // and `fields` of them already fallow to set the county's total.
        let case = |fields: usize, population: i32| {
            let mut c = County::new();
            let mut map = CampaignMap::empty();
            for i in 0..20 {
                let tile = crate::map::index(i as u8, 8);
                c.set_field_tile(i, Some(tile));
                map.terrain[tile] = if i < fields { terrain::FALLOW } else { terrain::WASTE };
            }
            crate::field::recount(&mut c, &map);
            c.population = population;
            order_fields(&c, &mut map)
        };
        assert_eq!(case(0, 10), 1, "an empty county always gets its first field");
        assert_eq!(case(2, 150), 0, "small, but not populous enough for any row");
        assert_eq!(case(2, 201), 1);
        assert_eq!(case(4, 401), 1);
        assert_eq!(case(6, 601), 1);
        assert_eq!(case(8, 1001), 1);
        assert_eq!(case(8, 601), 0, "under 1000 the fields < 9 row does not fire");
        assert_eq!(case(12, 1201), 2, "a big county adds two");
        assert_eq!(case(12, 1200), 0, "and the test is strictly greater");
    }

    /// A county whose twenty tiles are all in use gains nothing, however big.
    #[test]
    fn a_full_county_cannot_gain_another_field() {
        let (mut c, mut map) = county_with(crate::county::MAX_FIELDS);
        c.population = 5_000;
        assert_eq!(order_fields(&c, &mut map), 0);
        crate::field::recount(&mut c, &map);
        assert_eq!(c.field_total(), crate::county::MAX_FIELDS as i32);
    }

    /// A county with no wasteland left is told to add a field and adds nothing.
    #[test]
    fn a_county_with_nothing_left_to_reclaim_gains_no_field() {
        let (mut c, mut map) = county_with(2);
        c.population = 10;
        assert_eq!(order_fields(&c, &mut map), 0, "two fallow fields, no waste");
    }

    /// **A field already under reclamation eats a place in the quota.** Told to
    /// add one, a county already reclaiming one starts nothing; told to add
    /// two, it starts one.
    #[test]
    fn a_reclamation_already_running_consumes_the_quota_without_doing_anything() {
        let build = || {
            let mut c = County::new();
            let mut map = CampaignMap::empty();
            for i in 0..4 {
                let tile = crate::map::index(i as u8, 5);
                c.set_field_tile(i, Some(tile));
                map.terrain[tile] = terrain::WASTE;
            }
            // Slot 0 is already halfway through a reclamation.
            map.terrain[c.field_tile(0).unwrap()] = terrain::RECLAIM_FIRST + 1;
            crate::field::recount(&mut c, &map);
            (c, map)
        };
        let (c, mut map) = build();
        assert_eq!(crate::field::order_reclamation(&c, &mut map, 1), 0);
        let (c, mut map) = build();
        assert_eq!(crate::field::order_reclamation(&c, &mut map, 2), 1);
        // An in-use field, by contrast, is skipped and costs nothing.
        let (mut c, mut map) = build();
        map.terrain[c.field_tile(0).unwrap()] = terrain::GRAIN;
        crate::field::recount(&mut c, &map);
        assert_eq!(crate::field::order_reclamation(&c, &mut map, 1), 1);
    }

    /// A realm whose lord has no personality record farms nothing rather than
    /// falling through to style 0 — the same refusal `ai::set_tax_rates` makes.
    #[test]
    fn a_lord_with_no_personality_record_farms_nothing() {
        let mut counties = vec![County::new(); 2];
        let mut map = CampaignMap::empty();
        counties[1].owner = 2;
        counties[1].population = 900;
        for i in 0..4 {
            let tile = crate::map::index(i as u8, 7);
            counties[1].set_field_tile(i, Some(tile));
            map.terrain[tile] = terrain::WASTE;
        }
        crate::field::recount(&mut counties[1], &map);
        assert_eq!(
            manage_county_farms(T, &mut counties, 1, &mut map, &realms(), 2, 5, &mut NoMarket, &env()),
            0
        );
        crate::field::recount(&mut counties[1], &map);
        assert_eq!(counties[1].fields_reclaiming, 0, "not even a field ordered");
    }

    // --- FUN_004A4694 -------------------------------------------------------

    /// The nudge is **one field at a time in both directions**, and the shrink
    /// needs three conditions where the growth needs two.
    #[test]
    fn the_cattle_nudge_moves_one_field_and_only_under_its_own_conditions() {
        let grow = |crowding: i32, herd: i32, cattle: usize, cap: i32| {
            let (mut c, mut map) = county_with(8);
            for slot in 0..cattle {
                map.terrain[c.field_tile(slot).unwrap()] = terrain::PASTURE;
            }
            crate::field::recount(&mut c, &map);
            c.herd = herd;
            c.herd_crowding = crowding;
            fit_cattle_fields(T, &mut c, &mut map, cap);
            c.fields_cattle
        };
        assert_eq!(grow(11, 100, 2, 5), 3, "crowded and under the cap: one more");
        assert_eq!(grow(11, 100, 5, 5), 5, "at the cap: nothing");
        assert_eq!(grow(10, 5, 3, 5), 2, "uncrowded, tiny herd, spare fields: one back");
        assert_eq!(grow(10, 5, 2, 5), 2, "two fields is the floor");
        assert_eq!(grow(10, 50, 3, 5), 3, "a herd of ten or more keeps its fields");
    }
}
