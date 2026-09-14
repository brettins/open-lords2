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

/// **The armoury's animation state** — the counters `Tick_Pulses` owns and the
/// soldier `Armoury_ClickRack` sends.
///
/// It is one struct because the original's `Screen_DrawWidgets` arm is one
/// three-call line shared by `0x0A` and `0x0D`, and because only the top screen
/// of our stack gets a tick: the rack panel is pushed *over* the armoury, so if
/// this lived on either screen the other's would stop. Both step this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Anim {
    /// Milliseconds of fixed tick accumulated toward the next 20 ms pulse.
    /// **Not a clock**: [`TICK_MS`] is a constant and this counts ticks.
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
    /// One fixed tick. Returns whether anything on screen changed, which is
    /// what `Screen::take_redraw` is for: a still armoury with no soldier in
    /// it still has two torches, so this is true roughly every fifth tick and
    /// not every one.
    ///
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
    /// **This used to subtract twenty and keep the rest**, which is the one
    /// reading under which the walk is exactly 200 pixels a second on every
    /// machine — a rate the original reaches only on a frame of exactly 20 ms
    /// and never above it. A player: *"his animation speed was faster than the
    /// regular game. not bad, but not the OG."* It was 1.6 times faster.
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

/// Screen `0x0A` for one county's levy.
pub struct ArmouryScreen {
    county: u8,
    /// One line of feedback. **Ours.**
    status: String,
    /// What the last *Create* did, for a test that wants to know without
    /// reading the kingdom.
    pub outcome: Raised,
    /// Set when [`Anim::tick`] moved something, so the machine repaints without
    /// an event having arrived — the torches gutter on a screen nobody is
    /// touching. Same mechanism as the campaign map's edge scroll.
    redraw: bool,
}

/// What the *Create* button did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raised {
    None,
    Army(usize),
    Refused(LevyRefusal),
}

// -------------------------------------------------------- 0x0D, one weapon

/// Screen `0x0D` — one rack, opened from the armoury.
pub struct RackScreen {
    county: u8,
    /// `DAT_00553F20`, 1…6. Held here as well as on the order because the
    /// screen is identified by it.
    troop: u8,
    /// See [`ArmouryScreen`] — the weapon in the well turns, and the soldier
    /// walks, on a screen nobody is touching.
    redraw: bool,
}

