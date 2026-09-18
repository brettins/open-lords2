#![allow(unused_imports)]
use super::*;
use super::setup::*;
use super::rendering::*;
use l2_game::game::{Assets, Game};
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Screen, Transition};
use l2_game::screens::setup::{SetupPage, SetupScreen, NAME_PLATE_X, NAME_PLATE_Y, NAME_X};
use l2_game::text::{FontMetrics, Kind, PlayerName, TextField, NAME_MAX_TYPED, PLAYER_NAME_LEN};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::Tables;
use l2_view::Canvas;

/// The **ablation** for this one is mechanical and was run: deleting the
/// `for name in &game.player_names` loop from `encode_prefix` turns
/// `crates/l2-testkit/tests/encoding/main.rs` red with
/// `Game.player_names — not named in encode`. It did **not**, on the first
/// attempt, because that check matched the comment above the loop; it reads
/// code with the comments stripped now, which is a defect fixed in the shared
/// check here.
#[test]
fn a_typed_name_survives_the_save_and_the_reload() {
    let mut game = Game::new(7);
    let names = ["Aethelred", "The Knight", "The Baron", "The Countess", "The Bishop", "No player"];
    for (slot, n) in game.player_names.iter_mut().zip(names) {
        *slot = PlayerName::new(n);
    }

    let bytes = l2_game::save::encode(&game);
    let back = l2_game::save::decode(&bytes, Tables::DEFAULT).expect("the save reads back");

    for (r, want) in names.iter().enumerate() {
        assert_eq!(back.player_names[r].as_str(), *want, "realm {r}");
    }
}

#[test]
fn a_name_is_thirty_one_bytes_and_truncates_rather_than_growing() {
    let long = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij";
    assert!(long.len() > PLAYER_NAME_LEN);
    let n = PlayerName::new(long);
    assert_eq!(n.bytes().len(), PLAYER_NAME_LEN);
    assert_eq!(n.as_str().len(), PLAYER_NAME_LEN);
    assert_eq!(n.as_str(), &long[..PLAYER_NAME_LEN]);
    assert!(PlayerName::EMPTY.is_empty());
    assert_eq!(PlayerName::from_bytes(*n.bytes()), n);
}

#[test]
fn the_name_field_stops_at_sixteen_characters() {
    let (mut game, assets) = bare();
    let mut m = name_page(&mut game, &assets);
    type_into(&mut m, &mut game, &assets, "ABCDEFGHIJKLMNOPQRSTUVWXYZ");
    assert_eq!(field_of(&m), "ABCDEFGHIJKLMNOP", "sixteen, and no message about the rest");

    let (mut game, assets) = bare();
    let mut m = name_page(&mut game, &assets);
    press(&mut m, &mut game, &assets, Key::Insert);
    press(&mut m, &mut game, &assets, Key::Home);
    type_into(&mut m, &mut game, &assets, "ABCDEFGHIJKLMNOPQRSTUVWXYZ");
    assert_eq!(
        field_of(&m),
        "ABCDEFGHIPlayer1",
        "nine inserted in front of the seven-character seed, and the tenth refused",
    );
    assert_eq!(field_of(&m).len(), NAME_MAX_TYPED);
}


