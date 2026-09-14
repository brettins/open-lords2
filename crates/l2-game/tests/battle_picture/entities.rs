#![allow(unused_imports)]
use super::*;
use super::render::*;
use super::motion::*;
use super::panel::*;
use super::ui::*;
use l2_formats::{DecodedFrame, Palette};
use l2_game::battlefield::{self as bf, LiveBattle};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::menubar;
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::terrain::DIM;
use l2_sim::{Motion, Troop, SIDE_A, SIDE_B};
use l2_view::sheet::Sheet;
use l2_view::Canvas;

/// **An arrow in the air is drawn, in the right frame and at the right
/// pixel** — `FUN_004BEED4` (`0x004BEED4`), the pass after the men.
///
/// Two literals out of the binary, and the test is built from them
/// from `l2_view`:
///
/// * the **frame** is `g_missileStats[class][4] + missile[+0x2E]` —
///   `BattleMan_FireMissile` (`0x00483337`) writes
///   `(ushort)missile[+0x2E] + missile[+0x34]`, and column 4 of
///   `g_missileStats` (`0x004D97B0`) is 0 for a bow;
/// * the **pixel** is `origin + (missile[+0x0A] − camX·0x20) + tileSize/2`,
///   from `FUN_004BC020`'s `DAT_004E6558 = 0`, `DAT_004E6554 = 0x18` and
///   `DAT_004E5D44 = param_11 / 2`. A missile's position is in thirty-seconds
///   of a cell and a battle tile is 32 pixels, so the two units are one unit.
///   every figure has one and a
///   missile does not.
///
/// The sheet is the install's own `A2_miss.pl8`, decoded here, and the frame
/// has to sit on the canvas exactly.
///
/// Ablation: drop the `+ TILE / 2` in `scene::draw_overlay_and_missiles` —
/// red, the arrow is found 16 pixels up and left of where the record puts it.
/// Drop the `m.dir` term in `missiles::frame` — red, every arrow drawn as
/// frame 0.
#[test]
fn an_arrow_in_flight_is_drawn_where_the_missile_record_puts_it() {
    let Some((assets, platform)) = install() else {
        l2_testkit::skip!("no game install, so no A2_miss.pl8 to find the arrow with");
    };
    let miss = Sheet::new(platform.vfs.read("A2_miss.pl8").expect("A2_miss.pl8")).expect("a PL8");

    // An archer standing on his deployment cell shoots the peasant walking at
    // him from six cells east. Nothing is ordered: an un-ordered figure's
    // destination is where it stands, which is what `fire_tick` wants.
    let (mut g, mut m) = staged_on(
        field_at((46, 36)),
        &[(Troop::Archers, 1)],
        &[(Troop::Peasants, 1)],
        |(x, y)| (x as i32 - 4, y as i32 - 6),
    );
    let mut canvas = Canvas::screen();

    let mut found = None;
    for _ in 0..240 {
        frame(&mut m, &mut g, &assets, &mut canvas);
        let shot = live(&g).runner.missiles.iter().next().map(|(_, m)| *m);
        if let Some(shot) = shot {
            // Repaint on a clean canvas, so nothing an earlier frame left
            // behind can be mistaken for the arrow.
            canvas = Canvas::screen();
            paint(&mut m, &mut g, &assets, &mut canvas);
            found = Some((shot, live(&g).cam));
            break;
        }
    }
    let Some((shot, cam)) = found else { panic!("no archer loosed in 240 frames") };

    assert_eq!(shot.class, 1, "a bow is class 1");
    let index = (shot.dir & 7) as usize; // + g_missileStats[1][4], which is 0
    let arrow = miss.frame(index).expect("the arrow's frame decodes");
    let want = (
        (shot.x as i32 - cam.0 * 32) + 16,      // DAT_004E6558 = 0
        24 + (shot.y as i32 - cam.1 * 32) + 16, // DAT_004E6554 = 0x18
    );
    assert!(
        want.0 >= FIELD_X0 && want.0 < FIELD_X1 - 8 && want.1 >= FIELD_Y0 && want.1 < FIELD_Y1 - 8,
        "the arrow is off the viewport at {want:?} — the test cannot see it"
    );
    let hits = locate(&canvas, &arrow, want.0 - 6..want.0 + 7, want.1 - 6..want.1 + 7);
    assert!(
        hits.contains(&want),
        "frame {index} of A2_miss.pl8 is not at {want:?}; found at {hits:?}"
    );
}

/// **A catapult is drawn from `Engine.pl8`, with its arm from `Catarm1.pl8`
/// over it** — `FUN_00480F8B`'s colourless sheet, `FUN_00488436`'s
/// `dirc + 5`, and `FUN_004BE7BE`'s second sprite.
///
/// The literals, again from the binary and not from `l2_view`: the carriage is
/// frame `dirc + 5` of slot 8 of the asset table at `0x004DA550`
/// (`engine.pl8`); the arm is frame `dirc·5 − 0x14·(dirc > 3)` of slot 9 or 10
/// (`catarm1/2.pl8`) plus `g_horseWalkCycle[animPhase >> 2]`, which is 0 at
/// rest; and both sit at `cell + (0x10 − w/2, 8 − w/2)`, `BattleFigure_Draw`'s
/// own centring with no `troopType` nudge, because a catapult is neither 9 nor
/// 10.
///
/// Ablation: return `None` from `engines::frame` for a catapult — red, *"only
/// 0% of the carriage is at (176, 192)"*, which is the gap this branch closed.
/// Force `arm_sheet` to 0 — red, this catapult deploys facing west and the
/// arm comes out of `Catarm1.pl8`.
#[test]
fn a_catapult_is_drawn_from_engine_pl8_with_its_arm_on_top() {
    let Some((assets, platform)) = install() else {
        l2_testkit::skip!("no game install, so no Engine.pl8 to find the catapult with");
    };
    let engine = Sheet::new(platform.vfs.read("Engine.pl8").expect("Engine.pl8")).expect("a PL8");
    let arms = [
        Sheet::new(platform.vfs.read("Catarm1.pl8").expect("Catarm1.pl8")).expect("a PL8"),
        Sheet::new(platform.vfs.read("Catarm2.pl8").expect("Catarm2.pl8")).expect("a PL8"),
    ];

    let (mut g, mut m) =
        staged(24, &[(Troop::Catapults, 1)], &[(Troop::Peasants, 1)], |(x, y)| {
            (x as i32 - 7, y as i32 - 7)
        });
    let mut canvas = Canvas::screen();
    paint(&mut m, &mut g, &assets, &mut canvas);

    let cat = human_figure(&g);
    let f = &live(&g).runner.fighters[cat];
    assert_eq!(f.troop, Troop::Catapults);
    let facing = (f.facing % 8) as usize;
    let (cx, cy) = (f.x as i32 - live(&g).cam.0, f.y as i32 - live(&g).cam.1);
    let cell = (bf::VIEW.x + cx * bf::TILE, bf::VIEW.y + cy * bf::TILE);

    // The carriage is under the arm,
    // and an exact match is the wrong test for it — most of it must be there,
    // and somewhere else must not be.
    let body = engine.frame(5 + facing).expect("the carriage decodes");
    let want = (cell.0 + (32 / 2 - body.width as i32 / 2), cell.1 - body.width as i32 / 2 + 8);
    let here = matched(&canvas, &body, want);
    let elsewhere = matched(&canvas, &body, (want.0 + 24, want.1 + 24));
    assert!(here > 0.6, "only {:.0}% of the carriage is at {want:?}", here * 100.0);
    assert!(elsewhere < here / 2.0, "the carriage matches open ground just as well");

    // `dirc < 4` picks the first file; the base is `dirc * 5`, wrapped.
    let half = usize::from(facing >= 4);
    let arm_index = facing % 4 * 5;
    let arm = arms[half].frame(arm_index).expect("the arm decodes");
    let awant = (cell.0 + (32 / 2 - arm.width as i32 / 2), cell.1 - arm.width as i32 / 2 + 8);
    let ahits = locate(&canvas, &arm, awant.0 - 4..awant.0 + 5, awant.1 - 4..awant.1 + 5);
    assert!(
        ahits.contains(&awant),
        "Catarm{}.pl8 frame {arm_index} is not at {awant:?}; found at {ahits:?}",
        half + 1
    );
}

/// **A pot of boiling oil is drawn eight pixels lower than everything else,
/// and it bubbles** — `BattleFigure_Draw`'s `troopType == 10` arm
/// (`g_drawY += 8`) and `FUN_00488436`'s `(animPhase >> 3) + 0x23`.
///
/// The nudge is one of exactly two in the painter — `+0x0C` for a ram, `+8`
/// for a pot, on y only — and nothing else in the game has one. The idle loop
/// is six frames over forty-eight ticks,
/// changes picture, which is the half a static assertion would miss.
///
/// Ablation: return 0 from `engines::body_y_nudge` for a pot — red, frame 36
/// found at `(224, 240)` where the painter puts it at `(224, 248)`. Hold the
/// idle frame at `0x23` — red, frame 36 nowhere on the canvas.
#[test]
fn a_pot_of_oil_sits_eight_pixels_low_and_bubbles_through_six_frames() {
    let Some((assets, platform)) = install() else {
        l2_testkit::skip!("no game install, so no Engine.pl8 to find the pot with");
    };
    let engine = Sheet::new(platform.vfs.read("Engine.pl8").expect("Engine.pl8")).expect("a PL8");

    // **A side of nothing but siege engines has already lost**, so the pot
    // needs a man beside it or the battle ends on the first tick and the
    // screen stops drawing the field. A peasant holds it open.
    let (mut g, mut m) = staged(
        24,
        &[(Troop::Oil, 1), (Troop::Peasants, 1)],
        &[(Troop::Peasants, 1)],
        |(x, y)| (x as i32 - 7, y as i32 - 7),
    );
    let mut canvas = Canvas::screen();
    let pot = live(&g)
        .runner
        .fighters
        .iter()
        .position(|f| f.troop == Troop::Oil && f.side == SIDE_A)
        .expect("a pot of oil was raised");

    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..60 {
        canvas = Canvas::screen();
        frame(&mut m, &mut g, &assets, &mut canvas);
        let f = &live(&g).runner.fighters[pot];
        assert_eq!(f.anim, Motion::Idle, "the pot is meant to stand still");
        // `(animPhase >> 3) + 0x23`, animPhase wrapped past 0x2F.
        let index = 0x23 + (f.phase as usize % 0x30) / 8;
        seen.insert(index);
        let sprite = engine.frame(index).expect("the pot decodes");
        let (cx, cy) = (f.x as i32 - live(&g).cam.0, f.y as i32 - live(&g).cam.1);
        let w = sprite.width as i32;
        // `+ 8` twice: the sprite centring every figure gets, and the pot's
        // own `troopType == 10` nudge.
        let want = (
            bf::VIEW.x + cx * bf::TILE + (32 / 2 - w / 2),
            bf::VIEW.y + cy * bf::TILE - w / 2 + 8 + 8,
        );
        let hits = locate(&canvas, &sprite, want.0 - 12..want.0 + 13, want.1 - 12..want.1 + 13);
        assert!(
            hits.contains(&want),
            "Engine.pl8 frame {index} is not at {want:?}; found at {hits:?}"
        );
    }
    assert_eq!(
        seen.iter().copied().collect::<Vec<_>>(),
        vec![0x23, 0x24, 0x25, 0x26, 0x27, 0x28],
        "the pot showed {} of its six pictures in sixty frames",
        seen.len()
    );
}

// --------------------------------------------------------------- the menu bar
//
// > *"The menu buttons are deactivated in battle mode now."*
//
// Two facts, both read out of the binary and neither of them a grey:
//
// * `Screen_DrawMenuBar` (`0x00419C78`) guards itself on a list of screen ids
//   it **refuses** — `0x08 … 0x0D`, `0x17`, `0x1B … 0x20`, `0x22`,
//   `0x2C … 0x2F` — and `0x29`, `0x2A` and `0x2B` are in none of them. The bar
//   is painted through a battle; ours painted nothing above `VIEW.y == 24`, and
//   an empty bar and a dead bar look the same.
// * `Screen_FrameInput`'s `0x29` arm **opens** with
//   `Menu_OpenDropdown(&g_menuBarItems, 3)`, ahead of `Map_EdgeScroll`,
//   `Battle_ButtonClicked`, `Battle_DragSelect`, `Battle_OrderClicked` and
//   `Battle_UnitPanelClicked`, and nothing gates it — not `g_battlePhase`, not
//   the pause word, not `g_battleChoiceOwner`.
//
// `FUN_0040C725`
// paints each row in `0x18` under the pointer and `0x3F` otherwise, with no
