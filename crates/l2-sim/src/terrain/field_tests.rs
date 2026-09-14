//! `Battlefield_BuildRandom` (`0x0047AAA3`) on a synthesised raster. No asset
//! is committed: every test writes the source bytes it wants.

use super::*;

/// An all-open field with one marker pair per side, the way a shipped
/// `batfield.pl8` frame is laid out.
fn raster() -> Vec<u8> {
    let mut r = vec![0u8; RASTER_BYTES];
    // Side 0's marker at (10, 20): the pair, then `0x40 + 3` two cells east.
    r[20 * DIM + 10] = 0x04;
    r[21 * DIM + 10] = 0x04;
    r[20 * DIM + 12] = 0x43;
    // Side 4's at (10, 60), slot 0.
    r[60 * DIM + 10] = 0x0F;
    r[61 * DIM + 10] = 0x0F;
    r[60 * DIM + 12] = 0x40;
    r
}

#[test]
fn translation_table_is_battle_md_3_0() {
    let mut r = vec![0u8; RASTER_BYTES];
    let want: [(u8, u8); 12] = [
        (0x00, 1),
        (0x02, 4),
        (0x04, 0x14),
        (0x07, 0x28),
        (0x08, 0x29),
        (0x09, 0x0B),
        (0x0A, 0x0C),
        (0x0F, 0x1E),
        (0x10, 7),
        (0x12, 8),
        (0x14, 9),
        (0x15, 0x0D),
    ];
    // Row 40, spaced four apart so no auto-tiler or bridge stamp reaches a
    // neighbour under test.
    for (n, (src, _)) in want.iter().enumerate() {
        r[40 * DIM + 4 + n * 5] = *src;
    }
    // The two range arms and the pass-through default.
    r[10 * DIM + 10] = 0x2A;
    r[10 * DIM + 20] = 0x57;
    r[10 * DIM + 30] = 0x6B;
    let f = build_field(&r, 7);

    for (n, (src, id)) in want.iter().enumerate() {
        let c = f.at(4 + n * 5, 40);
        // The bridge parts erase their own id as they stamp; the rest stand.
        if !matches!(*src, 0x10 | 0x12 | 0x14) {
            assert_eq!(c.terrain, *id, "source {src:#04x}");
        }
    }
    let rocks = f.at(10, 10);
    assert_eq!((rocks.terrain, rocks.gfx), (id::ROCKS, 0x2A));
    assert_ne!(rocks.flags & flag::IMPASSABLE, 0);
    let six = f.at(20, 10);
    assert_eq!((six.terrain, six.gfx), (id::UNUSED6, 0x57 + 0x2C));
    assert_ne!(six.flags & flag::IMPASSABLE, 0);
    // Pass-through: `0x6B` is in no arm and lands in `terrain` unchanged.
    assert_eq!(f.at(30, 10).terrain, 0x6B);
}

/// **Ablation.** Drop the `0x50..=0x5F` arm — make it pass through like any
/// other unclaimed byte — and the frame, the flag and the id all go wrong.
#[test]
fn ablate_the_range_arm() {
    let mut r = vec![0u8; RASTER_BYTES];
    r[10 * DIM + 20] = 0x57;
    let f = build_field(&r, 7);
    let c = f.at(20, 10);
    assert_ne!(c.terrain, 0x57, "without the arm the id would be the source byte");
    assert_eq!(c.gfx, 0x83, "and the frame would be 0, not source + 0x2C");
}

#[test]
fn an_open_field_is_flat() {
    // `Battlefield_BuildRandom` writes cell byte +4 nowhere at all: the
    // elevation rules of docs/battle.md §6.2 and §7 are inert on a field.
    let f = build_field(&raster(), 3);
    assert!(f.cells.iter().all(|c| c.elevation == 0));
}

#[test]
fn markers_fill_the_slot_the_third_cell_names() {
    let f = build_field(&raster(), 3);
    // Slot 3 for side 0, slot 0 for side 4, each at the marker's (x + 2, y).
    assert_eq!(f.deploy_side0[3], (12, 20));
    assert_eq!(f.deploy_side4[0], (12, 60));
    assert_eq!(f.home_side0, (10, 20));
    assert_eq!(f.home_side4, (10, 60));
    // And the 6 x 2 block the marker occupied is open ground again.
    for y in 20..22 {
        for x in 10..16 {
            assert_eq!(f.at(x, y).terrain, id::OPEN, "({x}, {y})");
        }
    }
}

/// **Ablation.** A lone marker cell — no south twin — is not a marker, and
/// nothing is claimed for it. This is the `.skr` builder's rule, and applying
/// it here would put a slot wherever a stray `0x04` fell.
#[test]
fn a_lone_marker_cell_claims_no_slot() {
    let mut r = vec![0u8; RASTER_BYTES];
    r[20 * DIM + 10] = 0x04;
    r[20 * DIM + 12] = 0x43;
    let f = build_field(&r, 3);
    assert_eq!(f.deploy_side0, [(0, 0); 12]);
    assert_eq!(f.at(10, 20).terrain, field_id::MARKER_SIDE0);
}

#[test]
fn woodland_and_water_take_a_frame_and_a_surface() {
    let mut r = vec![0u8; RASTER_BYTES];
    for y in 30..40 {
        for x in 30..40 {
            r[y * DIM + x] = 0x0A;
            r[y * DIM + x + 20] = 0x09;
        }
    }
    let f = build_field(&r, 11);
    let wood = f.at(35, 35);
    assert_eq!((wood.terrain, wood.surface), (id::WOODLAND, 0x0F));
    assert_ne!(wood.gfx, 0);
    let water = f.at(55, 35);
    assert_eq!(water.terrain, id::WATER);
    assert_ne!(water.flags & flag::IMPASSABLE, 0);
    assert_ne!(water.gfx, 0);
}

#[test]
fn parse_reads_sixteen_byte_directory_records() {
    // Two frames, data after a 1000-byte directory.
    let mut bytes = vec![0u8; 1000 + 2 * RASTER_BYTES];
    for (n, off) in [1000usize, 1000 + RASTER_BYTES].iter().enumerate() {
        let at = n * 0x10 + 0x0C;
        bytes[at] = (off & 0xFF) as u8;
        bytes[at + 1] = ((off >> 8) & 0xFF) as u8;
        bytes[at + 2] = ((off >> 16) & 0xFF) as u8;
    }
    bytes[1000 + RASTER_BYTES] = 0x09;
    let sheets = FieldSheets::parse(&bytes).expect("two whole planes");
    assert_eq!(sheets.len(), 2);
    assert_eq!(sheets.get(1).unwrap()[0], 0x09);
    // The pick is ours and it is a pure function of the seed.
    assert_eq!(sheets.pick(3)[0], sheets.pick(5)[0]);
}
