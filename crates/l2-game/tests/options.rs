//! **The options panels, and the quirks page.**
//!
//! Driven with [`Event`] values, never with a real pointer — nothing here
//! touches the OS input queue and nothing needs a window.
//!
//! Three things this file is written to catch, all of which have already
//! reached a player from other screens:
//!
//! * **a near-miss acting anyway.** `crates/l2-game/src/screens/map/mod.rs`'s header
//!   records three wrong-screen bugs in one evening, every one a click that fell
//!   through to county selection. A page of check boxes is a page of small
//!   hotspots with the same exposure, so every test that clicks a box also
//!   clicks *beside* it and asserts nothing moved;
//! * **a window that closes when you click inside it**, reported by a player in
//!   those words;
//! * **a control that acts on the press where the original waits.** Every row
//!   of the four original panels is `Widget_Test` **kind 5** — the picture goes
//! down on the press and the toggle runs twenty frames later — and ours acted
//!   on the click. That is the yes/no gauntlets' defect (*"clicking yes/no is
//!   instant whereas the game waited"*), and `docs/input.md` §4 is the model.

use l2_game::game::{Assets, Prefs, PRESENTATION};
use l2_game::input::{Event, Key};
use l2_game::press;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::options::{self, OptionsScreen, Page, Setting};
use l2_game::Game;
use l2_kingdom::{Quirk, Quirks};

fn world() -> (Game, Assets) {
    (Game::new(1), Assets::placeholder())
}

/// The four pages that are the original's.
const ORIGINALS: [Page; 4] = [Page::Advanced, Page::Sound, Page::Display, Page::Help];

/// Click at a point and return what the screen asked the machine to do.
fn click(screen: &mut OptionsScreen, game: &mut Game, assets: &Assets, x: i32, y: i32) -> Transition {
    let mut ctx = Ctx { game, assets };
    screen.handle(Event::Click { x, y }, &mut ctx)
}

/// **A whole click on a kind-5 row**: press, release, and the twenty ticks the
/// original waits before the handler runs. Returns the first transition that
/// was not `Stay`, from any of the three.
fn press_and_wait(
    screen: &mut OptionsScreen,
    game: &mut Game,
    assets: &Assets,
    x: i32,
    y: i32,
) -> Transition {
    let mut ctx = Ctx { game, assets };
    let mut out = screen.handle(Event::Click { x, y }, &mut ctx);
    let released = screen.handle(Event::Release { x, y }, &mut ctx);
    if out == Transition::Stay {
        out = released;
    }
    for _ in 0..press::DELAYED_FRAMES {
        let t = screen.update(&mut ctx);
        if out == Transition::Stay {
            out = t;
        }
    }
    out
}

/// The middle of a rectangle.
fn mid(r: l2_game::input::Rect) -> (i32, i32) {
    (r.x + r.w / 2, r.y + r.h / 2)
}

fn value(setting: Setting, game: &mut Game, assets: &Assets) -> bool {
    options::value(setting, &Ctx { game, assets })
}

// ---------------------------------------------------------------------------
// The four the original has
// ---------------------------------------------------------------------------

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

/// The four the original has carry its `g_screenId` and its painter; the fifth
/// carries neither, because it is ours.
#[test]
fn four_pages_are_the_originals_and_one_is_ours() {
    let mine: Vec<Page> = Page::ALL.into_iter().filter(|p| p.is_ours()).collect();
    assert_eq!(mine, vec![Page::Quirks], "exactly one page is ours");
    for page in Page::ALL {
        assert_eq!(page.screen_id().is_some(), !page.is_ours(), "{page:?}");
        assert_eq!(page.painter().is_some(), !page.is_ours(), "{page:?}");
        assert_eq!(page.group().is_some(), !page.is_ours(), "{page:?}");
        assert_eq!(page.rows().is_empty(), page.is_ours(), "{page:?}");
    }
}

/// *Fight humans only?* is **stored inverted** — the byte is 0 when the option
/// displays *Yes* — and the panel is the only place that sense is turned round.
/// A row that read the byte as a bool would show every game backwards.
#[test]
fn fight_humans_only_displays_yes_when_its_byte_is_zero() {
    let (mut game, assets) = world();
    game.kingdom.options.fight_humans_only_byte = 0;
    assert!(value(Setting::FightHumansOnly, &mut game, &assets));
    game.kingdom.options.fight_humans_only_byte = 1;
    assert!(!value(Setting::FightHumansOnly, &mut game, &assets));

    // And the toggle puts the byte back, not the bool.
    let mut screen = OptionsScreen::new(Page::Advanced);
    let row = Page::Advanced.rows().iter().find(|r| r.setting == Setting::FightHumansOnly).unwrap();
    let (x, y) = mid(row.hit());
    press_and_wait(&mut screen, &mut game, &assets, x, y);
    assert_eq!(game.kingdom.options.fight_humans_only_byte, 0);
}

/// **Army foraging re-runs the ration pass in every county, on the flip.**
///
/// `Opt_ToggleArmyForaging` (`0x004345D0`) is not a flip alone: it runs
/// `Ration_Apply` and `County_RefreshEstimates` over counties `1 ..
/// g_countyCount`, because the rule changes who a county feeds. Ours flipped
/// the flag and left every county's ration fields describing the old rule.
///
/// The fixture settles both counties under the old rule through the ration
/// panel's own path first, so the fields are not zero before the flip — a
/// test from zero would pass for any pass run with either flag.
///
/// **Ablation, run:** delete the `for` loop from
/// `Kingdom::toggle_army_foraging` and both counties' assertions go red.
#[test]
fn army_foraging_re_runs_the_ration_pass_in_every_county() {
    let (mut game, assets) = world();
    game.kingdom.set_county_count(2);
    for id in 1..=2usize {
        let c = &mut game.kingdom.counties[id];
        c.owner = 1;
        c.population = 1_000;
        c.friendly_troops = 800;
        c.grain = 50_000;
        // No herd: five people a head are fed by the milk and eat nothing, so
        // a herd of any size would hide the difference. All of it from grain.
        c.herd = 0;
        c.ration_split = 0;
    }
    assert!(!game.kingdom.options.armies_eat, "a new world does not forage");
    for id in 1..=2usize {
        game.kingdom.set_ration_wanted(id, 2);
    }
    // What the ration pass records the county eating, meat and grain together:
    // `ration_split` decides the mix,
    let eaten = |game: &Game, id: usize| {
        let c = &game.kingdom.counties[id];
        (c.herd_eaten, c.grain_eaten)
    };
    let before: Vec<(i32, i32)> = (1..=2).map(|id| eaten(&game, id)).collect();
    assert!(before.iter().all(|&(h, g)| h + g > 0), "the fixture must feed somebody: {before:?}");

    let mut screen = OptionsScreen::new(Page::Advanced);
    let row = Page::Advanced.rows().iter().find(|r| r.setting == Setting::ArmyForaging).unwrap();
    let (x, y) = mid(row.hit());
    press_and_wait(&mut screen, &mut game, &assets, x, y);

    assert!(game.kingdom.options.armies_eat);
    for id in 1..=2usize {
        let (h, g) = eaten(&game, id);
        let (h0, g0) = before[id - 1];
        assert!(
            h + g > h0 + g0,
            "county {id} still eats {h} head and {g} sacks, the ration for its people alone \
             ({h0} and {g0}); the 800 men standing in it are fed from the moment foraging is \
             switched on",
        );
    }
}

/// **Full screen, on any desktop that is not 8bpp, leaves the panel and says the
/// display cannot change** — which is what the original does on the same
/// desktop, because `Display_Init` has already forced the flag on.
///
/// **Ablation, run:** make `OptionsScreen::fire`'s `FullScreen` arm return
/// `Transition::Stay` without the message and both assertions go red.
#[test]
fn full_screen_leaves_the_panel_and_says_the_display_cannot_change() {
    let (mut game, assets) = world();
    let mut screen = OptionsScreen::new(Page::Display);
    let row = Page::Display.rows().iter().find(|r| r.setting == Setting::FullScreen).unwrap();
    let (x, y) = mid(row.hit());
    assert_eq!(press_and_wait(&mut screen, &mut game, &assets, x, y), Transition::Pop);
    let posted: Vec<u16> = game.messages.waiting().iter().map(|r| r.group).collect();
    assert_eq!(posted, vec![options::FULL_SCREEN_REFUSAL], "L2.eng group 260, 'Cannot change display.'");
}

/// **Start game help presses, clicks, waits and does nothing.** `WinHelpA` on
/// a Windows 3.1 help file is not something this engine can open, and the arm
/// is filed `missing`; what is asserted is that the press is the original's and
/// that nothing else happens in its place.
#[test]
fn start_game_help_is_a_press_with_nothing_behind_it() {
    let (mut game, assets) = world();
    let prefs = game.prefs;
    let mut screen = OptionsScreen::new(Page::Help);
    let row = Page::Help.rows().iter().find(|r| r.setting == Setting::StartGameHelp).unwrap();
    let (x, y) = mid(row.hit());
    assert_eq!(press_and_wait(&mut screen, &mut game, &assets, x, y), Transition::Stay);
    assert_eq!(game.prefs, prefs);
    assert!(game.messages.waiting().is_empty());
}

/// The sound rows are **preferences**, not the world's: toggling one must not
/// change a single byte of the kingdom, or a sound setting would desync a
/// lockstep peer.
#[test]
fn toggling_sound_or_animations_does_not_touch_the_world() {
    let (mut game, assets) = world();
    let before = l2_kingdom::save::checksum(&game.kingdom);
    for page in [Page::Sound, Page::Display, Page::Help] {
        let mut screen = OptionsScreen::new(page);
        for row in page.rows() {
            let (x, y) = mid(row.hit());
            press_and_wait(&mut screen, &mut game, &assets, x, y);
        }
    }
    assert_ne!(game.prefs, Prefs::default(), "something moved, or this proves nothing");
    assert_eq!(
        l2_kingdom::save::checksum(&game.kingdom),
        before,
        "a preference reached the lockstep digest"
    );
}

// ---------------------------------------------------------------------------
// The quirks page — the group switch
// ---------------------------------------------------------------------------

/// **The list spans both homes**, in a stable, index-ordered sequence.
#[test]
fn the_quirk_list_is_both_halves_in_a_stable_order() {
    let rows = options::quirk_rows();
    assert_eq!(rows.len(), Quirk::ALL.len() + PRESENTATION.len());
    // Same answer every time it is asked: no map, nothing hash-ordered.
    let entries: Vec<&str> = rows.iter().map(|r| r.entry).collect();
    for _ in 0..8 {
        let again: Vec<&str> = options::quirk_rows().iter().map(|r| r.entry).collect();
        assert_eq!(entries, again, "the quirk list is not in a stable order");
    }
    // Behavioural first, in `Quirk::ALL`'s order.
    for (i, q) in Quirk::ALL.iter().enumerate() {
        assert_eq!(rows[i].entry, q.entry());
    }
}

/// **Faithful by default**, which is the whole shipping decision.
#[test]
fn a_new_game_reproduces_every_one_of_the_originals_bugs() {
    let (game, _) = world();
    assert_eq!(options::quirk_group(&game), l2_net::Group::AllReproduced);
    let (on, total) = options::quirk_tally(&game);
    assert_eq!(on, total);
    for row in options::quirk_rows() {
        assert!(options::quirk_reproduced(row, &game), "{} is off by default", row.entry);
    }
}

/// **The parent, through all three states and every transition** — the states a
/// two-value fixture never reaches. `docs/decisions.md` C26.
#[test]
fn the_parent_walks_all_three_states_and_back() {
    let (mut game, assets) = world();
    let mut screen = OptionsScreen::new(Page::Quirks);
    let (px, py) = mid(OptionsScreen::parent_hit());

    assert_eq!(options::quirk_group(&game), l2_net::Group::AllReproduced);

    // Parent clicked: everything off.
    click(&mut screen, &mut game, &assets, px, py);
    assert_eq!(options::quirk_group(&game), l2_net::Group::AllFixed);
    assert_eq!(options::quirk_tally(&game).0, 0);

    // Parent clicked again: everything back on.
    click(&mut screen, &mut game, &assets, px, py);
    assert_eq!(options::quirk_group(&game), l2_net::Group::AllReproduced);

    // **Every** child, one at a time, makes the parent mixed and then pure
    // again — not one sampled child.
    let rows = options::quirk_rows();
    for (i, row) in rows.iter().enumerate() {
        let (cx, cy) = mid(OptionsScreen::quirk_hit(i));
        click(&mut screen, &mut game, &assets, cx, cy);
        assert_eq!(options::quirk_group(&game), l2_net::Group::Mixed, "{} alone", row.entry);
        click(&mut screen, &mut game, &assets, cx, cy);
        assert_eq!(options::quirk_group(&game), l2_net::Group::AllReproduced, "{} back", row.entry);
    }

    // From mixed, one parent click goes to all-fixed — the direction the user
    // asked for by name.
    let (cx, cy) = mid(OptionsScreen::quirk_hit(0));
    click(&mut screen, &mut game, &assets, cx, cy);
    assert_eq!(options::quirk_group(&game), l2_net::Group::Mixed);
    click(&mut screen, &mut game, &assets, px, py);
    assert_eq!(options::quirk_group(&game), l2_net::Group::AllFixed);

    // And every child turned back on one at a time returns the parent to pure,
    // passing through Mixed exactly once on the way.
    for (i, row) in rows.iter().enumerate() {
        let (cx, cy) = mid(OptionsScreen::quirk_hit(i));
        click(&mut screen, &mut game, &assets, cx, cy);
        let expected = if i + 1 == rows.len() {
            l2_net::Group::AllReproduced
        } else {
            l2_net::Group::Mixed
        };
        assert_eq!(options::quirk_group(&game), expected, "after {}", row.entry);
    }
}

/// **The parent does not remember.** Turning it off and on again does not
/// restore a previous mixture — that would be a fourth state the check box
/// cannot show.
#[test]
fn the_parent_does_not_restore_a_previous_mixture() {
    let (mut game, assets) = world();
    let mut screen = OptionsScreen::new(Page::Quirks);
    let (cx, cy) = mid(OptionsScreen::quirk_hit(0));
    click(&mut screen, &mut game, &assets, cx, cy);
    let mixed = game.kingdom.options.quirks;

    let (px, py) = mid(OptionsScreen::parent_hit());
    click(&mut screen, &mut game, &assets, px, py); // all fixed
    click(&mut screen, &mut game, &assets, px, py); // all reproduced
    assert_ne!(game.kingdom.options.quirks, mixed, "the parent restored the old mixture");
    assert_eq!(game.kingdom.options.quirks, Quirks::FAITHFUL);
}

/// A near-miss on the quirks page toggles nothing, and neither does a click in
/// the empty space below the list.
#[test]
fn a_near_miss_on_the_quirks_page_changes_nothing() {
    let (mut game, assets) = world();
    let mut screen = OptionsScreen::new(Page::Quirks);
    let before = game.kingdom.options.quirks;
    let rows = options::quirk_rows().len();

    for i in 0..rows {
        let r = OptionsScreen::quirk_hit(i);
        for (x, y) in [
            (r.x - 1, r.y + r.h / 2),
            (r.x + r.w, r.y + r.h / 2),
            (r.x + r.w / 2, r.y - 1),
            (r.x + r.w / 2, r.y + r.h),
        ] {
            assert_eq!(click(&mut screen, &mut game, &assets, x, y), Transition::Stay);
        }
    }
    // Well below the last row, and well to the right of the boxes.
    click(&mut screen, &mut game, &assets, 400, 460);
    click(&mut screen, &mut game, &assets, 600, 100);
    assert_eq!(game.kingdom.options.quirks, before, "a miss moved a quirk");
}

/// **A quirk set on the page is the quirk set a new game starts with.**
///
/// The join between the interface and the world: `Settings::commit` takes the
/// value off the game, and `apply_to` puts it back. A page that wrote somewhere
/// nothing read would be a page of checkboxes that did nothing to a campaign,
/// which is the failure this whole branch exists to avoid.
#[test]
fn what_the_page_sets_is_what_a_new_game_is_played_with() {
    let (mut game, assets) = world();
    let mut screen = OptionsScreen::new(Page::Quirks);
    let (px, py) = mid(OptionsScreen::parent_hit());
    click(&mut screen, &mut game, &assets, px, py);
    let chosen = game.kingdom.options.quirks;
    assert_eq!(chosen, Quirks::FIXED);

    let settings = l2_game::setup::SetupOptions::new().commit(1, chosen);
    assert_eq!(settings.quirks, chosen);
    assert_eq!(settings.kingdom_options().quirks, chosen);
}

/// The presentation half is written where the drawing code will read it.
///
/// `Assets::quirks` is a per-frame projection of `Game::presentation_quirks`
/// (`main.rs`), so this asserts the *authority* moves; the projection itself is
/// one line and is asserted by the next test.
#[test]
fn a_presentation_quirk_is_written_to_the_session_not_to_the_world() {
    let (mut game, assets) = world();
    let before = l2_kingdom::save::checksum(&game.kingdom);
    let mut screen = OptionsScreen::new(Page::Quirks);
    for (i, row) in options::quirk_rows().iter().enumerate() {
        if matches!(row.home, options::Home::Presentation(_)) {
            let (x, y) = mid(OptionsScreen::quirk_hit(i));
            click(&mut screen, &mut game, &assets, x, y);
        }
    }
    assert_eq!(
        l2_kingdom::save::checksum(&game.kingdom),
        before,
        "a presentation quirk reached the lockstep digest — docs/bugs.md §6.3a"
    );
}

/// **The projection happens.** `Game::presentation_quirks` is the authority and
/// `Assets::quirks` is its copy; if nothing copied, every presentation quirk
/// would be a setting the drawing code never saw — `docs/decisions.md` C30's
/// shape once more.
#[test]
fn the_presentation_quirks_are_projected_into_the_assets() {
    let src = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join("main.rs"),
    )
    .expect("main.rs is part of this check");
    assert!(
        src.replace("\r\n", "\n").contains("self.assets.quirks = self.game.presentation_quirks;"),
        "main.rs no longer copies Game::presentation_quirks into Assets::quirks, so a \
         presentation quirk set on the quirks page would change nothing on screen"
    );
}

/// Reachability from the demo index is asserted inside `screens::index`, whose
/// row list is private. This is the other half of it: every page a `ScreenId`
/// can name is one `ScreenId::build` can make.
#[test]
fn every_options_page_can_be_built() {
    for page in Page::ALL {
        let built = ScreenId::Options(page).build();
        let (mut game, assets) = world();
        let ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(built.id(), ScreenId::Options(page));
        assert!(!built.title(&ctx).is_empty(), "{page:?} has no title");
        assert!(built.is_overlay(), "{page:?} is a window over what opened it");
    }
}
