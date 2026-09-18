//! `crates/l2-kingdom/src/ai/mod.rs` used to say that AI turn step 5 is
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
//! counties' grain style;
//! outer functions differ in more than their name — `AI_ManageFields` also
//! zeroes county `+0x1B0` on the way in, which `Ai_ManageCountyFarms` does not.
//!
//! All five are implemented here, plus `FUN_004A4694` (the cattle-field nudge)
//! and `FUN_004A4782` (the AI's ration setter), and [`manage_county_farms`] and
//! [`manage_neutral_fields`] are the two outer passes.
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
//!    Anything below −50 is already below −20, so the `-2` branch **never
//!    runs**: a ruined county gets the same one-field discount as a tired
//!    one. Reproduced — see [`winter_grain_quota`]. `[V]`,
//!    from the branch order in the decompilation of all three functions that
//!    carry it.
//!
//! 2. **Turning *Advanced Farming* off makes the AI plant far more grain, not
//!    less.** The option's `else` limb overwrites the whole ladder: neutral 0
//!    plants `total - 3`, realm 0 plants `total - 1` (nearly every field), and
//! style 9 plants `total / 2` instead of `total / 3`. `[V]`.
//!
//! 3. **`Labour_DefaultSharesBuilt` (`0x0045158B`) is the AI's setter.**
//!
//!    `docs/symbols.md` records of the two default-share functions that *"which
//!    of the two setters a county gets is not traced"*. All five styles call the
//!    *Built* one — farm 33/50/17, industry 40/15/15/15/15 with castle building
//!    favoured — and none calls `Labour_DefaultShares`. `[V]`.
//!
//! 4. **The AI's ration ladder punishes a cattle county.** Of its two dairy
//! rungs the Triple one can never change an answer,
//!    ever *lowers* the level: a county fed on cattle gets Double where a county
//!    fed on the same quantity of food as grain gets Triple. The arithmetic is
//!    in [`ration_wanted`]. `[V]`.
//!
//! 5. **"Add a field" is a reclamation order, and one already running eats the
//!    quota.** `crate::tables::AI_FIELD_LADDER` reads as *"give the county
//!    another field"*; `FUN_0044C6C4` paints
//!    [`crate::field::terrain::RECLAIM_FIRST`] onto a **wasteland** tile, and a
//!    tile already reclaiming spends a place in the quota without anything
//! happening —
//!    adds nothing. This crate used to add one to `County::fields_fallow`
//!    instead, which `crate::field::recount` overwrites from the map on the very
//!    next pass: **the AI's field expansion has never happened.** See
//!    [`crate::field::order_reclamation`]. `[V]`.
//!
//! * **The merchant.** Every style opens by buying food (`FUN_004A4B12` →
//! `Merchant_Trade`),
//!   trigger them — `if (grain < 100) buy 400; if (grain < 100) buy 200;` — so
//!   whether a lot arrives changes whether the next one is even attempted. That
//!   cannot be a list of requests returned at the end; it has to be a callback,
//!   which is [`Market`]. [`CountyStall`] is `Ai_BuyGood` and is what **all
//!   three** of the game's callers pass — the unowned counties' pass, AI step
//!   5, and `Ai_ManageFarmsAll` at the head of the season. [`NoMarket`]
//!   refuses everything and is left for a hand-built kingdom with no merchant
//!   model.
//!
//! * **`FUN_0049E39B`, the AI's selling pass**, which the three *realm* styles
//!   run first: it sells wood, iron and stone down to the lord's reserves at
//!   personality `+0x84`/`+0x88`/`+0x8C` and buys weapons of the county's type
//!   when gold clears `+0x78`. It is [`Market::trade_for_county`], defaulting
//!   to nothing and implemented by [`CountyStall`];
//!   [`FarmStyle::sells_first`] is the three styles that run it.

mod market;
pub use market::*;
mod policy;
pub use policy::*;
mod layout;
pub use layout::*;
mod tests;
pub use tests::*;

use crate::county::County;
use crate::field::{self, FieldType};
use crate::labour;
use crate::map::CampaignMap;
use crate::ration;
use crate::realm::Realm;
use crate::tables::{Season, Tables};

#[derive(Debug, Clone, Copy)]
pub struct FarmEnv {
    pub season: Season,
    pub season_next: Season,
    pub advanced_farming: bool,
    pub armies_eat: bool,
}

impl FarmEnv {
    pub fn new(season: Season, season_next: Season) -> FarmEnv {
        FarmEnv { season, season_next, advanced_farming: true, armies_eat: true }
    }
}

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

    /// Whether this style runs `FUN_0049E39B` — the surplus sale
    /// purchase — before it farms. The three realm styles do; the two neutral
    /// ones do not, because an unowned county has no realm to sell for.
    pub fn sells_first(self) -> bool {
        matches!(self, FarmStyle::RealmArable | FarmStyle::RealmGrazing | FarmStyle::RealmMixed)
    }

    pub fn industry_share(self) -> i32 {
        match self {
            FarmStyle::NeutralArable | FarmStyle::NeutralGrazing => 0,
            FarmStyle::RealmArable => 50,
            FarmStyle::RealmGrazing => 20,
            FarmStyle::RealmMixed => 40,
        }
    }

    pub fn sows_in_winter(self) -> bool {
        matches!(self, FarmStyle::NeutralArable | FarmStyle::RealmArable | FarmStyle::RealmMixed)
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyLine {
    pub good: Good,
    pub floor: i32,
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

/// `0x004A42E3`'s cascade — the shortest of the five,
/// tops its herd up to **41**.
const BUYS_REALM_GRAZING: [BuyLine; 2] = [cattle(41, 10), grain(100, 400)];

/// `0x004A440F`'s cascade — the longest,
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

