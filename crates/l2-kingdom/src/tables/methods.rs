use super::*;

impl Tables {
    pub const fn ration_happiness(&self, level: i32) -> i32 {
        self.ration_happiness_slope * level + self.ration_happiness_offset
    }

    pub fn health_band(&self, meter: i32) -> u8 {
        for &(up_to, band) in &self.health_band_ladder {
            if meter <= up_to {
                return band as u8;
            }
        }
        self.health_band_ladder[self.health_band_ladder.len() - 1].1 as u8
    }

    pub fn birth_rate(&self, population: i32) -> i32 {
        for &(up_to, percent) in &self.population.birth_rate_ladder {
            if population <= up_to {
                return percent;
            }
        }
        self.population.birth_rate_ladder[self.population.birth_rate_ladder.len() - 1].1
    }

    pub fn happiness_birth_factor(&self, happiness: i32) -> i32 {
        let ladder = &self.population.happiness_factor_ladder;
        for &(below, percent) in &ladder[..ladder.len() - 1] {
            if happiness < below {
                return percent;
            }
        }
        ladder[ladder.len() - 1].1
    }

    pub fn army_happiness_cost(&self, pct: i32) -> i32 {
        if pct <= 0 {
            return self.army_happiness_cost[0];
        }
        self.army_happiness_cost[(pct as usize).min(self.army_happiness_cost.len() - 1)]
    }

    pub fn ai_tax_ladder(&self, lord: u8) -> Option<&TaxLadder> {
        let index = (lord as usize).checked_sub(1)?;
        let row = self.ai.personality.get(index)?;
        self.ai.tax_ladders.get(row.tax_ladder)
    }

    /// **Indexed by the lord byte and never by the realm id.** The two are
    /// unrelated except through the new-game draw; `docs/diplomacy.md` §0.1 is
    /// the trap, and it is the shape of `docs/decisions.md` C3.
    pub fn ai_personality(&self, lord: u8) -> Option<&AiPersonalityRow> {
        self.ai.personality.get((lord as usize).checked_sub(1)?)
    }

    /// The farming style byte a lord stamps on every county he holds — record
    /// `+0x00`, and `None` for a lord byte with no record.
    ///
    /// `Ai_ManageCountyFarms` (`0x0049DD01`) copies this into county `+0x1FE`
    /// and dispatches on it; [`crate::ai_farm::FarmStyle::for_realm`] is that
    /// dispatch. The four values are `[1, 1, 0, 9]`, so **two of the four lords
    /// graze, one ploughs and one mixes**.
    pub fn ai_farm_style(&self, lord: u8) -> Option<u8> {
        Some(self.ai_personality(lord)?.farm_style)
    }

    pub fn score_gold_bracket(&self, gold: i32) -> i32 {
        for &(at_least, points) in &self.score.gold_brackets {
            if gold >= at_least {
                return points;
            }
        }
        0
    }
}

