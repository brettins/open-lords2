#![allow(unused_imports)]
use super::*;
use super::render::*;
use super::interaction::*;
use common::*;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::map;
use l2_game::screens::village::{self as village_screen};
use l2_game::screens::village::VillageScreen;
use l2_view::chrome;
use l2_view::village;
use l2_view::Canvas;

/// **The village animates, and `villani1.pl8` is what the iron mine animates
/// from.**
///
/// `Village_Animate` (`0x00412421`) draws six overlays and the sixth is the
/// only read of `villani1.pl8` in the whole executable — a file this project
/// had recorded as *"loaded by nothing"*. Three of the six are unconditional
/// and run in every county.
///
/// The clock is checked as *pulses*, not as pixels alone: eighty milliseconds
/// is one step of the fast counter and a hundred and sixty of the slow one,
/// which is `FUN_004BBC80`'s divider chain and not a rate of ours.
#[test]
fn the_village_animates_and_the_iron_mine_comes_out_of_villani1() {
    let (mut game, assets) = world!();
    let art = assets.village.as_ref().expect("the village artwork");
    assert!(art.has_villani1(), "villani1.pl8 is in the install and the iron mine needs it");

    let iron = game
        .kingdom
        .county_ids()
        .find(|&id| game.kingdom.counties[id].industry[1].has_resource)
        .expect("England has an iron county");

    let mut screen = VillageScreen::new(iron as u8);
    let first = draw(&mut screen, &mut game, &assets);

    // One slow pulse: eight 20 ms gates, and a gate is two 16 ms ticks because
    // `Tick_Pulses` (`0x004BBC80`) drops its remainder — C179. Every one of the
    // six overlays has moved at least once by then.
    let ticks = village::PULSE_SLOW_MS / village::GATE_MS * 2;
    assert_eq!(ticks, 16, "160 ms is eight gates, sixteen fixed ticks");
    for _ in 0..ticks {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.update(&mut ctx);
    }
    assert!(screen.take_redraw(), "a moved animation asks for a repaint");
    let later = draw(&mut screen, &mut game, &assets);
    let moved = first.diff_count(&later);
    assert!(moved > 40, "the village is still after ten ticks: {moved} pixels moved");

    // The clock is display state and nothing else: stepping it must not touch
    // the world. If it ever did, this is the assertion that would say so.
    let before = game.kingdom.clone();
    for _ in 0..100 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.update(&mut ctx);
    }
    assert_eq!(game.kingdom, before, "the animation clock reached the simulation");
}

/// **`villani2.pl8`'s own frame table confirms every one of the six overlays,
/// independently of the decompilation.**
///
/// The six runs in [`village::OVERLAYS`] — their first frame and their length —
/// were read from `Village_Animate`'s counter bounds. The sheet
/// looked at. Looking at it: `villani2.pl8`'s 44 frames fall into **five blocks
/// of equal-sized frames laid out in rows on the artist's canvas**, and the
/// blocks are
///
/// | frames | size | overlay |
/// |---|---|---|
/// | 0 … 6 | 26 × 29 | the stone quarry's, 7 frames |
/// | 7 … 14 | 39 × 40 | the lumber camp's, 8 |
/// | 15 … 24 | 15 × 12 | the third unconditional one, 10 |
/// | 25 … 32 | 32 × 42 | the first unconditional one, 8 |
/// | 33 … 39 | 19 × 18 | the second unconditional one, 7 |
///
/// which is **exactly** the six-entry table, start index and length, five times
/// over. What is left is frames 40, 41 and 43 — the three static buildings
/// `Village_Draw` blits at `0x28`, `0x29` and `0x2B` — and one 2 × 2 stub at
/// 42. Nothing over, nothing short.
///
/// A block boundary that fell one frame from where the counter wraps would show
/// up here as an animation that jumps to a different-sized picture, and this is
/// the assertion that would catch it. Sizes are the evidence and the
/// decompilation is the claim; they agree.
#[test]
fn the_animation_runs_are_the_blocks_the_sheet_is_laid_out_in() {
    let Some(dir) = install() else {
        l2_testkit::skip!("no game install, so there is no sheet to read");
    };
    let path = std::fs::read_dir(&dir)
        .ok()
        .and_then(|d| {
            d.filter_map(|e| e.ok()).map(|e| e.path()).find(|p| {
                p.file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|f| f.eq_ignore_ascii_case("villani2.pl8"))
            })
        })
        .expect("villani2.pl8 is in the install");
    let bytes = std::fs::read(path).expect("villani2.pl8 reads");
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("villani2.pl8 decodes");
    assert_eq!(pl8.frames.len(), 44);

    for overlay in village::OVERLAYS.iter().filter(|o| !o.villani1) {
        let run = &pl8.frames[overlay.first..overlay.first + overlay.frames];
        let size = (run[0].width, run[0].height);
        for (i, f) in run.iter().enumerate() {
            assert_eq!(
                (f.width, f.height),
                size,
                "frame {} of the run at {} is a different size",
                overlay.first + i,
                overlay.first
            );
        }
        // The frame *before* the run and the frame *after* it must both be a
        // different size, or the boundary is not where the counter wraps.
        if overlay.first > 0 {
            let prev = &pl8.frames[overlay.first - 1];
            assert_ne!(
                (prev.width, prev.height),
                size,
                "the run at {} starts one frame late: {} is the same size",
                overlay.first,
                overlay.first - 1
            );
        }
        let after = overlay.first + overlay.frames;
        let next = &pl8.frames[after];
        assert_ne!(
            (next.width, next.height),
            size,
            "the run at {} is one frame short: {after} is the same size",
            overlay.first
        );
    }

    // The three static buildings are the three big frames past the animation
    // blocks, and the sheet has nothing else in it.
    for (_, frame, _, _) in village::RESOURCE_BUILDINGS {
        let f = &pl8.frames[frame];
        assert!(f.width > 100 && f.height > 70, "frame {frame:#04X} is not a building");
    }
    eprintln!("villani2: five animation blocks and three buildings account for all 44 frames");
}

/// **Every overlay's frame run is inside the sheet it is indexed against**, and
/// The two counters nothing draws are recorded.
///
/// A frame index off the end of a PL8 is a hole here;
/// without this the six runs could be wrong by any amount and nothing would
/// say so.
///
/// # `villani1.pl8` holds 21 frames and the shipped game plays 18 of them
///
/// **[V]** for both numbers. The iron mine's counter, `DAT_004D2938`, wraps at
/// `0x11`, so it visits 0 … 17 and three frames of the file are never drawn.
///
/// And `Village_Animate`'s **dead** counter `DAT_004D2934` wraps at `0x14` —
/// **21 states, which is exactly the file's frame count** — and is read by
/// nothing in the executable. **[I]**, and deliberately only that: the two
/// Numbers agreeing is a coincidence.
/// that the mine was meant to run off that counter. `docs/bugs.md` B65 records
/// the numbers and says the same thing.
#[test]
fn every_village_overlay_run_fits_inside_its_own_sheet() {
    let Some(dir) = install() else {
        l2_testkit::skip!("no game install, so there are no sheets to index");
    };
    let read = |name: &str| -> Option<Vec<u8>> {
        std::fs::read_dir(&dir)
            .ok()?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .and_then(|f| f.to_str())
                    .is_some_and(|f| f.eq_ignore_ascii_case(name))
            })
            .and_then(|p| std::fs::read(p).ok())
    };
    let counts: Vec<usize> = ["villani1.pl8", "villani2.pl8"]
        .iter()
        .map(|n| {
            let b = read(n).unwrap_or_else(|| panic!("{n} is in the install"));
            l2_formats::Pl8::parse(&b).unwrap_or_else(|e| panic!("{n}: {e}")).frames.len()
        })
        .collect();
    assert_eq!(counts[0], 21, "villani1.pl8 holds 21 frames");
    assert_eq!(counts[1], 44, "villani2.pl8 holds 44");

    let mut clock = village::AnimationClock::new();
    let mut highest = [0usize; 2];
    // Several full turns of the longest run, so every counter visits every
    // value it can take.
    for _ in 0..18 * 4 {
        // One slow pulse is eight gates, and a gate takes whatever it takes.
        for _ in 0..village::PULSE_SLOW_MS / village::GATE_MS {
            clock.tick(village::GATE_MS);
        }
        for overlay in &village::OVERLAYS {
            let f = clock.frame_of(overlay);
            let sheet = usize::from(!overlay.villani1);
            assert!(
                f < counts[sheet],
                "frame {f} is off the end of {} ({} frames)",
                if overlay.villani1 { "villani1.pl8" } else { "villani2.pl8" },
                counts[sheet]
            );
            highest[sheet] = highest[sheet].max(f);
        }
    }
    // The iron mine stops at 17, three frames short of the file's end. That
    // is the original's, not a clamp of ours: `DAT_004D2938` wraps at `0x11`.
    assert_eq!(highest[0], 17, "the iron mine's counter visits 0 … 17");
    assert_eq!(counts[0] - (highest[0] + 1), 3, "three frames of villani1 are never drawn");

    // The two counters `Village_Animate` steps and nothing reads. Kept as a
    // claim so that finding a consumer later fails this and gets looked at.
    assert_eq!(village::DEAD_COUNTER_PERIODS.len(), 2);
    assert!(
        village::OVERLAYS.iter().all(|o| o.frames != 21),
        "no overlay uses the 21-state counter, which is why it is called dead"
    );
    // …and the coincidence, asserted so that it stays visible: the dead
// counter has as many states as villani1.pl8 has frames.
    assert_eq!(
        village::DEAD_COUNTER_PERIODS[0].1,
        counts[0],
        "the 21-state dead counter and villani1's 21 frames"
    );
}

