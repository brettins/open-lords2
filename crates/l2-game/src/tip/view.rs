#![allow(unused_imports)]
use super::*;
use super::update_part::*;
use super::text::*;
use crate::message::{self, Record};
use crate::screen::ScreenId;
use crate::Game;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct View {
    pub enabled: bool,
    pub in_play: bool,
    pub screen: Option<u8>,
    pub job: usize,
    pub zoom_far: bool,
    pub battle: bool,
    pub siege: bool,
    /// `DAT_0057A0F0` — the Battle Master skirmish, which the tips skip.
    pub skirmish: bool,
}

impl View {
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

pub fn screen_byte(id: ScreenId, game: &Game) -> Option<u8> {
    use ScreenId as S;
    match id {
        S::Campaign => Some(0x00),
        S::Village(_) => Some(0x02),
        S::Job(..) => Some(0x0F),
        S::RaiseArmy(_) => Some(0x17),
        S::Castle(_) => Some(0x1B),
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
        | S::Info(_)
        | S::Confirm(_) => None,
    }
}

