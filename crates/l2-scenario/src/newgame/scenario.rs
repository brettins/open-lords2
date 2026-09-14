#![allow(unused_imports)]
use super::*;
use super::build_part::*;
use super::sites::*;
use super::fields::*;
use super::setup::*;
use super::tests_part::*;
use l2_formats::maps::{MapSlot, Plane, PLANE_DIM};
use l2_kingdom::county::{MAX_COUNTIES, MAX_COUNTY_ID, MAX_FIELDS, MAX_NEIGHBOURS};
use l2_kingdom::map::{CampaignMap, MAP_TILES};
use l2_kingdom::merchant::{self, MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::MercenaryBands;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{Weather, JOB_COUNT};
use l2_kingdom::unit::{Unit, UnitKind, Units};
use l2_kingdom::Options;
use crate::{Clock, CountyState, IndustryState, RealmState, Scenario};

impl Scenario {
    /// **A world built from an `L2_maps.dat` slot.**
    ///
    /// The second constructor, and the one the *New Game* button needs.
    /// Everything it produces is the same plain data
    /// [`Scenario::from_save`] produces, so
    /// [`Scenario::starting_kingdom`] takes either without knowing which — and
    /// `crates/l2-scenario/tests/newgame/main.rs` builds England both ways and
    /// diffs them field by field.
    ///
    /// The clock is `Game_NewGame`'s: **Autumn 1267, with Winter next.**
    /// `Kingdom::start_new_game` then runs the one immediate `Season_Advance`
    /// that puts a new game in Winter 1268, so this is deliberately the
/// position *before* it,
    /// save.
    pub fn from_map(slot: &MapSlot<'_>, setup: &NewGame) -> Result<Scenario, MapError> {
        let world = build(slot, setup)?;
        Scenario::from_map_world(&world, setup)
    }

    /// The same, from an already-built [`MapWorld`] — for a caller that also
    /// wants the town tiles, the dwelling plots or the runtime frame plane,
    /// none of which fits on a [`Scenario`].
    pub fn from_map_world(w: &MapWorld, setup: &NewGame) -> Result<Scenario, MapError> {
        if setup.local_player < 1 || setup.local_player as usize >= MAX_REALMS {
            return Err(MapError::LocalPlayer(setup.local_player));
        }
        let lords = setup.lords.clamp(1, MAX_REALMS - 1);
        if setup.local_player as usize > lords {
            return Err(MapError::LocalPlayer(setup.local_player));
        }
        if !(1..=5).contains(&setup.shield) {
            return Err(MapError::Shield(setup.shield));
        }
        let seats = start_counties(w, lords, setup.seed)?;
        let assigned = assign_lords(setup, lords);

        let mut counties: Vec<Option<CountyState>> = vec![None; MAX_COUNTIES];
        for id in 1..=w.county_count {
            let mut c = county_reset(id);
            c.anchor = w.anchor[id];
            c.neighbours = w.neighbours[id].clone();
            c.field_tiles = w.field_tiles[id];
            for (record, slot) in c.industry.iter_mut().enumerate() {
                slot.has_resource = w.has_resource[id][record];
            }
            counties[id] = Some(c);
        }

        // `Game_SetupRealmsAndCounties`' seating half. The economy half — the
        // county-status row, the crowns, the armoury, the castle level — is
        // `l2_game::setup::Settings::apply_to`, which already runs on the save
        // path and now runs on this one too.
        let mut realms = vec![RealmState::default(); MAX_REALMS];
        for id in 1..MAX_REALMS {
            let realm = &mut realms[id];
            // **The walk's shield where it gave one, and `FUN_0049C995`'s seed
            // where it did not.** That seed — `shieldIndex = i` for realms
            // 1 … 5, run when the lobby is reset — is what a realm above the
            // lord count keeps, because `Realms_AssignLords` skips it. It is
            // the *default* and not the rule; reading it as the rule is what
            // this whole change is undoing, so it is written where it applies
            // and nowhere else.
            realm.shield_index =
                if assigned.shield[id] != 0 { assigned.shield[id] } else { id as u8 };
            if id > lords {
                // `Realms_AssignLords` writes `strength = 0` and hands out no
                // lord; `Game_SetupRealmsAndCounties` then skips the realm
                // entirely, so it gets no county and no gold.
                continue;
            }
            realm.in_play = true;
            realm.strength = 1;
            realm.is_human = id == setup.local_player as usize;
            // The walk already leaves a human at 0 — `g_realms[i].lord = 0` is
            // the first statement of every iteration — so this is the walk's
            // answer and not a second rule beside it.
            realm.lord = assigned.lord[id];
            realm.county_count = 1;
            // `g_realms[i].field_0x2a = 1` beside it (`0x00490000.c` setup walk,
            // `Game_SetupRealmsAndCounties`): the most counties ever held.
            realm.peak_counties = 1;
            let Some(&county) = seats.get(id - 1) else { continue };
            let Some(c) = counties.get_mut(county as usize).and_then(|c| c.as_mut()) else {
                continue;
            };
            c.owner = id as u8;
            // **The one industry a start county opens with switched on**, and
            // it is not the blacksmith: the loop skips record 2 and takes the
            // first of wood, iron, stone the county has a resource for.
            if let Some(slot) =
                c.industry.iter_mut().enumerate().find(|(r, s)| *r != IND_WEAPONS && s.has_resource)
            {
                slot.1.enabled = true;
            }
            // County `+0x1B0` — the castle-building switch, on from the first
            // turn in every start county.
            c.castle_switch = true;
        }

        let map = w.tiles.campaign_map();
        let units = spawn_merchants(w, &map);

        // **`Game_SetupRealmsAndCounties`' last statement** (`0x0049BD99`):
        // `FUN_0046DFD5(g_playerStartTable[g_localPlayer * 2])` — the local
        // player's start county, and a one-tile border, is seen. Every other
        // tile was cleared by `Map_InitScenario` (`FUN_0046DF51`), which is
        // `Explored::new`. Every seated realm gets its own, for the reason
        // `l2_kingdom::explore` gives; only the person's is ever drawn.
        //
        // **The garrison's square is not here, and it is not missing from
        // this line.** The original's `Army_Create` for the starting garrison
        // reveals thirteen by thirteen round it, and the England fixture's
        // bits carry it; this build does not raise that garrison yet
        // (`l2_game::setup::Settings::unhonoured`), and the day it does it
        // will go through `levy::create_army`, which reveals.
        let mut explored = l2_kingdom::explore::Explored::new();
        for id in 1..=w.county_count {
            if let Some(c) = counties.get(id).and_then(|c| c.as_ref()) {
                if c.owner != 0 {
                    explored.reveal_county(c.owner, &map, id as u8);
                }
            }
        }

        Ok(Scenario {
            county_count: w.county_count,
            local_player: setup.local_player,
            // `County_Reset` opens `g_weatherCounty` on 1.
            weather_county: 1,
            options: setup.options,
            clock: Clock { season: 3, season_next: 4, year: 1267, turn_count: 0 },
            counties,
            realms,
            map,
            // `Mercenary_Init` (`0x004AC904`), which `Game_NewGame` calls
            // straight after `Merchant_SpawnAll`. Nothing on the new-game path
            // ran it,
            mercenaries: MercenaryBands::init(w.county_count),
            explored,
            units,
            routes: w.routes.clone(),
            merchant_start: w.merchant_start,
        })
    }
}

