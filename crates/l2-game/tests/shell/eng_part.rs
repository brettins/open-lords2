#![allow(unused_imports)]
use super::*;
use super::navigation::*;
use super::conquest::*;
use super::font_part::*;
use super::glyph::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, Screen, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::conquest::ConquestScreen;
use l2_game::screens::setup::{self, SetupPage};
use l2_game::screens::court::CourtScreen;
use l2_game::screens::ratings::RatingsScreen;
use l2_game::shell::{font, Eng};
use l2_game::Game;
use l2_view::Canvas;

// ------------------------------------------------------- against the install

#[test]
fn l2_eng_says_what_every_screen_in_the_table_claims_it_says() {
    let Some(e) = eng() else {
        eprintln!("skipping: no game install");
        return;
    };
    // The front end. If these three are right, page 1 is the front end and
    // `docs/screens-county.md`'s old row for 0x1C was wrong.
    assert_eq!(e.get(11, 0), Some("Lords of the Realm 2"));
    assert_eq!(e.get(11, 2), Some("Single player"));
    assert_eq!(e.get(11, 4), Some("Exit game"));
    assert_eq!(e.get(11, 5), Some("Your options"));
    assert_eq!(e.get(11, 6), Some("Play Now!"));

// The conquest screen, which 0x1C is.
    assert_eq!(e.get(36, 0), Some("Congratulations!!"));
    assert_eq!(e.get(36, 1), Some("You have conquered"));
    assert_eq!(e.get(36, 4), Some("You have lost."));

    // The custom game's twelve options and the values they index.
    assert_eq!(e.get(102, 0), Some("Advanced Farming"));
    assert_eq!(e.get(102, 11), Some("Fight?"));
    assert_eq!(e.get(103, 0), Some("off"));
    assert_eq!(e.get(103, 45), Some("all"));
    assert_eq!(e.group(103).len(), 46, "the twelve runs cover 45 of these");

    // **And every index the last seven screens draw is a string that exists.**
    //
    // This walked `SHELLS` until the table emptied. The claim it was making -
    // *a group and index this engine draws must be a group and index the
    // player's own `L2.eng` has* - is worth more than the table it walked, so
    // it now names the seven modules' own constants. Two of those constants
// are the finding: group 70 index 1 (*"Arms"*) and
    // group 37 index 1 (*"Before"*) exist in the file and are drawn by
// **nothing in the binary**, so they are asserted present here
    // and appear in no painter.
    use l2_game::screens::{about, court, diplomacy, ratings, supplies};

    for i in [about::TITLE, about::VERSION, about::COPYRIGHT] {
        assert!(e.get(about::GROUP, i).is_some(), "about: group 59 has no {i}");
    }
    assert_eq!(e.get(about::GROUP, about::TITLE), Some("Lords 2."));
    assert_eq!(e.group(about::GROUP).len(), 3, "group 59 is three strings and no more");

    for i in [
        court::GOLD,
        court::ARMS,
        court::IRON,
        court::STONE,
        court::WOOD,
        court::COURT_OF,
        court::NOBLES,
        court::WAGES,
        court::TAX_EXPECTED,
    ] {
        assert!(e.get(court::GROUP, i).is_some(), "court: group 70 has no {i}");
    }
    assert_eq!(e.get(court::GROUP, court::COURT_OF), Some("Court of"));
    assert_eq!(e.get(court::GROUP, court::ARMS), Some("Arms"), "and nothing draws it");

// **Every row the screen can draw, taken from the screen
    // list beside it.** This used to name eight constants belonging to a stub;
    // the stub was replaced by the real screen in the same merge and its
    // constants went with it. Asking `Menu::rows()` means a menu that gains a
    // row is covered here without anyone remembering — the difference between a
    // test that checks the screen and one that checks a copy of what it was.
    let rows: std::collections::BTreeSet<usize> = [
        diplomacy::Menu::NoAlly,
        diplomacy::Menu::Allied,
        diplomacy::Menu::AlliedElsewhere,
        diplomacy::Menu::Dispatched,
    ]
    .iter()
    .flat_map(|m| m.rows().iter().copied())
    .collect();
    assert!(rows.len() >= 8, "only {} distinct rows across the four menus", rows.len());
    for i in rows {
        assert!(e.get(diplomacy::GROUP, i).is_some(), "diplomacy: group 72 has no {i}");
    }
    assert_eq!(e.get(diplomacy::GROUP, 0), Some("Diplomacy."), "which is the screen's name");

    for i in [
        supplies::TITLE,
        supplies::FROM,
        supplies::TO,
        supplies::GRAIN,
        supplies::SHEEP,
        supplies::CATTLE,
        supplies::DISPATCH,
        supplies::PROMPT,
    ] {
        assert!(e.get(supplies::GROUP, i).is_some(), "supplies: group 33 has no {i}");
    }
    assert_eq!(e.get(supplies::GROUP, supplies::SHEEP), Some("Sheep"), "and nothing draws it");

    for i in [ratings::HEADING, ratings::BEFORE, ratings::KILLED, ratings::KILLS, ratings::SCORED]
    {
        assert!(e.get(ratings::GROUP, i).is_some(), "ratings: group 37 has no {i}");
    }
    assert_eq!(e.get(ratings::GROUP, ratings::BEFORE), Some("Before"), "and nothing draws it");

    // **The compose dialogs' own strings, checked against the words and not
    // only for existence.** `docs/draws.md` §6: the armoury was filed under
    // group 16 and group 16 index 6 exists, so an existence check passed on a
    // screen that was wrong. Every index below is a literal argument to an
    // `Eng_DrawString` in `Diplo_DrawGiftGold` (`0x00417960`),
    // `Diplo_DrawLetter` (`0x00417AEF`) or `Diplo_DrawCountyRequest`
    // (`0x00417CEF`), and the sentence it makes is the sentence the dialog is.
    assert_eq!(e.get(diplomacy::GROUP, diplomacy::GIFT_TO), Some("Send gift of gold to"));
    assert_eq!(e.get(diplomacy::GROUP, diplomacy::LAST_GIFT), Some("Last gift was"));
    assert_eq!(e.get(diplomacy::GROUP, diplomacy::GIFT_OF), Some("Gift of"));
    assert_eq!(e.get(diplomacy::GROUP, diplomacy::DISPATCH), Some("Dispatch ?"));
    // 72/11 + kind - 1, the four letters, in `g_diploKind` order.
    let letters: Vec<&str> =
        (0..4).map(|k| e.get(diplomacy::GROUP, diplomacy::LETTER_BASE + k).unwrap()).collect();
    assert_eq!(
        letters,
        vec![
            "Give a compliment to",
            "Insult",
            "Ask for an alliance with",
            "End alliance with"
        ],
        "g_diploKind 1..=4"
    );
    // 72/15 + k, and the prompt pair that follows it.
    assert_eq!(e.get(diplomacy::GROUP, diplomacy::REQUEST_BASE), Some("Plead for help from"));
    assert_eq!(
        e.get(diplomacy::GROUP, diplomacy::REQUEST_BASE + 1),
        Some("Plan strategic attack with")
    );
    for k in 0..2 {
        assert!(e.get(diplomacy::GROUP, diplomacy::REQUEST_PROMPT + k).is_some());
        assert!(e.get(diplomacy::GROUP, diplomacy::REQUEST_PICKED + k).is_some());
    }
    // `Ui_DrawCount(value, 0, …)`: group 8's crown pair, singular then plural.
    // The **plural** is what a zero amount takes, which is the half a
    // reimplementation gets wrong.
    assert_eq!(e.get(l2_game::shell::COUNT_NOUN_GROUP, diplomacy::CROWN_NOUN), Some("Crown."));
    assert_eq!(e.get(l2_game::shell::COUNT_NOUN_GROUP, diplomacy::CROWN_NOUN + 1), Some("Crowns."));

    // **The front end, page by page.** Every one of these is the literal
    // argument to a `Ui_DrawCentred` or `Eng_DrawString` in the thirteen
    // painters, so the words are the check and not the existence.
    assert_eq!(e.get(setup::GROUP, 1), Some("\"The siege is on\""), "page 1's subtitle");
    let title: Vec<&str> =
        setup::TITLE_ITEMS.iter().map(|&i| e.get(setup::GROUP, i).unwrap()).collect();
    assert_eq!(
        title,
        vec!["Single player", "Multiple players", "Lords of Magic?", "Exit game"],
        "page 1, in FUN_0041EAA3's own order"
    );
    let options: Vec<&str> =
        setup::OPTION_ITEMS.iter().map(|&i| e.get(setup::GROUP, i).unwrap()).collect();
    assert_eq!(
        options,
        vec!["Play Now!", "Load a game", "Skirmish!", "Custom game", "Back"],
        "page 2, in FUN_0041ECE6's own order"
    );
    assert_eq!(e.get(setup::GROUP, 10), Some("Choose your title and your shield."), "page 4");
    // Page 4's two buttons, then pages 7 and 8's four, from FUN_0041F01F and
    // FUN_0041F6C7 / FUN_0041F77A.
    assert_eq!(e.get(setup::GROUP, 9), Some("Back"));
    assert_eq!(e.get(setup::GROUP, 11), Some("Continue"));
    let buttons: Vec<&str> =
        setup::CUSTOM_BUTTONS.iter().map(|&(_, i)| e.get(setup::GROUP, i).unwrap()).collect();
    assert_eq!(buttons, vec!["Cancel", "Start", "Defaults", "Load"], "pages 7 and 8");
    // Page 5 and page 6 share group 39 and differ only in which pair they draw.
    assert_eq!(e.get(setup::GROUP_EXPANSION, 0), Some("Expansion pack installed, choose:-"));
    assert_eq!(e.get(setup::GROUP_EXPANSION, 4), Some("Original campaign"), "page 5");
    assert_eq!(e.get(setup::GROUP_EXPANSION, 5), Some("The new campaign"), "page 5");
    assert_eq!(e.get(setup::GROUP_EXPANSION, 1), Some("Full game"), "page 6, host only");
    assert_eq!(e.get(setup::GROUP_EXPANSION, 2), Some("Skirmish"), "page 6, host only");
    assert_eq!(
        e.get(setup::GROUP_EXPANSION, 3),
        Some("Please wait while the session creator decides what type of game to play."),
        "page 6 as a joiner sees it - the arm this engine does not draw"
    );
    // Page 3 and page 13, and **the string page 3 used to draw and must not**:
    // 40/8 belongs to the skirmish file box, 40/5 is page 3's heading, and the
    // status line under page 3's list is 40/2, drawn only mid-load.
    assert_eq!(e.get(setup::GROUP_FILE, 5), Some("Loading a game."), "page 3's heading");
    assert_eq!(e.get(setup::GROUP_FILE, 2), Some("Loading game. Please wait."), "and its status");
    assert_eq!(e.get(setup::GROUP_FILE, 6), Some("Click on a skirmish file to load."), "page 13");
    assert_eq!(e.get(setup::GROUP_FILE, 8), Some("Right click to exit."), "page 13, not page 3");
    // Pages 11, 12 and 13's three captions, and the one this engine cannot
    // reach: 39 "Norm." replaces 38 "Cust." while DAT_0056899C is set.
    assert_eq!(e.get(setup::GROUP, 36), Some("Go"));
    assert_eq!(e.get(setup::GROUP, 37), Some("Back"));
    assert_eq!(e.get(setup::GROUP, 38), Some("Cust."));
    assert_eq!(e.get(setup::GROUP, 39), Some("Norm."), "the arm on DAT_0056899C");
    // Page 10, whose five indices are 16, 17, 18, 48, 49 - not 16..=20.
    assert_eq!(e.get(setup::GROUP, 16), Some("No Lords of the Realm CD"));
    assert_eq!(e.get(setup::GROUP, 48), Some("Siege Pack"), "drawn in colour 1, not 0x3F");
    assert!(e.get(setup::GROUP, 49).unwrap().contains("Siege pack CD"));
    // The scenario list's rows come out of group 101, sixty map names.
    assert_eq!(e.get(setup::GROUP_MAPS, 0), Some("England"));
    assert_eq!(e.group(setup::GROUP_MAPS).len(), setup::MAP_COUNT, "one name per map slot");
}

#[test]
fn the_option_runs_name_the_values_a_player_of_the_game_would_recognise() {
    let Some(e) = eng() else {
        eprintln!("skipping: no game install");
        return;
    };
    let values = |i: usize| -> Vec<&str> {
        (0..setup::OPTION_COUNT[i])
            .map(|v| e.get(103, setup::OPTION_BASE[i] + v).unwrap())
            .collect()
    };
    assert_eq!(values(0), vec!["off", "on"], "Advanced Farming");
    assert_eq!(values(2), vec!["two", "three", "four", "five"], "Nobles - never one");
    assert_eq!(values(4), vec!["easy", "normal", "hard", "impossible"], "Difficulty");
    assert_eq!(values(8), vec!["100", "500", "1000", "2500", "5000"], "Crowns");
    assert_eq!(values(11), vec!["humans", "all"], "Fight?");
    // The one string nothing reaches.
    assert_eq!(e.get(103, 4), Some("one"));
}

