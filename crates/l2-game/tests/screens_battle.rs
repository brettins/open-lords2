
#[macro_use]
mod common;

use common::*;

use l2_view::Canvas;

/// Group and index are the only things a screen can get wrong that no pixel
/// assertion would notice: a window in the right place, full of the wrong
/// sentence, looks finished. So this reads the shipped `L2.eng` and pins the
/// six indices `Screen_BattlePrompt` and `Screen_BattleResult` use
/// and the two they do not.
///
/// **Group 80 indices 4, 5 and 6 and group 81 indices 1 through 7 are dead.**
///
/// Every `Eng_DrawString` on group 80 in the whole binary is index 0, 1, 2, 3
/// or 7, and every one on group 81 is index 0 or 8. *"The army of"*, *"are
/// Victory graphics for outcomes.
#[test]
fn the_battle_screens_draw_the_original_sentences() {
    let (_game, assets) = world!();
    let eng = &assets.shell;
    use l2_game::screens::battle::{GROUP_BANNER, GROUP_OWNERLESS, GROUP_PROMPT, GROUP_RESULT};

    assert_eq!(eng.text(GROUP_PROMPT, 0), "A Battle is to be fought.");
    assert_eq!(eng.text(GROUP_PROMPT, 1), "Will you take the field?");
    assert_eq!(
        eng.text(GROUP_PROMPT, 2),
        "Your opponent has the choice of whether or not to take the field."
    );
    assert_eq!(eng.text(GROUP_PROMPT, 7), "The Siege commences.");
    assert_eq!(eng.text(GROUP_RESULT, 0), "The Battle is decided.");
    assert_eq!(eng.text(GROUP_RESULT, 8), "The siege is over.");
    assert_eq!(eng.text(GROUP_OWNERLESS, 0), r#""The people.""#);

    for (group, index) in [(GROUP_PROMPT, 4), (GROUP_PROMPT, 5), (GROUP_PROMPT, 6),
                           (GROUP_RESULT, 4), (GROUP_RESULT, 5), (GROUP_RESULT, 7)] {
        assert!(!eng.text(group, index).is_empty(), "{group}.{index} should exist");
    }

    for pair in 0..7usize {
        assert!(!eng.text(GROUP_BANNER, pair * 2).is_empty(), "banner {pair} heading");
        assert!(!eng.text(GROUP_BANNER, pair * 2 + 1).is_empty(), "banner {pair} body");
    }
    assert_eq!(eng.text(GROUP_BANNER, 0), "The Battle is won.");
    assert_eq!(eng.text(GROUP_BANNER, 12), "The conflict is over.");
    assert_eq!(eng.text(GROUP_BANNER, 14), "", "and there is no eighth pair");
}

/// `Screen_BattleMasterRatings` (`0x00421707`) draws
/// `Pl8_DrawFrame(g_miscCtySheet, g_realms[realm].shieldIndex + 8, 0x70, y)`
/// and `Ui_DrawText(&g_playerNames + realm * 0x2C, 0xD8, …)` — `g_localPlayer`
/// above, `DAT_0056D5CC` below. Ours wrote `PLAYER 1` / `PLAYER 2` over shield
/// indices it carried itself, so neither line could ever say a lord's name.
#[test]
fn the_ratings_blocks_name_the_two_lords() {
    use l2_game::screens::ratings::{Ratings, RatingsScreen, BLOCK_Y};
    use l2_game::text::PlayerName;

    let (mut game, assets) = world!();
    let band = |c: &Canvas, top: i32| {
        c.pixels[top as usize * c.width..(top + 140) as usize * c.width].to_vec()
    };

    game.player_names[1] = PlayerName::new("Aethelred");
    game.player_names[2] = PlayerName::new("The Baron");
    game.kingdom.realms[1].shield_index = 1;
    game.kingdom.realms[2].shield_index = 2;
    let mut screen = RatingsScreen::with(Ratings::for_local(1));
    assert_eq!(screen.ratings().realms, (1, 2), "Skirmish_Setup pairs 1 against 2");
    let before = draw(&mut screen, &mut game, &assets);

    game.player_names[2] = PlayerName::new("The Countess");
    let renamed = draw(&mut screen, &mut game, &assets);
    assert_ne!(
        band(&before, BLOCK_Y[1]),
        band(&renamed, BLOCK_Y[1]),
        "the opponent's block does not draw g_playerNames[DAT_0056D5CC]",
    );
    assert_eq!(
        band(&before, BLOCK_Y[0]),
        band(&renamed, BLOCK_Y[0]),
        "renaming the opponent changed the local player's block",
    );

    game.kingdom.realms[2].shield_index = 4;
    let reshielded = draw(&mut screen, &mut game, &assets);
    assert_ne!(
        band(&renamed, BLOCK_Y[1]),
        band(&reshielded, BLOCK_Y[1]),
        "the block ignores g_realms[realm].shieldIndex",
    );
}
