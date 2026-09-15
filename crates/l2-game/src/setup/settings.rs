#![allow(unused_imports)]
use super::*;
use super::tables::*;
use super::setup_options::*;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::unit::TROOP_TYPES;
use crate::game::Game;

impl Settings {
    pub fn kingdom_options(&self) -> l2_kingdom::kingdom::Options {
        l2_kingdom::kingdom::Options {
            difficulty: self.difficulty,
            advanced_farming: self.advanced_farming,
            armies_eat: self.armies_eat,
            fight_humans_only_byte: self.fight_humans_only_byte,
            exploration: self.exploration,
            time_limit: self.time_limit,
            quirks: self.quirks,
        }
    }

    /// **`FUN_0049BD99` over a world that is already built.**
    ///
    /// | `FUN_0049BD99` does | here |
    /// |---|---|
    /// | every county's grain, herd, population and happiness from the county-status row | yes |
    /// | `+100` grain to every unowned county | yes |
    /// | `realm.gold` from the crowns row, `iron`/`wood`/`stone` = 50 | yes |
    /// | `realm.weapons` from the armoury row, `+ difficulty * 20` mail for an AI | yes |
    /// | the start county's `castleType` from the castle row | yes |
    /// | realms past the lord count get `strength = 0` and no county | yes |
    /// | `Army_Create` for the starting garrison | yes — see the arm below |
    /// | seating the realms from the map's player-start table | **no** — the save already seats them |
    pub fn apply_to(&self, game: &mut Game) {
        game.kingdom.options = self.kingdom_options();

        // **`County_Reset` (`0x00451150`)'s tail, and it has to be first.**
        //
        // `Game_NewGame` runs the whole of `County_Reset` before `FUN_0049BD99`,
        // so its `Labour_Allocate` deals at *its own* opening numbers — 150
        // people, 40 head — which the county-status loop below then overwrites
        // without reallocating. `Scenario::from_map` reproduced the numbers and
        // not the six calls that close the loop; this is them.
        for id in game.kingdom.county_ids() {
            game.kingdom.reset_county_for_new_game(id);
        }

        // Which realms are in the game at all. `Realms_AssignLords`
        // (`0x0049C6C1`) hands a lord to at most `ai_lords` non-human realms
        // and writes `strength = 0` into the rest; `FUN_0049BD99` then skips
        // every realm without one, so it gets no county, no gold and no
        // armoury. Ascending by realm id, which is the order the original walks
        // them in — so two peers drop the same realms.
        let mut given = 0;
        let mut dropped = [false; MAX_REALMS];
        for id in 1..MAX_REALMS {
            let realm = &mut game.kingdom.realms[id];
            if !realm.in_play && realm.lord == 0 && !realm.is_human {
                continue;
            }
            if realm.is_human {
                continue;
            }
            if given < self.ai_lords {
                given += 1;
            } else {
                dropped[id] = true;
            }
        }

        for id in 1..MAX_REALMS {
            if dropped[id] {
                let realm = &mut game.kingdom.realms[id];
                realm.strength = 0;
                realm.in_play = false;
                realm.lord = 0;
                realm.county_count = 0;
                realm.gold = 0;
                continue;
            }
            let realm = &mut game.kingdom.realms[id];
            if !realm.in_play && !realm.is_human {
                continue;
            }
            realm.gold = self.gold;
            realm.iron = STARTING_MATERIALS;
            realm.wood = STARTING_MATERIALS;
            realm.stone = STARTING_MATERIALS;
            realm.weapons = self.armoury;
            if !realm.is_human {
                realm.weapons[AI_EXTRA_WEAPON_SLOT] +=
                    self.difficulty as i32 * AI_EXTRA_MAIL_PER_DIFFICULTY;
            }
            realm.wages = 0;
        }

        for id in game.kingdom.county_ids() {
            let owner = game.kingdom.counties[id].owner as usize;
            if owner < MAX_REALMS && dropped[owner] {
                game.kingdom.counties[id].owner = 0;
            }
        }

        for id in game.kingdom.county_ids() {
            let owned = game.kingdom.counties[id].owner != 0;
            let county = &mut game.kingdom.counties[id];
            county.grain = self.county.grain;
            county.herd = self.county.herd;
            county.population = self.county.population;
            county.pop_last = self.county.population;
            county.health_meter = self.county.health_meter;
            county.health_band = l2_kingdom::tables::health_band(self.county.health_meter) as u8;
            county.happiness = self.county.happiness;
            county.happiness_last = self.county.happiness;
            if !owned {
                county.grain += UNOWNED_COUNTY_GRAIN_BONUS;
            } else {
                county.castle_type = self.castle_type;
            }
            // `FUN_0049BD99`'s first loop closes on
            // `County_RecountFields(county); Herd_UpdateCrowding(county);`
            // (`0x00469B8D`, `0x0044D913`) — the row above has just replaced the
            // herd, so the crowding level it is tended at is stale. Without this
            // a county keeps `County_Reset`'s opening 10 into the opening
            // `Herd_SeasonTick` and breeds at the *low*-crowding rate: the
            // person's England county came out at 109 head against the save's
            // 101, and only the AI's counties were right because
            // `Ai_ManageFarmsAll` calls `Herd_UpdateCrowding` for its own.
            //
            // The recount is the binary's line and finds nothing to move on this
            // path — the world came from a save with its fields already counted;
            // ablating it alone leaves the suite green.
            let map = &mut game.kingdom.campaign.map;
            l2_kingdom::field::recount(&mut game.kingdom.counties[id], map);
            l2_kingdom::field::herd_update_crowding(
                &game.kingdom.tables,
                &mut game.kingdom.counties[id],
                map,
            );
        }

        // **The starting garrison** — `FUN_0049BD99`'s `g_startArmySize` arm
        // (`0x0049BF9E`)
        //
        // `FUN_0049BD99` runs the two lines after `Army_Create` unconditionally,
        // so a failed spawn adds a stale `g_lastUnitIndex`' wages; ours runs them
        // only on success. [I]
        if self.garrison.iter().any(|&n| n != 0) {
            for id in 1..MAX_REALMS {
                if dropped[id] {
                    continue;
                }
                let realm = &game.kingdom.realms[id];
                if !realm.in_play && !realm.is_human {
                    continue;
                }
                let Some(county) =
                    game.kingdom.county_ids().find(|&c| game.kingdom.counties[c].owner as usize == id)
                else {
                    continue;
                };
                let mut basket = l2_kingdom::LevyBasket::default();
                for t in 0..TROOP_TYPES {
                    basket.slots[t].chosen = self.garrison[t];
                    basket.slots[l2_kingdom::levy::BASKET_TOTAL].chosen += self.garrison[t];
                }
                let k = &mut game.kingdom;
                k.counties[county].population += basket.total();
                let muster = l2_kingdom::levy::Muster {
                    realm: id as u8,
                    county: county as u8,
                    happiness_cost: 0,
                    year: k.year,
                };
                // `Levy_ConsumeWeapons` debits an armoury that is still zero at
                // this line — `FUN_0049BD99` writes the `g_startArmoury` row
                // *after* the garrison (`0x0049C15B`) — so the garrison's
                // weapons are free. Ours has already written the row, so it is
                // put back. [V]
                let armoury = k.realms[id].weapons;
                let raised = l2_kingdom::levy::create_army(
                    &k.tables,
                    &k.campaign.map,
                    &mut k.counties,
                    &mut k.realms,
                    &mut k.campaign.units,
                    &mut k.campaign.names,
                    &basket,
                    muster,
                    &mut k.campaign.explored,
                );
                k.realms[id].weapons = armoury;
                if let Ok(unit) = raised {
                    k.counties[county].levy_surcharge = 0;
                    k.realms[id].gold += k.campaign.units.get(unit).map_or(0, |u| u.wages);
                }
            }
        }

        // **`FUN_0049BD99`'s two `Labour_Allocate / Ration_Apply /
        // County_RefreshEstimates` rounds, per start county** — after the
        // county-status row has given it people, and with its switches off as
        // the original has them at that line. The only allocation a person's
        // county gets before the opening season. `Kingdom::settle_start_county`.
        for id in game.kingdom.county_ids() {
            if game.kingdom.counties[id].owner != 0 {
                game.kingdom.settle_start_county(id);
            }
        }

        // `Diplo_Init` (`0x004A1C53`), and **it has to be here
        // the world builder**: the opening standing it writes is 5 for an
        // in-play AI realm and 0 for a person or a dropped one, so it has to
        // run *after* the loop above has decided which realms exist. Run it
        // before and every dropped realm would open with an opinion.
        game.kingdom.init_diplomacy();

        for (id, realm) in game.kingdom.realms.iter().enumerate() {
            game.gold_last[id] = realm.gold;
        }
        game.selected = game
            .kingdom
            .county_ids()
            .find(|&id| game.kingdom.counties[id].owner == game.player)
            .unwrap_or(0) as u8;
    }

    /// **What this build cannot honour**, as `L2.eng` group 102 labels.
    ///
    /// `docs/decisions.md` C21: an option wired to nothing must say so where
    /// somebody can see it
    /// grid. **It is empty**: all twelve are honoured.
    ///
    /// The two that used to be here: *Exploration* (`l2_kingdom::explore`,
    /// C172) and *Army Size* (`FUN_0049BD99`'s `Army_Create` arm, in
    /// [`Settings::apply_to`]).
    pub fn unhonoured(&self) -> Vec<usize> {
        Vec::new()
    }
}

