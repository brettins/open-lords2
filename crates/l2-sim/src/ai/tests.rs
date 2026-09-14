#![allow(unused_imports)]
use super::*;
use super::world::*;
use super::handlers::*;
use crate::figure::{Figure, State};
use crate::troop::Troop;
use crate::unit::{chebyshev, pct_of, Units, MAX_UNITS, REFORM_INTERVAL};
use l2_net::Pcg32;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::{SIDE_A, SIDE_B};
    use crate::unit::CATEGORY_OF_TROOP;

    /// A two-unit field battle: one AI unit of `ai_troop`, one human unit of
    /// `human_troop`, twenty cells apart.
    struct Fixture {
        units: Units,
        figures: Vec<Figure>,
        positions: Vec<(u8, u8)>,
        field: AiField,
        ai: Ai,
        pub ai_unit: usize,
        pub human_unit: usize,
    }

    impl Fixture {
        fn new(ai_troop: Troop, human_troop: Troop, n: usize) -> Self {
            let mut units = Units::new();
            let mut figures = Vec::new();
            let mut positions = Vec::new();
            let ai_unit = units
                .create(1, false, SIDE_B, CATEGORY_OF_TROOP[ai_troop.index()])
                .unwrap();
            let human_unit = units
                .create(2, true, SIDE_A, CATEGORY_OF_TROOP[human_troop.index()])
                .unwrap();
            for i in 0..n {
                let mut f = Figure::new(ai_troop, SIDE_B, 8);
                f.unit = ai_unit as u16;
                f.owner = 1;
                figures.push(f);
                positions.push((40 + i as u8, 50));
            }
            for i in 0..n {
                let mut f = Figure::new(human_troop, SIDE_A, 8);
                f.unit = human_unit as u16;
                f.owner = 2;
                f.owner_is_human = true;
                figures.push(f);
                positions.push((40 + i as u8, 30));
            }
            let mut fx = Fixture {
                units,
                figures,
                positions,
                field: AiField::field((40, 20), (40, 60)),
                ai: Ai::new(0x5EED),
                ai_unit,
                human_unit,
            };
            fx.units.rebuild_from_figures(&mut fx.figures);
            fx.units.recentre(ai_unit, &fx.figures, &fx.positions);
            fx.units.recentre(human_unit, &fx.figures, &fx.positions);
            fx
        }

        /// Run one whole think cycle with a chosen strength advantage.
        ///
        /// The number is written on the last frame before the handler reads it,
        /// which is the only way to hold it steady: the recompute would
        /// otherwise overwrite it every 101 frames. `multiplayer` is set so
        /// the jitter cannot move it either.
        fn think_at(&mut self, advantage: i32) -> Action {
            self.ai.multiplayer = true;
            for _ in 0..(THINK_INTERVAL - 1) {
                self.frame();
            }
            self.ai.strength_advantage = advantage;
            self.frame();
            self.action()
        }

        fn think_at_n(&mut self, advantage: i32, n: usize) -> Vec<Action> {
            (0..n).map(|_| self.think_at(advantage)).collect()
        }

        fn frame(&mut self) -> Vec<usize> {
            self.units.rebuild_from_figures(&mut self.figures);
            update_all_units(
                &mut self.units,
                &mut self.figures,
                &self.positions,
                &self.field,
                &mut self.ai,
            )
        }

        fn frames(&mut self, n: usize) {
            for _ in 0..n {
                self.frame();
            }
        }

        fn action(&self) -> Action {
            self.ai.last_action[self.ai_unit]
        }
    }

    // --- the think gate ----------------------------------------------------

    /// A unit decides once every two hundred frames. Nothing else in the battle
    /// is that slow.
    #[test]
    fn a_unit_thinks_once_every_two_hundred_frames_and_not_before() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        fx.ai.multiplayer = true;
        for _ in 0..199 {
            fx.frame();
            assert_eq!(fx.action(), Action::NoThink, "thought early");
            assert_eq!(fx.units.get(fx.ai_unit).orders, 0);
        }
        fx.frame();
        assert_ne!(fx.action(), Action::NoThink, "should have thought on frame 200");
        assert_eq!(fx.units.get(fx.ai_unit).orders, 1, "one think, one script step");
        for _ in 0..199 {
            fx.frame();
            assert_eq!(fx.units.get(fx.ai_unit).orders, 1, "and not again until 400");
        }
        fx.frame();
        assert_eq!(fx.units.get(fx.ai_unit).orders, 2);
    }

    /// Thirteen of the seventeen handlers refuse to run while any figure of the
    /// unit is in melee. Once the AI's line makes contact, most of the AI stops
    /// manoeuvring.
    #[test]
    fn a_unit_in_melee_stops_thinking_entirely() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        fx.ai.multiplayer = true;
        fx.figures[0].state = State::Melee;
        for _ in 0..1_000 {
            fx.frame();
            assert_eq!(fx.action(), Action::NoThink);
        }
        assert_eq!(fx.units.get(fx.ai_unit).orders, 0, "five thinks' worth of frames, none taken");
        // And it resumes the moment the melee ends.
        fx.figures[0].state = State::Idle;
        fx.frames(200);
        assert!(fx.units.get(fx.ai_unit).orders > 0);
    }

/// The four exceptions, so "13 of 17" is worth pinning: the two
    /// siege engines and the ram keep thinking while engaged, and so does the
    /// siege defender's foot.
    #[test]
    fn four_handlers_do_not_gate_on_melee() {
        for (troop, side) in [
            (Troop::Catapults, SIDE_B),
            (Troop::SiegeTowers, SIDE_B),
            (Troop::BatteringRams, SIDE_B),
            (Troop::Pikemen, SIDE_A),
        ] {
            let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
            fx.ai.is_siege = true;
            fx.ai.multiplayer = true;
            let u = fx
                .units
                .create(3, false, side, CATEGORY_OF_TROOP[troop.index()])
                .unwrap();
            let mut f = Figure::new(troop, side, 4);
            f.unit = u as u16;
            f.owner = 3;
            f.state = State::Melee;
            fx.figures.push(f);
            fx.positions.push((30, 40));
            fx.frames(250);
            assert!(
                fx.units.get(u).orders > 0,
                "{troop:?} should keep thinking in melee"
            );
        }
    }

    /// `SiegeDefFoot` and `SiegeDefMelee` think twice as often as everybody
    /// else.
    #[test]
    fn the_two_fast_siege_defenders_think_every_hundred_frames() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.is_siege = true;
        fx.ai.multiplayer = true;
        let foot = fx.units.create(3, false, SIDE_A, 2).unwrap();
        let melee = fx.units.create(3, false, SIDE_A, 3).unwrap();
        for (i, u) in [foot, melee].into_iter().enumerate() {
            let mut f = Figure::new(Troop::Pikemen, SIDE_A, 4);
            f.unit = u as u16;
            f.owner = 3;
            fx.figures.push(f);
            fx.positions.push((30 + i as u8, 40));
        }
        fx.frames(100);
        assert_eq!(fx.units.get(foot).orders, 1);
        assert_eq!(fx.units.get(melee).orders, 1);
        // The 200-frame handlers have not moved at all yet.
        assert_eq!(fx.units.get(fx.ai_unit).orders, 0);
    }

    #[test]
    fn a_human_controlled_unit_is_never_given_an_order() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        fx.ai.multiplayer = true;
        fx.frames(2_000);
        assert_eq!(fx.units.get(fx.human_unit).orders, 0);
        assert_eq!(fx.ai.last_action[fx.human_unit], Action::NoThink);
        assert!(fx.units.get(fx.ai_unit).orders > 0, "but the AI's did");
    }

    // --- the one number ----------------------------------------------------

    #[test]
    fn the_strength_advantage_is_weighted_ai_men_over_weighted_human_men() {
        let mut fx = Fixture::new(Troop::Knights, Troop::Peasants, 4);
        fx.ai.multiplayer = true;
        fx.ai.update_strength_advantage(&fx.figures);
        // 4 figures x 8 men: knights weigh 4, peasants 1, so 128 v 32 = 400 %.
        assert_eq!(fx.ai.strength_advantage, 300);

        let mut even = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        even.ai.multiplayer = true;
        even.ai.update_strength_advantage(&even.figures);
        assert_eq!(even.ai.strength_advantage, 0, "identical armies are even");
    }

/// An army with no living enemy reads −100.
    #[test]
    fn an_unopposed_army_reads_minus_one_hundred() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        fx.ai.multiplayer = true;
        for f in fx.figures.iter_mut().filter(|f| f.owner_is_human) {
            f.take_hits(60_000);
        }
        fx.ai.update_strength_advantage(&fx.figures);
        assert_eq!(fx.ai.strength_advantage, -100);
    }

    /// The jitter is **−10 to +21**, not ±10 and not ±16. Asymmetric because
/// the original is `(rand & 0x1F) - 10`, and reproduced:
    /// with the threshold at 5 the half-point bias toward attacking is a real
    /// behaviour.
    #[test]
    fn the_jitter_spans_minus_ten_to_plus_twenty_one() {
        let mut ai = Ai::new(0x1234_5678);
        let figures: Vec<Figure> = Vec::new();
        let (mut low, mut high) = (i32::MAX, i32::MIN);
        let mut seen = [false; 32];
        for _ in 0..5_000 {
            ai.update_strength_advantage(&figures);
            // No figures at all: pct_of(0, 0) is 0, so the base is -100 and
            // everything after it is the jitter.
            let j = ai.strength_advantage + 100;
            low = low.min(j);
            high = high.max(j);
            seen[(j + 10) as usize] = true;
        }
        assert_eq!(low, -10);
        assert_eq!(high, 21);
        assert!(seen.iter().all(|s| *s), "every one of the 32 values should occur");
    }

    /// Determinism is the property lockstep depends on, and the jitter is the
    /// only place a battle draws a random number at all.
    #[test]
    fn the_same_seed_gives_the_same_jitter_and_a_different_seed_does_not() {
        let roll = |seed: u64| {
            let mut ai = Ai::new(seed);
            let figures: Vec<Figure> = Vec::new();
            (0..50)
                .map(|_| {
                    ai.update_strength_advantage(&figures);
                    ai.strength_advantage
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(roll(7), roll(7));
        assert_ne!(roll(7), roll(8));
    }

    /// The multiplayer flag drops the jitter entirely, which is the
    /// original's own answer to "this is a networked game".
    #[test]
    fn the_multiplayer_flag_removes_the_jitter_and_the_draw() {
        let mut ai = Ai::new(99);
        ai.multiplayer = true;
        let before = ai.rng.clone();
        let figures: Vec<Figure> = Vec::new();
        for _ in 0..20 {
            ai.update_strength_advantage(&figures);
            assert_eq!(ai.strength_advantage, -100, "no jitter at all");
        }
        assert_eq!(ai.rng, before, "and the generator was not advanced");
    }

    #[test]
    fn the_advantage_is_recomputed_every_hundred_and_first_frame() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        let start = fx.ai.rng.clone();
        for _ in 0..100 {
            fx.frame();
        }
        assert_eq!(fx.ai.rng, start, "not yet");
        fx.frame();
        assert_ne!(fx.ai.rng, start, "the hundred-and-first frame draws");
    }

    // --- the field handlers ------------------------------------------------

    /// Above the aggression threshold a foot unit marches on the enemy's
    /// deployment marker for ten thinks, and then charges — which drops every
    /// figure into free pursuit and *stops the unit being a formation*.
    #[test]
    fn an_aggressive_foot_unit_marches_for_ten_thinks_and_then_charges() {
        let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
        let actions = fx.think_at_n(50, 12); // comfortably above 5
        assert_eq!(actions[0], Action::DoNothing, "the first two thinks are silent");
        assert_eq!(actions[1], Action::DoNothing);
        assert!(
            matches!(actions[2], Action::ToEnemyEnd(_)),
            "think 2 starts the march: {:?}",
            actions[2]
        );
        assert!(matches!(actions[9], Action::ToEnemyEnd(_)), "still marching at think 9");
        assert_eq!(actions[10], Action::Charge, "think 10 charges");
        assert!(fx.units.get(fx.ai_unit).halted, "and the unit is halted");
        assert!(fx
            .figures
            .iter()
            .filter(|f| f.unit as usize == fx.ai_unit)
            .all(|f| f.state == State::Chasing));
    }

    /// Melee units march three thinks longer than foot units and are silent for
/// four. The two functions differ only in constants.
    #[test]
    fn a_melee_unit_marches_thirteen_thinks_rather_than_ten() {
        let mut fx = Fixture::new(Troop::Knights, Troop::Peasants, 4);
        let actions = fx.think_at_n(50, 15);
        assert_eq!(actions[3], Action::DoNothing, "silent through think 3");
        assert!(matches!(actions[4], Action::ToEnemyEnd(_)));
        assert!(matches!(actions[12], Action::ToEnemyEnd(_)), "still marching at 12");
        assert_eq!(actions[13], Action::Charge);
    }

    /// **The aggression threshold is 5, and it is the whole field AI.** The
    /// same unit in the same position attacks or does not, on one number.
    #[test]
    fn five_is_the_line_between_marching_on_the_enemy_and_falling_back() {
        let run = |advantage: i32| {
            let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
            fx.think_at_n(advantage, 6)
        };
        let bold = run(AGGRESSION_THRESHOLD + 1);
        let timid = run(AGGRESSION_THRESHOLD);
        assert!(
            bold.iter().any(|a| matches!(a, Action::ToEnemyEnd(_))),
            "at 6 it marches on the enemy: {bold:?}"
        );
        assert!(
            !timid.iter().any(|a| matches!(a, Action::ToEnemyEnd(_))),
            "at exactly 5 it does not: {timid:?}"
        );
        assert!(
            timid.iter().any(|a| matches!(a, Action::ToRallyWaypoint(_))),
            "it falls back on a rally waypoint instead: {timid:?}"
        );
    }

    /// The field handlers' rally waypoints differ, which
    /// `docs/battle-ai.md` §2.3's pseudocode does not say: foot units go to
    /// waypoint 2 and melee units to waypoint 1.
    #[test]
    fn foot_and_melee_units_rally_on_different_waypoints() {
        let waypoint = |troop: Troop| {
            let mut fx = Fixture::new(troop, Troop::Peasants, 4);
            for _ in 0..40 {
                if let Action::ToRallyWaypoint(k) = fx.think_at(0) {
                    return Some(k);
                }
            }
            None
        };
        assert_eq!(waypoint(Troop::Peasants), Some(2));
        assert_eq!(waypoint(Troop::Knights), Some(1));
    }

    /// The charge radius is tiny, and a unit of fewer than three figures is
    /// invisible to it: `Enemy_NearestUnit(unit, 8, 3)`.
    #[test]
    fn a_worn_down_enemy_unit_becomes_invisible_to_the_melee_ai() {
        let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
        // Put the human unit right on top of the AI unit.
        let human: Vec<usize> = fx
            .figures
            .iter()
            .enumerate()
            .filter(|(_, f)| f.owner == 2)
            .map(|(i, _)| i)
            .collect();
        for i in &human {
            fx.positions[*i] = (41, 50);
        }
        assert_eq!(fx.think_at(0), Action::Charge, "four figures at one cell is a target");

        let mut worn = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
        for i in &human {
            worn.positions[*i] = (41, 50);
        }
        // Two figures left of four: below the three-figure floor.
        let doomed: Vec<usize> = worn
            .figures
            .iter()
            .enumerate()
            .filter(|(_, f)| f.owner == 2)
            .map(|(i, _)| i)
            .take(2)
            .collect();
        for i in doomed {
            worn.figures[i].take_hits(60_000);
        }
        assert_ne!(worn.think_at(0), Action::Charge, "two figures is not a target");
    }

    /// The only inter-unit message in the game: a melee unit that is being hit
    /// backs off *and* publishes its attacker's position, and a missile unit
    /// answers by shifting two cells toward it.
    #[test]
    fn a_melee_unit_under_attack_calls_the_archers_onto_its_attacker() {
        // Peasants, so this is `UnitOrder_FieldFoot`. The choice matters: the
        // cautious branch takes a *second* decision after the withdrawal, and
        // its rally is gated on `withdrawals == 0` — which the withdrawal has
        // just made false. `UnitOrder_FieldMelee` allows two withdrawals, so
        // there the rally runs and overwrites the destination the step-away
        // wrote. Both are the original; only the constant differs.
        let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
        fx.ai.engagement_budget = 100; // stay out of the two escalation branches
        fx.ai.men_missile = 100;
        fx.ai.men_total = 100;
        fx.ai.multiplayer = true;

        // The grudge lasts 50 frames, so it has to be created inside the last
        // 50 of the 200 the unit waits.
        for _ in 0..(THINK_INTERVAL - 10) {
            fx.frame();
        }
        fx.figures[0].was_hit = true;
        fx.figures[0].hit_by = Some(4);
        fx.ai.strength_advantage = 0;
        for _ in 0..10 {
            fx.frame();
        }

        assert_eq!(fx.action(), Action::StepAwayFromUnit(fx.human_unit));
        assert!(fx.ai.rally_request, "and the request went out");
        assert_eq!((fx.ai.rally_x, fx.ai.rally_y), (41, 30));
        assert!(fx.units.get(fx.ai_unit).withdrawing);
        assert_eq!(fx.units.get(fx.ai_unit).withdrawals, 1);
    }

    /// `g_aiCommitCounter` is an escalation, not a hold: while it is non-zero
    /// **every** AI melee unit charges regardless of what is near it.
    #[test]
    fn the_commit_counter_makes_every_melee_unit_charge_regardless() {
        let mut fx = Fixture::new(Troop::Peasants, Troop::Peasants, 4);
        // Twenty cells apart, so nothing is within the eight-cell radius.
        assert_ne!(fx.think_at(0), Action::Charge, "nothing in reach");

        // Two, not one: an unengaged unit decays the counter *before* reading
        // it, so a counter of one is already spent by the time it is tested.
        fx.ai.commit_counter = 2;
        assert_eq!(fx.think_at(0), Action::Charge, "the counter overrides distance");
        assert_eq!(fx.ai.commit_counter, 1, "and it decays by one per unengaged think");
    }

    /// Running out of archers raises the counter by twenty — "we have run out
    /// of archers, so go in" — and being over the engagement budget by three.
    #[test]
    fn the_two_escalations_raise_the_counter_by_three_and_by_twenty() {
        let escalate = |budget: i32, missile: i32| {
            let mut fx = Fixture::new(Troop::Swordsmen, Troop::Peasants, 4);
            fx.ai.multiplayer = true;
            fx.ai.engagement_budget = budget;
            fx.ai.men_total = 800;
            fx.ai.men_missile = missile;
            for _ in 0..(THINK_INTERVAL - 10) {
                fx.frame();
            }
            fx.figures[0].was_hit = true;
            fx.figures[0].hit_by = Some(4);
            fx.ai.strength_advantage = 0;
            for _ in 0..10 {
                fx.frame();
            }
            fx.ai.commit_counter
        };
        assert_eq!(escalate(0, 800), 3, "over the engagement budget");
        assert_eq!(escalate(100, 0), 20, "no archers left");
    }

    /// An AI archer unit that thinks it is winning never backs away: the
    /// retreat is a second decision that exists only on the cautious branch.
    #[test]
    fn a_winning_archer_unit_never_backs_away_however_often_it_is_hit() {
        let hit_ten_times = |advantage: i32| {
            let mut fx = Fixture::new(Troop::Archers, Troop::Peasants, 4);
            fx.ai.multiplayer = true;
            for _ in 0..(THINK_INTERVAL - 20) {
                fx.frame();
            }
            for _ in 0..12 {
                fx.figures[0].was_hit = true;
                fx.figures[0].hit_by = Some(4);
                fx.frame();
            }
            assert!(fx.units.get(fx.ai_unit).times_hit > 10);
            fx.ai.strength_advantage = advantage;
            for _ in 0..8 {
                fx.frame();
            }
            fx.action()
        };
        assert_eq!(
            hit_ten_times(0),
            Action::StepAwayFromUnit(2),
            "the cautious archer withdraws"
        );
        assert_ne!(
            hit_ten_times(50),
            Action::StepAwayFromUnit(2),
            "the confident one does not"
        );
    }

    /// A missile unit looks over the whole eighty-cell field; a melee unit
    /// looks eight or nine cells.
    #[test]
    fn the_search_radius_is_the_whole_field_for_missiles_and_a_few_cells_for_melee() {
        let mut fx = Fixture::new(Troop::Archers, Troop::Peasants, 4);
        let world = World {
            units: &mut fx.units,
            figures: &mut fx.figures,
            positions: &fx.positions,
            field: &fx.field,
            ai: &mut fx.ai,
        };
        assert_eq!(world.nearest_enemy_unit(1, 80, 1), 2, "twenty cells is within eighty");
        assert_eq!(world.nearest_enemy_unit(1, 9, 3), 0, "and outside nine");
        assert_eq!(world.nearest_enemy_unit(1, 20, 1), 2, "exactly twenty is inside");
        assert_eq!(world.nearest_enemy_unit(1, 19, 1), 0);
    }

    /// `Order_HalfwayToUnit` does nothing until an axis is separated by eight,
    /// and then moves only the axes separated by six. This is why AI archers
    /// advance in stages and then stop.
    #[test]
    fn halfway_to_unit_does_nothing_below_eight_cells_of_separation() {
        let close = |gap: u8| {
            let mut fx = Fixture::new(Troop::Archers, Troop::Peasants, 1);
            fx.positions[0] = (40, 50);
            fx.positions[1] = (40, 50 - gap);
            fx.units.rebuild_from_figures(&mut fx.figures);
            fx.units.recentre(1, &fx.figures, &fx.positions);
            fx.units.recentre(2, &fx.figures, &fx.positions);
            fx.units.get_mut(1).target_x = 40;
            fx.units.get_mut(1).target_y = 50;
            let mut world = World {
                units: &mut fx.units,
                figures: &mut fx.figures,
                positions: &fx.positions,
                field: &fx.field,
                ai: &mut fx.ai,
            };
            world.halfway_to_unit(1, 2);
            (fx.units.get(1).target_x, fx.units.get(1).target_y)
        };
        assert_eq!(close(7), (40, 50), "seven cells: nothing happens at all");
        assert_eq!(close(8), (40, 46), "eight: halve it");
        assert_eq!(close(20), (40, 40));
    }

    // --- the siege handlers ------------------------------------------------

    /// **The one thing in `Lords2.exe` that ends a battle by giving up.**
    ///
    /// `UnitOrder_SiegeAttKnight` is the only writer of `g_battleWithdrawal`
    /// (`0x0056D5C8`) in the whole binary, and the clause was missing here —
    /// which made [`crate::End::Withdrawal`] unreachable in a played game and
    /// with it the whole withdrawal half of the campaign seam, including
    /// `Army_WithdrawCasualties`, which `l2-kingdom` had never implemented
    /// because nothing could reach it. `docs/decisions.md` C71.
    ///
    /// Three cases, and the third is the one the two conditions are for:
    /// `g_aiMenTotal <= g_aiMenKnight` is *"every AI man still standing is a
    /// knight"*, over `Battle_CountMenByType`'s census of **both** sides'
    /// non-human figures.
    #[test]
    fn an_all_knight_besieger_at_an_unbreached_wall_leaves_the_field() {
        let raised = |breach: i32, dismount: bool| {
            let mut fx = Fixture::new(Troop::Knights, Troop::Peasants, 4);
            fx.ai.is_siege = true;
            if dismount {
                // One man of the besieging force who is not a knight,
                // the whole of what `total <= knights` asks about.
                fx.figures[0].troop = Troop::Peasants;
            }
            fx.ai.count_men(&fx.figures);
            assert!(fx.ai.men_total > 0, "the census sees the AI's own men");
            fx.ai.breach_score = breach;
            assert_eq!(fx.ai.withdrawal, None, "nothing before the unit has thought");
            fx.think_at(0);
            fx.ai.withdrawal
        };
        assert_eq!(raised(0, false), Some(SIDE_B), "all knights, nothing breached");
        assert_eq!(raised(4, false), None, "a breach is a reason to stay");
        assert_eq!(raised(0, true), None, "and so is one man who can climb");
    }

    /// The three defender stubs at categories 5, 6 and 7 are the catapult, the
/// siege tower and the ram — the three troop types
    /// `g_raiseOrderSiege` says a garrison never has.
    #[test]
    fn a_garrison_has_no_handler_for_the_three_troops_it_never_raises() {
        for category in [5usize, 6, 7] {
            assert_eq!(
                TABLE_SIEGE_DEF[category].name, "UnitOrder_None",
                "defender category {category} should be the empty handler"
            );
        }
        // And the mirror: oil is a defender's weapon, so the attacker's slot is
        // empty too.
        assert_eq!(TABLE_SIEGE_ATT[8].name, "UnitOrder_None");

        let all = || {
            TABLE_FIELD
                .iter()
                .chain(TABLE_SIEGE_ATT.iter())
                .chain(TABLE_SIEGE_DEF.iter())
        };
        assert_eq!(all().count(), 25, "twenty-five slots");
        assert_eq!(
            all().filter(|s| s.name == "UnitOrder_None").count(),
            7,
            "UnitOrder_None fills seven of them"
        );
        let mut distinct: Vec<u32> = all().map(|s| s.addr).collect();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), 18, "eighteen distinct functions");
        assert_eq!(distinct.len() - 1, 17, "seventeen of which do something");
        // The last real handler is UnitOrder_SiegeDefOil, not the helper at
        // 0x0048ECA9 that `battle.md` §11's range overshoots to.
        assert_eq!(*distinct.last().unwrap(), 0x0048_E8B8);
    }

    /// A garrison sallies out only above 260, which is close to never — and
    /// below it the defenders sit on the castle instead.
    #[test]
    fn a_garrison_only_sorties_when_it_believes_it_is_three_and_a_half_times_stronger() {
        let sortied = |advantage: i32| {
            let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
            fx.ai.is_siege = true;
            fx.ai.multiplayer = true;
            let def = fx.units.create(3, false, SIDE_A, 4).unwrap(); // knight
            let mut f = Figure::new(Troop::Knights, SIDE_A, 4);
            f.unit = def as u16;
            f.owner = 3;
            fx.figures.push(f);
            fx.positions.push((40, 40));
            fx.think_at(advantage);
            (fx.ai.last_action[def], fx.ai.drawbridge_down)
        };
        let (held, bridge) = sortied(SORTIE_THRESHOLD);
        assert_ne!(held, Action::Charge, "260 is not above 260");
        assert!(!bridge, "and the drawbridge stayed up");
        assert_eq!(sortied(SORTIE_THRESHOLD + 1), (Action::Charge, true));
    }

    /// Category 9's handler does nothing at all except count, so that unit
    /// holds the wall slot it deployed on for the whole battle. A test that
    /// only checked "it issued no order" would pass on a broken dispatch too,
    /// so check the counter moved.
    #[test]
    fn the_first_wall_missile_unit_holds_its_slot_for_the_entire_battle() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.is_siege = true;
        fx.ai.multiplayer = true;
        let u = fx.units.create(3, false, SIDE_A, 9).unwrap();
        let mut f = Figure::new(Troop::Archers, SIDE_A, 4);
        f.unit = u as u16;
        f.owner = 3;
        fx.figures.push(f);
        fx.positions.push((30, 44));
        fx.frames(2_000);
        assert_eq!(fx.units.get(u).orders, 10, "it thought ten times");
        assert_eq!(fx.ai.last_action[u], Action::DoNothing, "and never ordered anything");
        assert_eq!(
            (fx.units.get(u).target_x, fx.units.get(u).target_y),
            (30, 44),
            "its destination is still where it deployed"
        );
    }

    /// In an open field, categories 5 to 8 have no handler at all: the table is
    /// five entries and the bound is `< 5`. An AI catapult in a field battle is
    /// never repositioned.
    #[test]
    fn a_catapult_in_a_field_battle_is_never_given_an_order() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.multiplayer = true;
        let u = fx.units.create(3, false, SIDE_B, 5).unwrap();
        let mut f = Figure::new(Troop::Catapults, SIDE_B, 4);
        f.unit = u as u16;
        f.owner = 3;
        fx.figures.push(f);
        fx.positions.push((30, 55));
        fx.frames(2_000);
        assert_eq!(fx.units.get(u).orders, 0, "it never even thinks");
        assert_eq!(fx.ai.last_action[u], Action::NoThink);
        // The siege table, by contrast, does have a handler for it.
        fx.ai.is_siege = true;
        fx.frames(200);
        assert!(fx.units.get(u).orders > 0);
    }

    /// The catapult searches exactly its own firing range: `Siege_FindCellSurface4`
    /// is called with radius 20, and `g_missileStats` gives the catapult 160
    /// eighths of a cell = 20. Two unrelated constants agreeing.
    #[test]
    fn the_catapult_looks_for_a_wall_within_exactly_its_firing_range() {
        let found = |wall_distance: i32| {
            let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
            fx.ai.is_siege = true;
            fx.ai.multiplayer = true;
            fx.field.surface = vec![0u8; CELLS];
            let (cx, cy) = (30usize, 55usize);
            fx.field.surface[(cy as i32 - wall_distance) as usize * DIM + cx] = 4;
            let u = fx.units.create(3, false, SIDE_B, 5).unwrap();
            let mut f = Figure::new(Troop::Catapults, SIDE_B, 4);
            f.unit = u as u16;
            f.owner = 3;
            fx.figures.push(f);
            fx.positions.push((cx as u8, cy as u8));
            // Think 31 is the first that searches.
            for _ in 0..32 {
                fx.frames(200);
            }
            fx.ai.last_action[u]
        };
        assert_eq!(found(20), Action::ToCell, "twenty cells is in range");
        assert_eq!(found(21), Action::ToCastleApproach(5), "twenty-one is not");
    }

    /// Two handlers return before the `orders` increment on their wall-found
    /// path, so a unit that is doing its job stops advancing its script.
    #[test]
    fn a_catapult_that_has_found_its_wall_stops_advancing_its_script() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.is_siege = true;
        fx.ai.multiplayer = true;
        fx.field.surface = vec![0u8; CELLS];
        fx.field.surface[50 * DIM + 30] = 4;
        let u = fx.units.create(3, false, SIDE_B, 5).unwrap();
        let mut f = Figure::new(Troop::Catapults, SIDE_B, 4);
        f.unit = u as u16;
        f.owner = 3;
        fx.figures.push(f);
        fx.positions.push((30, 55));
        for _ in 0..32 {
            fx.frames(200);
        }
        let pinned = fx.units.get(u).orders;
        assert_eq!(fx.ai.last_action[u], Action::ToCell);
        for _ in 0..10 {
            fx.frames(200);
        }
        assert_eq!(fx.units.get(u).orders, pinned, "the script counter is stuck");
    }

    /// `SiegeAttFoot` abandons its whole approach script and charges once the
    /// advantage reaches 151.
    #[test]
    fn a_siege_attacker_on_foot_charges_outright_above_a_hundred_and_fifty_one() {
        let outcome = |advantage: i32| {
            let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
            fx.ai.is_siege = true;
            fx.ai.multiplayer = true;
            fx.ai.approach_score = 10;
            fx.ai.breach_score = 5;
            let u = fx.units.create(3, false, SIDE_B, 2).unwrap();
            let mut f = Figure::new(Troop::Pikemen, SIDE_B, 4);
            f.unit = u as u16;
            f.owner = 3;
            fx.figures.push(f);
            fx.positions.push((30, 55));
            for _ in 0..3 {
                fx.think_at(advantage);
            }
            fx.ai.last_action[u]
        };
        assert_ne!(outcome(SIEGE_CHARGE_ADVANTAGE - 1), Action::Charge);
        assert_eq!(outcome(SIEGE_CHARGE_ADVANTAGE), Action::Charge);
    }

    /// The castle layout flag makes two handlers jump `orders` to 100 outright,
    /// skipping the rest of the approach script.
    #[test]
    fn the_castle_layout_flag_jumps_the_approach_script_to_a_hundred() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 2);
        fx.ai.is_siege = true;
        fx.ai.multiplayer = true;
        fx.field.castle_layout_flag = true;
        let u = fx.units.create(3, false, SIDE_B, 2).unwrap();
        let mut f = Figure::new(Troop::Pikemen, SIDE_B, 4);
        f.unit = u as u16;
        f.owner = 3;
        fx.figures.push(f);
        fx.positions.push((30, 55));
        // Think 18 is the first in the 18..25 band that carries the jump.
        for _ in 0..19 {
            fx.frames(200);
        }
        assert_eq!(fx.units.get(u).orders, 101, "jumped to 100, then incremented");
    }

    // --- reforming ---------------------------------------------------------

    /// `re targ` counts 500 down and then reforms the unit's **own figures**;
    /// it does not look for an enemy. And it runs for human-controlled units
/// too, so the exemption for small human units exists at all.
    #[test]
    fn the_five_hundred_frame_countdown_reforms_and_runs_for_human_units_too() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 4);
        fx.ai.multiplayer = true;
        let mut who = Vec::new();
        for _ in 0..500 {
            who = fx.frame();
        }
        assert_eq!(who, vec![fx.ai_unit, fx.human_unit], "both sides reform");
        assert_eq!(fx.units.get(fx.human_unit).reform, REFORM_INTERVAL);
    }

    /// Two exemptions, and they are different in kind. A human unit of fewer
    /// than four figures is spared the tidy-up; a unit that has been ordered to
    /// charge is spared it because *it is no longer a formation*.
    #[test]
    fn a_small_human_unit_is_exempt_and_so_is_one_that_has_charged() {
        let mut fx = Fixture::new(Troop::Swordsmen, Troop::Swordsmen, 3);
        fx.ai.multiplayer = true;
        let mut who = Vec::new();
        for _ in 0..500 {
            who = fx.frame();
        }
        assert!(
            !who.contains(&fx.human_unit),
            "three figures and human-controlled: exempt"
        );
        assert!(who.contains(&fx.ai_unit), "the AI's unit of three is not");

// The charge exemption, checked on the flag the charge sets
        // by waiting fourteen thinks for one.
        let mut charged = *fx.units.get(fx.ai_unit);
        assert!(needs_reform(&charged));
        charged.halted = true;
        assert!(!needs_reform(&charged));
        // And an emptied unit is never reformed either.
        charged.halted = false;
        charged.figures = 0;
        assert!(!needs_reform(&charged));
    }

    // --- determinism -------------------------------------------------------

    /// The property lockstep depends on, at the level of the whole AI: two
    /// battles from the same seed must stay bit-identical, jitter and all.
    #[test]
    fn two_ai_battles_from_the_same_seed_stay_identical() {
        let build = || Fixture::new(Troop::Swordsmen, Troop::Archers, 6);
        let (mut a, mut b) = (build(), build());
        for _ in 0..3_000 {
            a.frame();
            b.frame();
            assert_eq!(a.units, b.units, "units diverged");
            assert_eq!(a.ai, b.ai, "ai state diverged");
            assert_eq!(a.figures, b.figures, "figures diverged");
        }
        // And the jitter really was in play,
        assert_ne!(a.ai.rng, Ai::new(0x5EED).rng, "the generator never advanced");
    }
}

