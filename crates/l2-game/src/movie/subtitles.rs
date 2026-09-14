#![allow(unused_imports)]
use super::*;
use super::player::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use l2_smk::{Decoder, Smk};
use crate::message::Record;
use crate::screen::{Machine, ScreenId, Transition};

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

