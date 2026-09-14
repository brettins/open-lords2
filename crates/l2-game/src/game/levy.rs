#![allow(unused_imports)]
use super::*;
use super::unit_frames::*;
use super::game_methods::*;
use l2_formats::maps::{MapSet, MapSlot};
use l2_formats::Palette;
use l2_kingdom::county::{LABOUR_CEILING_IGNORED, MAX_COUNTIES};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;
use l2_kingdom::{Kingdom, SeasonReport};
use l2_mods::vfs::Vfs;
use l2_view::campaign::{self, MapAssets};
use l2_view::chrome::{Chrome, Minimap};
use l2_view::village::VillageArt;
use l2_view::Ink;
use crate::shell::ShellAssets;
use l2_kingdom::tables::MAX_TAX_RATE;
use l2_kingdom::county::MAX_RATION_SPLIT;

/// `g_levyPercent`, `g_levyMen`, `g_levyHappinessCost`, `g_levyBasket` and
/// `DAT_0055446C` — **one order, three screens.**
///
/// The original has no stack. `g_screenId` is a byte, and the raise-army screen
/// (`0x17`), the armoury (`0x0A`) and one weapon's rack (`0x0D`) are three
/// values of it that all read and write the same globals: the slider on `0x17`
/// writes `men` and `happiness_cost`, the `+`/`−` on `0x0D` move men between
/// `basket` slots, and `Army_RaiseConfirm` — which is a button on the
/// **armoury**, not on the levy screen — spends the lot.
///
/// So it cannot live in a screen. Our machine destroys a screen the moment it
/// is replaced, and `0x17 → 0x0A → 0x17` is two replacements; anything the
/// player chose in between would go with them. It lives here because it lives
/// in the original's data segment, which is the same reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LevyOrder {
    /// The county being levied. 0
    pub county: u8,
    /// `g_levyPercent` (`0x0056D65C`) — **where the player put the slider**,
    /// which is not necessarily what the county gave up.
    ///
    /// `Sidebar_Button` does not reset it: it calls `Levy_SetPercent(county,
    /// g_levyPercent)` with whatever the last levy left there, so opening the
    /// screen for a second county starts at the first county's percentage.
    /// Reproduced — [`Game::open_levy`] takes no percentage.
    pub percent: i32,
    /// `g_levyMen` (`0x00543FD8`) and `g_levyHappinessCost` (`0x00565400`),
    /// both written by `Levy_SetPercent` and by nothing else.
    pub men: i32,
    pub happiness_cost: i32,
    /// `g_levyBasket[g_localPlayer]` (`0x0053F6A0`) — who is carrying what.
    pub basket: l2_kingdom::LevyBasket,
    /// `DAT_0055446C` — the mercenary hire flag, cleared by `Sidebar_Button`
    /// every time the screen opens and toggled by the tick and cross on it.
    pub hire: bool,
    /// `DAT_00553F20` — the rack the player last opened, 1…6, or 0 for none.
    /// `FUN_004AA90A` clears it whenever the basket is re-seeded.
    pub rack: u8,
    /// **The armoury's animation state** — `DAT_005679D0`, `DAT_0056D630`,
    /// `DAT_0052F008`, `DAT_0057CB10`, `DAT_005681F8`, `DAT_00568228` and the
    /// two frame counters `Tick_Pulses` steps for the torches and the turning
    /// weapon. Six more globals the armoury and the rack panel share,
    /// exactly why they are here beside the other five.
    ///
    /// **Display state, and it must stay that way.** Nothing in
    /// [`l2_kingdom::Kingdom`] reads it and nothing writes it from the
    /// simulation; it is on `LevyOrder` because `LevyOrder` is already session
    /// state the save resets and the lockstep digest cannot see
    /// (`docs/netcode.md`). See [`crate::screens::armoury::Anim`].
    pub anim: crate::screens::armoury::Anim,
}

