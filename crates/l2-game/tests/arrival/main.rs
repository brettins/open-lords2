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

mod arrival_tests;
pub use arrival_tests::*;

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
pub(crate) fn world(county_2: u8) -> (Game, Assets, Machine) {
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

