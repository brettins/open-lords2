#![allow(unused_imports)]
use super::*;
use super::motion_tests::*;
use super::*;
use super::render::*;
use super::panel::*;
use super::entities::*;
use super::ui::*;
use l2_formats::{DecodedFrame, Palette};
use l2_game::battlefield::{self as bf, LiveBattle};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::menubar;
use l2_game::Game;
use l2_sim::runner::{Army, BattleRunner};
use l2_sim::terrain::DIM;
use l2_sim::{Motion, Troop, SIDE_A, SIDE_B};
use l2_view::sheet::Sheet;
use l2_view::Canvas;

/// Two armies eight cells apart, marched at each other. 42 figures, no
/// install, no painter — [`l2_view::scene::figure_origin`] is the whole
/// picture and this counts how far it moves a live man in one tick.
fn march() -> BattleRunner {
    let human: &[(Troop, u16)] = &[(Troop::Swordsmen, 8), (Troop::Archers, 6), (Troop::Macemen, 7)];
    let ai: &[(Troop, u16)] = &[(Troop::Crossbowmen, 7), (Troop::Macemen, 8), (Troop::Peasants, 6)];
    let mut r = BattleRunner::deploy_armies(
        field(44),
        0x5EED,
        Army { troops: ai, owner: 2, human: false },
        Army { troops: human, owner: 1, human: true },
    );
    for u in 1..=l2_sim::unit::MAX_UNITS {
        if r.units.get(u).is_live() && r.units.get(u).human {
            r.order_unit(u, 40, 44);
        }
    }
    r
}

// ------------------------------------------------------------------ the ghosts

/// The pixels no painter on this screen writes: the menu bar's 24 rows, the
/// strip below the field, and the column between the field and the banners.
fn untouched_regions() -> Vec<(i32, i32, i32, i32)> {
    vec![
        (0, 0, 640, FIELD_Y0),
        (FIELD_X0, FIELD_Y1, FIELD_X1, 480),
        // `DAT_004D31F4`'s banners begin at x 484 at the earliest, and the
        // overview's fill ends at y 184; the buttons start at 448.
        (FIELD_X1, 184, 484, 448),
    ]
}

fn snapshot(canvas: &Canvas) -> Vec<u8> {
    let mut out = Vec::new();
    for (x0, y0, x1, y1) in untouched_regions() {
        for y in y0..y1 {
            for x in x0..x1 {
                out.push(canvas.at(x as usize, y as usize));
            }
        }
    }
    out
}

// -------------------------------------------------------------- determinism

/// FNV-1a, 64-bit — a fingerprint of the saved bytes to print, so the same
/// battle can be compared across two builds by eye.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
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

/// One battle, twice from the same events: one copy painted after every tick,
/// the other never painted.
///
/// **The battle ends inside the loop, and that is the original's answer.**
/// `Battle_CheckOutcome` (`0x00477DFC`): `if (menA < 1) g_battleLoser =
/// g_battleArmyB` — no clock, no morale break. It then counts `DAT_00568470`
/// to 5000 behind the banner, and `Smk_OnFinished` sets that to 5001 when the
/// outcome film ends. So with the install's artwork `turn::finish_battle` takes
/// the battle (tick 902 here); with the placeholder no film opens and the
/// banner keeps its 5000 frames. Casualties are therefore counted while there
/// is still a battle to count them from, and the per-tick equality below is
/// what holds the two copies to the same end.
pub(crate) fn played(assets: &Assets, ticks: u32) -> (Game, Game, u32) {
    let human = [(Troop::Swordsmen, 3), (Troop::Archers, 3)];
    let ai = [(Troop::Crossbowmen, 3), (Troop::Macemen, 3)];
    let cam = |_: (u8, u8)| (33, 33);
    let (mut drawn, mut dm) = staged(44, &human, &ai, cam);
    let (mut blind, mut bm) = staged(44, &human, &ai, cam);

    for (g, m) in [(&mut drawn, &mut dm), (&mut blind, &mut bm)] {
        // A box round the whole human army, then a click on an enemy.
        let cells = cells_of(live(g), SIDE_A);
        let lo = (cells.iter().map(|c| c.0).min().unwrap(), cells.iter().map(|c| c.1).min().unwrap());
        let hi = (cells.iter().map(|c| c.0).max().unwrap(), cells.iter().map(|c| c.1).max().unwrap());
        let (lx, ly) = pixel(live(g), lo);
        let (hx, hy) = pixel(live(g), hi);
        let from = (lx - bf::TILE / 2 + 2, ly - bf::TILE / 2 + 2);
        let to = (hx + bf::TILE / 2 - 2, hy + bf::TILE / 2 - 2);
        send(m, g, assets, Event::Pointer { x: from.0, y: from.1 });
        send(m, g, assets, Event::Click { x: from.0, y: from.1 });
        send(m, g, assets, Event::Pointer { x: to.0, y: to.1 });
        send(m, g, assets, Event::Release { x: to.0, y: to.1 });
        let enemy = *cells_of(live(g), SIDE_B).first().unwrap();
        click_at(m, g, assets, pixel(live(g), enemy));
        send(m, g, assets, Event::Pointer { x: 240, y: 240 });
    }
    assert!(live(&drawn).runner.selected_count(1) > 0, "the box picked nobody");
    assert_eq!(drawn.battle, blind.battle, "the same events left the two games different");

    let mut canvas = Canvas::screen();
    let mut killed: u32 = 0;
    let mut handed_back: Option<u32> = None;
    for t in 0..ticks {
        frame(&mut dm, &mut drawn, assets, &mut canvas);
        let mut ctx = Ctx { game: &mut blind, assets };
        bm.update(&mut ctx);
        assert!(drawn.battle == blind.battle, "painting changed the battle at tick {t}");
        match drawn.battle.as_deref() {
            Some(live) => {
                killed = l2_sim::ALL_TROOPS
                    .iter()
                    .map(|&troop| live.runner.sim.cues.melee_casualties(troop))
                    .sum()
            }
            None => handed_back = handed_back.or(Some(t)),
        }
    }
    eprintln!("{ticks} ticks: the battle was handed back to the turn at {handed_back:?}");
    (drawn, blind, killed)
}

/// Check the two plays agree to the byte, that the battle did something, and
/// print the fingerprint.
fn same_battle(drawn: &Game, blind: &Game, label: &str, killed: u32) {
    let (a, b) = (l2_game::save::encode(drawn), l2_game::save::encode(blind));
    assert_eq!(a, b, "the saved bytes differ between the painted and the unpainted battle");
    eprintln!("{label}: saved battle {} bytes, fnv1a {:016x}", a.len(), fnv1a(&a));
    assert!(killed > 0, "nobody fought, so the comparison is about a battle with nothing in it");
}

