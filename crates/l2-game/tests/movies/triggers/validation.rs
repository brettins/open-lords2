#![allow(unused_imports)]
use super::*;
use super::startup::*;
use super::gameplay::*;
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
fn every_trigger_names_a_film_the_install_ships() {
    let (_p, a) = install!();
    let mut played: Vec<&str> = vec!["intro.smk", "imptitle.smk", "credits.smk", "lom.smk"];
    played.extend(movie::CASTLE_FILMS);
    played.extend(movie::CAPTURE_FILMS);
    played.extend(movie::ENDING_FILMS.iter().flatten());
    played.push(movie::VICTORY_FILM);
    played.extend(movie::BATTLE_FILMS.iter().flatten());
    for name in &played {
        assert!(a.films.path(name).is_some(), "{name} is asked for and not shipped");
    }
    let unplayed: Vec<&str> = a.films.names().filter(|n| !played.contains(n)).collect();
    assert_eq!(
        unplayed,
        vec![
            "axemen.smk",
            // `DAT_0057A0F0`'s table, the third battle mode.
            "bat_los5.smk",
            "bat_los6.smk",
            "bat_win5.smk",
            "bat_win6.smk",
            "cas_los3.smk",
            "cas_win3.smk",
        ],
    );
}


