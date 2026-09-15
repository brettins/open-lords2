//! `Smk_Play` (`0x0042D91B`) is the one door every film goes through — it
//! opens the file, parks `g_screenId` at `0x22`, and remembers the screen to
//! go back to — and it has exactly seven callers, which are the whole of the
//! original's video. [`Film`] is those seven, one variant per call site, and
//! the rest of this module is the tables and ladders the call sites read.
//!
//! | caller | address | film | at | [`Film`] |
//! |---|---|---|---|---|
//! | `FUN_004B3571(0)`, from `App_WinMain` | `0x004B3571` | `intro.smk` | (40, 80) | [`Film::Intro`] |
//! | `Smk_OnFinished`, start-up only | `0x0042E060` | `imptitle.smk`, then `credits.smk` | (80, 80), (0, 0) | [`Film::ImpTitle`], [`Film::Credits`] |
//! | `FUN_00432B05` hotspot 4, setup page 1 | `0x00432B05` | `lom.smk` | (70, 80) | [`Film::LordsOfMagic`] |
//! | `CastleBuild_Confirm` | `0x00436B59` | `castle1`…`castle5.smk` | (158, 20) | [`Film::Castle`] |
//! | `Msg_DrawWindow`, category `0x0D` | `0x0047309E` | `cap_cty1`…`3.smk` in rotation | (40, 105) | [`Film::Capture`] |
//! | `Msg_DrawWindow`, category `0x0E` | `0x0047309E` | `FUN_00475B41`'s choice | (89, 105) | [`Film::Ending`] |
//! | `Battle_CheckOutcome` | `0x00477DFC` | one of 48 table entries | (39, 73) | [`Film::Battle`] |
//! | `Smk_ReplayIntro`, the debug viewer | `0x0042E381` | any of forty | (39, 73) | **not built** — see below |
//!
//! Every address and every coordinate above is `[V]`, read off the call site
//! in the decompilation; every file name is `[V]`, dumped out of `.rdata` at
//! the address the call site indexes.
//!
//! * **`Smk_ReplayIntro`** is the replay button of screen `0x44`, a Smacker
//!   test page (`g_smackTestWidgets`, `0x004DDFA0`, painted by `0x00425A6A`).
//!
//!   Scanning the whole image for `mov byte ptr [g_screenId], imm8` finds 52
//!   distinct immediates and **no `0x44`**; the other 48 stores are `mov
//!   [g_screenId], al` restoring a remembered screen, which cannot hold a value
//!   nothing ever wrote. `[D]` — so the page and its forty-name table
//!   (`s_Intro_smk_004d4d60`) are a debug tool the shipped game cannot open.
//!
//! * **`Smk_PlayThenClose`** (`0x0042D96E`) has no caller at all.
//!
//! * **The CD's `smk_high` directory.** `Msg_DrawWindow`'s ending branch has a
//!   second layout — a 502 × 314 well at (24, 80) — taken when `g_fastMedia` is
//!   set, and `Cd_PathForFile` then reads the film from `smk_high`. The flag is
//!   set only when `sierra.ini` reports a CD drive faster than 4× on a CPU
//!   faster than 70 MHz (`0x0040EE32`); this install has no `sierra.ini` and its
//!   films are the 296 × 184 ones the *slow* well fits. `[I]` that every
//!   hard-disk install takes the slow branch.

mod player;
pub use player::*;
mod subtitles;
pub use subtitles::*;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use l2_smk::{Decoder, Smk};

use crate::message::Record;
use crate::screen::{Machine, ScreenId, Transition};

/// **`FUN_004B3571(0)`** — `App_WinMain`'s last act before the message loop:
pub fn start_up(machine: &mut Machine) {
    machine.push(ScreenId::Movie(Film::Intro));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Film {
    /// `FUN_004B3571` — `Smk_Play("intro.smk", 0x28, 0x50, 0, g_screenId)`.
    Intro,
    ImpTitle,
    Credits,
    /// `FUN_00432B05`'s hotspot 4, *"Lords of Magic?"* on the title page —
    /// Sierra's trailer for its next game. `Smk_Play("lom.smk", 0x46, 0x50, 0,
    /// g_screenId)`.
    LordsOfMagic,
    Castle(u8),
    /// `Msg_DrawWindow`'s animated capture branch. `take` is `DAT_00553ED4`
    /// after its increment, 0…2; `record` is the message it dismissed.
    Capture { take: u8, record: Record },
    Ending { file: &'static str, record: Record, game_over: bool },
    Battle { file: &'static str },
}

pub const CASTLE_FILMS: [&str; 5] =
    ["castle1.smk", "castle2.smk", "castle3.smk", "castle4.smk", "castle5.smk"];

pub const CAPTURE_FILMS: [&str; 3] = ["cap_cty1.smk", "cap_cty2.smk", "cap_cty3.smk"];

pub const ENDING_FILMS: [[&str; 5]; 5] = [
    ["cart_kgt.smk", "pill_kgt.smk", "jail.smk", "hang.smk", "axmen.smk"],
    ["cart_kgt.smk", "pill_kgt.smk", "jail.smk", "hang.smk", "axmen.smk"],
    ["cart_brn.smk", "pill_brn.smk", "jail.smk", "hang.smk", "axmen.smk"],
    ["pill_cts.smk", "pill_cts.smk", "jail.smk", "hang.smk", "axmen.smk"],
    ["cart_bsp.smk", "cart_bsp.smk", "jail.smk", "hang.smk", "axmen.smk"],
];

/// `0x004D7058` — group `0xE1`, the player's own victory.
pub const VICTORY_FILM: &str = "win_game.smk";

/// `DAT_00553228`: the year a new game starts in, written once, by
/// `Game_NewGame` (`0x00497CED`: `g_year = 0x4F3; DAT_00553228 = 0x4F3;`).
///
/// `[V]` — one writer in the corpus.
pub const FIRST_YEAR: i32 = 0x4F3;

/// **`FUN_00475B41(realm, group)`** — which film a fallen lord gets.
///
/// ```c
/// if (group == 0xE1) "win_game.smk";
/// else if (!humanSlot[realm])              /* DAT_00553D77 + realm * 0x2C */
///     row = lord * 5;  years = g_year - DAT_00553228;
///     years < 6 ? cart : years < 12 ? pillory : years < 18 ? jail : years < 24 ? hang : axmen
/// else years < 12 ? jail : years < 32 ? hang : axmen     /* row 0 */
/// ```
///
/// **The later the fall, the worse the end.** A computer lord taken in the
/// first six years leaves in a cart; one who lasts twenty-four meets the axe. A
/// human is never carted or pilloried. `DAT_00553D77` is the player slot's
/// human flag — `FUN_0049BAE9` sets it and `g_realms[r].isHuman` to 1 together
/// and `FUN_0049BB9D` clears both together — so `l2_kingdom`'s
/// `Realm::is_human` is read for it. `[D]`: `Realm_Eliminate` also sets the
/// slot flag alone, on a realm that is by then out of the game.
pub fn ending_film(game: &crate::Game, realm: u8, group: u16) -> &'static str {
    if group == l2_kingdom::victory::MSG_VICTORY {
        return VICTORY_FILM;
    }
    let years = game.kingdom.year - FIRST_YEAR;
    let r = game.kingdom.realms.get(realm as usize);
    if r.is_some_and(|r| r.is_human) {
        let col = if years < 12 {
            2
        } else if years < 32 {
            3
        } else {
            4
        };
        return ENDING_FILMS[0][col];
    }
    let lord = r.map_or(0, |r| r.lord as usize).min(4);
    let col = match years {
        y if y < 6 => 0,
        y if y < 12 => 1,
        y if y < 18 => 2,
        y if y < 24 => 3,
        _ => 4,
    };
    ENDING_FILMS[lord][col]
}

/// `s_bat_win1_smk_004d9278` — **six outcomes of four**, indexed
/// `g_battleOutcome * 4 + DAT_0053F084`: won, lost, took the castle, driven
/// off it, held it, lost it. Taken when `DAT_0057A0F0` is clear, which is
/// every campaign battle.
pub const BATTLE_FILMS: [[&str; 4]; 6] = [
    ["bat_win1.smk", "bat_win2.smk", "bat_win3.smk", "bat_win4.smk"],
    ["bat_los1.smk", "bat_los2.smk", "bat_los3.smk", "bat_los4.smk"],
    ["cas_win1.smk", "cas_win2.smk", "cas_win1.smk", "cas_win2.smk"],
    ["cas_los1.smk", "cas_los2.smk", "cas_los1.smk", "cas_los2.smk"],
    ["sge_win1.smk", "sge_win2.smk", "sge_win1.smk", "sge_win2.smk"],
    ["sge_los1.smk", "sge_los2.smk", "sge_los1.smk", "sge_los2.smk"],
];

/// `s_bat_win5_smk_004d93f8` — the same shape, taken when `DAT_0057A0F0` is
/// set: the third battle mode, which is the only reach of `bat_win5`,
/// `bat_win6`, `bat_los5`, `bat_los6`, `cas_win3` and `cas_los3` — the six
/// films dated 1997 in an install whose others are dated 1995, so `[I]` the
/// patch's. **Not reachable here**: the mode is the one
/// [`crate::audio::track::BattleKind`] also declines to name.
pub const BATTLE_FILMS_THIRD_MODE: [[&str; 4]; 6] = [
    ["bat_win5.smk", "bat_win6.smk", "bat_win3.smk", "bat_win4.smk"],
    ["bat_los5.smk", "bat_los6.smk", "bat_los3.smk", "bat_los4.smk"],
    ["cas_win3.smk", "cas_win2.smk", "cas_win3.smk", "cas_win1.smk"],
    ["cas_los3.smk", "cas_los2.smk", "cas_los3.smk", "cas_los1.smk"],
    ["sge_win1.smk", "sge_win2.smk", "sge_win1.smk", "sge_win2.smk"],
    ["sge_los1.smk", "sge_los2.smk", "sge_los1.smk", "sge_los2.smk"],
];

impl Film {
    pub fn file(&self) -> &'static str {
        match *self {
            Film::Intro => "intro.smk",
            Film::ImpTitle => "imptitle.smk",
            Film::Credits => "credits.smk",
            Film::LordsOfMagic => "lom.smk",
            Film::Castle(level) => CASTLE_FILMS[(level as usize).min(4)],
            Film::Capture { take, .. } => CAPTURE_FILMS[(take as usize).min(2)],
            Film::Ending { file, .. } | Film::Battle { file } => file,
        }
    }

    pub fn at(&self) -> (i32, i32) {
        match self {
            Film::Intro => (0x28, 0x50),
            Film::ImpTitle => (0x50, 0x50),
            Film::Credits => (0, 0),
            Film::LordsOfMagic => (0x46, 0x50),
            Film::Castle(_) => (0x9E, 0x14),
            Film::Capture { .. } => (0x28, 0x69),
            Film::Ending { .. } => (0x59, 0x69),
            Film::Battle { .. } => (0x27, 0x49),
        }
    }

    /// The front-end films are preceded by `FUN_004B11CE` —
    /// `Palette_Set`, `FUN_004B1867` (which hands the whole back buffer to
    /// `FUN_004B3E51` — a clear, `[I]` from the length) and a repaint — so they
    /// play on black. The other four play over the screen that raised them,
    /// which is still in the back buffer because screen `0x22` has no painter:
    pub fn is_over_a_screen(&self) -> bool {
        !matches!(self, Film::Intro | Film::ImpTitle | Film::Credits | Film::LordsOfMagic)
    }

    pub fn is_front_end(&self) -> bool {
        !self.is_over_a_screen()
    }

    /// **`Smk_OnFinished` (`0x0042E060`)** — what the end of this film, or a
    /// skip, does next.
    ///
    /// # What each call site, `[V]` at all eight
    ///
    /// | call site | `returnScreen` | here |
    /// |---|---|---|
    /// | `FUN_004B3571` intro | `g_screenId` | the start-up chain |
    /// | `Smk_OnFinished` logo | `0x1F` | the start-up chain |
    /// | `Smk_OnFinished` credits | `0x1F` | [`Transition::Pop`], onto the front end |
    /// | `FUN_00432B05` trailer | `g_screenId` | [`Transition::Pop`] |
    /// | **`CastleBuild_Confirm`** | **`0`** | **[`Transition::Goto`]`(Campaign)`** |
    /// | `Msg_DrawWindow` capture | `g_screenId` | [`Transition::Pop`] |
    /// | `Msg_DrawWindow` ending, both media layouts | `g_screenId`, read after `Msg_Dismiss` | [`Transition::Pop`], or [`Transition::Replace`]`(Conquest)` when that dismissal entered `0x1C` |
    /// | `Battle_CheckOutcome` | `g_screenId` | [`Transition::Pop`] |
    pub fn then(&self) -> Transition {
        match self {
            Film::Intro => Transition::Replace(ScreenId::Movie(Film::ImpTitle)),
            Film::ImpTitle => Transition::Replace(ScreenId::Movie(Film::Credits)),
            Film::Ending { game_over: true, .. } => Transition::Replace(ScreenId::Conquest),
            Film::Castle(_) => Transition::Goto(ScreenId::Campaign),
            _ => Transition::Pop,
        }
    }

    /// **When `Smk_Open` fails** — a file the install does not have, or one
    /// that will not parse. `Smk_Play` then sets `g_screenId` to the return
    /// screen directly and **`Smk_OnFinished` is never called**, so nothing
    /// chains: a missing `intro.smk` goes straight to the title page
    /// (`FUN_004B3571`'s `DAT_005C9274 = 1`).
    pub fn on_failure(&self) -> Transition {
        match self {
            Film::Ending { game_over: true, .. } => Transition::Replace(ScreenId::Conquest),
            Film::Castle(_) => Transition::Goto(ScreenId::Campaign),
            _ => Transition::Pop,
        }
    }

    /// **`Msg_PlayVoice(DAT_004F0374, DAT_004F0354)`** — the capture and ending
    /// branches' last statement, after `Smk_Play` has returned. `Smk_Play`
    /// returns as soon as the first frame is up, so the narrator reads the
    /// message **over the start of the film**, not after it. The two globals
    /// are the group and variant `Msg_DrawWindow` saved on its first line.
    pub fn voice(&self) -> Option<(u16, u8)> {
        match self {
            Film::Capture { record, .. } | Film::Ending { record, .. } => {
                Some((record.group, record.variant))
            }
            _ => None,
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Film::Intro | Film::ImpTitle | Film::Credits => "Lords of the Realm II",
            Film::LordsOfMagic => "Lords of Magic",
            Film::Castle(_) => "Construction begins",
            Film::Capture { .. } => "A county is taken",
            Film::Ending { .. } => "The fall of a lord",
            Film::Battle { .. } => "The battle is decided",
        }
    }
}

