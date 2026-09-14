#![allow(unused_imports)]
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

// third colour and no skip, and not one of the sixteen handlers reads
// `g_battlePhase`.

/// A string in one of the **original's** fonts, found by its set pixels. The
/// bar is drawn in `Fntl2_14.pl8`, which our own 5 × 7 probe cannot see.
fn find_font_text(
    canvas: &Canvas,
    font: &l2_game::shell::font::Font,
    s: &str,
    colour: u8,
) -> Option<(i32, i32)> {
    let style = l2_game::shell::font::Style { colour: 1, shadow: None, caps: None };
    let w = font.width(s).max(1);
    let h = font.height(s).max(1);
    let mut probe = Canvas::new(w as usize, h as usize);
    font.draw(&mut probe, 0, 0, s, &style);
    let mut wanted: Vec<(i32, i32)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            if probe.at(x as usize, y as usize) == 1 {
                wanted.push((x, y));
            }
        }
    }
    if wanted.is_empty() {
        return None;
    }
    for oy in 0..(canvas.height as i32 - h) {
        for ox in 0..(canvas.width as i32 - w) {
            let (fx, fy) = wanted[0];
            if canvas.at((ox + fx) as usize, (oy + fy) as usize) != colour {
                continue;
            }
            if wanted.iter().all(|(x, y)| canvas.at((ox + x) as usize, (oy + y) as usize) == colour)
            {
                return Some((ox, oy));
            }
        }
    }
    None
}

/// A battle with twenty swordsmen a side, staged the way the tests above do.
fn skirmish() -> (Game, Machine) {
    staged(30, &[(Troop::Swordsmen, 20)], &[(Troop::Swordsmen, 20)], |(x, y)| {
        (x as i32 - 7, y as i32 - 7)
    })
}

/// **The bar's three titles are on the battlefield, and they answer.**
///
/// The probe is `Menu_HitTitle`'s own geometry through the install's own font
/// the arm hit-tests. The near-miss is the 32 pixels `g_penAdvance += 0x20`
/// leaves after a title, which `Menu_HitTitle` does **not** answer —
/// that passed by clicking anywhere in the band fails here.
///
/// Ablation: drop the `menubar::title_at` guard from the `0x29` ladder — red on
/// the push; drop the `draw_menu_bar` call — red on the caption.
#[test]
fn the_menu_bar_is_live_on_the_battlefield() {
    let Some((assets, _p)) = install() else {
        l2_testkit::skip!("no game install, so the bar has no words and no font");
    };
    let (mut g, mut m) = skirmish();
    let titles = {
        let ctx = Ctx { game: &mut g, assets: &assets };
        menubar::titles(&ctx)
    };
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8 loaded");

    // --- painted ------------------------------------------------------------
    let mut canvas = Canvas::screen();
    frame(&mut m, &mut g, &assets, &mut canvas);
    for i in 0..3 {
        let caption = {
            let ctx = Ctx { game: &mut g, assets: &assets };
            menubar::title_text(&ctx, i)
        };
        assert!(
            find_font_text(&canvas, body, &caption, l2_game::shell::font::TEXT).is_some(),
            "{caption:?} is not on the battlefield's menu bar"
        );
    }

    // --- and they open ------------------------------------------------------
    //
    // `Menu_OpenDropdown` writes `g_screenId = 0x32`, which is a push here.
    let at = (titles[0].x + titles[0].w / 2, titles[0].y + titles[0].h / 2);
    send(&mut m, &mut g, &assets, Event::Pointer { x: at.0, y: at.1 });
    send(&mut m, &mut g, &assets, Event::Click { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), Some(ScreenId::MenuBar(0)), "File did not open over the battle");

    // The near-miss: the dead 32 pixels after the last title. `Menu_HitTitle`
    // walks three records, returns 0, and the ladder falls through.
    let (mut g2, mut m2) = skirmish();
    let gap = (titles[2].x + titles[2].w + 16, titles[2].y + titles[2].h / 2);
    send(&mut m2, &mut g2, &assets, Event::Pointer { x: gap.0, y: gap.1 });
    send(&mut m2, &mut g2, &assets, Event::Click { x: gap.0, y: gap.1 });
    assert_eq!(m2.top_id(), Some(ScreenId::Battlefield), "the gap between titles opened a menu");
}

/// **What a battle takes off the bar is the shields and the date, and nothing
/// else** — `Screen_DrawMenuBar`'s two `if (g_battlePhase == 0)` guards, neither
/// of which is on a title or on the treasury.
///
/// Asserted as the difference between the same painter's two callers rather
/// than against a coordinate: the season must be absent and the gold present. A
/// guard written the wrong way round fails one half or the other.
#[test]
fn a_battle_takes_the_date_off_the_bar_and_leaves_the_treasury() {
    let Some((assets, _p)) = install() else {
        l2_testkit::skip!("no game install, so the bar has no words and no font");
    };
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8 loaded");
    let (mut g, mut m) = skirmish();
    g.kingdom.realms[1].gold = 1234;
    // **Group 29 is 1-based**: index 0 is *"No Season"*, which is what a
    // default-constructed kingdom reads and which would make the probe a
// fallback string
    g.kingdom.season = l2_kingdom::tables::Season::Summer as u8;
    let season = l2_game::screens::map::season_text(&assets, g.kingdom.season);
    assert_eq!(season, "Summer", "the probe must be L2.eng group 29's own word");

    let mut canvas = Canvas::screen();
    frame(&mut m, &mut g, &assets, &mut canvas);
    assert!(
        find_font_text(&canvas, body, &season, l2_game::shell::font::TEXT).is_none(),
        "{season:?} is on the bar during a battle; g_battlePhase guards it"
    );
    assert!(
        find_font_text(&canvas, body, "1234", l2_game::shell::font::TEXT).is_some(),
        "the treasury is not on the bar during a battle; it is outside both guards"
    );
}

/// **The battle goes on under an open menu** — `Battle_Frame`'s
/// `else if ((g_battlePhase == 2) && ticksDue)` arm, which has no `g_screenId`
/// test any more than the turn's arm above it does.
///
/// The drop-down is a screen, so opening *File* puts a screen over the
/// battlefield, and `Screen::update` is the top screen's alone. Measured on the
/// simulation's own `BattleRunner::tick`
///
/// Ablation: delete the `wind_battle` call from `Machine::wind_turn` — the
/// second count comes out zero and this goes red.
#[test]
fn a_battle_runs_on_under_an_open_menu() {
    let Some((assets, _p)) = install() else {
        l2_testkit::skip!("no game install");
    };
    let (mut g, mut m) = skirmish();
    let titles = {
        let ctx = Ctx { game: &mut g, assets: &assets };
        menubar::titles(&ctx)
    };

    // Ten frames with nothing over the battlefield, for the reference.
    let mut canvas = Canvas::screen();
    let before = live(&g).runner.tick;
    for _ in 0..10 {
        frame(&mut m, &mut g, &assets, &mut canvas);
    }
    let plain = live(&g).runner.tick - before;
    assert!(plain > 0, "the battle did not tick at all with nothing over it");

    // Open File, and run ten more.
    let at = (titles[0].x + titles[0].w / 2, titles[0].y + titles[0].h / 2);
    send(&mut m, &mut g, &assets, Event::Pointer { x: at.0, y: at.1 });
    send(&mut m, &mut g, &assets, Event::Click { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), Some(ScreenId::MenuBar(0)), "File did not open");
    let under = live(&g).runner.tick;
    for _ in 0..10 {
        frame(&mut m, &mut g, &assets, &mut canvas);
    }
    assert_eq!(
        live(&g).runner.tick - under,
        plain,
        "the battle froze under the open menu; Battle_Frame has no g_screenId test"
    );
}

/// **The battle screen's right column is 160 pixels wide and the campaign map's
/// is 162.**
///
/// A player asked: *"The sidebar iirc was much smaller in combat? or something?
/// I might be misremembering."* Measured, and the two columns differ by **two
/// pixels**. What differs is what is *in* them: `CountyStrip_Draw`
/// (`0x0040F7D3`) and the campaign minimap (`FUN_00410F4B`) both return at once
/// when `g_battlePhase != 0`, so the strip, the produce rows, the labour slider
/// and the five sidebar buttons are off this screen, and an overview, a banner
/// grid and five battle buttons are on it instead.
///
/// Every number is a literal from a hit test or a blit, never an expression of
/// the constant under test:
///
/// * `Hotspot_Test(0x1E0, 0x1C0, &DAT_004DC710, 5)` — `Battle_ButtonClicked`;
/// * `BattleMap_Click` rejects `g_mouseX < 0x1E0 || 0x27F < g_mouseX` and
///   `g_mouseY < 0x18 || 0xB7 < g_mouseY`;
/// * `FUN_004BC020(…, 0, 0x18, 0xF, 0xE, 0x20)` — the field, 15 × 14 tiles of
///   32 from (0, 24), whose right edge is therefore 480;
/// * `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)` — the campaign map's,
///   two pixels to the left of the battle's.
#[test]
fn the_battle_column_is_two_pixels_narrower_than_the_campaign_one() {
    const SCREEN_W: i32 = 640;
    // `BattleMap_Click`'s own bounds, written out.
    assert_eq!((bf::OVERVIEW.x, bf::OVERVIEW.y), (0x1E0, 0x18));
    assert_eq!(bf::OVERVIEW.x + bf::OVERVIEW.w - 1, 0x27F);
    assert_eq!(bf::OVERVIEW.y + bf::OVERVIEW.h - 1, 0xB7);
    // `Hotspot_Test(0x1E0, 0x1C0, …, 5)` — the five buttons fill the column.
    assert_eq!(bf::BUTTON_ORIGIN, (0x1E0, 0x1C0));
    assert_eq!(bf::BUTTON_SIZE * bf::BUTTON_COUNT as i32, SCREEN_W - 0x1E0);
    // The field stops exactly where the column starts.
    assert_eq!(bf::VIEW.x + bf::VIEW.w, 0x1E0);
    assert_eq!((bf::VIEW.x, bf::VIEW.y, bf::VIEW.h), (0, 0x18, 14 * 32));

    // And the campaign map's column, which is the thing being compared with.
    assert_eq!(
        l2_view::campaign::PANEL_X,
        0x1DE,
        "Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)"
    );
    assert_eq!(
        (SCREEN_W - 0x1E0, SCREEN_W - l2_view::campaign::PANEL_X),
        (160, 162),
        "the battle column is 160 wide and the campaign column 162"
    );
}

/// **A save is refused while a battle is live, and it is refused out loud.**
///
/// `Menu_SaveGame` (`0x00433F49`) is reachable from `0x29` and does not test
/// `g_battlePhase` — the original saves mid-battle, because its `.sav` is a
/// memory dump. Ours is a versioned format that does not encode
/// `LiveBattle`, and `crate::save::decode` puts `battle: None` back, so the
/// file would quietly lose the fight. `docs/arms.json`
/// `ours/save-refuses-mid-battle`.
///
/// The discrimination is the same screen with the battle taken away: it must
/// save. A refusal that fired on every save would pass the first half alone.
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
    // File > Save: the bar, then row 2 of `g_menuBarItems`' first item table.
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

    // **And what it says.** Group 40 index 4 is the heading the painter draws;
    // the detail under it has to name the thing being lost, so a player who
    // reads it knows the battle is not in the file. The same refusal, reached
    // on a screen the test holds, because `Machine` hands out no screen.
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

    // **The discrimination**: the same screen and the same keystrokes with no
    // battle live. A refusal that fired on every save would pass the half above
    // on its own.
    //
// A fresh machine under the open box,
    // because a battlefield that loses its battle answers `Transition::Pop`,
    // and a pop from *under* another screen truncates the stack — which is the
    // original's single `g_screenId` byte and would take the save box with it.
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
///
/// The pot is killed through the simulation — `men = 0`, which is what
/// `Melee_Tick` leaves — so the runner's own death arm starts the count.
///
/// Ablation: return `false` from `BattleRunner::corpse_gone` — red, *"the pot
/// is still on the field 120 frames after it died"*. Return
/// `ENGINE_CORPSE_FRAMES` from `Fighter::corpse_frames`, which is state 15's
/// count and not the pot's — red at the same line.
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

    // `Melee_Tick`'s end of it: the men are gone, and the runner's own death
    // arm moves the figure into the corpse state on the next step.
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
        // `(polarDirc >> 1) + 0x2A` — the picture state 2 draws a pot with.
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
    // The count starts on the tick the death arm sees, one frame after the men
    // were zeroed, and the picture goes with the frame that reaches the bound.
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
///
/// The probe is the band between the two windows' tops, y 48 … 144, which the
/// short window never reaches. Compared against the same frame with no banner
/// at all, so a band the field paints by itself cannot pass either half.
///
/// Ablation: drop `ctx.game.prefs.animations` from `animated` — red, *"the
/// short window painted the tall window's band"*.
#[test]
fn the_outcome_box_is_the_tall_one_only_when_animations_are_on() {
    let Some((assets, _p)) = install() else {
        l2_testkit::skip!("no game install, so no window art");
    };
    // y 0x30 … 0x90: the tall window's own height, above the short one's top.
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

