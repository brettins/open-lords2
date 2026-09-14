#![allow(unused_imports)]
use super::*;

use crate::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_net::{Quirk, Quirks};

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
            peak_counties: 0,
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
            tax_ledger: [0; 2],
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
    /// the arithmetic is reproduced with a wrapping subtraction
    /// guarded, so the shape of the expression stays visible.
    pub fn message_variant(&self) -> u8 {
        (self.lord.wrapping_mul(4)).wrapping_add(self.voice_rotation).wrapping_sub(4)
    }

    /// Advance the voice rotation, wrapping 3 → 0. Every message send does
    /// this,
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

    /// Copy the five score inputs `Realm_UpdateTotals` (`0x0049D1E0`) rebuilds
    /// out of the fields it writes.
    ///
    /// **Index 5 (`+0x4C`) is left alone on purpose**, and the reason has
    /// changed: it used to be *"nothing here knows what it is"* and it is now
    /// *"that one has a different owner"*. It is the castle count and
    /// `Castle_BuildTick` is the only thing in `Lords2.exe` that writes it —
    /// [`crate::tables::SCORE_INPUT_CASTLES`].
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
    ///
    /// **Switchable** — [`Quirk::EmpireTaxHappinessWraps`], `docs/bugs.md` B4.
    /// With the quirk fixed the running total is accumulated in `i32` and
    /// *saturated* into the byte, so sixteen counties at −15 land on −128 and
    /// stay there instead of coming back round as +16. The field stays an `i8`
    /// either way — widening it would change the *save*, and the bug is the
    /// missing clamp
    ///
    /// **The fixed path is not `saturating_add` on the byte**, and the
    /// difference is not pedantic: contributions can be either sign, so a walk
    /// that dips past −128 and climbs back would saturate to a *third* answer
    /// that is neither the original's nor the intended one. [`crate::tax`]
    /// therefore accumulates the realm's whole total in `i32` and calls
    /// [`Realm::set_empire_tax_happiness`] once, and this method is the
    /// original's per-county step.
    pub fn add_empire_tax_happiness(&mut self, contribution: i32, quirks: Quirks) {
        debug_assert!(
            quirks.reproduces(Quirk::EmpireTaxHappinessWraps),
            "the fixed path totals in i32 and calls set_empire_tax_happiness"
        );
        let _ = quirks;
        self.tax_hap_empire = self.tax_hap_empire.wrapping_add(contribution as i8);
    }

    /// Store a realm's empire tax term, clamped into the byte it lives in.
    ///
    /// The fixed half of [`Quirk::EmpireTaxHappinessWraps`]. The field stays an
    /// `i8` — widening it would change the *save*, and the defect is the
    /// missing clamp
    pub fn set_empire_tax_happiness(&mut self, total: i32) {
        self.tax_hap_empire = total.clamp(i8::MIN as i32, i8::MAX as i32) as i8;
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
    /// reward a realm that is winning
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

