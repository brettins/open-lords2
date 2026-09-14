#![allow(unused_imports)]
use super::*;
use super::england_fixture::*;
use super::turn_execution::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::scenario;
use l2_kingdom::tables::{Tables, Weather};
use l2_mods::Platform;

/// **A loaded save keeps its diplomatic matrix**, and this is the test that
/// could not have been written against the turn-one fixture.
///
/// `l2_formats::save::Realm` did not read `+0x84 … +0xE3` at all, so
/// `scenario::from_save` re-ran `Diplo_Init` on every load and a mid-game file
/// came back with every alliance and every grudge gone — which a player would
/// have experienced as the AI forgetting a war it was fighting.
///
/// **The turn-one fixture cannot fail this.** `Diplo_Init` opens an in-play AI
/// realm at 5 and England turn one *is* 5 everywhere, so asserting against it
/// would have proved nothing. The mid-game saves the player produced are the
/// only oracle: `siege-lastturn.sav` carries **18**, thirteen turns of the
/// +1-a-turn heal, and `Diplo_Init` would put it back to 5.
///
/// `docs/decisions.md` C83.
#[test]
fn a_mid_game_save_keeps_the_standing_it_was_saved_with() {
    let save = l2_testkit::fixture!("siege-lastturn.sav");
    let game = scenario::from_save(&save, Tables::DEFAULT).expect("the save loads");

    // Realm 2's view of realm 3, which the file holds at 18.
    let standing = game.kingdom.realms[2].pairs[3].standing;
    assert_ne!(
        standing, 5,
        "the standing is Diplo_Init's opening value, so the pair block was not carried",
    );
    assert_eq!(standing, 18, "the file's own byte, thirteen turns of healing above the opening 5");
}

/// **A loaded save carries the lords' names**, and before this they were
/// dropped on the floor.
///
/// `g_saveBlocks[2] = {0x00553D50, 264}` is the six-slot player table and the
/// name is each slot's `+0x04`; `l2_formats::save` did not read it, so
/// `Game::player_names` came back empty from every load and
/// `screens::message::lord_name` fell through to `L2.eng` group 7 and then to
/// `REALM n`.
///
/// **The human is the one the fallback cannot rescue.** `Realms_AssignLords`
/// (`0x0049CAAA`) writes `lord = 0` for a realm somebody is playing and fills
/// it only for the AIs, so group 7 indexed by the lord gives the human
/// *index 0* — `"No player"`. That is what the court and the diplomacy heading
/// said about the person reading them. The AI names cannot show the difference
/// at all: the fallback produces the same four titles the file holds, which is
/// why this asserts on the person's name and on the group-7 set separately.
#[test]
fn a_loaded_save_carries_the_lords_names() {
    let dir = install!();
    let game = game!();
    let assets = Assets::load(&platform(&dir).vfs).expect("assets load");

    let me = game.player as usize;
    let mine = game.player_names[me].as_str();
    assert!(!mine.is_empty(), "realm {me} is the person playing and has no name");

    // `Eng_Seek(7, lord)`'s five strings — "No player" and the four titles.
    let titles: Vec<String> = (0..5).map(|i| assets.shell.text(7, i).to_string()).collect();
    assert!(!titles[0].is_empty(), "L2.eng group 7 is missing from this install");
    assert_ne!(
        mine.trim_end(),
        titles[0],
        "the human's name is group 7 index 0, which is what the fallback produces \
         when nothing filled g_playerNames",
    );

    // Every AI in play is named, distinctly, and out of group 7 — the lord id
    // in its own realm record is the index, which is the cross-check the save
    // bytes and `L2.eng` can make against each other.
    let mut seen: Vec<String> = Vec::new();
    for id in 1..l2_kingdom::realm::MAX_REALMS {
        let realm = &game.kingdom.realms[id];
        if id == me || !realm.in_play {
            continue;
        }
        let name = game.player_names[id].as_str();
        assert!(!name.is_empty(), "realm {id} is in play and unnamed");
        assert_eq!(
            name.trim_end(),
            titles[(realm.lord as usize).min(4)],
            "realm {id}'s name is not L2.eng 7/{} — the lord its own record names",
            realm.lord,
        );
        assert!(!seen.contains(&name), "two realms share the name {name:?}");
        seen.push(name);
    }
    assert_eq!(seen.len(), 4, "England turn one has four AI lords");
    eprintln!("realm {me} is {mine:?}; the AI lords are {seen:?}");
}

