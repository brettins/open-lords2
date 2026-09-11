//! **The letters an army's arrival posts, played through the screens.**
//!
//! ```text
//! cargo test -p l2-game --test arrival
//! ```
//!
//! A player, on build `73DF34969`: *"county did not give me a message when I
//! moved an army into it."* Every test here puts the armies a scenario would
//! place, gives the player's orders with two clicks on the campaign map, lets
//! the frame loop walk them — `turn::tick_units_only`, which `MapScreen::update`
//! runs — and asserts what the message scroll then says, in the words the
//! window draws. Nothing here calls `Msg_Enqueue`'s stand-in or writes the ring.
//!
//! **Animations are off in every world**, because with them on
//! `Msg_DrawWindow` plays `cap_cty*.smk` in place of a category `0x0D` window
//! and dismisses it; that branch is the films' subject.

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

/// County 1 is `x < 32`, county 2 is the two columns `32` and `33`, county 3 the
/// rest. The 1 | 2 border is inside the band the campaign screen opens on, so
/// the player's clicks never scroll — the same choice `tests/castles.rs` makes —
/// and county 2 is narrow so that a lord's march across it is a few tiles long,
/// as every march `Unit_OrderMove` is given in these suites is.
const WEST: usize = 32;
const EAST: usize = 34;

// ---------------------------------------------------------------------- setup

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn pixel(x: u8, y: u8) -> Option<(i32, i32)> {
    let probe = map::MapScreen::new();
    campaign::tile_centre(probe.viewport(), probe.zoom(), x as usize, y as usize)
}

fn click_tile(m: &mut Machine, g: &mut Game, a: &Assets, at: (u8, u8)) {
    let (x, y) = pixel(at.0, at.1).expect("a tile on the opening screen");
    send(m, g, a, Event::Click { x, y });
}

/// A row on which every one of these columns is on screen.
fn row_showing(columns: &[u8]) -> u8 {
    (0..64u8)
        .find(|&y| columns.iter().all(|&x| pixel(x, y).is_some()))
        .expect("a row showing every column")
}

/// Three counties in a row and three more that hold no ground, so a capture's
/// share and `g_countyCount − 1` are not the same number. Realm 1 is the player
/// and holds county 1; realm 2, the Countess's lord 3, holds county 3; county
/// 2's owner is the test's.
fn world(county_2: u8) -> (Game, Assets, Machine) {
    let mut g = Game::new(0xA441);
    g.prefs.tip_screens = false;
    g.prefs.animations = false;
    g.kingdom.set_county_count(6);
    for id in 1..=6usize {
        let c = &mut g.kingdom.counties[id];
        c.population = 1_000;
        c.happiness = 77;
        c.grain = 60_000;
        c.herd = 400;
    }
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[2].owner = county_2;
    g.kingdom.counties[3].owner = 2;
    for (id, neighbours) in [(1usize, vec![2u8]), (2, vec![1, 3]), (3, vec![2])] {
        let c = &mut g.kingdom.counties[id];
        c.neighbour_count = neighbours.len() as u8;
        for (i, n) in neighbours.into_iter().enumerate() {
            c.neighbours[i] = n;
        }
    }
    for realm in 1..=2usize {
        let r = &mut g.kingdom.realms[realm];
        r.in_play = true;
        r.strength = 5;
        r.gold = 20_000;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[2].lord = 3;
    l2_kingdom::conquest::recount_realm_counties(&g.kingdom.counties, &mut g.kingdom.realms);
    for realm in 1..=2usize {
        // What `Game_SetupRealmsAndCounties` and every capture since would
        // have left: nobody here has lost ground.
        g.kingdom.realms[realm].peak_counties = g.kingdom.realms[realm].county_count;
    }

    let mut m = CampaignMap::empty();
    for i in 0..MAP_TILES {
        let x = i % MAP_DIM;
        m.county[i] = if x < WEST { 1 } else if x < EAST { 2 } else { 3 };
    }
    g.kingdom.campaign.map = m;
    g.player = 1;
    g.selected = 1;
    (g, Assets::placeholder(), Machine::new(ScreenId::Campaign))
}

fn army_at(g: &mut Game, owner: u8, men: i32, at: (u8, u8)) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, at.0, at.1);
    u.men = men;
    u.troops[TroopType::Peasant.index()] = men;
    u.county = g.kingdom.campaign.map.county_at(at.0, at.1);
    u.home_county = u.county;
    u.owner_is_human = owner == 1;
    g.kingdom.campaign.units.spawn(u).expect("a free slot")
}

/// The player's two clicks: the army, then where it is to go.
fn order(m: &mut Machine, g: &mut Game, a: &Assets, id: usize, to: (u8, u8)) {
    let at = g.kingdom.campaign.units.get(id).map(|u| (u.x, u.y)).expect("the army");
    click_tile(m, g, a, at);
    click_tile(m, g, a, to);
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving), "the order was taken");
}

/// A lord's march, which no click gives: `Unit_OrderMove`, as the AI calls it.
fn lord_marches(g: &mut Game, id: usize, to: (u8, u8)) {
    movement::order_move(&g.kingdom.campaign.map, &mut g.kingdom.campaign.units, id, to, Routing::Direct)
        .expect("open ground the whole way");
}

/// Run frames until the scroll is up, and return what it shows.
fn next_letter(m: &mut Machine, g: &mut Game, a: &Assets) -> Record {
    for _ in 0..4_000 {
        if m.top_id() == Some(ScreenId::Message) {
            return *g.messages.open().expect("the scroll is up, so a record is open");
        }
        tick(m, g, a);
    }
    panic!("no letter came; the screen is {:?}", m.top_id());
}

/// Run frames until nothing is walking, and say whether a letter came.
fn until_still(m: &mut Machine, g: &mut Game, a: &Assets) -> Option<Record> {
    for _ in 0..4_000 {
        if m.top_id() == Some(ScreenId::Message) {
            return g.messages.open().copied();
        }
        if !g.kingdom.campaign.units.iter().any(|(_, u)| u.moving) {
            for _ in 0..8 {
                tick(m, g, a);
            }
            return g.messages.open().copied();
        }
        tick(m, g, a);
    }
    panic!("the march never finished");
}

/// **The right button closes the scroll, whatever it is** — `Msg_HandleInput`.
fn close(m: &mut Machine, g: &mut Game, a: &Assets) {
    send(m, g, a, Event::RightClick { x: 320, y: 240 });
    assert_ne!(m.top_id(), Some(ScreenId::Message), "the scroll closed");
}

/// What the window says: [`scroll::body`], the call its painter makes.
fn says(g: &mut Game, a: &Assets, r: &Record) -> String {
    let ctx = Ctx { game: g, assets: a };
    scroll::body(&ctx, r)
}

// ----------------------------------------------------------- crossing a border

/// **The report, closed.** Three hundred men into a neutral county of a
/// thousand contented people is thirty per cent of it, and the county says so:
/// `County_GreetArmy`'s 20…49 % rung, `L2.eng` 133, in the envoy's panel.
///
/// Ablation: delete `crate::arrival::post(game, &moved.posted)` in
/// `turn::tick_units_only` and no letter comes.
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

/// **The same letter inside a turn.** An order given in the same breath as End
/// Turn walks inside the turn machine — `turn::run_phase_tick`'s sweep, not the
/// idle frame's — and the county greets it all the same.
///
/// Ablation: delete `crate::arrival::post` in `run_phase_tick` and no letter
/// comes; delete the one in `tick_units_only` instead and it still does, which
/// is what shows this test is on the turn's door.
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

/// **Your own county says nothing**, and neither does anything else on the way.
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
///
/// Ablation: delete `unit.dest_county != county` in
/// `l2_kingdom::arrival::enter_county` and the letter arrives at county 2.
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

/// **Your invasion is written, and you do not see it** — the letter is to the
/// lord, `Msg_Enqueue` keeps it out of your ring, and posting it still turns
/// your realm's voice rotation, which is the simulation's.
///
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

// ---------------------------------------------------------- taking the county

/// **Taking a county opens the capture letter**, category `0x0D` — the one the
/// capture films are wired to. A neutral county of happiness 10 greets the army
/// with a welcome and then surrenders its town without a fight; the player held
/// one county at a peak of one, so it is `L2.eng` 117.
///
/// Ablation: delete the `out.posted.push(Posted::Capture(..))` in
/// `l2_kingdom::units_tick` and the second letter never comes.
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
///
/// Ablation: delete `crate::arrival::post_captures` in `turn::record` and no
/// letter comes.
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

// ------------------------------------------------------ with the player's game

/// **The words are the player's own `L2.eng`**, and our transcription agrees
/// with it string for string — which is what makes it a fallback rather than a
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
    assert_eq!(strings, 17 * 2 + 17, "seventeen two-string groups and group 170's seventeen");
}
