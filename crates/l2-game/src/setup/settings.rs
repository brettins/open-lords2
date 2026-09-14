#![allow(unused_imports)]
use super::*;
use super::tables::*;
use super::setup_options::*;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::unit::TROOP_TYPES;
use crate::game::Game;

impl Settings {
    /// The six rule flags, as `l2-kingdom` wants them. This half is complete:
    /// every one of the six reaches a rule, or is carried in the save because
    /// nothing may read it yet and a save that forgot it would be a save that
    /// guessed.
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
    /// The original's version runs on a map it has just loaded, so it also
    /// seats the realms — `g_playerStartTable` decides who gets which county —
    /// and raises each realm's garrison with `Army_Create`. This one runs on a
    /// world [`crate::scenario`] built from a save, which already has its
    /// counties owned, so it does the part that is the *options'*: the stores,
    /// the treasury, the armoury, the castle, and how many lords are in play.
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

        // A county whose owner has just been dropped is nobody's.
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
        }

        // **The starting garrison** — `FUN_0049BD99`'s `g_startArmySize` arm
        // (`0x0049BF9E`), between the county-status row and the two food rounds:
        //
        // ```c
        // if (g_startArmySize != 0) {
        //     basket[7].chosen = 0;
        //     for (t = 0; t < 7; t++) {
        //         basket[t].chosen  = g_startTroops[row][t];
        //         basket[7].chosen += g_startTroops[row][t];
        //         county.population += g_startTroops[row][t];   /* pre-credit */
        //     }
        //     Army_Create(realm, county, 0, 0);
        //     county.levySurcharge = 0;
        //     realm.gold += g_units[g_lastUnitIndex].wages;
        // }
        // ```
        //
        // Row 0 of `g_startTroops` is all zeroes — *no army* raises nothing. The
        // pre-credit cancels `Levy_DebitPopulation` exactly
        // the county no people; happiness costs 0, no mercenaries
        // surcharge `Army_Create` writes is undone. `l2_kingdom::levy::create_army`
        // is the levy screen's own path, entered here with a basket built from
        // the table instead of from the slider.
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
        //
        // This is the whole answer to *"what writes `Pair::standing` in a real
        // game?"* — `docs/agents.md`'s rule that a field is only tested if
        // something a test reads was written by something the game runs.
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

