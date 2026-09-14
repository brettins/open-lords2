#![allow(unused_imports)]
use super::*;
use super::subtitles::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use l2_smk::{Decoder, Smk};
use crate::message::Record;
use crate::screen::{Machine, ScreenId, Transition};

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
/// An index: the films are 80 MB and a game plays a few of
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
/// `Smk_PlayLoop` advances only when `SmackWait` (called at `0x0042DBF7`)
/// answers 0, and `_SmackWait@4` is a `timeGetTime` comparison — `[V]`, it is
/// the only import that function calls. So the original's film clock is **real
/// milliseconds**, which is also what plays out its sound track; measured over
/// the install's 45 films the track runs `frames × period` long to within 1 ms
/// over 131 s, so the header's rate and the audio buffer are one clock.
///
/// This paces against ticks of [`crate::TICK_MS`] instead, which is the same
/// clock at a coarser grain **only because [`crate::clock::Ticker`] makes a
/// tick a true 16 ms**. It did not, and a film drifted two percent behind its
/// own sound — `docs/decisions.md` C193. A frame that falls due while
/// the machine was busy is decoded and not shown, which is what `SmackWait`
/// returning late does too.
///
/// What remains, measured and left: a frame is shown at the first tick
/// *at or after* it is due, as `Smk_PlayLoop` shows it at the first poll at or
/// after — but our poll is 16 ms and the original's is microseconds, so ours
/// is a mean 8 ms late; and the film's audio starts on the tick its screen
/// reaches the stack while frame 0 is decoded on the next, one tick later.
/// Both are constant, both under a quarter of a frame, and neither compounds.
pub struct Player {
    smk: Smk,
    decoder: Decoder,
    pub(super) ticks: u32,
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

