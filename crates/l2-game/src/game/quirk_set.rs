#![allow(unused_imports)]
use super::*;
use super::settings::*;
use super::assets::*;
use super::unit_frames::*;
use super::levy::*;
use l2_kingdom::county::MAX_COUNTIES;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::{Kingdom, SeasonReport};
use l2_kingdom::tables::MAX_TAX_RATE;
use l2_kingdom::county::MAX_RATION_SPLIT;

/// **Switches that turn one of the original's *presentation* defects off.**
///
/// Every field defaults to `false`, which is the original's behaviour, and
/// `docs/bugs.md` names the switch in the entry for the defect it undoes.
///
/// # Why this is not `Options`
///
/// `docs/bugs.md` §6.3 recommends `Options` as the home for a quirk set, and it
/// is right — for the quirks it is arguing about. Every one of them changes a
/// *rule*, and its three constraints follow from that: the value must reach the
/// simulation, must be agreed in the lobby handshake, and must be stamped into
/// replays, at the cost of a `save::VERSION` bump.
///
/// **None of that applies to the colour of a text shadow.** A quirk here cannot
/// reach `l2-kingdom` or `l2-sim`, is not in the save body, is not in the
/// lockstep state, and two players running with different values here compute
/// identical turns — `docs/netcode.md` D-12 says display state must *not* reach
/// the simulation, so putting a shadow colour in the hashed options would be
/// the wrong answer. It lives on [`Assets`],
/// whose whole definition is *"everything the screens draw with, not part of
/// the world."*
///
/// A behavioural quirk still belongs on [`l2_kingdom::kingdom::Options`] and
/// still costs the bump. `docs/bugs.md` §6.3a has the one-line test that tells
/// them apart — *if flipping it can change a number in a saved game it is
/// behavioural; if it can only change which pixels are painted from the same
/// numbers it is presentation* — and `docs/decisions.md` C62 the argument for
/// the group switch that spans both.
///
/// # The table is how the group switch reaches this half
///
/// [`PRESENTATION`] names every field and the `docs/bugs.md` entry it undoes,
/// and [`Quirks::get`] / [`Quirks::set`] go through it. Rust cannot enumerate a
/// struct's fields, and the quirks page must be able to walk *both* sets to show
/// one tri-state parent — a parent that spoke for only half of them would be a
/// parent a player could not trust. `crates/l2-testkit/tests/quirks_catalogue/main.rs`
/// asserts the table names every field and no others.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Quirks {
    /// Draw a county's **name** on the cloudy plate with the grey emboss the
    /// *Sovereign land* lines beneath it use, instead of the parchment emboss
    /// the original uses everywhere.
    ///
    /// Off by default, and that is the whole design: **a set bit means *fixed*,
    /// so faithful is zero.** `docs/bugs.md` B64 has the defect and
    /// [`crate::shell::font::SHADOW_GREY`] the two palette indices.
    ///
    /// This field and the table around it were built on two branches that
/// met, so the comment standing in this space said *"no
    /// presentation quirk has landed yet"* and named this one as the first.
    pub grey_county_name: bool,
}

/// `(field, docs/bugs.md entry)` for every field of [`Quirks`], in declaration
/// order.
///
/// **The join between this half of the switch list and the catalogue.** It is
/// prose that a machine reads, which is the only kind of prose that cannot go
/// stale: `quirks_catalogue.rs` fails if a field is missing from it, if a row
/// names a field that does not exist, or if a row cites an entry `docs/bugs.md`
/// does not have.
pub const PRESENTATION: &[(&str, &str)] = &[("grey_county_name", "B64")];

impl Quirks {
    /// Whether this defect is **fixed** — the field's own sense, and the
    /// opposite of `l2_net::Quirks::reproduces`, which is the sense the
    /// behavioural half is stored in.
    ///
    /// Named by string because the group switch walks [`PRESENTATION`]; every
    /// drawing site reads its own field directly and never comes through here.
    pub fn is_fixed(&self, field: &str) -> bool {
        // One arm per field, and then the fall-through. A field that reached
        // [`PRESENTATION`] without an arm here would read as *"the original's
        // behaviour"* for ever, silently, which is the one failure this method
// can have — so it is asserted.
        match field {
            "grey_county_name" => self.grey_county_name,
            other => {
                debug_assert!(
                    !PRESENTATION.iter().any(|(f, _)| *f == other),
                    "{other} is in PRESENTATION and has no arm in Quirks::is_fixed"
                );
                false
            }
        }
    }

/// Set one field. Unknown names are ignored: the
    /// caller is a click on a checkbox, and a page that could crash the game by
    /// naming a field that has been renamed is worse than one that does
    /// nothing.
    pub fn set_fixed(&mut self, field: &str, fixed: bool) {
        match field {
            "grey_county_name" => self.grey_county_name = fixed,
            other => debug_assert!(
                !PRESENTATION.iter().any(|(f, _)| *f == other),
                "{other} is in PRESENTATION and has no arm in Quirks::set_fixed"
            ),
        }
    }

    /// How many presentation defects are reproduced, of how many there are.
    pub fn tally(&self) -> (usize, usize) {
        let fixed = PRESENTATION.iter().filter(|(f, _)| self.is_fixed(f)).count();
        (PRESENTATION.len() - fixed, PRESENTATION.len())
    }
}

