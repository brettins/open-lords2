//! The seam, against a real save.
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-scenario
//! ```
//!
//! Two halves, and they are gated differently, which is the point of the split:
//!
//! * **The import *is* the file** — every value the kingdom ends up holding can
//!   be pointed at a byte. Where the numbers are the England turn-one position's
//!   own, the test is gated on that named fixture (`l2_testkit::england_turn1`)
//!   and fails loudly if handed a different game.
//! * **A save this code has misread is refused.** Each
//!   refusal below corrupts one byte of a real save and checks which error comes
//!   back, which is the only way to know the guards are reachable at all — and
//!   that is a property of the importer, not of any scenario, so it runs over
//!   **every** save the machine can offer.
//!
//! Nothing here reads a save out of the game install by a hard-coded path. That
//! is what broke: the install's `lastturn.sav` is the rolling autosave and three
//! directories on this machine hold a different game under that name.

mod county;
pub use county::*;
mod economy;
pub use economy::*;
mod units;
pub use units::*;

use l2_formats::save::{Layout, Save, COUNTY_BASE, COUNTY_STRIDE};
use l2_scenario::{ImportError, Scenario, STARTING_HEALTH_METER};
use l2_testkit::{saves, SaveFile};

/// Overwrite one byte of a *copy* of a save, at a runtime address, using the
/// layout the executable itself supplies. Nothing is written to any install.
fn poke(exe: &[u8], sav: &[u8], va: u32, value: u8) -> Vec<u8> {
    let layout = Layout::from_executable(exe).expect("block table");
    let at = layout.offset_of(va).expect("a saved address");
    let mut out = sav.to_vec();
    out[at] = value;
    out
}

/// Corrupt one byte of every reachable save and check the same error comes back
/// from all of them. A guard that is only reachable on one file is not a guard
/// anybody can rely on.
fn refusal_over_every_save(
    va: impl Fn(&SaveFile) -> u32,
    value: u8,
    expected: impl Fn(&SaveFile) -> ImportError,
) {
    let exe = l2_testkit::executable!();
    let saves = saves!();
    for s in &saves {
        let bytes = std::fs::read(&s.path).expect("re-read");
        let poked = poke(&exe, &bytes, va(s), value);
        let save = Save::open(&exe, &poked).expect("still the right length");
        assert_eq!(Scenario::from_save(&save), Err(expected(s)), "{}", s.label());
    }
    eprintln!("refusal reached on {} saves", saves.len());
}

