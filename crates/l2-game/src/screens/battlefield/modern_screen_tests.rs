//! C272: the battlefield screen hands a Modern seat's events to the Modern scheme and a Classic
//! seat's to the original's arms.

use super::*;
use crate::battlefield::BattleOp;
use crate::game::{BattleControls, Game};
use crate::input::{Event, Key};
use crate::screen::Screen;
use l2_sim::{runner::Army, Troop};

const ME: u8 = 1;

fn game(controls: BattleControls) -> Game {
    let field = l2_sim::terrain::build(&vec![0u8; l2_sim::terrain::CELLS], 1);
    let runner = l2_sim::runner::BattleRunner::deploy_armies(
        field,
        0x5EED,
        Army { troops: &[(Troop::Swordsmen, 12u16)], owner: ME, human: true },
        Army { troops: &[(Troop::Peasants, 8u16)], owner: 2, human: true },
    );
    let mut b = crate::battlefield::LiveBattle::new(runner, 0, 0, 0, None, ME, ME);
    b.paused = false;
    let mut game = Game::new(1);
    game.battle = Some(Box::new(b));
    game.prefs.battle_controls = controls;
    game
}

/// The pixel of one of my men, with the camera on him.
fn man(game: &mut Game) -> (i32, i32) {
    let b = game.battle.as_deref_mut().expect("a battle");
    let f = (0..b.runner.fighters.len())
        .find(|&f| b.runner.sim.figures[b.runner.fighters[f].sim].owner == ME)
        .expect("a man");
    let (x, y) = (b.runner.fighters[f].x, b.runner.fighters[f].y);
    b.look_at((x, y));
    (
        crate::battlefield::VIEW.x + (x as i32 - b.cam.0) * crate::battlefield::TILE + 16,
        crate::battlefield::VIEW.y + (y as i32 - b.cam.1) * crate::battlefield::TILE + 16,
    )
}

fn feed(s: &mut BattlefieldScreen, game: &mut Game, events: &[Event]) {
    let assets = crate::game::Assets::placeholder();
    for &e in events {
        s.handle(e, &mut Ctx { game: &mut *game, assets: &assets });
    }
}

/// Ablate `live.is_modern()` in the `Event::Click`/`Event::Release` arms: the Modern seat's click
/// takes the original's path and sets `figure.selected`.
#[test]
fn a_modern_seats_click_selects_a_unit_and_a_classic_seats_selects_figures() {
    for modern in [true, false] {
        let mut g = game(if modern { BattleControls::Modern } else { BattleControls::Classic });
        let mut s = BattlefieldScreen::new();
        s.update(&mut Ctx { game: &mut g, assets: &crate::game::Assets::placeholder() });
        let (x, y) = man(&mut g);
        feed(&mut s, &mut g, &[Event::Pointer { x, y }, Event::Click { x, y }, Event::Release { x, y }]);
        let b = g.battle.as_deref().expect("a battle");
        let figures_selected = b.runner.sim.figures.iter().any(|f| f.selected != 0);
        assert_eq!(b.sel.len(), usize::from(modern), "modern {modern}");
        assert_eq!(figures_selected, !modern, "modern {modern}");
    }
}

/// Ablate the `_ if live.is_modern() => live.m_key` arm: `S` falls through and nothing is sent.
#[test]
fn a_modern_seats_keys_and_right_click_send_commands_even_paused() {
    let mut g = game(BattleControls::Modern);
    g.battle.as_deref_mut().expect("a battle").paused = true;
    let mut s = BattlefieldScreen::new();
    s.update(&mut Ctx { game: &mut g, assets: &crate::game::Assets::placeholder() });
    let (x, y) = man(&mut g);
    feed(&mut s, &mut g, &[Event::Pointer { x, y }, Event::Click { x, y }, Event::Release { x, y }]);
    let unit = g.battle.as_deref().expect("a battle").sel[0] as usize;
    g.battle.as_deref_mut().expect("a battle").deferred = true;
    let (tx, ty) = (x + 3 * 32, y);
    feed(&mut s, &mut g, &[Event::Pointer { x: tx, y: ty }, Event::RightClick { x: tx, y: ty }, Event::KeyDown(Key::Char('S'))]);
    let b = g.battle.as_deref().expect("a battle");
    let verbs: Vec<_> = b
        .outbox
        .iter()
        .filter_map(|o| match o {
            BattleOp::Command { verb, units, .. } if units == &vec![unit as u8] => Some(*verb),
            _ => None,
        })
        .collect();
    assert_eq!(verbs, vec![crate::battlefield::Verb::Move, crate::battlefield::Verb::Stop]);
}
