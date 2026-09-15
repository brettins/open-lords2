#![allow(unused_imports)]

mod render;
pub use render::*;
mod armoury;
pub use armoury::*;
mod rack;
pub use rack::*;

use super::*;
use super::walker::*;
use super::tests::*;
use l2_kingdom::levy::{self, LevyRefusal};
use l2_kingdom::tables::WEAPON_TYPE_COUNT;
use l2_kingdom::unit::TroopType;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};
use crate::widget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Anim {
    acc_ms: u32,
    /// `DAT_005AEB2C`, the 20 ms counter whose fourth step is `g_pulse80`.
    div: u8,
    /// `DAT_005AEA54`, 0…12 — the torches.
    pub torch: u8,
    /// `DAT_005AEA48`, 0…23 — the weapon turning in the rack panel's well.
    pub weapon: u8,
    pub walker: Walker,
}

impl Anim {
    /// **At most one pulse a tick, and the remainder is thrown away.** Both are
    /// `Tick_Pulses` (`0x004BBC80`), which `Battle_Frame` calls once a frame:
    ///
    /// ```c
    /// now = timeGetTime();
    /// if (0x13 < (int)(now - stamp) || (int)(now - stamp) < 0) {
    ///     DAT_005AEB2C++;  DAT_0058FCB0 = 1;  stamp = now;     /* now, not +20 */
    /// }
    /// ```
    ///
    /// So the pulse comes on the **first frame at least 20 ms after the last
    /// pulse**, and on a 16 ms frame that is every second frame — 32 ms, not
    /// 20. `[V]` for the gate; *our tick is the frame* is the reading
    /// [`crate::press`] already makes of `FUN_004B20ED`, the 30 ms gate beside
    /// it, which resets its stamp the same way.
    ///
    /// `docs/decisions.md` C179.
    pub fn tick(&mut self) -> bool {
        self.acc_ms += TICK_MS;
        if self.acc_ms < PULSE_MS {
            return false;
        }
        self.acc_ms = 0;
        self.div += 1;
        let pulse80 = self.div >= PULSE80_DIVIDER;
        let mut moved = false;
        if pulse80 {
            self.div = 0;
            self.torch = (self.torch + 1) % TORCH_FRAMES;
            self.weapon = (self.weapon + 1) % WEAPON_FRAMES;
            moved = true;
        }
        if self.walker.active {
            self.walker.pulse(pulse80);
            moved = true;
        }
        moved
    }
}

pub struct ArmouryScreen {
    county: u8,
    status: String,
    pub outcome: Raised,
    redraw: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raised {
    None,
    Army(usize),
    Refused(LevyRefusal),
}


pub struct RackScreen {
    county: u8,
    /// `DAT_00553F20`, 1…6. Held here as well as on the order because the
    /// screen is identified by it.
    troop: u8,
    redraw: bool,
}

