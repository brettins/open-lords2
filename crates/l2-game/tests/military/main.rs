
mod battle_part;
pub use battle_part::*;
mod raising;
pub use raising::*;
mod marching;
pub use marching::*;
mod division;
pub use division::*;

use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::battlefield as bf;
use l2_game::screens::{armoury, army, battle, divide, info, map};
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

const BORDER_1_2: usize = 32;
const BORDER_2_3: usize = 48;

pub(crate) fn world() -> (Game, Assets) {
    let mut g = Game::new(11);
    g.prefs.tip_screens = false;
    g.kingdom.set_county_count(3);
    for id in 1..=3usize {
        let c = &mut g.kingdom.counties[id];
        c.population = 1_000;
        c.happiness = 90;
        c.grain = 500;
        c.herd = 100;
    }
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[2].owner = 2;
    g.kingdom.counties[3].owner = 2;
    l2_testkit::chain_neighbours!(g.kingdom);
    for realm in 1..=2usize {
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 5;
        g.kingdom.realms[realm].gold = 20_000;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].weapons = [200; 6];

    let mut m = CampaignMap::empty();
    for i in 0..MAP_TILES {
        let x = i % MAP_DIM;
        m.county[i] = if x < BORDER_1_2 {
            1
        } else if x < BORDER_2_3 {
            2
        } else {
            3
        };
    }
    g.kingdom.campaign.map = m;
    g.kingdom.campaign.mercenaries = MercenaryBands::init(2);
    g.player = 1;
    g.selected = 1;
    g.prefs.animations = false;
    (g, Assets::placeholder())
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

pub(crate) fn click(m: &mut Machine, g: &mut Game, a: &Assets, at: (i32, i32)) {
    send(m, g, a, Event::Click { x: at.0, y: at.1 });
}

fn press(m: &mut Machine, g: &mut Game, a: &Assets, c: char) {
    send(m, g, a, Event::KeyDown(Key::letter(c)));
}

fn run_until(
    m: &mut Machine,
    g: &mut Game,
    a: &Assets,
    what: &str,
    done: impl Fn(&Machine, &Game) -> bool,
) {
    for _ in 0..2_000 {
        if done(m, g) {
            return;
        }
        tick(m, g, a);
    }
    panic!("{what} never happened");
}

pub(crate) fn end_turn(m: &mut Machine, g: &mut Game, a: &Assets) {
    let before = g.kingdom.turn_count;
    press(m, g, a, 'e');
    run_until(m, g, a, "the turn", |_, g| g.kingdom.turn_count > before);
    for _ in 0..=l2_view::fade::PHASES {
        tick(m, g, a);
    }
}

fn on(r: l2_game::input::Rect) -> (i32, i32) {
    (r.centre_x(), r.y + r.h / 2)
}

fn press_and_wait(m: &mut Machine, g: &mut Game, a: &Assets, at: (i32, i32)) {
    let before = m.top_id();
    click(m, g, a, at);
    assert_eq!(m.top_id(), before, "a kind-5 press must not act on the press");
    for i in 1..l2_game::press::DELAYED_FRAMES as u32 {
        tick(m, g, a);
        assert_eq!(m.top_id(), before, "nor on tick {i} of {}", l2_game::press::DELAYED_FRAMES);
    }
    tick(m, g, a);
}

fn pixel(x: u8, y: u8) -> Option<(i32, i32)> {
    let probe = map::MapScreen::new();
    campaign::tile_centre(probe.viewport(), probe.zoom(), x as usize, y as usize)
}

fn adjacent_pair(first_x: impl Fn(u8) -> bool) -> ((u8, u8), (u8, u8)) {
    for y in 0..64u8 {
        for x in 0..63u8 {
            if !first_x(x) {
                continue;
            }
            if pixel(x, y).is_some() && pixel(x + 1, y).is_some() {
                return ((x, y), (x + 1, y));
            }
        }
    }
    panic!("no adjacent visible pair at the opening viewport");
}

fn army_at(g: &mut Game, owner: u8, county: u8, men: i32, at: (u8, u8)) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, at.0, at.1);
    u.men = men;
    u.troops[TroopType::Peasant.index()] = men;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = owner == 1;
    g.kingdom.campaign.units.spawn(u).expect("a free slot")
}

fn on_the_map() -> (Game, Assets, Machine) {
    let (g, a) = world();
    (g, a, Machine::new(ScreenId::Campaign))
}

fn right_click(m: &mut Machine, g: &mut Game, a: &Assets, at: (i32, i32)) {
    send(m, g, a, Event::RightClick { x: at.0, y: at.1 });
}

/// `g_infoUnitButtons` (`0x004DC560`) record `i`, at `(48 | 112 | 176, 352)`,
/// 40 square. `Layout { row: 2 }` for the player's own army is what puts them
/// at their table y.
fn info_button(i: usize) -> (i32, i32) {
    on(l2_game::input::Rect::new(
        info::BUTTON_X[i],
        info::BUTTON_DY,
        info::BUTTON_DIM,
        info::BUTTON_DIM,
    ))
}


fn frame(m: &mut Machine, g: &mut Game, a: &Assets) -> l2_view::Canvas {
    let mut c = l2_view::Canvas::screen();
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, &mut c);
    c
}

fn differing(a: &l2_view::Canvas, b: &l2_view::Canvas) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for y in 0..480usize {
        for x in 0..640usize {
            if a.at(x, y) != b.at(x, y) {
                out.push((x as i32, y as i32));
            }
        }
    }
    out
}

fn the_prompt_over(band: bool) -> l2_view::Canvas {
    let (mut g, a, mut m, attacker, _) = a_battle_is_about_to_happen();
    {
        let u = g.kingdom.campaign.units.get_mut(attacker).expect("the attacker");
        u.troops = [0; l2_kingdom::unit::TROOP_TYPES];
        u.men = 200;
        if band {
            // `Mercenary_Hire` (`0x004AC7F3`): the band goes in `+0x195…+0x197`
            // and into `menTotal`,
            u.mercenaries =
                Some(l2_kingdom::Mercenaries { band: 2, troop: TroopType::Pikeman, men: 200 });
        } else {
            u.troops[TroopType::Pikeman.index()] = 200;
        }
    }
    press(&mut m, &mut g, &a, 'e');
    assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt), "the prompt should be up");
    let mut canvas = l2_view::Canvas::screen();
    let ctx = Ctx { game: &mut g, assets: &a };
    m.draw(&ctx, &mut canvas);
    canvas
}

