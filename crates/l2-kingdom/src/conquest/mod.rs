//! The document traces the record, the levy, movement, supply, sieges and how a
//! battle *result* comes back, and there is a hole in the middle of it: what
//! makes an army standing on a county's castle *take* the county. That is
//! `FUN_004A6C68` (`0x004A6C68`, 1,264 bytes), called from the mover, and
//! nothing in the document mentions it, `FUN_004A50AE` which raises the
//! defence, or `FUN_004A72FE` which changes the owner. Three unnamed functions
//! between "armies move" and "there is a war". They are named here as
//! `Army_AttackCounty`, `County_RaiseDefence` (in [`crate::levy`]) and
//! `County_ChangeOwner`.
//!
//! **By stepping onto its castle tile.** `Unit_StepOnce` returns 5 for a
//! `plane0 & 0x40` tile, the mover then calls
//! `Transport_Deliver` and `Army_AttackCounty` with the tile's county. So the
//! castle site is expensive terrain the pathfinder routes around —
//! it is the objective, and `docs/armies.md` §2.2's castle row (*"5 →
//! `Transport_Deliver`, move ends"*) is half the story. `[D]`
//!
//! `[D]`

mod ownership;
pub use ownership::*;
mod attack;
pub use attack::*;
mod castle;
pub use castle::*;

use crate::county::{County, MAX_COUNTIES};
use crate::levy::{self, Defence};
use crate::map::CampaignMap;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::{Season, Tables};
use crate::unit::{ArmyNames, UnitKind, Units};

/// **The four globals `County_MakeIndependent` (`0x004AC3C6`) reads** and
/// `County_ChangeOwner`'s own arguments do not carry: `g_optArmiesEat` for
/// `Ration_Apply`, `g_seasonNext` and `g_optAdvancedFarming` for
/// `County_RefreshEstimates`, and `g_countyCount` for the blacksmiths' share.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Restore {
    /// `g_season` and `g_optAdvancedFarming` as `Ration_Apply` (`0x0044DF5F`)
    /// wants them: entering Spring it holds back the seed corn.
    pub sowing: crate::ration::Sowing,
    pub season_next: Season,
    pub advanced_farming: bool,
    pub armies_eat: bool,
    pub county_count: usize,
}

impl Restore {
    pub const NEUTRAL: Restore = Restore {
        sowing: crate::ration::Sowing::NONE,
        season_next: Season::Spring,
        advanced_farming: false,
        armies_eat: false,
        county_count: 0,
    };
}

/// In the shipped save every neutral county sits at 77, so on the England map
/// **every neutral county is a battle and none is a walk-in** at turn one. The
/// rule only bites once a county has been taxed or starved into misery, which
/// is the *"the people are wretched, my liege"* county of
/// `docs/armies.md` §2.5's greeting table. `[D]`
pub const SURRENDER_HAPPINESS: i32 = 11;

/// **An AI conqueror always costs 30; a human's cost rises with the
/// difficulty** — 10 at Easy, 30 at Normal, 50 at Hard. So the two are equal at
/// Normal and the setting decides whether conquest is cheaper or dearer for the
/// player than for the AI. `[D]`
pub fn capture_happiness_penalty(conqueror_is_human: bool, difficulty: u8) -> i32 {
    if conqueror_is_human {
        difficulty as i32 * 20 + 10
    } else {
        30
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    NotAnArmy,
    Ownerless,
    AlreadyYours,
    Garrisoned,
}

pub const ATTACK_MOVE_COST: i32 = 8;

/// `+0x167 = 1` — a defence levied out of the county's own people the moment
/// the attacker arrived. Winning with it takes the county; it is dissolved
/// afterwards and its survivors go home.
pub const RAISED: u8 = 1;
/// `+0x167 = 2` — an army that was already standing at the town. Winning with
/// it takes the county; it keeps its men and only loses the mark.
pub const PRESSED: u8 = 2;
pub const UNMARKED: u8 = 0;

/// **What `County_ChangeOwner` (`0x004A72FE`) knew when it chose its letter**,
/// as a value the same on every peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capture {
    pub new_owner: u8,
    pub old_owner: u8,
    pub county: u8,
    pub held_before: u8,
    /// Realm `+0x2A` before this capture raised it —
    /// [`crate::realm::Realm::peak_counties`].
    pub peak_before: u8,
    pub governable: bool,
    pub penalty: i32,
    /// What became of the castle's garrison — `FUN_00437535`, reached from
    /// `County_MakeIndependent`'s tail. `None` when the castle was empty or
    /// the garrison already belonged to the taker. See [`change_owner`].
    pub garrison: Option<LeftCastle>,
}

impl Capture {
    pub fn held_after(&self) -> u8 {
        self.held_before.wrapping_add(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastleArrival {
    Garrisoned(usize),
    GarrisonFull,
    Siege(Result<(), crate::siege::SiegeRefusal>),
    NotAnArmy,
}

pub const GARRISON_MOVE_COST: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeftCastle {
    NotAGarrison,
    /// `FUN_00437535` hands that pair to `Battle_BeginFromCampaign` and sets
    /// `g_battleCounty` to the county. **The battle is not staged here** —
    /// this crate cannot fight one; `l2_game::turn::raise_sortie` is the
    /// caller that does. The field is the original's `besiegedBy` test.
    Marched { tile: (u8, u8), sortie: Option<usize> },
    Destroyed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::explore::Explored;
    use crate::map::{CampaignMap, MAP_DIM, MAP_TILES};
    use crate::unit::Unit;

    const T: &Tables = &Tables::DEFAULT;

    fn two_county_map() -> CampaignMap {
        let mut m = CampaignMap::empty();
        for i in 0..MAP_TILES {
            m.county[i] = if i % MAP_DIM < 32 { 1 } else { 2 };
        }
        m
    }

    fn world() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
        let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
        let mut realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        for id in 1..=2 {
            counties[id].population = 500;
            counties[id].happiness = 77;
        }
        counties[1].owner = 1;
        realms[1].in_play = true;
        realms[1].is_human = true;
        realms[1].shield_index = 2;
        realms[2].in_play = true;
        bordering(&mut counties);
        (counties, realms)
    }

    fn attacker(units: &mut Units, owner: u8, county: u8) -> usize {
        let mut u = Unit::new(UnitKind::Army, owner, 40, 10);
        u.men = 400;
        u.troops[0] = 400;
        u.county = county;
        u.home_county = 1;
        units.spawn(u).unwrap()
    }

    #[test]
    fn a_county_needs_both_a_castle_and_a_garrison_to_be_shut() {
        let (mut counties, _) = world();
        let mut units = Units::new();
        let garrison = units.spawn(Unit::new(UnitKind::Army, 1, 0, 0)).unwrap();

        counties[1].castle_type = 0;
        counties[1].garrison_unit = 0;
        assert!(can_be_entered(&counties, &units, 1, 2), "open country");

        counties[1].castle_type = 3;
        assert!(can_be_entered(&counties, &units, 1, 2), "a castle nobody is in");

        counties[1].castle_type = 0;
        counties[1].garrison_unit = garrison;
        assert!(can_be_entered(&counties, &units, 1, 2), "a garrison with no castle");

        counties[1].castle_type = 3;
        assert!(!can_be_entered(&counties, &units, 1, 2), "both: this is a siege");
        assert!(can_be_entered(&counties, &units, 1, 1), "…unless the garrison is yours");
    }

    #[test]
    fn the_shipped_position_has_no_sieges_in_it() {
        let (counties, _) = world();
        let units = Units::new();
        for id in 1..=2u8 {
            assert_eq!(counties[id as usize].garrison_unit, 0);
            assert!(can_be_entered(&counties, &units, id, 9));
        }
    }

    #[test]
    fn a_shut_county_refuses_the_attack_rather_than_starting_a_battle() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let a = attacker(&mut units, 2, 1);
        let g = units.spawn(Unit::new(UnitKind::Army, 1, 1, 1)).unwrap();
        counties[1].castle_type = 4;
        counties[1].garrison_unit = g;

        let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 1, 0, 1268, &mut Explored::new(), Restore::NEUTRAL);
        assert_eq!(out, Attack::Refused(Refusal::Garrisoned));
        assert_eq!(counties[1].owner, 1, "and nothing changed hands");
        assert_eq!(units.get(a).unwrap().moves_used, 0, "not even the eight moves");
    }

    #[test]
    fn a_wretched_neutral_county_surrenders_and_a_contented_one_fights() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        counties[2].happiness = 10;
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let a = attacker(&mut units, 1, 2);

        let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268, &mut Explored::new(), Restore::NEUTRAL);
        assert!(matches!(out, Attack::Captured(_)), "got {out:?}");
        assert_eq!(counties[2].owner, 1);
        assert_eq!(units.get(a).unwrap().moves_used, ATTACK_MOVE_COST);

        let (mut counties, mut realms) = world();
        counties[2].happiness = SURRENDER_HAPPINESS;
        let mut units = Units::new();
        let a = attacker(&mut units, 1, 2);
        let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268, &mut Explored::new(), Restore::NEUTRAL);
        assert!(matches!(out, Attack::Battle { .. }), "got {out:?}");
        assert_eq!(counties[2].owner, 0, "the county is not taken by walking in");
    }

    #[test]
    fn a_neutral_county_at_the_shipped_happiness_raises_a_militia() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let a = attacker(&mut units, 1, 2);

        let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268, &mut Explored::new(), Restore::NEUTRAL);
        let Attack::Battle { attacker: att, defender } = out else { panic!("got {out:?}") };
        assert_eq!(att, a);
        let d = units.get(defender).unwrap();
        assert_eq!(d.owner, crate::levy::OWNERLESS);
        assert_eq!(d.men, 125, "a quarter of five hundred, at difficulty 0");
        assert_eq!(d.county, 2);
    }

    #[test]
    fn a_county_of_fewer_than_forty_people_is_simply_taken() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        counties[2].population = 30;
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let a = attacker(&mut units, 1, 2);
        assert!(matches!(
            attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268, &mut Explored::new(), Restore::NEUTRAL),
            Attack::Captured(_)
        ));
        assert_eq!(counties[2].owner, 1);
    }

    #[test]
    fn an_existing_army_in_an_owned_county_defends_it_rather_than_a_fresh_levy() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        counties[2].owner = 2;
        counties[2].anchor_x = 45;
        counties[2].anchor_y = 20;
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let a = attacker(&mut units, 1, 2);
        let mut standing = Unit::new(UnitKind::Army, 2, 45, 20);
        standing.men = 300;
        standing.county = 2;
        let existing = units.spawn(standing).unwrap();

        let out = attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 2, 0, 1268, &mut Explored::new(), Restore::NEUTRAL);
        assert_eq!(out, Attack::Battle { attacker: a, defender: existing });
        assert_eq!(units.len(), 2, "nothing new was levied");
        assert_eq!(counties[2].population, 500, "and nobody was called up");
    }


    fn standing(units: &mut Units, owner: u8, x: u8, y: u8, men: i32) -> usize {
        let mut u = Unit::new(UnitKind::Army, owner, x, y);
        u.men = men;
        u.county = 2;
        units.spawn(u).unwrap()
    }

    fn owned_county_two(anchor: (u8, u8)) -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS]) {
        let (mut counties, mut realms) = world();
        counties[2].owner = 2;
        counties[2].anchor_x = anchor.0;
        counties[2].anchor_y = anchor.1;
        realms[2].is_human = false;
        (counties, realms)
    }

    #[test]
    fn an_army_four_rows_from_the_town_does_not_defend_it() {
        let (counties, _) = owned_county_two((31, 50));

        let mut units = Units::new();
        standing(&mut units, 2, 30, 46, 300);
        assert_eq!(
            find_defender(&units, &counties, 2),
            None,
            "battle-before.sav: the army at (30,46) is outside the town's 4x4 block"
        );

        let mut units = Units::new();
        let close = standing(&mut units, 2, 30, 49, 300);
        assert_eq!(
            find_defender(&units, &counties, 2),
            Some(close),
            "…and one row nearer, it defends"
        );
    }

    #[test]
    fn the_defence_window_is_the_town_block_plus_its_one_tile_ring() {
        let (counties, _) = owned_county_two((31, 50));
        let inside = |x: u8, y: u8| {
            let mut units = Units::new();
            let u = standing(&mut units, 2, x, y, 300);
            find_defender(&units, &counties, 2) == Some(u)
        };

        for x in 29..=32u8 {
            for y in 48..=51u8 {
                assert!(inside(x, y), "({x},{y}) is inside the block");
            }
        }
        for (x, y) in [(28, 50), (33, 50), (31, 47), (31, 52), (28, 47), (33, 52)] {
            assert!(!inside(x, y), "({x},{y}) is outside it");
        }
    }

    #[test]
    fn the_largest_army_beside_the_town_defends_it_not_the_earliest_slot() {
        let (counties, _) = owned_county_two((31, 50));
        let mut units = Units::new();
        let _small = standing(&mut units, 2, 29, 48, 40);
        let big = standing(&mut units, 2, 32, 51, 400);
        let _middling = standing(&mut units, 2, 31, 50, 200);
        assert_eq!(find_defender(&units, &counties, 2), Some(big));
    }

    #[test]
    fn the_defence_search_ignores_other_realms_and_other_unit_kinds() {
        let (counties, _) = owned_county_two((31, 50));
        let mut units = Units::new();
        standing(&mut units, 1, 31, 50, 900);
        let mut trader = Unit::new(UnitKind::Merchant, 2, 30, 50);
        trader.men = 900;
        trader.county = 2;
        units.spawn(trader).unwrap();
        assert_eq!(find_defender(&units, &counties, 2), None);

        let own = standing(&mut units, 2, 29, 49, 10);
        assert_eq!(find_defender(&units, &counties, 2), Some(own), "ten men still beat nobody");
    }

    #[test]
    fn a_town_against_the_map_edge_finds_nobody() {
        for anchor in [(1u8, 50u8), (63, 50), (31, 1), (31, 63)] {
            let (counties, _) = owned_county_two(anchor);
            let mut units = Units::new();
            standing(&mut units, 2, anchor.0, anchor.1, 300);
            assert_eq!(find_defender(&units, &counties, 2), None, "anchor {anchor:?}");
        }
    }

    #[test]
    fn attacking_your_own_county_is_refused() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let a = attacker(&mut units, 1, 1);
        assert_eq!(
            attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, a, 1, 0, 1268, &mut Explored::new(), Restore::NEUTRAL),
            Attack::Refused(Refusal::AlreadyYours)
        );
    }

    #[test]
    fn a_merchant_cannot_take_a_county() {
        let m = two_county_map();
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let mut names = ArmyNames::new();
        let trader = units.spawn(Unit::new(UnitKind::Merchant, 1, 40, 10)).unwrap();
        assert_eq!(
            attack_county(T, &m, &mut counties, &mut realms, &mut units, &mut names, trader, 2, 0, 1268, &mut Explored::new(), Restore::NEUTRAL),
            Attack::Refused(Refusal::NotAnArmy)
        );
    }


    #[test]
    fn a_captured_county_loses_happiness_on_the_events_line() {
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        counties[2].happiness = 77;
        let lost = change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL).penalty;
        assert_eq!(lost, 10, "a human at difficulty 0");
        assert_eq!(counties[2].happiness, 67);
        assert_eq!(counties[2].shown_events, -10);
        assert_eq!(counties[2].owner, 1);
    }

    #[test]
    fn the_conquest_penalty_is_flat_for_an_ai_and_scales_for_a_human() {
        for d in 0..=3u8 {
            assert_eq!(capture_happiness_penalty(false, d), 30, "difficulty {d}");
        }
        assert_eq!(capture_happiness_penalty(true, 0), 10);
        assert_eq!(capture_happiness_penalty(true, 1), 30, "equal at Normal");
        assert_eq!(capture_happiness_penalty(true, 2), 50);
    }

    #[test]
    fn a_county_that_cannot_pay_the_penalty_goes_to_zero_and_the_panel_agrees() {
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        counties[2].happiness = 4;
        assert_eq!(change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 2, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL).penalty, 50);
        assert_eq!(counties[2].happiness, 0);
        assert_eq!(counties[2].shown_events, -4, "what was taken, not the fifty");
    }

    #[test]
    fn changing_hands_moves_the_county_between_the_two_realms_counts() {
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        counties[2].owner = 2;
        recount_realm_counties(&counties, &mut realms);
        assert_eq!((realms[1].county_count, realms[2].county_count), (1, 1));

        change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
        assert_eq!((realms[1].county_count, realms[2].county_count), (2, 0));
    }

    /// This is the turn `docs/plan.md` item 4 asks for, minus the battle. It
    /// exists because every other test here builds an army by hand, and a layer
    /// whose pieces each work but do not compose is the failure
    /// `docs/decisions.md` C21 is about.
    #[test]
    fn an_army_can_be_raised_marched_across_a_border_and_take_a_county() {
        use crate::levy::{self, LevyBasket, Muster};
        use crate::movement::Routing;

        let mut map = two_county_map();
        map.set_flags(50, 10, crate::map::flags::CASTLE);
        for x in 5..50u8 {
            map.set_flags(x, 10, crate::map::flags::ROAD);
        }
        let (mut counties, mut realms) = world();
        counties[2].population = 20; // too small to raise a defence
        realms[1].weapons = [100, 0, 0, 0, 0, 0];
        let mut units = Units::new();
        let mut names = ArmyNames::new();

        let levy = levy::set_percent(T, &counties[1], 20);
        assert_eq!(levy.men, 100);
        let mut basket = LevyBasket::seed(&realms[1], levy.men);
        basket.equip(crate::unit::TroopType::Crossbowman, 100);
        let army = levy::create_army(
            T, &map, &mut counties, &mut realms, &mut units, &mut names, &basket,
            Muster { realm: 1, county: 1, happiness_cost: levy.happiness_cost, year: 1268 },
            &mut Explored::new(),
        )
        .expect("county 1 has room");
        assert_eq!(counties[1].population, 400);
        assert_eq!(units.get(army).unwrap().men, 100);

        let mut campaign = crate::kingdom::Campaign::new();
        campaign.map = map;
        campaign.units = units;
        campaign.names = names;
        campaign.units.get_mut(army).unwrap().x = 5;
        campaign.units.get_mut(army).unwrap().y = 10;

        let mut seasons = 0;
        let outcome = loop {
            seasons += 1;
            assert!(seasons < 20, "it should not take twenty seasons to cross a map");
            crate::movement::order_move(&campaign.map, &mut campaign.units, army, (50, 10), Routing::Direct);
            let (_, outcome) =
                march_and_fight(T, &mut campaign, &mut counties, &mut realms, army, 0, 1268, Restore::NEUTRAL);
            if let Some(o) = outcome {
                break o;
            }
            campaign.units.reset_moves();
        };

        assert!(matches!(outcome, Attack::Captured(_)), "got {outcome:?}");
        assert_eq!(counties[2].owner, 1, "county 2 has changed hands");
        assert_eq!(realms[1].county_count, 2);
        assert_eq!(campaign.units.get(army).unwrap().county, 2, "and the army is standing in it");
    }

    fn bordering(counties: &mut [County; MAX_COUNTIES]) {
        counties[1].neighbour_count = 1;
        counties[1].neighbours[0] = 2;
        counties[2].neighbour_count = 1;
        counties[2].neighbours[0] = 1;
    }

    #[test]
    fn a_capture_reports_the_takers_holding_and_peak_from_before_the_write() {
        let (mut counties, mut realms) = world();
        bordering(&mut counties);
        let mut units = Units::new();
        counties[2].owner = 2;
        realms[1].peak_counties = 1;

        let c = change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
        assert_eq!((c.new_owner, c.old_owner, c.county), (1, 2, 2));
        assert_eq!(c.held_before, 1, "county 1, and not the county being taken");
        assert_eq!(c.held_after(), 2);
        assert_eq!(c.peak_before, 1);
        assert!(c.governable, "county 2 borders county 1");
        assert_eq!(realms[1].peak_counties, 2, "the peak rises to the new holding");
    }

    #[test]
    fn the_peak_remembers_ground_lost_and_retaking_it_is_not_a_new_high() {
        let (mut counties, mut realms) = world();
        bordering(&mut counties);
        let mut units = Units::new();
        counties[2].owner = 1;
        realms[1].peak_counties = 2;

        let lost = change_owner(T, &mut counties, &mut realms, &mut units, 2, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
        assert_eq!(lost.held_before, 0, "realm 2 held nothing, so it may take anything");
        assert!(lost.governable);
        assert_eq!(realms[1].peak_counties, 2, "the loser's peak stands");

        let back = change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
        assert_eq!((back.held_after(), back.peak_before), (2, 2), "back to the peak, not past it");
        assert_eq!(realms[1].peak_counties, 2);
    }

    #[test]
    fn a_county_far_from_the_takers_lands_declares_independence_instead() {
        let (mut counties, mut realms) = world();
        bordering(&mut counties);
        let mut units = Units::new();
        realms[1].peak_counties = 1;
        counties[3].population = 500;
        counties[3].happiness = 77;
        counties[3].owner = 2;
        counties[3].industry[0].enabled = true;
        counties[3].castle_switch = true;

        let far = change_owner(T, &mut counties, &mut realms, &mut units, 1, 3, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);

        assert!(!far.governable, "county 3 has no neighbours at all");
        assert_eq!(far.old_owner, 2, "the letter still knows who lost it");
        assert_eq!(counties[3].owner, 0, "nobody's, not the taker's");
        assert_eq!(realms[1].peak_counties, 1, "the peak is not raised");
        assert_eq!(realms[1].county_count, 1, "and neither is the count");
        assert_eq!(counties[3].happiness, 77, "the conquest penalty is in the other branch");
        assert!(
            counties[3].industry.iter().all(|i| !i.enabled),
            "`County_MakeIndependent` switches all four industries off",
        );
        assert!(!counties[3].castle_switch, "and `+0x1B0` with them");
    }

    #[test]
    fn a_realm_that_holds_nothing_keeps_a_county_it_cannot_reach() {
        let (mut counties, mut realms) = world();
        bordering(&mut counties);
        let mut units = Units::new();
        counties[3].population = 500;
        counties[3].happiness = 77;

        let first = change_owner(T, &mut counties, &mut realms, &mut units, 2, 3, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
        assert!(first.governable, "realm 2 held nothing");
        assert_eq!(counties[3].owner, 2);
    }

    #[test]
    fn a_second_change_owner_on_a_county_already_held_counts_it_twice() {
        let (mut counties, mut realms) = world();
        bordering(&mut counties);
        let mut units = Units::new();
        realms[1].peak_counties = 1;
        change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
        let again = change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
        assert_eq!(again.old_owner, 1, "nothing changes hands the second time");
        assert_eq!(again.held_before, 2, "the county is counted as already held");
        assert_eq!(realms[1].peak_counties, 3, "and the peak is one past the truth");
        assert_eq!(realms[1].county_count, 2, "while the recount is not");
    }

    #[test]
    fn taking_a_county_lifts_the_losers_garrison() {
        let (mut counties, mut realms) = world();
        let mut units = Units::new();
        let g = units.spawn(Unit::new(UnitKind::Army, 2, 0, 0)).unwrap();
        counties[2].owner = 2;
        counties[2].garrison_unit = g;
        // Both halves of the link, which is the only state `Army_GarrisonApply`
        // can leave: `FUN_00437535` clears the pair and tests neither.
        units.get_mut(g).unwrap().garrison_county = 2;
        let taken = change_owner(T, &mut counties, &mut realms, &mut units, 1, 2, 0, &CampaignMap::empty(), &mut Explored::new(), Restore::NEUTRAL);
        assert_eq!(counties[2].garrison_unit, 0);
        assert!(matches!(taken.garrison, Some(LeftCastle::Marched { .. })));
        // The loser keeps the men: `FUN_00437535` writes no owner byte.
        assert_eq!(units.get(g).map(|u| (u.owner, u.garrison_county)), Some((2, 0)));
    }
}

