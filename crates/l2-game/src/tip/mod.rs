//! **The tip screens** — `Tip_Update` (`0x00476AA7`), `Tip_Show`
//! (`0x00476DA9`), the restore `FUN_00476E21` and the reset `FUN_00476A5D`.
//!
//! Twenty `L2.eng` groups, 200…219, of which fourteen are posted: the first-time
//! advice the original gives a new player — *"Game Objectives:"* on the
//! campaign map, *"The Town Center:"* in the village, *"Army Movement:"* the
//! first time an army is picked up. None of it existed here until this module,
//! and with it went forty narration files: the first line of every tip and the
//! twenty-seven chained takes after it. `docs/audio-triggers.md`.
//!
//! # Four functions and five globals
//!
//! ```c
//! void Tip_Update(void) {                          /* once a frame, Battle_Frame */
//!   if (g_optTipScreens && g_appPhase == 3) {
//!     if (DAT_004F0358 == 0) { …the ladder, below… }
//!     else DAT_004F0358--;
//!   }
//! }
//! void Tip_Show(int group) {
//!   if (g_screenId != 0x27) {
//!     DAT_004F0350 = g_screenId; g_screenId = 0x27;
//!     g_tipShown[group] = 1; DAT_0052F004 = 0;
//!     Msg_Enqueue(0, g_localPlayer, group, 0, g_tipCategory[group], 0, 0, 0);
//!   }
//! }
//! void FUN_00476E21(void) {                        /* from every Msg_Dismiss */
//!   if (g_screenId == 0x27) {
//!     g_screenId = DAT_004F0350; if (DAT_004F0350 == 0x27) g_screenId = 0;
//!     DAT_004F0358 = 0x14;
//!   }
//! }
//! void FUN_00476A5D(void) {                        /* App_WinMain, Opt_ToggleTipScreens */
//!   for (i = 0; i < 0x14; i++) g_tipShown[200 + i] = 0;
//!   DAT_004F0350 = 0; DAT_004F0358 = 0x14;
//! }
//! ```
//!
//! `[V]`, all four, read out of the decompilation.
//!
//! # Two corrections to what was on file
//!
//! **The twenty frames are not "after a screen is first opened".**
//!
//! `docs/symbols.md` said so. `DAT_004F0358` is written in exactly two places —
//! the reset and the restore — so the delay is a re-arm after start-up, after
//! the toggle and **after every tip is dismissed**, and nothing else. A screen
//! that opens with the counter already at zero gets its tip on the same frame.
//!
//! **"Once per game" is once per run.** `FUN_00476A5D` has two callers,
//! `App_WinMain` and `Opt_ToggleTipScreens`. `Game_NewGame` does not clear
//! `g_tipShown`, and neither does a load, so a player who starts a second game
//! without quitting sees none of the tips again. That is why [`Tips`] is carried
//! across the two places this engine replaces the whole [`crate::Game`] —
//! `screens::setup`'s start and `screens::saveload`'s load.
//!
//! # Screen `0x27` is a screen, and it is [`crate::screen::ScreenId::Tip`]
//!
//! `Tip_Show` does not open a window; it **changes `g_screenId`** and posts a
//! message. The window follows because `Msg_Pump` runs on `0x27`, and the
//! screen the player was on stops answering because `Screen_FrameInput`
//! dispatches on the byte. A scan of the decompilation finds no comparison of
//! `g_screenId` with `0x27` anywhere but `Msg_Pump`, `Tip_Show` and
//! `FUN_00476E21` — the one `case 0x27` in the image is `App_WndProc`'s
//! VK_RIGHT — so `0x27` has **no input arm and no painter of its own** `[I]`: a
//! jump-table dispatch would not show up as a comparison, and none was looked
//! for. [`crate::screens::tip`] is that screen: an overlay that draws nothing
//! and consumes every event.
//!
//! [`Tips::hosting`] is `g_screenId == 0x27`, and
//! [`crate::screen::Machine`] keeps a [`crate::screen::ScreenId::Tip`] on the
//! stack exactly while it is true.
//!
//! # What this module does not know
//!
//! The ladder tests the original's screen byte, and our stack is not a byte.
//!
//! [`View`] is the projection, and [`View::of`] is where every mapping is
//! written down — two of them are not one-to-one and are explained there.
//!
//! **The battle arms cannot fire, and that is recorded.**
//!
//! Tips 212, 214 and 215 are guarded by `g_screenId == 0 && g_battlePhase == 2`.
//!
//! Every write of `g_battlePhase = 2` found — `Battle_Start`, `FUN_00477C89`,
//! the skirmish set-up — writes `g_screenId = 0x29` beside it, and both battle
//! ends write `0x13` or `0x2E` before `g_battlePhase = 0`. And `Msg_Pump` opens
//! with *"if `g_battlePhase == 2` and a message is up, dismiss it"*, so a tip
//! posted during a battle would be closed the frame after it opened. No path
//! was found on which the guard holds across a frame. `[I]` — the search was
//! the phase writers and the two outcome functions.
//!
//! hundred `g_screenId = 0` writes. The arms are built as the original writes
//! them, and in this engine they are equally unreachable.
//!
//! not-encoded: see [`Tips`].

mod view;
pub use view::*;
mod update_part;
pub use update_part::*;
mod text;
pub use text::*;

use crate::message::{self, Record};
use crate::screen::ScreenId;
use crate::Game;

/// The first tip group. `FUN_00476A5D` clears `g_tipShown + 200` for `0x14`
/// bytes, which is what fixes the range.
pub const FIRST: u16 = 200;
/// How many groups the range holds.
pub const COUNT: usize = 20;

/// `DAT_004F0358`'s re-arm: `0x14` frames, written by `FUN_00476E21` and
/// `FUN_00476A5D` and by nothing else.
pub const DELAY: u8 = 0x14;

/// **`g_tipCategory` (`0x004D6ED8`)**, groups 200…219 — the message category
/// `Tip_Show` posts each group with.
///
/// `[V]` read out of the executable at `0x004D6ED8 + 200`. For every group with
/// words the category is the paragraph count plus four — `Msg_DrawWindow`'s
/// `0x05`…`0x09` arm draws `category − 4` paragraphs — and the unit test below
/// holds that against [`TEXT`]. `docs/formats/eng.md` §5.3 says it of all
/// twenty; the six label-only groups (203…205, 213, 215, 216) were not checked
/// here.
pub const CATEGORY: [u8; COUNT] = [7, 8, 9, 5, 5, 5, 5, 8, 5, 8, 7, 5, 6, 5, 7, 5, 5, 9, 7, 5];

/// The group numbers the ladder names, as the original writes them.
pub mod group {
    pub const OBJECTIVES: u16 = 200;
    pub const GETTING_STARTED: u16 = 201;
    pub const FOOD_AND_HAPPINESS: u16 = 202;
    pub const KINGDOM_OVERVIEW: u16 = 206;
    pub const TOWN_CENTRE: u16 = 207;
    pub const BLACKSMITH: u16 = 208;
    pub const ARMOURY: u16 = 209;
    pub const ARMY_MOVEMENT: u16 = 210;
    pub const INVASIONS: u16 = 211;
    pub const BATTLES: u16 = 212;
    pub const SIEGES: u16 = 214;
    pub const SIEGES_2: u16 = 215;
    pub const CASTLE_BUILDING: u16 = 217;
    pub const ADVANCED_OPTIONS: u16 = 218;
}

/// **The tip screens' state** — `g_tipShown[200..220]`, `DAT_004F0358`,
/// `g_screenId == 0x27`, `DAT_00553210` and `DAT_0052F004`'s reset.
///
/// not-encoded: per-peer display state, and per *run* —
/// see the module header. Nothing in the world reads any of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tips {
    /// `g_tipShown` (`0x004F0298`), from group 200.
    shown: [bool; COUNT],
    /// `DAT_004F0358` — frames until the ladder runs again.
    delay: u8,
    /// `g_screenId == 0x27`: a tip has been posted and nothing has dismissed a
    /// message since. `DAT_004F0350`, the screen to go back to, is the stack
    /// underneath [`crate::screen::ScreenId::Tip`] and needs no field.
    hosting: bool,
    /// **`DAT_00553210` — the invasion flag.** Set by `Unit_EnterCounty`
    /// (`0x004ABB36`) when one of the local player's units crosses into a county
    /// whose owner is not that unit's owner, and cleared by nothing but the
    /// ladder's last arm. See [`Tips::note_incursions`].
    invaded: bool,
    /// How many times `Tip_Show` has posted. `Tip_Show` zeroes `DAT_0052F004`,
    /// the chained takes' cursor, and that cursor is audio state
    /// [`crate::audio::Director`] keeps — so the reset crosses the seam as a
    /// count the director diffs, the same shape as
    /// [`crate::screen::Machine::clicks`], and nothing flows back.
    shows: u32,
}

impl Default for Tips {
    fn default() -> Tips {
        Tips::new()
    }
}

impl Tips {
    /// `FUN_00476A5D` as `App_WinMain` calls it: nothing shown, and twenty
    /// frames before the first tip.
    pub fn new() -> Tips {
        Tips { shown: [false; COUNT], delay: DELAY, hosting: false, invaded: false, shows: 0 }
    }

    /// **`FUN_00476A5D` (`0x00476A5D`)** — every tip unshown and the delay
    /// re-armed. `Opt_ToggleTipScreens` (`0x00434787`) calls it on *every*
    /// flip, off as well as on.
    pub fn reset(&mut self) {
        self.shown = [false; COUNT];
        self.delay = DELAY;
    }

    /// **`FUN_00476E21` (`0x00476E21`)** — the restore `Msg_Dismiss` runs on
    /// every dismissal. It acts only while `g_screenId` is `0x27`, so a tip
    /// window pulled on the campaign map after its host was already restored
    /// re-arms nothing when it closes.
    pub fn restore(&mut self) {
        if self.hosting {
            self.hosting = false;
            self.delay = DELAY;
        }
    }

    /// **`Screen_FrameInput`'s epilogue writing `g_screenId = 0` over the
    /// `0x27`** — the byte goes and `FUN_00476E21` never runs, so the screen
    /// the tip was shown over is **not** put back and [`DELAY`] is **not**
    /// re-armed.
    ///
    /// That is the whole difference from [`Tips::restore`], and it is the
    /// original's: the epilogue assigns the byte directly, where every
    /// dismissal goes through `Msg_Dismiss` → `FUN_00476E21`. With the delay
    /// left where it was — at zero, since the ladder has already run — the next
    /// frame on the campaign map may post the next tip at once.
    pub fn unhost(&mut self) {
        self.hosting = false;
    }

    /// Whether `g_tipShown[group]` is set.
    pub fn shown(&self, group: u16) -> bool {
        index(group).is_some_and(|i| self.shown[i])
    }

    /// `DAT_004F0358`.
    pub fn delay(&self) -> u8 {
        self.delay
    }

    /// `g_screenId == 0x27`.
    pub fn hosting(&self) -> bool {
        self.hosting
    }

    /// `DAT_00553210`.
    pub fn invaded(&self) -> bool {
        self.invaded
    }

    /// How many tips `Tip_Show` has posted. See the field.
    pub fn shows(&self) -> u32 {
        self.shows
    }

    /// **`Unit_EnterCounty`'s one line for the tips**, fed from the simulation's
    /// report:
    ///
    /// ```c
    /// if (g_counties[county].owner != unit.owner) {
    ///     if (unit.owner == g_localPlayer) DAT_00553210 = 1;
    /// ```
    ///
    /// The comparison is made in `l2-kingdom` at the crossing, where the owner
    /// is the owner at that instant, and reported as
    /// [`l2_kingdom::units_tick::Incursion`]; the *local player* test is made
    /// here, because `g_localPlayer` is a peer's and not the world's.
    pub fn note_incursions(&mut self, incursions: &[l2_kingdom::units_tick::Incursion], player: u8) {
        if incursions.iter().any(|i| i.owner == player) {
            self.invaded = true;
        }
    }
}

fn index(group: u16) -> Option<usize> {
    let i = group.checked_sub(FIRST)? as usize;
    (i < COUNT).then_some(i)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Group 215 is posted and has no words.** `Tip_Update`'s siege arm shows
    /// it right after 214, `g_tipCategory[215]` is 5 — one paragraph — and the
    /// group holds only its label, `"Sieges2"`. So the window would draw that
    /// label as a heading over an empty paragraph. It cannot be reached (see the
    /// module header), which is presumably why nobody noticed.
    #[test]
    fn the_second_siege_tip_is_a_label_with_nothing_under_it() {
        assert_eq!(CATEGORY[(group::SIEGES_2 - FIRST) as usize], 5);
        assert_eq!(transcribed(group::SIEGES_2, 0), "Sieges2");
        assert_eq!(transcribed(group::SIEGES_2, 1), "");
    }

    /// The table's claim, checked against our own transcription: the category
    /// is the paragraph count plus four for every group with words.
    #[test]
    fn every_category_is_its_groups_paragraph_count_plus_four() {
        for (g, s) in TEXT {
            if s.len() < 2 {
                continue;
            }
            let c = CATEGORY[(*g - FIRST) as usize] as usize;
            assert_eq!(c, s.len() - 1 + 4, "group {g}");
        }
    }
}

