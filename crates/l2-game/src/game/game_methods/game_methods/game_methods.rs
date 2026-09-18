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
    pub fn new(seed: u64) -> Game {
        Game {
            kingdom: Kingdom::new(seed),
            player: 1,
            map_slot: 0,
            realm_colour: [0; MAX_REALMS],
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


    /// `Sidebar_Button`'s hotspot 1 (`0x0043AE30`) — **open the levy.**
    ///
    /// ```c
    /// if (county.owner != g_localPlayer) { Msg_Enqueue(0x70); return; }
    /// Levy_SetPercent(county, g_levyPercent);     /* the slider is NOT reset */
    /// FUN_004AA90A(county, g_levyMen);            /* seed the basket        */
    /// g_screenId = 0x17;  DAT_005679D0 = 0;  DAT_0055446C = 0;
    /// ```
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
    /// `g_armourySelectedType = 0; DAT_005679D0 = 0; _DAT_0057C8E4 = 0;`. The
    /// first is why the first rack a player opens after any door never sends a
    /// soldier — `FUN_004AABD8` is handed type 0 and its `0 < type` guard
    /// refuses — and the second ends a walk that was still on the floor.
    ///
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

    fn post_ending(&mut self, msg: l2_kingdom::victory::Ending) {
        let player = self.player;
        self.messages.enqueue(crate::message::Record::from(msg), player);
    }

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

    pub fn gold(&self) -> i32 {
        self.kingdom.realms.get(self.player as usize).map_or(0, |r| r.gold)
    }

    pub fn gold_change(&self) -> i32 {
        self.gold() - self.gold_last.get(self.player as usize).copied().unwrap_or(0)
    }

    pub fn owned_by(&self, realm: u8) -> usize {
        self.kingdom
            .county_ids()
            .filter(|&id| self.kingdom.counties[id].owner == realm)
            .count()
    }

    pub fn is_county(&self, id: u8) -> bool {
        id >= 1 && (id as usize) <= self.kingdom.county_count
    }

    pub fn is_players(&self, id: u8) -> bool {
        self.is_county(id) && self.kingdom.counties[id as usize].owner == self.player
    }

    pub fn is_players_unit(&self, unit: usize) -> bool {
        self.kingdom.campaign.units.get(unit).is_some_and(|u| u.owner == self.player)
    }

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

    pub fn player_units(&self) -> Vec<usize> {
        self.kingdom
            .campaign
            .units
            .iter()
            .filter(|(_, u)| u.owner == self.player)
            .map(|(id, _)| id)
            .collect()
    }

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
    /// `hire` is the county's standing offer, `county +0x1AD`, or `None` for a
    /// pure levy. The price is **not** checked inside
    /// [`l2_kingdom::MercenaryBands::hire`] — the screen refuses first, with
/// `L2.eng` 69/3 — so this checks it here
    /// negative.
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

    /// **`Tax_IncreaseCounty` (`0x0043AA83`) and `Tax_DecreaseCounty`**, whose
    /// second statement is `Tax_RecomputePreview` and whose third is a repaint.
    pub fn set_tax_rate(&mut self, id: u8, rate: i32) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.set_tax_rate(id as usize, rate)
    }

    /// It writes `rationWanted` (`+0x15E`), never `rationAchieved` (`+0x15D`):
    ///
    /// **`Ration_IncreaseCounty` (`0x0043A23F`)**, whose second statement is the
    /// food pass and whose fourth is a repaint — see
    /// [`l2_kingdom::Kingdom::set_ration_wanted`].
    pub fn set_ration(&mut self, id: u8, level: i32) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.set_ration_wanted(id as usize, level)
    }

    /// Set a county's grain-to-livestock split (`+0x15F`), clamped 0 … 100.
    pub fn set_ration_split(&mut self, id: u8, split: i32, sweep: bool) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.set_ration_split(id as usize, split, sweep)
    }

    /// `Labour_Move` (`0x00439B52`), and its caller `FUN_004399B0` which is
/// where the arithmetic is:
    ///
    /// ```c
    /// workers = selectedIcons * county[+0xB8];
    /// if (labour[from] < workers) workers = labour[from];
    /// labour[to] += workers; labour[from] -= workers;
    /// ```
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
        let workers = workers.min(held).max(0);
        self.kingdom.move_labour(id as usize, from, to, workers)
    }

    /// **The double click on the village: balance one job against the idle
    /// pool.** `Village_BalanceJob` (`0x00439F6A`), given a *cluster*.
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



