#![allow(unused_imports)]
use super::*;
use super::render::*;
use super::motion::*;
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

// ---------------------------------------------------------------- the minimap
//
// **A fourth report, on build `EE0CB9233`**: *"battle is still a blue mess
// where the grass should be, a black minimap"*. The blue was C183's palette;
// the black was a painter we did not have.

/// **The two 2 × 2 sheets, read straight out of the install** — an 8-byte
/// header, then 16-byte frame records with `dataOffset` at `+4`. Written out
/// here so the expectation is the
/// player's own file and not our decoder.
fn t2_pixels(bytes: &[u8], index: usize) -> [u8; 4] {
    let count = u16::from_le_bytes([bytes[2], bytes[3]]) as usize;
    assert!(index < count, "frame {index} of {count}");
    let rec = 8 + index * 16;
    assert_eq!(u16::from_le_bytes([bytes[rec], bytes[rec + 1]]), 2, "frame {index} is not 2 wide");
    assert_eq!(
        u16::from_le_bytes([bytes[rec + 2], bytes[rec + 3]]),
        2,
        "frame {index} is not 2 high"
    );
    let off = u32::from_le_bytes(bytes[rec + 4..rec + 8].try_into().unwrap()) as usize;
    bytes[off..off + 4].try_into().unwrap()
}

/// `FUN_004BC107(…, 0x1E0, 0x18, 2)` and `FUN_004BC51A`'s `g_drawX += 2` /
/// `g_drawY += 2`. Literals, not `l2_view::scene`'s constants.
const PANEL_X: usize = 0x1E0;
const PANEL_Y: usize = 0x18;
/// `FUN_004BC020(…, 0x50, 0x50, …)`.
const PANEL_CELLS: usize = 0x50;
/// `Battle_Frame`'s `FUN_004bc1d1(4)`.
const PANEL_ROWS_A_FRAME: usize = 4;

/// The colour `FUN_004BC51A` picks for the man standing on each cell:
/// `g_realms[man.owner].shieldIndex`, or `6` for owner 6, and `0` — no man —
/// everywhere else. The *cell* is `mapX`/`mapY`, which `FUN_00491B1F` moves at
/// the start of a crossing; [`l2_view::scene::drawn_cell`] is that rule, and is
/// not what these two tests are checking.
fn expected_occupants(g: &Game) -> Vec<u8> {
    let l = live(g);
    let mut occ = vec![0u8; PANEL_CELLS * PANEL_CELLS];
    for i in 0..l.runner.fighters.len() {
        if !l.runner.is_alive(i) {
            continue;
        }
        let f = &l.runner.fighters[i];
        let owner = l.runner.sim.figures[f.sim].owner;
        let colour = if owner == 6 { 6 } else { g.kingdom.realms[owner as usize].shield_index };
        if colour == 0 {
            continue;
        }
        let ((cx, cy), _) = l2_view::scene::drawn_cell(f);
        occ[cy as usize * PANEL_CELLS + cx as usize] = colour;
    }
    occ
}

/// The whole 160 × 160 panel as the original would have it this instant.
fn expected_panel(g: &Game, bat1: &[u8], spri: &[u8]) -> Vec<u8> {
    let occ = expected_occupants(g);
    let side = PANEL_CELLS * 2;
    let mut out = vec![0u8; side * side];
    for cy in 0..PANEL_CELLS {
        for cx in 0..PANEL_CELLS {
            let colour = occ[cy * PANEL_CELLS + cx];
            let px = if colour == 0 {
                t2_pixels(bat1, live(g).runner.field.at(cx, cy).gfx as usize)
            } else {
                t2_pixels(spri, colour as usize)
            };
            for j in 0..2 {
                for i in 0..2 {
                    out[(cy * 2 + j) * side + cx * 2 + i] = px[j * 2 + i];
                }
            }
        }
    }
    out
}

/// Which cell rows of the panel on `canvas` disagree with `want`.
fn stale_rows(canvas: &Canvas, want: &[u8]) -> Vec<usize> {
    let side = PANEL_CELLS * 2;
    (0..PANEL_CELLS)
        .filter(|&cy| {
            (0..2).any(|j| {
                let y = cy * 2 + j;
                (0..side).any(|x| canvas.at(PANEL_X + x, PANEL_Y + y) != want[y * side + x])
            })
        })
        .collect()
}

/// A staged battle whose two realms carry shields 2 and 5 — the byte
/// `FUN_004BC51A` indexes `t2_spri.pl8` with.
fn staged_with_shields(
    ai_row: usize,
    human: &[(Troop, u16)],
    ai: &[(Troop, u16)],
) -> (Game, Machine) {
    let (mut g, m) = staged(ai_row, human, ai, |(x, y)| (x as i32 - 7, y as i32 - 6));
    g.kingdom.realms[1].shield_index = 2;
    g.kingdom.realms[2].shield_index = 5;
    (g, m)
}

/// **The panel is `t2_bat1.pl8` under `t2_spri.pl8`, pixel for pixel, and there
/// is no rectangle on it.**
///
/// `Battle_LoadAssets` (`0x004987B7`) registers the painter with
/// `FUN_004BC107(t2_bat1, t2_bat2, t2_spri, 0x1E0, 0x18, 2)` — entries `0x0B`,
/// `0x0C` and `0x11` of the asset table at `0x004DA550` — and `FUN_004BC51A`
/// draws
/// `g_realms[owner].shieldIndex` of `t2_spri` over a cell holding a man. It
/// draws **no viewport rectangle**, and this compares every pixel of the panel,
///
/// The expectation is decoded from the install's own two files by [`t2_pixels`].
///
/// Ablation: the panel this replaced — `fill_rect(ink.background)`, a dot a
/// *side*, and a `widget::frame` round the camera — is stale on all eighty
/// rows.
#[test]
fn the_overview_panel_is_t2_bat1_under_t2_spri_with_no_rectangle_on_it() {
    let Some((assets, platform)) = install() else {
        l2_testkit::skip!("no game install, so no T2_bat1.pl8 to compare the panel against");
    };
    let bat1 = platform.vfs.read("T2_bat1.pl8").expect("T2_bat1.pl8");
    let spri = platform.vfs.read("T2_spri.pl8").expect("T2_spri.pl8");
    assert_eq!(
        u16::from_le_bytes([bat1[2], bat1[3]]),
        252,
        "t2_bat1 is one 2 x 2 frame per T32_bat1 tile"
    );
    assert_eq!(
        u16::from_le_bytes([spri[2], spri[3]]),
        7,
        "t2_spri is the erase tile and six colours"
    );

    let (mut g, mut m) = staged_with_shields(
        74,
        &[(Troop::Swordsmen, 3), (Troop::Archers, 3)],
        &[(Troop::Macemen, 3)],
    );
    let mut canvas = Canvas::screen();
    paint(&mut m, &mut g, &assets, &mut canvas);

    let want = expected_panel(&g, &bat1, &spri);
    let stale = stale_rows(&canvas, &want);
    assert!(stale.is_empty(), "the panel disagrees with the install's sheets on cell rows {stale:?}");

    // And the men are coloured by their **realm**, not by their side: shields 2
    // and 5 are different frames of `t2_spri`, so both colours are on the panel.
    let (a, b) = (t2_pixels(&spri, 2)[0], t2_pixels(&spri, 5)[0]);
    assert_ne!(a, b, "shields 2 and 5 are the same colour in this install's t2_spri.pl8");
    let count = |idx: u8| {
        (0..PANEL_CELLS * 2)
            .flat_map(|y| (0..PANEL_CELLS * 2).map(move |x| (x, y)))
            .filter(|&(x, y)| canvas.at(PANEL_X + x, PANEL_Y + y) == idx)
            .count()
    };
    assert!(count(a) >= 4, "realm 1's shield colour {a} is not on the panel");
    assert!(count(b) >= 4, "realm 2's shield colour {b} is not on the panel");
}

/// **Four rows a frame, and the whole panel in twenty** — `FUN_004BC1D1`
/// (`0x004BC1D1`), which `Battle_Frame` (`0x004B99C0`) calls with `4` once
/// `g_mapRedraw` has been spent:
///
/// ```c
/// DAT_004E5D74 += n;
/// if (0x50 - n < DAT_004E5D74) DAT_004E5D74 = 0;
/// if (DAT_004E5D58 == 2) FUN_004BC51A(DAT_004E5D74, n);
/// ```
///
/// `Screen_DrawBattlefield` (`0x004233F7`) enters with `g_mapRedraw = 1` and
/// `FUN_004bc1d1(0x50)`; `FUN_004BC142`, called straight after the schedule in
/// `Battle_Frame`, ends `if (g_mapRedraw != 0) g_mapRedraw--`. **So the full
/// pass happens once and the panel then lags the field by up to twenty
/// frames.** Without that decrement the obvious reading is that the full pass
/// runs every frame, and it does not.
///
/// Every cell's tile is changed under the panel, which makes all eighty cell
/// rows stale at once; one frame may then repaint four of them and no more.
///
/// Ablation: paint all eighty rows a frame — red, nothing is ever stale.
#[test]
fn the_overview_panel_is_repainted_four_cell_rows_a_frame() {
    let Some((assets, platform)) = install() else {
        l2_testkit::skip!("no game install, so no T2_bat1.pl8 to repaint the panel from");
    };
    let bat1 = platform.vfs.read("T2_bat1.pl8").expect("T2_bat1.pl8");
    let spri = platform.vfs.read("T2_spri.pl8").expect("T2_spri.pl8");

    let (mut g, mut m) = staged_with_shields(74, &[(Troop::Swordsmen, 3)], &[(Troop::Macemen, 3)]);
    // Paused, so the men stand still and the only thing that changes under the
    // panel is what this test changes. `Battle_Frame` schedules the panel
    // whether or not the battle is paused.
    g.battle.as_mut().expect("a live battle").paused = true;
    let mut canvas = Canvas::screen();
    // The entry pass: `Screen_DrawBattlefield`'s `FUN_004bc1d1(0x50)`.
    frame(&mut m, &mut g, &assets, &mut canvas);
    assert!(
        stale_rows(&canvas, &expected_panel(&g, &bat1, &spri)).is_empty(),
        "the entry pass did not paint the whole panel"
    );

    {
        let field = &mut g.battle.as_mut().unwrap().runner.field;
        for cell in field.cells.iter_mut() {
            let old = t2_pixels(&bat1, cell.gfx as usize);
            cell.gfx = (1..252)
                .map(|d| ((cell.gfx as usize + d) % 252) as u8)
                .find(|&n| t2_pixels(&bat1, n as usize) != old)
                .expect("some frame of t2_bat1 is drawn in different pixels");
        }
    }
    let want = expected_panel(&g, &bat1, &spri);
    assert_eq!(
        stale_rows(&canvas, &want).len(),
        PANEL_CELLS,
        "the field did not change under the panel"
    );

    // One frame repaints four rows, and they are the four the cursor lands on:
    // the entry pass left it at 0, so `0 + 4`.
    frame(&mut m, &mut g, &assets, &mut canvas);
    let stale = stale_rows(&canvas, &want);
    let fresh: Vec<usize> = (0..PANEL_CELLS).filter(|r| !stale.contains(r)).collect();
    assert_eq!(
        fresh,
        (PANEL_ROWS_A_FRAME..2 * PANEL_ROWS_A_FRAME).collect::<Vec<_>>(),
        "one frame repainted cell rows {fresh:?}"
    );

    // And the cursor wraps: eighty rows at four a frame is twenty frames.
    for _ in 1..(PANEL_CELLS / PANEL_ROWS_A_FRAME) {
        frame(&mut m, &mut g, &assets, &mut canvas);
    }
    let left = stale_rows(&canvas, &want);
    assert!(left.is_empty(), "twenty frames left cell rows {left:?} stale");
}

/// The same with the install's artwork, so the painter that runs is
/// `l2_view::scene::draw` — the one this file changed —
/// placeholder's blocks.
#[test]
fn painting_the_battlefield_with_its_artwork_does_not_change_the_battle() {
    let Some((assets, _platform)) = install() else {
        l2_testkit::skip!("no game install, so the placeholder test above is the whole check");
    };
    let (drawn, blind, killed) = played(&assets, 1_500);
    same_battle(&drawn, &blind, "install", killed);
}

// ------------------------------------------------- the arrow and the engine

