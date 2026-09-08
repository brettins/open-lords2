//! The realm record — `docs/kingdom.md` §2.
//!
//! Six records at `0x0057BF00`, stride `0x160`; index 0 is unused and 1..=5 are
//! the players. As with [`crate::county`], the semantics are reproduced and the
//! byte layout is not.

use crate::tables::{
    score_gold_bracket, AI_GOLD_GRANT, SCORE_WEIGHTS, WAGE_DIVISOR_AI, WAGE_DIVISOR_HUMAN,
    WEAPON_TYPE_COUNT,
};

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

/// A realm — one player, human or AI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Realm {
    /// `+0x00` — 0..=14 program counter through the AI's turn;
    /// [`AI_STEP_DONE`] when finished.
    pub ai_step: i32,
    /// `+0x04` — the realm exists.
    pub in_play: bool,
    /// `+0x05` — when set, the AI turn machine is skipped entirely. This is the
    /// byte `docs/battle.md` §6.2 could not explain; it means "a person is
    /// driving this realm".
    pub is_human: bool,
    /// `+0x07` — 0 for the human, 1..=5 for an AI lord, [`LORD_ELIMINATED`]
    /// when knocked out. Indexes `g_aiPersonality` and `g_aiGoldGrant`.
    pub lord: u8,
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
    /// `+0x158` — 0..=5, the escalation in `Wages_PayAll`.
    pub bankrupt_stage: u8,
    /// The six score inputs `docs/kingdom.md` §8.3 could **not** identify,
    /// in the order the weights apply: realm `+0x60, +0x10, +0x0C, +0x58,
    /// +0x54, +0x4C`.
    ///
    /// Deliberately unnamed. Naming them would be exactly the failure mode
    /// `docs/decisions.md` C3 records — a plausible story assembled from
    /// decompiler output.
    pub score_inputs: [i32; 6],
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
            is_human: false,
            lord: LORD_HUMAN,
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
            score_inputs: [0; 6],
        }
    }

    pub fn is_eliminated(&self) -> bool {
        self.lord == LORD_ELIMINATED
    }

    /// True once this realm's AI turn has finished, or immediately when a
    /// person is driving it — `docs/kingdom.md` §3.1 phase 4 waits on this for
    /// every realm.
    pub fn turn_done(&self) -> bool {
        !self.in_play || self.is_human || self.ai_step == AI_STEP_DONE
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
    pub fn wage_for_unit(&self, men: i32, difficulty: u8) -> i32 {
        let divisor = if self.is_human {
            WAGE_DIVISOR_HUMAN
        } else {
            WAGE_DIVISOR_AI[(difficulty as usize).min(WAGE_DIVISOR_AI.len() - 1)]
        };
        men / divisor
    }

    /// The per-season gold grant this realm's lord draws, by difficulty.
    /// The human's lord byte is 0 and row 0 is all zeros, so the human gets
    /// nothing. `docs/kingdom.md` §8.2.
    pub fn gold_grant(&self, difficulty: u8) -> i32 {
        let lord = (self.lord as usize).min(AI_GOLD_GRANT.len() - 1);
        let diff = (difficulty as usize).min(AI_GOLD_GRANT[0].len() - 1);
        AI_GOLD_GRANT[lord][diff]
    }

    /// `Score_RankRealms`' score expression. The six weighted inputs are
    /// unidentified; the gold bracket is the one term whose meaning is
    /// unambiguous. `docs/kingdom.md` §8.3.
    pub fn compute_score(&self) -> i32 {
        let mut score: i64 = 0;
        for i in 0..SCORE_WEIGHTS.len() {
            let (num, den) = SCORE_WEIGHTS[i];
            score += (self.score_inputs[i] as i64 * num as i64) / den as i64;
        }
        score += score_gold_bracket(self.gold) as i64;
        score as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A player measured 250 men -> 62 crowns, 252 -> 63, 254 -> 63.
    /// `docs/kingdom.md` §7.4.
    #[test]
    fn a_human_pays_a_quarter_of_a_crown_a_man() {
        let mut r = Realm::new();
        r.is_human = true;
        assert_eq!(r.wage_for_unit(250, 0), 62);
        assert_eq!(r.wage_for_unit(252, 0), 63);
        assert_eq!(r.wage_for_unit(254, 0), 63);
        // Difficulty does not touch the human divisor.
        for d in 0..=2 {
            assert_eq!(r.wage_for_unit(1000, d), 250);
        }
    }

    #[test]
    fn an_ai_pays_less_the_harder_the_game_is() {
        let mut r = Realm::new();
        r.is_human = false;
        assert_eq!(r.wage_for_unit(300, 0), 100);
        assert_eq!(r.wage_for_unit(300, 1), 60);
        assert_eq!(r.wage_for_unit(300, 2), 30);
        // Out-of-range difficulty clamps rather than panicking.
        assert_eq!(r.wage_for_unit(300, 9), 30);
    }

    #[test]
    fn the_human_realm_draws_no_gold_grant_at_any_difficulty() {
        let mut r = Realm::new();
        r.lord = LORD_HUMAN;
        for d in 0..=3 {
            assert_eq!(r.gold_grant(d), 0);
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
        assert_eq!(r.compute_score(), 10 + 10 + 2 + 2 + 20 + 50);
    }

    #[test]
    fn the_gold_bracket_is_the_only_term_with_a_known_meaning() {
        let mut r = Realm::new();
        for (gold, bonus) in [(0, 0), (2000, 0), (2001, 50), (5000, 50), (5001, 100), (10_000, 100), (10_001, 200)] {
            r.gold = gold;
            assert_eq!(r.compute_score(), bonus, "gold {gold}");
        }
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
