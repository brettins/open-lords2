#![allow(unused_imports)]
use super::*;
use super::screens::*;
use l2_game::audio::{self, names, Audio};
use l2_game::game::Assets;
use l2_game::screen::{Machine, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::info::Target;
use l2_game::screens::setup::SetupPage;
use l2_game::input::{Event, Key};
use l2_game::screen::Ctx;
use l2_game::Game;

pub(crate) fn drain(audio: &mut Audio, name: &str) {
    let mut buf = vec![0.0f32; 2 * 4096];
    for _ in 0..2_000 {
        if !audio.is_playing(name) {
            return;
        }
        audio.mix(&mut buf);
    }
    panic!("{name} never finished");
}

/// **A line or a fanfare asked for while the one-shot buffer sounds is
/// dropped, and one asked for once it has finished plays.** `Sound_PlayFile`
/// opens with `if (Sound_OneShotBusy()) return 0;`, and `Map_ZoomOut` and
/// `Battle_ChooseSettlement` call it with nothing in front of it, so the
/// narrator's zoom-out line and the battle fanfare wait for nobody: asked for
/// over another clip, they are not played at all. `[V]`
#[test]
fn a_line_or_a_fanfare_over_the_one_shot_buffer_is_dropped_and_after_it_plays() {
    let Some(mut audio) = headless() else { l2_testkit::skip!("no game install") };
    let mut game = world();
    game.kingdom.counties[1].ration_achieved = 3;
    game.kingdom.counties[1].herd = 400;
    game.kingdom.counties[1].herd_eaten = 0;
    game.kingdom.counties[1].grain_eaten = 0;
    game.select(1);
    let stack = |over: &[ScreenId]| {
        let mut m = Machine::new(APP_ROOT);
        m.push(ScreenId::Campaign);
        for id in over {
            m.push(*id);
        }
        m
    };
    let ration = ScreenId::County(1, Panel::Ration);
    let prompt = ScreenId::BattlePrompt;
    let mut director = audio::Director::new();
    listen(&mut director, &mut audio, &stack(&[]), &game);

    listen(&mut director, &mut audio, &stack(&[ration]), &game);
    assert!(
        audio.is_playing(names::speech::RATION_ON_DAIRY),
        "the ration line did not start, so nothing below is tested - heard {:?}",
        audio.heard()
    );
    assert!(audio.one_shot_busy());

    game.map_zoom_far = true;
    listen(&mut director, &mut audio, &stack(&[ration]), &game);
    assert!(!audio.heard().contains(&"s033_02.wav"), "Map_ZoomOut's line played over the ration line");
    assert!(audio.one_shot_busy(), "the dropped zoom-out line took the buffer");

    listen(&mut director, &mut audio, &stack(&[ration, prompt]), &game);
    assert!(!audio.heard().contains(&"ff_batl.wav"), "the battle fanfare played over the ration line");
    assert!(audio.one_shot_busy(), "the dropped fanfare took the buffer");

    drain(&mut audio, names::speech::RATION_ON_DAIRY);
    assert!(!audio.one_shot_busy());

    game.map_zoom_far = false;
    listen(&mut director, &mut audio, &stack(&[ration]), &game);
    game.map_zoom_far = true;
    listen(&mut director, &mut audio, &stack(&[ration]), &game);
    assert!(audio.heard().contains(&"s033_02.wav"), "heard {:?}", audio.heard());
    assert!(audio.one_shot_busy(), "the zoom-out line holds the buffer");

    drain(&mut audio, names::speech::ZOOM_OUT);
    listen(&mut director, &mut audio, &stack(&[ration, prompt]), &game);
    assert!(audio.heard().contains(&"ff_batl.wav"), "heard {:?}", audio.heard());
    assert!(audio.one_shot_busy(), "and the fanfare holds it in turn");
}

