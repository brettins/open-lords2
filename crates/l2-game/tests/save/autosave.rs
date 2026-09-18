#![allow(unused_imports)]
use super::*;
use super::roundtrip::*;
use super::isolation::*;
use super::ui::*;
use std::path::PathBuf;
use l2_game::input::{Event, Key};
use l2_game::save::{self, LoadError};
use l2_game::saves;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::saveload::{Mode, SaveLoadScreen, Status};
use l2_game::turn;
use l2_game::{Assets, Game};
use l2_kingdom::tables::{Tables, Weather};
use l2_kingdom::{Kingdom, Options};

/// **`Save_RotateAndWrite` (`0x0049A453`) keeps three turns, not one.**
///
/// ```c
/// if (DAT_00553260 < 1) {
///     remove(safeturn); rename(old_turn, safeturn); rename(lastturn, old_turn);
/// }
/// Save_Write(lastturn);
/// ```
#[test]
fn the_autosave_keeps_the_last_three_turns_newest_first() {
    let own = Saves::new("autosave-rotation");
    let mut game = played(1);
    let mut want = Vec::new();
    for _ in 0..3 {
        want.insert(0, digest(&game.kingdom));
        saves::rotate_and_write(&game).expect("the autosave is written");
        assert!(turn::end_turn(&mut game).is_some(), "a turn between two autosaves");
    }
    assert_eq!(
        own.files(),
        vec![file("lastturn"), file("old_turn"), file("safeturn")],
        "the original's three names, and nothing else"
    );
    let read = |n: &str| digest(&saves::read(n, Tables::DEFAULT).expect("readable").kingdom);
    let got: Vec<u64> = saves::AUTOSAVES.iter().map(|n| read(n)).collect();
    assert_eq!(got, want, "lastturn is the newest and safeturn the oldest");

    want.insert(0, digest(&game.kingdom));
    want.pop();
    saves::rotate_and_write(&game).expect("written");
    let got: Vec<u64> = saves::AUTOSAVES.iter().map(|n| read(n)).collect();
    assert_eq!(got, want, "three deep, always");
    assert_eq!(own.files().len(), 3, "no fourth file");
}

#[test]
fn the_first_autosave_of_a_game_writes_one_file_and_reports_no_failure() {
    let own = Saves::new("autosave-first");
    let game = played(1);
    saves::rotate_and_write(&game).expect("nothing to rotate is not a failure");
    assert_eq!(own.files(), vec![file("lastturn")]);
}

#[test]
fn the_autosave_lands_where_every_other_save_of_ours_lands() {
    let own = Saves::new("autosave-where");
    let game = played(1);
    let path = saves::rotate_and_write(&game).expect("written");
    assert!(path.starts_with(&own.path), "{} is not under the save directory", path.display());
    assert_eq!(path.extension().and_then(|e| e.to_str()), Some(save::EXTENSION));
}

/// **It must be raised in the dark, not on the button and not on the far side
/// of the light.** `FUN_0049A3E6` runs on `g_screenId == 0x24`, after the fade
/// has bottomed out and the seasonal art has been reloaded
/// holds is the *opening* of the turn that just began — which is what the last
/// assertion reads back.
#[test]
fn ending_a_turn_asks_for_exactly_one_autosave_and_asks_in_the_dark() {
    let own = Saves::new("autosave-turn");
    let (mut game, assets) = bare();
    game.prefs.tip_screens = false;
    let mut m = Machine::new(ScreenId::Campaign);
    let end = l2_game::screens::map::END_TURN_BUTTON;
    drive(&mut m, &mut game, &assets, &[Event::Click { x: end.centre_x(), y: end.y + 4 }]);
    assert!(
        saves::run_pending(&mut m, &game).is_none() && own.files().is_empty(),
        "the button itself does not autosave"
    );

    let before = game.kingdom.turn_count;
    let mut asked = Vec::new();
    let mut at_the_write = None;
    let mut came_round = None;
    for t in 0..l2_game::turn::MAX_TICKS {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        m.update(&mut ctx);
        if let Some(r) = saves::run_pending(&mut m, &game) {
            r.expect("the autosave is written");
            asked.push(t);
            at_the_write = Some(digest(&game.kingdom));
        }
        if came_round.is_none() && game.kingdom.turn_count != before {
            came_round = Some(t);
        }
        if came_round.is_some_and(|r| t > r + 4 * l2_view::fade::PHASES as u32) {
            break;
        }
    }
    let round = came_round
        .unwrap_or_else(|| panic!("the End Turn button ends a turn; stack {:?}", m.ids()));
    assert_eq!(asked.len(), 1, "one turn, one autosave: asked on ticks {asked:?}");
    assert!(asked[0] > round, "the autosave is written after the turn, not before it");
    assert_eq!(own.files(), vec![file("lastturn")], "one turn, one file");
    let back = saves::read("lastturn", Tables::DEFAULT).expect("readable");
    assert_eq!(digest(&back.kingdom), at_the_write.expect("the autosave was asked for"));
    assert_ne!(back.kingdom.turn_count, before, "the turn on disk is the new one");
}
