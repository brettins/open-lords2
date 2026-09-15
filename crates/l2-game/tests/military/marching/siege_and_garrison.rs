#![allow(unused_imports)]
use super::*;
use super::selection_and_orders::*;
use super::merging::*;
use super::movement_and_turn::*;
use super::rendering::*;
use super::*;
use super::battle_part::*;
use super::raising::*;
use super::division::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::battlefield as bf;
use l2_game::screens::{armoury, army, battle, divide, info, map};
use l2_game::Game;
use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

#[test]
fn clicking_a_besieging_army_opens_the_siege_screen() {
    let (mut g, a, mut m) = on_the_map();
    let (camp, _) = adjacent_pair(|x| x < 30);
    g.kingdom.counties[2].castle_type = 2;
    let garrison = army_at(&mut g, 2, 2, 120, (camp.0, camp.1 + 2));
    g.kingdom.campaign.units.get_mut(garrison).unwrap().garrison_county = 2;
    g.kingdom.counties[2].garrison_unit = garrison;

    let id = army_at(&mut g, 1, 1, 400, camp);
    g.kingdom.campaign.units.get_mut(id).unwrap().besieging_county = 2;
    g.kingdom.campaign.units.get_mut(garrison).unwrap().besieged_by = id as u8;

    click(&mut m, &mut g, &a, pixel(camp.0, camp.1).unwrap());
    assert_eq!(
        m.top_id(),
        Some(ScreenId::Siege(id)),
        "a besieging army is a siege, not a march order",
    );
}

#[test]
fn a_besieger_whose_garrison_has_gone_takes_orders_instead_of_opening_the_siege() {
    let (mut g, a, mut m) = on_the_map();
    let (camp, there) = adjacent_pair(|x| x < 30);
    let id = army_at(&mut g, 1, 1, 400, camp);
    g.kingdom.campaign.units.get_mut(id).unwrap().besieging_county = 2;
    g.kingdom.counties[2].garrison_unit = 0;

    click(&mut m, &mut g, &a, pixel(camp.0, camp.1).unwrap());
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "no siege screen: the link was stale");
    assert_eq!(
        g.kingdom.campaign.units.get(id).unwrap().besieging_county,
        0,
        "and the click is what cleared it",
    );
    click(&mut m, &mut g, &a, pixel(there.0, there.1).unwrap());
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving), "orders taken");
}

/// `Army_LeaveCastle`'s body `FUN_00437535` (`0x004374C4`) ends
/// `if (unit.besiegedBy && Battle_BeginFromCampaign(unit, unit.besiegedBy))
/// g_battleCounty = county;` — the marching garrison is `g_battleArmyA`, the
/// besieger `g_battleArmyB`,
/// occupant's. It used to march out onto its besieger and fight nothing.
#[test]
fn a_garrison_that_marches_out_onto_its_besieger_raises_the_battle_prompt() {
    let (mut g, a, mut m) = on_the_map();
    let (keep, camp) = adjacent_pair(|x| x < 30);
    g.kingdom.counties[1].castle_type = 2;

    let garrison = army_at(&mut g, 1, 1, 200, keep);
    g.kingdom.campaign.units.get_mut(garrison).unwrap().garrison_county = 1;
    g.kingdom.counties[1].garrison_unit = garrison;

    let besieger = army_at(&mut g, 2, 2, 300, camp);
    g.kingdom.campaign.units.get_mut(besieger).unwrap().besieging_county = 1;
    g.kingdom.campaign.units.get_mut(garrison).unwrap().besieged_by = besieger as u8;

    let out = g.leave_castle(garrison);
    assert!(
        matches!(out, l2_kingdom::conquest::LeftCastle::Marched { sortie: Some(b), .. }
            if b == besieger),
        "she marched, carrying the besieger: {out:?}",
    );

    let q = l2_game::turn::pending_question(&g).expect("the sortie is on the table");
    assert_eq!((q.attacker, q.defender), (garrison, besieger), "the marcher attacks");
    assert_eq!(q.county, 1, "g_battleCounty is the county left");
    assert!(!q.is_siege, "a field battle: Battle_BeginFromCampaign clears g_battleIsSiege");

    run_until(&mut m, &mut g, &a, "the sortie prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });
    assert!(
        g.kingdom.campaign.units.get(garrison).is_some()
            && g.kingdom.campaign.units.get(besieger).is_some(),
        "and nothing is resolved while the question stands",
    );
}

