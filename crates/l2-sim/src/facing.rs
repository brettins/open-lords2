
pub const FACINGS: usize = 8;

pub const FACING_DELTA: [(i32, i32); 8] =
    [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

pub fn facing_from_delta(dx: i32, dy: i32) -> Option<u8> {
    if dx == 0 && dy == 0 {
        return None;
    }
    let sx = dx.signum();
    let sy = dy.signum();
    let (ux, uy) = if dx.abs() > 2 * dy.abs() {
        (sx, 0)
    } else if dy.abs() > 2 * dx.abs() {
        (0, sy)
    } else {
        (sx, sy)
    };
    FACING_DELTA.iter().position(|&d| d == (ux, uy)).map(|i| i as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facings_and_deltas_are_inverse() {
        for (i, (dx, dy)) in FACING_DELTA.iter().enumerate() {
            assert_eq!(facing_from_delta(*dx, *dy), Some(i as u8), "facing {i}");
        }
        assert_eq!(facing_from_delta(0, 0), None);
        assert_eq!(facing_from_delta(10, 1), Some(2), "mostly east");
        assert_eq!(facing_from_delta(10, 9), Some(3), "south-east");
        assert_eq!(facing_from_delta(-1, -10), Some(0), "mostly north");
    }
}
