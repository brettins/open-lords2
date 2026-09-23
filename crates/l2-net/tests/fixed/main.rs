
mod rounding_and_conversions;
mod arithmetic_and_overflow;
mod math_functions;

use l2_net::Fixed;

fn f(n: i32) -> Fixed {
    Fixed::from_int(n)
}

