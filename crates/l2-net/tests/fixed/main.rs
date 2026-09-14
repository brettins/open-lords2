//! Q16.16 arithmetic, at the boundaries and around zero.
//!
//! Two things are being defended here.
//!
//! **Rounding.** `docs/netcode.md` D-2 requires every operation to be
//! explicit about it, and the reason is not pedantry: a rounding rule
//! that is asymmetric about zero is a directional bias in a
//! simulation, and a directional bias is invisible until someone
//! notices that units drift left over a long battle. The property
//! tests below check symmetry as a property
//! points.
//!
//! **Overflow.** `Fixed`'s range is deliberately much wider than the
//! game needs, so every overflow is a bug — but a bug that must
//! *behave identically on both peers*, in both debug and release
//! builds.
//! using Rust's built-in behaviour, which panics in debug and wraps in
//! release. Two peers on different profiles would otherwise compute
//! different numbers, and no test that runs one profile at a time would
//! ever see it.

mod rounding_and_conversions;
pub use rounding_and_conversions::*;
mod arithmetic_and_overflow;
pub use arithmetic_and_overflow::*;
mod math_functions;
pub use math_functions::*;

use l2_net::Fixed;

fn f(n: i32) -> Fixed {
    Fixed::from_int(n)
}

