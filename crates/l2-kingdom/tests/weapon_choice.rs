//! **The blacksmith's weapon choice — `FUN_0043A997` (`0x0043A997`) — by the
//! road the game travels.**
//!
//! ```text
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-kingdom --test weapon_choice
//! ```
//!
//! A player: *"I can't choose what type of weapon my blacksmiths are making."*
//! The type is `County::weapon_type`, county `+0x290`, and the six hotspots over
//! the smithy picture are the only control on any job popup — so until this
//! setter existed the byte was written by `AI_ChooseIndustry`'s rota and by
//! nothing a person could reach.
//!
//! The setter is five statements and four of them are the recompute:
//!
//! ```c
//! county[+0x290] = weaponType;
//! Industry_LabourEstimate(county, 2, 7, 0xF, 4);
//! Labour_Allocate(county);
//! County_RefreshEstimates(county, g_seasonNext);
//! FUN_00448648(owner);
//! ```
//!
//! # Why the road, and why two counties
//!
//! `docs/agents.md`, *a test that drives the picture from the wrong field passes
//! for ever*. The state this needs — two smithies of one realm, both switched on
//! and both staffed — is exactly the state `Industry_WeaponShares` (`0x0044F15B`)
//! divides between, and it is reached here the way a player reaches it:
//! [`Kingdom::toggle_industry`], the map click, and [`Kingdom::move_labour`], the
//! village drag. A hand-built county would have let the last claim pass against a
//! share nothing computed.
//!
//! **The fixture is `siege-lastturn.sav` and not England turn one**, and that is
//! the whole difficulty: on England turn one **every realm owns exactly one
//! county**, so the realm-wide half of this setter has nothing to reach and the
//! strongest claim below could not be made at all. The siege position's realm 1
//! holds three.
//!
//! # Why the costs are typed out
//!
//! `docs/agents.md`, *how to ablate wrongly*: nothing here reads `Tables`,
//! `weapon_shares` or `resource_limit`. The two weapons are named by the numbers
//! `g_weaponCost` (`0x004D8990`) holds for them, and the claim is a direction —
//! the ceiling *moves* — rather than an arithmetic identity this file would have
//! to re-derive.

use l2_kingdom::industry::MapToggle;
use l2_kingdom::tables::Commodity;
use l2_kingdom::Kingdom;
use l2_scenario::Scenario;

/// `g_weaponCost` rows. **Bow costs 13 wood and no iron; armour costs 4 wood and
/// 18.** They are the two extremes of the table and the pair most likely to move
/// a share in either direction.
const BOW: usize = 4;
const ARMOUR: usize = 5;

/// The blacksmith's labour slot, county `+0xC4 + 7*0x0C`.
const SMITH: usize = 7;
/// Wood cutting, the job England's counties open with people in.
const FOREST: usize = 6;

/// Two counties of one realm with a smithy each, both switched on and staffed.
/// Returns `(owner, a, b)`.
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
            // The map click that switches a smithy on — and only if it is off,
            // because the click is a toggle.
            if !kingdom.counties[id].industry[Commodity::Weapons.index()].enabled {
                kingdom.toggle_industry(id, MapToggle::Industry(Commodity::Weapons));
            }
            // The village drag that staffs it. `Labour_Move` clamps nothing, so
            // this asks for a share of the foresters rather than all of them.
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

/// **The write, the county's own recompute, and the realm's.**
///
/// Four claims:
///
/// 1. the byte lands — `county[+0x290]` is what was clicked;
/// 2. a county out of range and a weapon out of the table are refused, and the
///    refusal changes nothing;
/// 3. **the other county of the same realm moves**, which is `FUN_00448648`, the
///    setter's last statement: a blacksmith's ceiling is a share of the realm's
///    stockpile split across every staffed smithy it owns, so what one county
///    forges changes what another one can. The Readme's *"turning a blacksmith on
///    will reduce the resources available to other blacksmiths"*, from the other
///    side;
/// 4. and the clicked county's **headcount** moves, which is what fixes the order
///    of the middle three statements: the estimate writes `labour_useful[7]`, the
///    ceiling, and `Labour_Allocate` runs *after* it and deals against it.
///
/// **Ablations.** Deleting the closing `refresh_blacksmiths(owner)` turns claim 3
/// red and nothing else. Deleting the leading `industry::refresh(…)` — the
/// original's `Industry_LabourEstimate(county, 2, 7, 0xF, 4)` — turns claim 4 red
/// while the two refreshes below it put the *ceiling* right anyway, which is the
/// whole reason claim 4 reads the headcount and not the ceiling.
#[test]
fn choosing_a_weapon_rewrites_the_county_and_every_smithy_of_its_realm() {
    let save = l2_testkit::fixture!("siege-lastturn.sav");
    let scenario = Scenario::from_save(&save).expect("import");
    let mut kingdom = scenario.kingdom(1);

    let (owner, a, b) = realm_with_two_staffed_smithies(&mut kingdom);
    assert_ne!(a, b);
    assert_eq!(kingdom.counties[b].owner, owner);

    // Start both on the bow — 13 wood, no iron — so the change below is a real
    // change and not a repaint.
    assert!(kingdom.set_weapon_type(a, BOW), "the clicked county takes the bow");
    assert!(kingdom.set_weapon_type(b, BOW), "and so does its neighbour");
    assert_eq!(kingdom.counties[a].weapon_type, BOW, "claim 1: the byte lands");

    let before_b = (
        kingdom.counties[b].labour_useful[SMITH],
        kingdom.counties[b].industry[Commodity::Weapons.index()].next_season,
    );
    let before_a_smiths = kingdom.counties[a].labour[SMITH];

    // Claim 2, before anything moves: a refusal is a refusal.
    let snapshot = kingdom.counties[a].clone();
    assert!(!kingdom.set_weapon_type(0, ARMOUR), "county 0 is not a county");
    assert!(!kingdom.set_weapon_type(a, 6), "there are six weapons, 0..5");
    assert_eq!(kingdom.counties[a].weapon_type, BOW, "claim 2: a refusal writes nothing");
    assert_eq!(kingdom.counties[a].labour, snapshot.labour, "nor does it re-allocate");

    // The click: armour, 4 wood and 18 iron, the dearest row in the table.
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
