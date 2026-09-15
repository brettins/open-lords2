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

