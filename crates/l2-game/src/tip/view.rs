#![allow(unused_imports)]
use super::*;
use super::update_part::*;
use super::text::*;
use crate::message::{self, Record};
use crate::screen::ScreenId;
use crate::Game;

/// **What `Tip_Update` reads**, projected out of our stack and world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct View {
    /// `g_optTipScreens`.
    pub enabled: bool,
    /// `g_appPhase == 3`.
    pub in_play: bool,
    /// `g_screenId`, or `None` for a screen the ladder never names and whose
    /// byte is therefore not written down here.
    pub screen: Option<u8>,
    /// `g_jobPanelJob`, 1…9.
    pub job: usize,
    /// `g_mapZoom == 2`.
    pub zoom_far: bool,
    /// `g_battlePhase == 2`.
    pub battle: bool,
    /// `g_battleIsSiege`.
    pub siege: bool,
    /// `DAT_0057A0F0` — the Battle Master skirmish, which the tips skip.
    pub skirmish: bool,
}

impl View {
    /// **The projection, one question at a time.**
    ///
    /// * **`g_screenId`** is the top screen that is not the message scroll — the
    /// scroll is not a screen id in the original (`crate::screens::message`) —
    ///   answered by [`crate::screen::Screen::mode_screen_id`] where the
    /// original's byte is not a function of our [`ScreenId`], and by
    ///   [`screen_byte`] otherwise. The one screen that needs the first is the
    ///   campaign map: `Map_BeginMoveSelection` writes `g_screenId = 0x10`, the
    ///   only writer of that value in the image (`docs/screens.md` §9.5), and
    ///   ours keeps the same thing as `MapScreen::move_order`.
    /// * **`g_appPhase == 3`** is *the campaign is on the stack*. `Setup_StartGame`
    ///   and its two network twins write 3 as they enter the game, and every
    ///   writer of 2 leaves it for the front end, which here pops the campaign.
    /// * **`g_battlePhase == 2`** is *the battlefield is on the stack* — the same
    ///   reading `crate::audio::scene` makes, because the phase outlives the
    ///   field's three screen ids.
    /// * **`DAT_0057A0F0`** is set by the Battle Master's two set-up functions
    ///   (`0x0042B919` and its sibling) and cleared by `FUN_00497A34` on the
    /// campaign route.
    pub fn of(machine: &crate::screen::Machine, game: &Game) -> View {
        let ids = machine.ids();
        let job = match machine.top_screen_id() {
            Some(ScreenId::Job(_, j)) => j + 1,
            _ => 0,
        };
        View {
            enabled: game.prefs.tip_screens,
            in_play: ids.contains(&ScreenId::Campaign),
            screen: machine.top_screen_byte(game),
            job,
            zoom_far: game.map_zoom_far,
            battle: ids.contains(&ScreenId::Battlefield),
            siege: game.battle.as_ref().is_some_and(|b| b.is_siege()),
            skirmish: false,
        }
    }
}

/// **The original's `g_screenId` for a [`ScreenId`]**, for the screens the tip
/// ladder asks about, and `None` for the rest.
///
/// Exhaustive, with no wildcard: adding a screen is a compile error here until
/// somebody has said whether the tips care about it. `None` is not *"has no
/// byte"* — every screen of the original's has one — it is *"not one the
/// ladder names, so which byte it is was not needed and is not asserted"*.
pub fn screen_byte(id: ScreenId, game: &Game) -> Option<u8> {
    use ScreenId as S;
    match id {
        S::Campaign => Some(0x00),
        S::Village(_) => Some(0x02),
        S::Job(..) => Some(0x0F),
        S::RaiseArmy(_) => Some(0x17),
        S::Castle(_) => Some(0x1B),
        // A film is `g_screenId` `0x22`; no rung of the ladder tests it.
        S::Movie(_) => Some(0x22),
        S::Tip => Some(0x27),
        S::Options(page) => page.screen_id(),
        S::SaveLoad(mode) => Some(mode.screen_id()),
        S::Battlefield => Some(game.battle.as_ref().map_or(0x29, |b| b.screen_id())),
        S::Menu
        | S::Index
        | S::Message
        | S::County(..)
        | S::Setup(_)
        | S::Conquest
        | S::Diplomacy
        | S::DiploCompose(..)
        | S::Siege(_)
        | S::Armoury(_)
        | S::Rack(..)
        | S::Divide(_)
        | S::Merchant(_)
        | S::Trade(..)
        | S::BattlePrompt
        | S::BattleResult
        | S::MenuBar(_)
        | S::About
        | S::Court
        | S::Nobles
        | S::Supplies(_)
        | S::Ratings
        | S::Info(_) => None,
    }
}

