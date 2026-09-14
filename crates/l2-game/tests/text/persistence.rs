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

/// **A name that does not survive a save is a name a player loses.**
///
/// `docs/agents.md`: six fields have reached `main` written by nothing or
/// dropped by the codec, every one behind a green suite. This drives the real
/// encoder and the real decoder — `l2_game::save::encode`/`decode`, the same
/// pair a player's *Save* button reaches — and it is the reason
/// `l2_game::save::VERSION` moved to 3.
///
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
    // Six distinct names, so a codec that wrote one slot six times, or walked
    // the array in the wrong order, cannot pass.
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

/// The record is 31 bytes with no length prefix, which is `g_playerNames`'
/// own shape and what keeps the encoding fixed-width.
#[test]
fn a_name_is_thirty_one_bytes_and_truncates_rather_than_growing() {
    let long = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij";
    assert!(long.len() > PLAYER_NAME_LEN);
    let n = PlayerName::new(long);
    assert_eq!(n.bytes().len(), PLAYER_NAME_LEN);
    assert_eq!(n.as_str().len(), PLAYER_NAME_LEN);
    assert_eq!(n.as_str(), &long[..PLAYER_NAME_LEN]);
    assert!(PlayerName::EMPTY.is_empty());
    // Round trip through the raw bytes, which is what the codec does.
    assert_eq!(PlayerName::from_bytes(*n.bytes()), n);
}

/// **The two limits are the original's, and sixteen is the one a person meets.**
///
/// `Edit_Begin(&g_options, 0x10, 0xC0, 0)` and `Edit_Commit(&g_options, 0x1F)`
/// are different numbers on purpose: sixteen is what may be typed, thirty-one
/// is how wide the destination is.
#[test]
fn the_name_field_stops_at_sixteen_characters() {
    // **Overwrite**, the default: sixteen characters go in and the rest are
    // dropped in silence.
    let (mut game, assets) = bare();
    let mut m = name_page(&mut game, &assets);
    type_into(&mut m, &mut game, &assets, "ABCDEFGHIJKLMNOPQRSTUVWXYZ");
    assert_eq!(field_of(&m), "ABCDEFGHIJKLMNOP", "sixteen, and no message about the rest");

    // **Insert**, and the limit is a *different* expression of the same
    // sixteen: `Edit_Insert`'s insert branch tests the LENGTH against the
    // limit, not the caret, so a field still holding its seven-character seed
    // accepts only nine more. That asymmetry is the original's and this is
    // where it shows.
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

// ---------------------------------------------------------------------------
// 4. The picture
// ---------------------------------------------------------------------------

