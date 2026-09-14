use super::*;

/// `Fertility_Update` (`0x0044BFD5`).
///
/// ```text
/// county.fertility += 6 * county.fieldsFallow - 3 * county.fieldsGrain;
/// clamp -100 .. 100;
/// if (!g_optAdvancedFarming) county.fertility = 0;
/// ```
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
///
/// Runs at the top of both `Grain_Grow` and `Grain_Harvest`. If the county now
/// has *fewer* grain fields than it sowed, the crop is cut to
/// `PctOf(fieldsGrain, fieldsGrainSown)` percent of itself; if it has as many
/// or more, nothing happens. So ploughing a wheat field under in midsummer
/// costs a share of the year's crop, and painting new grain in midsummer buys
/// nothing until the next sowing.
pub fn field_share(county: &County, crop: i32) -> i32 {
    if county.fields_grain < county.fields_grain_sown {
        pct(crop, crate::math::pct_of(county.fields_grain, county.fields_grain_sown))
    } else {
        crop
    }
}

/// `FUN_0044D303` — **fertility, at half strength, once per growing season.**
///
/// `crop + Pct(crop, fertility / 2)`, so the −100…100 scalar
/// [`update_fertility`] keeps is worth −50 % … +50 % *per grow step* and there
/// are two of them a year: a perfectly fertile county reaps 2.25 times what a
/// neutral one does and a ruined one a quarter. The original writes the
/// division as an `if` whose two arms are identical, which is a compiler
/// artefact of a signed divide, not a rule.
///
/// This is the whole of fertility's effect on the crop. It is applied **after**
/// the labour cap, so fertility multiplies what the farmhands could
/// tend.
pub fn fertility_bonus(county: &County, crop: i32) -> i32 {
    crop + pct(crop, county.fertility / 2)
}

