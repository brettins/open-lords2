//! **A county taken on the map, all the way to `cap_cty*.smk`.**
//!
//! The neighbouring capture test posts the category-`0x0D` record by hand. This
//! one never touches the queue: the player's army walks onto an enemy town,
//! `County_ChangeOwner` runs, `l2_game::arrival::capture_record` writes the
//! letter, `Msg_Pump` opens it and `Msg_DrawWindow`'s animated branch plays the
//! film in its place. Every step is the game's own.
#![allow(unused_imports)]
use super::*;
use l2_game::game::Assets;
use l2_game::message::category;
use l2_game::movie::Film;
use l2_game::screen::{Machine, ScreenId};
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::movement::{self, Routing};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};

/// County 1 is `x < 32`, county 2 the two columns `32` and `33`, county 3 the
/// rest — `tests/arrival` holds the same bands.
const WEST: usize = 32;
const EAST: usize = 34;

/// **County 2 is the enemy's and has thirty people in it**, which is under
/// `levy::DEFENCE_MIN_POPULATION`, so `County_Attack` takes it by walking in
/// levying a militia and calling a battle. Row `test-worlds-need-
/// neighbours`: `County_BordersRealm` reads `county.neighbours`, and without
/// the two lines of adjacency below the capture is the ungovernable `else`
/// branch — letter 129, category `NOTICE`, and no film.
fn capture_world() -> (Game, Assets, Machine) {
    let mut g = realms();
    g.prefs.tip_screens = false;
    for id in 1..=3usize {
        let c = &mut g.kingdom.counties[id];
        c.population = 1_000;
        c.happiness = 77;
    }
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[2].owner = 2;
    g.kingdom.counties[2].population = 30;
    g.kingdom.counties[3].owner = 2;
    for (id, neighbours) in [(1usize, vec![2u8]), (2, vec![1, 3]), (3, vec![2])] {
        let c = &mut g.kingdom.counties[id];
        c.neighbour_count = neighbours.len() as u8;
        for (i, n) in neighbours.into_iter().enumerate() {
            c.neighbours[i] = n;
        }
    }
    l2_kingdom::conquest::recount_realm_counties(&g.kingdom.counties, &mut g.kingdom.realms);
    for realm in 1..=5usize {
        g.kingdom.realms[realm].peak_counties = g.kingdom.realms[realm].county_count;
    }

    let mut map = CampaignMap::empty();
    for i in 0..MAP_TILES {
        let x = i % MAP_DIM;
        map.county[i] = if x < WEST { 1 } else if x < EAST { 2 } else { 3 };
    }
    map.set_flags(33, 20, flags::CASTLE);
    g.kingdom.campaign.map = map;
    g.player = 1;
    g.selected = 1;
    (g, Assets::placeholder(), Machine::new(ScreenId::Campaign))
}

/// The player's army, marched at the enemy town as `Unit_OrderMove` marches it.
fn march_on_the_town(g: &mut Game) -> usize {
    let mut u = Unit::new(UnitKind::Army, 1, 31, 20);
    u.men = 600;
    u.troops[TroopType::Peasant.index()] = 600;
    u.county = 1;
    u.home_county = 1;
    u.owner_is_human = true;
    let id = g.kingdom.campaign.units.spawn(u).expect("a free slot");
    movement::order_move(&g.kingdom.campaign.map, &mut g.kingdom.campaign.units, id, (33, 20), Routing::Direct)
        .expect("open ground the whole way");
    id
}

/// Run frames until a film is up, or the machine settles on something else.
fn until_film(m: &mut Machine, g: &mut Game, a: &Assets) -> Option<Film> {
    for _ in 0..4_000 {
        if let Some(f) = film_on_top(m) {
            return Some(f);
        }
        if !g.kingdom.campaign.units.iter().any(|(_, u)| u.moving) && g.messages.open().is_some() {
            return None;
        }
        tick(m, g, a);
    }
    panic!("the march never finished; the screen is {:?}", m.top_id());
}

/// **The posted letter reaches the film.** Before this the path was read, not
/// run: `message::animate`'s `category::CAPTURE` arm had no test that a capture
/// on the map ever produced the record it matches.
///
/// Ablation: delete the `category::CAPTURE` arm from
/// `l2_game::message::queue::actions::animate` and this goes red — the ordinary
/// window stays up instead.
#[test]
fn a_county_taken_on_the_map_plays_the_capture_film() {
    let (mut g, a, mut m) = capture_world();
    let id = march_on_the_town(&mut g);

    let film = until_film(&mut m, &mut g, &a).unwrap_or_else(|| {
        panic!("no film; the open letter is {:?}", g.messages.open());
    });
    let Film::Capture { take, record } = film else { panic!("{film:?}") };

    assert_eq!(g.kingdom.counties[2].owner, 1, "the county changed hands");
    assert_eq!(g.kingdom.campaign.units.get(id).map(|u| u.county), Some(2), "the army is in it");
    assert_eq!((record.to, record.category, record.county), (1, category::CAPTURE, 2));
    assert_eq!(record.spare, 2, "the county's old owner rides in the record");
    // `DAT_00553ED4` is stepped before use, so a session's first capture is
    // take 1 — the rotation itself is the neighbouring test's.
    assert_eq!((take, film.file()), (1, "cap_cty2.smk"));
    assert!(g.messages.open().is_none(), "Msg_Dismiss ran inside the draw");
}

/// **The same capture with `g_optAnimations == 0`**: no film, and the ordinary
/// window is up with the letter still open — `Msg_DrawWindow` never reaches its
/// animated branch, so nothing dismisses the record.
///
/// Ablation: delete the `category::CAPTURE` arm from `animate` and this one
/// passes either way; delete `!game.prefs.animations` from the guard and the
/// film plays here too.
#[test]
fn the_same_capture_with_animations_off_is_an_ordinary_letter() {
    let (mut g, a, mut m) = capture_world();
    g.prefs.animations = false;
    march_on_the_town(&mut g);

    assert_eq!(until_film(&mut m, &mut g, &a), None, "a film played with animations off");
    assert_eq!(m.top_id(), Some(ScreenId::Message), "the window is up: {:?}", m.ids());
    let record = g.messages.open().copied().expect("the letter is still open");
    assert_eq!((record.to, record.category, record.county), (1, category::CAPTURE, 2));
    assert_eq!(g.kingdom.counties[2].owner, 1, "the county changed hands all the same");

    // The option is the only difference: the record is the same one, still
    // open, and `g_messageTimer` has not run past 0x7C6 yet.
    g.prefs.animations = true;
    tick(&mut m, &mut g, &a);
    assert!(
        matches!(film_on_top(&m), Some(Film::Capture { .. })),
        "the option turned back on, the same letter plays: {:?}",
        m.ids()
    );
}
