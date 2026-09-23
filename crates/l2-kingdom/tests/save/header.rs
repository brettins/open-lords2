#![allow(unused_imports)]
use super::*;
use super::furnished_state::*;
use super::campaign::*;
use super::round_trips::*;
use super::coverage::*;
use super::census_part::*;
use l2_kingdom::county::{ChangeReason, MAX_COUNTIES};
use l2_kingdom::realm::{Pair, MAX_REALMS};
use l2_kingdom::save::{
    checksum, decode, encode, ruleset_fingerprint, LoadError, HEADER_LEN, MAGIC, VERSION,
};
use l2_kingdom::tables::{Tables, Weather, JOB_COUNT};
use l2_kingdom::{Kingdom, Options};

#[test]
fn the_header_is_a_magic_a_version_a_fingerprint_and_a_length() {
    let k = furnished(1);
    let bytes = encode(&k);
    assert_eq!(&bytes[..8], &MAGIC);
    assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), VERSION);
    assert_eq!(
        u64::from_le_bytes(bytes[12..20].try_into().unwrap()),
        ruleset_fingerprint(&Tables::DEFAULT)
    );
    let declared = u32::from_le_bytes(bytes[20..24].try_into().unwrap()) as usize;
    assert_eq!(declared, bytes.len() - HEADER_LEN - 8, "body, then an eight-byte checksum");
}

#[test]
fn an_unknown_version_is_refused_rather_than_read() {
    let mut bytes = encode(&furnished(1));
    bytes[8..12].copy_from_slice(&(VERSION + 1).to_le_bytes());
    assert_eq!(
        decode(&bytes, Tables::DEFAULT),
        Err(LoadError::UnsupportedVersion { found: VERSION + 1, supported: VERSION })
    );

    let mut zeroed = encode(&furnished(1));
    zeroed[8..12].copy_from_slice(&0u32.to_le_bytes());
    assert!(matches!(
        decode(&zeroed, Tables::DEFAULT),
        Err(LoadError::UnsupportedVersion { found: 0, .. })
    ));
}

#[test]
fn something_that_is_not_a_save_is_refused() {
    assert_eq!(decode(b"", Tables::DEFAULT), Err(LoadError::NotASave));
    assert_eq!(decode(b"hello", Tables::DEFAULT), Err(LoadError::NotASave));
    assert_eq!(decode(&vec![0u8; 64], Tables::DEFAULT), Err(LoadError::NotASave));
    let mut bytes = encode(&Kingdom::new(1));
    bytes[0] = b'X';
    assert_eq!(decode(&bytes, Tables::DEFAULT), Err(LoadError::NotASave));
}

#[test]
fn a_save_made_under_another_ruleset_is_refused() {
    let mut modded = Tables::DEFAULT;
    modded.grain.yield_per_sack = 24;
    let k = Kingdom::with_tables(1, modded);
    let bytes = encode(&k);

    match decode(&bytes, Tables::DEFAULT) {
        Err(LoadError::RulesetMismatch { save, supplied }) => {
            assert_eq!(save, ruleset_fingerprint(&modded));
            assert_eq!(supplied, ruleset_fingerprint(&Tables::DEFAULT));
            assert_ne!(save, supplied);
        }
        other => panic!("expected a ruleset mismatch, got {other:?}"),
    }
    assert_eq!(decode(&bytes, modded).unwrap(), k);
}

#[test]
fn every_sub_table_reaches_the_fingerprint() {
    let base = ruleset_fingerprint(&Tables::DEFAULT);
    let mutations: Vec<(&str, Box<dyn Fn(&mut Tables)>)> = vec![
        ("food", Box::new(|t: &mut Tables| t.food.dairy_per_head += 1)),
        ("grain", Box::new(|t: &mut Tables| t.grain.yield_per_sack += 1)),
        ("field", Box::new(|t: &mut Tables| t.field.reclaim_per_season += 1)),
        ("event", Box::new(|t: &mut Tables| t.event.first_year += 1)),
        ("season", Box::new(|t: &mut Tables| t.season[4].death_rate += 1)),
        ("season dryness", Box::new(|t: &mut Tables| t.season[1].dryness += 1)),
        ("ration", Box::new(|t: &mut Tables| t.ration[3].divisor += 1)),
        ("health delta", Box::new(|t: &mut Tables| t.ration[3].health_delta[2] += 1)),
        ("ration happiness", Box::new(|t: &mut Tables| t.ration_happiness_slope += 1)),
        ("ration offset", Box::new(|t: &mut Tables| t.ration_happiness_offset += 1)),
        ("health", Box::new(|t: &mut Tables| t.health[3].death_rate += 1)),
        ("health ladder", Box::new(|t: &mut Tables| t.health_band_ladder[2].0 += 1)),
        ("birth ladder", Box::new(|t: &mut Tables| t.population.birth_rate_ladder[19].1 += 1)),
        ("happiness factor", Box::new(|t: &mut Tables| t.population.happiness_factor_ladder[4].1 += 1)),
        ("tax happiness", Box::new(|t: &mut Tables| t.tax_happiness_other[50] += 1)),
        ("weather", Box::new(|t: &mut Tables| t.weather[5].herd_pct += 1)),
        ("herd staffing", Box::new(|t: &mut Tables| t.herd.labour_per_head += 1)),
        ("herd staffing cap", Box::new(|t: &mut Tables| t.herd.staffing_max += 1)),
        ("herd understaffing", Box::new(|t: &mut Tables| t.herd.understaffing_divisor += 1)),
        ("herd crowding", Box::new(|t: &mut Tables| t.herd.crowding[3].birth_rate += 1)),
        ("herd crowding band", Box::new(|t: &mut Tables| t.herd.crowding[0].density_max += 1)),
        ("herd small bonus", Box::new(|t: &mut Tables| t.herd.small_bonus[2].1 += 1)),
        ("herd no pasture", Box::new(|t: &mut Tables| t.herd.no_pasture_divisor += 1)),
        ("herd no pasture floor", Box::new(|t: &mut Tables| t.herd.no_pasture_kill_all_below += 1)),
        ("herd no pasture density", Box::new(|t: &mut Tables| t.herd.no_pasture_density += 1)),
        ("herd calving", Box::new(|t: &mut Tables| t.herd.calving_season += 1)),
        ("herd culling", Box::new(|t: &mut Tables| t.herd.culling_season += 1)),
        ("herd season bonus", Box::new(|t: &mut Tables| t.herd.season_bonus.1 += 1)),
        ("cattle job", Box::new(|t: &mut Tables| t.job.cattle_farming += 1)),
        ("castle start", Box::new(|t: &mut Tables| t.castle.starting_type += 1)),
        ("castle tax", Box::new(|t: &mut Tables| t.castle.tax_base[5] += 1)),
        ("castle bonus", Box::new(|t: &mut Tables| t.castle.tax_bonus_pct[5] += 1)),
        ("castle cost", Box::new(|t: &mut Tables| t.castle.cost[4].1 += 1)),
        ("castle workforce", Box::new(|t: &mut Tables| t.castle.workforce[4].1 += 1)),
        ("garrison", Box::new(|t: &mut Tables| t.castle.garrison_cap[5] += 1)),
        ("free archers", Box::new(|t: &mut Tables| t.castle.free_archers[5] += 1)),
        ("commodity", Box::new(|t: &mut Tables| t.commodity[3].divisor += 1)),
        ("commodity job", Box::new(|t: &mut Tables| t.commodity[3].job += 1)),
        ("job", Box::new(|t: &mut Tables| t.job.castle_building += 1)),
        ("weapon", Box::new(|t: &mut Tables| t.weapon[5].iron += 1)),
        ("good", Box::new(|t: &mut Tables| t.good[14].sell_price += 1)),
        ("wages", Box::new(|t: &mut Tables| t.wages.divisor_ai[2] += 1)),
        ("bankrupt max", Box::new(|t: &mut Tables| t.wages.bankrupt_stage_max += 1)),
        ("efficiency", Box::new(|t: &mut Tables| t.efficiency.max += 1)),
        ("ale", Box::new(|t: &mut Tables| t.ale.max += 1)),
        ("army happiness", Box::new(|t: &mut Tables| t.army_happiness_cost[101] += 1)),
        ("gold grant", Box::new(|t: &mut Tables| t.ai.gold_grant[4][3] += 1)),
        ("grants", Box::new(|t: &mut Tables| t.ai.grant_min_grain += 1)),
        ("neutral ladder", Box::new(|t: &mut Tables| t.ai.tax_ladder_neutral[7].1 += 1)),
        ("ai ladders", Box::new(|t: &mut Tables| t.ai.tax_ladders[2][7].1 += 1)),
        ("personality style", Box::new(|t: &mut Tables| t.ai.personality[3].farm_style += 1)),
        ("personality ladder", Box::new(|t: &mut Tables| t.ai.personality[3].tax_ladder += 1)),
        ("gold brackets", Box::new(|t: &mut Tables| t.score.gold_brackets[3].1 += 1)),
        ("score weights", Box::new(|t: &mut Tables| t.score.weights[5].1 += 1)),
        ("score offsets", Box::new(|t: &mut Tables| t.score.input_offsets[5] += 1)),
    ];

    for (name, mutate) in &mutations {
        let mut t = Tables::DEFAULT;
        mutate(&mut t);
        assert_ne!(t, Tables::DEFAULT, "the {name} mutation changed nothing");
        assert_ne!(ruleset_fingerprint(&t), base, "changing {name} left the fingerprint alone");
    }
}

#[test]
fn the_fingerprint_covers_a_fixed_and_known_number_of_bytes() {
    let mut c = l2_net::Canonical::hashing();
    l2_net::Encode::encode(&Tables::DEFAULT, &mut c);
    assert_eq!(c.finish().len, 2_168, "the ruleset encoding changed - bump VERSION?");
}


#[test]
fn a_flipped_byte_is_caught_by_the_checksum() {
    let k = furnished(2);
    let bytes = encode(&k);
    let body = HEADER_LEN..bytes.len() - 8;
    let mut caught = 0;
    for at in body.step_by(997) {
        let mut spoiled = bytes.clone();
        spoiled[at] ^= 0x01;
        match decode(&spoiled, Tables::DEFAULT) {
            Err(LoadError::Corrupt { .. }) => caught += 1,
            Err(other) => panic!("byte {at}: {other}"),
            Ok(_) => panic!("byte {at} was flipped and the save still loaded"),
        }
    }
    assert!(caught > 20, "only {caught} strides checked");
}

#[test]
fn a_truncated_save_is_refused() {
    let bytes = encode(&furnished(2));
    for cut in [HEADER_LEN + 8, bytes.len() / 2, bytes.len() - 9, bytes.len() - 1] {
        let err = decode(&bytes[..cut], Tables::DEFAULT).expect_err("must refuse");
        assert!(
            matches!(err, LoadError::TruncatedBody { .. } | LoadError::NotASave),
            "cut at {cut}: {err}"
        );
    }
}

#[test]
fn trailing_bytes_are_refused() {
    let mut bytes = encode(&furnished(2));
    bytes.push(0);
    assert!(matches!(
        decode(&bytes, Tables::DEFAULT),
        Err(LoadError::TruncatedBody { .. })
    ));
}


