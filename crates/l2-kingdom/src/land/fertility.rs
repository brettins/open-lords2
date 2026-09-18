use super::*;

/// `Fertility_Update` (`0x0044BFD5`).
pub fn update_fertility(county: &mut County, advanced_farming: bool) {
    county.fertility +=
        FERTILITY_PER_FALLOW * county.fields_fallow - FERTILITY_PER_GRAIN * county.fields_grain;
    county.fertility = clamp(county.fertility, FERTILITY_MIN, FERTILITY_MAX);
    if !advanced_farming {
        county.fertility = 0;
    }
}

/// `FUN_0044D281` — **scale a standing crop by the grain fields still
/// standing.**
pub fn field_share(county: &County, crop: i32) -> i32 {
    if county.fields_grain < county.fields_grain_sown {
        pct(crop, crate::math::pct_of(county.fields_grain, county.fields_grain_sown))
    } else {
        crop
    }
}

/// `FUN_0044D303` — **fertility, at half strength, once per growing season.**
pub fn fertility_bonus(county: &County, crop: i32) -> i32 {
    crop + pct(crop, county.fertility / 2)
}

