//! **The pointer click — when it sounds, and far more often, when it does not.**
//!
//! `Widget_Test` (`0x0040DA1E`) plays `Sound_RestartSlot(1)` — `click3.wav` —
//! from inside the hit test, at exactly two sites `[V]`:
//!
//! ```c
//! if (kind == 4) { if (g_mouseLeftPressed || g_mouseLeftDoubleClick) { Sound_RestartSlot(1); … call handler … }
//!                  if (!g_mouseLeftDown) return 0;  … the auto-repeat: handler, NO sound … }
//! if (kind == 5) { if (!(g_mouseLeftPressed || g_mouseLeftDoubleClick)) return 0;
//!                  Sound_RestartSlot(1); rec[0x0D] = 0x14; return hit;   /* handler fires 20 frames later, silently */ }
//! ```
//!
//! **A test that only checks that a click sounds cannot see the defect worth
//! having a test for**: a spinner that clicks on every auto-repeat pulse is the
//! same *"one reproduced trigger"* in `docs/audio.json` as one that clicks once,
//! and it is thirty-three clicks a second in a player's ear. So most of this
//! file is the silent cases:
//!
//! | gesture | original | asserted here |
//! |---|---|---|
//! | kind 4 press, and its double click | click | loud |
//! | kind 4 auto-repeat pulses | silent | `the_spinner_clicks_once_however_long_it_is_held` |
//! | kind 5 press | click | loud |
//! | kind 5 delayed fire, twenty frames on | silent | `a_gauntlet_clicks_on_the_press_and_not_when_it_acts` |
//! | kind 2 (`Hotspot_Test`) and its flat pulse | silent | `the_hotspot_kinds_are_silent` |
//! | kinds 1 and 3 (`Hotspot_Test`) | silent | `the_hotspot_kinds_are_silent`, and the sidebar through the machine |
//! | `Ui_OkButtonClicked` (`0x0040E7E4`) | silent | `the_ok_buttons_are_silent` |
//!
//! These drive [`Machine::handle`] and [`Machine::update`] and read
//! [`Machine::clicks`], which is the wire `audio::Director` listens to. That the
//! wire reaches a speaker is `tests/audio_wiring.rs`'s
//! `a_widget_press_is_heard_once_and_a_hotspot_press_is_not`, which needs the
//! install; nothing here does.
//!
//! # The ablations
//!
//! Each test names the change that turns it red, and each was run.

use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::message;
use l2_game::press::{self, Kind, Press, Widget};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::Game;

fn world() -> (Game, Assets) {
    let mut g = Game::new(4);
    g.kingdom.set_county_count(2);
    for id in 1..=2usize {
        let c = &mut g.kingdom.counties[id];
        c.owner = 1;
        c.population = 1_000;
        c.happiness = 90;
        c.grain = 5_000;
        c.herd = 100;
    }
    g.kingdom.realms[1].in_play = true;
    g.kingdom.realms[1].is_human = true;
    g.player = 1;
    g.selected = 1;
    (g, Assets::placeholder())
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn centre(r: Rect) -> (i32, i32) {
    (r.x + r.w / 2, r.y + r.h / 2)
}

/// **The spinner clicks once, however long it is held.**
///
/// The tax arrow is `g_taxWidgets` kind 4. The press clicks; the hold keeps
/// stepping the tax on `REPEAT_GATE`'s ramp and every one of those steps goes
/// through `Widget_Test`'s hold branch, which has no `Sound_RestartSlot` in it.
///
/// The repeat is asserted to have *happened* before its silence is asserted,
/// because a hold that never repeated would pass the silence for free.
///
/// **Ablations, run:** delete `self.click()` from `Press::press` and the first
/// assertion goes red; add a `self.click()` on the fire path of `Press::tick`
/// and the last one does.
#[test]
fn the_spinner_clicks_once_however_long_it_is_held() {
    let (mut g, a) = world();
    let mut m = Machine::new(ScreenId::County(1, Panel::Tax));
    let up = centre(Panel::Tax.increase_button().expect("the tax panel has an up arrow"));
    assert_eq!(m.clicks(), 0);

    send(&mut m, &mut g, &a, Event::Click { x: up.0, y: up.1 });
    assert_eq!(m.clicks(), 1, "the press is Widget_Test's kind-4 Sound_RestartSlot(1)");

    let mut steps = 0;
    for _ in 0..120 {
        let before = g.kingdom.counties[1].tax_rate;
        tick(&mut m, &mut g, &a);
        if g.kingdom.counties[1].tax_rate != before {
            steps += 1;
        }
    }
    assert!(steps >= 4, "the hold must have repeated for its silence to mean anything: {steps}");
    assert_eq!(m.clicks(), 1, "{steps} auto-repeat pulses, and the original plays nothing on any of them");
    // And letting go does not smuggle one out. A click counted on the hold but
    // not carried by the tick would be drained by the next event — so this is
    // the half that sees it wherever it was counted.
    send(&mut m, &mut g, &a, Event::Release { x: up.0, y: up.1 });
    tick(&mut m, &mut g, &a);
    assert_eq!(m.clicks(), 1, "the release produced a click the hold had been holding");
}

/// **A double click is a press for kind 4, so it clicks.** Windows sends
/// `WM_LBUTTONDBLCLK` *instead of* the second `WM_LBUTTONDOWN`, and
/// `Widget_Test` tests `g_mouseLeftPressed || g_mouseLeftDoubleClick` in front of
/// the sound — so a fast double click on a spinner is two clicks and two steps.
#[test]
fn a_double_click_on_a_spinner_is_a_second_click() {
    let (mut g, a) = world();
    let mut m = Machine::new(ScreenId::County(1, Panel::Tax));
    let up = centre(Panel::Tax.increase_button().unwrap());
    send(&mut m, &mut g, &a, Event::Click { x: up.0, y: up.1 });
    send(&mut m, &mut g, &a, Event::Release { x: up.0, y: up.1 });
    send(&mut m, &mut g, &a, Event::DoubleClick { x: up.0, y: up.1 });
    assert_eq!(m.clicks(), 2);
}

/// **A press that closes its own screen is still heard.**
///
/// The message scroll's prompts are kind-4 widgets whose handler dismisses the
/// window, so the screen that counted the click is popped *by the same event*.
/// `Widget_Test` plays the sound before it calls the handler; ours reproduces
/// that order by draining the count in `Machine::handle` before the transition
/// is applied. A per-screen count summed over the live stack would lose exactly
/// these presses — the ones that change what the player is looking at.
///
/// **Ablation, run:** take the count after `apply_at` instead of before it and
/// this goes red — the screen is gone and its count with it.
#[test]
fn a_press_that_closes_its_own_screen_is_still_counted() {
    let (mut g, a) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    let mut rec = message::Record::default();
    rec.group = 247;
    rec.category = message::category::PAY_PROMPT;
    rec.to = g.player;
    assert!(g.messages.enqueue(rec, g.player));
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Message), "the pump opened the prompt");

    let [_, no] = message::Prompt::PayForHelp.widgets();
    let side = message::Prompt::SIDE;
    send(&mut m, &mut g, &a, Event::Click { x: no.0 + side / 2, y: no.1 + side / 2 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "declining closed the window");
    assert_eq!(m.clicks(), 1, "the thumb that closed the window still clicked");
}

/// **A gauntlet clicks on the press and not when it acts.**
///
/// Kind 5's `Sound_RestartSlot(1)` is on the press; the handler runs out of the
/// countdown loop twenty frames later and that loop plays nothing. Asserted on
/// `Press` directly and through both of its entry points, because the
/// diplomacy screen's six verb buttons call `Press::press_delayed` by hand
/// rather than through `Press::event`.
///
/// **Ablation, run:** delete `self.click()` from `Press::press_delayed` and the
/// two loud assertions go red; add one on `Press::tick`'s delayed fire and the
/// silent one does.
#[test]
fn a_gauntlet_clicks_on_the_press_and_not_when_it_acts() {
    let table = [Widget::new(Rect::new(0, 0, 32, 32), Kind::Delayed)];
    let mut p = Press::new();
    assert_eq!(p.event(&table, Event::Click { x: 4, y: 4 }), None, "kind 5 does not act on the press");
    assert_eq!(p.take_clicks(), 1, "and it clicks on it");

    let mut fired = None;
    for _ in 0..press::DELAYED_FRAMES {
        fired = fired.or(p.tick());
    }
    assert_eq!(fired, Some(0), "the handler ran twenty ticks on");
    assert_eq!(p.take_clicks(), 0, "and nothing sounded when it did");

    // The hand-rolled entry point, as `DiplomacyScreen::handle` uses it.
    p.press_delayed(0);
    assert_eq!(p.take_clicks(), 1);
}

/// **`Hotspot_Test`'s three kinds are silent** — the majority of the interface.
///
/// `Hotspot_Test` (`0x0040E3EE`) has no `Sound_RestartSlot` anywhere in it,
/// and `Widget_Test`'s own kind 2 arm is the toggle, which also plays nothing.
/// Driven through the kinds' own gestures so that each one's handler provably
/// ran: a silence that never fired would prove nothing.
///
/// **Ablation, run:** make the `Kind::Press` arm of `Press::event` call the
/// click and the first assertion goes red.
#[test]
fn the_hotspot_kinds_are_silent() {
    let r = Rect::new(0, 0, 32, 32);

    let mut p = Press::new();
    assert_eq!(p.event(&[Widget::new(r, Kind::Press)], Event::Click { x: 4, y: 4 }), Some(0));
    assert_eq!(p.take_clicks(), 0, "kind 1 fired and was silent");

    let mut p = Press::new();
    let release = [Widget::new(r, Kind::Release)];
    assert_eq!(p.event(&release, Event::Click { x: 4, y: 4 }), None);
    assert_eq!(p.event(&release, Event::Release { x: 4, y: 4 }), Some(0));
    assert_eq!(p.take_clicks(), 0, "kind 3 fired and was silent");

    let mut p = Press::new();
    assert_eq!(p.event(&[Widget::new(r, Kind::Held)], Event::Click { x: 4, y: 4 }), Some(0));
    let pulses = (0..200).filter(|_| p.tick().is_some()).count();
    assert!(pulses >= 3, "the held pulse must have run: {pulses}");
    assert_eq!(p.take_clicks(), 0, "kind 2 fired {pulses} more times and was silent throughout");
}

/// **The campaign sidebar is `Hotspot_Test` kind 1, and opening a screen from it
/// is silent.** Through the machine, because the sidebar is not a [`Press`] at
/// all — which is the point: a hotspot cannot click because it has no path to
/// the only thing that counts.
///
/// **This one cannot be ablated from `press.rs`**, and that is a finding about
/// the shape rather than a gap in the test: `MapScreen` owns no `Press`, so no
/// edit to the click's rule reaches it. What would turn it red is a screen
/// answering its hotspots through a `Press` table with the wrong kind — which
/// is the defect it exists to catch.
#[test]
fn the_sidebar_opens_screens_without_a_sound() {
    let mut opened = 0;
    for b in map::SIDEBAR_BUTTONS {
        let (mut g, a) = world();
        let mut m = Machine::new(ScreenId::Campaign);
        let (x, y) = centre(b.rect());
        send(&mut m, &mut g, &a, Event::Click { x, y });
        if m.top_id() != Some(ScreenId::Campaign) {
            opened += 1;
        }
        assert_eq!(m.clicks(), 0, "{} is a hotspot and plays nothing", b.name);
    }
    assert!(opened >= 3, "the sidebar must actually have opened screens: {opened} of 5");
}

/// **`Ui_OkButtonClicked` is silent, and so is the message scroll's corner.**
///
/// The county panel's corner closes on the release (`Ui_OkButtonClicked`,
/// `0x0040E7E4`, 26 call sites, no sound); the message scroll's corner closes
/// on the press through `Msg_HandleInput`'s own 48 × 48 test, which is not a
/// widget either. Both are asserted to have closed their window.
#[test]
fn the_ok_buttons_are_silent() {
    let (mut g, a) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::County(1, Panel::Tax));
    let ok = centre(Panel::Tax.ok_button_for(g.kingdom.options.armies_eat));
    send(&mut m, &mut g, &a, Event::Click { x: ok.0, y: ok.1 });
    send(&mut m, &mut g, &a, Event::Release { x: ok.0, y: ok.1 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the corner closed the panel");
    assert_eq!(m.clicks(), 0, "Ui_OkButtonClicked plays nothing");

    let (mut g, a) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    let mut rec = message::Record::default();
    rec.group = 130;
    rec.category = message::category::NOTICE;
    rec.to = g.player;
    assert!(g.messages.enqueue(rec, g.player));
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Message));
    let hit = message::frame_of(&rec).expect("a notice has a frame").ok_hitbox();
    let (x, y) = centre(hit);
    send(&mut m, &mut g, &a, Event::Click { x, y });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the corner dismissed the notice");
    assert_eq!(m.clicks(), 0, "Msg_HandleInput's corner plays nothing");
}
