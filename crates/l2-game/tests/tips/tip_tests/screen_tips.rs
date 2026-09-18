#![allow(unused_imports)]
use super::*;
use super::input_and_layout::*;
use super::game_events::*;
use super::audio_and_narration::*;
use super::*;
use std::collections::BTreeMap;
use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, Frame};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::options::{self, Page, Setting};
use l2_game::tip::{self, Tips, View};
use l2_game::Game;
use l2_kingdom::units_tick::Incursion;

/// Ablation: swap any two groups in `tip::update`'s chain, or its `job == 8`,
/// and the row naming it goes red.
#[test]
fn each_screen_the_ladder_names_gets_its_own_tip() {
    for (screen, job, want) in [
        (0x17, 0, 209), // raise army: "The Armoury:"
        (0x0F, 8, 208), // the job popup on job 8: "The Blacksmith:"
        (0x02, 0, 207), // the village: "The Town Center:"
        (0x39, 0, 218), // advanced options: "Advanced Game Options:"
        (0x1B, 0, 217), // castle building: "Castle Building:"
        (0x10, 0, 210), // move-order mode: "Army Movement:"
    ] {
        let mut t = armed();
        assert_eq!(
            tip::update(&mut t, &View { job, ..view(screen) }),
            Some(want),
            "screen {screen:#04x}"
        );
    }
    let mut t = armed();
    assert_eq!(tip::update(&mut t, &View { job: 7, ..view(0x0F) }), None, "job 7 is not the smithy");
    for screen in [0x04, 0x0A, 0x14, 0x27, 0x31, 0x42] {
        let mut t = armed();
        assert_eq!(tip::update(&mut t, &view(screen)), None, "screen {screen:#04x} has no tip");
    }

    let mut t = armed();
    assert_eq!(tip::update(&mut t, &View { zoom_far: true, ..view(0x00) }), Some(206));
    let mut t = armed();
    assert_eq!(tip::update(&mut t, &view(0x00)), Some(200));
    let mut t = armed();
    assert_eq!(tip::update(&mut t, &View { enabled: false, ..view(0x02) }), None);
    assert_eq!(tip::update(&mut t, &View { in_play: false, ..view(0x02) }), None);
}

/// Ablation: delete `ScreenId::RaiseArmy(_) => Some(0x17)` in
/// `tip::screen_byte` and the raise-army row goes red.
#[test]
fn our_screens_read_as_the_originals_screen_bytes() {
    let g = world();
    for (id, byte) in [
        (ScreenId::Village(1), 0x02),
        (ScreenId::Job(1, 7), 0x0F),
        (ScreenId::RaiseArmy(1), 0x17),
        (ScreenId::Castle(1), 0x1B),
        (ScreenId::Options(Page::Advanced), 0x39),
    ] {
        let mut m = Machine::new(ScreenId::Campaign);
        m.push(id);
        let v = View::of(&m, &g);
        assert_eq!(v.screen, Some(byte), "{id:?}");
        assert!(v.in_play, "the campaign is on the stack");
    }
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Job(1, 7));
    assert_eq!(View::of(&m, &g).job, 8);
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Message);
    assert_eq!(View::of(&m, &g).screen, Some(0x00));
    let m = Machine::new(ScreenId::Setup(l2_game::screens::setup::SetupPage::Title));
    assert!(!View::of(&m, &g).in_play);
}

/// **Three tips on the campaign map, twenty frames apart.** Through the real
/// machine: the ladder, `Tip_Show`'s screen `0x27`, the pump pulling on it, a
/// right click, and `FUN_00476E21`'s re-arm.
#[test]
fn a_new_game_meets_three_tips_twenty_frames_apart() {
    let (mut g, a, mut m) = campaign();
    assert_eq!(
        ticks_until_posted(&mut m, &mut g, &a, 100),
        Some(21),
        "FUN_00476A5D arms twenty frames at start-up, so the first tip is on the twenty-first"
    );
    assert!(m.ids().contains(&ScreenId::Tip), "Tip_Show writes g_screenId = 0x27: {:?}", m.ids());
    assert_eq!(m.top_id(), Some(ScreenId::Message), "Msg_Pump runs on 0x27 in the same frame");
    let r = *g.messages.open().expect("the tip window is open");
    assert_eq!(
        (r.group, r.category, r.to, r.from, r.variant),
        (200, 7, 1, 0, 0),
        "Msg_Enqueue(0, g_localPlayer, 200, 0, g_tipCategory[200], …)"
    );

    for want in [201u16, 202] {
        send(&mut m, &mut g, &a, right_click());
        assert!(!g.tips.hosting(), "Msg_Dismiss reaches FUN_00476E21");
        assert_eq!(
            ticks_until_posted(&mut m, &mut g, &a, 100),
            Some(21),
            "twenty quiet frames after the dismissal, then {want}"
        );
        assert_eq!(open_group(&g), Some(want));
    }

    send(&mut m, &mut g, &a, right_click());
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 200), None, "the campaign map has three");
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "screen 0x27 went with the last of them");
}

#[test]
fn a_tip_holds_its_screens_input_and_gives_it_back_when_dismissed() {
    let (mut g, a, mut m) = campaign();
    m.push(ScreenId::Options(Page::Advanced));
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 100), Some(21));
    assert_eq!(open_group(&g), Some(218), "\"Advanced Game Options:\"");
    assert_eq!(
        m.ids(),
        vec![
            ScreenId::Campaign,
            ScreenId::Options(Page::Advanced),
            ScreenId::Tip,
            ScreenId::Message
        ]
    );

    let farming = g.kingdom.options.advanced_farming;
    send(&mut m, &mut g, &a, Event::Click { x: 280 + 8, y: 156 + 8 });
    assert_eq!(g.kingdom.options.advanced_farming, farming, "g_screenId is 0x27, not 0x39");
    assert!(g.messages.is_open());

    let r = *g.messages.open().unwrap();
    let ok = {
        let ctx = Ctx { game: &mut g, assets: &a };
        l2_game::screens::message::window_frame(&ctx, &r).expect("a tip window has a frame").ok_button()
    };
    send(&mut m, &mut g, &a, Event::Click { x: ok.0 + 4, y: ok.1 + 4 });
    assert!(!g.messages.is_open(), "the left button closes a tip window");
    tick(&mut m, &mut g, &a);
    assert_eq!(m.ids(), vec![ScreenId::Campaign, ScreenId::Options(Page::Advanced)]);
}

