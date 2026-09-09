//! The realm record — `docs/kingdom.md` §2.
//!
//! Six records at `0x0057BF00`, stride `0x160`; index 0 is unused and 1..=5 are
//! the players. As with [`crate::county`], the semantics are reproduced and the
//! byte layout is not.

use crate::tables::{Tables, WEAPON_TYPE_COUNT};

/// `g_realms` is 6 records and **index 0 is unused** (`docs/kingdom.md` §2), so
/// the usable realm ids are 1..=5 — which is also `g_playerStartCount`'s
/// maximum and DirectPlay's `dwMaxPlayers` in the original
/// (`docs/netcode.md` §1).
pub const MAX_REALMS: usize = 6;

/// The highest playable realm id.
pub const MAX_REALM_ID: u8 = (MAX_REALMS - 1) as u8;

/// `aiStep` when the realm has finished its turn. `Turn_AllRealmsDone` tests
/// exactly this. `docs/kingdom.md` §3.2.
pub const AI_STEP_DONE: i32 = 999;

/// The `lord` byte (`+0x07`) when the realm has been knocked out.
/// `docs/kingdom.md` §2.
pub const LORD_ELIMINATED: u8 = 6;

/// The `lord` byte of the human player. Row 0 of every lord-indexed table is
/// the human's row, and `g_aiGoldGrant`'s row 0 is all zeros.
pub const LORD_HUMAN: u8 = 0;

/// The `lord` byte of the Bishop.
///
/// Named because exactly one rule tests it directly — the reproduced bug in
/// [`crate::diplomacy::offend`] — and writing `4` there would hide that it is
/// the same byte every personality table is indexed by.
/// `docs/diplomacy.md` §0 and §5.
pub const LORD_BISHOP: u8 = 4;

/// Where the per-pair diplomacy block starts inside the realm record, and how
/// wide one sub-record is: `realm[me] + 0x84 + them * 0x10`.
/// `docs/diplomacy.md` §1. `[V]`
pub const PAIR_BLOCK_OFFSET: usize = 0x84;
pub const PAIR_RECORD_STRIDE: usize = 0x10;

/// One past the last byte of the block — `0x84 + 6 * 0x10`.
///
/// **This closing is the evidence that the block is what it looks like.**
/// `+0xE4` is referenced nowhere in the binary and `+0xE5` is the AI's chosen
/// muster county, so six sixteen-byte sub-records fit between two known things
/// with nothing left over. [`PAIR_FIELDS`] tiles one record and
/// `the_pair_block_closes_exactly_on_0xe4` asserts both halves.
pub const PAIR_BLOCK_END: usize = PAIR_BLOCK_OFFSET + MAX_REALMS * PAIR_RECORD_STRIDE;

/// The sixteen bytes of one pair sub-record, as `(name, offset, width)`.
///
/// Carried as data so a test can assert that the record tiles exactly rather
/// than trusting the prose, and so the two runs of bytes that are *not* fields
/// stay visible. `docs/diplomacy.md` §1 and §9: `+0x06`/`+0x07` are neither
/// written by `Diplo_Init` nor read anywhere, and `+0x0E`/`+0x0F` are zeroed at
/// init and read nowhere. Both are left as gaps rather than invented into
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

/// How `me` feels about `them`, and what has passed between them — the
/// sixteen-byte sub-record at `realm[me] + 0x84 + them * 0x10`.
///
/// **The relationship is asymmetric.** `realms[a].pair(b)` is a's view of b and
/// `realms[b].pair(a)` is b's view of a, and nearly every rule moves only one
/// of them. `docs/diplomacy.md` §1.
///
/// As everywhere else in this crate the *semantics* are reproduced and the byte
/// layout is not; [`PAIR_FIELDS`] is what carries the layout, for the oracle
/// and for a reader.
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
    /// Ratchets up, never down, and it is what the next gift is judged
    /// against. §3.1.
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
    /// A cleared record. `Diplo_Init` writes **1** into `help_price_multiple`
    /// and 0 into every other field, so the multiple is the one whose zero
    /// would be wrong — a never-initialised record would price help at nothing.
    /// [`crate::diplomacy::init`] is what puts the opening standing in.
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

/// A realm — one player, human or AI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Realm {
    /// `+0x00` — 0..=14 program counter through the AI's turn;
    /// [`AI_STEP_DONE`] when finished.
    pub ai_step: i32,
    /// `+0x04` — the realm exists. Kept as a bool because that is how every
    /// reader in the binary uses it, but the byte itself is
    /// [`Realm::strength`], and this is `strength != 0`.
    pub in_play: bool,
    /// `+0x04` again — the number the byte actually holds.
    ///
    /// **`docs/kingdom.md` §2 calls `+0x04` `inPlay` and marks it `[V]`; it is a
    /// weighted strength count.** `FUN_0049B42B` rebuilds it at the top of
    /// every AI turn as `3 * ownedCounties + 1 * armies`, and a realm is
    /// eliminated when it comes out zero. Every other site only tests it
    /// against zero, which is why "inPlay" fits everything but the write.
    /// See [`crate::ai::begin_realm_turn`].
    pub strength: u8,
    /// `+0x05` — when set, the AI turn machine is skipped entirely. This is the
    /// byte `docs/battle.md` §6.2 could not explain; it means "a person is
    /// driving this realm".
    pub is_human: bool,
    /// `+0x07` — 0 for the human, 1..=5 for an AI lord, [`LORD_ELIMINATED`]
    /// when knocked out. Indexes `g_aiPersonality` and `g_aiGoldGrant`.
    ///
    /// **The lord is not the realm and it is not the colour.** Setup draws a
    /// lord for each realm out of `g_lordChoice`, so nothing may be indexed by
    /// realm id or by [`Realm::shield_index`] that the original indexes by
    /// this. `docs/diplomacy.md` §0.1.
    pub lord: u8,
    /// `+0x0A` — 1..=5, the realm's banner colour. Carried because the lord
    /// card and the map draw it and because §0.1's trap is easier to walk into
    /// when the field is absent; no rule in this crate reads it.
    pub shield_index: u8,
    /// `+0x28` — the sum of every owned county's `tax_hap_other`, added to
    /// every county's tax happiness term.
    ///
    /// **Held as `i8`, deliberately.** `docs/kingdom.md` §4.1 flags this
    /// explicitly: `Tax_SumEmpireHappiness` sums signed bytes from up to
    /// sixteen counties *into a signed byte* and nothing clamps it. Widening it
    /// here would be a silent balance change, so the type is kept and the
    /// wrap is reproduced and tested (see [`Realm::add_empire_tax_happiness`]).
    pub tax_hap_empire: i8,
    /// `+0x29` — owned counties; selects between the two AI gold-grant tables.
    pub county_count: u8,
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
    ///
    /// `docs/hypotheses.json` guessed the pairs were *this season* and *the
    /// running total*, with the caveat that if neither is ever reset they are
    /// something else. Neither is ever reset, so the guess is refuted and the
    /// names are deliberately `a` and `b`: what is verified is that two run on
    /// spending, two on income, both of a pair always take the same number, and
    /// no reader distinguishes them. Naming them anything more specific would
    /// be a claim about a mechanic the shipped game does not have.
    pub trade_spent_a: i32,
    pub trade_spent_b: i32,
    pub trade_received_a: i32,
    pub trade_received_b: i32,

    // --- the totals AI step 14 rebuilds (`FUN_0049D1E0`) --------------------
    /// `+0x10` — the realm's total population, summed over its counties.
    /// Score input, weighted `/10`.
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
    /// **Five of the six are identified now** — see
    /// [`crate::tables::SCORE_INPUT_OFFSETS`] — and
    /// [`Realm::sync_score_inputs`] copies them out of the named fields above.
    /// Index 5, `+0x4C`, is still unknown and is left for a caller to set;
    /// naming it would be exactly the failure mode `docs/decisions.md` C3
    /// records.
    pub score_inputs: [i32; 6],

    // --- diplomacy: `docs/diplomacy.md` §1.2 -------------------------------
    /// `+0x1C` — an alliance offer to a human is outstanding. Cleared at the
    /// top of this realm's own turn, and read by
    /// [`crate::diplomacy::pick_ally_candidate`] so two AIs do not court the
    /// same realm at once.
    pub offer_pending: bool,
    /// `+0x80` — who [`crate::diplomacy::ai_diplomacy`] has decided to court.
    pub ally_candidate: u8,
    /// `+0x81` — **one byte, so one ally.** 0 for none. Exclusivity is not a
    /// rule written anywhere; it is the width of this field.
    pub ally: u8,
    /// `+0x84 + other * 0x10` — this realm's view of each other realm.
    /// Index 0 is never used, matching the realm array itself.
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
    ///
    /// It is a *realm* counter walked inside a loop over the realm's counties,
    /// so a realm of four counties advances it four places in one step and the
    /// counties of one realm end up making four different weapons. The rota
    /// itself is six values in the personality record —
    /// [`crate::tables::AI_PERSONALITY_WEAPON_ROTA`] — visited in the order
    /// 0,1,2,3,0,1,2,3,4,5, so the first four are seen twice per lap.
    /// See [`crate::ai::choose_industry`].
    pub weapon_rota: i32,
    /// `+0x159` — 0..=3, advanced after **every** message this realm sends.
    /// It picks which of the lord's four recorded takes plays, and it is half
    /// of the `lord * 4 + rot - 4` variant index. `docs/diplomacy.md` §0.
    pub voice_rotation: u8,

    // --- the war plan: what AI steps 7, 9 and 10 write ----------------------
    // These seven fields are the AI's *standing orders*. They persist between
    // turns, which is the whole reason they are realm state and not locals:
    // a realm that decided last turn to attack county 4 out of county 2 is
    // still doing that this turn. See [`crate::ai_army`].
    /// `+0xE5` — **the muster county**: where this realm raises its main army.
    /// Rebuilt every turn by [`crate::ai_army::choose_muster_counties`], which
    /// scores every owned county on population, happiness and how much spare
    /// food it has.
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
    /// So the Bishop goes looking every second turn and the Baron every
    /// fourth.
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

impl Realm {
    pub fn new() -> Realm {
        Realm {
            ai_step: 0,
            in_play: false,
            strength: 0,
            is_human: false,
            lord: LORD_HUMAN,
            shield_index: 0,
            tax_hap_empire: 0,
            county_count: 0,
            rank: 0,
            score: 0,
            wages: 0,
            gold: 0,
            iron: 0,
            stone: 0,
            wood: 0,
            weapons: [0; WEAPON_TYPE_COUNT],
            bankrupt_stage: 0,
            trade_spent_a: 0,
            trade_spent_b: 0,
            trade_received_a: 0,
            trade_received_b: 0,
            population_total: 0,
            population_last: 0,
            population_mean: 0,
            mean_happiness: 0,
            mean_health: 0,
            share_of_map_pct: 0,
            army_count: 0,
            total_men: 0,
            score_inputs: [0; 6],
            offer_pending: false,
            ally_candidate: 0,
            ally: 0,
            pairs: [Pair::new(); MAX_REALMS],
            target_county: 0,
            taunt_timer: 0,
            taunt_stage: 0,
            war_target: 0,
            offer_timer: 0,
            crowned_once: false,
            weapon_rota: 0,
            voice_rotation: 0,
            muster_county: 0,
            raid_county: 0,
            muster_timer: 0,
            threat_realm: 0,
            attack_county: 0,
            raid_timer: 0,
            want: [0; 4],
        }
    }

    /// Realm `+0x138` — **the total weapon stock**, the sum of the six
    /// counters at `+0x140`.
    ///
    /// `Realm_RecountWeapons` (`0x004487A9`) maintains it as a stored field and
    /// `Merchant_Trade` and the weapons branch of `Industry_Produce` both call
    /// it after moving a stock; it is derived here for the reason
    /// [`crate::ai::build_castles`] derives the castle concurrency count — one
    /// loop already answers it, and a stored copy is a second source of truth.
    /// It is the number the panel draws as *Arms* (`L2.eng` group 70) and the
    /// number AI step 9 tests against
    /// [`crate::tables::AI_PERSONALITY_MUSTER_ARMS`].
    pub fn weapons_total(&self) -> i32 {
        self.weapons.iter().sum()
    }

    pub fn is_eliminated(&self) -> bool {
        self.lord == LORD_ELIMINATED
    }

    /// This realm's view of `other`. Out-of-range ids read slot 0, which
    /// nothing else uses — the original would index off the end of the block
    /// and into the muster county, and a panic here would be a worse
    /// reproduction than a harmless slot.
    pub fn pair(&self, other: u8) -> &Pair {
        &self.pairs[(other as usize).min(MAX_REALMS - 1)]
    }

    pub fn pair_mut(&mut self, other: u8) -> &mut Pair {
        &mut self.pairs[(other as usize).min(MAX_REALMS - 1)]
    }

    /// The variant index a message from this realm carries:
    /// `lord * 4 + rot - 4`, which for lords 1..=4 is exactly 0..=15 in four
    /// contiguous blocks of four. `docs/diplomacy.md` §0.
    ///
    /// A human's lord byte is 0, so a human's variant would be `rot - 4` —
    /// negative. Nothing in the original sends a letter *from* a human through
    /// this path (the player's letter is drawn from a text buffer instead), and
    /// the arithmetic is reproduced with a wrapping subtraction rather than
    /// guarded, so the shape of the expression stays visible.
    pub fn message_variant(&self) -> u8 {
        (self.lord.wrapping_mul(4)).wrapping_add(self.voice_rotation).wrapping_sub(4)
    }

    /// Advance the voice rotation, wrapping 3 → 0. Every message send does
    /// this, which is why two consecutive letters from one lord never use the
    /// same recorded take.
    pub fn advance_voice(&mut self) {
        self.voice_rotation += 1;
        if self.voice_rotation > 3 {
            self.voice_rotation = 0;
        }
    }

    /// True once this realm's AI turn has finished, or immediately when a
    /// person is driving it — `docs/kingdom.md` §3.1 phase 4 waits on this for
    /// every realm.
    /// The test is `>=`, not `==`: the original writes 999 and then falls
    /// through an increment that leaves 1000 in the record. See
    /// [`crate::ai::run_step`].
    pub fn turn_done(&self) -> bool {
        !self.in_play || self.is_human || self.ai_step >= AI_STEP_DONE
    }

    /// Copy the five identified score inputs out of the fields
    /// [`crate::ai::update_realm_totals`] rebuilds. Index 5 (`+0x4C`) is left
    /// alone, because nothing here knows what it is.
    pub fn sync_score_inputs(&mut self) {
        self.score_inputs[0] = self.share_of_map_pct;
        self.score_inputs[1] = self.population_total;
        self.score_inputs[2] = self.mean_happiness;
        self.score_inputs[3] = self.mean_health;
        self.score_inputs[4] = self.total_men;
    }

    /// Accumulate one county's contribution to the empire tax term.
    ///
    /// **Wrapping is the behaviour, not a bug in this function.** See the
    /// field's own documentation; `docs/kingdom.md` §4.1 says the original
    /// sums into a signed byte with nothing clamping it, and marks whether it
    /// wraps in play as untested. Reproducing it keeps that question askable.
    pub fn add_empire_tax_happiness(&mut self, contribution: i32) {
        self.tax_hap_empire = self.tax_hap_empire.wrapping_add(contribution as i8);
    }

    /// `Wages_ForUnit` (`0x004AD52B`) — the whole army upkeep rule.
    ///
    /// `difficulty` is `g_optDifficulty`, 0..=2 (and anything above clamps into
    /// the table). Troop type does not enter it: a knight and a peasant cost
    /// the same.
    pub fn wage_for_unit(&self, t: &Tables, men: i32, difficulty: u8) -> i32 {
        let divisor = if self.is_human {
            t.wages.divisor_human
        } else {
            t.wages.divisor_ai[(difficulty as usize).min(t.wages.divisor_ai.len() - 1)]
        };
        men / divisor
    }

    /// The per-season gold grant this realm's lord draws, by difficulty.
    ///
    /// **Two tables**: a realm holding fewer than three counties draws from the
    /// smaller [`AI_GOLD_GRANT_SMALL`], which is uniformly *less* — the grants
    /// reward a realm that is winning rather than propping up one that is
    /// losing. The human's lord byte is 0 and row 0 of both tables is all
    /// zeros, so the human gets nothing either way. `docs/kingdom.md` §8.2.
    pub fn gold_grant(&self, t: &Tables, difficulty: u8) -> i32 {
        let table = if crate::ai::uses_small_gold_table(self.county_count) {
            &crate::tables::AI_GOLD_GRANT_SMALL
        } else {
            &t.ai.gold_grant
        };
        let lord = (self.lord as usize).min(table.len() - 1);
        let diff = (difficulty as usize).min(table[0].len() - 1);
        table[lord][diff]
    }

    /// `Score_RankRealms`' score expression. The six weighted inputs are
    /// unidentified; the gold bracket is the one term whose meaning is
    /// unambiguous. `docs/kingdom.md` §8.3.
    pub fn compute_score(&self, t: &Tables) -> i32 {
        let mut score: i64 = 0;
        for i in 0..t.score.weights.len() {
            let (num, den) = t.score.weights[i];
            score += (self.score_inputs[i] as i64 * num as i64) / den as i64;
        }
        score += t.score_gold_bracket(self.gold) as i64;
        score as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The stock ruleset. Every rule below takes it as an argument now.
    const T: &Tables = &Tables::DEFAULT;

    /// A player measured 250 men -> 62 crowns, 252 -> 63, 254 -> 63.
    /// `docs/kingdom.md` §7.4.
    #[test]
    fn a_human_pays_a_quarter_of_a_crown_a_man() {
        let mut r = Realm::new();
        r.is_human = true;
        assert_eq!(r.wage_for_unit(T, 250, 0), 62);
        assert_eq!(r.wage_for_unit(T, 252, 0), 63);
        assert_eq!(r.wage_for_unit(T, 254, 0), 63);
        // Difficulty does not touch the human divisor.
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
        // Out-of-range difficulty clamps rather than panicking.
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

    /// The hazard `docs/kingdom.md` §4.1 flags and leaves untested: sixteen
    /// counties at a punitive tax rate overflow a signed byte.
    #[test]
    fn the_empire_tax_term_wraps_exactly_as_the_original_would() {
        let mut r = Realm::new();
        // Sixteen counties each contributing -10 is -160, which does not fit.
        for _ in 0..16 {
            r.add_empire_tax_happiness(-10);
        }
        assert_eq!(r.tax_hap_empire, (-160i32 as i8), "expected the i8 wrap");
        assert_eq!(r.tax_hap_empire, 96, "and -160 wraps to +96 - a *bonus*");

        // Below the ceiling nothing surprising happens.
        let mut ok = Realm::new();
        for _ in 0..12 {
            ok.add_empire_tax_happiness(-10);
        }
        assert_eq!(ok.tax_hap_empire, -120);
    }

    #[test]
    fn score_applies_each_weight_to_its_own_input() {
        let mut r = Realm::new();
        r.score_inputs = [1, 100, 1, 1, 100, 1];
        assert_eq!(r.compute_score(T), 10 + 10 + 2 + 2 + 20 + 50);
    }

    /// **The bracket pays 50 all the way up**, because the shipped ladder tests
    /// its smallest threshold first and never reaches the other two. The
    /// disassembly is in [`crate::tables::score_gold_bracket`]; this test used
    /// to assert 100 at 5,001 and 200 at 10,001, which is what the table looks
    /// like rather than what the executable does.
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

    /// `docs/diplomacy.md` §1's headline claim, as arithmetic rather than as
    /// prose: sixteen bytes per sub-record, six sub-records, and the block
    /// finishing exactly on `+0xE4` where the next named field begins.
    ///
    /// Both halves are asserted, because either alone is satisfiable by a
    /// wrong layout: fields that tile 16 bytes prove nothing about the block,
    /// and a block that ends on `0xE4` proves nothing about the fields.
    #[test]
    fn the_pair_block_closes_exactly_on_0xe4() {
        assert_eq!(PAIR_BLOCK_END, 0xE4, "0x84 + 6 * 0x10");

        let mut next = 0;
        for (name, offset, width) in PAIR_FIELDS {
            assert_eq!(offset, next, "{name} does not start where the last field ended");
            next += width;
        }
        assert_eq!(next, PAIR_RECORD_STRIDE, "the fields do not fill sixteen bytes");

        // And the two runs that are gaps rather than fields are still gaps: a
        // future reading that named them would have to change this count.
        let named = PAIR_FIELDS.iter().filter(|(n, _, _)| !n.starts_with('-')).count();
        assert_eq!(named, 9, "nine fields and two untraced gaps");
    }

    /// §0's arithmetic: `lord * 4 + rot - 4` runs 0..=15 over lords 1..=4 and
    /// rotations 0..=3, in four contiguous blocks, with no value repeated.
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
