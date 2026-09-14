use super::*;

// =========================================================================
//  The player's half of the battle: selection, orders, formation, charge.
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::Action;

    fn small_battle() -> BattleRunner {
        BattleRunner::deploy(
            blank_field(),
            &[(Troop::Swordsmen, 6), (Troop::Archers, 4)],
            &[(Troop::Pikemen, 6), (Troop::Peasants, 4)],
        )
    }

    // --- the outcome test, `FUN_00477DFC` --------------------------------

    #[test]
    fn a_battle_with_both_armies_standing_has_not_concluded() {
        let r = small_battle();
        assert_eq!(r.conclusion(), None);
        assert_eq!(r.men_of_side(SIDE_A), 40, "ten figures of four");
        assert_eq!(r.men_of_side(SIDE_B), 40);
    }

    /// The whole of the non-siege rule: a side's men reaching zero.
    #[test]
    fn a_side_with_no_men_left_has_lost_and_the_other_holds_the_field() {
        let mut r = small_battle();
        for f in &mut r.sim.figures {
            if f.side == SIDE_A {
                f.men = 0;
                f.state = State::Dead;
            }
        }
        assert_eq!(r.men_of_side(SIDE_A), 0);
        assert_eq!(
            r.conclusion(),
            Some(Conclusion { winner: SIDE_B, cause: End::Annihilation })
        );
    }

    /// The original tests army A first,
    /// won by B — army A is side 4 here, and `menA < 1` is the first arm.
    #[test]
    fn a_battle_that_kills_everyone_at_once_falls_to_the_side_tested_first() {
        let mut r = small_battle();
        for f in &mut r.sim.figures {
            f.men = 0;
            f.state = State::Dead;
        }
        assert_eq!(r.conclusion().unwrap().winner, SIDE_A, "army B holds an empty field");
    }

    /// Withdrawal outranks annihilation: the original tests `DAT_0056D5C8`
    /// before it looks at either men counter,
    /// withdrawn loses even if the enemy is the one that was wiped out.
    #[test]
    fn a_withdrawal_decides_the_battle_before_the_men_are_counted() {
        let mut r = small_battle();
        for f in &mut r.sim.figures {
            if f.side == SIDE_A {
                f.men = 0;
                f.state = State::Dead;
            }
        }
        r.withdraw(SIDE_B);
        let c = r.conclusion().unwrap();
        assert_eq!(c.cause, End::Withdrawal);
        assert_eq!(c.winner, SIDE_A, "the side that left the field loses whatever the count says");
    }

    /// Siege engines are worth no men: the original's counting loop is
    /// `if (troopType < 7)`,
    #[test]
    fn siege_engines_do_not_count_towards_a_sides_men() {
        let mut r = BattleRunner::deploy_muster(
            blank_field(),
            DEFAULT_SEED,
            Muster { troops: &[(Troop::Catapults, 2)], owner: 1, human: false },
            Muster { troops: &[(Troop::Peasants, 40)], owner: 2, human: true },
        );
        assert!(r.sim.figures.iter().any(|f| f.troop == Troop::Catapults), "they were raised");
        assert_eq!(r.men_of_side(SIDE_B), 0, "and they are worth nobody");
        assert_eq!(r.conclusion().unwrap().winner, SIDE_A);
        r.withdraw(SIDE_A);
        assert_eq!(r.conclusion().unwrap().winner, SIDE_B);
    }

    // --- raising from real men, `docs/battle.md` §5.1 and §5.2 ------------

/// The last figure of a unit takes the remainder
    /// complement, so the men that go in are the men that come out.
    #[test]
    fn the_last_figure_of_a_unit_carries_the_remainder() {
        // 306 + 200 = 506, between the ladder's first two breaks at 305 and 609:
        // class 1, eight men a figure.
        let a = [(Troop::Peasants, 306u32)];
        let b = [(Troop::Peasants, 200u32)];
        let r = BattleRunner::deploy_muster(
            blank_field(),
            DEFAULT_SEED,
            Muster { troops: &a, owner: 1, human: false },
            Muster { troops: &b, owner: 2, human: true },
        );
        // Army A deploys as side 4 - `Battle_InitArmies`.
        assert_eq!(r.men_per_figure(SIDE_B), 8);
        assert_eq!(r.men(SIDE_B), 306, "not 312, which 39 full figures of eight would give");
        assert_eq!(r.survivors(SIDE_B)[Troop::Peasants.index()], 306);
        // ceil(306 / 8) = 39 figures, cut into units of at most twelve.
        assert_eq!(r.sim.figures.iter().filter(|f| f.side == SIDE_B).count(), 39);
    }

    /// A side small enough to be nearly invisible halves its scale on its own —
    /// and the *other* side keeps the scale the pair chose.
    #[test]
    fn a_small_side_beside_a_large_one_gets_its_own_finer_scale() {
        // 1000 + 40 = 1040, under the third break at 1217: class 2, sixteen a
        // figure. The small side would draw two figures at that scale.
        let a = [(Troop::Peasants, 1000u32)];
        let b = [(Troop::Peasants, 40u32)];
        let r = BattleRunner::deploy_muster(
            blank_field(),
            DEFAULT_SEED,
            Muster { troops: &a, owner: 1, human: false },
            Muster { troops: &b, owner: 2, human: true },
        );
        assert_eq!(size_class(1040), 2);
        assert_eq!(r.men_per_figure(SIDE_B), 16, "the large side keeps the pair's scale");
        assert_eq!(r.men_per_figure(SIDE_A), 8, "and the small one halves it");
        assert_eq!(r.men(SIDE_A), 40, "nobody lost either way");
        assert_eq!(r.men(SIDE_B), 1000);
    }

    /// `deploy_muster` walks [`RAISE_ORDER`] whatever order the caller wrote
    /// the troops in — the tail of that order is what gets truncated at the
    /// eighty-figure ceiling, so it is not cosmetic.
    #[test]
    fn the_raise_order_is_the_binarys_and_not_the_callers() {
        let a = [(Troop::Peasants, 40u32), (Troop::Knights, 40)];
        let b = [(Troop::Peasants, 40u32)];
        let r = BattleRunner::deploy_muster(
            blank_field(),
            DEFAULT_SEED,
            Muster { troops: &a, owner: 1, human: false },
            Muster { troops: &b, owner: 2, human: true },
        );
        let first = r.sim.figures.iter().find(|f| f.side == SIDE_B).unwrap();
        assert_eq!(first.troop, Troop::Knights, "knights are raised before peasants");
    }

    #[test]
    fn deployment_puts_every_figure_on_its_own_passable_cell() {
        let r = small_battle();
        assert_eq!(r.fighters.len(), 20);
        let mut seen = std::collections::HashSet::new();
        for f in &r.fighters {
            assert!(seen.insert((f.x, f.y)), "two figures on {:?}", (f.x, f.y));
            assert!(!r.field.at(f.x as usize, f.y as usize).impassable());
            assert!(f.x < 80 && f.y < 80);
        }
        assert_eq!(r.occupant.iter().filter(|o| o.is_some()).count(), 20);
    }

    /// The unit layer, which is what the AI dispatches on. Six swordsmen are
    /// one unit (the ceiling is eight); four archers are another; and the
    /// categories are the ones `g_unitOrderTableField` is indexed by.
    #[test]
    fn each_troop_group_becomes_a_unit_of_at_most_its_ceiling() {
        let r = small_battle();
        let live: Vec<usize> = r.units.live().collect();
        assert_eq!(live.len(), 4, "two units a side");
        let sizes: Vec<u8> = live.iter().map(|&u| r.units.get(u).figures).collect();
        assert_eq!(sizes, vec![6, 4, 6, 4]);
        // Categories: swordsmen 3, archers 1, pikemen 2, peasants 2.
        let cats: Vec<u8> = live.iter().map(|&u| r.units.get(u).category).collect();
        assert_eq!(cats, vec![3, 1, 2, 2]);
        // Every figure belongs to exactly one of them, and no figure is loose.
        for f in &r.sim.figures {
            assert!(f.unit >= 1 && f.unit as usize <= MAX_UNITS, "figure with no unit");
        }
        // Side 4 is the AI and side 0 the player.
        assert!(!r.units.get(live[0]).human && r.units.get(live[0]).side == SIDE_B);
        assert!(r.units.get(live[2]).human && r.units.get(live[2]).side == SIDE_A);
    }

    /// A unit bigger than its ceiling splits, and each part takes its own
    /// deployment slot. Twenty peasants is two units of twelve and eight, not
    /// one of twenty — `MAX_FIGURES_PER_UNIT[peasants] == 12`.
    #[test]
    fn an_oversized_troop_group_splits_into_units_at_the_ceiling() {
        let r = BattleRunner::deploy(blank_field(), &[(Troop::Peasants, 20)], &[]);
        let live: Vec<usize> = r.units.live().collect();
        assert_eq!(live.len(), 2);
        assert_eq!(r.units.get(live[0]).figures, 12);
        assert_eq!(r.units.get(live[1]).figures, 8);
        // And they are not on top of one another: different marker slots.
        let a = (r.units.get(live[0]).x, r.units.get(live[0]).y);
        let b = (r.units.get(live[1]).x, r.units.get(live[1]).y);
        assert_ne!(a, b, "both units deployed on the same slot");
    }

    #[test]
    fn the_two_sides_deploy_at_opposite_markers_and_face_each_other() {
        let r = small_battle();
        let side4: Vec<u8> = r.fighters.iter().filter(|f| f.side == SIDE_B).map(|f| f.y).collect();
        let side0: Vec<u8> = r.fighters.iter().filter(|f| f.side == SIDE_A).map(|f| f.y).collect();
        assert!(!side4.is_empty() && !side0.is_empty());
        // Side 0 is the 0x04 marker at y = 20; side 4 is 0x0F at y = 60.
        assert!(side0.iter().all(|&y| y < 40), "side 0 should be at the low end: {side0:?}");
        assert!(side4.iter().all(|&y| y > 40), "side 4 should be at the high end: {side4:?}");
        // `BattleMan_Create` picks the facing from the row: north of 41 a man
        // faces south (4), south of it he faces north (0).
        for f in &r.fighters {
            assert_eq!(f.facing, if f.y < 41 { 4 } else { 0 }, "at {:?}", (f.x, f.y));
        }
        // And nobody has been ordered anywhere yet: `BattleUnit_Recentre` seeds
// an un-ordered unit's destination from its own position, so
        // an army stands still until something tells it not to.
        for u in r.units.live() {
            let unit = r.units.get(u);
            assert_eq!((unit.target_x, unit.target_y), (unit.x, unit.y));
        }
    }

    /// **The commit is the move, and the delay is what follows it.**
    /// `BattleMan_Step` (`0x0048F1DD`) calls `FUN_00491B1F` — which rewrites
    /// `mapX`/`mapY` — the tick `Cell_TryEnter` says the cell is free, and
    /// only then counts `walking` 1, 3 … 15. So an ordered pikeman is on the
    /// next cell at once and stays there for 40 ticks.
    ///
    /// This used to assert the opposite — *"nobody moves before their troop's
    /// move delay has elapsed"* — which is the order our runner had and not
    /// the original's.
    #[test]
    fn a_committed_man_is_on_the_next_cell_at_once_and_holds_it_for_the_delay() {
        let mut r = BattleRunner::deploy(
            blank_field(),
            &[(Troop::Pikemen, 1)],
            &[(Troop::Pikemen, 1)],
        );
        r.order_unit(r.unit_of(0), 40, 70);
        let start = (r.fighters[0].x, r.fighters[0].y);
        let mut moves = Vec::new();
        let mut at = start;
        for t in 1..=90 {
            r.step();
            let now = (r.fighters[0].x, r.fighters[0].y);
            if now != at {
                moves.push(t);
                at = now;
            }
        }
        assert_ne!(start, at, "he never moved");
        // One cell on the commit, then one every 8 * (moveDelay + 1) = 40.
        let first = moves[0];
        assert!(first <= 2, "the commit did not move him: first move at tick {first}");
        let gaps: Vec<u32> = moves.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(gaps.iter().all(|&g| g == 40), "a pikeman's cells came {gaps:?} ticks apart");
    }

    #[test]
    fn a_knight_crosses_five_cells_while_a_pikeman_crosses_one() {
        // Past the first cell, which every troop gets on the commit itself.
        fn cells_after_the_first(troop: Troop, ticks: u32) -> i32 {
            let mut r = BattleRunner::deploy(blank_field(), &[(troop, 1)], &[]);
            r.order_unit(r.unit_of(0), 40, 20);
            let start = r.fighters[0].y as i32;
            r.run(1);
            let committed = r.fighters[0].y as i32;
            assert_ne!(committed, start, "{troop:?} did not commit on its first tick");
            r.run(ticks);
            (r.fighters[0].y as i32 - committed).abs()
        }
        assert_eq!(cells_after_the_first(Troop::Knights, 40), 5);
        assert_eq!(cells_after_the_first(Troop::Pikemen, 40), 1);
    }

    /// The AI advances of its own accord — nothing here orders anybody. The
    /// only thing that moves side 4 is `Battle_UpdateAllUnits` dispatching
    /// `UnitOrder_FieldMelee` on its swordsmen.
    #[test]
    fn the_ai_marches_on_the_enemy_without_being_told_to() {
        let mut r = small_battle();
        let gap = |r: &BattleRunner| -> i32 {
            let a = r.fighters.iter().filter(|f| f.side == SIDE_B).map(|f| f.y as i32).min().unwrap();
            let b = r.fighters.iter().filter(|f| f.side == SIDE_A).map(|f| f.y as i32).max().unwrap();
            a - b
        };
        let before = gap(&r);
        // First order at frame 1000, then forty cells at 36 ticks a cell for a
        // swordsman: contact lands a little after 2400.
        //
        // > **Contact is watched for, not sampled at frame 3,000.** It used to
        // > be `any(anim == Attacking)` on the state at exactly 3,000, and that
// > is a snapshot of an emergent timing
        // > is named for. It went red the day blocked figures started detouring
        // > around each other instead of standing still —
        // > closed *sooner* and the whole fight was over by 3,000, with every
        // > survivor already `Dying`. The claim is *"they close and they
        // > fight"*; sampling one frame tests *"they are still fighting at this
        // > particular frame"*, which is a different and much weaker thing.
        let mut engaged = false;
        for _ in 0..30 {
            r.run(100);
            engaged |= r.fighters.iter().any(|f| f.anim == Motion::Attacking);
        }
        let after = gap(&r);
        assert!(after < before, "the armies did not close: {before} -> {after}");
        assert!(engaged, "nobody ever engaged");
    }

    /// The cadence, which is the AI's most distinctive property: a unit decides
    /// once every 200 frames and does nothing at all in between.
    ///
    /// The two silent thinks at the start are `docs/battle-ai.md` §12's first
    /// erratum — `UnitOrder_FieldMelee` calls `Order_DoNothing` while
    /// `orders < 4`, so the first *order* a swordsman unit issues is on its
    /// fifth think, at frame 1000.
    #[test]
    fn a_unit_thinks_once_every_two_hundred_frames_and_not_before() {
        let mut r = small_battle();
        let swords = r.units.live().next().unwrap();
        assert_eq!(r.units.get(swords).category, 3, "the swordsman unit");

        r.run(199);
        assert_eq!(r.units.get(swords).orders, 0, "no think inside the first 200 frames");
        r.step();
        assert_eq!(r.units.get(swords).orders, 1, "the 200th frame is the first think");
        assert_eq!(r.ai.last_action[swords], Action::DoNothing);

        r.run(800);
        assert_eq!(r.units.get(swords).orders, 5, "five thinks by frame 1000");
        assert!(
            matches!(r.ai.last_action[swords], Action::ToEnemyEnd(_)),
            "the fifth think should march: {:?}",
            r.ai.last_action[swords]
        );
// And that order reached the men.
        let marching = r
            .fighters
            .iter()
            .enumerate()
            .filter(|&(i, _)| r.unit_of(i) == swords)
            .any(|(_, f)| f.target != (f.x, f.y));
        assert!(marching, "the unit was ordered but no figure was given a destination");
    }

    /// The one number the whole AI turns on, recomputed on the 101st frame and
    /// not the 100th.
    #[test]
    fn the_strength_advantage_is_recomputed_every_hundred_and_first_frame() {
        let mut r = small_battle();
        assert_eq!(r.ai.strength_advantage, 0, "nothing computed before the first frame");
        r.run(100);
        assert_eq!(r.ai.advantage_timer, 100, "counted up, not yet fired");
        assert_eq!(r.ai.strength_advantage, 0);
        r.step();
        assert_eq!(r.ai.advantage_timer, 0, "fired on frame 101");
        // Six swordsmen (weight 3) and four archers (2) against six pikemen (2)
        // and four peasants (1), four men a figure: 104 against 64, so +62
        // before the -10..+21 jitter.
        assert!(
            (52..=83).contains(&r.ai.strength_advantage),
            "advantage {} outside 62 plus the jitter",
            r.ai.strength_advantage
        );
        assert!(r.ai.strength_advantage > crate::ai::AGGRESSION_THRESHOLD);
    }

    /// Contact drops the rest of an **AI** unit into free pursuit and leaves a
    /// player's unit formed up — `docs/battle-ai.md` §4.2, the single biggest
    /// visible difference between how the two sides fight.
    #[test]
    fn contact_dissolves_the_ai_unit_and_not_the_players() {
        let mut r = small_battle();
        for _ in 0..30_000 {
            r.step();
            if r.fighters.iter().any(|f| f.anim == Motion::Attacking) {
                break;
            }
        }
        assert!(
            r.fighters.iter().any(|f| f.anim == Motion::Attacking),
            "the armies never met"
        );
        let chasing = |side: Side, r: &BattleRunner| {
            r.fighters
                .iter()
                .enumerate()
                .filter(|(i, f)| f.side == side && r.is_alive(*i))
                .filter(|(i, _)| r.sim.figures[r.fighters[*i].sim].state == State::Chasing)
                .count()
        };
        assert!(chasing(SIDE_B, &r) > 0, "the AI unit did not dissolve on contact");
        assert_eq!(chasing(SIDE_A, &r), 0, "a player's unit must not be dissolved");
    }

    #[test]
    fn a_battle_ends_with_one_side_standing() {
        let mut r = BattleRunner::deploy(
            blank_field(),
            &[(Troop::Knights, 8)],
            &[(Troop::Peasants, 8)],
        );
        for _ in 0..60_000 {
            r.step();
            if r.is_decided() {
                break;
            }
        }
        assert!(r.is_decided(), "battle never resolved");
        assert_eq!(r.living(SIDE_A), 0, "knights should have beaten peasants");
        assert!(r.living(SIDE_B) > 0);
        // Everybody who died is drawn falling, and has released their cell.
        for (i, f) in r.fighters.iter().enumerate() {
            if !r.is_alive(i) {
                assert_eq!(f.anim, Motion::Dying);
            }
        }
    }

    /// The property lockstep depends on. Two runs from the same setup must
    /// reach bit-identical state, with nothing leaking in from allocation
    /// order, hashing or the clock.
    #[test]
    fn two_runs_of_the_same_battle_stay_identical() {
        let mut a = small_battle();
        let mut b = small_battle();
        for _ in 0..3_000 {
            a.step();
            b.step();
            assert_eq!(a.fighters, b.fighters, "diverged at tick {}", a.tick);
            assert_eq!(a.sim, b.sim, "simulation diverged at tick {}", a.tick);
            assert_eq!(a.units, b.units, "units diverged at tick {}", a.tick);
            assert_eq!(a.ai, b.ai, "the AI diverged at tick {}", a.tick);
        }
    }

    /// And a different seed is a different battle: the jitter is real state,
    /// not decoration. If it ever stopped reaching a decision this would stop
    /// failing.
    #[test]
    fn a_different_seed_reaches_a_different_strength_advantage() {
        let armies = || {
            (
                vec![(Troop::Swordsmen, 4)],
                vec![(Troop::Swordsmen, 4)],
            )
        };
        let run = |seed: u64| {
            let (a, b) = armies();
            let mut r = BattleRunner::deploy_armies(
                blank_field(),
                seed,
                Army { troops: &a, owner: 1, human: false },
                Army { troops: &b, owner: 2, human: true },
            );
            r.run(101);
            r.ai.strength_advantage
        };
        assert_ne!(run(1), run(2), "the seed does not reach the advantage");
    }

    /// **A lopsided fight is seed-invariant because the threshold swallows the
    /// jitter,
    ///
    /// Seven seeds at 400 v 200 and at 200 v 400 gave identical survivors and
    /// tick counts; only an even fight varied. The roll is still drawn:
    /// `Battle_UpdateStrengthAdvantage` (`0x0047FC01`) runs every hundred and
    /// first frame and adds `(rand & 0x1F) - 10`, a span of 31 either side of
    /// nothing. Two to one is an advantage of ±50 … 100, so no draw can carry
    /// it across [`AGGRESSION_THRESHOLD`] (5) and every unit takes the same
    /// branch in every battle — the aggressive one in `unit_order_advance`
    /// (`ai.rs`), the cautious stand below it. An even fight sits inside the
    /// jitter's reach and is the only place a seed decides anything.
    ///
    /// **Ablation.** Neutralise the `+= (rng & 0x1F) - 10` — keeping the draw,
    /// so the generator still advances — and two seeds reach the same
    /// advantage of 100: measured, `assertion left != right failed: 40 v 20`.
    /// The even-fight half then goes red too, every seed on one branch.
    #[test]
    fn a_lopsided_fight_draws_its_jitter_and_the_threshold_swallows_it() {
        let run = |seed: u64, ai_figs: u16, human_figs: u16| {
            let a = vec![(Troop::Swordsmen, ai_figs)];
            let b = vec![(Troop::Swordsmen, human_figs)];
            let mut r = BattleRunner::deploy_armies(
                blank_field(),
                seed,
                Army { troops: &a, owner: 1, human: false },
                Army { troops: &b, owner: 2, human: true },
            );
            let fresh = r.ai.rng.clone();
            r.run(101);
            let adv = r.ai.strength_advantage;
            (adv, r.ai.rng != fresh, adv > crate::ai::AGGRESSION_THRESHOLD)
        };

        for (ai_figs, human_figs) in [(40u16, 20u16), (20, 40)] {
            let (adv1, drew1, branch1) = run(1, ai_figs, human_figs);
            let (adv2, drew2, branch2) = run(2, ai_figs, human_figs);
            assert!(drew1 && drew2, "{ai_figs} v {human_figs}: the generator never advanced");
            assert_ne!(adv1, adv2, "{ai_figs} v {human_figs}: the seed did not reach the jitter");
            assert_eq!(branch1, branch2, "{ai_figs} v {human_figs}: two seeds, two branches");
            // 31 is the jitter's whole span, so this is *why* the branch holds.
            assert!(
                (adv1 - crate::ai::AGGRESSION_THRESHOLD).abs() > 31,
                "{ai_figs} v {human_figs}: advantage {adv1} is within the jitter's reach"
            );
        }

        // And the even fight, where the same draw does decide.
        let branches: Vec<bool> = (1..=7).map(|s| run(s, 30, 30).2).collect();
        assert!(
            branches.iter().any(|&b| b) && branches.iter().any(|&b| !b),
            "an even fight should split on the seed: {branches:?}"
        );
    }

    #[test]
    fn a_wall_of_obstacles_is_routed_around_rather_than_walked_through() {
        let mut layer = vec![0u8; terrain::CELLS];
        layer[20 * DIM + 40] = 0x04;
        layer[60 * DIM + 40] = 0x0F;
        // A wall across the middle with one gap, well clear of both markers.
        for x in 5..70 {
            layer[40 * DIM + x] = 0x02;
        }
        for x in 60..70 {
            layer[40 * DIM + x] = 0x00;
        }
        let field = terrain::build(&layer, 1);
        // Side 0 (the player's side, at the y = 20 marker) ordered across the
        // wall, so this tests the pathfinder and not the AI.
        let mut r = BattleRunner::deploy(field, &[], &[(Troop::Knights, 3)]);
        r.order_side(SIDE_A, 40, 70);
        r.run(3_000);
        for f in &r.fighters {
            assert!(
                !r.field.at(f.x as usize, f.y as usize).impassable(),
                "a figure stands on impassable ground at {:?}",
                (f.x, f.y)
            );
        }
        assert!(
            r.fighters.iter().any(|f| f.y > 40),
            "nobody got past the wall: {:?}",
            r.fighters.iter().map(|f| (f.x, f.y)).collect::<Vec<_>>()
        );
        assert!(r.fighters.iter().any(|f| f.reroutes > 0), "nobody ever asked for a route");
    }

    /// A reform re-issues the whole unit onto a rectangle around its
    /// destination, and the rectangle is the one `Formation_ComputeRect`
    /// builds — not a heap of figures on one cell.
    #[test]
    fn an_order_forms_the_unit_up_on_a_rectangle_around_the_destination() {
        let mut r = BattleRunner::deploy(blank_field(), &[], &[(Troop::Pikemen, 10)]);
        let unit = r.unit_of(0);
        r.order_unit(unit, 40, 40);
        let targets: Vec<(u8, u8)> = (0..r.fighters.len())
            .map(|i| r.fighters[i].target)
            .collect();
        let unique: std::collections::HashSet<_> = targets.iter().collect();
        assert_eq!(unique.len(), 10, "every figure needs its own slot: {targets:?}");
        // Pikemen: footprint 1, five to a row,
        // destination.
        let xs: Vec<u8> = targets.iter().map(|t| t.0).collect();
        let ys: Vec<u8> = targets.iter().map(|t| t.1).collect();
        let (x0, x1) = (*xs.iter().min().unwrap(), *xs.iter().max().unwrap());
        let (y0, y1) = (*ys.iter().min().unwrap(), *ys.iter().max().unwrap());
        assert_eq!((x1 - x0, y1 - y0), (4, 1), "a 5 x 2 rectangle");
        assert!((x0..=x1).contains(&40) && (y0..=y1).contains(&40), "centred on (40, 40)");
    }

    // --- missiles ---------------------------------------------------------

    /// Two figures standing still, `gap` cells apart, and nothing else on the
    /// field. The archers do not have to walk anywhere, so the only thing that
    /// can happen is that they shoot.
    fn firing_line(shooter: Troop, target: Troop, gap: u8) -> BattleRunner {
        let mut r = BattleRunner::empty(blank_field(), DEFAULT_SEED);
        let a = r.sim.add(shooter, SIDE_A, 4).unwrap();
        let b = r.sim.add(target, SIDE_B, 4).unwrap();
        r.sim.figures[a].owner = 1;
        r.sim.figures[b].owner = 2;
        for (sim, x) in [(a, 20u8), (b, 20 + gap)] {
            r.fighters.push(Fighter {
                sim,
                troop: r.sim.figures[sim].troop,
                side: r.sim.figures[sim].side,
                x,
                y: 40,
                target: (x, 40),
                facing: 2,
                progress: Progress::default(),
                anim: Motion::Idle,
                phase: 0,
                facing_drawn: 0,
                fidget: 0,
                fidget_period: 0xB4,
                path: Vec::new(),
                barred: 0,
                hold: 0,
                reroutes: 0,
                moat_cell: None,
                moat_load: 0,
                polar: 0,
                corpse: 0,
            });
            r.occupant[40 * DIM + x as usize] = Some((r.fighters.len() - 1) as u16);
        }
        r.settle();
        r
    }

    /// **The gap this whole change exists to close.** An arrow leaves the bow,
    /// crosses the ground and kills somebody who is out of reach of a sword.
    ///
    /// Ten cells apart is inside a bow's fifteen and outside anybody's arm, so
    /// nothing but a missile can produce a casualty here at all.
    #[test]
    fn an_archer_kills_a_man_ten_cells_away_and_a_swordsman_cannot() {
        let mut bows = firing_line(Troop::Archers, Troop::Peasants, 10);
        bows.run(600);
        assert!(bows.men_of_side(SIDE_B) < 4, "the peasants should be losing men");
        assert!(bows.missiles.live() > 0 || bows.tick > 0, "arrows are being loosed");

        let mut swords = firing_line(Troop::Swordsmen, Troop::Peasants, 10);
        swords.run(600);
        assert_eq!(swords.men_of_side(SIDE_B), 4, "a sword does not reach ten cells");
        assert_eq!(swords.missiles.live(), 0, "and looses nothing");
    }

/// The manual's sentence, now measurable in the simulation
    /// in the table: *archers have greater range and a faster rate of fire than
    /// crossbowmen but do less damage per shot.*
    ///
    /// At nine cells the bow reaches and the crossbow does not.
    #[test]
    fn a_bow_reaches_nine_cells_and_a_crossbow_does_not() {
        let mut bow = firing_line(Troop::Archers, Troop::Peasants, 9);
        bow.run(600);
        let mut xbow = firing_line(Troop::Crossbowmen, Troop::Peasants, 9);
        xbow.run(600);
        assert!(bow.men_of_side(SIDE_B) < 4, "fifteen cells of range");
        assert_eq!(xbow.men_of_side(SIDE_B), 4, "eight, and nine is out of it");
    }

    /// **A body in the flight path takes the arrow.** The whole reason this had
    /// to be established before anything was written: the missile reads the
    /// figure out of the cell it enters, so an enemy standing between the
    /// shooter and its chosen target is hit instead.
    ///
    /// The screen is put in *after* the shot is loosed, so the acquisition
    /// cannot have chosen it.
    #[test]
    fn a_body_that_walks_into_the_flight_path_takes_the_arrow() {
        let mut r = firing_line(Troop::Archers, Troop::Peasants, 12);
        // Fire one volley, then interpose a second enemy four cells out.
        r.run(WeaponClass::Bow.stats().reload as u32 + 2);
        assert!(r.missiles.live() > 0, "an arrow should be in the air");
        let target_sim = r.sim.figures[r.fighters[0].sim].target.expect("a chosen target");
        let screen = r.sim.add(Troop::Peasants, SIDE_B, 4).unwrap();
        r.sim.figures[screen].owner = 2;
        r.fighters.push(Fighter {
            sim: screen,
            troop: Troop::Peasants,
            side: SIDE_B,
            x: 24,
            y: 40,
            target: (24, 40),
            facing: 6,
            progress: Progress::default(),
            anim: Motion::Idle,
            phase: 0,
            facing_drawn: 0,
            fidget: 0,
            fidget_period: 0xB4,
            path: Vec::new(),
            barred: 0,
            hold: 0,
            reroutes: 0,
            moat_cell: None,
            moat_load: 0,
            polar: 0,
            corpse: 0,
        });
        r.occupant[40 * DIM + 24] = Some((r.fighters.len() - 1) as u16);
        let before = r.sim.figures[screen].hits;
        r.run(60);
        assert!(
            r.sim.figures[screen].hits > before || r.sim.figures[screen].men < 4,
            "the interposed man should have been hit, not the one aimed at"
        );
        assert_ne!(screen, target_sim, "and he is not the one that was aimed at");
    }

    /// **An arrow does not stop where it was aimed.** Once the Bresenham line is
    /// spent the missile coasts along its launch direction,
    /// *behind* the target is in danger too.
    #[test]
    fn a_shot_that_misses_keeps_flying_past_the_target() {
        let mut ms = crate::missile::Missiles::new();
        let slot =
            missile::spawn(&mut ms, 1, WeaponClass::Bow, 1, (10, 40), (20, 40), 50, 0).unwrap();
        assert_eq!(ms.get(slot).dir, 2, "due east");
        // Ten cells is 320 sub-cell units, so 400 sub-steps is well past the
        // impact point — and the missile is still going.
        for _ in 0..400 {
            ms.get_mut(slot).sub_step();
        }
        assert!(
            ms.get(slot).cell_x > 20,
            "it should have overshot: at {}",
            ms.get(slot).cell_x
        );
    }

    /// A missile is retired by its range and by nothing else when it meets
    /// nobody —
    #[test]
    fn an_arrow_that_hits_nothing_dies_at_the_end_of_its_range() {
        let mut r = firing_line(Troop::Archers, Troop::Peasants, 40);
        // Forty cells is well outside a bow's fifteen, so nothing is ever
        // acquired and nothing is ever loosed.
        r.run(400);
        assert_eq!(r.missiles.live(), 0, "nothing to shoot at, nothing in the air");
        assert_eq!(r.men_of_side(SIDE_B), 4);
    }

    /// A shot cannot hit a figure of its own owner, and is not consumed by one
    /// either — the test is on the owner byte, not the side.
    #[test]
    fn an_arrow_passes_through_a_friendly_body() {
        let mut r = firing_line(Troop::Archers, Troop::Peasants, 12);
        // A friendly standing directly in front of the archer.
        let friend = r.sim.add(Troop::Peasants, SIDE_A, 4).unwrap();
        r.sim.figures[friend].owner = 1;
        r.fighters.push(Fighter {
            sim: friend,
            troop: Troop::Peasants,
            side: SIDE_A,
            x: 22,
            y: 40,
            target: (22, 40),
            facing: 2,
            progress: Progress::default(),
            anim: Motion::Idle,
            phase: 0,
            facing_drawn: 0,
            fidget: 0,
            fidget_period: 0xB4,
            path: Vec::new(),
            barred: 0,
            hold: 0,
            reroutes: 0,
            moat_cell: None,
            moat_load: 0,
            polar: 0,
            corpse: 0,
        });
        r.occupant[40 * DIM + 22] = Some((r.fighters.len() - 1) as u16);
        r.run(600);
        assert_eq!(r.sim.figures[friend].men, 4, "friendly fire is impossible");
        assert_eq!(r.sim.figures[friend].hits, 0);
        assert!(r.men_of_side(SIDE_B) < 4, "and the arrows got past him");
    }

    /// The reload cycle, to the tick: nothing is in the air before the interval
    /// expires and something is immediately after.
    #[test]
    fn nothing_is_loosed_before_the_reload_interval_expires() {
        let reload = WeaponClass::Bow.stats().reload as u32;
        let mut r = firing_line(Troop::Archers, Troop::Peasants, 10);
        r.run(reload);
        assert_eq!(r.missiles.live(), 0, "not yet");
        r.run(1);
        assert_eq!(r.missiles.live(), 1, "and now");
    }

    /// Determinism, with arrows in it. The property lockstep depends on, over
    /// the state this change added.
    #[test]
    fn two_runs_of_a_battle_with_missiles_stay_identical() {
        let build = || {
            BattleRunner::deploy(
                blank_field(),
                &[(Troop::Archers, 6), (Troop::Swordsmen, 4)],
                &[(Troop::Crossbowmen, 6), (Troop::Peasants, 4)],
            )
        };
        let (mut a, mut b) = (build(), build());
        for _ in 0..2_000 {
            a.step();
            b.step();
            assert_eq!(a.missiles, b.missiles, "diverged at tick {}", a.tick);
        }
        assert_eq!(a.fighters, b.fighters);
        assert_eq!(a.sim, b.sim);
    }

    /// A catapult cannot hurt a man. `Missile_Step`'s hit test is gated on
    /// `class < 3`, so its shot passes straight through a crowd.
    #[test]
    fn a_catapult_shot_cannot_hurt_a_man() {
        let mut r = firing_line(Troop::Catapults, Troop::Peasants, 10);
        r.run(1_000);
        assert_eq!(r.men_of_side(SIDE_B), 4, "a catapult is a wall-breaker only");
    }

    // --- cues ---------------------------------------------------------------

    /// **An arrow is cued as it leaves, as it lands and as it kills** — the
    /// three occasions `BattleMan_FireMissile` and `Missile_Step` sound on.
    ///
    /// Every tick is checked, not only the end: a loose must move the loose
    /// count on exactly the tick a missile appears, and a man lost to an arrow
    /// must move the hit and casualty counts on that tick. Ablation: delete
    /// `self.sim.cues.loose(class)` and the first assertion names the tick.
    #[test]
    fn an_arrow_is_cued_when_it_leaves_when_it_lands_and_when_it_kills() {
        for (troop, class) in [(Troop::Archers, WeaponClass::Bow), (Troop::Crossbowmen, WeaponClass::Crossbow)] {
            let mut r = firing_line(troop, Troop::Peasants, 6);
            let target = r.fighters[1].sim;
            let mut deaths = 0;
            for _ in 0..3_000 {
                let (was, men, live) = (r.sim.cues, r.sim.figures[target].men, r.missiles.live());
                r.step();
                let now = r.sim.cues;
                if r.missiles.live() > live {
                    assert_eq!(now.loosed(class), was.loosed(class) + 1, "{troop:?} loosed at {} uncued", r.tick);
                }
                if r.sim.figures[target].men < men {
                    assert!(now.missile_hits(class) > was.missile_hits(class), "a man fell to no hit at {}", r.tick);
                    assert!(now.missile_casualties(class) > was.missile_casualties(class));
                }
                if men > 0 && r.sim.figures[target].men == 0 {
                    assert_eq!(now.missile_deaths(), was.missile_deaths() + 1);
                    deaths += 1;
                }
                // And nothing is cued for the other weapon.
                let other = if class == WeaponClass::Bow { WeaponClass::Crossbow } else { WeaponClass::Bow };
                assert_eq!(now.loosed(other), 0);
                assert_eq!(now.missile_hits(other), 0);
            }
            assert_eq!(deaths, 1, "{troop:?} should have killed the peasants in 3,000 ticks");
            // An arrow's 50 takes two hits a peasant and a bolt's 200 takes two
            // peasants a hit, so four men are at least eight arrows or two bolts.
            let least = if class == WeaponClass::Bow { 8 } else { 2 };
            assert!(r.sim.cues.missile_hits(class) >= least, "four men need at least {least} hits");
            assert_eq!(r.sim.cues.melee_casualties(troop), 0, "nobody came to blows");
        }
    }

    /// A catapult's shot is cued as loosed and never as a hit on a man.
    #[test]
    fn a_catapult_is_cued_as_it_fires_and_never_as_hitting_a_man() {
        let mut r = firing_line(Troop::Catapults, Troop::Peasants, 10);
        r.run(1_000);
        assert!(r.sim.cues.loosed(WeaponClass::Catapult) > 0, "the catapult fired");
        assert_eq!(r.sim.cues.missile_deaths(), 0);
        assert_eq!(r.sim.cues.loosed(WeaponClass::Bow), 0);
    }

    /// **The simulation never reads its cues.** Two copies of one battle with
    /// arrows and swords in it; one has its record wiped before every tick.
    /// Every other part of the runner must agree at every tick.
    ///
    /// This is the property `docs/netcode.md` D-3 needs from an event stream a
    /// listener reads — nothing it records may reach a decision — and it is
/// checked on the running battle
/// uses. Ablation, by insertion: make
    /// [`BattleRunner::step`] skip its missile sweep while
    /// `self.sim.cues.missile_hits(WeaponClass::Bow) > 0` and this goes red on
    /// the tick after the first hit.
    #[test]
    fn a_battle_whose_cues_are_wiped_every_tick_is_the_same_battle() {
        let build = || {
            BattleRunner::deploy(
                blank_field(),
                &[(Troop::Archers, 6), (Troop::Swordsmen, 4)],
                &[(Troop::Crossbowmen, 6), (Troop::Macemen, 4)],
            )
        };
        let (mut heard, mut wiped) = (build(), build());
        for t in 0..6_000u32 {
            // The same two orders to both copies, so the armies meet: a battle
            // that only stands and shoots never exercises the melee record.
            if t % 300 == 0 {
                for r in [&mut heard, &mut wiped] {
                    r.order_side(SIDE_A, 40, 40);
                    r.order_side(SIDE_B, 40, 40);
                }
            }
            wiped.sim.cues = crate::cue::Cues::default();
            heard.step();
            wiped.step();
            let mut same = wiped.clone();
            same.sim.cues = heard.sim.cues;
            assert_eq!(heard, same, "the cues changed the battle at tick {}", heard.tick);
        }
        // The comparison above is only worth something if the record it wiped
        // had something in it.
        let c = heard.sim.cues;
        let shots = c.loosed(WeaponClass::Bow) + c.loosed(WeaponClass::Crossbow);
        let hits = c.missile_hits(WeaponClass::Bow) + c.missile_hits(WeaponClass::Crossbow);
        let melee: u32 = crate::ALL_TROOPS.iter().map(|&t| c.melee_casualties(t)).sum();
        assert!(shots > 0 && hits > 0, "the battle must actually shoot and hit: {c:?}");
        assert!(melee > 0, "and come to blows: {c:?}");
    }

    /// A weakened figure shoots for less — the strength band, which until now
    /// nothing in the simulation read.
    #[test]
    fn a_weakened_archer_shoots_for_less() {
        let full = crate::missile::band_scaled(WeaponClass::Bow.stats().damage, 0);
        let hurt = crate::missile::band_scaled(WeaponClass::Bow.stats().damage, 3);
        assert_eq!(full, 50);
        assert_eq!(hurt, 25, "half at the bottom band");
    }
}

