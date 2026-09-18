#![allow(unused_imports)]
use super::*;
use super::layout_tests::*;
use super::interaction_tests::*;
use super::sound_tests::*;
use super::close_tests::*;
use super::*;
use super::page_quirks::*;
use l2_game::game::{Assets, Prefs, PRESENTATION};
use l2_game::input::{Event, Key};
use l2_game::press;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::options::{self, OptionsScreen, Page, Setting};
use l2_game::Game;
use l2_kingdom::{Quirk, Quirks};

/// `Widget_Test`'s kind-5 arm sets `+0x0D = 0x14` and returns without calling
/// the handler; the handler runs from the countdown on the frame it reaches
/// zero, and `Widget_Draw` shows `base + 1` the whole time. All twelve records
/// carry kind 5 in the player's own `Lords2.exe` (`tests/arms.rs` reads it).
///
/// **The first value assertion is the one that goes red if a row fires on the
/// click**, which is what every one of them did before this test existed.
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

