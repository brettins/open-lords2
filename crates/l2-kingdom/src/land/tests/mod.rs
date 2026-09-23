
mod fertility_tests;
mod grain_tests;
mod herd_tests;
pub(crate) use herd_tests::*;
mod crowding;

use super::*;

#[allow(dead_code)]
const Q: Quirks = Quirks::FAITHFUL;

const T: &Tables = &Tables::DEFAULT;

