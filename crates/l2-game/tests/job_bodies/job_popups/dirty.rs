//! **What screen `0x0F` marks dirty, and when.**
//!
//! Drawing is never gated in the original — every painter draws into the back
//! buffer whenever it is called — so what a mark decides is whether the frame
//! reaches the screen. Three markers touch this panel, all **[V]**:
//!
//! * `Panel_JobDetail` (`0x00412B33`) third statement, `Gfx_MarkAllDirty()`,
//!   before it reads `iconvill.pl8`: the panel opens with the whole frame
//!   marked, whichever of the nine jobs it is.
//! * `Panel_JobBlacksmith` (`0x00413155`) last statement, `Gfx_MarkAllDirty()`
//!   again. Its two callers are `Panel_JobDetail` and `FUN_0043A997`, the
//!   weapon change — an opening and a choice, never a per-frame cost.
//! * `FUN_00413526` last statement, `Gfx_MarkSpriteDirty(0x58, 0x9D, 8, 8, 1)`
//!   — 128 × 128 at (88, 157), **not** the frame. It is the only per-frame
//!   marker here and `Screen_DrawWidgets` (`0x004BA26E`) runs it behind
//!   `if (g_jobPanelJob == 8)`.
//!
//! Ablations, run. The `job == BLACKSMITH` guard in `JobScreen::update`
//! deleted → the grain popup asks for a repaint every 80 ms and
//! `a_still_job_popup_marks_nothing_after_it_opens` fails at the first pulse.
//! `mark_sprite` swapped for `mark_all` → `the_forge_fire_marks_its_own_cells`
//! sees `(0, 0, 0x27F, 0x1DF)`. The `mark_all` in `JobScreen::new` deleted →
//! `the_job_popup_opens_with_the_whole_frame_marked` sees `None`.

#![allow(unused_imports)]
use super::*;
use l2_game::screen::{Ctx, Screen, Transition};
use l2_game::screens::job::{JobScreen, HOTSPOT_ORIGIN, WEAPON_HOTSPOTS};

/// `Gfx_MarkAllDirty`'s rectangle — `Gfx_MarkDirty(0, 0, 0x27F, 0x1DF)`.
const WHOLE_FRAME: (i32, i32, i32, i32) = (0, 0, 0x27F, 0x1DF);
/// `Gfx_MarkSpriteDirty(0x58, 0x9D, 8, 8, 1)` in pixels: eight cells of sixteen
/// on both axes, added to the origin.
const FIRE_RECT: (i32, i32, i32, i32) = (0x58, 0x9D, 0x58 + 0x80, 0x9D + 0x80);

/// Our zero-based blacksmith, `g_jobPanelJob == 8`.
const BLACKSMITH: usize = 7;

/// Enough fixed ticks to carry the fire past one `g_pulse80`, with room: the
/// divider is small and the test asserts *which* frame moved, not when.
const TICKS: usize = 400;

#[test]
fn the_job_popup_opens_with_the_whole_frame_marked() {
    for job in [JOB_GRAIN_FARMING, JOB_CASTLE_BUILDING, BLACKSMITH] {
        let mut screen = JobScreen::new(1, job);
        assert_eq!(
            screen.take_dirty_rect(),
            Some(WHOLE_FRAME),
            "job {job} opens with Panel_JobDetail's Gfx_MarkAllDirty"
        );
        assert_eq!(screen.take_dirty_rect(), None, "job {job}: one mark, one present");
    }
}

#[test]
fn a_still_job_popup_marks_nothing_after_it_opens() {
    let (mut game, assets) = england_world!();
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    for job in [JOB_GRAIN_FARMING, JOB_CATTLE_FARMING, JOB_IRON_MINING, JOB_FIELD_RECLAMATION] {
        let mut screen = JobScreen::new(1, job);
        assert!(screen.take_redraw(), "job {job}'s opening mark");
        for tick in 0..TICKS {
            screen.update(&mut ctx);
            assert!(
                !screen.take_redraw(),
                "job {job} asked for a repaint on tick {tick}; \
                 Screen_DrawWidgets runs FUN_00413526 only when g_jobPanelJob == 8"
            );
        }
    }
}

#[test]
fn the_forge_fire_marks_its_own_cells() {
    let (mut game, assets) = england_world!();
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let mut screen = JobScreen::new(1, BLACKSMITH);
    assert_eq!(screen.take_dirty_rect(), Some(WHOLE_FRAME), "the opening mark");

    let mut marks = Vec::new();
    for _ in 0..TICKS {
        screen.update(&mut ctx);
        if let Some(r) = screen.take_dirty_rect() {
            marks.push(r);
        }
    }
    assert!(!marks.is_empty(), "the fire never moved in {TICKS} ticks");
    for r in &marks {
        assert_eq!(*r, FIRE_RECT, "the fire marks Gfx_MarkSpriteDirty's cells, not the frame");
    }
}

#[test]
fn the_weapon_choice_marks_the_whole_frame() {
    let (mut game, assets) = england_world!();
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let mut screen = JobScreen::new(1, BLACKSMITH);
    assert_eq!(screen.take_dirty_rect(), Some(WHOLE_FRAME), "the opening mark");

    // The near corner of weapon 5's rectangle, in screen coordinates:
    // `Hotspot_Test(0, 0x18, …)` offsets the table by the smithy's origin.
    let (x0, y0, ..) = WEAPON_HOTSPOTS[5];
    let t = screen.handle(
        l2_game::input::Event::Click { x: x0 + HOTSPOT_ORIGIN.0, y: y0 + HOTSPOT_ORIGIN.1 },
        &mut ctx,
    );
    assert_eq!(t, Transition::Stay, "the weapon choice stays on the page");
    assert_eq!(
        screen.take_dirty_rect(),
        Some(WHOLE_FRAME),
        "FUN_0043A997 repaints through Panel_JobBlacksmith, whose last statement marks all"
    );
}
