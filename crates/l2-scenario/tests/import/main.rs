
mod county;
pub use county::*;
mod economy;
pub use economy::*;
mod units;
pub use units::*;

use l2_formats::save::{Layout, Save, COUNTY_BASE, COUNTY_STRIDE};
use l2_scenario::{ImportError, Scenario, STARTING_HEALTH_METER};
use l2_testkit::{saves, SaveFile};

fn poke(exe: &[u8], sav: &[u8], va: u32, value: u8) -> Vec<u8> {
    let layout = Layout::from_executable(exe).expect("block table");
    let at = layout.offset_of(va).expect("a saved address");
    let mut out = sav.to_vec();
    out[at] = value;
    out
}

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

