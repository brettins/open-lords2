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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LevyOrder {
    pub county: u8,
    /// `g_levyPercent` (`0x0056D65C`) — **where the player put the slider**,
    /// which is not necessarily what the county gave up.
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
    ///
    /// `FUN_004AA90A` clears it whenever the basket is re-seeded.
    pub rack: u8,
    /// **The armoury's animation state** — `DAT_005679D0`, `DAT_0056D630`,
    /// `DAT_0052F008`, `DAT_0057CB10`, `DAT_005681F8`, `DAT_00568228` and the
    /// two frame counters `Tick_Pulses` steps for the torches and the turning
    /// weapon. Six more globals the armoury and the rack panel share,
    /// exactly why they are here beside the other five.
    pub anim: crate::screens::armoury::Anim,
}

