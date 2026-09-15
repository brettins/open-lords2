#![allow(unused_imports)]
use super::*;
use super::view::*;
use super::text::*;
use crate::message::{self, Record};
use crate::screen::ScreenId;
use crate::Game;

/// **`Tip_Update` (`0x00476AA7`)**, the whole ladder. Returns the group it
/// hands to `Tip_Show`, which may still refuse — see [`show`].
///
/// * the campaign map's four tips are **one per re-arm**, in the order 206
/// (only when zoomed out), 200, 201, 202 — so a player meets three windows
///   in a row with twenty frames between each;
/// * the invasion arm is the **last `else if` and has no screen test**, and it
///   clears `DAT_00553210` *before* asking whether 211 was shown. So the flag is
///   consumed by any frame that reaches that arm — including one on screen
/// `0x27`, where `Tip_Show` then refuses and the tip is lost until the next
///   crossing. Reproduced; `docs/bugs.md` B101.
pub fn update(tips: &mut Tips, view: &View) -> Option<u16> {
    use group as g;
    if !(view.enabled && view.in_play) {
        return None;
    }
    if tips.delay != 0 {
        tips.delay -= 1;
        return None;
    }
    let shown = tips.shown;
    let unshown = move |group: u16| index(group).is_some_and(|i| !shown[i]);
    let screen = view.screen;

    if screen == Some(0x00) && !view.battle {
        if view.zoom_far && unshown(g::KINGDOM_OVERVIEW) {
            return Some(g::KINGDOM_OVERVIEW);
        }
        for group in [g::OBJECTIVES, g::GETTING_STARTED, g::FOOD_AND_HAPPINESS] {
            if unshown(group) {
                return Some(group);
            }
        }
    }
    if screen == Some(0x00) && view.battle {
        if view.skirmish {
            return None;
        }
        if !view.siege {
            if unshown(g::BATTLES) {
                return Some(g::BATTLES);
            }
        } else {
            if unshown(g::SIEGES) {
                return Some(g::SIEGES);
            }
            if unshown(g::SIEGES_2) {
                return Some(g::SIEGES_2);
            }
        }
    }
    if screen == Some(0x17) && unshown(g::ARMOURY) {
        Some(g::ARMOURY)
    } else if screen == Some(0x0F) && view.job == 8 && unshown(g::BLACKSMITH) {
        Some(g::BLACKSMITH)
    } else if screen == Some(0x02) && unshown(g::TOWN_CENTRE) {
        Some(g::TOWN_CENTRE)
    } else if screen == Some(0x39) && unshown(g::ADVANCED_OPTIONS) {
        Some(g::ADVANCED_OPTIONS)
    } else if screen == Some(0x1B) && unshown(g::CASTLE_BUILDING) {
        Some(g::CASTLE_BUILDING)
    } else if screen == Some(0x10) && unshown(g::ARMY_MOVEMENT) {
        Some(g::ARMY_MOVEMENT)
    } else if tips.invaded {
        tips.invaded = false;
        unshown(g::INVASIONS).then_some(g::INVASIONS)
    } else {
        None
    }
}

/// **`Tip_Show` (`0x00476DA9`).** Returns whether it posted.
pub fn show(game: &mut Game, group: u16) -> bool {
    let Some(i) = index(group) else { return false };
    if game.tips.hosting {
        return false;
    }
    game.tips.hosting = true;
    game.tips.shown[i] = true;
    game.tips.shows = game.tips.shows.wrapping_add(1);
    let player = game.player;
    let record = Record {
        to: player,
        from: 0,
        group,
        variant: 0,
        category: CATEGORY[i],
        county: 0,
        spare: 0,
        payload: 0,
    };
    game.messages.enqueue(record, player);
    true
}

pub fn tick(game: &mut Game, view: &View) -> Option<u16> {
    let group = update(&mut game.tips, view)?;
    show(game, group).then_some(group)
}


