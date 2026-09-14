#![allow(unused_imports)]
use super::*;

use super::*;
use super::*;
use super::unit_frames::*;
use super::levy::*;
use l2_formats::maps::{MapSet, MapSlot};
use l2_formats::Palette;
use l2_kingdom::county::{LABOUR_CEILING_IGNORED, MAX_COUNTIES};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::JOB_IDLE_TOWNSFOLK;
use l2_kingdom::{Kingdom, SeasonReport};
use l2_mods::vfs::Vfs;
use l2_view::campaign::{self, MapAssets};
use l2_view::chrome::{Chrome, Minimap};
use l2_view::village::VillageArt;
use l2_view::Ink;
use crate::shell::ShellAssets;
use l2_kingdom::tables::MAX_TAX_RATE;
use l2_kingdom::county::MAX_RATION_SPLIT;

impl Game {
    /// An empty world. The scenario loader fills it; nothing else should
    /// construct a half-populated one.
    pub fn new(seed: u64) -> Game {
        Game {
            kingdom: Kingdom::new(seed),
            player: 1,
            map_slot: 0,
            realm_colour: [0; MAX_REALMS],
            // Empty, not "Player1": `Options_SetDefaults` seeds the persisted
            // *settings* block with a default name and new-game setup is what
            // copies a name into `g_playerNames`. A world nobody has set up has
            // no lords in it to be called anything.
            player_names: [crate::text::PlayerName::EMPTY; MAX_REALMS],
            selected: 0,
            event_posted: [false; MAX_COUNTIES],
            anchor_x: [0; MAX_COUNTIES],
            anchor_y: [0; MAX_COUNTIES],
            gold_last: [0; MAX_REALMS],
            last_report: None,
            turns_played: 0,
            ai_granted: false,
            campaign: crate::victory::Campaign::new(crate::victory::Track::First),
            field_policy: crate::engagement::Answer::Decline,
            turn: None,
            prefs: Prefs::default(),
            presentation_quirks: Quirks::default(),
            levy: LevyOrder::default(),
            battle: None,
            begin_move_order: None,
            combine_ask: None,
            map_zoom_far: false,
            messages: crate::message::MessageQueue::new(),
            tips: crate::tip::Tips::new(),
            multiplayer: false,
            turn_clock: crate::turn_clock::TurnClock::default(),
            films: crate::movie::Reel::default(),
            unit_frames: UnitFrames::default(),
            // `Game_NewGame` (`0x00497CED`) writes 0 into `DAT_0055CE7C`,
            // new game opens the standings on *"Most counties,"*.
            nobles_category: 0,
            nobles_spoken: 0,
            spoken: (0, ""),
        }
    }

    // ------------------------------------------------------------ the levy

    /// `Sidebar_Button`'s hotspot 1 (`0x0043AE30`) — **open the levy.**
    ///
    /// ```c
    /// if (county.owner != g_localPlayer) { Msg_Enqueue(0x70); return; }
    /// Levy_SetPercent(county, g_levyPercent);     /* the slider is NOT reset */
    /// FUN_004AA90A(county, g_levyMen);            /* seed the basket        */
    /// g_screenId = 0x17;  DAT_005679D0 = 0;  DAT_0055446C = 0;
    /// ```
    ///
    /// Returns false for a county that is not the player's, which is the
    /// message-`0x70` arm.
    pub fn open_levy(&mut self, county: u8) -> bool {
        if !self.is_players(county) {
            return false;
        }
        self.levy.county = county;
        self.set_levy_percent(self.levy.percent);
        self.seed_levy_basket();
        self.levy.hire = false;
        true
    }

    /// `Levy_SetPercent(g_selectedCounty, g_levyPercent)` — the whole of what
    /// the slider does. **It does not touch the basket**; `Levy_SliderClick`'s
    /// tail is this call and a redraw request, and nothing else.
    pub fn set_levy_percent(&mut self, percent: i32) {
        self.levy.percent = percent.clamp(0, 100);
        let Some(county) = self.kingdom.counties.get(self.levy.county as usize) else { return };
        let levy = l2_kingdom::levy::set_percent(&self.kingdom.tables, county, self.levy.percent);
        self.levy.men = levy.men;
        self.levy.happiness_cost = levy.happiness_cost;
    }

    /// `FUN_004AA90A(county, g_levyMen)` — re-seed the basket from the realm's
    /// weapon stocks and the levy's headcount, and forget the selected rack.
    ///
    /// **Every door into the armoury calls it.** `Sidebar_Button` on the way in
    /// to `0x17`, `FUN_00435CBF` on the *Continue* button, and the right-release
    /// arm of `0x17`. So walking back to the levy screen and forward again
    /// throws away everything the player equipped — the original's behaviour,
    /// slider itself never touches the basket.
    ///
    /// **Its last three statements are the armoury's**, not the basket's:
    /// `g_armourySelectedType = 0; DAT_005679D0 = 0; _DAT_0057C8E4 = 0;`. The
    /// first is why the first rack a player opens after any door never sends a
    /// soldier — `FUN_004AABD8` is handed type 0 and its `0 < type` guard
    /// refuses — and the second ends a walk that was still on the floor.
    /// `[V]`, `0x004AA90A`. See [`crate::screens::armoury::Walker`].
    pub fn seed_levy_basket(&mut self) {
        let realm = self.player as usize;
        let Some(realm) = self.kingdom.realms.get(realm) else { return };
        self.levy.basket = l2_kingdom::LevyBasket::seed(realm, self.levy.men);
        self.levy.rack = 0;
        self.levy.anim.walker.active = false;
    }

    /// Whether this game has ended, and how. `DAT_0053F0C4`.
    pub fn outcome(&self) -> l2_kingdom::victory::Outcome {
        self.campaign.outcome
    }

    /// `FUN_0049B42B` for one realm, then `Score_RankRealms` — the ending chain's
    /// two halves in the order the original runs them, with the messages landing
    /// in [`Game::campaign`].
    ///
    /// **Called for every realm, the human included.** `AI_RunTurnStep`'s
    /// `isHuman` test guards the fourteen handlers, not the step-0
    /// initialisation above them,
    /// human's defeat detected on the human's own turn. Skipping humans here is
    /// the one way to build a game that cannot be lost.
    pub fn recount_realm(&mut self, realm: u8) {
        let msg = l2_kingdom::victory::recount_strength(
            &mut self.kingdom.realms,
            &self.kingdom.counties,
            self.kingdom.county_count,
            &self.kingdom.campaign.units,
            realm,
            self.player,
        );
        if let Some(msg) = msg {
            self.post_ending(msg);
        }
        self.rank_realms();
    }

    /// `Msg_Enqueue` for one of the ending chain's letters.
    ///
    /// **The filter is the interesting part and it is not ours.** `recount_realm`
    /// raises an AI's obituary with `to == 0` and your own defeat with
    /// `to == g_localPlayer`, and `Msg_Enqueue` keeps a record only when
    /// `to == 0 || to == g_localPlayer` —
    /// is told nothing, which `l2_kingdom::victory::recount_strength` already
    /// records and which this is the other half of. See [`crate::message`].
    fn post_ending(&mut self, msg: l2_kingdom::victory::Ending) {
        let player = self.player;
        self.messages.enqueue(crate::message::Record::from(msg), player);
    }

    /// `Score_RankRealms`, with its three globals kept.
    pub fn rank_realms(&mut self) {
        let mut out = Vec::new();
        self.campaign.ranking = l2_kingdom::victory::rank_and_crown(
            &self.kingdom.tables,
            &mut self.kingdom.realms,
            self.player,
            self.kingdom.options.quirks,
            &mut out,
        );
        for msg in out {
            self.post_ending(msg);
        }
    }

    /// The player's treasury.
    pub fn gold(&self) -> i32 {
        self.kingdom.realms.get(self.player as usize).map_or(0, |r| r.gold)
    }

    /// What the treasury did over the last end-of-turn.
    pub fn gold_change(&self) -> i32 {
        self.gold() - self.gold_last.get(self.player as usize).copied().unwrap_or(0)
    }

    /// Counties held by a realm, counted in index order.
    pub fn owned_by(&self, realm: u8) -> usize {
        self.kingdom
            .county_ids()
            .filter(|&id| self.kingdom.counties[id].owner == realm)
            .count()
    }

    pub fn is_county(&self, id: u8) -> bool {
        id >= 1 && (id as usize) <= self.kingdom.county_count
    }

    /// Whether the player may give this county orders. Setting another realm's
    /// taxes is not a thing the interface refuses for tidiness;
    /// player's county.
    pub fn is_players(&self, id: u8) -> bool {
        self.is_county(id) && self.kingdom.counties[id as usize].owner == self.player
    }

    /// Whether the player may give this **unit** orders.
    ///
    /// Ownership, not type: a player's merchant does not exist (merchants are
    /// realm 6's), but a peasant mob or a transport of the player's realm is
    /// theirs to move, and the original's map click does not check the type
    /// either.
    pub fn is_players_unit(&self, unit: usize) -> bool {
        self.kingdom.campaign.units.get(unit).is_some_and(|u| u.owner == self.player)
    }

    /// **Is this tile in the dark for the person at the screen?** — the test
    /// every one of the original's fog-honouring painters makes, and nothing
    /// else makes:
    ///
    /// ```c
    /// if (g_optExploration == 1 && (g_tiles[tile].bank & 0x20) == 0)
    /// ```
    ///
    /// `Map_DrawTile`, `Map_DrawTileApex`, `Sprite_TopIt` and `Map_DrawArmies`
    /// test it on the tile they are about to draw. **No input arm tests it** —
    /// a click on a dark tile resolves
    /// painters. `l2_kingdom::explore` has the readers and the writers.
    pub fn hides_tile(&self, tile: usize) -> bool {
        l2_kingdom::explore::hides(
            self.kingdom.options.exploration,
            &self.kingdom.campaign.explored,
            self.player,
            tile,
        )
    }

    /// **`Units_Tick` (`0x004650B0`) as the frame loop runs it** — the sweep,
    /// with the frame each unit's tick handler writes before it steps held for
    /// the painter. See [`UnitFrames`]. Both doors into the sweep in this crate,
    /// [`crate::turn::tick_units_only`] and the turn machine's phase tick, come
    /// through here,
    pub fn sweep_units(&mut self) -> l2_kingdom::units_tick::UnitsTick {
        let written = UnitFrames::written(&self.kingdom.campaign.units);
        let moved = self.kingdom.tick_units();
        self.unit_frames.hold(written, &self.kingdom.campaign.units);
        moved
    }

    /// **The frame `Map_DrawArmies` reads out of this unit's `+0x07`** — the
    /// one its tick handler wrote on the way into the last sweep, while nothing
    /// has touched the unit since, and otherwise the one its record gives now.
    pub fn unit_frame(&self, id: usize, unit: &l2_kingdom::Unit) -> usize {
        self.unit_frames.frame(id, unit)
    }

    /// `Unit_OrderMove` (`0x004A7EEC`) — **the player's move order**, and the
    /// way anything on the campaign map is set walking from outside the turn
    /// machine.
    ///
/// Three things it is, each of which is a rule:
    ///
    /// * **[`Routing::Direct`]**, because a human order uses the cost map as it
    ///   stands. Road-hugging is what the game does for its own units — the AI's
    ///   armies, the merchants, the transports — and
    ///   [`l2_kingdom::movement::Routing::PreferRoads`] records that asymmetry.
    /// * **it starts the unit immediately**, `moving = 2` in the original, so
    /// the army walks on the next tick whatever phase is current.
    ///   phase for player movement, which is the finding
    ///   [`crate::turn`] is built on.
    /// * **the cost map is rebuilt for the order**, inside
    /// [`l2_kingdom::movement::order_move`],
    ///   is already impassable to this one.
    ///
    /// Returns the number of steps ordered, or `None` if the unit is not the
    /// player's or no path reaches the tile. A refusal changes nothing.
    pub fn order_unit_move(&mut self, unit: usize, dest: (u8, u8)) -> Option<usize> {
        if !self.is_players_unit(unit) {
            return None;
        }
        l2_kingdom::movement::order_move(
            &self.kingdom.campaign.map,
            &mut self.kingdom.campaign.units,
            unit,
            dest,
            l2_kingdom::movement::Routing::Direct,
        )
    }

    /// **`Army_LeaveCastle` (`0x004374C4`)** — the garrison marches out, by the
    /// first button of the garrisoned info panel's table (`0x004DC5A8`).
    ///
    /// The original stashes `g_pickedTileUnit` and `g_pickedTileCounty`, closes
    /// the panel and runs `FUN_00437535`, which is
    /// [`l2_kingdom::conquest::leave_castle`]; under `g_multiplayer` it sends
    /// command `0x36` instead.
    ///
    /// **The sortie is staged here**, which is `FUN_00437535`'s tail:
    /// `if (unit.besiegedBy && Battle_BeginFromCampaign(unit, unit.besiegedBy))
    /// g_battleCounty = county;` — [`crate::turn::raise_sortie`], with the
    /// county the garrison left.
    pub fn leave_castle(&mut self, unit: usize) -> l2_kingdom::conquest::LeftCastle {
        let county = self.kingdom.campaign.units.get(unit).map_or(0, |u| u.garrison_county);
        let l2_kingdom::Kingdom { counties, realms, campaign, .. } = &mut self.kingdom;
        let out = l2_kingdom::conquest::leave_castle(
            &campaign.map,
            counties,
            realms,
            &mut campaign.units,
            unit,
            county,
        );
        if let l2_kingdom::conquest::LeftCastle::Marched { sortie: Some(besieger), .. } = out {
            crate::turn::raise_sortie(self, unit, besieger, county);
        }
        out
    }

    /// The player's units, in ascending slot order — what a map screen would
    /// draw and cycle through.
    pub fn player_units(&self) -> Vec<usize> {
        self.kingdom
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.owner == self.player)
            .map(|(id, _)| id)
            .collect()
    }

    /// The unit standing on a tile, if any. `Map_ResolvePick`'s
    /// `g_pickedTileUnit`, which is what every branch of [`Map_Click`] tests
    /// first.
    ///
    /// [`Map_Click`]: crate::screens::map
    pub fn unit_at(&self, x: u8, y: u8) -> Option<usize> {
        self.kingdom.campaign.units.at(x, y)
    }

    /// **Raise an army** — `FUN_00435B4D`, the raise-army screen's yes-button,
    /// end to end.
    ///
    /// ```c
    /// if (levyTotal == 0   && !hireMercs) message 0xA8;   /* group 168 */
    /// else if (levyTotal < 0x32 && !hireMercs) message 0x94;   /* group 148 */
    /// else if (Army_Create(localPlayer, county, hireMercs, g_levyHappinessCost) == 0)
    ///     message 0xDD;                                   /* group 221 */
    /// ```
    ///
    /// The two size guards are `&&`-ed with `hireMercs`, so **hiring a band
    /// bypasses both**: the band supplies the men and a levy of nothing is a
    /// legal army. [`l2_kingdom::levy::refuse_levy`] is that pair of guards and
    /// this is its only caller.
    ///
    /// `hire` is the county's standing offer, `county +0x1AD`, or `None` for a
    /// pure levy. The price is **not** checked inside
    /// [`l2_kingdom::MercenaryBands::hire`] — the screen refuses first, with
/// `L2.eng` 69/3 — so this checks it here
    /// negative.
    ///
    /// Returns the new army's slot.
    pub fn raise_army(
        &mut self,
        county: u8,
        basket: &l2_kingdom::LevyBasket,
        happiness_cost: i32,
        hire: Option<u8>,
    ) -> Result<usize, l2_kingdom::LevyRefusal> {
        if !self.is_players(county) {
            return Err(l2_kingdom::LevyRefusal::NowhereToStand);
        }
        if let Some(no) = l2_kingdom::levy::refuse_levy(basket.total(), hire.is_some()) {
            return Err(no);
        }
        let k = &mut self.kingdom;
        let muster = l2_kingdom::levy::Muster {
            realm: self.player,
            county,
            happiness_cost,
            year: k.year,
        };
        let id = l2_kingdom::levy::create_army(
            &k.tables,
            &k.campaign.map,
            &mut k.counties,
            &mut k.realms,
            &mut k.campaign.units,
            &mut k.campaign.names,
            basket,
            muster,
            &mut k.campaign.explored,
        )?;
        // `Mercenary_Hire` runs from **inside** `Army_Create`, after the men
        // and the troop counts are written and before the wage recount. Ours
        // runs immediately after, which lands the same numbers because nothing
        // between the two reads `men`.
        if let Some(band) = hire {
            let k = &mut self.kingdom;
            let l2_kingdom::Kingdom { counties, realms, campaign, .. } = k;
            campaign.mercenaries.hire(&mut campaign.units, counties, realms, id, band);
            l2_kingdom::unit::refresh_wages(
                &k.tables,
                &mut k.campaign.units,
                &mut k.realms,
                self.player,
                0,
            );
        }
        Ok(id)
    }

    /// **Split an army** — `FUN_00437AFB` then `Army_Split` (`0x00437FD7`).
    /// See [`l2_kingdom::divide`].
    pub fn split_army(
        &mut self,
        army: usize,
        basket: &l2_kingdom::SplitBasket,
        into: l2_kingdom::SplitInto,
    ) -> Result<usize, l2_kingdom::SplitRefusal> {
        if !self.is_players_unit(army) {
            return Err(l2_kingdom::SplitRefusal::NotAnArmy);
        }
        let k = &mut self.kingdom;
        let l2_kingdom::Kingdom { tables, counties, realms, campaign, year, .. } = k;
        let l2_kingdom::kingdom::Campaign { units, map, mercenaries, names, .. } = campaign;
        l2_kingdom::divide::split(
            tables, map, counties, realms, units, names, mercenaries, army, basket, into, *year,
        )
    }

    /// **Disband an army** — `Panel_DisbandButton` (`0x0043733A`) then
    /// `Army_Disband` (`0x00438681`). Returns the county the men joined and how
    /// many joined it.
    pub fn disband_army(&mut self, army: usize) -> Result<(u8, i32), l2_kingdom::DisbandRefusal> {
        if !self.is_players_unit(army) {
            return Err(l2_kingdom::DisbandRefusal::NotAnArmy);
        }
        let k = &mut self.kingdom;
        let difficulty = k.options.difficulty;
        let l2_kingdom::Kingdom { tables, counties, realms, campaign, .. } = k;
        let l2_kingdom::kingdom::Campaign { units, mercenaries, names, .. } = campaign;
        l2_kingdom::divide::disband(
            tables, counties, realms, units, names, mercenaries, army, difficulty,
        )
    }

    /// Select a county, or clear the selection with 0. An id
/// county on this map is refused.
    pub fn select(&mut self, id: u8) -> bool {
        if id == 0 {
            self.selected = 0;
            return true;
        }
        if !self.is_county(id) {
            return false;
        }
        self.selected = id;
        true
    }

    /// Set a county's tax rate, clamped. Returns false, and changes nothing,
    /// for a county the player does not hold.
    /// **`Tax_IncreaseCounty` (`0x0043AA83`) and `Tax_DecreaseCounty`**, whose
    /// second statement is `Tax_RecomputePreview` and whose third is a repaint.
    /// See [`l2_kingdom::Kingdom::set_tax_rate`].
    ///
    /// This wrote the rate and stopped — the same omission as the ration
    /// slider, on the panel next door, found the same evening by the same
    /// player: *"'People pay 0 crowns' on the tax thing always says 0 crowns.
    /// And the happiness bonus/minus on the tax screen is also stuck and not
    /// adjusting."* Two symptoms, one missing call.
    pub fn set_tax_rate(&mut self, id: u8, rate: i32) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.set_tax_rate(id as usize, rate)
    }

    /// Set a county's wanted ration level, clamped to the six the table holds.
    ///
    /// It writes `rationWanted` (`+0x15E`), never `rationAchieved` (`+0x15D`):
/// what the player asks for and what the county's stores could
    /// feed are different fields, and only the **food pass** decides the second
    /// one. That pass is not the season's alone — this control runs it too, and
    /// the sentence used to say *"only the season pipeline"*, which is what made
    /// the test below assert the defect.
    ///
    /// **`Ration_IncreaseCounty` (`0x0043A23F`)**, whose second statement is the
    /// food pass and whose fourth is a repaint — see
    /// [`l2_kingdom::Kingdom::set_ration_wanted`].
    ///
    /// The third of the three controls on this panel to be found writing its
    /// field and returning, and the only one of the three **not** reported by a
    /// player: it was found by enumerating the class the other two belong to.
    /// An unread member of an enumerated class is a known unknown, and this one
/// was filed `open` for a day.
    pub fn set_ration(&mut self, id: u8, level: i32) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.set_ration_wanted(id as usize, level)
    }

    /// Set a county's grain-to-livestock split (`+0x15F`), clamped 0 … 100.
    ///
    /// The third order the original's ration panel gives, and the only one of
/// the three that is a slider.
    ///
    /// **`sweep` is 1 for a jump on the track and 0 for an arrow**, and it is
    /// `Ration_SliderClick`'s `g_uiHotspotArg`. The two gestures end
    /// differently and a player can see the difference, so it is a parameter
    /// and not a detail — see [`l2_kingdom::Kingdom::set_ration_split`], which
    /// is the whole of the rule.
    ///
    /// This used to write the field and stop, and its own doc comment said so:
    /// *"we write the field and stop, because our food pass only runs at end of
    /// turn."* A player reported the result as **"rations slider moves but is
    /// inoperable"**, which it was — the thumb travelled and every number on
    /// the panel stayed where it was. The original re-runs the food pass on the
/// spot, searches for a split that changes something, reallocates
    /// the county twice and repaints the panel.
    ///
    /// Returns whether the split ended anywhere other than where it started.
    pub fn set_ration_split(&mut self, id: u8, split: i32, sweep: bool) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.set_ration_split(id as usize, split, sweep)
    }

    /// Move peasants from one job to another — the village screen's only order.
    ///
    /// `Labour_Move` (`0x00439B52`), and its caller `FUN_004399B0` which is
/// where the arithmetic is:
    ///
    /// ```c
    /// workers = selectedIcons * county[+0xB8];
    /// if (labour[from] < workers) workers = labour[from];
    /// labour[to] += workers; labour[from] -= workers;
    /// ```
    ///
    /// So **an icon is `popBand` people**, and dragging every icon out of a job
    /// takes every worker out of it even when `icons * popBand` overshoots.
/// Returns how many people moved.
    ///
    /// Everything after the arithmetic is `Labour_Move`'s and lives in
    /// [`l2_kingdom::Kingdom::move_labour`]: the drop switches the destination
    /// industry on, the forecasts are recomputed, and the county's shares are
    /// rewritten from where people ended up — which is what keeps the drag
    /// from being undone by the season's `Labour_Allocate`. This used to say
    /// *"ours runs those at end of turn"*; it ran none of them, and three
    /// player reports were that sentence (`docs/decisions.md` C180).
    pub fn move_labour(&mut self, id: u8, from: usize, to: usize, icons: i32) -> i32 {
        let band = self.kingdom.counties.get(id as usize).map_or(0, |c| c.pop_band);
        self.move_workers(id, from, to, icons.saturating_mul(band))
    }

/// The same move counted in **people**.
    ///
    /// `Labour_Move` itself takes a worker count; it is `Village_Drop` that
    /// multiplies by `popBand` and clamps. The double click
    /// (`Village_BalanceJob`, `0x00439F6A`) does not go through icons at all —
    /// it moves exactly the shortfall or exactly the surplus — so the two
    /// callers need the two shapes, and [`Game::move_labour`] is now this
    /// function with the icon arithmetic in front of it.
    pub fn move_workers(&mut self, id: u8, from: usize, to: usize, workers: i32) -> i32 {
        if !self.is_players(id) || from == to || workers <= 0 {
            return 0;
        }
        let c = &self.kingdom.counties[id as usize];
        let (Some(&held), true) = (c.labour.get(from), to < c.labour.len()) else {
            return 0;
        };
        // `Village_Drop`'s clamp, and `Village_BalanceJob`'s arithmetic never
        // needs it — both are the caller's, not `Labour_Move`'s.
        let workers = workers.min(held).max(0);
        self.kingdom.move_labour(id as usize, from, to, workers)
    }

    /// **The double click on the village: balance one job against the idle
    /// pool.** `Village_BalanceJob` (`0x00439F6A`), given a *cluster*.
    ///
    /// One gesture, two directions, and which one it is depends on the job:
    ///
    /// * a job **below its wanted floor** takes people *from* the idle
    ///   townsfolk — as many as it is short, or as many as are idle, whichever
    ///   is fewer;
    /// * a job **above its useful ceiling** puts the surplus *back* into the
    ///   idle townsfolk. That is the one the player asked for: *"I can't double
    ///   click idle peasants in a task to remove them from the task."*
    ///
    /// `fill` is the original's third argument. With it clear the shortfall
    /// branch is skipped entirely, so the job can only *shed* — which is how
    /// [`Game::balance_all_labour`] empties every job before refilling any.
    ///
    /// The floor is ignored when it is not positive and the ceiling when it is
    /// [`l2_kingdom::county::LABOUR_CEILING_IGNORED`] or above,
    /// original's two guards do. Returns how many people moved.
    pub fn balance_labour(&mut self, id: u8, cluster: usize, fill: bool) -> i32 {
        let Some(c) = self.kingdom.counties.get(id as usize) else { return 0 };
        let slot = l2_view::village::slot_for_cluster(
            cluster,
            c.industry[3].has_resource,
            c.industry[1].has_resource,
        );
        let wanted = c.labour_wanted[slot];
        let useful = c.labour_useful[slot];
        let workers = c.labour[slot];
        let short = if wanted < 1 { 0 } else { wanted - workers };
        let surplus = if useful < LABOUR_CEILING_IGNORED { workers - useful } else { 0 };
        let idle = c.labour[JOB_IDLE_TOWNSFOLK];

        if short < 1 || !fill {
            if surplus < 1 {
                return 0;
            }
            self.move_workers(id, slot, JOB_IDLE_TOWNSFOLK, surplus)
        } else {
            if idle == 0 {
                return 0;
            }
            self.move_workers(id, JOB_IDLE_TOWNSFOLK, slot, idle.min(short))
        }
    }

    /// **A double click on the idle townsfolk themselves: put everybody to
    /// work.** `Village_BalanceAll` (`0x00439EDB`)'s cluster-6 branch.
    ///
    /// Two passes, and the order is the whole point: every job **sheds** its
    /// surplus into the pool first, and only then does every job draw from the
    /// pool to fill its shortfall. One pass would let whichever job came first
    /// take people the later ones needed.
    ///
    /// Ten clusters, not eight — see
    /// [`l2_view::village::CLUSTER_TO_SLOT_BALANCE`].
    pub fn balance_all_labour(&mut self, id: u8) -> i32 {
        let clusters = l2_view::village::CLUSTER_TO_SLOT_BALANCE.len();
        let mut moved = 0;
        for fill in [false, true] {
            for cluster in 0..clusters {
                moved += self.balance_labour(id, cluster, fill);
            }
        }
        moved
    }
}



