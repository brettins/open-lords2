    use super::*;

    #[test]
    fn the_gathered_tables_agree_with_the_free_functions_everywhere() {
        let t = Tables::DEFAULT;
        for meter in -50..=200 {
            assert_eq!(t.health_band(meter), health_band(meter), "meter {meter}");
        }
        for level in 0..RATION_LEVEL_COUNT as i32 {
            assert_eq!(t.ration_happiness(level), ration_happiness(level));
        }
        for pop in [0, 1, 39, 40, 41, 417, 500, 501, 1999, 2000, 3000, 50_000] {
            assert_eq!(t.birth_rate(pop), birth_rate(pop), "pop {pop}");
        }
        for h in -20..=150 {
            assert_eq!(t.happiness_birth_factor(h), happiness_birth_factor(h), "happiness {h}");
        }
        for gold in [0, 2000, 2001, 5000, 5001, 10_000, 10_001, 1_000_000] {
            assert_eq!(t.score_gold_bracket(gold), score_gold_bracket(gold), "gold {gold}");
        }
        for pct in -10..=200 {
            assert_eq!(t.army_happiness_cost(pct), army_happiness_cost(pct), "pct {pct}");
        }
        for lord in 0..=8u8 {
            assert_eq!(t.ai_tax_ladder(lord), ai_tax_ladder(lord), "lord {lord}");
        }
    }

    #[test]
    fn the_gathered_tables_carry_the_ramp_and_ale_bounds() {
        let t = Tables::DEFAULT;
        assert_eq!(t.efficiency.max, EFFICIENCY_MAX);
        assert_eq!(t.efficiency.without_advanced_farming, EFFICIENCY_WITHOUT_ADVANCED_FARMING);
        assert_eq!(t.ale.step_pct, ALE_HAPPINESS_STEP_PCT);
        assert_eq!(t.ale.max, ALE_HAPPINESS_MAX);
        assert_eq!(t.ai.tax_ladder_neutral, AI_TAX_LADDER_NEUTRAL);
        assert_eq!(t.ai.tax_ladders, AI_TAX_LADDERS);
        for (i, row) in t.ai.personality.iter().enumerate() {
            assert_eq!(row.farm_style, AI_PERSONALITY_FARM_STYLE[i], "lord {}", i + 1);
            assert_eq!(row.tax_ladder, AI_PERSONALITY_TAX_LADDER[i], "lord {}", i + 1);
        }
    }

    #[test]
    fn the_commodity_rows_match_the_commodity_methods() {
        for c in Commodity::ALL {
            let row = Tables::DEFAULT.commodity[c.index()];
            assert_eq!(row.job, c.job(), "{c:?} job");
            assert_eq!(row.divisor, c.divisor(), "{c:?} divisor");
            assert_eq!(row.base_efficiency, c.base_efficiency(), "{c:?} efficiency");
        }
    }
