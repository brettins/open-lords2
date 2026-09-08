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
    /// `+0x158` — 0..=5, the escalation in `Wages_PayAll`. Stage 5 is the
    /// mutiny and it resets to 0 afterwards; it does not saturate.
    pub bankrupt_stage: u8,

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
            population_total: 0,
            population_last: 0,
            population_mean: 0,
            mean_happiness: 0,
            mean_health: 0,
            share_of_map_pct: 0,
            army_count: 0,
            total_men: 0,
            score_inputs: [0; 6],
        }
    }

    pub fn is_eliminated(&self) -> bool {
        self.lord == LORD_ELIMINATED
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

    #[test]
    fn the_gold_bracket_is_the_only_term_with_a_known_meaning() {
        let mut r = Realm::new();
        for (gold, bonus) in [(0, 0), (2000, 0), (2001, 50), (5000, 50), (5001, 100), (10_000, 100), (10_001, 200)] {
            r.gold = gold;
            assert_eq!(r.compute_score(T), bonus, "gold {gold}");
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
