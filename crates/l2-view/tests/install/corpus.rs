#![allow(unused_imports)]
use super::*;
use super::oracle::*;
use super::render::*;
use std::{fs, path::{Path, PathBuf}};
use l2_sim::runner::{self as battle, BattleRunner};
use l2_sim::terrain;
use l2_sim::{Troop, SIDE_A, SIDE_B};
use l2_view::campaign;
use l2_view::canvas::Canvas;
use l2_view::chrome;
use l2_view::figures::{self, Anim, Colour};
use l2_view::scene::{self, BattleAssets, Camera};
use l2_view::sheet::Sheet;

/// The identity that makes the frame layout believable: a sheet holds exactly
/// eight facings of `poses_per_facing` poses, then eighteen shared frames — six
/// unclassified and twelve of dying.
///
/// One sheet agreeing would prove nothing. Thirty-six do, across six player
/// colours, and the same `poses_per_facing` also has to place the dying
/// handler's base at exactly `8 * poses + 6`. Getting the value wrong for any
/// troop breaks both identities at once.
#[test]
fn the_frame_layout_accounts_for_every_frame_of_every_shipped_sheet() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let mut checked = 0;
    for colour in Colour::ALL {
        for troop in STRIDE_TROOPS {
            let name = figures::sprite_file(colour, troop).unwrap();
            let Some(bytes) = read(&dir, &name) else {
                panic!("{name} missing from the install");
            };
            let sheet = Sheet::new(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
            let poses = figures::poses_per_facing(troop) as usize;
            assert_eq!(
                sheet.frame_count(),
                8 * poses + 18,
                "{name}: {} frames, expected 8 x {poses} + 18",
                sheet.frame_count()
            );
            // The dying block is the last twelve of the eighteen extras, and
            // every dying frame must be inside the sheet.
            for facing in 0..8u8 {
                for phase in [0u8, 40, 80] {
                    let f = figures::frame(troop, Anim::Dying, facing, phase);
                    assert!(f >= 8 * poses + 6, "{name}: dying frame {f} below its base");
                    assert!(f < sheet.frame_count(), "{name}: dying frame {f} off the sheet");
                }
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 36, "expected 6 colours x 6 troop types");
    eprintln!("frame layout: {checked} sheets agree");
}

/// **`Ui_OkButton` does not draw a tick. It draws a hole.**
///
/// A player described the corner of a county panel as *"a little mouse icon
/// around a black hole"*, against five documents and four source files calling
/// it a tick. Nobody had decoded the frame; decoding it settled it
/// (`docs/decisions.md` C46), and this is what stops the label drifting back.
///
/// The claim is made as a **rank**, because a threshold
/// picked to pass is not evidence: of the sheet's 84 frames, `Ui_OkButton`'s
/// two are the **4th and 5th darkest**, at 15.3% and 11.5% of their area in
/// near-black ink against a median frame's 1.2%. The widgets that really are
/// thin strokes sit where you would expect — the tax-up arrow at 0.2%, the
/// slider knob at 0.0%. A tick cannot come 4th out of 84.
#[test]
fn the_ok_button_frames_are_a_hole_rather_than_a_tick() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let bytes = read(&dir, "System.pl8").expect("System.pl8 is in the install");
    let sheet = Sheet::new(bytes).expect("System.pl8 decodes");
    let palette = l2_formats::Palette::from_bytes(&read(&dir, "Base01.256").expect("Base01.256"))
        .expect("the palette decodes");
    // Index 0 is this sheet's transparency, so ink is everything else, and the
    // hole is the ink that is nearly black under the campaign palette.
    let darkness = |index: usize| -> f64 {
        let Some(f) = sheet.frame(index) else { return 0.0 };
        let dark = f
            .indices
            .iter()
            .filter(|&&i| {
                let [r, g, b] = palette.rgb(i);
                i != 0 && (r as u32 + g as u32 + b as u32) < 90
            })
            .count();
        dark as f64 / f.indices.len().max(1) as f64
    };

    let dim = chrome::system::OK_DIM as usize;
    let mut all: Vec<(usize, f64)> =
        (0..sheet.frame_count()).map(|i| (i, darkness(i))).collect();
    all.sort_by(|a, b| b.1.total_cmp(&a.1));
    let median = all[all.len() / 2].1;

    for (label, index) in [("mode 0", chrome::system::OK), ("mode 1", chrome::system::OK_ALT)] {
        let f = sheet.frame(index).unwrap_or_else(|| panic!("{label}: frame {index:#04X}"));
        assert_eq!(
            (f.width as usize, f.height as usize),
            (dim, dim),
            "{label} is not 24 x 24"
        );
        let rank = all.iter().position(|&(i, _)| i == index).expect("it is in the sheet") + 1;
        let pct = darkness(index);
        assert!(
            rank <= 8,
            "{label}: frame {index:#04X} is only the {rank}th darkest of {} — a hole should be \
             near the top",
            sheet.frame_count()
        );
        assert!(
            pct > median * 8.0,
            "{label}: {:.1}% near-black against a median frame's {:.1}%",
            pct * 100.0,
            median * 100.0
        );
        eprintln!(
            "Ui_OkButton {label}: frame {index:#04X} is {:.1}% near-black, rank {rank}/{} \
             (median {:.1}%)",
            pct * 100.0,
            sheet.frame_count(),
            median * 100.0
        );
    }

    // **And the skin trap, measured.** `System2.pl8` is the same size with the
    // same 84-frame table, and §4.2 already records that 69 of its frames are
    // entirely index 0. Frame 0x10 is one of them — so a panel that draws its
    // corner in **mode 1** under skin 0 draws nothing at all, while mode 0 is
    // painted in both. That is why `Chrome::load` prefers `System.pl8`.
    let alt = Sheet::new(read(&dir, "System2.pl8").expect("System2.pl8 is in the install"))
        .expect("System2.pl8 decodes");
    let blank = alt.frame(chrome::system::OK_ALT).expect("frame 0x10");
    assert!(
        blank.indices.iter().all(|&i| i == 0),
        "System2.pl8's mode 1 frame is expected to be entirely transparent"
    );
    let painted = alt.frame(chrome::system::OK).expect("frame 0x33");
    assert!(
        painted.indices.iter().any(|&i| i != 0),
        "but its mode 0 frame is painted, which is why only mode 1 vanishes"
    );
}

/// Knights come off an 8 x 8 `(body, target)` table whose live entries are
/// spaced three apart and top out at 53. With the walk cycle's maximum of 2
/// that reaches frame 55 — and `A2*_knig.pl8` holds exactly 56 real frames
/// plus four 2x2 stubs.
#[test]
fn the_knight_frame_table_fits_the_knight_sheets() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    for colour in Colour::ALL {
        let name = figures::sprite_file(colour, Troop::Knights).unwrap();
        let sheet = Sheet::new(read(&dir, &name).expect(&name)).unwrap();
        assert_eq!(sheet.frame_count(), 60, "{name}");
        for facing in 0..8u8 {
            for phase in 0..40u8 {
                let f = figures::frame(Troop::Knights, Anim::Walking, facing, phase);
                assert!((8..56).contains(&f), "{name}: facing {facing} -> frame {f}");
                assert!(sheet.frame(f).is_some(), "{name}: frame {f} does not decode");
            }
        }
    }
    // The horse under him: eight facings of six.
    let horse = Sheet::new(read(&dir, figures::HORSE_FILE).expect("A2_horse.pl8")).unwrap();
    assert_eq!(horse.frame_count(), 48, "A2_horse.pl8 should be 8 x 6");
    for facing in 0..8u8 {
        for phase in 0..40u8 {
            assert!(figures::horse_frame(facing, phase) < 48);
        }
    }
}

/// **The missile and siege-engine frame maps against the shipped files, and
/// they close with one frame spare.**
///
/// `A2_miss.pl8` is slot 6 of the battle asset table at `0x004DA550`,
/// `engine.pl8` slot 8, `catarm1/2.pl8` slots 9 and 10. The arithmetic that
/// indexes them is read out of `BattleMan_FireMissile` (`0x00483337`),
/// `Missile_UpdateAll` (`0x00485BB1`), `FUN_00488436`, `FUN_00488793`,
/// `FUN_0048895E` and `FUN_004BE7BE`; the frame counts are read off the files.
/// Getting any of the bases wrong breaks one of the three identities:
///
/// * **81** in `A2_miss.pl8` = 33 + six shields × 8 — the banner block
/// `FUN_004BD574` indexes runs to the last frame, and the missile blocks
///   0 … 40 sit under it;
/// * **46** in `Engine.pl8`, and `(polarDirc >> 1) + 0x2A` reaches 45;
/// * **20** in each `Catarm`, and `dirc % 4 × 5 + g_horseWalkCycle[…]`
///   reaches 19.
///
/// The physical frame sizes are a fourth, independent check that costs
/// nothing: the blocks the arithmetic claims are each one size — 96 × 96 for
/// the tower's four and the docked stair's four, 128 × 120 for the catapult's
/// eight, 32 × 32 for the pot's eleven.
#[test]
fn the_missile_and_engine_frame_maps_fit_the_shipped_sheets() {
    use l2_sim::missile::{Missile, CLASS_DEBRIS, CLASS_FIRE};
    use l2_sim::Motion;
    use l2_view::{engines, missiles};

    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let sheet = |name: &str| Sheet::new(read(&dir, name).unwrap_or_else(|| panic!("{name}"))).unwrap();

    let miss = sheet(missiles::MISSILE_SHEET);
    assert_eq!(miss.frame_count(), 81, "A2_miss.pl8 = 33 + 6 shields x 8");
    for class in [1u8, 2, 3, CLASS_DEBRIS, CLASS_FIRE] {
        for dir in 0..8u8 {
            for ttl in 0..=0x280i16 {
                let m = Missile { owner: 1, class, dir, ttl, ..Missile::default() };
                let Some(f) = missiles::frame(&m) else { continue };
                assert!(miss.frame(f).is_some(), "A2_miss.pl8 frame {f} (class {class})");
            }
        }
    }
    // The banner `FUN_004BD574` draws — `shield * 8 + counter + 0x21` — ends
    // on the sheet's last frame, which is what fixes the six blocks under it.
    assert_eq!(0x21 + 5 * 8 + 7, miss.frame_count() - 1);

    let engine = sheet(engines::ENGINE_SHEET);
    assert_eq!(engine.frame_count(), 46, "Engine.pl8");
    for troop in [Troop::Catapults, Troop::SiegeTowers, Troop::BatteringRams, Troop::Oil] {
        for anim in [Motion::Idle, Motion::Walking, Motion::Attacking, Motion::Dying] {
            for facing in 0..8u8 {
                for polar in [0u8, 2, 4, 6] {
                    for phase in 0..=255u8 {
                        let f = engines::frame(troop, anim, facing, polar, phase).unwrap();
                        assert!(engine.frame(f).is_some(), "Engine.pl8 frame {f} ({troop:?})");
                    }
                }
            }
        }
    }
    for phase in 0..=255u8 {
        let ((u, _), (l, _)) = engines::ram_strips(Motion::Attacking, phase).unwrap();
        assert!(engine.frame(u).is_some() && engine.frame(l).is_some(), "the ram's strips");
    }
    for (_, f) in engines::DOCK_OVERLAY {
        assert!(engine.frame(f).is_some(), "the docked stair, frame {f}");
    }

    // Each block the map claims is one physical size, which no wrong base
    // could reproduce.
    let size = |i: usize| engine.frame(i).map(|f| (f.width, f.height)).unwrap();
    assert!((1..=4).all(|i| size(i) == (96, 96)), "the tower's four facings");
    assert!((5..=12).all(|i| size(i) == (128, 120)), "the catapult's eight");
    assert!((0x1F..=0x22).all(|i| size(i) == (96, 96)), "the docked stair's four");
    assert!((0x23..=0x2D).all(|i| size(i) == (32, 32)), "the pot's eleven");
    assert_eq!(size(0x0D), size(0x15), "the ram's lower strip is one block");
    assert_eq!(size(0x16), size(0x1E), "and its upper strip another");

    for (half, name) in engines::ARM_SHEETS.iter().enumerate() {
        let arm = sheet(name);
        assert_eq!(arm.frame_count(), 20, "{name} = 4 facings x 5 poses");
        for facing in 0..8u8 {
            if engines::arm_sheet(facing) != half {
                continue;
            }
            for swing in 0..=engines::ARM_SWING_TICKS {
                let f = engines::arm_frame(facing, swing);
                assert!(arm.frame(f).is_some(), "{name} frame {f}");
            }
        }
    }
}

