#![allow(unused_imports)]
use super::*;

use super::*;
use crate::ai;
use crate::county::{County, MAX_COUNTIES, MAX_COUNTY_ID};
use crate::event;
use crate::happiness;
use crate::health;
use crate::industry;
use crate::land;
use crate::phase::{Pass, Phase, PhaseTick, TurnMachine, SEASON_PIPELINE};
use crate::population;
use crate::ration;
use crate::realm::{Realm, MAX_REALMS};
use crate::report::{Message, SeasonReport};
use crate::tables::{Commodity, Season, Tables};
use crate::tax;
use crate::unrest;
use crate::weather;
use l2_net::Pcg32;

impl Kingdom {
    /// **The wanted ration level — the third control on the ration panel, and
    /// the third with the same omission.** `Ration_IncreaseCounty`
    /// (`0x0043A23F`) and its twin:
    ///
    /// ```c
    /// if (rationWanted < 5) rationWanted++;      /* the cap is the table's length */
    /// Ration_Apply(county, g_season);            /* ONCE, not twice */
    /// County_RefreshEstimates(county, g_seasonNext);
    /// Panel_Ration();
    /// ```
    ///
    /// **One pass each, where `Ration_SetSplit` runs two. Do not tidy this into
    /// symmetry.** It is the original's asymmetry, it is deliberate
    /// reason is legible: the split's search walks the value up to a hundred
    /// times and can leave the county's labour describing a split it then
    /// walked away from, so that control re-runs the pair to settle it. A level
    /// change moves once and has nothing to settle.
    ///
/// Written here because the next reader
    /// of these two functions will see `for _ in 0..2` beside a bare call and
    /// reach for the loop.
    ///
    /// The recompute is unconditional — the guard is only on the increment — so
    /// a click at the cap still re-applies and repaints. Ours wrote
    /// `ration_wanted` and returned, exactly like the other two, and this one
/// was found by *enumerating the class*
    /// it: `docs/agents.md`, **the correction that identifies a class must
    /// enumerate the class**.
    ///
    /// It writes `rationWanted` (`+0x15E`) and never `rationAchieved`
/// (`+0x15D`): what the player asks for and what the stores could
    /// feed are different fields, and only the pass decides the second.
    ///
    /// Returns whether the level moved.
    pub fn set_ration_wanted(&mut self, county: usize, level: i32) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let level = level.clamp(0, crate::tables::RATION_LEVEL_COUNT as i32 - 1);
        let moved = self.counties[county].ration_wanted != level;
        self.counties[county].ration_wanted = level;
        let armies_eat = self.options.armies_eat;
        // `Ration_Apply` computes and records; it does not spend. The spending
        // twin is `crate::ration::apply`, whose name matches the original's and
        // whose behaviour does not — see `set_ration_split`.
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);
        self.refresh_estimates(county);
        moved
    }

/// **`Opt_ToggleArmyForaging` (`0x004345D0`).**
    ///
    /// ```c
    /// g_optArmiesEat = (g_optArmiesEat != 1);
    /// for (i = 1; i <= g_countyCount; i++) {
    ///     Ration_Apply(i, g_season);
    ///     County_RefreshEstimates(i, g_seasonNext);
    /// }
    /// ```
    ///
    /// The switch changes who a county feeds — [`crate::ration::people_to_feed`]
    /// adds the armies standing in it — so the original re-runs the ration
    /// pass and the forecasts over every county *on the flip*
    /// panel is right the moment the options panel closes. Ours flipped the
    /// flag and left every county's ration fields describing the old rule until
    /// the next season. `Ration_Apply` is [`crate::ration::preview`] here for the
    /// reason [`Kingdom::set_ration_wanted`] gives: it records and does not
    /// spend. `[V]` against the decompilation.
    ///
    /// The multiplayer branch — `Net_SendCommand(0x32, 0)` instead of the flip —
    /// is not here: `docs/netcode.md`, the original is not the authority there.
    pub fn toggle_army_foraging(&mut self) {
        self.options.armies_eat = !self.options.armies_eat;
        let armies_eat = self.options.armies_eat;
        for id in 1..=self.county_count {
            crate::ration::preview(&self.tables, &mut self.counties[id], armies_eat);
            self.refresh_estimates(id);
        }
    }

/// **The tax rate.**
    /// `Tax_IncreaseCounty` (`0x0043AA83`) and its twin, whole:
    ///
    /// ```c
    /// if (taxRate < 0x32) taxRate++;      /* 50 is the player's ceiling */
    /// Tax_RecomputePreview(county);       /* taxShown, both happiness terms */
    /// Panel_Tax();                        /* repaint */
    /// ```
    ///
    /// and `Tax_RecomputePreview` itself ends with `Tax_SumEmpireHappiness(owner)`
    /// and `FUN_0044BA35`, the empire-wide sum of `taxShown` that the court
    /// prints. **Every tax control in the original recomputes and repaints**,
    /// exactly like the ration slider — and this is the second panel found with
/// the same omission, which is the finding.
    ///
    /// Ours wrote `taxRate` and stopped, so `tax_shown` kept whatever the last
    /// season's [`crate::tax::collect`] left in it (zero, before the first
    /// collection) and both happiness terms went stale the moment the rate
    /// moved. A player reported both halves in one sentence.
    ///
    /// **Watch which term is expected to move.** `d_hap_tax_local` is `5 - rate`
    /// and moves on every click; `tax_hap_other` is `g_taxHappinessOther[rate]`,
    /// which is **flat zero from rate 0 to 19**, so the *Other counties* line
/// does not budge over most of the range a player uses. That is
    /// the panel being right, and `docs/rules.md` says so.
    ///
    /// Returns whether the rate moved.
    pub fn set_tax_rate(&mut self, county: usize, rate: i32) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let rate = rate.clamp(0, crate::tables::MAX_TAX_RATE);
        let moved = self.counties[county].tax_rate != rate;
        self.counties[county].tax_rate = rate;
        crate::tax::recompute_preview(&self.tables, &mut self.counties[county]);
        // `Tax_SumEmpireHappiness(owner)` — the realm's own term is a sum over
        // its counties, so one county's rate moves every county's *This county*
// line. Recomputed here for the same
        // reason the rest of this function exists.
        let quirks = self.options.quirks;
        crate::tax::sum_empire_happiness(
            &self.tables,
            &mut self.counties,
            &mut self.realms,
            self.county_count,
            quirks,
        );
        moved
    }

/// **The grain-to-livestock split.**
    /// `Ration_SetSplit` (`0x0043A5A9`), whole.
    ///
    /// A player reported the ration panel's slider as *"moves but is
    /// inoperable"*. It was writing the field and stopping, and every number on
    /// the panel stayed where it was until the turn ended — so the thumb
    /// travelled and nothing else did. The original does four more things, and
    /// three of them are visible:
    ///
    /// ```c
    /// old = rationSplit;  dir = sign(split - old);
    /// rationSplit = split;
    /// was = herdEaten;
    /// Ration_Apply(county, g_season);                       /* the food pass, NOW */
    /// if (herd && herdEaten && dir && split != 0 && split != 100 && herdEaten == was) {
    ///     rationSplit = old;                                /* ... the search ... */
    ///     do {
    ///         if (++n > 100) goto done;
    ///         rationSplit = clamp(rationSplit + dir, 0, 100);
    ///         Ration_Apply(county, g_season);
    ///         if (herdEaten != was) goto done;
    ///     } while (rationSplit != split || sweep == 0);
    ///     rationSplit = old; Ration_Apply(county, g_season); /* give up: spring back */
    /// }
    /// done:
    /// Labour_Allocate(county); County_RefreshEstimates(county, g_seasonNext);   /* twice */
    /// Labour_Allocate(county); County_RefreshEstimates(county, g_seasonNext);
    /// if (g_selectedCounty == county) Panel_Ration();
    /// ```
    ///
    /// **The slider refuses to sit on a value that changes nothing.** If the
    /// county has a herd, the herd is being eaten, the value moved, the
    /// *requested* split is strictly inside 0…100, and `herdEaten` came out
    /// unchanged, it puts the old value back and walks one point at a time
    /// towards the request, re-running the food pass at every step, and stops at
/// the first split that moves `herdEaten`.
    ///
    /// **`sweep` is what tells a track jump from an arrow**, and it changes the
    /// ending. `Ration_SliderClick` passes 1 for a click on the track and 0 for
    /// an arrow:
    ///
    /// * **track jump** (`sweep`): the loop stops when it reaches the requested
    ///   value, and if nothing changed on the way the split is **restored** —
    ///   the thumb springs back to where it was.
    /// * **arrow step** (`!sweep`): `(rationSplit != split) || (sweep == 0)` is
    ///   *always* true, so the walk does not stop at the requested value. It
    ///   keeps going in the same direction until `herdEaten` moves or a hundred
    ///   steps are spent — so **one click of an arrow can move the split by far
    ///   more than one**, and it does not spring back.
    ///
    /// **`Ration_Apply` does not spend.** It writes `rationAchieved`,
    /// `herdEaten`, `grainEaten`, the two `available` fields and the happiness
    /// delta
    /// to a hundred times is [`crate::ration::preview`] and **not**
    /// [`crate::ration::apply`], whose name matches the original's and whose
    /// behaviour does not — reaching for the same-named function would have had
    /// a drag eat the county's herd a hundred times over.
    ///
    /// Returns whether the split ended anywhere other than where it started,
    /// which is what a caller needs to know to decide whether to repaint.
    pub fn set_ration_split(&mut self, county: usize, split: i32, sweep: bool) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let split = split.clamp(0, crate::county::MAX_RATION_SPLIT);
        let armies_eat = self.options.armies_eat;
        let old = self.counties[county].ration_split;
        let dir = match split.cmp(&old) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Greater => 1,
            std::cmp::Ordering::Equal => 0,
        };
        let was = self.counties[county].herd_eaten;

        {
            let c = &mut self.counties[county];
            c.ration_split = split;
        }
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);

        let stuck = {
            let c = &self.counties[county];
            c.herd != 0
                && c.herd_eaten != 0
                && dir != 0
                && split != 0
                && split != crate::county::MAX_RATION_SPLIT
                && c.herd_eaten == was
        };
        if stuck {
            self.counties[county].ration_split = old;
            let mut steps = 0;
            loop {
                steps += 1;
                if steps > 100 {
                    break;
                }
                {
                    let c = &mut self.counties[county];
                    c.ration_split = (c.ration_split + dir).clamp(0, crate::county::MAX_RATION_SPLIT);
                }
                crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);
                if self.counties[county].herd_eaten != was {
                    break;
                }
                // The `do … while` condition
                // between the two gestures.
                if self.counties[county].ration_split == split && sweep {
                    self.counties[county].ration_split = old;
                    crate::ration::preview(
                        &self.tables,
                        &mut self.counties[county],
                        armies_eat,
                    );
                    break;
                }
            }
        }

        for _ in 0..2 {
            crate::labour::allocate(&mut self.counties[county]);
            self.refresh_estimates(county);
        }
        self.counties[county].ration_split != old
    }

    /// The map tiles that are one county's fields, with what each is being
    /// used for — what a screen needs to draw the brush's targets.
    pub fn field_tiles(&self, county: usize) -> Vec<(usize, crate::field::FieldType)> {
        let Some(c) = self.counties.get(county) else { return Vec::new() };
        (0..crate::county::MAX_FIELDS)
            .filter_map(|slot| c.field_tile(slot))
            .map(|tile| (tile, crate::field::classify(self.campaign.map.terrain[tile])))
            .collect()
    }

    pub fn run_ai_tax_rates(&mut self, realm: u8) {
        let Some(r) = self.realms.get(realm as usize) else { return };
        let lord = r.lord;
        ai::set_tax_rates(&self.tables, &mut self.counties, self.county_count, realm, lord);
    }

    /// `AI_SetTaxRates`' resource grants, which run in the AI's turn rather
    /// than in `Season_Advance`. Exposed separately for that reason.
    pub fn run_ai_grants(&mut self) {
        ai::grant_resources(
            &self.tables,
            &mut self.counties,
            &mut self.realms,
            self.county_count,
            self.options.difficulty,
        );
    }

    /// What the AI's farming passes read besides the counties and the map.
    fn farm_env(&self) -> crate::ai_farm::FarmEnv {
        crate::ai_farm::FarmEnv {
            season: self.season().unwrap_or(Season::Spring),
            season_next: Season::from_index(self.season_next).unwrap_or(Season::Spring),
            advanced_farming: self.options.advanced_farming,
            armies_eat: self.options.armies_eat,
        }
    }

    /// AI step 5 — `Ai_ManageCountyFarms`. Orders fields and runs the lord's
    /// farming style over every county the realm holds.
    ///
    /// `market` is the merchant seam. **The game's own callers pass the stall**
    /// — [`Kingdom::run_ai_farms_at_the_stall`] — and this form stays for a
    /// hand-built kingdom with no merchant model, which passes
    /// [`crate::ai_farm::NoMarket`] and still gets every style's layout,
    /// rations and labour split.
    pub fn run_ai_farms(&mut self, realm: u8, market: &mut dyn crate::ai_farm::Market) -> i32 {
        let Some(r) = self.realms.get(realm as usize) else { return 0 };
        let lord = r.lord;
        let env = self.farm_env();
        crate::ai_farm::manage_county_farms(
            &self.tables,
            &mut self.counties,
            self.county_count,
            &mut self.campaign.map,
            &self.realms,
            realm,
            lord,
            market,
            &env,
        )
    }

    /// Turn phase 1, step 2 — `AI_ManageFields(0)`, the unowned counties.
    pub fn run_neutral_farms(&mut self, market: &mut dyn crate::ai_farm::Market) -> i32 {
        let env = self.farm_env();
        crate::ai_farm::manage_neutral_fields(
            &self.tables,
            &mut self.counties,
            self.county_count,
            &mut self.campaign.map,
            &self.realms,
            market,
            &env,
        )
    }

    /// Turn phase 1, step 2, **with the county's own merchant stall attached**
    /// — which is what the original runs and what makes a lordless county able
    /// to feed itself.
    ///
    /// [`Kingdom::run_neutral_farms`] takes any [`crate::ai_farm::Market`] and
    /// stays the seam; this is the one call that supplies the real one
    /// from `County::merchant_count` / `merchant_unit` and the unit array
/// as `Ai_BuyGood` reads them. Returns the number of fields
    /// ordered, as [`Kingdom::run_neutral_farms`] does.
    pub fn run_neutral_farms_at_the_stall(&mut self) -> i32 {
        let env = self.farm_env();
        // The stall holds the realms mutably; the pass reads a copy. Exact for
        // the reason given at [`Kingdom::run_ai_farms_at_the_stall`], and here
        // trivially so: an unowned county's trade never touches a realm.
        let view = self.realms.clone();
        let mut market = crate::ai_farm::CountyStall::new(
            &self.tables,
            &self.counties,
            &self.campaign.units,
            &mut self.realms,
            env.season_next,
            env.armies_eat,
        );
        crate::ai_farm::manage_neutral_fields(
            &self.tables,
            &mut self.counties,
            self.county_count,
            &mut self.campaign.map,
            &view,
            &mut market,
            &env,
        )
    }

    /// AI step 5, **with the county's own merchant stall attached** —
    /// `Ai_ManageCountyFarms` (`0x0049DD01`) as the original runs it, where
    /// each realm style's opening `Ai_BuyGood` lines (`0x004A4B12`) pay out of
    /// the owning realm's treasury.
    ///
    /// Returns the number of fields ordered, as [`Kingdom::run_ai_farms`] does.
    ///
    /// # Why the pass may read a copy of the realms
    ///
    /// The stall needs the realm array mutably, to test and debit the treasury
    /// and book the spend; the pass needs it immutably, because
    /// `County_RefreshEstimates` reads the owner's record for the blacksmith's
    /// ceiling. The pass is handed a copy taken before it starts
/// is **exact**: a grain or cattle purchase
    /// writes `gold`, `trade_spent_a` and `trade_spent_b` and nothing else on
    /// the realm, and no estimate reads any of the three (`crate::field` and
    /// `crate::industry`'s estimates read no treasury — the one `gold` in
    /// `crate::industry` is `Wages_PayAll`'s, which is a season pass).
    /// `Ai_TradeForCounty`, which *would* move wood and iron under the
    /// estimates, is not implemented; if it arrives, this reasoning has to be
/// redone.
    pub fn run_ai_farms_at_the_stall(&mut self, realm: u8) -> i32 {
        let Some(r) = self.realms.get(realm as usize) else { return 0 };
        let lord = r.lord;
        let env = self.farm_env();
        let view = self.realms.clone();
        let mut market = crate::ai_farm::CountyStall::new(
            &self.tables,
            &self.counties,
            &self.campaign.units,
            &mut self.realms,
            env.season_next,
            env.armies_eat,
        );
        crate::ai_farm::manage_county_farms(
            &self.tables,
            &mut self.counties,
            self.county_count,
            &mut self.campaign.map,
            &view,
            realm,
            lord,
            &mut market,
            &env,
        )
    }

    /// **`Ai_ManageFarmsAll` (`0x0049A990`)** — the first thing
    /// `Season_Advance` (`0x00448440`) does, ahead of `Rand_Advance` and the
    /// clock.
    ///
    /// ```c
    /// for (r = 1; r < 6; r++)
    ///     if (g_realms[r].strength != 0 && g_realms[r].isHuman == 0)
    ///         Ai_ManageCountyFarms(r);
    /// ```
    ///
    /// So an AI lord's counties are farmed **twice** a turn — once by his own
    /// step 5 in phase 4, and once here at the start of phase 7 — and the
    /// second time is ahead of tax, rations and industry, which then read the
    /// fields, the labour split and the larder it leaves. It shops at the
    /// stall like step 5 does. `g_season` is still the season that is ending
    /// when it runs, so the Winter re-sow fires here on the Winter turn.
    ///
    /// **The class is four callers of `Ai_ManageCountyFarms`, and three are
    /// reproduced.** AI step 5 and this are two. `FUN_0049DF48` is a
    /// byte-for-byte twin of this loop (tests in the other order) whose only
    /// caller is the tail of `Battle_ReturnToCampaign` (`0x004AB383`):
    /// `FUN_004AD426(); Panels_RefreshAll(); FUN_0049DF48();`. That tail is not
    /// in `crate::battle::return_to_campaign`, so an AI's farms are **not**
    /// re-managed after a battle here. Open, not cleared.
    pub fn ai_manage_farms_all(&mut self) -> i32 {
        let mut ordered = 0;
        for id in 1..MAX_REALMS {
            let realm = &self.realms[id];
            if realm.strength != 0 && !realm.is_human {
                ordered += self.run_ai_farms_at_the_stall(id as u8);
            }
        }
        ordered
    }

    /// `FUN_0049DF48` — the **last call of `Battle_ReturnToCampaign`**
    /// (`0x004AB383`): `FUN_004AD426(); Panels_RefreshAll(); FUN_0049DF48();`.
    ///
    /// It is the fourth caller of `Ai_ManageCountyFarms` and the same loop as
    /// [`Kingdom::ai_manage_farms_all`] (`Ai_ManageFarmsAll`, `0x0049A990`)
    /// with its two tests written the other way round — `isHuman == 0 &&
    /// strength != 0` there against `strength != 0 && isHuman == 0` here.
    /// Neither test has a side effect, so the predicate is one predicate and
    /// this delegates
    ///
    /// So **every battle re-farms every AI realm on the map**, not only the two
    /// that fought and not only the county fought over: an AI whose army died
    /// three counties away re-lays its fields the same instant.
    pub fn ai_manage_farms_after_battle(&mut self) -> i32 {
        self.ai_manage_farms_all()
    }

    /// AI step 6 — `AI_BuildCastles`.
    pub fn run_ai_castles(&mut self, realm: u8) -> Vec<u8> {
        ai::build_castles(
            &self.tables,
            &mut self.counties,
            self.county_count,
            &mut self.realms,
            realm,
        )
    }

    /// AI step 12 — the weapon rota and the industry switchboard, then the
    /// labour re-allocation and the estimate refresh the original's third loop
    /// does.
    pub fn run_ai_industry(&mut self, realm_id: u8) {
        let Some(realm) = self.realms.get(realm_id as usize) else { return };
        let mut realm = realm.clone();
        ai::choose_industry(&self.tables, &mut self.counties, self.county_count, &mut realm, realm_id);
        self.realms[realm_id as usize] = realm;
        let season_next = Season::from_index(self.season_next).unwrap_or(Season::Spring);
        for id in 1..=self.county_count {
            if self.counties[id].owner != realm_id {
                continue;
            }
            crate::labour::allocate(&mut self.counties[id]);
            // The blacksmith's ceiling is a share of the realm's stockpile
            // split across every *staffed* smithy it owns
            // on the line above is what staffs them — so the share is
// recomputed per county, inside the loop, as
            // `crate::field::set_type` recomputes it inside its own.
            let share = crate::industry::weapon_shares(
                &self.tables,
                &self.counties,
                self.county_count,
                realm_id,
            );
            crate::field::refresh_estimates(
                &mut self.counties[id],
                &self.campaign.map,
                season_next,
                &self.tables,
                self.options.advanced_farming,
                &self.realms[realm_id as usize],
                share,
            );
        }
    }

    /// `Diplo_Init` (`0x004A1C53`) — clear every inbox and open every realm's
    /// view of every other. **Call it once, after the realms are set up and
    /// before the first turn**, because the opening standing it writes depends
    /// on which realms are in play and which are people.
    ///
    /// `Game_NewGame` calls it in exactly that position, and this is the whole
    /// of the *"what writes it in a real game?"* answer for
    /// [`crate::realm::Pair::standing`]: nothing else puts an opening value in.
    pub fn init_diplomacy(&mut self) {
        crate::diplomacy::init(&mut self.realms, &mut self.diplomacy);
    }

    /// **AI step 1** — `Diplo_AnswerInbox`. Returns the replies.
    pub fn run_ai_inbox(&mut self, realm_id: u8) -> Vec<crate::diplomacy::Letter> {
        crate::diplomacy::answer_inbox(
            &mut self.realms,
            &mut self.diplomacy,
            &self.tables,
            realm_id,
        )
    }

    /// **AI step 2** — `AI_Diplomacy`. Returns the letters the realm sent.
    ///
    /// `g_rankLeader` is derived from the ranks [`ai::rank_realms`] wrote, the
    /// same way [`ai::rank_trailer`] is: the original keeps both as globals and
    /// deriving them is the same answer with one fewer thing to keep in step.
    pub fn run_ai_diplomacy(&mut self, realm_id: u8) -> Vec<crate::diplomacy::Letter> {
        let leader = ai::rank_leader(&self.realms);
        crate::diplomacy::ai_diplomacy(
            &mut self.realms,
            &self.tables,
            realm_id,
            self.year,
            leader,
        )
    }

    /// `Diplo_ReconcileAlliances` (`0x004A1847`) — called from `Turn_Tick`, and
    ///
    pub fn reconcile_alliances(&mut self) {
        crate::diplomacy::reconcile_alliances(&mut self.realms);
    }

    /// `Diplo_Post` — a person's letter into an AI's inbox. The player's whole
    /// outgoing side in single player
    pub fn post_letter(
        &mut self,
        from: u8,
        to: u8,
        kind: crate::diplomacy::Kind,
        gold: i32,
        county: u8,
    ) {
        crate::diplomacy::post(&mut self.realms, &mut self.diplomacy, from, to, kind, gold, county);
    }

    /// `Diplo_Offend` — apply one act's diplomatic damage. The four call sites
    /// are in [`crate::diplomacy::offence`]; this is where a caller holding a
    /// [`crate::movement::Offence`] or a [`crate::battle::Aftermath`] brings it.
    pub fn offend(&mut self, offended: u8, offender: u8, amount: i8) -> Vec<crate::diplomacy::Letter> {
        crate::diplomacy::offend(&mut self.realms, offended, offender, amount)
    }

    /// AI step 13 — `AI_Taunt`. Returns the letters the realm sent.
    pub fn run_ai_taunt(&mut self, realm_id: u8) -> Vec<ai::Taunt> {
        let trailer = ai::rank_trailer(&self.realms);
        let snapshot = self.realms.clone();
        let Some(realm) = self.realms.get_mut(realm_id as usize) else { return Vec::new() };
        ai::taunt(realm, realm_id, &snapshot, trailer)
    }
}


