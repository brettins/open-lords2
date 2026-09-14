
mod ladders;
pub use ladders::*;
mod tables;
pub use tables::*;

use super::*;

// ---------------------------------------------------------------------------
// The AI's tax ladders
// ---------------------------------------------------------------------------

/// How many rungs a tax ladder has room for. The neutral ladder uses all
/// eight; the three personality ladders use five, five and six and pad the
/// rest. A size, not a balance figure.
pub const TAX_LADDER_RUNGS: usize = 8;

/// How many ladders the personality table chooses between.
pub const AI_TAX_LADDER_COUNT: usize = 3;

/// A tax ladder: `(happiness_below, rate)` rows, walked in order, taking the
/// first row the county's happiness is strictly below. The last row is the
/// fallback and its threshold is never tested.
///
/// **These are `if`/`else if` chains in the original, not a table** — there is
/// no address to read them from, and `tools/oracle/kingdom.ps1` recovers all
/// four out of `AI_SetTaxRates`' instruction stream instead: the thresholds
/// are the `CMP EAX, imm8` immediates and the rates are the
/// `MOV byte ptr [county+0xB9], imm8` stores, interleaved in source order.
pub type TaxLadder = [(i32, i32); TAX_LADDER_RUNGS];

/// The ladder `AI_SetTaxRates(0)` applies to **unowned** counties, which phase
/// 1 (`docs/kingdom.md` §3.1) runs every turn. `[V]`
///
/// Eight rungs, and the only ladder that goes above 12.
pub const AI_TAX_LADDER_NEUTRAL: TaxLadder =
    [(20, 0), (40, 1), (50, 2), (60, 3), (70, 4), (80, 6), (90, 8), (i32::MAX, 12)];

/// The three ladders an AI realm picks between, indexed by the *personality*
/// int at [`AI_PERSONALITY_TAX_LADDER`]. `[V]`
///
/// The unused rungs are padded by repeating the fallback threshold so all four
/// ladders have one shape; only the rows before `i32::MAX` are ever tested.
///
/// Ladder 0 is the greediest - 15% on a happy county. Ladder 2 is the gentlest
/// and the only one that charges nothing below 60 happiness.
pub const AI_TAX_LADDERS: [TaxLadder; AI_TAX_LADDER_COUNT] = [
    [(30, 0), (50, 2), (65, 4), (80, 10), (i32::MAX, 15), (i32::MAX, 15), (i32::MAX, 15), (i32::MAX, 15)],
    [(30, 0), (50, 1), (65, 3), (80, 7), (i32::MAX, 12), (i32::MAX, 12), (i32::MAX, 12), (i32::MAX, 12)],
    [(60, 0), (70, 1), (80, 2), (90, 3), (95, 8), (i32::MAX, 10), (i32::MAX, 10), (i32::MAX, 10)],
];

/// Walk a ladder. Happiness is a signed byte in the original and the
/// comparisons are signed,
#[inline]
pub fn tax_rate_for(ladder: &TaxLadder, happiness: i32) -> i32 {
    let mut i = 0;
    while i < ladder.len() {
        if happiness < ladder[i].0 {
            return ladder[i].1;
        }
        i += 1;
    }
    ladder[ladder.len() - 1].1
}

/// `g_aiPersonality` (`0x004D8A58`) is records of this many bytes, one per AI
/// lord. `[I]` on the base and the stride: the code addresses it as
/// `base + (lord * 3 - 3) * 0x50`, i.e. three 0x50-byte rows per lord, and only
/// ever uses the first row. The arithmetic closes on the low side -
/// `0x004D8A40 + 24` (the free-archer table) is exactly `0x004D8A58`.
pub const AI_PERSONALITY_STRIDE: usize = 0xF0;

/// **Four** personality records, for lords 1..=4, indexed `lord - 1`.
///
/// `docs/kingdom.md` §2 says the lord byte runs *"1 … 5 for an AI lord"*, but
/// the table stops at four. A fifth record would begin at `0x004D8E18`, and
/// what is there does not fit the shape: the two fields below read 17 and 0
/// where every real record reads a farm style of 0, 1 or 9, and the field after
/// them reads 5000 where the four records read 100, 100, 200 and 50. So
/// `0x004D8E18` is taken to be **past the end of the table**, and this crate
/// refuses to answer for lord 5
/// bytes whose meaning is unknown. `[I]`, and it is the one thing about
/// `AI_SetTaxRates` still not settled.
pub const AI_PERSONALITY_COUNT: usize = 4;

/// The personality field at record `+0x04` that selects an [`AI_TAX_LADDERS`]
/// row. `[V]` for lords 1..=4 - three of the four AI lords tax on the same,
/// gentlest ladder and only lord 4 uses a different one.
pub const AI_PERSONALITY_TAX_LADDER: [usize; AI_PERSONALITY_COUNT] = [2, 2, 2, 1];

/// The personality field at record `+0x00` that `AI_ManageFields` copies into
/// county `+0x1FE` and dispatches on. `[V]` for lords 1..=4; 0, 1 and 9 are
/// exactly the three values the dispatch tests, which is a check on the field's
/// identity. **What each style does was not traced** - the three handlers
/// (`FUN_004A4052`, `FUN_004A42E3`, `FUN_004A440F`) allocate labour and are not
/// reproduced here.
pub const AI_PERSONALITY_FARM_STYLE: [u8; AI_PERSONALITY_COUNT] = [1, 1, 0, 9];

// --- the diplomacy half of the record, `docs/diplomacy.md` §8.4 -------------
//
// Every column below is `[V]`: read out of the four records in the file at
// 0x004D8A58, 0x004D8B48, 0x004D8C38, 0x004D8D28, and held against the binary
// by tools/oracle/kingdom.ps1. What each field *means* is `[D]` — one reader
// each, named in the doc comment.

/// Record `+0x08` — the increment a gift is judged against.
///
/// A gift under `bestGift + T/2` **costs** 8 standing and one at or above
/// `bestGift + T` gains 10,
/// width of the band that insults. The Bishop's 50 makes him the cheapest lord
/// to please and the Countess's 200 the dearest.
/// Read by `Diplo_ReplyGift`. `docs/diplomacy.md` §3.1.
pub const AI_PERSONALITY_GIFT_INCREMENT: [i32; AI_PERSONALITY_COUNT] = [100, 100, 200, 50];

/// Record `+0x0C` — the base price of military help, multiplied by the pair's
/// `help_price_multiple`. Read by `Diplo_ReplyHelpRequest` and
/// `Diplo_ReplyAttackRequest`. §3.5.
pub const AI_PERSONALITY_HELP_PRICE: [i32; AI_PERSONALITY_COUNT] = [500, 1000, 1600, 1500];

/// Record `+0x10` — how much grudge an alliance survives.
///
/// The comparison is `tolerance < grudge`, so the Knight's 5 breaks on the
/// sixth point and the Bishop's 20 on the twenty-first. Read by `AI_Diplomacy`.
/// §4.1.
pub const AI_PERSONALITY_GRUDGE_TOLERANCE: [i32; AI_PERSONALITY_COUNT] = [5, 10, 15, 20];

/// Record `+0x14` — turns of courtship before an alliance is offered. The
/// Bishop moves in four turns where the Knight takes twelve. §4.1.
pub const AI_PERSONALITY_OFFER_INTERVAL: [i32; AI_PERSONALITY_COUNT] = [12, 10, 8, 4];

/// Record `+0x30` — the floor below which an ally will not move at all.
///
/// **`docs/diplomacy.md` §3.5 calls this a treasury floor and it is not one.**
/// `Diplo_ReplyHelpRequest` compares it against realm `+0x14`, which
/// [`crate::Realm::population_mean`] holds — the mean population of the
/// realm's counties, §8.4's own note that the same field is
/// *"also a county-population floor at step 9"* is the corroboration: it is a
/// population floor in both places. See [`crate::diplomacy::reply_help_request`].
pub const AI_PERSONALITY_HELP_POPULATION_FLOOR: [i32; AI_PERSONALITY_COUNT] =
    [750, 800, 900, 1000];

/// Record `+0x40` — the percentage of a county's population one AI muster
/// conscripts. `[V]` on the values, `[D]` on the meaning. §8.2.
///
/// > It used to say *"nothing in this crate raises an army, so it is carried
/// > and not read"*. [`crate::Kingdom::run_ai_raise_army`] reads it.
pub const AI_PERSONALITY_MUSTER_PCT: [i32; AI_PERSONALITY_COUNT] = [30, 30, 40, 50];

/// Record `+0x28` — **how many turns a lord waits between musters** when he
/// has neither a war target nor an ally's request. `docs/diplomacy.md` §8.4
/// already carried the values and called them *"turns between musters"*; this
/// is the reader that makes the name a claim.
///
/// `FUN_0049F977` (AI step 9) counts realm `+0x45` up and returns early while
/// it is **below** the lord's figure, then resets it to 0 — so the muster
/// happens on the turn the counter *reaches* the number, and the Bishop looks
/// for a war every second turn where the Baron and Countess look every fourth.
/// `[V]` on the values, read out of `Lords2.exe` at `0x004D8A58 + lord*0xF0 +
/// 0x28`; `[D]` on the meaning, from its single reader.
pub const AI_PERSONALITY_MUSTER_PATIENCE: [i32; AI_PERSONALITY_COUNT] = [3, 4, 4, 2];

/// Record `+0x68` — **the weapon stock a lord wants before he musters.**
///
/// AI step 9 tests it against realm `+0x138`, the maintained sum of the six
/// weapon counters (`Realm_RecountWeapons`, `0x004487A9`) — the number the
/// panel draws as *Arms*. Below it, the lord raises nothing at all **unless**
/// [`crate::ai_army::emergency_weapons`] says he is in trouble, which is the
/// one branch that bypasses the gate.
///
/// > **`docs/diplomacy.md` §8.4 says this is "a threshold on realm `+0x38`".
/// > It is `+0x138`.** `+0x38` is inside the twenty-four army-name counters at
/// > `+0x2D`. A missing digit,
/// > and the kind that is only found by trying to use the field.
///
/// `[V]` on the values; `[D]` on the meaning.
pub const AI_PERSONALITY_MUSTER_ARMS: [i32; AI_PERSONALITY_COUNT] = [100, 120, 200, 250];

/// Record `+0x70` — **the county population a lord needs before he will raise
/// a garrison for its castle.** Read by `FUN_0049F12F`, the second of AI step
/// 7's three passes.
///
/// `docs/diplomacy.md` §8.4 lists `+0x70` among the fields that *"hold
/// plausible per-lord values and were not traced"*. Traced. Note it runs the
/// **opposite** way to the castle-building floor at `+0xC8`: the Bishop needs
/// the largest county before he will *build* (600) and the smallest before he
/// will *garrison* (150).
///
/// `[V]` on the values; `[D]` on the meaning.
pub const AI_PERSONALITY_GARRISON_MIN_POPULATION: [i32; AI_PERSONALITY_COUNT] =
    [300, 300, 250, 150];

/// Record `+0x74` — **turns between raids.** AI step 10 sends one unit and
/// then loads realm `+0x15A` with this, counting it down one a turn and
/// sending nothing until it is 0.
///
/// Another of §8.4's untraced six. The Countess raids every five turns, the
/// Baron and the Bishop every ten.
///
/// `[V]` on the values; `[D]` on the meaning.
pub const AI_PERSONALITY_RAID_INTERVAL: [i32; AI_PERSONALITY_COUNT] = [6, 10, 5, 10];

/// Record `+0x9C` — **the tax rate a lord puts on a county he has given up
/// on.** The last of §8.4's untraced six that has a reader.
///
/// `FUN_0049F431`, AI step 7's third pass, decides a county cannot be held,
/// levies what is left of it, sets the tax rate to this, sets the industry
/// share to 100 and ships or sells everything in the larder. Every one of the
/// four is **far above** anything the lord's own tax ladder would ever charge
/// a county he meant to keep — [`AI_TAX_LADDERS`] tops out at 15 — so this is
/// a lord stripping a county on the way out.
///
/// `[V]` on the values; `[D]` on the meaning.
pub const AI_PERSONALITY_ABANDON_TAX_RATE: [i32; AI_PERSONALITY_COUNT] = [32, 28, 23, 35];

/// Record `+0x90` — how many castles a lord will have in progress at once.
///
/// The Bishop's **1** is half the story of §8.1: his money does not spread, so
/// whichever county he starts on gets the whole treasury's worth.
pub const AI_PERSONALITY_CASTLE_CONCURRENT: [i32; AI_PERSONALITY_COUNT] = [4, 3, 2, 1];

/// Record `+0x50` … `+0x64` — **the lord's weapon rota**: six weapon types,
/// stepped through by AI step 12 (`FUN_0049E77D`).
///
/// **`[V]`** — read straight out of `Lords2.exe` at `0x004D8A58 + lord*0xF0`
///, so the values are the file's. They
/// agree entry for entry with the table `docs/diplomacy.md` §8.3 had already
/// recovered by a different route, which is the corroboration that lifts the
/// mark: two independent reads of the same twenty-four numbers.
///
/// §8.3 stopped short of claiming these index [`WEAPON_NAMES`] — *"nothing ties
/// `+0x50` to `g_weaponCost` beyond both being small integers under 6"*. What
/// closes it is `FUN_0049ED13`, the call the rota loop makes immediately after
/// assigning one: it looks the value up in **`g_weaponCost`** to decide whether
/// the realm can afford the county's weapon. The field is a `g_weaponCost`
/// index because a `g_weaponCost` lookup is the next thing done with it. `[V]`.
///
/// That also settles §8.3's open half: on this reading the **Baron makes pikes,
/// bows and armour and no crossbows at all**, which is the *opposite* of the
/// player claim that he favours peasant armies.
///
/// The cursor at [`crate::realm::Realm::weapon_rota`] runs 0..=9 and the ten
/// steps index these six as [`AI_WEAPON_ROTA_ORDER`] — `0,1,2,3` twice, then
/// `4,5`. So the first four entries are visited **twice as often** as the last
/// two, and a lord's programme is a ten-county cycle
/// one.
///
/// The values index [`WEAPON_NAMES`]: 0 crossbow, 1 mace, 2 sword, 3 pike,
/// 4 bow, 5 armour. Read as behaviour the four lords are recognisably
/// different: lord 2 makes nothing but pikes, bows and armour; lord 4 makes
/// bows and pikes almost exclusively; lord 3's rota is the cheap end of the
/// table — crossbows, maces and swords — and only lord 1 spreads across the
/// whole armoury.
pub const AI_PERSONALITY_WEAPON_ROTA: [[usize; 6]; AI_PERSONALITY_COUNT] =
    [[0, 1, 4, 2, 5, 2], [3, 5, 4, 4, 4, 5], [0, 1, 1, 2, 4, 0], [4, 4, 3, 4, 4, 3]];

/// Which of the six rota slots each of the ten cursor values selects.
///
/// `FUN_0049E77D` is ten `else if` limbs, not an array lookup, and the limbs
/// repeat: 0→`+0x50`, 1→`+0x54`, 2→`+0x58`, 3→`+0x5C`, **4→`+0x50`, 5→`+0x54`,
/// 6→`+0x58`, 7→`+0x5C`**, 8→`+0x60`, 9→`+0x64`. Written as data because that
/// is what it is; `[V]` from the ten branches.
pub const AI_WEAPON_ROTA_ORDER: [usize; 10] = [0, 1, 2, 3, 0, 1, 2, 3, 4, 5];

/// Record `+0xC8` — the county population a castle project needs before it is
/// started at all.
pub const AI_PERSONALITY_CASTLE_MIN_POPULATION: [i32; AI_PERSONALITY_COUNT] =
    [700, 650, 600, 600];

/// Record `+0xCC` … `+0xDC` — the treasury each castle type needs, by lord.
///
/// Columns are castle types 1..=5: palisade, motte & bailey, Norman keep, stone
/// castle, **royal castle**. `AI_BuildCastles` tests them from the top down and
/// takes the first the treasury clears.
///
/// **A zero means the type is not offered to
/// that lord.** The original guards every rung with `threshold != 0 &&` before
/// the comparison, which is the only reason a zero row does not make every
/// lord build a palisade for nothing. See
/// [`AiPersonalityRow::castle_for_gold`].
///
/// The player's remark that *"the bishop always tries to build really big
/// castles"* is this table's last column: **2,000 crowns for the Bishop and
/// 10,000 for the Knight**, with the Baron and the Countess never reaching one
/// at any treasury. §8.1.
pub const AI_PERSONALITY_CASTLE_GOLD: [[i32; AI_CASTLE_LADDER_LEN]; AI_PERSONALITY_COUNT] = [
    [200, 0, 1000, 0, 10_000], // Knight
    [0, 500, 0, 4000, 0],      // Baron
    [0, 300, 0, 2000, 0],      // Countess
    [0, 0, 100, 0, 2000],      // Bishop
];

/// Personality record `+0xA0` — **which siege engines the lord builds**, read
/// by `crate::siege::prepare` and by nothing else.
///
/// `docs/diplomacy.md` §8.4 listed `+0xA0` among eleven fields that *"hold
/// plausible per-lord values and were never traced"*. `Siege_Prepare`
/// (`0x004A7EB5`) is its reader, and the four values read straight out of
/// `Lords2.exe` are below.
///
/// | lord | value | what it orders |
/// |---|---:|---|
/// | Knight | 8 | four siege towers |
/// | Baron | 9 | one battering ram, and the default two towers |
/// | Countess | 7 | three catapults, the default two towers, and a ram against a stone or royal castle after season 2 |
/// | Bishop | 7 | the same |
///
/// **[V]** — three of the four are exactly the three constants the function
/// compares against, and the fourth repeats one of them. A field that meant
/// something else would not land on that set. Two consequences: the function's
/// *"default: two towers"* arm is **unreachable for every shipped lord**, and
/// the Knight is the only lord who brings no artillery to a siege.
pub const AI_PERSONALITY_SIEGE_DOCTRINE: [i32; AI_PERSONALITY_COUNT] = [8, 9, 7, 7];

/// Record `+0x78` — **the treasury floor `Ai_TradeForCounty` (`0x0049E39B`)
/// buys weapons above**: `if (personality[+0x78] < realm.gold)`, strictly.
///
/// **[V]** — read out of `g_aiPersonality` (`0x004D8A58`, stride `0xF0`) in
/// `Lords2.exe`, by the same dump that re-derives the two neighbouring fields
/// this crate already carries (`+0x70` 300/300/250/150 and `+0x74` 6/10/5/10).
pub const AI_PERSONALITY_TRADE_GOLD_FLOOR: [i32; AI_PERSONALITY_COUNT] =
    [1000, 1500, 2500, 4000];

/// Record `+0x7C` — **how many weapons of the county's own type the lord buys**
/// once his gold clears [`AI_PERSONALITY_TRADE_GOLD_FLOOR`]. One order, halved
/// by `Ai_BuyGoodDownTo` (`0x004A4C41`) until the treasury covers it. **[V]**.
pub const AI_PERSONALITY_WEAPON_BUY_QTY: [i32; AI_PERSONALITY_COUNT] = [100, 80, 70, 150];

/// Record `+0x84`, `+0x88`, `+0x8C` — **the wood, stone and iron a lord keeps
/// back.** Everything above the reserve is sold to the county merchant, and
/// only when the matching realm *want* ([`crate::realm::Realm::want`]) is zero.
///
/// All three offsets hold the same number within a lord, which is consistent
/// with one "keep this much of everything" figure written into three slots
/// (`docs/diplomacy.md` §8.4). They are carried as three because
/// `Ai_TradeForCounty` reads three, and a mod may want them apart. **[V]**.
pub const AI_PERSONALITY_RESERVE_WOOD: [i32; AI_PERSONALITY_COUNT] = [250, 300, 500, 1000];
pub const AI_PERSONALITY_RESERVE_STONE: [i32; AI_PERSONALITY_COUNT] = [250, 300, 500, 1000];
pub const AI_PERSONALITY_RESERVE_IRON: [i32; AI_PERSONALITY_COUNT] = [250, 300, 500, 1000];

