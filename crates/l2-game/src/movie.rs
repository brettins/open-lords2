//! **The game's films: which one plays, when, where, and what happens after.**
//!
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
//! # What is not here, and why
//!
//! * **`Smk_ReplayIntro`** is the replay button of screen `0x44`, a Smacker
//!   test page (`g_smackTestWidgets`, `0x004DDFA0`, painted by `0x00425A6A`).
//!   Scanning the whole image for `mov byte ptr [g_screenId], imm8` finds 52
//!   distinct immediates and **no `0x44`**; the other 48 stores are `mov
//!   [g_screenId], al` restoring a remembered screen, which cannot hold a value
//!   nothing ever wrote. `[D]` — so the page and its forty-name table
//!   (`s_Intro_smk_004d4d60`) are a debug tool the shipped game cannot open.
//! * **`Smk_PlayThenClose`** (`0x0042D96E`) has no caller at all.
//! * **The CD's `smk_high` directory.** `Msg_DrawWindow`'s ending branch has a
//!   second layout — a 502 × 314 well at (24, 80) — taken when `g_fastMedia` is
//!   set, and `Cd_PathForFile` then reads the film from `smk_high`. The flag is
//!   set only when `sierra.ini` reports a CD drive faster than 4× on a CPU
//!   faster than 70 MHz (`0x0040EE32`); this install has no `sierra.ini` and its
//!   films are the 296 × 184 ones the *slow* well fits. `[I]` that every
//!   hard-disk install takes the slow branch.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use l2_smk::{Decoder, Smk};

use crate::message::Record;
use crate::screen::{Machine, ScreenId, Transition};

/// **`FUN_004B3571(0)`** — `App_WinMain`'s last act before the message loop:
/// play `intro.smk`, and on failure go straight to the front end.
///
/// A function in the library rather than two lines in `main.rs`, for
/// `crate::audio::Director`'s reason: a binary's code cannot be called by a
/// test.
pub fn start_up(machine: &mut Machine) {
    machine.push(ScreenId::Movie(Film::Intro));
}

/// One call of `Smk_Play`, as a value a screen can name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Film {
    /// `FUN_004B3571` — `Smk_Play("intro.smk", 0x28, 0x50, 0, g_screenId)`.
    Intro,
    /// `Smk_OnFinished`'s first `strcmp`: during start-up, the end of
    /// `intro.smk` plays `imptitle.smk` at (0x50, 0x50).
    ImpTitle,
    /// …and the end of `imptitle.smk` plays `credits.smk` at (0, 0), returning
    /// to screen `0x1F`.
    Credits,
    /// `FUN_00432B05`'s hotspot 4, *"Lords of Magic?"* on the title page —
    /// Sierra's trailer for its next game. `Smk_Play("lom.smk", 0x46, 0x50, 0,
    /// g_screenId)`.
    LordsOfMagic,
    /// `CastleBuild_Confirm` — `Smk_Play(castle1.smk + level * 0x10, 0x9E,
    /// 0x14, 0, 0)`, the level 0-based.
    Castle(u8),
    /// `Msg_DrawWindow`'s animated capture branch. `take` is `DAT_00553ED4`
    /// after its increment, 0…2; `record` is the message it dismissed.
    Capture { take: u8, record: Record },
    /// `Msg_DrawWindow`'s animated ending branch. `file` is what
    /// [`ending_film`] chose; `game_over` is whether `Msg_Dismiss` entered
    /// screen `0x1C`, which is the screen `Smk_Play` was told to go back to.
    Ending { file: &'static str, record: Record, game_over: bool },
    /// `Battle_CheckOutcome`, over the outcome banner.
    Battle { file: &'static str },
}

/// `s_castle1_smk_004d5590`, five entries at 0x10 bytes.
pub const CASTLE_FILMS: [&str; 5] =
    ["castle1.smk", "castle2.smk", "castle3.smk", "castle4.smk", "castle5.smk"];

/// `s_cap_cty1_smk_004d6e88`, three entries.
pub const CAPTURE_FILMS: [&str; 3] = ["cap_cty1.smk", "cap_cty2.smk", "cap_cty3.smk"];

/// `s_cart_kgt_smk_004d6cf8`: five rows of five, **indexed by the fallen
/// lord** — 0 unused, then the Knight, the Baron, the Countess and the Bishop
/// — and across by how long the game has run. A lord with no cart film of his
/// own is given his pillory twice (the Countess) or his cart twice (the
/// Bishop), which is exactly the five names `docs/formats/smk.md` found
/// referenced and missing: `cart_cts`, `pill_bsp`, and the cut `_hmn` pair.
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
    /// The name `Lords2.exe` asks for. Resolved case-insensitively, because the
    /// install spells them `Cap_cty1.smk` and the executable `cap_cty1.smk`.
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

    /// `Smk_Play`'s second and third arguments: where `SmackToBuffer` puts the
    /// film's top-left pixel in the 640 × 480 screen.
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

    /// **Whether the screen behind the film is cleared first.**
    ///
    /// The front-end films are preceded by `FUN_004B11CE` —
    /// `Palette_Set`, `FUN_004B1867` (which hands the whole back buffer to
    /// `FUN_004B3E51` — a clear, `[I]` from the length) and a repaint — so they
    /// play on black. The other four play over the screen that raised them,
    /// which is still in the back buffer because screen `0x22` has no painter:
    /// the castle chooser's preview well, the dimmed map with the message
    /// window drawn once, the battlefield under its banner.
    pub fn is_over_a_screen(&self) -> bool {
        !matches!(self, Film::Intro | Film::ImpTitle | Film::Credits | Film::LordsOfMagic)
    }

    /// Played before `Game_NewGame` could have run. The question
    /// [`crate::audio`]'s exhaustive `before_the_campaign` asks of every screen.
    pub fn is_front_end(&self) -> bool {
        !self.is_over_a_screen()
    }

    /// **`Smk_OnFinished` (`0x0042E060`)** — what the end of this film, or a
    /// skip, does next.
    ///
    /// * During start-up (`g_appPhase == 1`) it `strcmp`s the path: `intro.smk`
    ///   plays `imptitle.smk`, `imptitle.smk` plays `credits.smk`, and anything
    ///   else lets the front end up. **So a skip moves one film along, not
    ///   out of the sequence**, which is what the original does to a player
    ///   who clicks through the intro.
    /// * Otherwise `g_screenId = g_smkReturnScreen`: the screen that raised the
    ///   film — or, for an ending, the conquest screen `Msg_Dismiss` had
    ///   already entered.
    pub fn then(&self) -> Transition {
        match self {
            Film::Intro => Transition::Replace(ScreenId::Movie(Film::ImpTitle)),
            Film::ImpTitle => Transition::Replace(ScreenId::Movie(Film::Credits)),
            Film::Ending { game_over: true, .. } => Transition::Replace(ScreenId::Conquest),
            _ => Transition::Pop,
        }
    }

    /// **When `Smk_Open` fails** — a file the install does not have, or one
    /// that will not parse. `Smk_Play` then sets `g_screenId` to the return
    /// screen directly and **`Smk_OnFinished` is never called**, so nothing
    /// chains: a missing `intro.smk` goes straight to the title page
    /// (`FUN_004B3571`'s `DAT_005C9274 = 1`) rather than on to the logo.
    pub fn on_failure(&self) -> Transition {
        match self {
            Film::Ending { game_over: true, .. } => Transition::Replace(ScreenId::Conquest),
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

    /// The screen this film belongs to, for a window title.
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

/// **The two film counters the original keeps in its data segment**, and one
/// latch of ours for the edge `Smk_OnFinished` writes.
///
/// Presentation, not world: neither counter is in a save block, and neither
/// changes anything but which of two or four pictures a player sees.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Reel {
    /// `DAT_00553ED4` — which `cap_cty` film is next. Stepped **before** it is
    /// used and wrapped past 2, so the first capture of a session is
    /// `cap_cty2.smk`. Cleared by `FUN_004975D3` at start-up and nowhere else.
    pub capture: u8,
    /// `DAT_0053F084` — which of a battle row's four is next. Used **then**
    /// stepped, wrapped past 3. Cleared at start-up and by the new-game
    /// initialiser.
    pub battle: u8,
    /// Set when a film **ends or is skipped**, not when it fails to open —
    /// which is the difference between `Smk_OnFinished` running and not. The
    /// battlefield reads it for `if (g_screenId == 0x2B) DAT_00568470 = 5001`.
    pub finished: Option<Film>,
}

impl Reel {
    /// `DAT_00553ED4 = DAT_00553ED4 + 1; if (2 < DAT_00553ED4) DAT_00553ED4 = 0;`
    pub fn next_capture(&mut self) -> u8 {
        self.capture += 1;
        if self.capture > 2 {
            self.capture = 0;
        }
        self.capture
    }

    /// The index, then `DAT_0053F084 = DAT_0053F084 + 1; if (3 < …) … = 0;`
    pub fn next_battle(&mut self) -> u8 {
        let take = self.battle.min(3);
        self.battle += 1;
        if self.battle > 3 {
            self.battle = 0;
        }
        take
    }

    /// Take the finished-film latch.
    pub fn take_finished(&mut self) -> Option<Film> {
        self.finished.take()
    }
}

/// The install's `.smk` files, by lower-cased name.
///
/// An index rather than bytes: the films are 80 MB and a game plays a few of
/// them, so a film is read when it is asked for. Built through the same
/// [`l2_mods::vfs::Vfs`] as every other asset, which is what makes
/// `axmen.smk` find `AXMEN.SMK` and not `Axemen.smk` — two different files,
/// `docs/formats/smk.md`.
#[derive(Debug, Clone, Default)]
pub struct FilmFiles {
    paths: BTreeMap<String, PathBuf>,
}

impl FilmFiles {
    pub fn index(vfs: &l2_mods::vfs::Vfs) -> FilmFiles {
        let mut paths = BTreeMap::new();
        for name in vfs.entries_with_extension("smk") {
            if let Some(p) = vfs.resolve(name) {
                paths.insert(name.to_ascii_lowercase(), p.to_path_buf());
            }
        }
        FilmFiles { paths }
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Every film the install has, lower-cased, sorted.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.paths.keys().map(String::as_str)
    }

    pub fn path(&self, name: &str) -> Option<&Path> {
        self.paths.get(&name.to_ascii_lowercase()).map(|p| p.as_path())
    }

    /// **`Smk_Open`**, without the CD. `None` for a film the install does not
    /// have or that does not parse, which is `SmackOpen` returning null.
    pub fn open(&self, name: &str) -> Option<Smk> {
        let bytes = std::fs::read(self.path(name)?).ok()?;
        match Smk::parse(bytes) {
            Ok(s) => Some(s),
            Err(e) => {
                eprintln!("film {name}: {e}");
                None
            }
        }
    }
}

/// What one tick of a film did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Playing,
    /// **The last frame came due.** `Smk_PlayLoop` decodes it and does not draw
    /// it — `if (currentFrame < frames - 1) SmackToBuffer(…)` guards the blit
    /// and the `else` closes the film — so the last picture a player sees is
    /// the second to last.
    Finished,
}

/// **`Smk_PlayLoop` (`0x0042DBC7`)** on our clock: the film, its decoder, and
/// how long it has been up.
///
/// `SmackWait` paces the original against the wall clock; this paces against
/// ticks of [`crate::TICK_MS`], the application's only clock, which is the
/// same clock at a coarser grain. A frame that falls due while the machine was
/// busy is decoded and not shown, which is what `SmackWait` returning late does
/// too.
pub struct Player {
    smk: Smk,
    decoder: Decoder,
    ticks: u32,
    /// The subtitle cue state; see [`Subtitles`].
    pub subtitles: Subtitles,
}

impl Player {
    /// `Smk_Open`: the file is opened and **frame 0 is decoded at once**.
    pub fn open(smk: Smk, cued: bool) -> Result<Player, l2_smk::Error> {
        let mut decoder = smk.decoder();
        let mut subtitles = Subtitles::new(cued);
        decoder.next_frame(&smk)?;
        subtitles.cue(0);
        Ok(Player { smk, decoder, ticks: 0, subtitles })
    }

    pub fn smk(&self) -> &Smk {
        &self.smk
    }

    pub fn decoder(&self) -> &Decoder {
        &self.decoder
    }

    /// The frame on screen.
    pub fn frame(&self) -> usize {
        self.decoder.frame().unwrap_or(0)
    }

    /// One tick of [`crate::TICK_MS`].
    pub fn tick(&mut self) -> Result<Step, l2_smk::Error> {
        self.ticks += 1;
        let frames = self.smk.frames();
        if frames <= 1 {
            return Ok(Step::Finished);
        }
        // Tens of microseconds, the unit the header's interval is written in.
        let elapsed = self.ticks as u64 * crate::TICK_MS as u64 * 100;
        let due = (elapsed / self.smk.header().period_10us().max(1) as u64) as usize;
        while self.frame() < due.min(frames - 1) {
            self.decoder.next_frame(&self.smk)?;
            let f = self.frame();
            self.subtitles.cue(f);
        }
        Ok(if due >= frames - 1 { Step::Finished } else { Step::Playing })
    }
}

/// **`FUN_0041A166(frame)`** — the intro's frame cues, and the one piece of
/// text any film carries.
///
/// `Smk_PlayLoop` calls it on every frame of `intro.smk` and of no other film.
/// Its first act is `Eng_CopyString(300, 0)` against the literal `"English"`,
/// seven characters — and `L2.eng` group 300 index 0 is *"English - DO NOT
/// TRANSLATE THIS!!!!"* — so **on an English install every cue is skipped** and
/// the narration is the soundtrack alone. A translated `L2.eng` gets group
/// 301's eleven lines, *"1268 AD"* onward, drawn centred across the bottom of
/// the screen at `y = 400` in the body font, colour `0xF5`, each cleared by a
/// 16- or 32-row fill of colour 0 at `y = 398` a few seconds later.
///
/// The cues are equality tests on the frame number, so this is fed every
/// frame the decoder produces — including ones decoded to catch up and never
/// shown, which the original's loop would have cued as well.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Subtitles {
    enabled: bool,
    /// The group 301 indices on screen, at y 400 and y 416.
    pub lines: [Option<usize>; 2],
}

/// `(frame, what)` — `Some((first, second))` draws, `None` clears.
pub const CUES: [(usize, Option<(usize, Option<usize>)>); 20] = [
    (0x1D, Some((0, None))),
    (0x32, None),
    (0x41, Some((1, None))),
    (0x5C, None),
    (0xE6, Some((2, None))),
    (0x105, None),
    (0x106, Some((3, None))),
    (0x116, None),
    (0x117, Some((4, Some(5)))),
    (0x16C, None),
    (0x16D, Some((6, None))),
    (0x19C, None),
    (0x19D, Some((7, Some(8)))),
    (500, None),
    (0x444, Some((9, None))),
    (0x460, None),
    (0x4C5, Some((10, None))),
    (0x4EE, None),
    // Two slots of padding keep the table's length a round number; they can
    // never match a frame a film has.
    (usize::MAX, None),
    (usize::MAX, None),
];

/// `L2.eng`'s language tag, and the seven characters `FUN_0041A166` compares.
pub fn is_english(language_tag: &str) -> bool {
    language_tag.as_bytes().get(..7) == Some(b"English")
}

impl Subtitles {
    pub fn new(enabled: bool) -> Subtitles {
        Subtitles { enabled, lines: [None, None] }
    }

    pub fn cue(&mut self, frame: usize) {
        if !self.enabled {
            return;
        }
        if let Some(&(_, what)) = CUES.iter().find(|(f, _)| *f == frame) {
            self.lines = match what {
                Some((a, b)) => [Some(a), b],
                None => [None, None],
            };
        }
    }
}
