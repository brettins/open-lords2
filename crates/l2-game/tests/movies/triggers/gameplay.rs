#![allow(unused_imports)]
use super::*;
use super::startup::*;
use super::validation::*;
use super::*;
use super::playback::*;
use super::audio_part::*;
use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::message::{category, Record};
use l2_game::movie::{self, Film};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{castle, setup};
use l2_game::Game;
use l2_kingdom::unit::{TroopType, Unit, UnitKind};

#[test]
fn an_ordered_castle_plays_its_film_over_the_chooser() {
    let a = Assets::placeholder();
    let (mut g, mut m) = castle_world();
    assert_eq!(g.kingdom.counties[1].castle_type, 0, "nothing is ordered by the press");
    order_the_castle(&mut m, &mut g, &a);
    assert_eq!(g.kingdom.counties[1].castle_type, 1, "the order stands on the twentieth frame");
    assert_eq!(
        m.ids(),
        vec![ScreenId::Campaign, ScreenId::Castle(1), ScreenId::Movie(Film::Castle(0))],
        "the film plays over the chooser's preview well"
    );
    for _ in 0..3 {
        tick(&mut m, &mut g, &a);
    }
    // The tip the chooser posted during its twenty press frames may be *up* by
    // now, over the map, and that is the original: `FUN_00476E21`'s record
    // outlives the screen that queued it and `Msg_Pump` opens it on `0x00`. So
    // the assertion is about the chooser and the film, and the base.
    assert_eq!(m.ids()[0], ScreenId::Campaign, "the film ends on the map: {:?}", m.ids());
    assert!(
        !m.ids().iter().any(|id| matches!(id, ScreenId::Movie(_) | ScreenId::Castle(_))),
        "and takes the chooser with it: {:?}",
        m.ids()
    );

    let (mut g, mut m) = castle_world();
    g.prefs.animations = false;
    order_the_castle(&mut m, &mut g, &a);
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "no film with animations off");
}

/// **`FUN_00475B41`** — the later the fall, the worse the end. Every name is a
/// literal out of `.rdata`. Ablation: swap two of the year thresholds.
#[test]
fn a_fallen_lords_film_is_chosen_by_his_title_and_how_long_the_game_has_run() {
    let mut g = realms();
    let at = |g: &mut Game, years: i32, realm: u8| {
        g.kingdom.year = movie::FIRST_YEAR + years;
        movie::ending_film(g, realm, 194)
    };
    assert_eq!(at(&mut g, 1, 3), "cart_brn.smk");
    assert_eq!(at(&mut g, 6, 3), "pill_brn.smk");
    assert_eq!(at(&mut g, 12, 3), "jail.smk");
    assert_eq!(at(&mut g, 18, 3), "hang.smk");
    assert_eq!(at(&mut g, 24, 3), "axmen.smk");
    assert_eq!(at(&mut g, 1, 4), "pill_cts.smk");
    assert_eq!(at(&mut g, 7, 5), "cart_bsp.smk");
    assert_eq!(at(&mut g, 1, 1), "jail.smk");
    assert_eq!(at(&mut g, 24, 1), "hang.smk");
    assert_eq!(at(&mut g, 32, 1), "axmen.smk");
    assert_eq!(movie::ending_film(&g, 3, 225), "win_game.smk", "group 0xE1");
}

#[test]
fn an_ending_with_animations_on_is_a_film_and_the_victory_after_it_is_another() {
    let a = Assets::placeholder();
    let mut g = realms();
    ai_turns_over(&mut g);
    g.campaign.ranking = l2_kingdom::victory::Ranking { opponents_remaining: 0, ..Default::default() };
    let player = g.player;
    assert!(g.messages.enqueue(ending(3, 194), player));
    let mut m = Machine::new(ScreenId::Campaign);

    tick(&mut m, &mut g, &a);
    let Some(Film::Ending { file, record, game_over }) = film_on_top(&m) else {
        panic!("no film: {:?}", m.ids());
    };
    assert_eq!((file, record.group, game_over), ("cart_brn.smk", 194, false));
    assert!(g.messages.open().is_none(), "Msg_Dismiss ran inside the draw");
    assert_eq!(g.messages.waiting().iter().map(|r| r.group).collect::<Vec<_>>(), vec![225]);

    let mut victory = None;
    for _ in 0..12 {
        tick(&mut m, &mut g, &a);
        if let Some(f @ Film::Ending { record, .. }) = film_on_top(&m) {
            if record.group == 225 {
                victory = Some(f);
                break;
            }
        }
    }
    let Some(Film::Ending { file, game_over, .. }) = victory else { panic!("no victory film") };
    assert_eq!((file, game_over), ("win_game.smk", true));
    tick(&mut m, &mut g, &a);
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Conquest), "Smk_Play was told to return to 0x1C");
    assert_eq!(g.outcome(), l2_kingdom::victory::Outcome::Won);
}

/// **The animated capture's rotation, `DAT_00553ED4`**: stepped before use, so
/// the first capture of a session is `cap_cty2.smk`.
#[test]
fn a_capture_letter_would_play_the_capture_films_in_rotation() {
    let a = Assets::placeholder();
    let mut g = realms();
    let mut m = Machine::new(ScreenId::Campaign);
    let mut takes = Vec::new();
    for _ in 0..3 {
        let player = g.player;
        let r = Record { to: 0, group: 0x75, category: category::CAPTURE, county: 2, ..Record::default() };
        assert!(g.messages.enqueue(r, player));
        tick(&mut m, &mut g, &a);
        match film_on_top(&m) {
            Some(f @ Film::Capture { take, .. }) => {
                takes.push((take, f.file()));
            }
            other => panic!("{other:?}"),
        }
        tick(&mut m, &mut g, &a);
        tick(&mut m, &mut g, &a);
        assert_eq!(m.top_id(), Some(ScreenId::Campaign));
    }
    assert_eq!(takes, vec![(1, "cap_cty2.smk"), (2, "cap_cty3.smk"), (0, "cap_cty1.smk")]);

    g.prefs.animations = false;
    let player = g.player;
    let r = Record { to: 0, group: 0x75, category: category::CAPTURE, county: 2, ..Record::default() };
    assert!(g.messages.enqueue(r, player));
    tick(&mut m, &mut g, &a);
    assert_eq!(film_on_top(&m), None, "a capture letter played a film with animations off");
    assert_eq!(m.top_id(), Some(ScreenId::Message), "and the ordinary window is up instead");
}

/// **`Battle_CheckOutcome`'s film**, from `g_battleOutcome * 4 +
/// DAT_0053F084`. Our attacker is `SIDE_B`. The siege row is the one this
/// branch corrected: the player who **held** his castle is outcome 4, not 2.
#[test]
fn a_decided_battle_plays_the_film_for_its_outcome_in_rotation() {
    let a = Assets::placeholder();
    let (mut g, mut m) = decided(1, None, l2_sim::SIDE_B);
    tick(&mut m, &mut g, &a);
    assert_eq!(film_on_top(&m), Some(Film::Battle { file: "bat_win1.smk" }), "won, take 0");
    tick(&mut m, &mut g, &a);
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield));
    let live = g.battle.as_ref().expect("still up");
    assert!(live.outcome_ticks < l2_game::battlefield::OUTCOME_FRAMES, "the banner keeps its wait");
    assert_eq!(g.films.battle, 1, "DAT_0053F084 stepped");

    let (mut g, mut m) = decided(2, Some(3), l2_sim::SIDE_A);
    g.films.battle = 1;
    tick(&mut m, &mut g, &a);
    assert_eq!(film_on_top(&m), Some(Film::Battle { file: "sge_win2.smk" }), "outcome 4, take 1");

    let (mut g, mut m) = decided(1, None, l2_sim::SIDE_A);
    g.prefs.animations = false;
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield));
}

