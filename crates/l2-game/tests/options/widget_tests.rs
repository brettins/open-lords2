#![allow(unused_imports)]
use super::*;
use super::page_quirks::*;
use l2_game::game::{Assets, Prefs, PRESENTATION};
use l2_game::input::{Event, Key};
use l2_game::press;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::options::{self, OptionsScreen, Page, Setting};
use l2_game::Game;
use l2_kingdom::{Quirk, Quirks};

/// Every page's window, close button and every widget box is on the 640 × 480
/// screen, and no two boxes on one page overlap.
///
/// The overlap half is the one that matters: two hotspots sharing a pixel is a
/// click whose meaning depends on the order of a `for` loop.
#[test]
fn every_box_is_on_screen_and_no_two_on_a_page_overlap() {
    for page in Page::ALL {
        let (x, y, cols, rows, _) = page.window();
        assert!(x >= 0 && y >= 0, "{page:?}");
        assert!(x + cols * 16 <= 640, "{page:?} is {} wide", x + cols * 16);
        assert!(y + rows * 16 <= 480, "{page:?} is {} tall", y + rows * 16);

        let close = page.close_hit();
        assert!(close.x + close.w <= 640 && close.y + close.h <= 480, "{page:?}'s close button");

        let mut boxes: Vec<l2_game::input::Rect> =
            page.rows().iter().map(|r| r.hit()).collect();
        boxes.push(close);
        for (i, a) in boxes.iter().enumerate() {
            for b in boxes.iter().skip(i + 1) {
                let apart = a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y;
                assert!(apart, "{page:?}: two hotspots overlap at {a:?} and {b:?}");
            }
        }
    }
}

/// **Every row of every original panel toggles the setting behind it**, and the
/// row above and below it stay where they were.
#[test]
fn each_widget_toggles_its_own_row_and_only_its_own() {
    for page in ORIGINALS {
        for row in page.rows() {
            let (mut game, assets) = world();
            let before: Vec<bool> =
                page.rows().iter().map(|r| value(r.setting, &mut game, &assets)).collect();

            let mut screen = OptionsScreen::new(page);
            let (x, y) = mid(row.hit());
            let t = press_and_wait(&mut screen, &mut game, &assets, x, y);
            // `Opt_ToggleFullScreen` leaves the panel before anything else; no
            // other row does.
            let leaves = row.setting == Setting::FullScreen;
            assert_eq!(t == Transition::Pop, leaves, "{page:?} {:?} answered {t:?}", row.setting);

            let after: Vec<bool> =
                page.rows().iter().map(|r| value(r.setting, &mut game, &assets)).collect();

            for (i, r) in page.rows().iter().enumerate() {
                if r.setting == row.setting {
                    if row.supported() {
                        assert_ne!(before[i], after[i], "{page:?} {:?} did not move", r.setting);
                    } else {
                        assert_eq!(before[i], after[i], "{page:?} {:?} is not ours to honour", r.setting);
                    }
                } else {
                    assert_eq!(before[i], after[i], "{page:?} {:?} moved and should not have", r.setting);
                }
            }
        }
    }
}

/// **Every row goes down on the press and acts on the twentieth tick, and not
/// one tick sooner.**
///
/// `Widget_Test`'s kind-5 arm sets `+0x0D = 0x14` and returns without calling
/// the handler; the handler runs from the countdown on the frame it reaches
/// zero, and `Widget_Draw` shows `base + 1` the whole time. All twelve records
/// carry kind 5 in the player's own `Lords2.exe` (`tests/arms.rs` reads it).
///
/// **The first value assertion is the one that goes red if a row fires on the
/// click**, which is what every one of them did before this test existed.
///
/// **Ablations, run, and which line each one stops at:**
///
/// * keep kind 5 and *also* run the row's handler from the `Click` arm — ours
///   before this test — and it goes red at *"Advanced AdvancedFarming acted on
///   the press; Widget_Test kind 5 waits twenty frames"*;
/// * declare the rows `Kind::Press` and fire them on the click, and it goes red
///   one line earlier, at *"the picture goes down at once"*: kind 1 has no
///   pressed frame. **`tests/arms.rs` used to stay green under this one**,
///   because it compared the marker's word with the record's and never read the
///   `Kind` the code declares. The marker is the declaration now — each row's
///   `crate::arm!` — so declaring a row `Press` there turns `arms.rs` red,
///   naming the arm and both words, and a bare `Kind::Press` under a
///   `left-press-delayed` comment is refused as a comment claiming a kind only
///   `Press` answers. Both run;
/// * delete the `self.press.tick()` call from `OptionsScreen::update` and it
///   goes red at *"up again on the twentieth"*: the row stays down and never
///   acts.
#[test]
fn every_row_goes_down_on_the_press_and_acts_twenty_ticks_later() {
    let mut seen = 0;
    for page in ORIGINALS {
        for (i, row) in page.rows().iter().enumerate() {
            let (mut game, assets) = world();
            let mut screen = OptionsScreen::new(page);
            let before = value(row.setting, &mut game, &assets);
            let (x, y) = mid(row.hit());
            {
                let mut ctx = Ctx { game: &mut game, assets: &assets };
                assert_eq!(screen.handle(Event::Click { x, y }, &mut ctx), Transition::Stay);
                // Letting go changes nothing: kind 5 does not wait for it.
                assert_eq!(screen.handle(Event::Release { x, y }, &mut ctx), Transition::Stay);
            }
            assert_eq!(screen.pressed_rows(), vec![i], "{page:?} {:?}: the picture goes down at once", row.setting);
            assert_eq!(
                value(row.setting, &mut game, &assets),
                before,
                "{page:?} {:?} acted on the press; Widget_Test kind 5 waits twenty frames",
                row.setting,
            );
            for t in 1..press::DELAYED_FRAMES {
                let mut ctx = Ctx { game: &mut game, assets: &assets };
                assert_eq!(screen.update(&mut ctx), Transition::Stay, "{page:?} {:?} tick {t}", row.setting);
                assert_eq!(
                    value(row.setting, &mut game, &assets),
                    before,
                    "{page:?} {:?} acted at tick {t}",
                    row.setting,
                );
                assert_eq!(screen.pressed_rows(), vec![i], "{page:?} {:?} came up at tick {t}", row.setting);
            }
            let last = {
                let mut ctx = Ctx { game: &mut game, assets: &assets };
                screen.update(&mut ctx)
            };
            assert!(screen.pressed_rows().is_empty(), "{page:?} {:?}: up again on the twentieth", row.setting);
            if row.supported() {
                assert_ne!(
                    value(row.setting, &mut game, &assets),
                    before,
                    "{page:?} {:?} did not act on the twentieth tick",
                    row.setting,
                );
                assert_eq!(last, Transition::Stay);
                seen += 1;
            }
        }
    }
    assert_eq!(seen, 10, "ten of the twelve rows are honoured; the other two are asserted elsewhere");
}

/// **Two rows pressed five ticks apart both toggle, each on its own twentieth
/// tick.**
///
/// `+0x0D` is a byte of each 24-byte record, and `Widget_Test`'s countdown loop
/// decrements every record's timer on every call and calls each kind-5 handler
/// whose timer reaches zero. `Press` held one timer and one pending widget, so
/// the second press overwrote the first and *Music* never toggled.
///
/// **Ablation, run:** zero every other record's timer in `Press::press_delayed`
/// — the one-pending-press model — and this goes red on tick 6, at *"both rows
/// are drawn down"*: the second press put the first row's picture back up, and
/// its twentieth tick never comes.
#[test]
fn two_rows_pressed_five_ticks_apart_both_toggle_each_on_its_own_twentieth_tick() {
    let (mut game, assets) = world();
    let page = Page::Sound;
    let (first, second) = (page.rows()[0], page.rows()[1]);
    let (a0, b0) = (value(first.setting, &mut game, &assets), value(second.setting, &mut game, &assets));
    let mut screen = OptionsScreen::new(page);
    let mut press = |screen: &mut OptionsScreen, game: &mut Game, r: options::Row| {
        let (x, y) = mid(r.hit());
        let mut ctx = Ctx { game, assets: &assets };
        screen.handle(Event::Click { x, y }, &mut ctx);
        screen.handle(Event::Release { x, y }, &mut ctx);
    };
    press(&mut screen, &mut game, first);
    for t in 1..=25u32 {
        if t == 6 {
            press(&mut screen, &mut game, second);
            assert_eq!(screen.pressed_rows(), vec![0, 1], "both rows are drawn down");
        }
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            screen.update(&mut ctx);
        }
        let (a, b) = (value(first.setting, &mut game, &assets), value(second.setting, &mut game, &assets));
        assert_eq!(a != a0, t >= 20, "{:?} at tick {t}", first.setting);
        assert_eq!(b != b0, t >= 25, "{:?} at tick {t}", second.setting);
    }
}

/// **A row clicks on the press and is silent when it acts** — through the
/// machine, which is the wire `audio::Director` listens to.
///
/// `Widget_Test`'s `Sound_RestartSlot(1)` is on the kind-5 press; the countdown
/// that runs the handler twenty frames later plays nothing.
///
/// **Ablation, run:** return `0` from `OptionsScreen::take_clicks` and the first
/// assertion goes red (`left: 0`). Deleting the countdown instead turns the
/// second red, which is what that assertion is for: a silence after a toggle
/// that never ran would prove nothing.
#[test]
fn a_row_clicks_on_the_press_and_is_silent_when_it_acts() {
    let (mut game, assets) = world();
    // The machine runs the tip screens; this test is not about them.
    game.prefs.tip_screens = false;
    let before = game.prefs.music;
    let mut m = Machine::new(ScreenId::Options(Page::Sound));
    let row = Page::Sound.rows().iter().find(|r| r.setting == Setting::Music).unwrap();
    let (x, y) = mid(row.hit());
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.handle(Event::Click { x, y }, &mut ctx);
        m.handle(Event::Release { x, y }, &mut ctx);
    }
    assert_eq!(m.clicks(), 1, "the press is Widget_Test's kind-5 Sound_RestartSlot(1)");
    for _ in 0..press::DELAYED_FRAMES {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
    }
    assert_ne!(game.prefs.music, before, "the toggle must have run for its silence to mean anything");
    assert_eq!(m.clicks(), 1, "the delayed fire plays nothing");
}

/// **The close button closes on the release, not the press.**
///
/// All four arms call `Ui_OkButtonClicked` (`0x0040E7E4`), whose first line is
/// `if (g_mouseLeftReleased == 0) return 0;`. Ours closed on the press.
///
/// **Ablation, run:** move the close test back to the `Event::Click` arm and the
/// first assertion goes red on the quirks page and on all four panels.
#[test]
fn the_close_button_answers_the_release_and_not_the_press() {
    for page in Page::ALL {
        let (mut game, assets) = world();
        let mut screen = OptionsScreen::new(page);
        let (x, y) = mid(page.close_hit());
        assert_eq!(click(&mut screen, &mut game, &assets, x, y), Transition::Stay, "{page:?} closed on the press");
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(screen.handle(Event::Release { x, y }, &mut ctx), Transition::Pop, "{page:?} release");
    }
}

/// **A click one pixel outside a widget does nothing at all** — it does not
/// toggle, and it does not close the panel, however long you wait.
///
/// Both halves are faults that have reached a player from other screens: the
/// near-miss that acted anyway, and the window that closed when clicked inside.
#[test]
fn a_near_miss_neither_toggles_nor_closes() {
    for page in ORIGINALS {
        for row in page.rows() {
            let (mut game, assets) = world();
            let before = value(row.setting, &mut game, &assets);
            let mut screen = OptionsScreen::new(page);
            let r = row.hit();
            // Just off each of the four edges.
            for (x, y) in [
                (r.x - 1, r.y + r.h / 2),
                (r.x + r.w, r.y + r.h / 2),
                (r.x + r.w / 2, r.y - 1),
                (r.x + r.w / 2, r.y + r.h),
            ] {
                assert_eq!(
                    press_and_wait(&mut screen, &mut game, &assets, x, y),
                    Transition::Stay,
                    "{page:?}: a click at ({x}, {y}) closed the panel"
                );
                let now = value(row.setting, &mut game, &assets);
                assert_eq!(before, now, "{page:?}: a click at ({x}, {y}) toggled {:?}", row.setting);
                assert!(screen.pressed_rows().is_empty(), "{page:?}: a miss at ({x}, {y}) went down");
            }
        }
    }
}

/// The close button closes, and so do `Escape` and a right-click — the
/// original's own two and our one, and no fourth.
#[test]
fn the_panel_closes_three_ways_and_only_three() {
    for page in Page::ALL {
        let (mut game, assets) = world();
        let mut screen = OptionsScreen::new(page);
        let (x, y) = mid(page.close_hit());
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        screen.handle(Event::Click { x, y }, &mut ctx);
        assert_eq!(screen.handle(Event::Release { x, y }, &mut ctx), Transition::Pop, "{page:?} button");
        assert_eq!(
            screen.handle(Event::KeyDown(Key::Escape), &mut ctx),
            Transition::Pop,
            "{page:?} escape"
        );
        assert_eq!(
            screen.handle(Event::RightClick { x: 1, y: 1 }, &mut ctx),
            Transition::Pop,
            "{page:?} right-click"
        );
        // And the one that must NOT close it: Enter and Space, which every
        // shell treated as "confirm". These are not shells any more.
        assert_eq!(screen.handle(Event::KeyDown(Key::Enter), &mut ctx), Transition::Stay);
        assert_eq!(screen.handle(Event::KeyDown(Key::Space), &mut ctx), Transition::Stay);
    }
}

/// **A click never falls through.** `Transition::Pass` would offer the event to
/// whatever is underneath, which for a panel over the campaign map is the map.
#[test]
fn no_click_anywhere_is_ever_passed_to_the_screen_underneath() {
    for page in Page::ALL {
        let (mut game, assets) = world();
        let mut screen = OptionsScreen::new(page);
        for x in (0..640).step_by(37) {
            for y in (0..480).step_by(31) {
                let t = click(&mut screen, &mut game, &assets, x, y);
                assert_ne!(t, Transition::Pass, "{page:?}: ({x}, {y}) fell through to the map");
            }
        }
    }
}

