//! from the lockstep checksum too: two peers can diverge on it and every
//! checksum they exchange reports agreement. That is the failure lockstep
//! exists to prevent, and it has now happened three times
//! (`docs/decisions.md` C30, C39).
//!
//! The property is **"every field reaches the bytes"**, and it is *not* the
//! same as "a field round-trips". A field that both `Encode` and `Decode`
//! ignore round-trips perfectly: the encoder drops it, the decoder leaves
//! `County::new`'s default, and if the source value also holds that default,
//! `PartialEq` is satisfied. All four C30 fields would have survived such a
//! test.
//!
//! The predecessor was `every_part_of_the_state_reaches_the_bytes`, a
//! hand-written enumeration of ~130 mutations. It is deleted. It could only
//! ever check the fields somebody listed, it missed the four C30 fields, it
//! missed six more that were found by matching it against the struct, and it
//! missed the twelve of C39 — and leaving it beside a derived check is how the
//! derived one rots.

mod furnished_state;
pub(crate) use furnished_state::*;
mod campaign;
pub(crate) use campaign::*;
mod round_trips;
mod coverage;
mod header;
mod census_part;


// --- the fixture: begin ---------------------------------------------------
// Everything between these two markers is what the census searches. Keep the
// markers, and keep helpers that furnish state inside them.
