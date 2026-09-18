#![allow(unused_imports)]
use super::*;
use super::menu_bar::*;
use super::*;
use super::render::*;
use super::motion::*;
use super::panel::*;
use super::entities::*;
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

/// `Menu_SaveGame` (`0x00433F49`) is reachable from `0x29` and does not test
/// `g_battlePhase`; `Save_Write` (`0x004ADE93`) then writes `g_battlefield`
/// (`0x005440E0`), `g_battleMen` (`0x00554480`), the missiles (`0x0057A100`)
/// and `g_battleUnits` (`0x00566520`) as blocks 1, 7, 8 and 9 of its table.
/// **[V]** — those four are all-zero in `battle-before.sav`, saved on the map,
/// and set in `noorders-start.sav`, saved with the battle screen open.
#[test]
fn saving_mid_battle_writes_it_and_loading_lands_back_on_the_field() {
    let Some((assets, _p)) = install() else {
        l2_testkit::skip!("no game install");
    };
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("saves-midfight-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a directory of the test's own");
    let _scope = l2_game::saves::scoped_dir(&dir);
    let written = || {
        std::fs::read_dir(&dir)
            .expect("the test's own directory")
            .filter_map(|e| Some(e.ok()?.file_name().to_string_lossy().into_owned()))
            .collect::<Vec<_>>()
    };

    let (mut g, mut m) = skirmish();
    for _ in 0..40 {
        let mut ctx = Ctx { game: &mut g, assets: &assets };
        m.update(&mut ctx);
    }
    let saved_tick = live(&g).runner.tick;
    assert!(saved_tick > 0, "the battle never ran, so there is nothing to lose");

    let menu = |m: &mut Machine, g: &mut Game, item: usize| {
        let titles = {
            let ctx = Ctx { game: g, assets: &assets };
            menubar::titles(&ctx)
        };
        let at = (titles[0].x + titles[0].w / 2, titles[0].y + titles[0].h / 2);
        send(m, g, &assets, Event::Pointer { x: at.0, y: at.1 });
        send(m, g, &assets, Event::Click { x: at.0, y: at.1 });
        let row = menubar::item_rect(&titles, 0, item);
        let on_row = (row.x + row.w / 2, row.y + row.h / 2);
        send(m, g, &assets, Event::Pointer { x: on_row.0, y: on_row.1 });
        send(m, g, &assets, Event::Click { x: on_row.0, y: on_row.1 });
    };

    // **`SaveLoad_Tick`'s latch**: Enter arms it and the file is written
    // `WORK_TICKS` ticks later. The name comes out lowercase —
    // `Edit_TypeChar`'s `0x004011B0`, `if ('A' <= ch && ch <= 'Z') ch += 0x20`.
    let frames = l2_game::screens::saveload::WORK_TICKS as u32 + 4;
    let typed = |m: &mut Machine, g: &mut Game| {
        for c in "MIDFIGHT".chars() {
            send(m, g, &assets, Event::Text(c));
        }
        send(m, g, &assets, Event::KeyDown(l2_game::input::Key::Enter));
        for _ in 0..frames {
            let mut ctx = Ctx { game: g, assets: &assets };
            m.update(&mut ctx);
        }
    };

    menu(&mut m, &mut g, 2);
    assert_eq!(
        m.top_id(),
        Some(ScreenId::SaveLoad(l2_game::screens::saveload::Mode::Save)),
        "File > Save did not open the save box from the battlefield"
    );
    typed(&mut m, &mut g);
    assert_eq!(
        written(),
        vec!["midfight.l2sav".to_string()],
        "the save was refused or written elsewhere"
    );
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "the box did not close on success");
    // The file is written on `SaveLoad_Tick`'s `WORK_TICKS`th tick and the
    // battle winds on underneath the box, so this is the frame the file holds
    // plus the few the closing box ran.
    let wrote_at = live(&g).runner.tick;
    assert!(wrote_at > saved_tick, "the battle stood still under the save box");

    // Run the battle on, so a load that did nothing would be visible.
    for _ in 0..120 {
        let mut ctx = Ctx { game: &mut g, assets: &assets };
        m.update(&mut ctx);
    }
    let before_load = live(&g).runner.tick;
    assert!(before_load > saved_tick + 60, "the battle stood still while the box was open");

    menu(&mut m, &mut g, 1);
    assert_eq!(
        m.top_id(),
        Some(ScreenId::SaveLoad(l2_game::screens::saveload::Mode::Load)),
        "File > Load did not open the load box from the battlefield"
    );
    typed(&mut m, &mut g);
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Battlefield),
        "a load of a mid-battle save must land on the battlefield"
    );
    // The loader lands `WORK_TICKS` into the run and the battlefield winds the
    // battle for the few frames after it, so the tick is the written one plus
    // those - and nowhere near where the run had got to.
    let back = live(&g).runner.tick;
    assert!(back + 60 < before_load, "the battle carried on instead of coming back from the file");
    assert!(back <= wrote_at + 8, "the battle came back at frame {back}, written at {wrote_at}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **What the loader does that the blocks cannot.** `Save_Read` (`0x004ADFE6`)
/// closes the file and calls `FUN_004976A1` (`004a0000.c:5952`), which restarts
/// every live realm's turn program — `aiStep = 1` where `strength != 0`,
/// `00490000.c:3077-3080` — and, through `FUN_00477C89`, clears the withdrawal
/// (`00470000.c:3220`). Neither `g_battleWithdrawal` (`0x0056D5C8`) nor the side
/// that left (`0x005656F8`) is in `g_saveBlocks` (`0x004DE960`), so the original
/// cannot bring one back even by accident. Ablation: drop the `save::resume`
/// line from `saves::read_path` and both halves come back out of the file.
#[test]
fn the_loader_restarts_the_ai_program_and_cannot_bring_a_withdrawal_back() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("saves-resume-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a directory of the test's own");
    let _scope = l2_game::saves::scoped_dir(&dir);

    let (mut g, _m) = skirmish();
    g.kingdom.realms[2].strength = 40;
    g.kingdom.realms[2].ai_step = l2_kingdom::AI_STEP_DONE;
    // The county's ladder says 2 and the file will say 0: `FUN_00477C89`
    // recomputes over block 125 (`00470000.c:3197-3208`).
    g.kingdom.counties[1].castle_type = 3;
    g.kingdom.counties[1].castle_degraded = 0;
    let b = g.battle.as_mut().expect("a live battle");
    b.runner.withdraw(l2_sim::SIDE_B);
    b.county = 1;
    b.castle_level = Some(0);
    b.paused = false;
    let tables = g.kingdom.tables;

    // The blocks carry both: `g_realms` is block 3 and our `withdrawn` is in
    // the battle tag, so what follows is the loader and not the codec.
    let coded = l2_game::save::decode(&l2_game::save::encode(&g), tables).expect("our own bytes");
    assert_eq!(coded.kingdom.realms[2].ai_step, l2_kingdom::AI_STEP_DONE);
    let coded = coded.battle.expect("a live battle");
    assert_eq!(coded.runner.withdrawal(), Some(l2_sim::SIDE_B), "the codec is meant to be a codec");
    assert_eq!((coded.castle_level, coded.paused), (Some(0), false));

    l2_game::saves::write("resumed", &g).expect("the test's own directory");
    let back = l2_game::saves::read("resumed", tables).expect("our own bytes");
    assert_eq!(
        back.kingdom.realms[2].ai_step, 1,
        "a realm parked at AI_STEP_DONE must restart its program on a load"
    );
    let back = back.battle.expect("a live battle");
    assert_eq!(
        back.runner.withdrawal(),
        None,
        "a withdrawal survived a load, and the original has nowhere to keep one"
    );
    assert_eq!(
        (back.castle_level, back.runner.siege.castle_level),
        (Some(2), 2),
        "the level must be recomputed from the county, not read out of block 125"
    );
    assert!(back.paused, "`DAT_0053F238 = 0xFFFFFFFF`: a load lands paused");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **A body is cleared away, and a spent pot of oil with it** — the corpse
/// state's own count, figure record `+0x173`.
///
/// `docs/battle.md` §14.2: state 2 steps the collapse animation and counts
/// `+0x173` to **80** before freeing the slot; state 15 is the siege engine's
/// twin at 120. Nothing here had a corpse lifetime, so every body stayed on
/// the field for the rest of the battle — and for a pot that is worse than
/// untidy, because `FUN_0047A814` puts a spent pot into state **2** and the
/// state-2 tick is what draws `FUN_0048895E`'s pour frames 42 … 45. A pot that
/// had poured therefore held its pouring picture for ever.
#[test]
fn a_corpse_is_cleared_away_after_eighty_frames() {
    let Some((assets, platform)) = install() else {
        l2_testkit::skip!("no game install, so no Engine.pl8 to find the pot with");
    };
    let engine = Sheet::new(platform.vfs.read("Engine.pl8").expect("Engine.pl8")).expect("a PL8");

    let (mut g, mut m) = staged(
        24,
        &[(Troop::Oil, 1), (Troop::Peasants, 1)],
        &[(Troop::Peasants, 1)],
        |(x, y)| (x as i32 - 7, y as i32 - 7),
    );
    let pot = live(&g)
        .runner
        .fighters
        .iter()
        .position(|f| f.troop == Troop::Oil && f.side == SIDE_A)
        .expect("a pot of oil was raised");

    let mut canvas = Canvas::screen();
    frame(&mut m, &mut g, &assets, &mut canvas);
    let (px, py) = {
        let f = &live(&g).runner.fighters[pot];
        (f.x, f.y)
    };

    let sim_index = live(&g).runner.fighters[pot].sim;
    g.battle.as_mut().unwrap().runner.sim.figures[sim_index].men = 0;

    let mut first_gone = None;
    let mut pour_seen = false;
    for n in 1..=(l2_sim::runner::CORPSE_FRAMES as usize + 40) {
        canvas = Canvas::screen();
        frame(&mut m, &mut g, &assets, &mut canvas);
        let l = live(&g);
        let f = &l.runner.fighters[pot];
        assert_eq!((f.x, f.y), (px, py), "a corpse does not move");
        let index = 0x2A + (f.polar % 8) as usize / 2;
        let sprite = engine.frame(index).expect("the pour frame decodes");
        let (cx, cy) = (f.x as i32 - l.cam.0, f.y as i32 - l.cam.1);
        let w = sprite.width as i32;
        let want = (
            bf::VIEW.x + cx * bf::TILE + (32 / 2 - w / 2),
            bf::VIEW.y + cy * bf::TILE - w / 2 + 8 + 8,
        );
        let there = locate(&canvas, &sprite, want.0..want.0 + 1, want.1..want.1 + 1)
            .contains(&want);
        if there {
            pour_seen = true;
            assert!(first_gone.is_none(), "the pot came back {n} frames after it died");
        } else if first_gone.is_none() && pour_seen {
            first_gone = Some(n);
        }
    }

    assert!(pour_seen, "the dead pot never showed its collapse picture at all");
    let gone = first_gone.expect("the pot is still on the field 120 frames after it died");
    assert!(
        (l2_sim::runner::CORPSE_FRAMES as usize..=l2_sim::runner::CORPSE_FRAMES as usize + 3)
            .contains(&gone),
        "the pot vanished on frame {gone}, not around {}",
        l2_sim::runner::CORPSE_FRAMES
    );
    assert!(live(&g).runner.corpse_gone(pot), "and it stays gone");
}

/// **`Screen_BattleOutcome` (`0x00423241`) reads `g_optAnimations` too**, and
/// what it chooses is the *box*, not a film: off is `FUN_004093E0(0x10, 0x90,
/// 0x1C, 0x0A)`, the short window at (16, 144); on is `FUN_004093E0(0x10,
/// 0x30, 0x1C, 0x16)` with `Ui_DrawInsetRect(0x27, 0x48, 0x192, 0xC2)` inside
/// it — a window that starts at y 48 and a recess for the film. The fourth
/// reader of the flag, beside `CastleBuild_Confirm` (`0x00436B59`),
/// `Msg_DrawWindow` (`0x0047309E`) and `Battle_CheckOutcome` (`0x00477DFC`),
/// and the only one whose answer is pixels.
#[test]
fn the_outcome_box_is_the_tall_one_only_when_animations_are_on() {
    let Some((assets, _p)) = install() else {
        l2_testkit::skip!("no game install, so no window art");
    };
    let band = |c: &Canvas| {
        let mut v = Vec::new();
        for y in 0x30..0x90usize {
            for x in 0x10..(0x10 + 0x1C * 16) as usize {
                v.push(c.at(x, y));
            }
        }
        v
    };
    let shot = |mode: bf::Mode, animations: bool| {
        let (mut g, mut m) =
            staged(24, &[(Troop::Peasants, 8)], &[(Troop::Peasants, 8)], |(x, y)| {
                (x as i32 - 7, y as i32 - 7)
            });
        g.prefs.animations = animations;
        g.battle.as_deref_mut().expect("a live battle").mode = mode;
        let mut canvas = Canvas::screen();
        paint(&mut m, &mut g, &assets, &mut canvas);
        band(&canvas)
    };

    let none = shot(bf::Mode::Field, true);
    assert_eq!(
        shot(bf::Mode::Outcome, false),
        none,
        "the short window painted the tall window's band"
    );
    assert_ne!(shot(bf::Mode::Outcome, true), none, "animations on, and no tall window was drawn");
}


