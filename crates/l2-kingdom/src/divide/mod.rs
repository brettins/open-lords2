//! **Splitting an army in two, and disbanding one** — the other two ends of
//! [`crate::levy`].
//!
//! `levy::create_army` puts men on the map and [`crate::unit::combine`] merges
//! two armies into one. This module is the pair that was missing: `Army_Split`
//! (`0x00437FD7`) and `Army_Disband` (`0x00438681`), with the gate each of them
//! sits behind.
//!
//! # The shipped `Readme.txt` is a source here, and it is the better one
//!
//! Three of the rules below are stated in the v1.03 errata in words before they
//! are found in the instruction stream, and each of the three is one a reader of
//! the code alone would get wrong:
//!
//! * *"An army normally can only be split only at the start of its movement in
//!   a turn. Splitting does not use all the movement for a turn, but cannot be
//!   done if the army has used any movement points that turn."* — the gate is
//! `movesUsed < 1` in `FUN_004378B3`.
//!   **+5 both halves pay** in `Army_Split`, which nothing had recorded.
//! * *"When splitting into castles, you may split off less than 50 men. This
//!   will make it much easier to bring a current garrison up to its maximum
//!   size."* — the minimum is a local that is `0x32` on the plain path and **0**
//! when a destination county was named.
//!   limit less whoever is already inside.
//! * *"If you no longer control an army's county of origin, you can only disband
//!   it by moving it to any friendly county before disbanding it."* —
//!   `Panel_DisbandButton` picks the home county, falls back to the county the
//!   army is standing in when the home county has changed hands, and refuses
//!   with `L2.eng` group 145 when *that* is not the owner's either. The string
//!   states the two-step rule clause for clause, which is what makes it `[V]`.
//!
//! # The split screen reuses the levy basket, with different field meanings
//!
//! `docs/armies.md` §6.2 says so and this is the confirmation:
//! `Screen_SplitArmyRows` draws the **parent** from `g_levyBasket[t].chosen` and
//! the **daughter** from `g_levyBasket[t].available` — the second word of the
//! same sixteen-byte slot — for t = 0…6, and slot **7 holds the mercenary
//! band's men**
//! `DAT_00554468` and `DAT_00554040`, each recomputed as the sum of all eight
//! slots after every button.
//!
//! [`SplitBasket`] is that state, with the two halves as two arrays because we
//! are not byte-compatible with the original's buffer and aliasing a field on
//! purpose is how a reader ends up reading the wrong half. `[V]`

mod split_part;
pub use split_part::*;
mod disband_part;
pub use disband_part::*;

use crate::county::{County, MAX_COUNTIES};
use crate::map::{flags, CampaignMap};
use crate::mercenary::MercenaryBands;
use crate::realm::{Realm, MAX_REALMS};
use crate::tables::Tables;
use crate::unit::{
    ArmyNames, Mercenaries, TroopType, Unit, UnitKind, Units, MAX_UNIT_ID, TROOP_TYPES,
};

/// `FUN_00437FD7`'s `movesUsed += 5` on **both** halves of a plain split.
///
/// The Readme's *"splitting does not use all the movement for a turn"* is this
/// number: an army with the full fifteen keeps ten after dividing. It is only
/// charged when no destination county was named — a split *into* a castle takes
/// the other arm, where the daughter is given a path instead. `[D]`
pub const SPLIT_MOVE_COST: i32 = 5;

/// The cap `FUN_00437AFB` uses when there is no castle to split into: a number
/// far above [`crate::tables::ARMY_MAX_MEN`], so on the plain path it never
/// bites. Kept as the literal
/// it is the branch the castle path replaces.
pub const SPLIT_NO_CASTLE_CAP: i32 = 5000;

/// How far `FUN_0046733C` will look for somewhere to stand a new army.
pub const SPLIT_SEARCH_RADIUS: i32 = 5;

// ---------------------------------------------------------------- the basket

/// The army-division screen's state: who stays and who leaves.
///
/// Slot 7 — the mercenary band — is **not** a count that can be divided. It
/// moves whole or not at all, which is the same atomicity `combine` refuses two
/// of and `Mercenary_Release` walks off in one piece. `docs/armies.md` §5.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SplitBasket {
    /// `g_levyBasket[t].chosen`, t = 0…6 — the army that stays.
    pub parent: [i32; TROOP_TYPES],
    /// `g_levyBasket[t].available`, t = 0…6 — the army that leaves.
    pub daughter: [i32; TROOP_TYPES],
    /// The band, and which side of the screen it is on. `None` when the army
    /// carries none.
    pub mercenaries: Option<Mercenaries>,
    /// Whether the band sits in the daughter's column — slot 7's `available`
    /// being non-zero, in the original.
    pub mercenaries_leave: bool,
}

impl SplitBasket {
    /// `FUN_004378B3`'s seeding: every man in the parent's column, the daughter
    /// empty.
    pub fn seed(unit: &Unit) -> SplitBasket {
        SplitBasket {
            parent: unit.troops,
            daughter: [0; TROOP_TYPES],
            mercenaries: unit.mercenaries,
            mercenaries_leave: false,
        }
    }

    /// What the parent's *"Total men"* line reads — the sum of all eight slots,
    /// the band included.
    pub fn parent_total(&self) -> i32 {
        self.parent.iter().sum::<i32>() + self.side_mercenaries(false)
    }

    /// The daughter's *"Total men"* line.
    pub fn daughter_total(&self) -> i32 {
        self.daughter.iter().sum::<i32>() + self.side_mercenaries(true)
    }

    fn side_mercenaries(&self, leaving: bool) -> i32 {
        match self.mercenaries {
            Some(m) if self.mercenaries_leave == leaving => m.men(),
            _ => 0,
        }
    }

    /// `FUN_00437E9E` — the button that moves one man from the parent's column
    /// into the daughter's. Returns how many actually moved.
    pub fn to_daughter(&mut self, troop: TroopType, n: i32) -> i32 {
        let t = troop.index();
        let moved = n.min(self.parent[t]).max(0);
        self.parent[t] -= moved;
        self.daughter[t] += moved;
        moved
    }

    /// `FUN_00437D65` — the same button the other way.
    pub fn to_parent(&mut self, troop: TroopType, n: i32) -> i32 {
        let t = troop.index();
        let moved = n.min(self.daughter[t]).max(0);
        self.daughter[t] -= moved;
        self.parent[t] += moved;
        moved
    }

    /// Hotspot 7 on either handler: the band crosses whole. Returns whether it
    /// moved
    pub fn move_mercenaries(&mut self, leaving: bool) -> bool {
        if self.mercenaries.is_none() || self.mercenaries_leave == leaving {
            return false;
        }
        self.mercenaries_leave = leaving;
        true
    }
}

// --------------------------------------------------------------- the refusals

/// Where a split is going: a plain division on the open map, or into a castle.
///
/// The variant is `DAT_0053F080`, the destination county, being zero or not —
/// and it is what decides both the minimum and the cap in `FUN_00437AFB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitInto {
    /// No destination county. Both halves need [`crate::tables::ARMY_MIN_MEN`]
    /// and both pay [`SPLIT_MOVE_COST`].
    Field,
    /// The daughter is walking into `county`'s castle. **No minimum**.
    /// cap is that castle's garrison limit less whoever is already in it.
    Castle { county: u8, tile: (u8, u8) },
}

impl SplitInto {
    /// `DAT_0053F080` — the destination county, and **zero is the whole of what
    /// the plain path is**. Every branch in `FUN_00437AFB` and `Army_Split`
    /// tests this against 0
    pub fn county(self) -> u8 {
        match self {
            SplitInto::Field => 0,
            SplitInto::Castle { county, .. } => county,
        }
    }
}

/// Why a split was refused, each carrying the message the original raises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitRefusal {
    /// The slot is empty or is not an army.
    NotAnArmy,
    /// `FUN_004378B3`'s gate: message `0x95` = `L2.eng` group 149. The army has
    /// already spent movement this season, so the screen never opens.
    AlreadyMoved,
    /// One of the two columns is empty. `FUN_00437AFB` clears the hotspot and
    /// returns — no message, nothing happens, which is the *cancel* case rather
    /// than an error.
    Empty,
    /// Message `0x94` = group 148, *"impractical to create an army of less than
    /// 50 men"*. Only reachable on [`SplitInto::Field`].
    TooFew,
    /// Message `0x11A` = group 282. The daughter is bigger than the room left in
    /// the castle; the number is what would fit.
    GarrisonFull(i32),
    /// `FUN_0046733C` found no free tile within [`SPLIT_SEARCH_RADIUS`], or
    /// every unit slot is taken. The original returns silently.
    NowhereToStand,
}

/// Why a disband was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisbandRefusal {
    NotAnArmy,
    /// Message `0x91` = `L2.eng` group 145: *"Your army must disband to its
    /// county of origin. If you no longer rule the county, the army must then
    /// disband inside a county that you do rule."*
    NowhereToGo,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tables::Tables;

    const T: &Tables = &Tables::DEFAULT;

    fn army(men: [i32; TROOP_TYPES]) -> Unit {
        let mut u = Unit::new(UnitKind::Army, 1, 10, 10);
        u.troops = men;
        u.men = men.iter().sum();
        u.home_county = 1;
        u.county = 1;
        u
    }

    fn world() -> ([County; MAX_COUNTIES], [Realm; MAX_REALMS], Units, ArmyNames, MercenaryBands) {
        let mut counties: [County; MAX_COUNTIES] = core::array::from_fn(|_| County::new());
        counties[1].owner = 1;
        let realms: [Realm; MAX_REALMS] = core::array::from_fn(|_| Realm::new());
        (counties, realms, Units::new(), ArmyNames::new(), MercenaryBands::none())
    }

    #[test]
    fn the_basket_moves_men_one_column_at_a_time_and_both_totals_follow() {
        let u = army([100, 0, 0, 60, 0, 0, 0]);
        let mut b = SplitBasket::seed(&u);
        assert_eq!(b.parent_total(), 160);
        assert_eq!(b.daughter_total(), 0);
        assert_eq!(b.to_daughter(TroopType::Peasant, 60), 60);
        assert_eq!(b.parent_total(), 100);
        assert_eq!(b.daughter_total(), 60);
        // A column cannot go below zero.
        assert_eq!(b.to_daughter(TroopType::Peasant, 999), 40);
        assert_eq!(b.to_parent(TroopType::Crossbowman, 5), 0);
        assert_eq!(b.parent_total() + b.daughter_total(), 160, "men are conserved");
    }

    #[test]
    fn the_mercenary_band_crosses_whole_or_not_at_all() {
        let mut u = army([200, 0, 0, 0, 0, 0, 0]);
        u.mercenaries = Some(Mercenaries { band: 3, troop: TroopType::Maceman, men: 150 });
        u.men += 150;
        let mut b = SplitBasket::seed(&u);
        assert_eq!(b.parent_total(), 350, "slot 7 is counted in the total");
        assert_eq!(b.daughter_total(), 0);
        assert!(b.move_mercenaries(true));
        assert_eq!(b.parent_total(), 200);
        assert_eq!(b.daughter_total(), 150);
        assert!(!b.move_mercenaries(true), "it is already there");
        assert!(b.move_mercenaries(false));
    }

    #[test]
    fn an_army_that_has_moved_this_season_cannot_be_split() {
        let (mut counties, mut realms, mut units, mut names, mut bands) = world();
        let mut u = army([200, 0, 0, 0, 0, 0, 0]);
        u.moves_used = 1;
        let id = units.spawn(u.clone()).unwrap();
        let mut b = SplitBasket::seed(&u);
        b.to_daughter(TroopType::Peasant, 100);
        let map = CampaignMap::empty();
        assert_eq!(
            split(T, &map, &mut counties, &mut realms, &mut units, &mut names, &mut bands, id, &b, SplitInto::Field, 1268),
            Err(SplitRefusal::AlreadyMoved),
        );
    }

    #[test]
    fn the_field_split_needs_fifty_a_side_and_the_castle_split_does_not() {
        let (counties, _, mut units, _, _) = world();
        let u = army([200, 0, 0, 0, 0, 0, 0]);
        units.spawn(u.clone());
        let mut b = SplitBasket::seed(&u);
        b.to_daughter(TroopType::Peasant, 20);
        assert_eq!(
            refuse_split(T, &counties, &units, &b, SplitInto::Field),
            Some(SplitRefusal::TooFew),
            "20 men is under the 0x32 the plain path enforces",
        );
        // The Readme's own words: "When splitting into castles, you may split
        // off less than 50 men."
        let mut counties = counties;
        counties[2].owner = 1;
        counties[2].castle_type = 1;
        assert_eq!(
            refuse_split(T, &counties, &units, &b, SplitInto::Castle { county: 2, tile: (5, 5) }),
            None,
        );
    }

    #[test]
    fn the_castle_split_is_capped_by_the_room_left_in_the_garrison() {
        let (mut counties, _, mut units, _, _) = world();
        counties[2].owner = 1;
        counties[2].castle_type = 1; // a wooden palisade: 150 men
        let u = army([400, 0, 0, 0, 0, 0, 0]);
        units.spawn(u.clone());
        let mut b = SplitBasket::seed(&u);
        b.to_daughter(TroopType::Peasant, 200);
        let into = SplitInto::Castle { county: 2, tile: (5, 5) };
        assert_eq!(
            refuse_split(T, &counties, &units, &b, into),
            Some(SplitRefusal::GarrisonFull(150)),
        );
        // With ninety already inside, only sixty more fit.
        let mut garrison = army([90, 0, 0, 0, 0, 0, 0]);
        garrison.garrison_county = 2;
        counties[2].garrison_unit = units.spawn(garrison).unwrap();
        assert_eq!(
            refuse_split(T, &counties, &units, &b, into),
            Some(SplitRefusal::GarrisonFull(60)),
        );
    }

    #[test]
    fn a_disband_returns_the_weapons_to_the_treasury_and_the_men_to_the_county() {
        let (mut counties, mut realms, mut units, mut names, mut bands) = world();
        counties[1].population = 1_000;
        // 40 peasants, 30 crossbowmen, 30 knights.
        let mut u = army([40, 30, 0, 0, 0, 0, 30]);
        u.owner = 1;
        let id = units.spawn(u).unwrap();
        let (county, men) =
            disband(T, &mut counties, &mut realms, &mut units, &mut names, &mut bands, id, 0)
                .unwrap();
        assert_eq!((county, men), (1, 100));
        assert_eq!(counties[1].population, 1_100, "the men join the county");
        assert_eq!(realms[1].weapons[0], 30, "thirty crossbows come back");
        assert_eq!(realms[1].weapons[5], 30, "and thirty suits of mail");
        assert_eq!(realms[1].weapons[1], 0, "a peasant returns nothing");
        assert!(units.get(id).is_none(), "the slot is free again");
    }

    /// The Readme's *"If you no longer control an army's county of origin, you
    /// can only disband it by moving it to any friendly county"*, both halves.
    #[test]
    fn an_army_whose_home_county_has_changed_hands_disbands_where_it_stands() {
        let (mut counties, mut realms, mut units, mut names, mut bands) = world();
        counties[1].owner = 2; // the home county has fallen
        counties[3].owner = 1; // but the army is standing somewhere friendly
        let mut u = army([100, 0, 0, 0, 0, 0, 0]);
        u.owner = 1;
        u.home_county = 1;
        u.county = 3;
        let id = units.spawn(u.clone()).unwrap();
        assert_eq!(disband_county(&counties, &units, id), Some(3));

        // Standing in enemy country there is nowhere to go at all.
        units.get_mut(id).unwrap().county = 2;
        assert_eq!(disband_county(&counties, &units, id), None);
        assert_eq!(
            disband(T, &mut counties, &mut realms, &mut units, &mut names, &mut bands, id, 0),
            Err(DisbandRefusal::NowhereToGo),
        );
        assert!(units.get(id).is_some(), "a refused disband changes nothing");
    }

    #[test]
    fn a_disbanded_army_walks_its_mercenaries_off_rather_than_settling_them() {
        let (mut counties, mut realms, mut units, mut names, _) = world();
        counties[1].population = 500;
        let mut bands = MercenaryBands::init(14);
        let mut u = army([100, 0, 0, 0, 0, 0, 0]);
        u.owner = 1;
        let id = units.spawn(u).unwrap();
        bands.hire(&mut units, &mut counties, &mut realms, id, 1);
        assert!(units.get(id).unwrap().men > 100, "the band joined the total");
        let (_, men) =
            disband(T, &mut counties, &mut realms, &mut units, &mut names, &mut bands, id, 0)
                .unwrap();
        assert_eq!(men, 100, "only the levy joins the county");
        assert_eq!(counties[1].population, 600);
        assert!(bands.get(1).unwrap().is_available(), "the band is for hire again");
    }
}

