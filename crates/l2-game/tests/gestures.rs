//! **The gesture kinds, driven through the screens that answer them.**
//!
//! `crates/l2-game/src/press.rs` has unit tests for the state machine and
//! `crates/l2-game/tests/press.rs` pins the ramp's 48 bytes against the
//! player's own `Lords2.exe`. Neither can catch the thing that actually went
//! wrong, which is a screen that owns a [`Press`] and never asks it anything:
//! `docs/arms.json` marked nineteen arms `reproduced` under a kind none of them
//! had, and every test in the tree passed.
//!
//! So these tests drive `Machine::handle` and `Machine::update` with the same
//! `Event` values `main.rs` produces, and assert on **what the player sees**:
//! the number does not move until the countdown expires, the arrow keeps
//! stepping while it is held, the picture changes while it is down.
//!
//! # The ablations
//!
//! Each test names the line that must be deleted to turn it red, because a test
//! written against a passing tree has never been observed failing
//! (`docs/agents.md`).

use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::press::{self, Kind, Press, Widget};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::{self, Panel};
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

fn on(r: Rect) -> (i32, i32) {
    (r.centre_x(), r.y + r.h / 2)
}

/// The tax panel, open on county 1.
fn tax_panel() -> (Game, Assets, Machine) {
    let (g, a) = world();
    let m = Machine::new(ScreenId::County(1, Panel::Tax));
    (g, a, m)
}

/// **Holding the tax arrow keeps raising the tax, on the original's ramp.**
///
/// This is the player's report — *"holding on a button doesn't seem to make it
/// go up faster; I recall you could click an up arrow and after a few seconds
/// the number would go up fast"* — turned into an assertion. The arm is
/// `docs/arms.json` `0x004BA9C8/tax-and-ration-arrows`, and it was filed
/// `left-press` while `g_taxWidgets`' four records all carry kind **4**.
///
/// The schedule is not re-derived here: it is [`press::fires_on_step`]'s, and
/// this test asserts only the two things a player would notice — that it
/// repeats at all, and that it speeds up.
///
/// **Ablation, run: delete the `self.press.tick()` arm in
/// `CountyScreen::update`** and the repeat assertion goes red -- the rate falls
/// to zero.
#[test]
fn holding_the_tax_arrow_keeps_raising_the_tax_and_speeds_up() {
    let (mut g, a, mut m) = tax_panel();
    let start = g.kingdom.counties[1].tax_rate;
    let up = on(Panel::Tax.increase_button().expect("the tax panel has an up arrow"));

    send(&mut m, &mut g, &a, Event::Click { x: up.0, y: up.1 });
    let after_press = g.kingdom.counties[1].tax_rate;
    assert_eq!(after_press, start + 1, "the press itself steps once");

    // Hold. The pointer stays on the button, so nothing calls `release`.
    let mut at = Vec::new();
    for t in 1..=120 {
        let before = g.kingdom.counties[1].tax_rate;
        tick(&mut m, &mut g, &a);
        if g.kingdom.counties[1].tax_rate != before {
            at.push(t);
        }
    }
    assert!(!at.is_empty(), "holding the arrow must keep stepping it");
    // The acceleration, as a player would feel it: the gap between the first
    // two repeats is bigger than the gap between the last two.
    assert!(at.len() >= 4, "only {} repeats in 120 ticks: {at:?}", at.len());
    let first_gap = at[1] - at[0];
    let last_gap = at[at.len() - 1] - at[at.len() - 2];
    assert!(
        last_gap < first_gap,
        "the repeat must accelerate: first gap {first_gap} ticks, last {last_gap}, fires {at:?}",
    );
    // And the tax stops at its ceiling rather than running away, which is
    // `Tax_Increase`'s own `if (taxRate < 0x32)`.
    assert!(g.kingdom.counties[1].tax_rate <= l2_game::game::MAX_TAX_RATE);
}

/// **Letting go stops it**, and so does sliding off the arrow.
///
/// `Widget_Test` re-runs the hit test every frame, so a pointer that has walked
/// off the button simply stops matching and the record's counter is never
/// advanced. Both halves are here because they are two different callers of
/// [`Press::pointer`] / [`Press::release`] and only one of them is obvious.
///
/// **Ablation, run: delete the `Event::Release` / `Event::Pointer` bookkeeping
/// block at the top of `CountyScreen::handle`** and this goes red.
#[test]
fn releasing_the_arrow_or_sliding_off_it_stops_the_repeat() {
    for slide_off in [false, true] {
        let (mut g, a, mut m) = tax_panel();
        let up = on(Panel::Tax.increase_button().expect("an up arrow"));
        send(&mut m, &mut g, &a, Event::Click { x: up.0, y: up.1 });
        if slide_off {
            send(&mut m, &mut g, &a, Event::Pointer { x: 4, y: 4 });
        } else {
            send(&mut m, &mut g, &a, Event::Release { x: up.0, y: up.1 });
        }
        let held = g.kingdom.counties[1].tax_rate;
        for _ in 0..200 {
            tick(&mut m, &mut g, &a);
        }
        assert_eq!(
            g.kingdom.counties[1].tax_rate, held,
            "the tax kept climbing after {}",
            if slide_off { "the pointer left the arrow" } else { "the button came up" },
        );
    }
}

/// **The arrow is drawn pressed while its timer runs, and `base + 1` is the
/// whole of what that means.**
///
/// `Widget_Draw` (`0x0040CFD2`) is
/// `if (kind == 4 || kind == 5) { frame = rec[0x04]; if (rec[0x0D]) frame++; }`.
/// Nothing in this engine drew one until the gesture-kind work, and the county
/// panel's own painter said so in a comment for weeks.
///
/// This asserts on [`Press`] rather than on the canvas on purpose: the pressed
/// picture is a *sprite index* and an install with no artwork draws the
/// fallback button, so a canvas assertion here would be gated and would measure
/// the fallback. The painter's use of it is one expression, `frame + 1`, three
/// lines below `let down = self.press.pressed();`.
///
/// **Ablation, run: delete `self.frames = PRESS_FRAMES;` from `Press::press`**
/// and the second assertion goes red — the picture never goes down. The kind-1
/// half of the test is the control: it is red in the *other* direction if
/// `press` is given a press timer it should not have.
#[test]
fn a_kind_four_button_shows_the_pressed_picture_and_a_kind_one_box_does_not() {
    let table = [Widget::new(Rect::new(0, 0, 24, 24), Kind::Repeat)];
    let mut p = Press::new();
    assert_eq!(p.event(&table, Event::Click { x: 4, y: 4 }), Some(0), "kind 4 fires on the press");
    assert_eq!(p.pressed(), Some(0), "and the picture goes down");

    let hotspot = [Widget::new(Rect::new(0, 0, 24, 24), Kind::Press)];
    let mut p = Press::new();
    assert_eq!(p.event(&hotspot, Event::Click { x: 4, y: 4 }), Some(0), "kind 1 fires too");
    assert_eq!(p.pressed(), None, "but a hotspot record is never drawn, so it has no picture");
    assert!(!Kind::Press.has_pressed_frame() && Kind::Repeat.has_pressed_frame());
}

/// **A kind-5 press does not act, and twenty ticks later it does.**
///
/// This is the gauntlet: *"clicking yes/no is instant, whereas the game waited
/// on mouse-up, and the gauntlet would go down slightly when clicked."* Both
/// halves are one kind byte.
///
/// **Ablation, run: delete the `if let Some(w) = self.pending.take()` return
/// from `Press::tick`** and the last assertion goes red -- the handler never
/// runs at all. The SAME ablation turns two military.rs tests red through the
/// divide screen, which is the wiring rather than the mechanism.
#[test]
fn a_kind_five_press_waits_twenty_ticks_with_the_picture_held_down() {
    let table = [
        Widget::new(Rect::new(0, 0, 32, 32), Kind::Delayed),
        Widget::new(Rect::new(48, 0, 32, 32), Kind::Delayed),
    ];
    let mut p = Press::new();
    assert_eq!(p.event(&table, Event::Click { x: 60, y: 4 }), None, "the press does not act");
    assert_eq!(p.pressed(), Some(1), "the gauntlet goes down at once");
    for t in 1..press::DELAYED_FRAMES as u32 {
        assert_eq!(p.tick(), None, "nothing has happened at tick {t}");
        assert_eq!(p.pressed(), Some(1), "and it is still down");
    }
    assert_eq!(p.tick(), Some(1), "the handler runs on the twentieth tick");
    assert_eq!(p.pressed(), None, "and the picture comes back up");
}

/// **Kind 2 is a flat pulse and kind 4 is a ramp**, and the whole point of
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
    assert_eq!(p.pressed(), None, "a hotspot record has no picture");

    let mut at = Vec::new();
    for t in 1..=200u32 {
        if p.tick() == Some(0) {
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
/// would invent, which is why it is worth a test.
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
/// `g_mouseLeftReleased`, so a release inside it fires it whether or not the
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
/// `crates/l2-game/tests/arms.rs`; this is the third artefact of the three, and
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
    // And the mapping from the exe's own byte, which is the only place the
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
/// `Delayed`, and the tests above prove what `Press` then does with that. It
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

/// **Either supplies thumb survives its twentieth tick.** `g_sendSuppliesWidgets`,
/// as [`l2_game::screens::supplies::widgets`] builds it, holds each of the two
/// [`l2_game::screens::supplies::ROWS`]' minus and plus and then the two thumbs,
/// so the thumbs are records 4 and 5. `THUMB_UP_INDEX` was 6: a thumb's
/// countdown reached `SuppliesScreen::fire` below it, took the spinner arm and
/// indexed `ROWS[2]`, and the game panicked twenty ticks after either thumb was
/// pressed. Found by the input branch (`worktree-agent-aec40493a34ed1508`).
///
/// **Ablation, run:** put `THUMB_UP_INDEX` back to `6` and this panics with an
/// index out of bounds in `SuppliesScreen::fire`.
#[test]
fn either_supplies_thumb_fires_on_its_twentieth_tick_without_panicking() {
    use l2_game::screens::supplies::{THUMB_DOWN, THUMB_UP};
    for (thumb, name, cancels) in [(THUMB_DOWN, "the thumb down", true), (THUMB_UP, "the thumb up", false)] {
        let (mut g, a) = world();
        g.prefs.tip_screens = false;
        let mut m = Machine::new(ScreenId::Campaign);
        m.push(ScreenId::Supplies(1));
        let at = on(thumb);
        send(&mut m, &mut g, &a, Event::Click { x: at.0, y: at.1 });
        for t in 1..press::DELAYED_FRAMES as u32 {
            tick(&mut m, &mut g, &a);
            assert_eq!(m.top_id(), Some(ScreenId::Supplies(1)), "{name} acted on tick {t}, before its twentieth");
        }
        for _ in press::DELAYED_FRAMES as u32..=25 {
            tick(&mut m, &mut g, &a);
        }
        if cancels {
            assert_eq!(m.top_id(), Some(ScreenId::Campaign), "FUN_0043B04C's cancel, on the twentieth tick");
        }
    }
}
