use super::*;

#[allow(clippy::too_many_arguments)]
pub fn lay_out(
    t: &Tables,
    style: FarmStyle,
    counties: &mut [County],
    county_count: usize,
    id: usize,
    map: &mut CampaignMap,
    realms: &[Realm],
    market: &mut dyn Market,
    env: &FarmEnv,
) {
    // `Ai_TradeForCounty` (`0x0049E39B`) is the **first statement of each of
    // the three realm styles**, before the food cascade it pays for.
    if style.sells_first() {
        market.trade_for_county(id, &mut counties[id], map);
    }
    counties[id].purse += neutral_purse_top_up(style, &counties[id]);
    run_buys(style, id, &mut counties[id], map, market);

    let total = counties[id].field_total();

    counties[id].industry_share = style.industry_share();
    default_shares_built(&mut counties[id]);
    labour::allocate(&mut counties[id]);

    let search = split_search(style, &counties[id]);
    set_rations(
        t,
        &mut counties[id],
        search,
        env.armies_eat,
        crate::ration::Sowing::new(env.season, env.advanced_farming),
    );

    let winter = env.season == Season::Winter;
    match style {
        FarmStyle::NeutralArable | FarmStyle::RealmArable => {
            let county = &mut counties[id];
            field::clear_type(county, map, FieldType::Pasture);
            if winter {
                field::clear_type(county, map, FieldType::Grain);
                let quota = if env.advanced_farming {
                    winter_grain_quota(county.fertility, total / 2)
                } else if style == FarmStyle::NeutralArable {
                    total - 3
                } else {
                    total - 1
                };
                field::set_count(county, map, FieldType::Grain, quota);
            }
            if county.herd > 10 {
                field::set_count(county, map, FieldType::Pasture, 1);
                field::recount(county, map);
                field::herd_update_crowding(t, county, map);
            }
            refresh(t, counties, county_count, id, map, realms, env);
            labour::allocate(&mut counties[id]);
            refresh(t, counties, county_count, id, map, realms, env);
        }
        FarmStyle::NeutralGrazing | FarmStyle::RealmGrazing => {
            field::clear_type(&counties[id], map, FieldType::Grain);
            fit_cattle_fields(t, &mut counties[id], map, total - 1);
            refresh(t, counties, county_count, id, map, realms, env);
            labour::allocate(&mut counties[id]);
            refresh(t, counties, county_count, id, map, realms, env);
        }
        FarmStyle::RealmMixed => {
            if winter {
                let county = &mut counties[id];
                field::clear_type(county, map, FieldType::Grain);
                let quota = if env.advanced_farming {
                    winter_grain_quota(county.fertility, total / 3)
                } else {
                    total / 2
                };
                field::set_count(county, map, FieldType::Grain, quota);
            }
            fit_cattle_fields(t, &mut counties[id], map, total / 3);
            refresh(t, counties, county_count, id, map, realms, env);
            labour::allocate(&mut counties[id]);
        }
    }
}

fn refresh(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    id: usize,
    map: &CampaignMap,
    realms: &[Realm],
    env: &FarmEnv,
) {
    let owner = counties[id].owner;
    let share = crate::industry::weapon_shares(t, counties, county_count, owner);
    let neutral = Realm::new();
    let realm = realms.get(owner as usize).unwrap_or(&neutral);
    field::refresh_estimates(
        &mut counties[id],
        map,
        env.season_next,
        t,
        env.advanced_farming,
        realm,
        share,
    );
}


pub fn order_fields(county: &County, map: &mut CampaignMap) -> i32 {
    let total = county.field_total();
    for &(fields_below, population_above, n) in crate::tables::AI_FIELD_LADDER.iter() {
        if total < fields_below && county.population > population_above {
            return field::order_reclamation(county, map, n);
        }
    }
    0
}

/// `Ai_ManageCountyFarms` (`0x0049DD01`) — **AI turn step 5**, and also the
/// first thing `Season_Advance` does, through `Ai_ManageFarmsAll`
/// (`0x0049A990`), for every realm in play.
///
/// That second caller matters more than the first: it runs **ahead of the whole
/// economy**, so an AI's fields and labour split are already this season's when
/// tax, rations and industry read them. The human's are not. Both are
/// reproduced — [`crate::Kingdom::run_ai_farms_at_the_stall`] and
/// [`crate::Kingdom::ai_manage_farms_all`] — and both pass [`CountyStall`], so
/// the realm styles' `Ai_BuyGood` lines pay out of the treasury. A third caller,
/// `FUN_0049DF48` at the tail of `Battle_ReturnToCampaign`, is **not**
/// reproduced; see `ai_manage_farms_all`.
///
/// **What each realm style runs first**: `Ai_TradeForCounty` (`0x0049E39B`),
/// the surplus sale and weapon purchase — [`Market::trade_for_county`], which
/// [`lay_out`] calls for the three styles [`FarmStyle::sells_first`] names.
#[allow(clippy::too_many_arguments)]
pub fn manage_county_farms(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    map: &mut CampaignMap,
    realms: &[Realm],
    realm: u8,
    lord: u8,
    market: &mut dyn Market,
    env: &FarmEnv,
) -> i32 {
    let Some(style_byte) = t.ai_farm_style(lord) else { return 0 };
    let mut ordered = 0;
    for id in 1..=county_count.min(counties.len().saturating_sub(1)) {
        if counties[id].owner != realm {
            continue;
        }
        ordered += order_fields(&counties[id], map);
        counties[id].farm_style = style_byte;
        if let Some(style) = FarmStyle::for_realm(style_byte) {
            lay_out(t, style, counties, county_count, id, map, realms, market, env);
        }
        labour::allocate(&mut counties[id]);
    }
    ordered
}

/// `AI_ManageFields(0)` (`0x0049DFC6`) — **turn phase 1, step 2**: the same
/// treatment for every county nobody owns.
///
/// it zeroes county `+0x1B0` on the way in, and it never *writes* the style
/// byte. So an unowned county farms itself according to whichever lord held it
/// last, and one that has always been free farms as style 0.
///
/// `+0x1B0` is not modelled in this crate — `docs/kingdom.md` has it as an
/// untraced flag that AI step 12 sets back to 1 — so the zeroing is recorded
/// here and not performed.
#[allow(clippy::too_many_arguments)]
pub fn manage_neutral_fields(
    t: &Tables,
    counties: &mut [County],
    county_count: usize,
    map: &mut CampaignMap,
    realms: &[Realm],
    market: &mut dyn Market,
    env: &FarmEnv,
) -> i32 {
    let mut ordered = 0;
    for id in 1..=county_count.min(counties.len().saturating_sub(1)) {
        if counties[id].owner != 0 {
            continue;
        }
        ordered += order_fields(&counties[id], map);
        if let Some(style) = FarmStyle::for_neutral(counties[id].farm_style) {
            lay_out(t, style, counties, county_count, id, map, realms, market, env);
        }
        labour::allocate(&mut counties[id]);
    }
    ordered
}

