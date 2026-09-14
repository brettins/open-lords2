//! [`super`] - the frame layout, checked against the shipped art.

    use super::*;

    const SPRITE_TROOPS: [Troop; 6] = [
        Troop::Peasants,
        Troop::Crossbowmen,
        Troop::Macemen,
        Troop::Swordsmen,
        Troop::Pikemen,
        Troop::Archers,
    ];

    #[test]
    fn the_walk_offset_trails_the_cell_being_entered() {
        // Walking north: drawn below the destination, closing on it.
        assert_eq!(walk_offset(0, 1), (0, 30));
        assert_eq!(walk_offset(0, 16), (0, 0));
        // Walking east: drawn to the west of it.
        assert_eq!(walk_offset(2, 1), (-30, 0));
        // Every facing's offset points opposite its movement.
        for f in 0..8u8 {
            let (dx, dy) = FACING_DELTA[f as usize];
            let (ox, oy) = walk_offset(f, 2);
            assert_eq!(ox.signum(), -dx.signum(), "facing {f} x");
            assert_eq!(oy.signum(), -dy.signum(), "facing {f} y");
        }
        assert_eq!(walk_offset(3, 0), (0, 0), "not walking means no offset");
    }

    #[test]
    fn every_animation_frame_lands_inside_its_sheet() {
        for troop in SPRITE_TROOPS {
            let total = FACINGS * poses_per_facing(troop) as usize + 18;
            for facing in 0..8u8 {
                for phase in 0..=120u8 {
                    // `Shooting` is left out on purpose: `Anim_DrawBowA2`'s
                    // stride is 13 whatever the troop, so a pikeman drawn by it
                    // lands past his own sheet in the binary too.
                    for anim in [Anim::Idle, Anim::Walking, Anim::Attacking, Anim::Dying] {
                        let f = frame(troop, anim, facing, phase);
                        assert!(
                            f < total,
                            "{troop:?} {anim:?} facing {facing} phase {phase} -> {f} of {total}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn walking_and_attacking_stay_inside_their_own_pose_bands() {
        for troop in SPRITE_TROOPS {
            let stride = poses_per_facing(troop) as usize;
            let bow = matches!(troop, Troop::Archers | Troop::Crossbowmen);
            for facing in 0..8usize {
                for phase in 0..=120u8 {
                    // §14.5: walking is 0…5, striking is 6…, and they used to
                    // be asserted the other way round here.
                    let w = frame(troop, Anim::Walking, facing as u8, phase) - facing * stride;
                    assert!(w < 6, "{troop:?} walk pose {w}");
                    let a = frame(troop, Anim::Attacking, facing as u8, phase) - facing * stride;
                    assert!((6..stride).contains(&a), "{troop:?} strike pose {a}");
                    let swing = Pose { swing: phase as u16, ..Pose::default() };
                    let s =
                        frame(troop, Anim::Shooting, facing as u8, swing) - facing * drawbow::STRIDE;
                    match bow {
                        true => assert!((10..13).contains(&s), "{troop:?} bow pose {s}"),
                        // No curve, so the base and nothing else.
                        false => assert_eq!(s, drawbow::BASE, "{troop:?} has no draw curve"),
                    }
                }
            }
        }
    }

    /// **Ablation** — the swap §14.5 corrects. Drawing the two bands the old
    /// way round puts a striking pose where the walk belongs, and for a pikeman
    /// (N = 8, strike band 6…7) it walks clean out of the bow band's room too.
    #[test]
    fn the_old_swapped_bands_would_fail_the_band_test() {
        let stride = poses_per_facing(Troop::Archers) as usize;
        let old_walk = 6 + STRIKE_LIGHT[0] as usize;
        assert!(old_walk >= 6, "the old walk sat in the strike band");
        assert_eq!(frame(Troop::Archers, Anim::Walking, 0, 0), 0);
        // And the bow band is only reachable at N = 13.
        assert!(stride >= 13);
        assert!((poses_per_facing(Troop::Pikemen) as usize) < 13);
    }

    /// `Anim_StandA2` (`0x004872AE`): eight neighbours stand in six postures.
    ///
    /// **Ablation** - the old `pose N-1` gave every man of a troop the same
    /// frame, so a standing rank was eight copies of one picture.
    #[test]
    fn a_standing_rank_stands_in_six_postures() {
        let seen: std::collections::HashSet<usize> = (0..16)
            .map(|i| frame(Troop::Swordsmen, Anim::Idle, 0, Pose { index: i, ..Pose::default() }))
            .collect();
        assert_eq!(seen, (0..6).collect());
        assert_eq!(stand_pose(6), 1, "6 folds back to 1");
        assert_eq!(stand_pose(7), 2);
    }

    /// `Anim_StrikeA2` (`0x00486249`): only `role == 1` swings.
    ///
    /// **Ablation** - running the cycle for both men of a pair, which is what
    /// we did, had two swordsmen hitting each other on the same tick forever.
    #[test]
    fn the_defender_of_a_pair_stands_while_the_attacker_swings() {
        for (troop, stand) in
            [(Troop::Swordsmen, 11), (Troop::Archers, 9), (Troop::Pikemen, 7), (Troop::Peasants, 9)]
        {
            let d = Pose { phase: 16, defending: true, ..Pose::default() };
            assert_eq!(frame(troop, Anim::Attacking, 0, d), stand, "{troop:?} defends");
            let a = frame(troop, Anim::Attacking, 0, 16u8);
            assert!(a >= STRIKE_BASE as usize, "{troop:?} attacks: {a}");
        }
    }

    #[test]
    fn dying_uses_four_half_facings_of_three_frames() {

        let troop = Troop::Swordsmen;
        let base = FACINGS * poses_per_facing(troop) as usize + 6;
        let mut seen = std::collections::HashSet::new();
        for facing in 0..8u8 {
            for phase in 0..96u8 {
                seen.insert(frame(troop, Anim::Dying, facing, phase));
            }
        }
        assert_eq!(seen.len(), 12, "4 half-facings x 3 frames");
        assert_eq!(*seen.iter().min().unwrap(), base);
        assert_eq!(*seen.iter().max().unwrap(), base + 11);
        // Facings that share a half-facing share their death frames.
        assert_eq!(
            frame(troop, Anim::Dying, 2, 0),
            frame(troop, Anim::Dying, 3, 0),
            "2 and 3 are the same half-facing"
        );
    }

    /// **Three knight arms, not one.** `Anim_StrikeA2` (`00480000.c:2565`) is
    /// the only handler that reads `DAT_004D9C30`; `Anim_WalkA2` (`2896`) and
    /// `Anim_StandA2` (`2873`) set the rider frame to the bare facing and put
    /// the cadence on the horse.
    ///
    /// **Ablation**: putting walk and stand back on the strike formula gives a
    /// riding knight the swing frames and breaks the `< 8` bound below.
    #[test]
    fn a_knight_rides_on_the_bare_facing_and_swings_from_the_table() {
        for facing in 0..8u8 {
            for phase in 0..=120u8 {
                for anim in [Anim::Walking, Anim::Idle] {
                    let f = frame(Troop::Knights, anim, facing, phase);
                    assert_eq!(f, facing as usize, "the rider is the bare facing");
                }
                let f = frame(Troop::Knights, Anim::Attacking, facing, phase);
                assert!(f >= 8, "facing {facing} fell through to the rider-only frames");
                assert!(f < 56, "facing {facing} phase {phase} -> {f}, past the 56 real frames");
            }
        }
    }

    /// The horse carries the walk's six-pose cadence and stands still under a
    /// knight who is not walking — `horseFrame` in `00480000.c:2762` / `2896`.
    #[test]
    fn the_horse_walks_and_the_standing_horse_does_not() {
        for facing in 0..8u8 {
            let base = facing as usize * HORSE_POSES as usize;
            let mut seen = std::collections::HashSet::new();
            for phase in 0..24u8 {
                let f = horse_frame(facing, Anim::Walking, phase);
                assert!((base..base + HORSE_POSES as usize).contains(&f));
                seen.insert(f);
                assert_eq!(horse_frame(facing, Anim::Idle, phase), base, "a still horse");
                assert_eq!(horse_frame(facing, Anim::Attacking, phase), base);
            }
            assert_eq!(seen.len(), HORSE_POSES as usize, "all six walk poses reached");
        }
    }

    #[test]
    fn sprite_file_names_follow_the_asset_table() {
        assert_eq!(sprite_file(Colour::Red, Troop::Swordsmen).unwrap(), "A2r_swor.pl8");
        assert_eq!(sprite_file(Colour::Blue, Troop::Peasants).unwrap(), "A2b_psnt.pl8");
        assert_eq!(sprite_file(Colour::Black, Troop::Knights).unwrap(), "A2k_knig.pl8");
        assert!(sprite_file(Colour::Red, Troop::Catapults).is_none());
    }
