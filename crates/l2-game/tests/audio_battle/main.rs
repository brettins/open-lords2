
mod cries;
pub use cries::*;
mod events;
pub use events::*;
mod determinism;
pub use determinism::*;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use l2_game::audio::{self, names, Audio, Request, TroopCries};
use l2_game::battlefield::{self as bf, cry, Cry, LiveBattle};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::{Cues, Troop, SIDE_A, SIDE_B};


fn near_field() -> l2_sim::Battlefield {
    let mut layer = vec![0u8; l2_sim::terrain::CELLS];
    layer[36 * l2_sim::terrain::DIM + 40] = 0x04;
    layer[44 * l2_sim::terrain::DIM + 40] = 0x0F;
    l2_sim::terrain::build(&layer, 1)
}

fn battle(human: &[(Troop, u16)], ai: &[(Troop, u16)]) -> LiveBattle {
    let runner = BattleRunner::deploy_armies(
        near_field(),
        0x5EED,
        Army { troops: ai, owner: 2, human: false },
        Army { troops: human, owner: 1, human: true },
    );
    let mut live = LiveBattle::new(runner, 0, 0, 0, None, 1, 1);
    live.paused = false;
    live.cam = (33, 33);
    live
}

fn staged(human: &[(Troop, u16)], ai: &[(Troop, u16)]) -> (Game, Machine) {
    let mut g = Game::new(5);
    g.prefs.tip_screens = false;
    g.player = 1;
    g.battle = Some(Box::new(battle(human, ai)));
    (g, Machine::new(ScreenId::Battlefield))
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn live(g: &Game) -> &LiveBattle {
    g.battle.as_deref().expect("a live battle")
}

fn pixel(live: &LiveBattle, cell: (u8, u8)) -> (i32, i32) {
    let (cx, cy) = (cell.0 as i32 - live.cam.0, cell.1 as i32 - live.cam.1);
    assert!(
        (0..bf::VIEW_COLS).contains(&cx) && (0..bf::VIEW_ROWS).contains(&cy),
        "cell {cell:?} is off screen with the camera at {:?}",
        live.cam
    );
    (bf::VIEW.x + cx * bf::TILE + bf::TILE / 2, bf::VIEW.y + cy * bf::TILE + bf::TILE / 2)
}

fn cells_of(live: &LiveBattle, side: u8) -> Vec<(u8, u8)> {
    live.runner
        .fighters
        .iter()
        .enumerate()
        .filter(|(i, f)| f.side == side && live.runner.is_alive(*i))
        .map(|(_, f)| (f.x, f.y))
        .collect()
}

fn click_at(m: &mut Machine, g: &mut Game, a: &Assets, (x, y): (i32, i32)) {
    send(m, g, a, Event::Pointer { x, y });
    send(m, g, a, Event::Click { x, y });
    send(m, g, a, Event::Release { x, y });
}

fn box_the_army(m: &mut Machine, g: &mut Game, a: &Assets) {
    let cells = cells_of(live(g), SIDE_A);
    let lo = (cells.iter().map(|c| c.0).min().unwrap(), cells.iter().map(|c| c.1).min().unwrap());
    let hi = (cells.iter().map(|c| c.0).max().unwrap(), cells.iter().map(|c| c.1).max().unwrap());
    let (lx, ly) = pixel(live(g), lo);
    let (hx, hy) = pixel(live(g), hi);
    let from = (lx - bf::TILE / 2 + 2, ly - bf::TILE / 2 + 2);
    let to = (hx + bf::TILE / 2 - 2, hy + bf::TILE / 2 - 2);
    send(m, g, a, Event::Pointer { x: from.0, y: from.1 });
    send(m, g, a, Event::Click { x: from.0, y: from.1 });
    send(m, g, a, Event::Pointer { x: to.0, y: to.1 });
    send(m, g, a, Event::Release { x: to.0, y: to.1 });
}

fn empty_cell(live: &LiveBattle) -> (u8, u8) {
    let (cx, cy) = (live.cam.0 as u8, live.cam.1 as u8);
    (cx + 1, cy + 1)
}

fn file_of(r: Request) -> &'static str {
    match r {
        Request::Slot(n) => names::slot(names::Bank::Battle, n).expect("a battle slot that plays"),
        Request::File(f) => f,
    }
}

fn fight(
    human: &[(Troop, u16)],
    ai: &[(Troop, u16)],
    ticks: u32,
    advance: bool,
) -> (BTreeSet<&'static str>, LiveBattle) {
    let mut live = battle(human, ai);
    let mut asked = BTreeSet::new();
    for t in 0..ticks {
        if advance && t % 300 == 0 {
            live.runner.order_side(SIDE_A, 40, 44);
        }
        let was = live.runner.sim.cues;
        live.tick();
        for r in audio::battle_requests(&was, &live.runner.sim.cues) {
            asked.insert(file_of(r));
        }
        if live.conclusion.is_some() {
            break;
        }
    }
    (asked, live)
}


