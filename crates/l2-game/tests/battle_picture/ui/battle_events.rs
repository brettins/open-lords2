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
/// `g_battlePhase` — the original saves mid-battle, because its `.sav` is a
/// memory dump. Ours is a versioned format that does not encode
/// `LiveBattle`, and `crate::save::decode` puts `battle: None` back, so the
/// file would quietly lose the fight. `docs/arms.json`
/// `ours/save-refuses-mid-battle`.
#[test]
fn saving_is_refused_while_a_battle_is_live() {
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
    let titles = {
        let ctx = Ctx { game: &mut g, assets: &assets };
        menubar::titles(&ctx)
    };
    let at = (titles[0].x + titles[0].w / 2, titles[0].y + titles[0].h / 2);
    send(&mut m, &mut g, &assets, Event::Pointer { x: at.0, y: at.1 });
    send(&mut m, &mut g, &assets, Event::Click { x: at.0, y: at.1 });
    let row = menubar::item_rect(&titles, 0, 2);
    let on_row = (row.x + row.w / 2, row.y + row.h / 2);
    send(&mut m, &mut g, &assets, Event::Pointer { x: on_row.0, y: on_row.1 });
    send(&mut m, &mut g, &assets, Event::Click { x: on_row.0, y: on_row.1 });
    assert_eq!(
        m.top_id(),
        Some(ScreenId::SaveLoad(l2_game::screens::saveload::Mode::Save)),
        "File > Save did not open the save box from the battlefield"
    );

    // Type a name and confirm. **`SaveLoad_Tick`'s latch**: Enter arms it and
    // the file is written `WORK_FRAMES` frames later, so the frames have to be
    // run before a directory listing means anything. The name comes out
    // lowercase — `Edit_TypeChar`'s `0x004011B0`, `if ('A' <= ch && ch <= 'Z')
    // ch += 0x20`, which a filename field takes and a text field does not.
    let frames = l2_game::screens::saveload::WORK_FRAMES as u32 + 4;
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

    typed(&mut m, &mut g);
    assert!(
        written().is_empty(),
        "a save was written while a battle was live; it would have lost the battle: {:?}",
        written()
    );
    assert_eq!(
        m.top_id(),
        Some(ScreenId::SaveLoad(l2_game::screens::saveload::Mode::Save)),
        "the box closed on a refusal, so the player was told nothing"
    );

    {
        let mut screen = l2_game::screens::saveload::SaveLoadScreen::new(
            l2_game::screens::saveload::Mode::Save,
        );
        let mut ctx = Ctx { game: &mut g, assets: &assets };
        for c in "MIDFIGHT2".chars() {
            l2_game::Screen::handle(&mut screen, Event::Text(c), &mut ctx);
        }
        let mut t = l2_game::Screen::handle(
            &mut screen,
            Event::KeyDown(l2_game::input::Key::Enter),
            &mut ctx,
        );
        for _ in 0..l2_game::screens::saveload::WORK_FRAMES {
            t = l2_game::Screen::update(&mut screen, &mut ctx);
        }
        assert_eq!(t, l2_game::Transition::Stay, "a refused save leaves the box open");
        assert_eq!(
            screen.status(),
            &l2_game::screens::saveload::Status::Failed(
                l2_game::screens::saveload::BATTLE_REFUSAL.into()
            ),
            "the refusal must say the battle is not saved"
        );
        assert!(
            l2_game::screens::saveload::BATTLE_REFUSAL.contains("NOT SAVED"),
            "and it must say it in words, not in a format name"
        );
    }

    let mut plain = Machine::new(ScreenId::SaveLoad(l2_game::screens::saveload::Mode::Save));
    g.battle = None;
    typed(&mut plain, &mut g);
    assert_eq!(
        written(),
        vec!["midfight.l2sav".to_string()],
        "the refusal fires with no battle live, so it is not testing the battle"
    );
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


