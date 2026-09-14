#![allow(unused_imports)]
use super::*;
use super::tick::*;
use crate::conquest::{self, Attack};
use crate::kingdom::Kingdom;
use crate::merchant;
use crate::movement::{self, Entry, Offence};
use crate::phase::Phase;
use crate::unit::{UnitKind, Units};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
    use crate::unit::Unit;
    use crate::Options;

    /// Two counties either side of x = 32, a road along y = 10, and a realm 1
    /// that is human.
    fn kingdom() -> Kingdom {
        let mut k = Kingdom::new(0x2E5);
        assert!(k.set_county_count(2));
        k.season = 1;
        k.season_next = 2;
        k.year = 1268;
        k.options = Options { armies_eat: false, ..Options::default() };

        let mut map = CampaignMap::empty();
        for i in 0..MAP_TILES {
            map.county[i] = if i % MAP_DIM < 32 { 1 } else { 2 };
        }
        for x in 0..64u8 {
            map.set_flags(x, 10, flags::ROAD);
        }
        k.campaign.map = map;

        for id in 1..=2 {
            k.counties[id].population = 500;
            k.counties[id].happiness = 70;
        }
        k.counties[1].owner = 1;
        k.counties[1].anchor_x = 10;
        k.counties[1].anchor_y = 10;
        k.counties[2].anchor_x = 50;
        k.counties[2].anchor_y = 10;
        k.realms[1].in_play = true;
        k.realms[1].is_human = true;
        k
    }

    fn army(k: &mut Kingdom, owner: u8, x: u8, y: u8) -> usize {
        let mut u = Unit::new(UnitKind::Army, owner, x, y);
        u.men = 100;
        u.troops[0] = 100;
        u.county = k.campaign.map.county_at(x, y);
        u.owner_is_human = k.realms[owner as usize].is_human;
        k.campaign.units.spawn(u).expect("a free slot")
    }

    /// A unit walks **one tile per crossing, and a crossing on a road is eight
    /// ticks** — not to exhaustion, and not one tile a tick.
    ///
    /// **This test used to be called `a_unit_enters_one_tile_a_tick` and it
    /// asserted the defect**, in its name, its doc comment and its numbers:
    /// *"fifteen points on a road is fifteen tiles, and it takes fifteen
    /// ticks."* Fifteen tiles is right and fifteen ticks was the whole of
    /// `docs/decisions.md` **C134** — the missing half of
    /// `Unit_StepOnce` (`0x0046634D`), which admits every tick on a road and
    /// needs **eight** admissions to cross a sixteen-wide tile. It is left
/// here because a test that has to be rewritten to
    /// make a fix pass is the strongest evidence that the fix is a change in
    /// behaviour and not a tidy-up.
    ///
    /// The first tile is entered on the tick the order is walked, because
    /// `Unit_Spawn` (`0x0046E1B0`) leaves `+0x14B` bit 0 set, so the arrivals
    /// fall on ticks 1, 9, 17 … and the fifteenth on 113.
    ///
    /// **The 8 is typed here, not read from [`crate::tables::SUBTILE_SPAN`] or
    /// [`crate::tables::SUBTILE_STEP_SOLO`]**, so that ablating either of those
    /// constants cannot move this probe with it — `docs/agents.md`, *ablating a
    /// constant while computing your probe from that same constant tests
    /// nothing at all*.
    #[test]
    fn a_unit_enters_one_tile_every_eight_ticks_on_a_road() {
        let mut k = kingdom();
        let id = army(&mut k, 1, 5, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, id, (20, 10), movement::Routing::Direct)
            .expect("a road runs the whole way");

        let mut ticks = 0usize;
        // (the tick it happened on, the tile it arrived at) — one entry per
// tile entered,
        let mut arrivals: Vec<(usize, u8)> = Vec::new();
        while k.units_moving(UnitKind::Army) && ticks < 1000 {
            let t = k.tick_units();
            ticks += 1;
            assert!(t.stepped <= 1, "one unit, one tile");
            let x = k.campaign.units.get(id).unwrap().x;
            if arrivals.last().map(|&(_, px)| px) != Some(x) {
                arrivals.push((ticks, x));
            }
        }
        assert_eq!(k.campaign.units.get(id).unwrap().x, 20);
        assert_eq!(
            arrivals.iter().map(|&(_, x)| x).collect::<Vec<u8>>(),
            (6..=20).collect::<Vec<u8>>(),
            "fifteen tiles, in order"
        );
        assert_eq!(
            arrivals.iter().map(|&(t, _)| t).collect::<Vec<usize>>(),
            (0..15).map(|n| 1 + n * 8).collect::<Vec<usize>>(),
            "eight ticks a road tile, and the first one free"
        );
        assert_eq!(k.campaign.units.get(id).unwrap().moves_used, 15);
    }

    /// **`Unit_EnterCounty`'s owner test**, as the report carries it: an army
    /// leaving its own county for a neutral one is an incursion, the same army
    /// walking home is not, and a merchant — whose tick never calls
    /// `Unit_EnterCounty` — crossing the same border is not either.
    ///
    /// Ablation: delete the `u.kind == UnitKind::Army` test and the merchant
    /// line goes red; flip `!=` and all three do.
    #[test]
    fn an_army_crossing_into_a_county_its_owner_does_not_hold_is_an_incursion() {
        let mut k = kingdom();
        let out = army(&mut k, 1, 30, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, out, (34, 10), movement::Routing::Direct)
            .unwrap();
        let mut seen = Vec::new();
        for _ in 0..200 {
            seen.extend(k.tick_units().incursions);
        }
        assert_eq!(seen, vec![Incursion { unit: out, owner: 1, county: 2 }]);

        movement::order_move(&k.campaign.map, &mut k.campaign.units, out, (30, 10), movement::Routing::Direct)
            .unwrap();
        let mut home = Vec::new();
        for _ in 0..200 {
            home.extend(k.tick_units().incursions);
        }
        assert_eq!(k.campaign.units.get(out).unwrap().county, 1, "it walked home");
        assert!(home.is_empty(), "county 1 is realm 1's: {home:?}");

        let mut trader = Unit::new(UnitKind::Merchant, 6, 30, 12);
        trader.county = 1;
        let t = k.campaign.units.spawn(trader).unwrap();
        movement::order_move(&k.campaign.map, &mut k.campaign.units, t, (34, 12), movement::Routing::Direct)
            .unwrap();
        let mut carts = Vec::new();
        for _ in 0..400 {
            carts.extend(k.tick_units().incursions);
        }
        assert_eq!(k.campaign.units.get(t).unwrap().county, 2, "the merchant crossed");
        assert!(carts.is_empty(), "only Army_Tick calls Unit_EnterCounty: {carts:?}");
    }

    /// **The sweep reports `Unit_EnterCounty`'s letter at the crossing and
    /// `County_ChangeOwner`'s at the town**, in that order.
    ///
    /// Ablation: delete the `out.posted.push(Posted::Letter(..))` in `step_one`
    /// and the greeting goes; delete the capture's and the second entry does.
    #[test]
    fn crossing_into_a_neutral_county_posts_its_greeting_and_taking_its_town_posts_the_capture() {
        let mut k = kingdom();
        k.counties[2].happiness = 5;
        for (a, b) in [(1usize, 2u8), (2, 1)] {
            k.counties[a].neighbour_count = 1;
            k.counties[a].neighbours[0] = b;
        }
        k.realms[1].peak_counties = 1;
        k.campaign.map.set_flags(40, 10, flags::CASTLE);
        let out = army(&mut k, 1, 30, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, out, (40, 10), movement::Routing::Direct)
            .unwrap();
        let mut posted = Vec::new();
        for _ in 0..400 {
            posted.extend(k.tick_units().posted);
        }
        assert_eq!(posted.len(), 2, "{posted:?}");
        let Posted::Letter(greeting) = posted[0] else { panic!("{posted:?}") };
        assert_eq!((greeting.to, greeting.group, greeting.county), (1, 0x82, 2), "wretched");
        let Posted::Capture(capture) = posted[1] else { panic!("{posted:?}") };
        assert_eq!((capture.new_owner, capture.old_owner, capture.county), (1, 0, 2));
        assert_eq!((capture.held_before, capture.peak_before), (1, 1));
        assert_eq!(k.counties[2].owner, 1, "a wretched county surrenders");
    }

    /// The phase wait is answered by the unit array, and it goes false exactly
    /// when the last unit stops.
    #[test]
    fn the_wait_follows_the_units() {
        let mut k = kingdom();
        assert!(!k.units_moving(UnitKind::Army));
        let id = army(&mut k, 1, 5, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, id, (8, 10), movement::Routing::Direct)
            .unwrap();
        assert!(k.units_moving(UnitKind::Army));
        // **This used to be `for _ in 0..10`**, which was long enough when a
// It is a `while`
        // bigger number on purpose: a fixed count that happens to be large
        // enough asserts nothing about *when* the wait drops, and the count is
        // what rots the next time the pacing moves. 5 → 8 is three road tiles
        // — the first on the tick the order is walked, eight for each of the
        // two after it — **and then eight more to cross the last one.**
        //
        // **This was 17, and 17 was the tick the last tile was *entered*.**
        // `Unit_Step` (`0x00465D28`) does not stop a unit on the commit that
        // empties its path: the commit returns 1, `moving` stays 2, the unit
        // crosses into the tile over the next eight admissions, and only at
        // that tile's edge does the latched arm find `field_0x1c == 0` and
        // write `moving = 0`. Ours stopped it on the commit, which left every
        // finished march parked at `+0x149 = 1` — one whole tile back from
        // where it stood, the moment the walk tables drew it. Typed, not
        // computed: 1 + 8 + 8 + 8.
        let mut ticks = 0usize;
        while k.units_moving(UnitKind::Army) && ticks < 500 {
            k.tick_units();
            ticks += 1;
        }
        assert!(!k.units_moving(UnitKind::Army));
        assert_eq!(ticks, 25, "the wait drops on the tick the last tile is crossed, not the tick it is entered");
        assert_eq!(k.campaign.units.get(id).unwrap().sub_tile, 0, "and the army stands at rest, not a tile back");
        assert_eq!(k.campaign.units.get(id).unwrap().tile(), (8, 10));
    }

/// Two enemy armies meet and the driver reports a battle
    /// resolving one — and the sweep stops there.
    #[test]
    fn two_enemy_armies_meeting_raise_a_battle_and_stop_the_sweep() {
        let mut k = kingdom();
        k.realms[2].in_play = true;
        let a = army(&mut k, 1, 5, 10);
        let _b = army(&mut k, 2, 6, 10);
        let c = army(&mut k, 1, 20, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, a, (8, 10), movement::Routing::Direct)
            .unwrap();
        movement::order_move(&k.campaign.map, &mut k.campaign.units, c, (24, 10), movement::Routing::Direct)
            .unwrap();

        let t = k.tick_units();
        let e = t.battle().expect("a battle is due");
        assert_eq!(e.mover, a);
        assert_eq!(e.occupant, _b);
        assert_eq!(e.county, 1);
        assert_eq!(k.campaign.units.get(a).unwrap().tile(), (5, 10), "and nobody moved onto it");
        assert_eq!(
            k.campaign.units.get(c).unwrap().tile(),
            (20, 10),
            "the higher slot did not move at all: the sweep was abandoned"
        );
    }

    /// Same owner is not a battle. It is not a merge here either — see
    /// `Contact::Blocked`.
    #[test]
    fn a_friendly_unit_blocks_rather_than_fights() {
        let mut k = kingdom();
        let a = army(&mut k, 1, 5, 10);
        let b = army(&mut k, 1, 6, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, a, (8, 10), movement::Routing::Direct)
            .unwrap();
        let t = k.tick_units();
        assert_eq!(t.battle(), None);
        assert_eq!(t.contacts, vec![Contact::Blocked { mover: a, occupant: b }]);
    }

    /// An ally is walked into no more than an enemy is fought.
    #[test]
    fn an_ally_is_not_attacked() {
        let mut k = kingdom();
        k.realms[2].in_play = true;
        k.realms[1].ally = 2;
        let a = army(&mut k, 1, 5, 10);
        let b = army(&mut k, 2, 6, 10);
        movement::order_move(&k.campaign.map, &mut k.campaign.units, a, (8, 10), movement::Routing::Direct)
            .unwrap();
        let t = k.tick_units();
        assert_eq!(t.battle(), None);
        assert_eq!(t.contacts, vec![Contact::Blocked { mover: a, occupant: b }]);
    }

    /// A merchant is never attacked, whoever it belongs to — the second rung of
    /// `Unit_EnterOccupiedTile`'s ladder.
    ///
    /// **Corrected, and the assertion reversed.** This used to expect
/// `Contact::Blocked`, on the reading that a merchant cannot be
    /// *fought*. The rung says `return local_8`, and `local_8` is the ordinary
    /// Road or Open code — so the army walks *through* the merchant and no
    /// contact is reported at all. [`crate::movement::pass_through`] is where
    /// that now happens, which makes rungs 1 and 2 of
/// [`UnitsTick::classify_occupied`] unreachable by construction
    /// by comment. `docs/decisions.md` C40.
    #[test]
    fn a_merchant_is_walked_through_rather_than_attacked() {
        let mut k = kingdom();
        k.realms[2].in_play = true;
        let a = army(&mut k, 1, 5, 10);
        let mut m = Unit::new(UnitKind::Merchant, OWNERLESS, 6, 10);
        m.county = 1;
        k.campaign.units.spawn(m).unwrap();
        movement::order_move(&k.campaign.map, &mut k.campaign.units, a, (8, 10), movement::Routing::Direct)
            .unwrap();
        let t = k.tick_units();
        assert_eq!(t.battle(), None);
        assert_eq!(t.contacts, vec![], "no fight, and no obstacle either");
        assert_eq!(k.campaign.units.get(a).unwrap().tile(), (6, 10), "the army is on the tile");
    }

    /// Phase 2 starts nothing. The whole point of the module's headline.
    #[test]
    fn phase_two_originates_no_movement() {
        let mut k = kingdom();
        let id = army(&mut k, 1, 5, 10);
        assert_eq!(k.begin_unit_phase(Phase::ArmyMovement), 0);
        assert!(!k.campaign.units.get(id).unwrap().moving);
    }

    /// Phase 3 re-targets a transport at its **cargo county's anchor**, every
    /// turn, whether or not it was already going somewhere.
    #[test]
    fn phase_three_sends_every_transport_to_its_cargo_county_anchor() {
        let mut k = kingdom();
        let mut t = Unit::new(UnitKind::Transport, 1, 5, 10);
        t.county = 1;
        t.cargo_county = 2;
        t.needs_destination = false;
        t.dest = Some((9, 9));
        let id = k.campaign.units.spawn(t).unwrap();

        assert_eq!(k.begin_unit_phase(Phase::SupplyTransports), 1);
        let u = k.campaign.units.get(id).unwrap();
        assert_eq!(u.dest, Some((50, 10)), "county 2's anchor, not a tile near it");
        assert_eq!(u.dest_county, 2);
        assert!(u.moving);
    }

    /// **The other end of a shipment.** `Transport_Deliver` (`0x004296B5`):
    /// the transport reaches the destination county's town tile, its cargo
    /// lands in `grain` and `herd`, and the slot is freed.
    ///
    /// Ablation: drop the transport arm in `step_one` and the cart stands on
    /// the town for ever with the food still on it — county 2 gains nothing.
    #[test]
    fn a_transport_reaching_its_cargo_countys_town_unloads_and_is_gone() {
        let mut k = kingdom();
        k.campaign.map.set_flags(40, 10, flags::CASTLE);
        let (grain, herd) = (k.counties[2].grain, k.counties[2].herd);
        let mut t = Unit::new(UnitKind::Transport, 1, 30, 10);
        t.county = 1;
        t.morale = 1;
        t.cargo_county = 2;
        t.needs_destination = false;
        t.troops[0] = 300;
        t.troops[2] = 40;
        t.men = 340;
        let id = k.campaign.units.spawn(t).unwrap();
        movement::order_move(&k.campaign.map, &mut k.campaign.units, id, (40, 10), movement::Routing::Direct)
            .unwrap();

        for _ in 0..400 {
            k.tick_units();
        }
        assert!(k.campaign.units.get(id).is_none(), "the cart is unloaded and gone");
        assert_eq!(k.counties[2].grain, grain + 300);
        assert_eq!(k.counties[2].herd, herd + 40);
        assert_eq!(k.counties[2].owner, 0, "and a transport takes no county");
    }

    /// A transport whose cargo is for somebody else walks over the town and
    /// keeps its load — `Transport_Deliver`'s `destCounty == county` guard.
    #[test]
    fn a_transport_passing_a_town_that_is_not_its_destination_unloads_nothing() {
        let mut k = kingdom();
        k.campaign.map.set_flags(40, 10, flags::CASTLE);
        let grain = k.counties[2].grain;
        let mut t = Unit::new(UnitKind::Transport, 1, 30, 10);
        t.county = 1;
        t.cargo_county = 1;
        t.needs_destination = false;
        t.troops[0] = 300;
        t.men = 300;
        let id = k.campaign.units.spawn(t).unwrap();
        movement::order_move(&k.campaign.map, &mut k.campaign.units, id, (40, 10), movement::Routing::Direct)
            .unwrap();

        for _ in 0..400 {
            k.tick_units();
        }
        assert_eq!(k.campaign.units.get(id).map(|u| u.troops[0]), Some(300));
        assert_eq!(k.counties[2].grain, grain);
    }

    /// Phase 5's cursor is shared, so two mobs are sent to two different
    /// counties — and neither is sent to the county it is standing in.
    #[test]
    fn phase_five_walks_one_cursor_for_every_mob() {
        let mut k = kingdom();
        for (x, county) in [(5u8, 1u8), (50, 2)] {
            let mut m = Unit::new(UnitKind::PeasantMob, OWNERLESS, x, 10);
            m.county = county;
            m.men = 200;
            k.campaign.units.spawn(m).unwrap();
        }
        k.season = MOB_RETARGET_SEASON;
        k.begin_unit_phase(Phase::PeasantMobs);

        let dests: Vec<u8> =
            ids_of_kind(&k.campaign.units, UnitKind::PeasantMob)
                .iter()
                .map(|&id| k.campaign.units.get(id).unwrap().dest_county)
                .collect();
        assert_eq!(dests, vec![2, 1], "the cursor advanced between them");
    }

    /// Only realm 6's mobs hold phase 5 open — `FUN_004A4E3D(2, 6)`.
    #[test]
    fn a_mob_belonging_to_a_realm_does_not_hold_phase_five_open() {
        let mut k = kingdom();
        let mut m = Unit::new(UnitKind::PeasantMob, 1, 5, 10);
        m.county = 1;
        m.moving = true;
        k.campaign.units.spawn(m).unwrap();
        assert!(!k.units_moving(UnitKind::PeasantMob));

        let mut m = Unit::new(UnitKind::PeasantMob, OWNERLESS, 6, 10);
        m.county = 1;
        m.moving = true;
        k.campaign.units.spawn(m).unwrap();
        assert!(k.units_moving(UnitKind::PeasantMob));
    }

    /// A unit that cannot move has its flag cleared, or the phase waiting on it
    /// never ends. This is the failure mode the whole driver has to avoid.
    #[test]
    fn a_unit_with_no_path_stops_being_moving() {
        let mut k = kingdom();
        let id = army(&mut k, 1, 5, 10);
        k.campaign.units.get_mut(id).unwrap().moving = true;
        assert!(k.units_moving(UnitKind::Army));
        k.tick_units();
        assert!(!k.units_moving(UnitKind::Army), "no path is not a reason to wait for ever");
        assert!(k.campaign.units.get(id).unwrap().needs_destination);
    }

    /// The allowance is derived from the type on every tick,
    /// arrives from a save with a zero in it still walks.
    #[test]
    fn the_allowance_is_rebuilt_from_the_type() {
        let mut k = kingdom();
        let id = army(&mut k, 1, 5, 10);
        k.campaign.units.get_mut(id).unwrap().move_allowance = 0;
        refresh_allowances(&mut k.campaign.units);
        assert_eq!(k.campaign.units.get(id).unwrap().move_allowance, 15);
    }
}

