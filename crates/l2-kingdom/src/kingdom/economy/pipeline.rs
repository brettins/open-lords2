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
    /// `Castle_RaiseFreeGarrison` (`0x004A551B`) — the archers a new castle
    /// comes with, mustered and marched straight inside.
    ///
    /// The original tops the realm's **bow** stock up by exactly the number it
    /// is about to hand out, so `Levy_ConsumeWeapons` takes them back and the
/// men cost nothing. Reproduced, because the
    /// order matters if the realm is short of bows: the top-up happens first,
    /// so it never is.
    pub(super) fn raise_free_garrison(&mut self, county: u8, archers: i32) {
        let owner = self.counties[county as usize].owner;
        let Some(realm) = self.realms.get_mut(owner as usize) else { return };
        let bow = crate::unit::TroopType::Archer.weapon_slot().unwrap_or(4);
        realm.weapons[bow] += archers;
        let mut basket = crate::levy::LevyBasket::seed(realm, archers);
        basket.equip(crate::unit::TroopType::Archer, archers);
        let map = self.campaign.map.clone();
        let muster = crate::levy::Muster {
            realm: owner,
            county,
            year: self.year,
            happiness_cost: 0,
        };
        let Ok(unit) = crate::levy::create_army(
            &self.tables,
            &map,
            &mut self.counties,
            &mut self.realms,
            &mut self.campaign.units,
            &mut self.campaign.names,
            &basket,
            muster,
            &mut self.campaign.explored,
        ) else {
            return;
        };
        let realms = self.realms.clone();
        crate::conquest::garrison_apply(
            &self.tables,
            &map,
            &mut self.counties,
            &realms,
            &mut self.campaign.units,
            unit,
            county,
        );
    }

    pub(super) fn migration_update(&mut self) {
        population::migrate_all(&mut self.counties, self.county_count, self.options.quirks);
    }

    pub(super) fn population_update(&mut self) {
        let Some(season) = self.season() else { return };
        population::update_all(
            &self.tables,
            &mut self.counties,
            self.county_count,
            season,
            self.options.quirks,
        );
    }

    /// The history ring — `FUN_004AE7DD`. See [`History`].
    pub(super) fn history(&mut self) {
        self.history.record(&self.counties);
    }

    /// `AI_SetTaxRates`' first half — set every county `realm` owns to the
    /// rate its happiness earns on that realm's ladder.
    ///
/// Like [`Kingdom::run_ai_grants`] this runs in the AI's turn
/// in `Season_Advance`, so it is exposed.
    /// **Realm 0 is the unowned counties**, which the original taxes once a
    /// turn in phase 1 on the neutral ladder. An out-of-range realm index does
/// nothing, because the caller is a turn machine and
    /// not a rule.
    /// **Paint one field.** `Field_SetType` (`0x00438BEC`) with this kingdom's
    /// own map, ruleset and clock supplied — the whole of what a click on the
    /// campaign map does to the simulation.
    ///
    /// This is the only writer of the field counts a player can reach, and
    /// until it existed there was none: every county of the England position
    /// starts with `fieldsGrain = 0` and nothing but the AI's own farming
    /// styles ever changed one. See [`crate::field`].
    ///
    /// A refusal is a refusal — the tile is not one of the county's twenty
    /// fields, it is blighted this season, or the brush is not one that tile's
    /// menu offers — and never a silent no-op.
    pub fn paint_field(
        &mut self,
        county: usize,
        tile: usize,
        brush: crate::field::FieldType,
    ) -> Result<(), crate::field::BrushRefusal> {
        let season_next = Season::from_index(self.season_next).unwrap_or(Season::Spring);
        crate::field::set_type(
            &mut self.counties,
            self.county_count,
            &mut self.campaign.map,
            county,
            tile,
            brush,
            season_next,
            &self.tables,
            self.options.advanced_farming,
            &self.realms,
        )
    }

    /// `County_RefreshEstimates` (`0x004485A5`) for one county, with the owning
    /// realm and its blacksmiths' share of the stockpile looked up.
    ///
/// Every caller in this crate goes through here
    /// arguments itself, because getting the *owner* wrong is the failure that
    /// leaves a county full of idle townsfolk.
    pub fn refresh_estimates(&mut self, county: usize) {
        if county == 0 || county >= self.counties.len() {
            return;
        }
        let season_next = Season::from_index(self.season_next).unwrap_or(Season::Spring);
        let advanced = self.options.advanced_farming;
        let owner = self.counties[county].owner;
        let share =
            crate::industry::weapon_shares(&self.tables, &self.counties, self.county_count, owner);
        let neutral = Realm::new();
        let realm = self.realms.get(owner as usize).unwrap_or(&neutral);
        crate::field::refresh_estimates(
            &mut self.counties[county],
            &self.campaign.map,
            season_next,
            &self.tables,
            advanced,
            realm,
            share,
        );
    }

    /// **Switch one industry, or castle building, on or off.**
    /// `Industry_ToggleFromMap` (`0x0043D309`) assembled — the enable byte, the
    /// industry share
    ///
    /// Like [`Kingdom::paint_field`] this is only reachable from a click on the
    /// map, because in the original it is only reachable from a click on the
    /// map: no county panel has an industry switch. Returns the state the
    /// switch ends in.
    ///
    /// ```c
    /// enabled ^= 1;                                  /* or the castle switch */
    /// County_RefreshEstimates(county, seasonNext);
    /// Labour_ToggleIndustryShare(county, job, enabled);
    /// County_RefreshEstimates(county, seasonNext);
    /// Labour_Allocate(county); Ration_Apply(county, season); Labour_Allocate(county);
    /// County_RefreshEstimates(county, seasonNext);
    /// Industry_UpdateSiteTile(county, industry);
    /// FUN_00448648(owner);                           /* every blacksmith of the realm */
    /// ```
    ///
    /// [`crate::industry::toggle_from_map`] does the flip and the share toggle
    /// together, so the first two refreshes are one here: an estimate reads no
    /// labour share, and it is idempotent while the efficiency write-back is
    /// not ported (C136)
    /// same refresh. **The last line was missing, and it is a visible one.**
    /// The Readme: *"turning a blacksmith on will reduce the resources
    /// available to other blacksmiths"* — every other smithy of the realm has
    /// its ceiling and its sidebar forecast recomputed on the click, and ours
    /// kept the old numbers until the season. So was `Ration_Apply`, which is
    /// [`crate::ration::preview`] for the reason
    /// [`Kingdom::set_ration_wanted`] gives.
    pub fn toggle_industry(&mut self, county: usize, what: crate::industry::MapToggle) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let quirks = self.options.quirks;
        let armies_eat = self.options.armies_eat;
        let on = crate::industry::toggle_from_map(&mut self.counties[county], what, quirks);
        self.refresh_estimates(county);
        crate::labour::allocate(&mut self.counties[county]);
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);
        crate::labour::allocate(&mut self.counties[county]);
        self.refresh_estimates(county);
        // `Industry_UpdateSiteTile(county, industry)` — the *visible* half of
        // the switch. A player: *"there's no message saying or visually
        // showing mining on / mining off."* The message is the caller's; this
        // is the picture.
        if let crate::industry::MapToggle::Industry(c) = what {
            self.update_industry_site(county, c);
        }
        let owner = self.counties[county].owner;
        self.refresh_blacksmiths(owner);
        on
    }

    /// **`Game_SetupRealmsAndCounties` (`0x0049BD99`)'s two rounds for one start
    /// county**, which run before it switches anything on.
    ///
    /// ```c
    /// Labour_Allocate(county); Ration_Apply(county, g_season); County_RefreshEstimates(county, g_seasonNext);
    /// Labour_Allocate(county); Ration_Apply(county, g_season); County_RefreshEstimates(county, g_seasonNext);
    /// castleType = g_startCastle; …materials, armoury…
    /// for (i = 0; i < 4; i++) if (i != 2 && industry[i].hasResource) { industry[i].enabled = 1; break; }
    /// castleSwitch = 1;
    /// ```
    ///
    /// **This is the only allocation a person's county gets before its first
    /// season.** An AI county is allocated again by `Ai_ManageFarmsAll` at the
    /// head of `Season_Advance`; the human one is not, so without these rounds
    /// it went into `Herd_SeasonTick` with nobody minding the cattle and into
    /// `Industry_ProduceAll` with nobody in the forest. Measured on a new
    /// England: the human county opened on 47 head and 47 idle, forecasting
    /// −10 cattle, where every AI county already matched `england-turn1.sav`.
    ///
    /// **The switches are off while the rounds run**, because the original sets
    /// them after. `l2_scenario::Scenario::from_map` has already set them, so
    /// they are put aside and put back. It is visible in the file: the human
    /// realm is the one whose wood stock did not move in the opening season —
    /// `Industry_ProduceAll`'s `symbols.json` note, *"(0, 66, 66, 132, 166)"* —
    /// because its foresters were dealt with the forest switched off.
    ///
    /// `Ration_Apply` is [`crate::ration::preview`]: it records and does not
    /// spend, for the reason [`Kingdom::set_ration_wanted`] gives.
    pub fn settle_start_county(&mut self, county: usize) {
        if county == 0 || county > self.county_count {
            return;
        }
        let armies_eat = self.options.armies_eat;
        let switches: [bool; 4] = core::array::from_fn(|i| self.counties[county].industry[i].enabled);
        let castle = self.counties[county].castle_switch;
        for industry in self.counties[county].industry.iter_mut() {
            industry.enabled = false;
        }
        self.counties[county].castle_switch = false;
        for _ in 0..2 {
            crate::labour::allocate(&mut self.counties[county]);
            crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);
            self.refresh_estimates(county);
        }
        for (industry, on) in self.counties[county].industry.iter_mut().zip(switches) {
            industry.enabled = on;
        }
        self.counties[county].castle_switch = castle;
    }

    /// **`FUN_00448648`** — `Industry_LabourEstimate(c, weapons)` for every
    /// county `owner` holds.
    ///
    /// ```c
    /// for (c = 1; c <= g_countyCount; c++)
    ///     if (g_counties[c].owner == owner) Industry_LabourEstimate(c, 2, 7, 15, 4);
    /// ```
    ///
    /// A blacksmith's ceiling is a share of the realm's stockpile split across
    /// every staffed smithy it owns ([`industry::weapon_shares`]), so anything
    /// that staffs or switches one moves every other smithy's number. Its two
    /// callers are `Labour_Move` (twice) and `Industry_ToggleFromMap`. The
    /// share cannot move inside the loop — an estimate staffs nothing — so it
    /// is taken once.
    pub fn refresh_blacksmiths(&mut self, owner: u8) {
        let share = industry::weapon_shares(&self.tables, &self.counties, self.county_count, owner);
        let advanced = self.options.advanced_farming;
        let count = self.county_count;
        let neutral = Realm::new();
        let (counties, realms, tables) = (&mut self.counties, &self.realms, &self.tables);
        let realm = realms.get(owner as usize).unwrap_or(&neutral);
        for id in 1..=count {
            if counties[id].owner == owner {
                industry::refresh(tables, &mut counties[id], Commodity::Weapons, realm, share, advanced);
            }
        }
    }

    /// **`FUN_0043A997(county, weaponType)` (`0x0043A997`) — what the blacksmith
    /// page's six hotspots do**
    /// simulation: *"I can't choose what type of weapon my blacksmiths are
    /// making."*
    ///
    /// ```c
    /// county[+0x290] = weaponType;                 /* County::weapon_type */
    /// Industry_LabourEstimate(county, 2, 7, 0xF, 4);   /* weapons, the blacksmith */
    /// Labour_Allocate(county);
    /// County_RefreshEstimates(county, g_seasonNext);
    /// FUN_00448648(owner);                         /* every blacksmith of the realm */
    /// DAT_005530D0 = 1;                            /* a redraw */
    /// if (g_localPlayer == owner) { Panel_JobBlacksmith(); Sound_RestartSlot(8); }
    /// ```
    ///
    /// **Five statements and four of them are the recompute**, which is the
/// whole reason this is a method here
    /// screen. The type is a divisor in three places at once:
    /// [`industry::weapon_shares`] sums `g_weaponCost[type]` over the realm's
    /// staffed smithies, so **changing one county's weapon moves every other
    /// county's ceiling in the same realm** — the Readme's *"turning a
    /// blacksmith on will reduce the resources available to other
    /// blacksmiths"*, reached from the other side. That is what the closing
    /// [`Kingdom::refresh_blacksmiths`] is for, and it is the original's own
    /// last line.
    ///
    /// **The estimate comes before the allocation**, which is the opposite way
    /// round from [`Kingdom::toggle_industry`] and is the original's order:
    /// `Industry_LabourEstimate` writes `labour_useful[7]`, the ceiling the
    /// allocator then deals against, so
    /// on the same click. There is **no `Ration_Apply` and no second
    /// allocation** here; `docs/decisions.md` C177 and
    /// [`Kingdom::set_ration_wanted`] on why that asymmetry is not tidied.
    ///
    /// **No owner guard, and that is the original's too.** `Screen_HandleInput`'s
    /// `0x0F` arm hit-tests the six hotspots on `g_jobPanelJob == 8` alone; the
    /// only `g_localPlayer` test in the function is the one that decides whether
    /// to repaint and play the hammer. What keeps a player out of an AI's smithy
    /// is that the job popup opens on `g_selectedCounty`.
    ///
    /// Returns `false` for a county out of range or a weapon out of
    /// [`crate::tables::WEAPON_TYPE_COUNT`], and does nothing in that case. The
    /// original indexes `g_weaponCost` with the byte unchecked; the hotspot
    /// table can only ever publish 0…5
    /// screen and is here because this is a public method.
    pub fn set_weapon_type(&mut self, county: usize, weapon: usize) -> bool {
        if county == 0 || county > self.county_count || self.counties.len() <= county {
            return false;
        }
        if weapon >= crate::tables::WEAPON_TYPE_COUNT {
            return false;
        }
        self.counties[county].weapon_type = weapon;
        // `Industry_LabourEstimate(county, 2, 7, 0xF, 4)` — the weapons row
        // alone, with the realm share as it stands *before* the county-wide
        // refresh below. `industry::refresh` is that function whole: the search
        // loop's two words into `labour[7]` and the tail's forecast.
        let advanced = self.options.advanced_farming;
        let owner = self.counties[county].owner;
        let share =
            industry::weapon_shares(&self.tables, &self.counties, self.county_count, owner);
        let neutral = Realm::new();
        {
            let realm = self.realms.get(owner as usize).unwrap_or(&neutral);
            industry::refresh(
                &self.tables,
                &mut self.counties[county],
                Commodity::Weapons,
                realm,
                share,
                advanced,
            );
        }
        crate::labour::allocate(&mut self.counties[county]);
        self.refresh_estimates(county);
        self.refresh_blacksmiths(owner);
        true
    }

    /// **`Labour_Move` (`0x00439B52`) — the village's drag and its double
    /// click, whole.**
    ///
    /// ```c
    /// labour[to] += workers; labour[from] -= workers;
    /// FUN_00439CC2(county, from, to);                /* switch the destination on */
    /// Ration_Apply(county, g_season);
    /// County_RefreshEstimates(county, g_seasonNext);
    /// FUN_00448648(owner);
    /// Labour_RecomputeIndustryShare(county);
    /// Labour_RecomputeShares(county);
    /// Ration_Apply(county, g_season);
    /// County_RefreshEstimates(county, g_seasonNext);
    /// FUN_00448648(owner);
    /// ```
    ///
    /// allocator deals a county out from its shares
    /// twice; `Labour_RecomputeShares` rewrites the shares from where people
    /// now stand, so the season deals the player's own split back to him. Ours
    /// moved the counts and nothing else, so every drag lasted until the
    /// season re-dealt the old split — *"the labor slider seems to reset each
    /// turn so that I have to reassign peasants to wheat each turn"* — and every
    /// forecast on the sidebar went on describing the staffing the player had
    /// just changed: *"industry values don't seem to update"*.
    ///
    /// `workers` is already clamped: that is `Village_Drop`'s job, and
    /// `Village_BalanceJob`'s arithmetic never overshoots, so `Labour_Move`
    /// does not repeat it. Returns `workers`. `Ration_Apply` is
    /// [`crate::ration::preview`], for the reason
    /// [`Kingdom::set_ration_wanted`] gives.
    pub fn move_labour(&mut self, county: usize, from: usize, to: usize, workers: i32) -> i32 {
        if county == 0 || county > self.county_count || from == to {
            return 0;
        }
        let jobs = self.counties[county].labour.len();
        if from >= jobs || to >= jobs {
            return 0;
        }
        let owner = self.counties[county].owner;
        let armies_eat = self.options.armies_eat;
        self.counties[county].labour[to] += workers;
        self.counties[county].labour[from] -= workers;
        self.switch_on_by_drop(county, to);
        for pass in 0..2 {
            if pass == 1 {
                crate::labour::recompute_industry_share(&mut self.counties[county]);
                crate::labour::recompute_shares(&mut self.counties[county]);
            }
            crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);
            self.refresh_estimates(county);
            self.refresh_blacksmiths(owner);
        }
        workers
    }

    /// **`FUN_00439CC2`** — the first thing `Labour_Move` does after the
    /// arithmetic, and it reads only the destination.
    ///
    /// ```c
    /// if      (to == 6 && industry[0].hasResource) rec = 0;   /* wood   */
    /// else if (to == 5 && industry[3].hasResource) rec = 3;   /* stone  */
    /// else if (to == 4 && industry[1].hasResource) rec = 1;   /* iron   */
    /// else if (to == 7 && industry[2].hasResource) rec = 2;   /* smithy */
    /// if (rec != 999) { industry[rec].enabled = 1; Industry_UpdateSiteTile(county, rec); }
    /// if (to == 3) castleSwitch = 1;
    /// ```
    ///
    /// **Putting men on a site is how a player switches it on**
    /// way besides the map. A new game opens with one industry on per start
    /// county — the first of wood, iron and stone it has, so never the mine in
    /// a county that also has a forest (`Game_SetupRealmsAndCounties`,
    /// `0x0049BD99`) — and without this a player who staffed that mine saw his
    /// men drawn as idle, no iron row on the sidebar
    /// home. `docs/decisions.md` C121 found it; nothing had built it. The share
    /// is not toggled here: `Labour_RecomputeShares` writes it from the workers.
    fn switch_on_by_drop(&mut self, county: usize, to: usize) {
        use crate::tables::{
            JOB_BLACKSMITH, JOB_CASTLE_BUILDING, JOB_IRON_MINING, JOB_STONE_QUARRYING,
            JOB_WOOD_CUTTING,
        };
        let record = match to {
            JOB_WOOD_CUTTING => Some(Commodity::Wood),
            JOB_STONE_QUARRYING => Some(Commodity::Stone),
            JOB_IRON_MINING => Some(Commodity::Iron),
            JOB_BLACKSMITH => Some(Commodity::Weapons),
            _ => None,
        };
        if let Some(c) = record {
            if self.counties[county].industry[c.index()].has_resource {
                self.counties[county].industry[c.index()].enabled = true;
                self.update_industry_site(county, c);
            }
        }
        if to == JOB_CASTLE_BUILDING {
            self.counties[county].castle_switch = true;
        }
    }

    /// **`Industry_UpdateSiteTile` (`0x0044EDC2`)**, the half of it that is
    /// terrain.
    ///
    /// ```c
    /// if (hasResource == 0 || disabledSeasons != 0) return;
    /// g_tiles[site].content = base + (enabled != 0);      /* 1 iron, 4 stone, 7 weapons, 10 wood */
    /// if (total - totalSnapshot < 1)
    ///     g_tiles[site].frame = idleFrame;                /* 30 iron, 0 stone, 10 weapons, 20 wood */
    /// ```
    ///
    /// **The `content` write is the on/off appearance**, and it is a whole
/// terrain value: an enabled site is `base + 1` and
    /// `Sprite_TopIt` animates exactly that value. So *"on"* is **motion**, not
    /// a different picture — see
    /// [`l2_view::campaign::industry_frames`](../../l2_view/campaign/fn.industry_frames.html).
    ///
    /// The frame reset is the other half and lives in the view, because this
    /// crate holds no graphics: a site with no output this season shows its
    /// idle frame, which is where the wheel starts from again.
    ///
    /// The guard matters and is easy to miss. A **wrecked** site — three
/// seasons on the countdown — is left as `Unit_TrampleTile` wrote
    /// it, so switching a trampled mine on and off changes nothing on the map
    /// until the countdown expires and `Industry_Produce`'s own call here
    /// repaints it.
    pub fn update_industry_site(&mut self, county: usize, c: crate::tables::Commodity) {
        let Some(record) = self.counties.get(county).map(|k| k.industry[c.index()]) else {
            return;
        };
        if !record.has_resource || record.disabled_seasons != 0 {
            return;
        }
        let Some(tile) = crate::map::industry_site(&self.campaign.map, county as u8, c) else {
            return;
        };
        let base = crate::map::terrain::INDUSTRY_IDLE[c.index()];
        self.campaign.map.terrain[tile] = base + u8::from(record.enabled);
    }

    /// **The farm/industry labour split** — `Labour_SetIndustryShare`
    /// (`0x0043933B`), which is what `Labour_SplitSliderDrag` (`0x00439122`,
    /// the *drag*, not the writer) calls once the track position has become a
    /// percentage:
    ///
    /// ```c
    /// county[+0x08] = share;
    /// Labour_Allocate(county);
    /// Ration_Apply(county, g_season);
    /// County_RefreshEstimates(county, g_seasonNext);
    /// ```
    ///
    /// **One pass, not two, and with `Ration_Apply` between them.** Ours ran
    /// `Labour_Allocate; County_RefreshEstimates` *twice* and never re-applied
    /// the ration at all — copied from [`toggle_industry`](Self::toggle_industry)
    /// on the reasoning that a control which moves the labour must re-allocate,
    /// which is true and is not the same as running the same pair twice. The
    /// omitted `Ration_Apply` is the one that matters: `herd_eaten` sizes the
    /// herd the estimate that follows searches over
    /// it twice. Two passes and a bare `Ration_SetSplit` is a different
    /// control — see [`set_ration_wanted`](Self::set_ration_wanted), which
    /// explains why that asymmetry is deliberate and must not be tidied.
    ///
    /// The doc this replaces named `FUN_00439122` and county `+0x2C`; the
    /// writer is `0x0043933B` and the field is `+0x08`.
    ///
    /// [`crate::labour::allocate`] reads
    /// [`crate::county::County::industry_share`] to size the industry pool, so
    /// moving the split without re-running it would leave every job's headcount
    /// describing the split the player just left.
    ///
    /// **What this does *not* fix, and cannot:** a click here also re-runs an
    /// allocation the season left owing, so on a county whose cattle ceiling is
    /// bounded by its population the milkmaid count *rises* when the player
    /// drags towards industry. That is the original's, and `docs/bugs.md` has
    /// it — the pipeline refreshes the ceilings after the last
    /// `Labour_AllocateAll`, so
    pub fn set_industry_share(&mut self, county: usize, share: i32) -> bool {
        if county == 0 || county > self.county_count {
            return false;
        }
        let share = share.clamp(0, 100);
        if self.counties[county].industry_share == share {
            return false;
        }
        self.counties[county].industry_share = share;
        crate::labour::allocate(&mut self.counties[county]);
        let armies_eat = self.options.armies_eat;
        // `Ration_Apply` records and does not spend — [`set_ration_wanted`] and
        // [`toggle_army_foraging`] make the same substitution for the same
        // reason. Calling the spending twin here would charge the county for a
        // meal every time the player nudged the slider.
        crate::ration::preview(&self.tables, &mut self.counties[county], armies_eat);
        self.refresh_estimates(county);
        true
    }

}

