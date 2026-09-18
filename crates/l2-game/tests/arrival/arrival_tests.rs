#![allow(unused_imports)]
use super::*;

use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::message::{category, Record};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{map, message as scroll};
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::movement::{self, Routing};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_view::campaign;

/// `County_GreetArmy`'s 20…49 % rung, `L2.eng` 133, in the envoy's panel.
#[test]
fn marching_into_a_neutral_county_opens_its_greeting_in_the_countys_own_words() {
    let (mut g, a, mut m) = world(0);
    let y = row_showing(&[31, 33]);
    let id = army_at(&mut g, 1, 300, (31, y));

    order(&mut m, &mut g, &a, id, (33, y));
    let r = next_letter(&mut m, &mut g, &a);

    assert_eq!(g.kingdom.campaign.units.get(id).map(|u| u.county), Some(2), "it is over the border");
    assert_eq!((r.to, r.from, r.group, r.category, r.county), (1, 0, 133, 2, 2));
    assert_eq!(
        says(&mut g, &a, &r),
        "The presence of your men at arms in our county is unacceptable. Remove your troops now, or face the consequences!"
    );
}

#[test]
fn an_army_walking_inside_a_turn_is_greeted_too() {
    let (mut g, a, mut m) = world(0);
    let y = row_showing(&[31, 33]);
    let id = army_at(&mut g, 1, 300, (31, y));
    order(&mut m, &mut g, &a, id, (33, y));
    let before = g.kingdom.turn_count;
    send(&mut m, &mut g, &a, Event::KeyDown(Key::letter('e')));

    let r = next_letter(&mut m, &mut g, &a);
    // `Game::turn` is the crate's; the ablation above is what shows the
    // letter came through the turn's sweep and not the idle frame's.
    assert_eq!(g.kingdom.turn_count, before, "the letter came before the turn ended");
    assert_eq!((r.to, r.group, r.category, r.county), (1, 133, 2, 2));
}

#[test]
fn walking_into_your_own_county_says_nothing() {
    let (mut g, a, mut m) = world(1);
    let y = row_showing(&[31, 33]);
    let id = army_at(&mut g, 1, 300, (31, y));
    order(&mut m, &mut g, &a, id, (33, y));
    assert_eq!(until_still(&mut m, &mut g, &a), None);
    assert_eq!(g.kingdom.campaign.units.get(id).map(|u| u.county), Some(2));
    assert!(g.messages.is_empty(), "and nothing is waiting either");
}

/// **A lord writes to the county he is marching on, and not to the one he
/// marches through.** The Countess's lord 3 crosses the player's county 2 on
/// his way to county 1: one letter, at the second border, taunt 9 of group 170.
#[test]
fn a_lord_marching_on_your_county_writes_to_you_and_passing_through_does_not() {
    let (mut g, a, mut m) = world(1);
    g.kingdom.realms[2].voice_rotation = 1;
    let lord = army_at(&mut g, 2, 400, (36, 20));
    lord_marches(&mut g, lord, (29, 20));

    let r = next_letter(&mut m, &mut g, &a);
    assert_eq!(g.kingdom.campaign.units.get(lord).map(|u| u.county), Some(1), "at the second border");
    assert_eq!((r.to, r.from, r.group, r.category, r.county), (1, 2, 170, category::LETTER, 1));
    assert_eq!(r.variant, 9, "lord 3, rotation 1: 3 * 4 + 1 - 4");
    assert_eq!(g.kingdom.realms[2].voice_rotation, 2, "one letter, one step of the rotation");
    assert_eq!(
        says(&mut g, &a, &r),
        "\"The presence of your troops in my lands is most unwelcome.  It is time I removed them!\""
    );

    close(&mut m, &mut g, &a);
    assert_eq!(until_still(&mut m, &mut g, &a), None, "and nothing more on the way");
}

/// Ablation: delete the rotation's increment in `enter_county` and the last
/// assertion goes red.
#[test]
fn marching_on_a_lords_county_writes_to_him_turns_your_rotation_and_shows_you_nothing() {
    let (mut g, a, mut m) = world(2);
    let y = row_showing(&[31, 33]);
    let id = army_at(&mut g, 1, 300, (31, y));
    order(&mut m, &mut g, &a, id, (33, y));
    assert_eq!(until_still(&mut m, &mut g, &a), None);
    assert!(g.messages.is_empty());
    assert_eq!(g.kingdom.realms[1].voice_rotation, 1, "the letter was posted, to realm 2");
}


/// **Taking a county opens the capture letter**, category `0x0D` — the one the
/// capture films are wired to. A neutral county of happiness 10 greets the army
/// with a welcome and then surrenders its town without a fight; the player held
/// one county at a peak of one, so it is `L2.eng` 117.
#[test]
fn taking_a_neutral_town_opens_the_capture_letter_after_the_greeting() {
    let (mut g, a, mut m) = world(0);
    g.kingdom.counties[2].happiness = 10;
    let y = row_showing(&[31, 33]);
    g.kingdom.campaign.map.set_flags(33, y, flags::CASTLE);
    let id = army_at(&mut g, 1, 300, (31, y));

    order(&mut m, &mut g, &a, id, (33, y));
    let greeting = next_letter(&mut m, &mut g, &a);
    assert_eq!((greeting.group, greeting.category), (131, 2), "happiness 10 is below 30: a welcome");
    close(&mut m, &mut g, &a);

    let r = next_letter(&mut m, &mut g, &a);
    assert_eq!(g.kingdom.counties[2].owner, 1, "the county is the player's");
    assert_eq!((r.to, r.from, r.group, r.category, r.county, r.spare), (1, 0, 117, category::CAPTURE, 2, 0));
    assert_eq!(r.shape(), l2_game::message::Shape::Capture);
    assert_eq!(g.kingdom.realms[1].peak_counties, 2);
    assert_eq!(
        says(&mut g, &a, &r),
        "Bravo!! The county has fallen to our troops.   You have made an excellent start in your bid to become the King."
    );
}

/// **A lord taking your county writes twice**: his taunt at the border, then
/// your loss at the town — `L2.eng` 170 and 115, in that order. County 1 has
/// thirty people, too few to raise a defence, so it falls without a battle.
#[test]
fn a_lord_taking_your_county_sends_his_taunt_and_then_word_of_your_loss() {
    let (mut g, a, mut m) = world(2);
    g.kingdom.counties[4].owner = 1;
    g.kingdom.counties[1].population = 30;
    g.kingdom.campaign.map.set_flags(29, 20, flags::CASTLE);
    let lord = army_at(&mut g, 2, 400, (33, 20));
    lord_marches(&mut g, lord, (29, 20));

    let taunt = next_letter(&mut m, &mut g, &a);
    assert_eq!((taunt.group, taunt.from, taunt.county), (170, 2, 1));
    close(&mut m, &mut g, &a);

    let r = next_letter(&mut m, &mut g, &a);
    assert_eq!(g.kingdom.counties[1].owner, 2);
    assert_eq!((r.to, r.from, r.group, r.category, r.county), (1, 2, 115, category::NOTICE, 1));
    assert_eq!(says(&mut g, &a, &r), "Our county is lost! The town was overrun by enemy troops.");
}

/// **A capture won in battle is told from the end of the battle.** The lord's
/// thousand beat a neutral county's hundred-man militia in a battle nobody is
/// asked about, and the player hears of it: `L2.eng` 116, from
/// `Battle_ReturnToCampaign`'s `County_ChangeOwner`.
#[test]
fn a_lord_winning_a_neutral_county_in_battle_is_reported_to_you() {
    let (mut g, a, mut m) = world(0);
    g.kingdom.counties[2].population = 400;
    g.kingdom.campaign.map.set_flags(33, 20, flags::CASTLE);
    let lord = army_at(&mut g, 2, 1_000, (36, 20));
    lord_marches(&mut g, lord, (33, 20));

    let r = next_letter(&mut m, &mut g, &a);
    assert_eq!(g.kingdom.counties[2].owner, 2, "the militia lost");
    assert_eq!((r.to, r.from, r.group, r.category, r.county), (1, 2, 116, category::NOTICE, 2));
    assert_eq!(
        says(&mut g, &a, &r),
        "This poor, defenseless county has been ruthlessly occupied by one of your enemies."
    );
}


/// **The words are the player's own `L2.eng`**, and our transcription agrees
/// with it string for string — which is what makes it a fallback
/// rewrite. `CLAUDE.md` rule 6.
#[test]
fn every_arrival_letter_is_the_players_own_words_and_our_transcription_is_them() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no L2.eng");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let mut strings = 0;
    for (group, words) in l2_game::arrival::TEXT {
        for (i, want) in words.iter().enumerate() {
            assert_eq!(assets.shell.text(*group as usize, i), *want, "L2.eng {group}/{i}");
            assert_eq!(l2_game::arrival::words(&assets.shell, *group, i), *want);
            strings += 1;
        }
        assert_eq!(assets.shell.text(*group as usize, words.len()), "", "group {group} has no more");
    }
    assert_eq!(strings, 18 * 2 + 17, "eighteen two-string groups and group 170's seventeen");
}

