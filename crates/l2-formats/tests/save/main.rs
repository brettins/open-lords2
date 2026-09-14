//! **Invariants of the save format**, over every save this machine can reach.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" LORDS2_FIXTURES="E:\dev\lords2-fixtures" \
//!     cargo test -p l2-formats --test save
//! ```
//!
//! # Why this file was split
//!
//! It used to hold two different kinds of assertion under one roof, and the
//! difference only became visible when it broke. Half of it read properties that
//! must hold of *any* valid save — the block table accounting for the file
//! exactly, adjacency being symmetric, an owner byte naming a realm that exists.
//! The other half asserted the numbers of one particular saved game: five owned
//! counties, happiness 72 and 77, population 417 everywhere.
//!
//! Both halves pointed at `lastturn.sav` inside the game install, which is the
//! **rolling autosave** — the game rewrites it every turn a human plays. Ten
//! minutes of play replaced it, and nine tests went red at once with bare
//! assertion diffs that read like a broken reader. One of them was
//! `the_neighbour_lists_are_symmetric_and_name_only_real_counties`, whose name
//! promises an invariant; it failed on `assert_eq!(real.len(), 14)`, which is
//! not one. The invariant it is named after held perfectly.
//!
//! So: **scenario values live in `save_england_turn1.rs`**, behind a named
//! fixture with a fingerprint. This file asserts only what is true of every
//! save, and runs over every save it can find — the preserved fixtures in
//! `LORDS2_FIXTURES` *and* whatever volatile saves the install happens to hold.
//! One file agreeing proves nothing about a format (`docs/decisions.md` C1);
//! these run over as many as exist.

mod structure;
pub use structure::*;
mod realms_and_counties;
pub use realms_and_counties::*;
mod units_and_merchants;
pub use units_and_merchants::*;

use l2_formats::save::{Save, SaveError, COUNTY_RECORDS, NEIGHBOUR_SLOTS, REALM_RECORDS};
use l2_testkit::{executable, saves, skip, SaveFile};

