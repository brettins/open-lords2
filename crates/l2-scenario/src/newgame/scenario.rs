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
    pub fn from_map(slot: &MapSlot<'_>, setup: &NewGame) -> Result<Scenario, MapError> {
        let world = build(slot, setup)?;
        Scenario::from_map_world(&world, setup)
    }

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
            // `Realms_AssignLords` (`0x0049CAAA`) closes on a loop over realms
            // 1 … 5 that runs whether or not the realm is seated
            // (`00490000.c:5800-5801`): `+0x2A = 1; +0x29 = +0x2A;`. So a realm
            // above the lord count carries `countyCount = 1` and owns nothing,
            // and `Realm_UpdateTotals` (`0x0049D1E0`) never recounts it — it is
            // called only for an in-play realm. `fought-old_turn.sav` realms
            // 3 … 5: `strength 0`, `countyCount 1`, `peakCounties 1`. [V]
            realm.county_count = 1;
            realm.peak_counties = 1;
            if id > lords {
                continue;
            }
            realm.in_play = true;
            realm.strength = 1;
            realm.is_human = id == setup.local_player as usize || setup.humans[id] != 0;
            realm.lord = assigned.lord[id];
            let Some(&county) = seats.get(id - 1) else { continue };
            let Some(c) = counties.get_mut(county as usize).and_then(|c| c.as_mut()) else {
                continue;
            };
            c.owner = id as u8;
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
        //
        // `FUN_0046DFD5(g_playerStartTable[g_localPlayer * 2])` — the local
        // player's start county, and a one-tile border, is seen. Every other
        // tile was cleared by `Map_InitScenario` (`FUN_0046DF51`), which is
        // `Explored::new`. Every seated realm gets its own, for the reason
        // `l2_kingdom::explore` gives; only the person's is ever drawn.
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

