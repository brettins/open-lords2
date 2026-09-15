#![allow(unused_imports)]
use super::*;
use super::button_kinds::*;
use super::double_click::*;
use super::corner_closing::*;
use super::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::press::{self, Kind, Press, Widget};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::{self, Panel};
use l2_game::Game;

/// This is the player's report — *"holding on a button doesn't seem to make it
/// go up faster; I recall you could click an up arrow and after a few seconds
/// the number would go up fast"* — turned into an assertion. The arm is
/// `docs/arms.json` `0x004BA9C8/tax-and-ration-arrows`, and it was filed
/// `left-press` while `g_taxWidgets`' four records all carry kind **4**.
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

    let mut at = Vec::new();
    for t in 1..=120 {
        let before = g.kingdom.counties[1].tax_rate;
        tick(&mut m, &mut g, &a);
        if g.kingdom.counties[1].tax_rate != before {
            at.push(t);
        }
    }
    assert!(!at.is_empty(), "holding the arrow must keep stepping it");
    assert!(at.len() >= 4, "only {} repeats in 120 ticks: {at:?}", at.len());
    let first_gap = at[1] - at[0];
    let last_gap = at[at.len() - 1] - at[at.len() - 2];
    assert!(
        last_gap < first_gap,
        "the repeat must accelerate: first gap {first_gap} ticks, last {last_gap}, fires {at:?}",
    );
    assert!(g.kingdom.counties[1].tax_rate <= l2_game::game::MAX_TAX_RATE);
}

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

/// A player: *"The click and hold seems to increase the value but it is not
/// visually shown until release. OG game you could see the numbers count up
/// when you held mousedown."* `Tax_IncreaseCounty` (`0x0043AA83`) ends
/// `Panel_Tax()`, so the number is painted on the frame it stepped. Ours
/// repaints when the machine is dirty,
/// is not an event.
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


