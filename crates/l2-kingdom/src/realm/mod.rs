//! Six records at `0x0057BF00`, stride `0x160`; index 0 is unused and 1..=5 are
//! the players. As with [`crate::county`], the semantics are reproduced and the
//! byte layout is not.

mod methods;
pub use methods::*;

use crate::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_net::{Quirk, Quirks};

pub const MAX_REALMS: usize = 6;

pub const MAX_REALM_ID: u8 = (MAX_REALMS - 1) as u8;

pub const AI_STEP_DONE: i32 = 999;

/// The `lord` byte (`+0x07`) when the realm has been knocked out.
pub const LORD_ELIMINATED: u8 = 6;

pub const LORD_HUMAN: u8 = 0;

pub const LORD_BISHOP: u8 = 4;

/// `docs/diplomacy.md` §1. `[V]`
pub const PAIR_BLOCK_OFFSET: usize = 0x84;
pub const PAIR_RECORD_STRIDE: usize = 0x10;

/// `+0xE4` is referenced nowhere in the binary and `+0xE5` is the AI's chosen
/// muster county, so six sixteen-byte sub-records fit between two known things
/// with nothing left over. [`PAIR_FIELDS`] tiles one record and
/// `the_pair_block_closes_exactly_on_0xe4` asserts both halves.
pub const PAIR_BLOCK_END: usize = PAIR_BLOCK_OFFSET + MAX_REALMS * PAIR_RECORD_STRIDE;

/// Carried as data so a test can assert that the record tiles exactly rather
/// than trusting the prose, and so the two runs of bytes that are *not* fields
/// stay visible. `docs/diplomacy.md` §1 and §9: `+0x06`/`+0x07` are neither
/// written by `Diplo_Init` nor read anywhere, and `+0x0E`/`+0x0F` are zeroed at
/// init and read nowhere. Both are left as gaps
/// fields.
pub const PAIR_FIELDS: [(&str, usize, usize); 11] = [
    ("standing", 0x00, 1),
    ("allied", 0x01, 1),
    ("grudge", 0x02, 1),
    ("warningsSent", 0x03, 1),
    ("atWar", 0x04, 1),
    ("complimentsFrom", 0x05, 1),
    ("- never written, never read", 0x06, 2),
    ("bestGift", 0x08, 4),
    ("hasMail", 0x0C, 1),
    ("helpPriceMultiple", 0x0D, 1),
    ("- zeroed at init, read nowhere", 0x0E, 2),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pair {
    /// `+0x00` — [`crate::diplomacy::STANDING_MIN`] ..=
    /// [`crate::diplomacy::STANDING_MAX`]. Every write site clamps.
    pub standing: i8,
    /// `+0x01` — set and cleared in pairs by
    /// [`crate::diplomacy::form_alliance`] and
    /// [`crate::diplomacy::break_alliance`].
    pub allied: bool,
    /// `+0x02` — accumulates while allied; past the lord's tolerance the
    /// alliance breaks. `docs/diplomacy.md` §4.1.
    pub grudge: u8,
    /// `+0x03` — 0 → 1 → 2 → 3, the *Warning / Warning / Notice of revenge*
    /// ladder. §5.
    pub warnings_sent: u8,
    /// `+0x04` — permanently blocks alliance offers once set.
    pub at_war: bool,
    /// `+0x05` — how many compliments `them` has sent `me`. **Never reset**,
    /// which is what turns the third compliment into a permanent −4. §3.2.
    pub compliments_from: u8,
    /// `+0x08` — the largest single gift `me` has ever had from `them`.
    pub best_gift: i32,
    /// `+0x0C` — a letter from `them` is waiting in my inbox. Cleared for
    /// every sender when the inbox is answered.
    pub has_mail: bool,
    /// `+0x0D` — starts at **1** and rises by one every time `me` is paid to
    /// help `them`, so the price of help doubles, trebles, quadruples. §3.5.
    pub help_price_multiple: u8,
}

impl Default for Pair {
    fn default() -> Pair {
        Pair::new()
    }
}

impl Pair {
    pub const fn new() -> Pair {
        Pair {
            standing: 0,
            allied: false,
            grudge: 0,
            warnings_sent: 0,
            at_war: false,
            compliments_from: 0,
            best_gift: 0,
            has_mail: false,
            help_price_multiple: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Realm {
    /// `+0x00` — 0..=14 program counter through the AI's turn;
    /// [`AI_STEP_DONE`] when finished.
    pub ai_step: i32,
    /// `+0x04` — the realm exists. Kept as a bool because that is how every
    /// reader in the binary uses it, but the byte itself is
    /// [`Realm::strength`], and this is `strength != 0`.
    pub in_play: bool,
    /// `+0x04` again — the number the byte holds.
    ///
    /// **`docs/kingdom.md` §2 calls `+0x04` `inPlay` and marks it `[V]`; it is a
    /// weighted strength count.** `FUN_0049B42B` rebuilds it at the top of
    /// every AI turn as `3 * ownedCounties + 1 * armies`, and a realm is
    /// eliminated when it comes out zero. Every other site only tests it
    /// against zero,
    /// See [`crate::ai::begin_realm_turn`].
    pub strength: u8,
    /// `+0x05` — when set, the AI turn machine is skipped entirely. This is the
    /// byte `docs/battle.md` §6.2 could not explain; it means "a person is
    /// driving this realm".
    pub is_human: bool,
    /// `+0x07` — 0 for the human, 1..=5 for an AI lord, [`LORD_ELIMINATED`]
    /// when knocked out. Indexes `g_aiPersonality` and `g_aiGoldGrant`.
    pub lord: u8,
    /// `+0x0A` — 1..=5, the realm's banner colour. Carried because the lord
    /// card and the map draw it and because §0.1's trap is easier to walk into
    /// when the field is absent; no rule in this crate reads it.
    pub shield_index: u8,
    /// `+0x28` — the sum of every owned county's `tax_hap_other`, added to
    /// every county's tax happiness term.
    pub tax_hap_empire: i8,
    /// `+0x29` — owned counties; selects between the two AI gold-grant tables.
    pub county_count: u8,
    /// `+0x2A` — **the most counties this realm has ever held.**
    ///
    /// `Game_SetupRealmsAndCounties` writes it 1 beside `+0x29`, and
    /// `County_ChangeOwner` (`0x004A72FE`) is its only other writer and its only
    /// reader: it raises it to the taker's new count, and before that it reads it
    /// to choose the local player's capture letter — `L2.eng` 117 *"Bravo!!"*
    /// for the first county past the peak, 118 for the second, the share-of-map
    /// ladder after that, and 126 *"The county is yours. May you rule it
    /// wisely."* for a county that only wins back ground already held once. It
    /// equals `+0x29` in every save on file, because no fixture has lost a
    /// county. See [`crate::conquest::Capture`].
    pub peak_counties: u8,
    /// `+0x2B` — 1..=5 from `Score_RankRealms`.
    pub rank: u8,
    /// `+0x50` — recomputed every turn.
    pub score: i32,
    /// `+0xFC` — this season's army bill.
    pub wages: i32,
    /// `+0x118` — the treasury.
    pub gold: i32,
    /// `+0x120`, `+0x128`, `+0x130` — the realm-wide stockpiles
    /// `Industry_Produce` credits. `L2.eng` group 70 is
    /// *"Gold, Arms, Iron, Stone, Wood"*.
    pub iron: i32,
    pub stone: i32,
    pub wood: i32,
    /// `+0x140 + t*4` — one counter per weapon type.
    pub weapons: [i32; WEAPON_TYPE_COUNT],
    /// `+0x158` — 0..=5, the escalation in `Wages_PayAll`. Stage 5 is the
    /// mutiny and it resets to 0 afterwards; it does not saturate.
    pub bankrupt_stage: u8,

    /// `+0x104` and `+0x108` — **crowns spent at the merchant**, and
    /// `+0x10C` / `+0x110` — crowns taken.
    ///
    /// Four accumulators, two per direction: `Merchant_Trade` adds the bill to
    /// `+0x108` then `+0x104` on a purchase, and the proceeds to `+0x110` then
    /// `+0x10C` on a sale. Nothing else in the binary writes them and
    /// **nothing at all reads them.** The only other instruction that touches
    /// any of the four is `Game_SetupRealmsAndCounties` zeroing all four at
    /// new game.
    pub trade_spent_a: i32,
    pub trade_spent_b: i32,
    pub trade_received_a: i32,
    pub trade_received_b: i32,

    /// `+0xF4` and `+0xF8` — **crowns banked from tax**, the same shape as the
    /// four trade accumulators above and one pair further down the record.
    ///
    /// `Tax_CollectAll` (`0x0044B59B`) adds each owned county's take to both,
    /// straight after it adds it to [`Realm::gold`]; an unowned county's take
    /// goes to its own purse and touches neither. **Nothing reads either.**
    ///
    /// `[V]`, two ways that share no step: the decompilation names
    /// `field_0xf4`/`field_0xf8` of `g_realms` in exactly two functions —
    /// `Tax_CollectAll` and `Game_SetupRealmsAndCounties` (`0x0049BD99`), which
    /// zeroes them at new game — and a scan of `Lords2.exe` for the absolute
    /// addresses `0x0057BFF4` / `0x0057BFF8` finds two instructions each, at
    /// `0x0044B7BD`/`0x0044B7DE` inside the first and
    /// `0x0049C364`/`0x0049C37C` inside the second. The same scan finds the
    /// trade pair's `+0x10C` in `Merchant_Trade` and the setup clear and
    /// nowhere else, and `+0x50` (the score) in fourteen places, so it sees
    /// readers where there are readers.
    ///
    /// What the scan cannot see, said beside it: an access through a pointer
    /// to the record plus a small displacement. The only whole-record consumers
    /// found are `Save_Write`, which stores the realm block, and
    /// `Sync_CompareState`, which compares every realm byte from `+0x06` — so
    /// in the original these are **stored, desync-checked, and read by no
    /// rule**. They are carried for exactly the reason the trade pair is: they
    /// are simulation state the original keeps, a save that dropped them would
    /// load a different record, and two lockstep peers must agree on them.
    ///
    /// Index 0 is `+0xF4`, index 1 is `+0xF8`. Named `a`/`b` by position for
    /// the trade pair's reason: both always take the same number and nothing
    /// distinguishes them, so a more specific name would be a claim about a
    /// mechanic the shipped game does not have.
    pub tax_ledger: [i32; 2],

    // --- the totals AI step 14 rebuilds (`FUN_0049D1E0`) --------------------
    /// `+0x10` — the realm's total population, summed over its counties.
    pub population_total: i32,
    /// `+0x18` — the previous turn's [`Realm::population_total`], snapshotted
    /// before the recount.
    pub population_last: i32,
    /// `+0x14` — the mean population per owned county, 0 when there are none.
    pub population_mean: i32,
    /// `+0x0C` — the mean happiness over the realm's counties. Score input,
    /// weighted `x2`.
    pub mean_happiness: i32,
    /// `+0x58` — the mean health meter over the realm's counties. Score input,
    /// weighted `x2`.
    pub mean_health: i32,
    /// `+0x60` — `PctOf(ownedCounties, g_countyCount)`: the share of the map
    /// this realm holds, 0..=100. Score input, and the **heaviest identified
    /// one** at `x10`.
    pub share_of_map_pct: i32,
    /// `+0x2C` — the realm's armies.
    pub army_count: u8,
    /// `+0x54` — the total men over those armies. Score input, weighted `/5`.
    pub total_men: i32,

    /// The six score inputs in the order [`crate::tables::SCORE_WEIGHTS`]
    /// applies: realm `+0x60, +0x10, +0x0C, +0x58, +0x54, +0x4C`.
    ///
    /// **Index 5, `+0x4C`, is the realm's castle count and is deliberately not
    /// one of them**: `Castle_BuildTick` (`0x004508DE`) is its only writer, once
    /// a season, and `Kingdom::castle_build_tick` is where we write it. See
    /// [`crate::tables::SCORE_INPUT_CASTLES`] for why the difference between
    /// "written by a season pass" and "derived when scoring" is behaviour rather
    /// than style.
    pub score_inputs: [i32; 6],

    // --- diplomacy: `docs/diplomacy.md` §1.2 -------------------------------
    /// `+0x1C` — an alliance offer to a human is outstanding. Cleared at the
    /// top of this realm's own turn, and read by
    /// [`crate::diplomacy::pick_ally_candidate`] so two AIs do not court the
    /// same realm at once.
    pub offer_pending: bool,
    /// `+0x80` — who [`crate::diplomacy::ai_diplomacy`] has decided to court.
    pub ally_candidate: u8,
    /// `+0x81` — **one byte, so one ally.** 0 for none.
    pub ally: u8,
    /// `+0x84 + other * 0x10` — this realm's view of each other realm.
    pub pairs: [Pair; MAX_REALMS],
    /// `+0xE8` — the county an ally has been asked to march on.
    ///
    /// **`docs/diplomacy.md` §3.5 calls this the ally's `warTarget`; it is a
    /// different field.** `Diplo_PayForHelp` writes `+0xE8` and the army code
    /// reads it as a county id, while [`Realm::war_target`] at `+0xEB` is a
    /// *realm* id that `Diplo_Offend` writes. Two fields three bytes apart,
    /// and conflating them would have an ally march on a realm number.
    pub target_county: u8,
    /// `+0xE9` — counts to 8 before a taunt is sent. §6.
    pub taunt_timer: u8,
    /// `+0xEA` — 0 → *"How are you doing?"*, 1 → *"Helpful advice."*
    pub taunt_stage: u8,
    /// `+0xEB` — the realm this one has resolved to attack, 0 for none. Only
    /// ever written when it is already 0, so the first grievance sticks.
    pub war_target: u8,
    /// `+0xEC` — counts up to the lord's `offer_interval` between alliance
    /// offers. Signed: the original compares it as a `char`.
    pub offer_timer: i8,
    /// `+0xED` — the one-shot guard on *"Just call me king."*
    pub crowned_once: bool,
    /// `+0x6C` — **the weapon rota's cursor**, 0..=9, advanced once per county
    /// by AI step 12 and wrapping at 10.
    pub weapon_rota: i32,
    /// `+0x159` — 0..=3, advanced after **every** message this realm sends.
    pub voice_rotation: u8,

    /// `+0xE5` — **the muster county**: where this realm raises its main army.
    pub muster_county: u8,
    /// `+0xE6` — the **raiding** county, the second output of the same scoring
    /// pass. It takes the *opposite* food term: the county picked to muster
    /// from is the one with food to spare, and this is the one whose larder is
    /// under strain. Written by the same function and read by nothing in the
    /// shipped binary that this crate has found — carried because the pass
    /// writes it and a save that dropped it would diverge on nothing today and
    /// on something tomorrow. `[D]`
    pub raid_county: u8,
    /// `+0x45` — **the muster patience counter.** A realm with neither a
    /// war target nor an ally's request counts up here and only looks for
    /// somewhere to attack when it reaches the lord's
    /// [`crate::tables::AI_PERSONALITY_MUSTER_PATIENCE`], then resets to 0.
    ///
    /// The byte sits immediately after the twenty-four army-name counters at
    /// `+0x2D`, which end exactly on `+0x45` — `docs/records.json`.
    pub muster_timer: u8,
    /// `+0x48` — **the realm this one has decided is the threat**: the
    /// top-ranked realm holding at least 40% of the map, or the declared
    /// [`Realm::war_target`] when there is one. Rebuilt every muster;
    /// [`crate::ai_army::pick_threat`].
    pub threat_realm: u8,
    /// `+0x4B` — **the county the main army is aimed at.** Chosen by
    /// [`crate::Kingdom::pick_attack_county`] and read again by step 10, which
    /// sends its raider to the same place.
    pub attack_county: u8,
    /// `+0x15A` — **the raid cooldown.** Step 10 sends one unit and then loads
    /// this with the lord's [`crate::tables::AI_PERSONALITY_RAID_INTERVAL`],
    /// counting it down one a turn and sending nothing until it is 0 again.
    pub raid_timer: u8,

    /// `+0x70`, `+0x74`, `+0x78`, `+0x7C` — **what the realm wants to buy**,
    /// in wood, iron, (unwritten) and stone.
    ///
    /// AI step 4 ([`crate::ai_army::resource_wants`]) zeroes all four and
    /// refills the three it uses; `Ai_TradeForCounty` (`0x0049E39B`) is the
    /// reader, and it is the pass that decides whether the lord sells his
    /// surplus or buys. **`+0x78` is zeroed every turn and never written**,
    /// which is a fact about the shipped game and not a gap here.
    ///
    /// Easy to confuse with the personality record's `+0x70 … +0x7C`, which
    /// are different numbers in a different record — `docs/diplomacy.md` §8.4
    /// warns about exactly this pair.
    pub want: [i32; 4],
}

impl Default for Realm {
    fn default() -> Self {
        Realm::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    const Q: Quirks = Quirks::FAITHFUL;

    const T: &Tables = &Tables::DEFAULT;

    #[test]
    fn a_human_pays_a_quarter_of_a_crown_a_man() {
        let mut r = Realm::new();
        r.is_human = true;
        assert_eq!(r.wage_for_unit(T, 250, 0), 62);
        assert_eq!(r.wage_for_unit(T, 252, 0), 63);
        assert_eq!(r.wage_for_unit(T, 254, 0), 63);
        for d in 0..=2 {
            assert_eq!(r.wage_for_unit(T, 1000, d), 250);
        }
    }

    #[test]
    fn an_ai_pays_less_the_harder_the_game_is() {
        let mut r = Realm::new();
        r.is_human = false;
        assert_eq!(r.wage_for_unit(T, 300, 0), 100);
        assert_eq!(r.wage_for_unit(T, 300, 1), 60);
        assert_eq!(r.wage_for_unit(T, 300, 2), 30);
        assert_eq!(r.wage_for_unit(T, 300, 9), 30);
    }

    #[test]
    fn the_human_realm_draws_no_gold_grant_at_any_difficulty() {
        let mut r = Realm::new();
        r.lord = LORD_HUMAN;
        for d in 0..=3 {
            assert_eq!(r.gold_grant(T, d), 0);
        }
    }

    #[test]
    fn the_empire_tax_term_wraps_exactly_as_the_original_would() {
        let mut r = Realm::new();
        for _ in 0..16 {
            r.add_empire_tax_happiness(-10, Q);
        }
        assert_eq!(r.tax_hap_empire, (-160i32 as i8), "expected the i8 wrap");
        assert_eq!(r.tax_hap_empire, 96, "and -160 wraps to +96 - a *bonus*");

        let mut ok = Realm::new();
        for _ in 0..12 {
            ok.add_empire_tax_happiness(-10, Q);
        }
        assert_eq!(ok.tax_hap_empire, -120);
    }

    #[test]
    fn score_applies_each_weight_to_its_own_input() {
        let mut r = Realm::new();
        r.score_inputs = [1, 100, 1, 1, 100, 1];
        assert_eq!(r.compute_score(T), 10 + 10 + 2 + 2 + 20 + 50);
    }

    #[test]
    fn the_gold_bracket_is_the_only_term_with_a_known_meaning() {
        let mut r = Realm::new();
        for (gold, bonus) in [
            (0, 0),
            (2000, 0),
            (2001, 50),
            (5000, 50),
            (5001, 50),
            (10_000, 50),
            (10_001, 50),
            (1_000_000, 50),
        ] {
            r.gold = gold;
            assert_eq!(r.compute_score(T), bonus, "gold {gold}");
        }
    }

    /// `docs/diplomacy.md` §1's headline claim, as arithmetic
    /// prose: sixteen bytes per sub-record, six sub-records, and the block
    /// finishing exactly on `+0xE4` where the next named field begins.
    #[test]
    fn the_pair_block_closes_exactly_on_0xe4() {
        assert_eq!(PAIR_BLOCK_END, 0xE4, "0x84 + 6 * 0x10");

        let mut next = 0;
        for (name, offset, width) in PAIR_FIELDS {
            assert_eq!(offset, next, "{name} does not start where the last field ended");
            next += width;
        }
        assert_eq!(next, PAIR_RECORD_STRIDE, "the fields do not fill sixteen bytes");

        let named = PAIR_FIELDS.iter().filter(|(n, _, _)| !n.starts_with('-')).count();
        assert_eq!(named, 9, "nine fields and two untraced gaps");
    }

    #[test]
    fn the_message_variant_covers_zero_to_fifteen_once_each() {
        let mut seen = [false; 16];
        for lord in 1..=4u8 {
            for rot in 0..=3u8 {
                let mut r = Realm::new();
                r.lord = lord;
                r.voice_rotation = rot;
                let v = r.message_variant();
                assert_eq!(v as usize, (lord as usize - 1) * 4 + rot as usize);
                assert!(!seen[v as usize], "variant {v} twice");
                seen[v as usize] = true;
            }
        }
        assert!(seen.iter().all(|s| *s));
    }

    #[test]
    fn the_voice_rotation_cycles_through_four_takes() {
        let mut r = Realm::new();
        let mut seen = Vec::new();
        for _ in 0..9 {
            seen.push(r.voice_rotation);
            r.advance_voice();
        }
        assert_eq!(seen, vec![0, 1, 2, 3, 0, 1, 2, 3, 0]);
    }

    #[test]
    fn a_human_realm_is_always_done_and_an_ai_realm_is_not_until_it_says_so() {
        let mut r = Realm::new();
        r.in_play = true;
        r.is_human = true;
        assert!(r.turn_done());
        r.is_human = false;
        assert!(!r.turn_done());
        r.ai_step = AI_STEP_DONE;
        assert!(r.turn_done());
    }
}

