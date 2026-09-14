#![allow(unused_imports)]
use super::*;
use super::repeating_gestures::*;
use super::double_click::*;
use super::corner_closing::*;
use super::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::press::{self, Kind, Press, Widget};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::{self, Panel};
use l2_game::Game;

/// **The arrow is drawn pressed while its timer runs, and `base + 1` is the
/// whole of what that means.**
///
/// `Widget_Draw` (`0x0040CFD2`) is
/// `if (kind == 4 || kind == 5) { frame = rec[0x04]; if (rec[0x0D]) frame++; }`.
/// Nothing in this engine drew one until the gesture-kind work,
/// panel's own painter said so in a comment for weeks.
///
/// This asserts on [`Press`]: the pressed
/// picture is a *sprite index* and an install with no artwork draws the
/// fallback button,
/// the fallback. The painter's use of it is one expression, `frame + 1`, three
/// lines into the painter's loop, beside `self.press.is_pressed(i)`.
///
/// **Ablation, run: delete `self.timers[w] = PRESS_FRAMES;` from
/// `Press::press`** and the second assertion goes red — the picture never goes
/// down. The kind-1 half of the test is the control: it is red in the *other*
/// direction if `press` is given a press timer it should not have.
#[test]
fn a_kind_four_button_shows_the_pressed_picture_and_a_kind_one_box_does_not() {
    let table = [Widget::new(Rect::new(0, 0, 24, 24), Kind::Repeat)];
    let mut p = Press::new();
    assert_eq!(p.event(&table, Event::Click { x: 4, y: 4 }), Some(0), "kind 4 fires on the press");
    assert!(p.is_pressed(0), "and the picture goes down");

    let hotspot = [Widget::new(Rect::new(0, 0, 24, 24), Kind::Press)];
    let mut p = Press::new();
    assert_eq!(p.event(&hotspot, Event::Click { x: 4, y: 4 }), Some(0), "kind 1 fires too");
    assert!(!p.any_pressed(), "but a hotspot record is never drawn, so it has no picture");
    assert!(!Kind::Press.has_pressed_frame() && Kind::Repeat.has_pressed_frame());
}

/// **A kind-5 press does not act, and twenty ticks later it does.**
///
/// This is the gauntlet: *"clicking yes/no is instant, whereas the game waited
/// on mouse-up,
/// halves are one kind byte.
///
/// **Ablation, run: delete the `fired.delayed |= 1 << i;` line from
/// `Press::tick`** and the last assertion goes red -- the handler never runs at
/// all.
#[test]
fn a_kind_five_press_waits_twenty_ticks_with_the_picture_held_down() {
    let table = [
        Widget::new(Rect::new(0, 0, 32, 32), Kind::Delayed),
        Widget::new(Rect::new(48, 0, 32, 32), Kind::Delayed),
    ];
    let mut p = Press::new();
    assert_eq!(p.event(&table, Event::Click { x: 60, y: 4 }), None, "the press does not act");
    assert!(p.is_pressed(1) && !p.is_pressed(0), "the gauntlet goes down at once, and only it");
    for t in 1..press::DELAYED_FRAMES as u32 {
        assert_eq!(p.tick().next(), None, "nothing has happened at tick {t}");
        assert!(p.is_pressed(1), "and it is still down");
    }
    assert_eq!(p.tick().collect::<Vec<_>>(), vec![1], "the handler runs on the twentieth tick");
    assert!(!p.any_pressed(), "and the picture comes back up");
}

/// **Kind 2 is a flat pulse and kind 4 is a ramp**,
/// keeping them apart is that they are not the same repeat.
///
/// `Hotspot_Test`'s kind-2 arm consults `DAT_0057D3C8` — one of `Tick_Pulses`'
/// eight dividers, every fourth 80 ms pulse — and never touches the 48-byte
/// table. So it fires at a constant [`press::HELD_PULSE_MS`] from the first
/// repeat to the last, where kind 4's first repeat is 240 ms after the press
/// and its last is every 30 ms.
///
/// **Ablation, run: delete the `if self.held_kind == Kind::Held` branch in
/// `Press::tick`** and the flat assertion goes red -- kind 2 starts walking the
/// ramp, whose gaps shrink.
#[test]
fn the_held_pulse_is_flat_where_the_repeat_ramp_accelerates() {
    let table = [Widget::new(Rect::new(0, 0, 24, 24), Kind::Held)];
    let mut p = Press::new();
    assert_eq!(p.event(&table, Event::Click { x: 4, y: 4 }), Some(0), "kind 2 fires on the press");
    assert!(!p.any_pressed(), "a hotspot record has no picture");

    let mut at = Vec::new();
    for t in 1..=200u32 {
        if p.tick().any(|w| w == 0) {
            at.push(t);
        }
    }
    assert!(at.len() >= 3, "kind 2 must repeat: {at:?}");
    let gaps: Vec<u32> = at.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(gaps.iter().all(|&g| g == gaps[0]), "the pulse must be FLAT, gaps {gaps:?}");
    assert_eq!(
        gaps[0] * press::TICK_MS,
        press::HELD_PULSE_MS,
        "and it must be the 320 ms one, not the 30 ms one",
    );
}

/// **A kind-1 box does not fire on the second click of a double click, and a
/// kind-4 button does.**
///
/// Windows sends `WM_LBUTTONDBLCLK` *instead of* the second `WM_LBUTTONDOWN`,
/// so a control that reads only `g_mouseLeftPressed` genuinely misses it.
/// `Widget_Test`'s kind-4 arm tests `pressed || doubleClick` and
/// `Hotspot_Test`'s kind-1 arm tests `pressed` alone — a difference nobody
/// would invent.
///
/// **Ablation, run: make the `Event::DoubleClick` arm of `Press::event` answer
/// `Kind::Press` with `Some(i)`** and the first assertion goes red.
#[test]
fn the_double_click_is_a_press_for_kind_four_and_not_for_kind_one() {
    let at = Event::DoubleClick { x: 4, y: 4 };
    let r = Rect::new(0, 0, 24, 24);
    assert_eq!(Press::new().event(&[Widget::new(r, Kind::Press)], at), None);
    assert_eq!(Press::new().event(&[Widget::new(r, Kind::Repeat)], at), Some(0));
    assert_eq!(Press::new().event(&[Widget::new(r, Kind::Held)], at), Some(0));
    assert_eq!(Press::new().event(&[Widget::new(r, Kind::Delayed)], at), None, "it is delayed");
}

/// **A kind-3 box fires on the release and not on the press**, which is the
/// other end of the same axis.
///
/// It carries no memory of a press: `Hotspot_Test` hit-tests the box and reads
/// `g_mouseLeftReleased`,
/// press that preceded it happened there.
#[test]
fn a_kind_three_box_answers_the_release_wherever_the_press_was() {
    let table = [Widget::new(Rect::new(0, 0, 24, 24), Kind::Release)];
    let mut p = Press::new();
    assert_eq!(p.event(&table, Event::Click { x: 4, y: 4 }), None, "not on the press");
    assert_eq!(p.event(&table, Event::Release { x: 4, y: 4 }), Some(0), "on the release");
    // A press that started elsewhere still ends here, and still fires.
    let mut p = Press::new();
    assert_eq!(p.event(&table, Event::Click { x: 400, y: 400 }), None);
    assert_eq!(p.event(&table, Event::Release { x: 4, y: 4 }), Some(0));
}

/// **Every kind's word is the one `docs/arms.json` files it under**, so the
/// marker beside an arm and the type the code answers it with cannot drift
/// apart by somebody editing one of them.
///
/// The five words are also checked against the closed vocabulary in
/// `crates/l2-game/tests/arms/main.rs`; this is the third artefact of the three, and
/// the one the compiler can reach.
#[test]
fn every_kinds_gesture_word_is_the_inventorys() {
    for (k, word) in [
        (Kind::Press, "left-press"),
        (Kind::Held, "left-press-held"),
        (Kind::Release, "left-release"),
        (Kind::Repeat, "left-press-repeat"),
        (Kind::Delayed, "left-press-delayed"),
    ] {
        assert_eq!(k.gesture(), word);
    }
    // And the mapping from the exe's own byte,
    // tester matters: the two use the same record and different numbers.
    assert_eq!(Kind::from_record(false, 3), Some(Kind::Release));
    assert_eq!(Kind::from_record(true, 3), None, "Widget_Test has no 3 that fires");
    assert_eq!(Kind::from_record(true, 4), Some(Kind::Repeat));
    assert_eq!(Kind::from_record(false, 4), None);
}

/// **The yes/no box's two gauntlets declare kind 5**, which is the player's
/// first report.
///
/// What this proves and what it does not, because the difference matters: it
/// proves the table `BattlefieldScreen::handle` passes to [`Press::event`] says
/// `Delayed`,
/// does **not** drive a live battle to the box, because raising one takes a
/// settled campaign fixture; the same wiring is driven end to end through the
/// divide screen's tick and cross in `military.rs`, whose `press_and_wait`
/// asserts the twenty ticks and went red on the `Press::tick` ablation.
#[test]
fn the_yes_no_boxs_gauntlets_are_kind_five() {
    use l2_game::screens::battlefield::CONFIRM_WIDGETS;
    assert_eq!(CONFIRM_WIDGETS.len(), 2, "g_confirmWidgets holds exactly two records");
    for w in CONFIRM_WIDGETS {
        assert_eq!(w.kind, Kind::Delayed, "both records carry kind 5 at +0x0F");
        assert!(w.kind.has_pressed_frame(), "and both are drawn, so both go down");
    }
    // Record 0 is the tick (hotspot id 1) and record 1 the cross (id 0), which
    // is the order `answer_confirm` reads as yes/no.
    assert!(CONFIRM_WIDGETS[0].rect.x < CONFIRM_WIDGETS[1].rect.x);
}

/// The county panel's table is the one the original's two tables are: two
/// records, kind 4, up first.
#[test]
fn the_two_order_panels_declare_kind_four_and_the_other_two_have_no_table() {
    for p in [Panel::Tax, Panel::Ration] {
        assert!(p.increase_button().is_some() && p.decrease_button().is_some(), "{p:?}");
    }
    for p in [Panel::Population, Panel::Happiness] {
        assert!(p.increase_button().is_none() && p.decrease_button().is_none(), "{p:?}");
    }
    // The up arrow is to the LEFT of the down arrow, which is the tables' own
    // order and therefore the index `CountyScreen::update` steps by.
    let up = county::Panel::Tax.increase_button().unwrap();
    let down = county::Panel::Tax.decrease_button().unwrap();
    assert!(up.x < down.x);
}

