#![allow(unused_imports)]
use super::*;

use super::*;
use super::widget_tests::*;
use l2_game::game::{Assets, Prefs, PRESENTATION};
use l2_game::input::{Event, Key};
use l2_game::press;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::options::{self, OptionsScreen, Page, Setting};
use l2_game::Game;
use l2_kingdom::{Quirk, Quirks};

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

#[test]
fn fight_humans_only_displays_yes_when_its_byte_is_zero() {
    let (mut game, assets) = world();
    game.kingdom.options.fight_humans_only_byte = 0;
    assert!(value(Setting::FightHumansOnly, &mut game, &assets));
    game.kingdom.options.fight_humans_only_byte = 1;
    assert!(!value(Setting::FightHumansOnly, &mut game, &assets));

    let mut screen = OptionsScreen::new(Page::Advanced);
    let row = Page::Advanced.rows().iter().find(|r| r.setting == Setting::FightHumansOnly).unwrap();
    let (x, y) = mid(row.hit());
    press_and_wait(&mut screen, &mut game, &assets, x, y);
    assert_eq!(game.kingdom.options.fight_humans_only_byte, 0);
}

/// `Opt_ToggleArmyForaging` (`0x004345D0`) is not a flip alone: it runs
/// `Ration_Apply` and `County_RefreshEstimates` over counties `1 ..
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
        c.herd = 0;
        c.ration_split = 0;
    }
    assert!(!game.kingdom.options.armies_eat, "a new world does not forage");
    for id in 1..=2usize {
        game.kingdom.set_ration_wanted(id, 2);
    }
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


#[test]
fn the_quirk_list_is_both_halves_in_a_stable_order() {
    let rows = options::quirk_rows();
    assert_eq!(rows.len(), Quirk::ALL.len() + PRESENTATION.len());
    let entries: Vec<&str> = rows.iter().map(|r| r.entry).collect();
    for _ in 0..8 {
        let again: Vec<&str> = options::quirk_rows().iter().map(|r| r.entry).collect();
        assert_eq!(entries, again, "the quirk list is not in a stable order");
    }
    for (i, q) in Quirk::ALL.iter().enumerate() {
        assert_eq!(rows[i].entry, q.entry());
    }
}

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

    click(&mut screen, &mut game, &assets, px, py);
    assert_eq!(options::quirk_group(&game), l2_net::Group::AllFixed);
    assert_eq!(options::quirk_tally(&game).0, 0);

    click(&mut screen, &mut game, &assets, px, py);
    assert_eq!(options::quirk_group(&game), l2_net::Group::AllReproduced);

    let rows = options::quirk_rows();
    for (i, row) in rows.iter().enumerate() {
        let (cx, cy) = mid(OptionsScreen::quirk_hit(i));
        click(&mut screen, &mut game, &assets, cx, cy);
        assert_eq!(options::quirk_group(&game), l2_net::Group::Mixed, "{} alone", row.entry);
        click(&mut screen, &mut game, &assets, cx, cy);
        assert_eq!(options::quirk_group(&game), l2_net::Group::AllReproduced, "{} back", row.entry);
    }

    let (cx, cy) = mid(OptionsScreen::quirk_hit(0));
    click(&mut screen, &mut game, &assets, cx, cy);
    assert_eq!(options::quirk_group(&game), l2_net::Group::Mixed);
    click(&mut screen, &mut game, &assets, px, py);
    assert_eq!(options::quirk_group(&game), l2_net::Group::AllFixed);

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
    click(&mut screen, &mut game, &assets, 400, 460);
    click(&mut screen, &mut game, &assets, 600, 100);
    assert_eq!(game.kingdom.options.quirks, before, "a miss moved a quirk");
}

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


