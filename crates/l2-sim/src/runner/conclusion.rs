use super::*;

/// The same 3 as `crate::siege`'s campaign-side gate, from a different
/// function: `Siege_LaunchAssault` refuses to *start* an engineless assault at
/// level 3, and `Battle_CheckOutcome` refuses to *continue* one. Two
/// independent statements of the same rule, which is what makes it `[V]`.
pub const ASSAULT_REPEATS_BELOW_LEVEL: u8 = 3;

pub const ASSAULT_REPEAT_SCORE: i32 = 4;

pub fn other_side(side: Side) -> Side {
    if side == SIDE_A {
        SIDE_B
    } else {
        SIDE_A
    }
}

/// The siege handlers then find nothing to move to and issue no order, which is
/// what the original does with an unbuilt castle. A siege fills those tables
/// instead from [`crate::castle::ai_field`], out of `stnfield.pl8`'s structure
/// layer — `docs/decisions.md` C203.
pub fn ai_field_for(field: &Battlefield) -> AiField {
    let mut f = AiField::field(
        (field.home_side0.0 as i16, field.home_side0.1 as i16),
        (field.home_side4.0 as i16, field.home_side4.1 as i16),
    );
    f.surface = field.cells.iter().map(|c| c.surface).collect();
    f.elevation = field.cells.iter().map(|c| c.elevation).collect();
    f
}

/// The order `Battle_RaiseSide` walks the eleven troop types in
/// (`g_raiseOrder`, `0x004D9870`): ram, oil, knight, sword, mace, pike,
/// crossbow, archer, peasant, tower, catapult. **[V]** — read out of the binary
/// as `9, 10, 6, 3, 2, 4, 1, 5, 0, 8, 7`. It matters because the tail is what
/// gets truncated when an army would overflow the 80-figure array.
pub const RAISE_ORDER: [Troop; 11] = [
    Troop::BatteringRams,
    Troop::Oil,
    Troop::Knights,
    Troop::Swordsmen,
    Troop::Macemen,
    Troop::Pikemen,
    Troop::Crossbowmen,
    Troop::Archers,
    Troop::Peasants,
    Troop::SiegeTowers,
    Troop::Catapults,
];

pub fn army_from_counts(counts: &[u32; 11], men_per_figure: u32) -> Vec<(Troop, u16)> {
    let per = men_per_figure.max(1);
    RAISE_ORDER
        .iter()
        .filter_map(|t| {
            let men = counts[t.index()];
            if men == 0 {
                return None;
            }
            Some((*t, men.div_ceil(per).min(crate::MAX_FIGURES as u32) as u16))
        })
        .collect()
}

pub fn blank_field() -> Battlefield {
    let mut layer = vec![0u8; terrain::CELLS];
    layer[20 * DIM + 40] = 0x04;
    layer[60 * DIM + 40] = 0x0F;
    terrain::build(&layer, 1)
}

