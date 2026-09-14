//! The canonical encoding: fixed widths, self-delimiting fields, and
//! one byte stream shared by the checksum, the snapshot and the dump.

mod encoding_tests;
pub use encoding_tests::*;
mod decoding_tests;
pub use decoding_tests::*;

use l2_net::{decode_all, Canonical, CodecError, Fixed, Pcg32, Reader, CHECKSUM_SEED};

// --- the stream is what it says it is ---------------------------------

