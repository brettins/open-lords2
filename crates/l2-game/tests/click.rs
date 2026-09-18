//! `Widget_Test` (`0x0040DA1E`) plays `Sound_RestartSlot(1)` — `click3.wav` —
//! from inside the hit test, at exactly two sites `[V]`:
//!
//! | gesture | original | asserted here |
//! |---|---|---|
//! | kind 4 press, and its double click | click | loud |
//! | kind 4 auto-repeat pulses | silent | `the_spinner_clicks_once_however_long_it_is_held` |
//! | kind 5 press | click | loud |
//! | kind 5 delayed fire, twenty frames on | silent | `a_gauntlet_clicks_on_the_press_and_not_when_it_acts` |
//! | kind 2 (`Hotspot_Test`) and its flat pulse | silent | `the_hotspot_kinds_are_silent` |
//! | kinds 1 and 3 (`Hotspot_Test`) | silent | `the_hotspot_kinds_are_silent`
//! | `Ui_OkButtonClicked` (`0x0040E7E4`) | silent | `the_ok_buttons_are_silent` |
//!
//! # The ablations

use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::message;
use l2_game::press::{self, Kind, Press, Widget};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::Game;

pub(crate) fn world() -> (Game, Assets) {
    let mut g = Game::new(4);
    g.kingdom.set_county_count(2);
    l2_testkit::chain_neighbours!(g.kingdom);
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
    send(&mut m, &mut g, &a, Event::Release { x: up.0, y: up.1 });
    tick(&mut m, &mut g, &a);
    assert_eq!(m.clicks(), 1, "the release produced a click the hold had been holding");
}

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

#[test]
fn a_gauntlet_clicks_on_the_press_and_not_when_it_acts() {
    let table = [Widget::new(Rect::new(0, 0, 32, 32), Kind::Delayed)];
    let mut p = Press::new();
    assert_eq!(p.event(&table, Event::Click { x: 4, y: 4 }), None, "kind 5 does not act on the press");
    assert_eq!(p.take_clicks(), 1, "and it clicks on it");

    let mut fired = None;
    for _ in 0..press::DELAYED_FRAMES {
        fired = fired.or(p.tick().next());
    }
    assert_eq!(fired, Some(0), "the handler ran twenty ticks on");
    assert_eq!(p.take_clicks(), 0, "and nothing sounded when it did");

    p.press_delayed(0);
    assert_eq!(p.take_clicks(), 1);
}

/// `Hotspot_Test` (`0x0040E3EE`) has no `Sound_RestartSlot` anywhere in it,
/// and `Widget_Test`'s own kind 2 arm is the toggle, which also plays nothing.
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
    let pulses = (0..200).map(|_| p.tick().count()).sum::<usize>();
    assert!(pulses >= 3, "the held pulse must have run: {pulses}");
    assert_eq!(p.take_clicks(), 0, "kind 2 fired {pulses} more times and was silent throughout");
}

/// **This one cannot be ablated from `press.rs`**, and that is a finding about
/// the shape: `MapScreen` owns no `Press`, so no
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

/// The county panel's corner closes on the release (`Ui_OkButtonClicked`,
/// `0x0040E7E4`, 26 call sites, no sound); the message scroll's corner closes
/// on the press through `Msg_HandleInput`'s own 48 × 48 test,
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
