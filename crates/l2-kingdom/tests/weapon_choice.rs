//! **The blacksmith's weapon choice — `FUN_0043A997` (`0x0043A997`) — by the
//! road the game travels.**
//!
//! A player: *"I can't choose what type of weapon my blacksmiths are making."*
//! The type is `County::weapon_type`, county `+0x290`, and the six hotspots over
//! the smithy picture are the only control on any job popup — so until this
//! setter existed the byte was written by `AI_ChooseIndustry`'s rota and by
//! nothing a person could reach.
//!
//! ```c
//! county[+0x290] = weaponType;
//! Industry_LabourEstimate(county, 2, 7, 0xF, 4);
//! Labour_Allocate(county);
//! County_RefreshEstimates(county, g_seasonNext);
//! FUN_00448648(owner);
//! ```
//!
//! `docs/agents.md`, *a test that drives the picture from the wrong field passes
//! for ever*. The state this needs — two smithies of one realm, both switched on
//! and both staffed — is exactly the state `Industry_WeaponShares` (`0x0044F15B`)
//! divides between, and it is reached here the way a player reaches it:
//!
//! `docs/agents.md`, *how to ablate wrongly*: nothing here reads `Tables`,
//! `weapon_shares` or `resource_limit`. The two weapons are named by the numbers
//! `g_weaponCost` (`0x004D8990`) holds for them, and the claim is a direction —
//! the ceiling *moves* — this file would have
//! to re-derive.

use l2_kingdom::industry::MapToggle;
use l2_kingdom::tables::Commodity;
use l2_kingdom::Kingdom;
use l2_scenario::Scenario;

const BOW: usize = 4;
const ARMOUR: usize = 5;

/// The blacksmith's labour slot, county `+0xC4 + 7*0x0C`.
const SMITH: usize = 7;
const FOREST: usize = 6;

fn realm_with_two_staffed_smithies(kingdom: &mut Kingdom) -> (u8, usize, usize) {
    let count = kingdom.county_count;
    for owner in 1..kingdom.realms.len() as u8 {
        let mine: Vec<usize> = (1..=count)
            .filter(|&id| {
                kingdom.counties[id].owner == owner
                    && kingdom.counties[id].industry[Commodity::Weapons.index()].has_resource
                    && kingdom.counties[id].pop_band > 0
            })
            .collect();
        if mine.len() < 2 {
            continue;
        }
        let (a, b) = (mine[0], mine[1]);
        for id in [a, b] {
            if !kingdom.counties[id].industry[Commodity::Weapons.index()].enabled {
                kingdom.toggle_industry(id, MapToggle::Industry(Commodity::Weapons));
            }
            let spare = kingdom.counties[id].labour[FOREST] / 2;
            if spare > 0 {
                kingdom.move_labour(id, FOREST, SMITH, spare);
            }
        }
        if kingdom.counties[a].labour[SMITH] > 0 && kingdom.counties[b].labour[SMITH] > 0 {
            return (owner, a, b);
        }
    }
    panic!("setup: no realm in this fixture has two counties that can both staff a smithy");
}

/// 1. the byte lands — `county[+0x290]` is what was clicked;
/// 2. a county out of range and a weapon out of the table are refused, and the
///    refusal changes nothing;
/// 3. **the other county of the same realm moves**, which is `FUN_00448648`, the
///    setter's last statement: a blacksmith's ceiling is a share of the realm's
///    stockpile split across every staffed smithy it owns, so what one county
///    forges changes what another one can. The Readme's *"turning a blacksmith on
///    will reduce the resources available to other blacksmiths"*, from the other
///    side;
/// 4. and the clicked county's **headcount** moves,
///    of the middle three statements: the estimate writes `labour_useful[7]`, the
///    ceiling, and `Labour_Allocate` runs *after* it and deals against it.
#[test]
fn choosing_a_weapon_rewrites_the_county_and_every_smithy_of_its_realm() {
    let save = l2_testkit::fixture!("siege-lastturn.sav");
    let scenario = Scenario::from_save(&save).expect("import");
    let mut kingdom = scenario.kingdom(1);

    let (owner, a, b) = realm_with_two_staffed_smithies(&mut kingdom);
    assert_ne!(a, b);
    assert_eq!(kingdom.counties[b].owner, owner);

    assert!(kingdom.set_weapon_type(a, BOW), "the clicked county takes the bow");
    assert!(kingdom.set_weapon_type(b, BOW), "and so does its neighbour");
    assert_eq!(kingdom.counties[a].weapon_type, BOW, "claim 1: the byte lands");

    let before_b = (
        kingdom.counties[b].labour_useful[SMITH],
        kingdom.counties[b].industry[Commodity::Weapons.index()].next_season,
    );
    let before_a_smiths = kingdom.counties[a].labour[SMITH];

    let snapshot = kingdom.counties[a].clone();
    assert!(!kingdom.set_weapon_type(0, ARMOUR), "county 0 is not a county");
    assert!(!kingdom.set_weapon_type(a, 6), "there are six weapons, 0..5");
    assert_eq!(kingdom.counties[a].weapon_type, BOW, "claim 2: a refusal writes nothing");
    assert_eq!(kingdom.counties[a].labour, snapshot.labour, "nor does it re-allocate");

    assert!(kingdom.set_weapon_type(a, ARMOUR));
    assert_eq!(kingdom.counties[a].weapon_type, ARMOUR);

    let after_b = (
        kingdom.counties[b].labour_useful[SMITH],
        kingdom.counties[b].industry[Commodity::Weapons.index()].next_season,
    );
    assert_ne!(
        before_b, after_b,
        "claim 3: county {b}'s ceiling and forecast did not move when county {a} changed \
         what it forges — FUN_00448648 is the setter's last statement and it runs over \
         every county of realm {owner}"
    );

    assert_ne!(
        before_a_smiths,
        kingdom.counties[a].labour[SMITH],
        "claim 4: county {a}'s own headcount did not move — the estimate runs before \
         Labour_Allocate, so the allocator deals against the ceiling the new weapon makes"
    );
}
