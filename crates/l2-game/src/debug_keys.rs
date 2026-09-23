//! `App_WndProc` (`0x004B29BE`) `WM_KEYDOWN` arms that test no screen, or
//! only `g_screenId`: `004b0000.c:1929-2006`. None consumes the key.

use crate::input::Key;
use crate::Game;

/// Last figure index `g_debugSelectedMan` reaches: `< 0x50` then `+ 1`.
pub const LAST_MAN: u32 = 0x50;

/// not-encoded: debug and overlay switches, none in a save block.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DebugKeys {
    /// `DAT_0053F684`. [V] its only writes are `= 0` (`0x0040EB..`,
    /// `00400000.c:8247`; `0x00497500`, `00490000.c:3061`), so F3 and F4 are
    /// dead in the shipped game.
    pub debug: bool,
    /// `g_debugSelectedMan` (`0x00553E68`), what `BattleDebug_Panel` prints.
    pub selected_man: u32,
    /// `DAT_00568960`, the overlay mode, 0..=2.
    pub panel_mode: u8,
    /// `DAT_0055402C`, the overlays' draw gate (`00420000.c:1469`).
    pub panel_on: bool,
    /// `DAT_00553E60`, the food band debug branch of `Minimap_ComputeBands`.
    pub food_bands: bool,
}

/// `screen` is `g_screenId`.
pub fn key_down(game: &mut Game, key: Key, screen: Option<u8>) {
    let dk = &mut game.debug_keys;
    match key {
        // `case 0x25: if (0 < g_debugSelectedMan) g_debugSelectedMan--;`
        // `case 0x27: if (g_debugSelectedMan < 0x50) g_debugSelectedMan++;`
        // arm: 0x004B29BE/debug-figure-step key
        Key::Left if dk.selected_man > 0 => dk.selected_man -= 1,
        Key::Right if dk.selected_man < LAST_MAN => dk.selected_man += 1,
        // `case 0x71`: 0 -> 1 -> 2 -> 0, on every screen.
        // arm: 0x004B29BE/debug-panel-cycle key
        Key::F2 => {
            dk.panel_mode = match dk.panel_mode {
                0 => 1,
                1 => 2,
                _ => 0,
            };
            // `if (g_screenId == '\t') DAT_00553e60 ^= 1`.
            // arm: 0x004B29BE/court-f2 key
            if screen == Some(0x09) {
                dk.food_bands = !dk.food_bands;
            }
        }
        // `case 0x72: if (DAT_0053f684 == 1) DAT_0055402c ^= 1`.
        // arm: 0x004B29BE/debug-panel-toggle key
        Key::F3 if dk.debug => dk.panel_on = !dk.panel_on,
        // `case 0x73`, gated on `DAT_0053f684 == 1`.
        // arm: 0x004B29BE/debug-add-hits key
        // [D] CNEW-net-local: world-changing, so never in a network game.
        Key::F4 if dk.debug && !game.net.lockstep => add_hits(game),
        _ => {}
    }
}

/// `g_battlePhase == 0`: `g_counties[3].population++`; else
/// `for (i = 1; i < 0x15; i++) g_battleMen[i].hits++`. [I] ours is 0-based:
/// `g_battleMen[i]` is `figures[i - 1]`.
fn add_hits(game: &mut Game) {
    match game.battle.as_mut() {
        None => {
            if let Some(c) = game.kingdom.counties.get_mut(3) {
                c.population += 1;
            }
        }
        Some(live) => {
            for f in live.runner.sim.figures.iter_mut().take(0x14) {
                f.hits = f.hits.wrapping_add(1);
            }
        }
    }
}

#[cfg(test)]
#[path = "debug_keys_tests.rs"]
mod tests;
