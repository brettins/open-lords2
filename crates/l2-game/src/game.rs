//! The world, and the assets drawn from it.
//!
//! # One `Game`, borrowed by the screens
//!
//! `docs/plan.md`: *"A `Game` holds the kingdom, the active battle if any, and
//! the screen stack. Screens borrow it; they do not each keep a copy of the
//! world."* [`Game`] is that state, and it is a **plain struct of plain
//! fields** — fixed arrays, small integers, and types that are themselves plain
//! (`l2_kingdom::Kingdom` is arrays of counties and realms). No handle, no
//! index into a texture table, no `Rc`, nothing that only means something while
//! this process is running. That is what lets somebody serialise it later
//! without rewriting it first.
//!
//! [`Assets`] is deliberately *not* part of it. Decoded sprite sheets and a
//! palette are what the machine happens to have loaded, not what the world is,
//! and putting them in the same struct is how a save file ends up with a
//! tile-set in it.

use l2_formats::maps::{MapSet, MapSlot};
use l2_formats::Palette;
use l2_kingdom::county::{LABOUR_CEILING_IGNORED, MAX_COUNTIES};
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::tables::{JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT};
use l2_kingdom::{Kingdom, SeasonReport};
use l2_mods::vfs::Vfs;
use l2_view::campaign::{self, MapAssets};
use l2_view::chrome::{Chrome, Minimap};
use l2_view::village::VillageArt;
use l2_view::Ink;

use crate::shell::ShellAssets;

/// The highest tax rate the interface will set.
///
/// **It lives in `l2-kingdom` now, and this is a re-export.** It was defined
/// here, in the application crate, which is the wrong side of the seam: 50 is
/// not a widget's range, it is the length of `g_taxHappinessOther` minus one
/// (`l2_kingdom::tables::TAX_HAPPINESS_OTHER`), and a ruleset that replaces
/// that table is entitled to move it. The screens keep importing it under this
/// name; only its home changed.
pub use l2_kingdom::tables::MAX_TAX_RATE;

/// The grain-to-livestock split runs the full width of its slider track.
///
/// `Ration_SliderClick` (`0x0043A379`) clamps `mouseX - 224` to `0 … 100` and
/// the track is exactly 100 pixels wide, so the field's range and the widget's
/// geometry are the same number.
pub const MAX_RATION_SPLIT: i32 = 100;

/// Everything the screens draw with. Not part of the world.
pub struct Assets {
    pub palette: Palette,
    pub ink: Ink,
    pub map: MapAssets,
    /// The original's interface artwork — `Panels.pl8` and `Misc_cty.pl8`.
    ///
    /// `None` when the install does not supply them, which is the placeholder
    /// case: every screen then falls back to its own flat panels, and looks it.
    /// That is deliberate — a stub that is visibly ours beats one that looks
    /// finished.
    pub chrome: Option<Chrome>,
    /// `vill.pl8`, `villtops.pl8` and `vill_gd8.pl8` — the village screen's own
    /// files, which no other screen loads.
    ///
    /// `None` on an install without them, and the village then draws its own
    /// ground and refuses to move anybody, because the grid that decides where
    /// a drop lands *is* one of those files.
    pub village: Option<VillageArt>,
    /// What the shell screens draw with: `L2.eng`, the two panel fonts, and
    /// the per-screen artwork the front end and the management screens load.
    /// See [`crate::shell`].
    pub shell: ShellAssets,
    /// `L2_maps.dat` whole. A `MapSlot` borrows its file, so the bytes are kept
    /// and the slot is re-parsed on demand — which is bounds arithmetic, not
    /// decoding, and costs nothing.
    maps: Vec<u8>,
    /// The `MAPnn.PL8` files, by file number 1..=15, unparsed. Four map slots
    /// live in each and only one is ever wanted at a time, so they are decoded
    /// on demand by [`Assets::minimap`] and cached by the screen.
    minimap_files: Vec<Option<Vec<u8>>>,
}

impl Assets {
    /// Load through the mod overlay, so a mod that supplies its own `Base2a.pl8`
    /// or its own palette is picked up with no change to any drawing path.
    pub fn load(vfs: &Vfs) -> Result<Assets, String> {
        let maps = vfs.read("L2_maps.dat").map_err(|e| format!("L2_maps.dat: {e}"))?;
        MapSet::parse(&maps).map_err(|e| format!("L2_maps.dat: {e}"))?;
        let palette = vfs
            .palette(campaign::PALETTE)
            .map_err(|e| format!("{}: {e}", campaign::PALETTE))?;
        let map = MapAssets::load(|name| vfs.read(name).map_err(|e| format!("{name}: {e}")))?;
        // The chrome is optional: a partial install still starts, with our own
        // panels instead of the original's.
        let chrome =
            Chrome::load(|name| vfs.read(name).map_err(|e| format!("{name}: {e}"))).ok();
        let village =
            VillageArt::load(|name| vfs.read(name).map_err(|e| format!("{name}: {e}"))).ok();
        // The game ships 11 of the 15 `MAPnn.PL8` names; the four it does not
        // are exactly the empty map slots 24..39 (`docs/screens.md` §3.1).
        let minimap_files = (0..16)
            .map(|n| vfs.read(&Minimap::file_for_slot(n * 4)).ok())
            .collect();
        Ok(Assets {
            ink: Ink::for_palette(&palette),
            palette,
            map,
            chrome,
            village,
            shell: ShellAssets::load(vfs),
            maps,
            minimap_files,
        })
    }

    pub fn slot(&self, index: usize) -> Option<MapSlot<'_>> {
        MapSet::parse(&self.maps).ok()?.slot(index).ok()
    }

    /// The two 128 x 128 minimap rasters for a map slot, or `None` when the
    /// install has no `MAPnn.PL8` for it.
    pub fn minimap(&self, slot: usize) -> Option<Minimap> {
        let bytes = self.minimap_files.get(slot >> 2)?.as_ref()?;
        Minimap::load(bytes, slot).ok()
    }

    /// Assets with nothing in them: a grey ramp for a palette, one blank map
    /// slot, and five tile banks holding a single 2 x 2 frame.
    ///
    /// This is what lets the interface be tested on a machine with no copy of
    /// the game — every screen still lays out, every button is still where it
    /// is, and every assertion about *structure* still holds. Assertions about
    /// the shipped artwork need the install and live in the tests that skip
    /// without it.
    pub fn placeholder() -> Assets {
        // 256 greys, in the 6-bit range a `.256` file holds.
        let mut palette_bytes = vec![0u8; Palette::FILE_LEN];
        for i in 0..256usize {
            let v = (i / 4) as u8;
            palette_bytes[i * 3] = v;
            palette_bytes[i * 3 + 1] = v;
            palette_bytes[i * 3 + 2] = v;
        }
        let palette = Palette::from_bytes(&palette_bytes).expect("768 bytes");

        // The smallest legal PL8: one raw 2 x 2 frame.
        let mut pl8 = vec![0u8; 8 + 16];
        pl8[2] = 1; // one frame
        pl8[8] = 2; // width
        pl8[10] = 2; // height
        pl8[12..16].copy_from_slice(&24u32.to_le_bytes());
        pl8.extend_from_slice(&[1, 2, 3, 4]);
        let map = MapAssets::load(|_| Ok(pl8.clone())).expect("a synthetic sheet parses");

        Assets {
            ink: Ink::for_palette(&palette),
            palette,
            map,
            chrome: None,
            village: None,
            shell: ShellAssets::empty(),
            maps: vec![0u8; l2_formats::maps::SLOT_LEN],
            minimap_files: vec![None; 16],
        }
    }
}

/// The world.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Game {
    pub kingdom: Kingdom,
    /// `g_localPlayer` — the realm this machine drives.
    pub player: u8,
    /// The map slot the scenario runs on. **`g_scenarioIndex` *is* the slot**,
    /// 0..=59, used unshifted: `Map_LoadLattice` seeks `slot * 0x80C1`, the
    /// slot stride, and `Eng_DrawString(101, g_scenarioIndex, …)` indexes the
    /// 60 slot names in `L2.eng` group 101. An earlier revision shifted it
    /// right by two on the belief that the low bits selected a season; they do
    /// not — the season is its own global, and the low two bits pick which of
    /// four map slots inside a `MAPnn.PL8` the minimap comes from.
    pub map_slot: usize,
    /// Realm `+0x0A`, **raw**: which colour a realm flies. It picks the banner
    /// in the menu bar and the ramp the minimap tints a county with.
    ///
    /// Stored as the save holds it and clamped only at the point of use, by
    /// [`l2_view::chrome::realm_colour`] — the same 1..=5 clamp `FUN_004171EE`
    /// applies before using it as a frame index. Clamping on load would turn a
    /// misread offset into a plausible colour 1 for every realm, which is
    /// exactly the failure a test cannot see.
    pub realm_colour: [u8; MAX_REALMS],
    /// The county under the cursor's last click, or 0 for none. County ids are
    /// 1-based in the original, so 0 is a usable "nothing".
    pub selected: u8,
    /// County `+0x6C`, `+0x6D` — each county's anchor tile, which is where its
    /// marker is drawn. Two arrays rather than an array of pairs: index order
    /// is the only order anything here is ever walked in.
    pub anchor_x: [u8; MAX_COUNTIES],
    pub anchor_y: [u8; MAX_COUNTIES],
    /// Each realm's treasury as it stood before the last end-of-turn, so the
    /// interface can show which way the money went.
    pub gold_last: [i32; MAX_REALMS],
    /// What the last season did. Plain data: passes, messages and revolts.
    pub last_report: Option<SeasonReport>,
    /// How many turns this session has ended. `Kingdom::turn_count` is the
    /// game's own counter and starts at 1 in the England turn-one fixture; this one counts
    /// what the player did.
    pub turns_played: u32,
    /// **Whether this game is over, and where it sits in its campaign.**
    ///
    /// The three globals a campaign is made of — `DAT_0053F258`, `DAT_0053F640`
    /// and `DAT_0053F0C4` — plus the ending messages the current map has raised.
    /// It is here rather than in [`Kingdom`] because the original keeps it here
    /// too: `Game_NewGame` *clears* the outcome and *does not touch* the campaign
    /// counter, which is exactly the line between "the world" and "the session
    /// playing through it". See [`crate::victory`].
    pub campaign: crate::victory::Campaign,
    /// **What to answer *"Will you take the field?"* when nobody is asked.**
    ///
    /// [`crate::turn::end_turn`] is the headless door and cannot raise a screen,
    /// so every prompt it meets is answered with this. Declining is the
    /// original's own autocalc branch — it is a way out of *watching* a battle,
    /// not out of fighting one — so it is the default and nothing about a
    /// headless turn changed when the prompt was built.
    ///
    /// The interactive door ([`crate::turn::begin_turn`]) ignores it and asks.
    pub field_policy: crate::engagement::Answer,
    /// **A turn that stopped to ask.** `None` between turns, which is almost
    /// always.
    ///
    /// It is here rather than in the caller's hands because a half-run turn is
    /// not something a caller may drop: the kingdom is in a state no rule
    /// describes — two armies on one tile with the battle unresolved — and the
    /// only safe thing to do with it is finish it. See [`crate::turn`].
    pub(crate) turn: Option<crate::turn::TurnProgress>,
}

impl Game {
    /// An empty world. The scenario loader fills it; nothing else should
    /// construct a half-populated one.
    pub fn new(seed: u64) -> Game {
        Game {
            kingdom: Kingdom::new(seed),
            player: 1,
            map_slot: 0,
            realm_colour: [0; MAX_REALMS],
            selected: 0,
            anchor_x: [0; MAX_COUNTIES],
            anchor_y: [0; MAX_COUNTIES],
            gold_last: [0; MAX_REALMS],
            last_report: None,
            turns_played: 0,
            campaign: crate::victory::Campaign::new(crate::victory::Track::First),
            field_policy: crate::engagement::Answer::Decline,
            turn: None,
        }
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
    /// initialisation above them, so the human's strength is recounted and the
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
            self.campaign.raise(msg);
        }
        self.rank_realms();
    }

    /// `Score_RankRealms`, with its three globals kept.
    pub fn rank_realms(&mut self) {
        let mut out = Vec::new();
        self.campaign.ranking = l2_kingdom::victory::rank_and_crown(
            &self.kingdom.tables,
            &mut self.kingdom.realms,
            self.player,
            &mut out,
        );
        for msg in out {
            self.campaign.raise(msg);
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
    /// taxes is not a thing the interface refuses for tidiness; it is not the
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

    /// `Unit_OrderMove` (`0x004A7EEC`) — **the player's move order**, and the
    /// way anything on the campaign map is set walking from outside the turn
    /// machine.
    ///
    /// Three things it is, each of which is a rule rather than a convenience:
    ///
    /// * **[`Routing::Direct`]**, because a human order uses the cost map as it
    ///   stands. Road-hugging is what the game does for its own units — the AI's
    ///   armies, the merchants, the transports — and
    ///   [`l2_kingdom::movement::Routing::PreferRoads`] records that asymmetry.
    /// * **it starts the unit immediately**, `moving = 2` in the original, so
    ///   the army walks on the next tick whatever phase is current. There is no
    ///   phase for player movement, which is the finding
    ///   [`crate::turn`] is built on.
    /// * **the cost map is rebuilt for the order**, inside
    ///   [`l2_kingdom::movement::order_move`], so a tile trampled two steps ago
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
    /// `L2.eng` 69/3 — so this checks it here rather than taking the treasury
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

    /// Select a county, or clear the selection with 0. An id that is not a
    /// county on this map is refused rather than stored.
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
    pub fn set_tax_rate(&mut self, id: u8, rate: i32) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.counties[id as usize].tax_rate = rate.clamp(0, MAX_TAX_RATE);
        true
    }

    /// Set a county's wanted ration level, clamped to the six the table holds.
    ///
    /// It writes `rationWanted` (`+0x15E`), never `rationAchieved` (`+0x15D`):
    /// what the player asks for and what the county's stores could actually
    /// feed are different fields, and only the season pipeline decides the
    /// second one.
    pub fn set_ration(&mut self, id: u8, level: i32) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.counties[id as usize].ration_wanted =
            level.clamp(0, RATION_LEVEL_COUNT as i32 - 1);
        true
    }

    /// Set a county's grain-to-livestock split (`+0x15F`), clamped 0 … 100.
    ///
    /// The third order the original's ration panel gives, and the only one of
    /// the three that is a slider rather than a pair of arrows.
    ///
    /// **What this does not reproduce:** `Ration_SetSplit` (`0x0043A5A9`) also
    /// re-runs the county's food pass and, when the new split changes nothing,
    /// walks back towards the old value hunting for one that does. We write the
    /// field and stop, because our food pass only runs at end of turn.
    pub fn set_ration_split(&mut self, id: u8, split: i32) -> bool {
        if !self.is_players(id) {
            return false;
        }
        self.kingdom.counties[id as usize].ration_split = split.clamp(0, MAX_RATION_SPLIT);
        true
    }

    /// Move peasants from one job to another — the village screen's only order.
    ///
    /// `Labour_Move` (`0x00439B52`), and its caller `FUN_004399B0` which is
    /// where the arithmetic actually is:
    ///
    /// ```c
    /// workers = selectedIcons * county[+0xB8];
    /// if (labour[from] < workers) workers = labour[from];
    /// labour[to] += workers; labour[from] -= workers;
    /// ```
    ///
    /// So **an icon is `popBand` people**, and dragging every icon out of a job
    /// takes every worker out of it even when `icons * popBand` overshoots.
    /// Returns how many people actually moved.
    ///
    /// **What this does not do**, and the original does: `Labour_Move` re-runs
    /// the county's food pass, its industry estimates and its labour-share
    /// recompute *twice* before returning, so the whole panel is live the
    /// instant you let go. Ours runs those at end of turn, so the numbers a
    /// drag changes are the worker counts and nothing else. That is the same
    /// choice [`Game::set_ration_split`] already documents.
    pub fn move_labour(&mut self, id: u8, from: usize, to: usize, icons: i32) -> i32 {
        let band = self.kingdom.counties.get(id as usize).map_or(0, |c| c.pop_band);
        self.move_workers(id, from, to, icons.saturating_mul(band))
    }

    /// The same move counted in **people** rather than icons.
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
        let c = &mut self.kingdom.counties[id as usize];
        let (Some(&held), true) = (c.labour.get(from), to < c.labour.len()) else {
            return 0;
        };
        let workers = workers.min(held).max(0);
        c.labour[to] += workers;
        c.labour[from] -= workers;
        workers
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
    /// [`l2_kingdom::county::LABOUR_CEILING_IGNORED`] or above, exactly as the
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

#[cfg(test)]
mod tests {
    use super::*;

    fn two_counties() -> Game {
        let mut g = Game::new(7);
        g.kingdom.set_county_count(2);
        g.kingdom.counties[1].owner = 1;
        g.kingdom.counties[2].owner = 2;
        g
    }

    #[test]
    fn only_the_players_own_counties_take_orders() {
        let mut g = two_counties();
        assert!(g.set_tax_rate(1, 9));
        assert_eq!(g.kingdom.counties[1].tax_rate, 9);

        assert!(!g.set_tax_rate(2, 9), "county 2 belongs to another realm");
        assert_eq!(g.kingdom.counties[2].tax_rate, 0, "and it is unchanged");
        assert!(!g.set_ration(2, 5));
        assert!(!g.set_tax_rate(9, 1), "and 9 is not a county at all");
    }

    #[test]
    fn orders_are_clamped_to_the_ranges_the_rules_have() {
        let mut g = two_counties();
        g.set_tax_rate(1, -40);
        assert_eq!(g.kingdom.counties[1].tax_rate, 0);
        g.set_tax_rate(1, 10_000);
        assert_eq!(g.kingdom.counties[1].tax_rate, MAX_TAX_RATE);

        let achieved = g.kingdom.counties[1].ration_achieved;
        g.set_ration(1, 99);
        assert_eq!(g.kingdom.counties[1].ration_wanted, RATION_LEVEL_COUNT as i32 - 1);
        g.set_ration(1, -3);
        assert_eq!(g.kingdom.counties[1].ration_wanted, 0);
        assert_eq!(
            g.kingdom.counties[1].ration_achieved, achieved,
            "what the player asks for (+0x15E) is not what the county managed to feed (+0x15D)"
        );
    }

    #[test]
    fn selection_refuses_ids_that_are_not_counties_on_this_map() {
        let mut g = two_counties();
        assert!(g.select(2));
        assert_eq!(g.selected, 2);
        assert!(!g.select(3), "county 3 is past g_countyCount");
        assert_eq!(g.selected, 2, "and the refusal leaves the old selection alone");
        assert!(g.select(0));
        assert_eq!(g.selected, 0);
    }

    #[test]
    fn counting_owners_walks_the_map_rather_than_trusting_the_realm_record() {
        let mut g = two_counties();
        g.kingdom.realms[1].county_count = 99; // a stale record
        assert_eq!(g.owned_by(1), 1);
        assert_eq!(g.owned_by(0), 0);
    }
}
