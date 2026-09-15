//! **Emptying the one-shot buffer — the two sites ours did not have.**
//!
//! Reports: *"no VO for the mercenary offer"* and *"no drum roll when I try to
//! attack a country"*, both on builds where the line **was** wired. The buffer
//! is one, `Sound_PlayFile` (`0x00427990`) drops what it cannot fit, and a
//! stale occupant nothing releases eats every bare call after it.
//!
//! `Screen_FrameInput` (`0x0042FF10`) releases it on both ways out of the job
//! popup (screen `0x0F`) — the OK button / right-release arm, and the press on
//! the map — and `Music_StartBattle` (`0x00477B2F`) releases it as the
//! battlefield opens: `Music_Stop(0); Sound_StopOneShot();`. `[V]` on all three
//! statements.
//!
//! The popup is *pushed* here, because the row that opens it is the county
//! strip's and its rects are `screens_map`'s subject; what is driven is the
//! **close**, which is the statement under test.

#![allow(unused_imports)]
use super::*;
use super::routing::*;
use l2_game::audio::{self, Audio, Scene};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;

/// The blacksmith's row: `job + 1 == 8`, the one whose popup puts `fire.wav`
/// in the one-shot buffer (`Panel_JobDetail`, `0x00412B33`).
const FORGE: usize = 7;

/// How the player closes the popup.
#[derive(Clone, Copy)]
enum Close {
    /// `Ui_OkButtonClicked() || g_mouseRightReleased` — `00420000.c:6867`.
    Own,
    /// The press on the map: `if (g_screenId == 0x0f) { Sound_StopOneShot();
    /// FUN_0041438c(); }` — `00420000.c:7686`. Ours is the minimap raster,
    /// which is the part of the map a popup leaves showing.
    Map,
}

/// The forge popup opened over the map and closed again, then the ARMY button
/// with a band waiting. Answers what the buffer was asked for.
fn the_forge_then_the_army_button(
    platform: &l2_mods::Platform,
    band: u8,
    close: Close,
) -> Vec<String> {
    let assets = Assets::placeholder();
    let mut game = world();
    game.selected = 1;
    // `County::mercenary_offer` (`+0x1AD`), what `Mercenary_AdvanceAll`
    // (`0x004ACA2B`) leaves behind.
    game.kingdom.counties[1].mercenary_offer = band;

    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();

    machine.push(ScreenId::Job(1, FORGE));
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
    }
    director.listen(&mut audio, &machine, &game);
    assert!(audio.one_shot_busy(), "the popup did not load fire.wav into the buffer");

    let event = match close {
        Close::Own => Event::RightClick { x: 320, y: 200 },
        Close::Map => {
            let r = l2_view::chrome::minimap_hit_area();
            Event::Click { x: (r.x0 + r.x1) / 2, y: (r.y0 + r.y1) / 2 }
        }
    };
    send(&mut machine, &mut game, &assets, event);
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
    }
    assert_eq!(machine.top_id(), Some(ScreenId::Campaign), "the popup did not close");
    director.listen(&mut audio, &machine, &game);

    let b = l2_game::screens::map::SIDEBAR_BUTTONS[0];
    assert_eq!(b.name, "ARMY", "the first sidebar button is hotspot 1");
    let r = b.rect();
    send(&mut machine, &mut game, &assets, Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 });
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
    }
    assert!(machine.ids().contains(&ScreenId::RaiseArmy(1)), "the ARMY button opened nothing");
    director.listen(&mut audio, &machine, &game);
    audio.heard().iter().map(|s| s.to_string()).collect()
}

/// **The offer is announced after the forge popup, closed either way.**
///
/// Without `Screen_FrameInput`'s stop, `fire.wav` is still the occupant when
/// the sidebar asks for `S016_nn.wav` and the line is dropped — which is the
/// report, on a build where the line is wired.
///
/// **Ablation, run:** delete the `audio.stop_one_shot()` on the job arm of
/// `Director::listen` and both halves go red on `s016_01.wav`.
#[test]
fn the_forge_popup_releases_the_buffer_and_the_mercenary_line_is_heard() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no S016 clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");

    for close in [Close::Own, Close::Map] {
        let heard = the_forge_then_the_army_button(&platform, 1, close);
        assert!(heard.iter().any(|n| n == "fire.wav"), "the popup's own sound; heard {heard:?}");
        assert!(
            heard.iter().any(|n| n == "s016_01.wav"),
            "the Scottish pikemen were not announced; heard {heard:?}",
        );
    }
}

/// **And the drum roll, which is the same buffer.**
///
/// `Battle_ChooseSettlement` (`0x004A6A30`) plays `ff_batl.wav` bare, so the
/// question raised while the forge is still burning is silent.
#[test]
fn the_forge_popup_releases_the_buffer_and_the_battle_fanfare_is_heard() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no ff_batl.wav");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();
    let mut game = world();
    game.selected = 1;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();

    machine.push(ScreenId::Job(1, FORGE));
    director.listen(&mut audio, &machine, &game);
    assert!(audio.one_shot_busy(), "the popup did not load fire.wav into the buffer");
    send(&mut machine, &mut game, &assets, Event::RightClick { x: 320, y: 200 });
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
    }
    director.listen(&mut audio, &machine, &game);

    machine.push(ScreenId::BattlePrompt);
    director.listen(&mut audio, &machine, &game);
    let heard: Vec<String> = audio.heard().iter().map(|s| s.to_string()).collect();
    assert!(heard.iter().any(|n| n == "ff_batl.wav"), "no drum roll; heard {heard:?}");
}

/// **`Music_StartBattle` (`0x00477B2F`) empties the buffer as the field
/// opens**, so a line the campaign screen was still reading does not swallow
/// the battlefield's own first sound.
///
/// Driven through the director, whose `Audio::follow` is the site: the
/// battlefield on the stack is `Scene::Battle`.
///
/// **Ablation, run:** delete the `stop_one_shot` at the head of
/// `Audio::follow` and the cry is dropped, the assertion below goes red, and
/// the bed still starts — the two statements are separate.
#[test]
fn the_battlefield_opening_releases_the_buffer_and_the_bed_starts() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no battle bank");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let game = world();
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    director.listen(&mut audio, &machine, &game);

    // A narration still running when the battle starts — any bare
    // `Sound_PlayFile`; this is the split-army line.
    assert!(audio.play_file(l2_game::audio::names::speech::SPLIT_ARMY, true), "the line started");
    assert!(audio.one_shot_busy(), "the narrator holds the buffer");

    machine.push(ScreenId::Battlefield);
    director.listen(&mut audio, &machine, &game);
    assert!(!audio.one_shot_busy(), "Music_StartBattle's Sound_StopOneShot did not run");
    assert!(audio.music_name().is_some(), "and its Music_Play did not start the bed");

    // Which is what the field's own first sound needs: it is another bare call.
    assert!(audio.play_file("bathit2.wav", false), "the field's first sound was dropped");

    // The field staying open is not a second entry, and neither is a film over
    // it: a cry sounding must not be cut by the next frame.
    director.listen(&mut audio, &machine, &game);
    assert!(audio.one_shot_busy(), "the stop fired again on a frame that started no battle");
}
