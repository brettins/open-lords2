
mod births;
pub use births::*;
mod migration;
pub use migration::*;

use super::*;

#[allow(dead_code)]
const Q: Quirks = Quirks::FAITHFUL;

const T: &Tables = &Tables::DEFAULT;

