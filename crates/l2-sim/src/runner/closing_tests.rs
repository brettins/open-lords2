use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Move a fighter to a cell, carrying its occupancy with it.
    fn place(r: &mut BattleRunner, i: usize, x: u8, y: u8) {
        let old = r.fighters[i].y as usize * DIM + r.fighters[i].x as usize;
        if r.occupant[old] == Some(i as u16) {
            r.occupant[old] = None;
        }
        let f = &mut r.fighters[i];
        f.x = x;
        f.y = y;
        f.target = (x, y);
        f.path.clear();
        f.progress = Progress::default();
        r.occupant[y as usize * DIM + x as usize] = Some(i as u16);
    }

    fn cells_apart(r: &BattleRunner, a: usize, b: usize) -> i32 {
        let (fa, fb) = (&r.fighters[a], &r.fighters[b]);
        (fa.x as i32 - fb.x as i32).abs().max((fa.y as i32 - fb.y as i32).abs())
    }

    /// One `troop` figure at (20, 40) and one peasant at (40, 40) — twenty
    /// cells, outside any bow — each a unit of its own.
    fn closing_line(troop: Troop, ai_shooter: bool) -> (BattleRunner, usize, usize) {
        let mut r = BattleRunner::deploy_armies(
            blank_field(),
            DEFAULT_SEED,
            Army { troops: &[(Troop::Peasants, 1)], owner: 2, human: true },
            Army { troops: &[(troop, 1)], owner: 1, human: !ai_shooter },
        );
        let s = r.fighters.iter().position(|f| f.troop == troop).expect("the shooter");
        let t = r.fighters.iter().position(|f| f.troop == Troop::Peasants).expect("the peasant");
        place(&mut r, s, 20, 40);
        place(&mut r, t, 40, 40);
        r.settle();
        (r, s, t)
    }

    /// **State 17, end to end: sent onto an enemy, a shooter walks until its
    /// target is inside its range less two cells, and looses its own shot.**
    ///
    /// The two distances are `g_missileStats`' 120 and 64 eighths less
    /// `BattleMan_StateCloseToAttack`'s `+ 2` (`00480000.c:2024`), and no
    /// expression here computes them. Ablations, each red here alone: read the
    /// slot's occupant again in [`BattleRunner::send_figure`] and the first
    /// assertion fails; drop the `+ 2` and it looses from 15 and 8; delete
    /// `self.sim.cues.closing_loose(class)` and `first` is `None`.
    #[test]
    fn a_shooter_sent_onto_an_enemy_closes_to_its_range_less_two_and_looses_its_own_shot() {
        for (troop, class, from) in
            [(Troop::Archers, WeaponClass::Bow, 13), (Troop::Crossbowmen, WeaponClass::Crossbow, 6)]
        {
            let (mut r, s, t) = closing_line(troop, false);
            let (ssim, tsim) = (r.fighters[s].sim, r.fighters[t].sim);
            let unit = r.unit_of(s);
            r.order_unit(unit, 40, 40);
            assert_eq!(
                r.sim.figures[ssim].state,
                State::Shooting,
                "{troop:?}: an order onto an enemy is state 17"
            );
            assert_eq!(
                r.sim.figures[ssim].target,
                Some(tsim),
                "{troop:?}: aimed at the man on the unit's destination"
            );

            let mut first = None;
            for _ in 0..2_000 {
                let was = r.sim.cues;
                r.step();
                if r.sim.cues.closing_loosed(class) != was.closing_loosed(class) {
                    first = Some(cells_apart(&r, s, t));
                    break;
                }
            }
            assert_eq!(first, Some(from), "{troop:?} loosed from the wrong distance");
            assert_eq!(r.sim.cues.loosed(class), 0, "{troop:?}: BattleMan_FireMissile never ran");

            // `:2049-2056`, what the loose writes besides the missile.
            let f = &r.sim.figures[ssim];
            assert!(f.straffed && !f.engaged, "{troop:?}: movStraff set, engaged cleared");
            let u = r.units.get(unit);
            assert_eq!(u.firing, crate::unit::FIRING, "{troop:?}: the unit's firing");
            assert_eq!(
                (u.target_x, u.target_y),
                (r.fighters[s].x as i16, r.fighters[s].y as i16),
                "{troop:?}: the unit's destination moves to where it shot from"
            );

            for _ in 0..3_000 {
                r.step();
                if r.sim.figures[tsim].men < 4 {
                    break;
                }
            }
            assert!(r.sim.figures[tsim].men < 4, "{troop:?}: nothing it loosed ever landed");
            assert!(r.sim.cues.missile_hits(class) > 0);
            assert_eq!(r.sim.cues.loosed(class), 0, "{troop:?}: every shot was state 17's");
        }
    }

    /// **Every bow of a unit sent onto an enemy takes that one man.**
    ///
    /// `DAT_005681FC` is cached off the *unit's* destination by
    /// `Formation_RectIsClear` (`00480000.c:3963`). A one-figure unit's first
    /// searched slot **is** the destination, so the test above passes either
    /// way; four archers get four slots and only one of them is his cell.
    /// Ablation: read the slot's occupant in [`BattleRunner::send_figure`] and
    /// three of the four stay out of state 17.
    #[test]
    fn every_bow_of_a_unit_sent_onto_an_enemy_closes_on_the_same_man() {
        let mut r = BattleRunner::deploy_armies(
            blank_field(),
            DEFAULT_SEED,
            Army { troops: &[(Troop::Peasants, 1)], owner: 2, human: true },
            Army { troops: &[(Troop::Archers, 4)], owner: 1, human: true },
        );
        let archers: Vec<usize> =
            (0..r.fighters.len()).filter(|&i| r.fighters[i].troop == Troop::Archers).collect();
        assert_eq!(archers.len(), 4);
        let unit = r.unit_of(archers[0]);
        assert!(archers.iter().all(|&a| r.unit_of(a) == unit), "the four are one unit");
        let t = r.fighters.iter().position(|f| f.troop == Troop::Peasants).expect("the peasant");
        place(&mut r, t, 40, 40);
        for (k, &a) in archers.iter().enumerate() {
            place(&mut r, a, 20, 36 + 2 * k as u8);
        }
        r.settle();
        r.order_unit(unit, 40, 40);
        let tsim = r.fighters[t].sim;
        let got: Vec<(usize, State, Option<usize>)> = archers
            .iter()
            .map(|&a| {
                let f = &r.sim.figures[r.fighters[a].sim];
                (a, f.state, f.target)
            })
            .collect();
        let want: Vec<(usize, State, Option<usize>)> =
            archers.iter().map(|&a| (a, State::Shooting, Some(tsim))).collect();
        assert_eq!(got, want, "every archer of the unit, in state 17 on the one man");
    }

    /// **After its first shot, a closing archer holds its ground** — until the
    /// unit is ordered again, the one thing that clears `movStraff`.
    ///
    /// Ablation: make `:2024`'s arm ignore `straffed` and the archer walks
    /// after the peasant, so the first assertion goes red; delete
    /// `clear_straff` from [`BattleRunner::order_unit`] and the last one does.
    #[test]
    fn a_closing_archer_that_has_shot_holds_its_ground_until_it_is_ordered_again() {
        let (mut r, s, t) = closing_line(Troop::Archers, false);
        let ssim = r.fighters[s].sim;
        let unit = r.unit_of(s);
        r.order_unit(unit, 40, 40);
        for _ in 0..2_000 {
            let was = r.sim.cues.closing_loosed(WeaponClass::Bow);
            r.step();
            if r.sim.cues.closing_loosed(WeaponClass::Bow) != was {
                break;
            }
        }
        assert!(r.sim.figures[ssim].straffed, "it shot");
        let stood = r.fighters[s].x;

        place(&mut r, t, stood + 20, 40);
        r.run(400);
        assert_eq!(r.fighters[s].x, stood, "a straffed archer does not follow");
        assert!(r.sim.figures[ssim].engaged, "and it is out of range, which is engaged");
        assert_eq!(r.sim.figures[ssim].state, State::Shooting);

        r.order_unit(unit, stood + 20, 40);
        assert!(!r.sim.figures[ssim].straffed, "BattleUnit_Order clears movStraff");
        r.run(400);
        assert!(r.fighters[s].x > stood, "and ordered again, it closes");
    }

    /// **A shooting AI unit's reform is held at fifty** while its closing
    /// figure is not engaged, and not otherwise — `00480000.c:1995-1998`.
    /// Ablation: delete the `u.reform = CLOSING_REFORM` write.
    #[test]
    fn a_shooting_ai_unit_has_its_reform_held_at_fifty() {
        let (mut r, s, t) = closing_line(Troop::Archers, true);
        let (ssim, tsim) = (r.fighters[s].sim, r.fighters[t].sim);
        let unit = r.unit_of(s);
        r.sim.figures[ssim].state = State::Shooting;
        r.sim.figures[ssim].target = Some(tsim);

        r.units.get_mut(unit).reform = 400;
        assert!(r.close_to_attack_tick(s), "twenty cells out, it walks");
        assert_eq!(r.units.get(unit).reform, 50, "not engaged when it looked");
        assert!(r.sim.figures[ssim].engaged, "and walking out of range engages it");

        r.units.get_mut(unit).reform = 400;
        r.close_to_attack_tick(s);
        assert_eq!(r.units.get(unit).reform, 400, "an engaged figure does not hold it");

        r.sim.figures[ssim].engaged = false;
        r.units.get_mut(unit).human = true;
        r.close_to_attack_tick(s);
        assert_eq!(r.units.get(unit).reform, 400, "nor does a human's");
    }

    /// **A shooter keeps aiming at a body until its slot is freed** — 80
    /// frames in `BattleMan_StateDead`, and `owner == 0` (`00480000.c:1999`) is
    /// the only thing state 17 lets go on. It shoots at the body meanwhile.
    ///
    /// Ablation: test `is_alive` instead of [`BattleRunner::corpse_gone`] in
    /// `closing_target` and it lets go on the first frame.
    #[test]
    fn a_closing_shooter_keeps_aiming_at_a_body_until_its_slot_is_freed() {
        let (mut r, s, t) = closing_line(Troop::Archers, false);
        place(&mut r, t, 30, 40);
        let (ssim, tsim) = (r.fighters[s].sim, r.fighters[t].sim);
        r.sim.figures[ssim].state = State::Shooting;
        r.sim.figures[ssim].target = Some(tsim);
        r.sim.figures[tsim].men = 0;
        r.sim.figures[tsim].state = State::Dead;

        let mut held = 0u32;
        for _ in 0..200 {
            r.step();
            if r.sim.figures[ssim].state != State::Shooting {
                break;
            }
            held += 1;
        }
        assert_eq!(held, CORPSE_FRAMES as u32 + 1, "the body's slot, then one frame to read it");
        assert_eq!(r.sim.figures[ssim].state, State::Idle, "freed: back to state 5");
        assert!(r.sim.cues.closing_loosed(WeaponClass::Bow) > 0, "and it shot at the body");
    }

    /// **`Missile_Step` tells a shooter its shot ran out of range** —
    /// `00490000.c:1263`, `g_battleMen[shooter].engaged = 1` beside the free.
    ///
    /// Ablation: delete `mark_shooter_engaged` from the spent arm.
    #[test]
    fn a_shot_that_runs_out_of_range_marks_its_shooter_engaged() {
        let (mut r, s, t) = closing_line(Troop::Archers, false);
        place(&mut r, t, 30, 40);
        let ssim = r.fighters[s].sim;
        // No order, so the archer stands and `BattleMan_FireMissile` acquires.
        for _ in 0..600 {
            r.step();
            if r.missiles.live() > 0 {
                break;
            }
        }
        assert_eq!(r.missiles.live(), 1, "one arrow in the air");
        r.sim.figures[ssim].engaged = false;
        // Its target leaves, so the arrow flies on to the end of its range.
        place(&mut r, t, 78, 40);
        r.sim.figures[r.fighters[t].sim].state = State::Dead;
        r.sim.figures[r.fighters[t].sim].men = 0;
        for _ in 0..400 {
            if r.missiles.live() == 0 {
                break;
            }
            r.step();
        }
        assert_eq!(r.missiles.live(), 0, "the arrow expired");
        assert!(r.sim.figures[ssim].engaged, "and its shooter was told");
    }

    /// **C166's proof, over state 17.** The closing battle with its record
    /// wiped every tick is the same battle, and the record it wiped had state
    /// 17's loose in it.
    #[test]
    fn a_closing_battle_whose_cues_are_wiped_every_tick_is_the_same_battle() {
        let build = || {
            let (mut r, s, _) = closing_line(Troop::Crossbowmen, false);
            let unit = r.unit_of(s);
            r.order_unit(unit, 40, 40);
            r
        };
        let (mut heard, mut wiped) = (build(), build());
        for _ in 0..1_500u32 {
            wiped.sim.cues = crate::cue::Cues::default();
            heard.step();
            wiped.step();
            let mut same = wiped.clone();
            same.sim.cues = heard.sim.cues;
            assert_eq!(heard, same, "the cues changed the battle at tick {}", heard.tick);
        }
        assert!(heard.sim.cues.closing_loosed(WeaponClass::Crossbow) > 0, "state 17 loosed");
    }
}
