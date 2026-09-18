
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
    assert_eq!(screen.ratings().expect("with() named them").realms, (1, 2), "Skirmish_Setup pairs 1 against 2");
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

/// **`Screen_FrameInput`'s two sheets** (`00420000.c:7500-7518`): `0x2E` goes
/// FORWARD to `0x2F` on either button pressed anywhere, and `0x2F` leaves
/// through `FUN_0042B780`, which writes `g_screenId = 0x1F` and
/// `g_setupPage = 0xC`. Neither sheet has a back door.
///
/// **Ablation:** `crates/l2-game/src/screens/ratings/render.rs`, the
/// `Transition::Replace(ScreenId::Rank)` line — put `Transition::Pop` back and
/// the first assertion is red.
#[test]
fn the_ratings_sheet_goes_forward_to_the_rank_sheet_and_that_goes_to_page_12() {
    use l2_game::screens::ratings::{RankScreen, RatingsScreen};
    use l2_game::screens::setup::SetupPage;

    let (mut game, assets) = world!();
    let mut sheet = RatingsScreen::new();
    for e in [Event::Click { x: 20, y: 400 }, Event::RightClick { x: 300, y: 300 }] {
        assert_eq!(
            send(&mut sheet, &mut game, &assets, e),
            Transition::Replace(ScreenId::Rank),
            "0x2E's arm is `g_screenId = '/'` on both buttons",
        );
    }
    let mut rank = RankScreen::new();
    for e in [Event::Click { x: 1, y: 1 }, Event::RightClick { x: 639, y: 479 }] {
        assert_eq!(
            send(&mut rank, &mut game, &assets, e),
            Transition::Replace(ScreenId::Setup(SetupPage::Skirmish)),
            "FUN_0042B780 writes g_setupPage = 0xC",
        );
    }
}

/// `FUN_0042D179` (`0x0042D179`) fills the book with ten scores out of
/// `DAT_004D49C8` and ten names out of `L2.eng` group 7 at the indices in
/// `DAT_004D49F0`; `FUN_0042D216`'s missing-file limb leaves exactly that
/// standing, and nothing here writes `score.dat`. The sheet then draws the ten
/// names at x 200 and the twelfth heading out of group 38.
///
/// **Ablation:** `crates/l2-game/src/screens/ratings/master.rs`, the
/// `DEFAULT_NAME_ENG` lookup in `seed_defaults` — index it with `i` instead and
/// the *"The Baron"* assertion is red, because row 0 is group 7 index **2**.
#[test]
fn the_rank_sheet_is_the_default_top_ten_out_of_the_executable() {
    use l2_game::screens::ratings::{RankScreen, DEFAULT_NAME_ENG, DEFAULT_SCORES, GROUP_LORDS};

    let (mut game, assets) = world!();
    assert_eq!(assets.shell.text(GROUP_LORDS, DEFAULT_NAME_ENG[0]), "The Baron");
    assert_eq!(assets.shell.text(38, 11), "Rank of Supreme commander");
    assert_eq!(assets.shell.text(37, 5), "The skirmish masters!!");
    assert_eq!(DEFAULT_SCORES[0], 500);

    let mut rank = RankScreen::new();
    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        rank.update(&mut ctx);
    }
    let book = &game.battle_master;
    assert_eq!(book.top.len(), 10);
    assert_eq!(book.top[0].name, "The Baron");
    assert_eq!(book.top[9].score, 10);

    let canvas = draw(&mut rank, &mut game, &assets);
    assert!(
        find_body(&canvas, &assets, "The Baron", l2_game::shell::font::TEXT).is_some(),
        "the ten rows draw DAT_0051FBC0 + row * 0x20",
    );
}

/// **`FUN_0042CC4A`** (`0x0042CC4A`): the rank is `average / 100` plus a bonus
/// from the battles-played ladder, capped by that ladder's other column, and
/// the two scores are then inserted into the ten-row table in descending
/// order. The early `return` on a duplicate name-and-score skips **every**
/// insert, including the player's own five.
///
/// **Ablation:** `crates/l2-game/src/screens/ratings/master.rs`, the
/// `.clamp(0, cap)` in `record` — drop the cap and the first assertion is red,
/// because two battles cap the rank at 0 whatever the average is.
#[test]
fn the_battle_masters_rank_is_capped_by_battles_played() {
    use l2_game::screens::ratings::BattleMaster;

    let mut book = BattleMaster::default();
    book.top = (0..10).map(|i| l2_game::screens::ratings::Entry {
        name: format!("L{i}"),
        score: (10 - i) * 100,
    }).collect();

    book.record("Me", "Them", 950, 100);
    assert_eq!(book.rank, 0, "battles < 3 caps local_14 at 0");
    assert_eq!((book.top[1].name.as_str(), book.top[1].score), ("Me", 950), "950 beats row 1's 900");
    assert_eq!(book.mine[0], 950, "DAT_0051FD28 takes the local score only");

    for _ in 0..10 {
        book.record("Me", "Them", 950, 100);
    }
    assert_eq!(book.battles, 11, "DAT_00568988 counts every call");
    assert_eq!(book.total, 950 * 11, "DAT_00568214 sums the local score");
    assert_eq!(book.rank, 5, "average 950 / 100 = 9, capped at local_14 = 5 for 11 battles");
    assert_eq!(book.mine[1], 0, "the duplicate name-and-score returns before every insert");
}
