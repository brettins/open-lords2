#![allow(unused_imports)]
use super::*;
use super::conquest_part::*;
use super::battle::*;
use l2_formats::save::Save;
use l2_game::engagement::{self, Answer, Resolution};
use l2_kingdom::conquest::{self, Attack};
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM};
use l2_kingdom::unit::{TroopType, Unit, UnitKind, TROOP_TYPES};

/// **`FUN_0049DF48` — a battle re-manages every AI realm's farms, and nobody
/// else's.**
///
/// `Battle_ReturnToCampaign` (`0x004AB383`) ends
/// `FUN_004AD426(); Panels_RefreshAll(); FUN_0049DF48();`, and `FUN_0049DF48`
/// is `Ai_ManageCountyFarms` for every realm with `isHuman == 0 &&
/// strength != 0`. `docs/decisions.md` C170 listed it as the one caller of the
/// four that was not ported.
///
/// The probe is the style byte: `Ai_ManageCountyFarms` writes the lord's
/// `farmStyle` into county `+0x1FE` for every county the realm holds, so a
/// sentinel there survives exactly where the pass does not reach. Realm 2 is an
/// AI with strength, realm 3 a person, realm 4 an AI at strength 0 — the loop's
/// two tests, one each.
///
/// *Ablation*: drop `ai_manage_farms_after_battle` from `engagement`'s battle
/// path and the first assertion goes red.
#[test]
fn a_battle_re_manages_every_ai_realms_farms_and_nobody_elses() {
    const SENTINEL: u8 = 200;
    let before_save: Save = l2_testkit::fixture!("battle-before.sav");
    let (mut k, attacker) = before(&before_save);

    for (realm, county, human, strength) in
        [(2usize, 1usize, false, 1u8), (3, 2, true, 1), (4, 4, false, 0)]
    {
        let r = &mut k.realms[realm];
        r.in_play = true;
        r.is_human = human;
        r.strength = strength;
        r.lord = 1;
        k.counties[county].owner = realm as u8;
        k.counties[county].farm_style = SENTINEL;
    }
    let style = k.tables.ai_farm_style(1).expect("lord 1 has a style");
    assert_ne!(style, SENTINEL, "the probe has to be distinguishable");

    let outcome = {
        let restore = k.restore();
        let Kingdom { counties, realms, campaign, options, tables, year, .. } = &mut k;
        conquest::attack_county(
            tables,
            &campaign.map,
            counties,
            realms,
            &mut campaign.units,
            &mut campaign.names,
            attacker,
            COUNTY,
            options.difficulty,
            *year,
            &mut campaign.explored,
            restore,
        )
    };
    engagement::resolve(&mut k, outcome, COUNTY, Answer::Decline, 1).expect("a battle");

    assert_eq!(k.counties[1].farm_style, style, "the AI lord's county was re-farmed");
    assert_eq!(k.counties[2].farm_style, SENTINEL, "a person's county was not");
    assert_eq!(k.counties[4].farm_style, SENTINEL, "nor a realm at strength 0");
}

