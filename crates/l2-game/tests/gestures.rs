//! **The gesture kinds, driven through the screens that answer them.**
//!
//! `crates/l2-game/src/press/mod.rs` has unit tests for the state machine and
//! `crates/l2-game/tests/press.rs` pins the ramp's 48 bytes against the
//! player's own `Lords2.exe`.
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
    // And the tax stops at its ceiling,
    // `Tax_Increase`'s own `if (taxRate < 0x32)`.
    assert!(g.kingdom.counties[1].tax_rate <= l2_game::game::MAX_TAX_RATE);
}

/// **Letting go stops it**, and so does sliding off the arrow.
///
/// `Widget_Test` re-runs the hit test every frame,
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

/// **A double click on a repeating button fires once and does not hold.**
///
/// `App_WndProc` (`0x004B29BE`) answers `WM_LBUTTONDBLCLK` (`0x203`) with
/// `DAT_004EADA1 |= 1` and nothing else; only `0x201` sets the down bit. So
/// `g_mouseLeftDown` is clear for as long as the second press is held, and
/// `Widget_Test`'s hold branch returns at `if (g_mouseLeftDown == 0) return
/// 0;`. `[V]`
///
/// **Ablation, run:** delete `self.release()` from the `Kind::Repeat` arm of
/// `Press::event`'s double click and the repeat count goes red.
#[test]
fn a_double_click_on_a_repeating_button_fires_once_and_does_not_hold() {
    let r = Rect::new(0, 0, 24, 24);
    for kind in [Kind::Repeat, Kind::Held] {
        let mut p = Press::new();
        assert_eq!(p.event(&[Widget::new(r, kind)], Event::DoubleClick { x: 4, y: 4 }), Some(0));
        let repeats: usize = (0..200).map(|_| p.tick().count()).sum();
        assert_eq!(repeats, 0, "{kind:?}: a double click leaves g_mouseLeftDown clear");
    }
}

/// **A held arrow repaints its number on every step, before the release.**
///
/// A player: *"The click and hold seems to increase the value but it is not
/// visually shown until release. OG game you could see the numbers count up
/// when you held mousedown."* `Tax_IncreaseCounty` (`0x0043AA83`) ends
/// `Panel_Tax()`, so the number is painted on the frame it stepped. Ours
/// repaints when the machine is dirty,
/// is not an event.
///
/// This presents — draw only when `take_dirty` says
/// so, onto a canvas that persists — and on each tick the tax moved it draws
/// the stack **again** onto a copy. A screen that was presented changes nothing
/// on a second draw; a screen that was not paints the new number. The digits'
/// box is where the difference is counted, and it is also required to have
/// changed from the step before, so the box is known to hold the number.
///
/// **Ablation, run:** return `false` from `CountyScreen::take_redraw` and this
/// goes red on the first repeat, while the tax after the release is still
/// right.
#[test]
fn a_held_tax_arrow_shows_every_step_before_the_release() {
    use l2_view::Canvas;
    let (mut g, a, mut m) = tax_panel();
    g.prefs.tip_screens = false;
    let up = on(Panel::Tax.increase_button().expect("an up arrow"));
    // `Panel_Tax`'s rate, `pen.body(canvas, 256, 168, " N%")`.
    let digits = Rect::new(248, 160, 96, 32);
    let count_in = |a: &Canvas, b: &Canvas| {
        (digits.y..digits.y + digits.h)
            .flat_map(|y| (digits.x..digits.x + digits.w).map(move |x| (x as usize, y as usize)))
            .filter(|&(x, y)| a.at(x, y) != b.at(x, y))
            .count()
    };
    let mut shown = Canvas::screen();
    let mut present = |m: &mut Machine, g: &mut Game, shown: &mut Canvas| {
        if m.take_dirty() {
            let ctx = Ctx { game: g, assets: &a };
            m.draw(&ctx, shown);
        }
    };
    present(&mut m, &mut g, &mut shown);
    send(&mut m, &mut g, &a, Event::Click { x: up.0, y: up.1 });
    present(&mut m, &mut g, &mut shown);

    let mut steps = 0;
    for t in 1..=120 {
        let (before, last) = (g.kingdom.counties[1].tax_rate, shown.clone());
        tick(&mut m, &mut g, &a);
        present(&mut m, &mut g, &mut shown);
        if g.kingdom.counties[1].tax_rate == before {
            continue;
        }
        steps += 1;
        let mut again = shown.clone();
        {
            let ctx = Ctx { game: &mut g, assets: &a };
            m.draw(&ctx, &mut again);
        }
        assert_eq!(
            count_in(&shown, &again),
            0,
            "tick {t}: the tax is {} and the screen still shows {before}",
            g.kingdom.counties[1].tax_rate,
        );
        assert_ne!(count_in(&last, &shown), 0, "tick {t}: the digits' box did not change");
    }
    assert!(steps >= 4, "the hold must have repeated for this to mean anything: {steps}");
}

/// **A double click on a prompt's thumb answers it; anywhere else it passes
/// down.** `Msg_HandleInput` (`0x0047685D`) runs `Widget_Test` on the open
/// prompt's table — kind 4, pressed or double-clicked — and its 48 × 48 corner
/// opens `if (g_mouseLeftPressed == 0) { uVar1 = 0; }`. `[V]` This screen passed
/// every double click down, prompt or not.
///
/// **Ablation, run:** delete the message screen's `Event::DoubleClick` arm and
/// the prompt stays up.
#[test]
fn a_double_click_on_a_prompt_thumb_answers_it() {
    let (mut g, a) = world();
    g.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::Campaign);
    let mut rec = l2_game::message::Record::default();
    rec.group = 247;
    rec.category = l2_game::message::category::PAY_PROMPT;
    rec.to = g.player;
    assert!(g.messages.enqueue(rec, g.player));
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Message), "the pump opened the prompt");

    let [_, no] = l2_game::message::Prompt::PayForHelp.widgets();
    let side = l2_game::message::Prompt::SIDE;
    send(&mut m, &mut g, &a, Event::DoubleClick { x: no.0 + side / 2, y: no.1 + side / 2 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the double click declined and closed it");
    assert_eq!(m.clicks(), 1, "Widget_Test's press, so it clicks");
}

/// **A double click on the ration slider's track does not start a drag.**
///
/// `Ration_SliderClick` (`0x0043A379`) has two doors: the arrows want
/// `g_mouseLeftPressed || g_mouseLeftDoubleClick`,
/// `g_mouseLeftDown && g_mouseInputChanged`. `App_WndProc` (`0x004B29BE`)
/// answers `WM_LBUTTONDBLCLK` with `DAT_004EADA1 |= 1` and nothing else — only
/// `WM_LBUTTONDOWN` sets the down bit — so after a double click the button is
/// not down and moving the pointer moves nothing.
///
/// Ours latched `slider_held` on the double click, so the thumb then followed
/// the cursor with the button up.
///
/// **Ablation, run:** put `self.slider_held = true;` back in the
/// `Event::DoubleClick` arm of `CountyScreen::handle` and the second assertion
/// goes red — the pointer drags the split to the far end.
#[test]
fn a_double_click_on_the_ration_slider_does_not_leave_a_drag_running() {
    use l2_game::screens::county::split_track;
    let (mut g, a) = world();
    g.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::County(1, Panel::Ration));
    let track = split_track();
    let (low, high) = (track.x + 10, track.x + 90);
    let y = track.y + track.h / 2;

    // The double click still jumps the value: the arm is entered on either flag.
    send(&mut m, &mut g, &a, Event::DoubleClick { x: low, y });
    assert_eq!(g.kingdom.counties[1].ration_split, 10, "the double click still jumps");

    send(&mut m, &mut g, &a, Event::Pointer { x: high, y });
    assert_eq!(
        g.kingdom.counties[1].ration_split, 10,
        "a double click must not leave the drag latched on",
    );

    // A real press does latch it, which is the other half of the same field.
    send(&mut m, &mut g, &a, Event::Click { x: low, y });
    send(&mut m, &mut g, &a, Event::Pointer { x: high, y });
    assert_eq!(g.kingdom.counties[1].ration_split, 90, "and the press does drag");
}

/// **The castle chooser's corner picture closes on the release, not the press.**
///
/// `Screen_FrameInput`'s `0x1B` arm is `Ui_OkButtonClicked()` (`0x0040E7E4`),
/// whose first statement is `if (g_mouseLeftReleased == 0) return 0;`. The
/// marker in `castle.rs` said `left-release` and the code read `Event::Click`.
///
/// **Ablation, run:** move the `CORNER_OK.contains` test back into the
/// `Event::Click` arm and the first assertion goes red.
#[test]
fn the_castle_choosers_corner_closes_on_the_release() {
    use l2_game::screens::castle::CORNER_OK;
    let (mut g, a) = world();
    g.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Castle(1));
    let opened = m.top_id();
    let at = on(CORNER_OK);
    send(&mut m, &mut g, &a, Event::Click { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), opened, "the press does nothing: the corner reads g_mouseLeftReleased");
    send(&mut m, &mut g, &a, Event::Release { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the release closes it");
}

/// **The raise-army screen's corner picture closes on the release too.**
///
/// `Screen_FrameInput`'s `0x17` arm asks `Levy_SliderClick()` (`0x00435CEF`)
/// first — which cannot answer a release: its arrow branches want
/// `g_mouseLeftPressed || g_mouseLeftDoubleClick` and its track branch
/// `g_mouseLeftDown` — and then `Ui_OkButtonClicked()`.
///
/// **Ablation, run:** move the `OK.contains` test back into the `Event::Click`
/// arm and the first assertion goes red.
#[test]
fn the_raise_army_screens_corner_closes_on_the_release() {
    use l2_game::screens::army::OK;
    let (mut g, a) = world();
    g.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::RaiseArmy(1));
    let opened = m.top_id();
    let at = on(OK);
    send(&mut m, &mut g, &a, Event::Click { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), opened, "the press does nothing");
    send(&mut m, &mut g, &a, Event::Release { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the release closes it");
}

/// **A double click on the supplies thumb down restarts its twenty frames and
/// then leaves.** `Screen_HandleInput`'s `0x18` arm runs `Widget_Test` on
/// `g_sendSuppliesWidgets`, kinds 4 and 5, whose guards read the double click;
/// the icon hotspots and the minimap pick read `g_mouseLeftPressed` alone.
/// `[V]` This screen dropped it.
///
/// **Ablation, run:** disable the supplies screen's `Event::DoubleClick` arm and
/// the click count goes red at 0 — the double click reached nothing, so nothing
/// went down and nothing will leave.
#[test]
fn a_double_click_on_the_supplies_thumb_down_leaves_twenty_ticks_later() {
    use l2_game::screens::supplies::THUMB_DOWN;
    let (mut g, a) = world();
    g.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Supplies(1));
    let at = on(THUMB_DOWN);
    send(&mut m, &mut g, &a, Event::DoubleClick { x: at.0, y: at.1 });
    assert_eq!(m.clicks(), 1, "kind 5's press clicks");
    for t in 1..press::DELAYED_FRAMES as u32 {
        tick(&mut m, &mut g, &a);
        assert_eq!(m.top_id(), Some(ScreenId::Supplies(1)), "left on tick {t}");
    }
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "FUN_0043B04C's cancel, on the twentieth tick");
}

/// **A double click on the information panel's garrison widget turns the tile
/// half into the unit half.** `0x04`'s arm tests the corner and the brush
/// (releases), then `FUN_00438A91`'s `Widget_Test` kind 4, then the unit
/// buttons (`Hotspot_Test` kind 1); only the widget reads the double click.
/// `[V]` This screen dropped it.
///
/// **Ablation, run:** delete the information panel's `Event::DoubleClick` arm
/// and the panel stays on the tile.
#[test]
fn a_double_click_on_the_garrison_widget_opens_the_garrison() {
    use l2_game::screens::info::{Target, GARRISON_WIDGET};
    let (mut g, a) = world();
    g.prefs.tip_screens = false;
    let tile = l2_kingdom::map::index(20, 20);
    g.kingdom.campaign.map.county[tile] = 1;
    g.kingdom.campaign.map.terrain[tile] = l2_kingdom::map::terrain::CASTLE_PLOT + 1;
    g.kingdom.campaign.map.flags[tile] |= l2_kingdom::map::flags::SETTLEMENT;
    g.kingdom.counties[1].garrison_unit = 7;
    g.map_zoom_far = false;
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Info(Target::Tile(tile)));
    let at = on(GARRISON_WIDGET);
    send(&mut m, &mut g, &a, Event::DoubleClick { x: at.0, y: at.1 });
    assert_eq!(m.top_id(), Some(ScreenId::Info(Target::Unit(7))), "FUN_00438ACC ran on the double click");
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

/// **Either supplies thumb survives its twentieth tick.** `g_sendSuppliesWidgets`,
/// as [`l2_game::screens::supplies::widgets`] builds it, holds each of the two
/// [`l2_game::screens::supplies::ROWS`]' minus and plus and then the two thumbs,
/// so the thumbs are records 4 and 5. `THUMB_UP_INDEX` was 6: a thumb's
/// countdown reached `SuppliesScreen::fire` below it, took the spinner arm and
/// indexed `ROWS[2]`,
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
