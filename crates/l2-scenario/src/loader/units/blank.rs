#![allow(unused_imports)]
use super::*;
use super::records::*;
use super::restore::*;
use super::build::*;
use super::super::*;
use super::super::helpers::*;
use super::super::*;
use l2_formats::save::{Save, SaveError, COUNTY_BASE, COUNTY_STRIDE, REALM_BASE, REALM_STRIDE};
use l2_kingdom::county::{County, MAX_COUNTY_ID, MAX_FIELDS};
use l2_kingdom::explore::Explored;
use l2_kingdom::map::MAP_TILES;
use l2_kingdom::merchant::{MerchantRoutes, ROUTES, ROUTE_SLOTS};
use l2_kingdom::mercenary::{Band, MercenaryBands, MERCENARY_BANDS, ROSTER};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{health_band, Tables, Weather, JOB_COUNT};
use l2_kingdom::unit::{Mercenaries, TroopType, Unit, UnitKind, Units, MAX_UNITS, TROOP_TYPES};
use l2_kingdom::{field, land, CampaignMap, Kingdom, Options};

impl Scenario {
    pub(crate) fn skeleton(&self, seed: u64, tables: Tables) -> Kingdom {
        let mut k = Kingdom::with_tables(seed, tables);
        k.campaign.map = self.map.clone();
        k.campaign.explored = self.explored.clone();
        k.campaign.routes = self.routes.clone();
        // `Mercenary_Hire` and `Mercenary_AdvanceAll` both index this table, and
        // the county's `+0x1AD` names a slot in it: without it a loaded game has
        // no band in play
        k.campaign.mercenaries = self.mercenaries.clone();
        let mut units = Units::new();
        for (slot, unit) in &self.units {
            if *slot < MAX_UNITS {
                units.put(*slot, unit.clone());
            }
        }
        k.campaign.units = units;
        k.options = self.options;
        k.weather_county = self.weather_county;
        assert!(
            k.set_county_count(self.county_count),
            "from_save bounds the county count before it is stored"
        );

        for (id, r) in self.realms.iter().enumerate().take(MAX_REALMS).skip(1) {
            let RealmState {
                in_play,
                strength,
                is_human,
                lord,
                shield_index,
                county_count,
                peak_counties,
                rank,
                score,
                gold,
                wages,
                iron,
                stone,
                wood,
                weapons,
                pairs,
                army_names,
                ai_step: _,
                tax_hap_empire: _,
                population_total: _,
                population_mean: _,
                population_last: _,
                mean_happiness: _,
                mean_health: _,
                share_of_map_pct: _,
                army_count: _,
                total_men: _,
                castle_count: _,
                offer_pending: _,
                ally_candidate: _,
                ally: _,
                target_county: _,
                taunt_timer: _,
                taunt_stage: _,
                war_target: _,
                offer_timer: _,
                crowned_once: _,
                weapon_rota: _,
                voice_rotation: _,
                muster_county: _,
                raid_county: _,
                muster_timer: _,
                threat_realm: _,
                attack_county: _,
                raid_timer: _,
                want: _,
                bankrupt_stage: _,
                trade_spent_a: _,
                trade_spent_b: _,
                trade_received_a: _,
                trade_received_b: _,
                tax_ledger: _,
            } = r;
            let realm = &mut k.realms[id];
            realm.in_play = *in_play;
            realm.strength = *strength;
            realm.is_human = *is_human || id == self.local_player as usize;
            realm.lord = *lord;
            // `Game_SetupRealms` (`0x0049C5xx`) initialises every realm with
            // `g_realms[i].shieldIndex = i`, and only a custom game's colour
            // picker permutes it (`0x0049CE1F` walks a free-slot pool), which is
            // why a default game read either way comes out the same. Taken from
// the save's own byte at realm `+0x0A`,
            // custom game's flags are its own colours and not the realm order.
            realm.shield_index = *shield_index;
            realm.county_count = *county_count;
            realm.peak_counties = *peak_counties;
            realm.rank = *rank;
            realm.score = *score;
            realm.gold = *gold;
            realm.wages = *wages;
            realm.iron = *iron;
            realm.stone = *stone;
            realm.wood = *wood;
            realm.weapons = *weapons;
            // **The diplomatic matrix, **
            // `scenario::from_save` ran `Diplo_Init` because nothing read
            // these bytes; a mid-game save came back with every alliance and
            // every grudge gone. `docs/decisions.md` C83.
            realm.pairs = *pairs;
            k.campaign.names.set_counters(id as u8, *army_names);
        }

        for id in self.county_ids() {
            let Some(s) = &self.counties[id] else { continue };
            let c = &mut k.counties[id];
            *c = County::new();
            c.owner = s.owner;
            c.anchor_x = s.anchor.0;
            c.anchor_y = s.anchor.1;
            for &n in &s.neighbours {
                c.add_neighbour(n);
            }
            c.tax_rate = s.tax_rate;
            c.ration_wanted = s.ration_wanted;
            c.ration_split = s.ration_split;
            c.castle_type = s.castle_type;
            c.castle_building = s.castle_building;
            c.castle_switch = s.castle_switch;
            // **The three bytes that say whether an industry runs.** They were
            // not imported at all until C57, so every county arrived claiming
// all four resources and all four switches on — so no
            // county ever showed a mine in its village and why the map's
            // industry toggles all started in the wrong position. Everything
            // else in the record (`output`, `efficiency`, `capacity`,
            // `total`) is still `County::new()`'s.
            //
            // **And the forecast the row draws**,
            // held at zero — so every industry row was blank until the first
            // season ended. C153, and [`IndustryState::next_season`].
            for (slot, s) in s.industry.iter().enumerate() {
                c.industry[slot].has_resource = s.has_resource;
                c.industry[slot].disabled_seasons = s.disabled_seasons;
                c.industry[slot].enabled = s.enabled;
                c.industry[slot].next_season = s.next_season;
            }
            c.field_tiles = s.field_tiles;
            c.fertility = s.fertility;
            c.weather = s.weather;
            c.dryness = s.dryness;
            c.grain = s.grain;
            c.herd = s.herd;
            c.labour = s.labour;
            c.labour_wanted = s.labour_wanted;
            c.labour_useful = s.labour_useful;
            c.labour_share = s.labour_share;
            c.industry_share = s.industry_share;
            // `+0x1FE`. Not carried at all until now — see [`FARM_STYLE`].
            c.farm_style = s.farm_style;
            // `FUN_0044D913` is called from everywhere a county's herd or
            // pasture can change, county setup included,
            // arrives with its crowding already computed. Deriving it here
            field::recount(c, &self.map);
            c.herd_crowding = land::herd_crowding(&tables, c.herd, c.fields_cattle);
        }

        // The original keeps both halves — county `+0x1BC` names the unit and
        // unit `+0x198` names the county (`docs/armies.md` §2, both **[V]**) —
        // and `Castle_Garrison` writes them together. We import only the unit's
        // half, as [`Unit::garrison_county`], while **every** consumer reads the
        // county's: `conquest`'s ownership test, `divide`, `siege`'s
        // still-inside check, and `Map_DrawFrame`'s castle flag, which is gated
        // on `county.garrisonUnit != 0`. `County::new` seeds it to 0 and no
        // import path overwrote it,
        // garrisoned anywhere** — including the five siege fixtures that exist
        // for exactly that position,
        // symptom that exposed it was the castle flag never drawing; the
        // consequences in `conquest` and `siege` were the same bug and had not
        // been noticed. C59.
        for (slot, unit) in &self.units {
            let county = unit.garrison_county as usize;
            if county == 0 || *slot >= MAX_UNITS {
                continue;
            }
            if let Some(c) = k.counties.get_mut(county) {
                if c.garrison_unit == 0 {
                    c.garrison_unit = *slot;
                }
            }
        }
        k
    }
}



