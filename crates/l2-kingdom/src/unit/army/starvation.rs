#![allow(unused_imports)]
use super::*;
use super::unit_impl::*;
use super::units_impl::*;
use super::wages::*;
use super::combine_part::*;
use super::destroy_part::*;
use super::*;
use crate::county::{County, MAX_COUNTIES};
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;

/// What one army's starvation check did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Starvation {
    /// Fed, or garrisoned, or the option is off. The counter is back to 0.
    Fed,
    /// The counter stepped to 1: a warning and nothing else.
    Warned,
    /// The counter is 2…4: a tenth of every troop count over ten walks off.
    Deserted { lost: i32 },
    /// The counter reached 5. The army is gone.
    Perished,
}

/// `Army_Starve` (`0x004ACE5E`), once a season, from inside `Wages_PayAll` and
/// **before** anyone is paid — so an army can desert from hunger and be billed
/// the reduced wage in the same season.
///
/// ```c
/// for unit in 1..=150 where owner != 0 and type == 1:
///     unit.orderFlags = 0, 0, 0;                     /* unconditionally */
///     if (!g_optArmiesEat)                        unit.starvation = 0;
///     else if (unit.men < Food_Available(county))  unit.starvation = 0;   /* fed */
///     else if (unit.garrisonCounty != 0)          unit.starvation = 0;   /* the castle feeds it */
///     else {
///         unit.starvation++;
///         if (unit.starvation == 1)      warn;
///         else if (unit.starvation < 5)  { Army_Desert(unit); warn louder; }
///         else                           { warn; Army_Destroy(unit); }
///     }
/// ```
///
/// Four things worth pinning down, all `[D]`:
///
/// * **the fed test is a strict `<`**, so an army of exactly the county's
///   available food starves;
/// * the food it is compared against is **not** what is left after the peasants
///   ate — see [`crate::ration::food_available`] — and takes no account of
///   other armies in the same county, so an army of 400 in a county with 3,000
///   available is fed however many armies stand beside it;
/// * a **garrison never starves**, whatever the county holds;
/// * in the desert case the men leave *before* the message; in the destroy case
///   the message is raised *before* the army goes. That ordering is not
///   cosmetic — it decides the order two lockstep peers append to the report.
///
/// `armies_eat` is `g_optArmiesEat`, `L2.eng` group 50 index 2 — the advanced
/// option the game itself calls *"Army foraging"*, and it is **off in the
/// shipped save**
pub fn starve(
    t: &Tables,
    units: &mut Units,
    counties: &[County; MAX_COUNTIES],
    armies_eat: bool,
    out: &mut Vec<crate::report::Message>,
) -> Vec<(usize, Starvation)> {
    let ids: Vec<usize> = units
        .iter()
        .filter(|(_, u)| u.kind == UnitKind::Army && u.owner != 0)
        .map(|(i, _)| i)
        .collect();
    let mut outcomes = Vec::new();
    let mut doomed = Vec::new();

    for id in ids {
        let Some(u) = units.get(id) else { continue };
        let (county_id, men, garrisoned, realm) = (u.county, u.men, u.is_garrisoned(), u.owner);
        let food = counties
            .get(county_id as usize)
            .map_or(0, |c| crate::ration::food_available(t, c));

        let fed = !armies_eat || men < food || garrisoned;
        let u = units.get_mut(id).expect("still there");
        if fed {
            u.starvation = 0;
            outcomes.push((id, Starvation::Fed));
            continue;
        }
        u.starvation += 1;
        let stage = u.starvation;
        if stage == 1 {
            out.push(crate::report::Message::ArmyStarving { realm, unit: id, county: county_id, stage });
            outcomes.push((id, Starvation::Warned));
        } else if stage < crate::tables::STARVATION_LIMIT {
            let lost = u.desert();
            out.push(crate::report::Message::ArmyStarving { realm, unit: id, county: county_id, stage });
            outcomes.push((id, Starvation::Deserted { lost }));
        } else {
            out.push(crate::report::Message::ArmyStarving { realm, unit: id, county: county_id, stage });
            outcomes.push((id, Starvation::Perished));
            doomed.push(id);
        }
    }
    for id in doomed {
        units.remove(id);
    }
    outcomes
}

