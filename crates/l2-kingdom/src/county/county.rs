#![allow(unused_imports)]
use super::*;

use crate::tables::{
    Commodity, Weather, FIELD_PROGRESS_MAX, JOB_CATTLE_FARMING, JOB_COUNT, JOB_GRAIN_FARMING,
    JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT,
};

/// A county.
///
/// Field names follow `docs/kingdom.md`'s names, and each carries the original
/// offset it was identified at. Fields *not* in the document's table of 52 are
/// marked **engine state** — they are things the rules provably need that the
/// document does not place, and they are almost certainly among the ~150
/// untraced offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct County {
    // --- identity, happiness and health (docs/kingdom.md §1.1) -------------
    /// `+0x00` — set by `Event_RollAll` when this county drew a random event.
    pub event_fired: bool,
    /// `+0x1AA` — the id of the event that fired, which is also its `L2.eng`
    /// group. Cleared to 0 by `Event_RollAll` before it draws, and cleared
    /// again by any handler whose guard fails. See [`crate::event`].
    pub event_id: u16,
    /// `+0x05` — realm index 1..=5; **0 = unowned**. The first byte the
    /// desync comparator checks.
    pub owner: u8,
    /// `+0x09` — 0..=4 = *Diseased, Sick, Average, Good, Perfect*.
    pub health_band: u8,
    /// `+0x0B` — 0..=100, banded into [`County::health_band`].
    pub health_meter: i32,
    /// `+0x0C` — 0..=100. `L2.eng` group 85 index 7, *"This Season"*.
    pub happiness: i32,
    /// `+0x0D` — group 85 index 1, *"Last season"*.
    pub happiness_last: i32,
    /// `+0x0E` — the tax term applied this turn.
    pub d_hap_tax: i32,
    /// `+0x0F` — the "this county" half of the tax effect, for the tax panel.
    pub d_hap_tax_local: i32,
    /// `+0x10` — the health term.
    pub d_hap_health: i32,
    /// `+0x11` — the ration term.
    pub d_hap_ration: i32,
    /// `+0x12 … +0x15` — display copies taken at the moment the update ran,
    /// group 85 indices 2, 3, 4, 5: tax, ration, health, army.
    pub shown_tax: i32,
    pub shown_ration: i32,
    pub shown_health: i32,
    pub shown_army: i32,
    /// `+0x16` — group 86 index 4, *"Other counties"*; summed across the realm
    /// into realm `+0x28`.
    pub tax_hap_other: i32,
    /// `+0x17` — group 85 index 9, *"From events"*.
    pub shown_events: i32,
    /// `+0x18` — group 85 index 8, *"Average happiness"*.
    pub happiness_avg: i32,
    /// `+0x1C` — running total of `happiness` over all turns.
    pub happiness_sum: i32,
    /// `+0x194` — group 85 index 6, *"From ale"*. Written by
    /// [`crate::happiness::buy_ale`]; `docs/kingdom.md` §12 records the ale
    /// purchase path as untraced, and it is traced now.
    pub shown_ale: i32,
    /// `+0x219` — the happiness ale has given this county **this season**,
    /// which is what caps the bonus at five.
    ///
    /// **Corrected.** This field, `docs/kingdom.md` §7.6, `docs/mechanics.md`
    /// and `docs/symbols.json` all said *"nothing in the binary resets it, so
    /// the cap is for the life of the game"*. `Happiness_UpdateAll`
    /// (`0x0044BAEA`) resets it, in the same statement that clears
    /// [`County::shown_ale`] and the other five display terms
    /// every county. So the five points are a **seasonal** allowance, which is
    /// also what `Readme.txt`'s *"Ale Limitations"* describes. See
    /// [`crate::tables::ALE_HAPPINESS_MAX`] and `docs/decisions.md` C53.
    pub ale_happiness_given: i32,
    /// `+0x20` — 0..=4. At 4 the county revolts (§6).
    pub unrest: u8,
    /// `+0x21` — the "warned" flag `Unrest_UpdateAll` clears at happiness
/// >= 30, so message `0x92` fires once.
    /// `docs/kingdom.md` §6 describes the flag without giving its offset, and
    /// this said *engine state* for that reason: `Unrest_UpdateAll`
    /// (`0x0044AA41`) is the only reader and writer of `+0x21`, sets it under
    /// `0x1E`, and every save on this machine carries it set only in counties
    /// below thirty. `[V]`, `crates/l2-scenario/tests/import.rs`;
    /// `docs/decisions.md` C161.
    pub unrest_warned: bool,

    // --- population (docs/kingdom.md §1.2) ---------------------------------
    /// `+0x24` — group 73 index 7, *"This Season"*.
    pub population: i32,
    /// `+0x28` — *"Last season"*.
    pub pop_last: i32,
    /// `+0x2C` — `|pop - popLast| * 100 / popLast`.
    pub pop_change_pct: i32,
    /// `+0x30` — *"Births"*.
    pub births: i32,
    /// `+0x34` — *"Deaths"*, drawn negated.
    pub deaths: i32,
    /// `+0x38` — *"Army"*. Zeroed by `Population_UpdateAll` and filled
    /// elsewhere.
    pub army: i32,
    /// `+0x3C` — *"Emigrants to"*, with the destination in
    /// [`County::emigrant_destination`].
    pub emigrants: i32,
    /// `+0x40` — *"Total immigrants"*.
    pub immigrants: i32,
    /// `+0x44` — the biggest single incoming stream.
    pub largest_inflow: i32,
    /// `+0x58` — the destination county of this county's emigrants.
    pub emigrant_destination: u8,
    /// `+0x59` — the source county of [`County::largest_inflow`].
    pub largest_inflow_source: u8,
    /// `+0x48 … +0x57` — see [`crate::population`] for the documented bug in
    /// the loop that fills this.
    pub inflow_sources: [u8; MAX_INFLOW_SOURCES],
    /// `+0x5A` and `+0x5C …` — adjacency, read by migration and by the
    /// regional weather swing.
    pub neighbour_count: u8,
    pub neighbours: [u8; MAX_NEIGHBOURS],
    /// `+0x5B`.
    pub change_reason: ChangeReason,
    /// `+0xB8` — `(pop - 1) / 25 + 1`.
    pub pop_band: i32,
    /// `+0x6C`, `+0x6D` — the county's anchor tile.
    pub anchor_x: u8,
    pub anchor_y: u8,

    // --- money, food and land (docs/kingdom.md §1.3) -----------------------
    /// `+0xB9` — group 86 index 1, *"Tax rate"*.
    pub tax_rate: i32,
/// `+0xBC` — what the treasury banks.
    pub tax_collected: i32,
    /// `+0xC0` — group 86 index 2, *"People pay"*.
    pub tax_shown: i32,
    /// `+0x1F4` — **an unowned county's own treasury.**
    ///
    /// A county with no lord still farms, still taxes and still trades. Its tax
/// is banked here, the style-0 neutral pass
    /// tops it up by 100 ([`crate::ai_farm::NEUTRAL_PURSE_TOP_UP`]), and
    /// [`crate::trade::trade`] pays out of it whenever the realm argument is 0 —
    /// which is every trade `Ai_BuyGood` makes on behalf of an unowned county.
    ///
    /// **`[V]`
    ///
    /// * The banker is **`Tax_CollectAll` itself** (`0x0044B59B`), not
///   `FUN_0044B4F3` — it is inside
    ///   `Territory_BlockContains`. Its last statement is
    ///   `if (realm == 0) county.purse += county.taxCollected;` against the
    /// `else` that credits `realm.gold` and the two realm accumulators.
    ///   [`crate::tax::bank`] is that branch.
    /// * *"It is 0 in every fixture"* was **false**, and it is the sentence
    ///   that kept the neutral counties from shopping. Read out of the six
    ///   one-turn-apart saves at `+0x1F4`: county 1 carries 186, 297, 260 and
    ///   county 3 carries 195, 316, 294 across three consecutive turns of the
    ///   battle game, and siege county 3 carries 436. The claim was true of
    ///   `england-turn1.sav`, which is turn one — the only save in which no
    ///   season has ever banked anything here. `docs/decisions.md`
    ///   C149.
    pub purse: i32,
    /// `+0x1A4` — **how many merchants are standing in this county**, and
    /// therefore whether it has a stall to trade at.
    ///
    /// `Ai_BuyGood` (`0x004A4B12`) opens `if (county.merchantCount != 0)` and
    /// does nothing whatever when it is zero, so this byte is the gate in front
    /// of every purchase the AI and the neutral counties make. Written once a
    /// season by [`crate::merchant::recount_all`]
    /// (`County_RecountMerchants`, `0x00451061`) and by nothing else.
    pub merchant_count: i32,
    /// `+0x1A5` — **the unit index of the last merchant counted here**, whose
    /// morale prices the county's stall.
    ///
    /// `Ai_BuyGood` reads `g_units[county.merchantUnit].morale` and marks the
/// stall price up by it, so it is half of what
    /// a sack of grain costs. Two merchants in one county leaves the *higher*
/// index, because the recount overwrites.
    pub merchant_unit: u8,
    /// `+0x1A0` — **the lifetime count of merchant visits** to this county.
    ///
    /// The recount deliberately does not clear it: it zeroes `+0x1A4` and
    /// `+0x1A5` and then adds one here per merchant found, so it rises by that
    /// season's [`County::merchant_count`] every turn. Nothing in the binary
    /// reads it; carried because the pass writes it.
    pub merchant_visits: i32,
    /// `+0xC4 + job*0x0C` — workers assigned to each of nine jobs.
    pub labour: [i32; JOB_COUNT],
    /// `+0xC4 + job*0x0C + 0x04` — **the wanted floor**: how many workers this
    /// job needs before it stops going backwards.
    ///
    /// **`[D]`.** The second word of the twelve-byte labour record
    /// first of the two this project imported as nothing at all. Three
    /// different passes write it, and all three mean the same thing:
    ///
    /// * **Grain** (`FUN_0044D374`, `0x0044D374`) walks `workers = 0 … population`, calls
    ///   `Grain_Sow` / `Grain_Grow` / `Grain_Harvest` at each, and stores the
    ///   *first* count that reaches the best result. Grain's floor and ceiling
    ///   come out of the same search and are therefore equal.
    /// * **Cattle** (`FUN_0044DD4D`) walks the same range through
    ///   `Herd_BirthsAndDeaths` and stores the **first count at which births
    ///   less deaths stops being negative** — break-even — or, if the herd
    ///   cannot break even at any staffing, the least-bad count.
    /// * **Every other job** writes [`LABOUR_NO_FLOOR`]: reclamation
    ///   (`Field_ReclaimEstimate`, `0x0044C278`), castle building
    /// (`Castle_BuildEstimate`, `0x00450E46`) and the four industries
    ///   (`FUN_0044F318`) all set it to −1, meaning *no requirement*.
    ///
    /// Two things read it
    /// the worker count **red** when `labour < labour_wanted`, and
    /// `Village_RebuildIcons` draws the shortfall as extra, unselectable icons
    /// in the cluster. Nothing in the season pipeline reads it — it is a
    /// *recommendation*, computed by the passes that know, for the player and
    /// for the auto-allocator to act on.
    pub labour_wanted: [i32; JOB_COUNT],
    /// `+0xC4 + job*0x0C + 0x08` — **the useful ceiling**: the worker count
    /// past which more workers do the job no good.
    ///
    /// **`[D]`**, and unlike [`County::labour_wanted`] this one is not only a
    /// display hint: the labour allocator (`FUN_0044F6E7`) fills each of slots
    /// 0..=7 **up to this number and no further**, and drops what is left over
    /// into [`crate::tables::JOB_IDLE_TOWNSFOLK`].
    ///
    /// Its writers, and their sentinels:
    ///
    /// | job | value |
    /// |---|---|
    /// | grain | the fewest farmers that reach the best yield |
    /// | cattle | the staffing that **maximises** births less deaths |
    /// | reclamation | the work left in all reclaimable fields, capped 200 each |
    /// | castle building | the work the current build still needs |
    /// | iron, stone, wood | [`LABOUR_UNBOUNDED`] — more miners always help |
    /// | blacksmith | the smiths that turn the most iron into weapons |
    ///
    /// and **0 whenever the county has no such resource**, which is how a
    /// county with no mine ends up with no miners without the slot ever going
    /// away. [`LABOUR_UNSET`] means the estimate
    /// allocator reads it as 0.
    pub labour_useful: [i32; JOB_COUNT],
    /// `+0x130 + job*0x04` — **eight percentages, one per job**
    /// allocator's only instruction about where people should go.
    ///
    /// **`[V]`.** `FUN_00450000` recomputes them from the worker counts and
    /// indexes them as `(&DAT_0053FAE0)[i * 4]` for `i` in 0..3 and again for
    /// `i` in 3..8, which is the array written out. They are **two groups that
    /// each sum to 100**, not one that sums to 100:
    ///
    /// * jobs 0, 1, 2 — grain, cattle, reclamation — share the *farm*
    ///   workforce;
    /// * jobs 3 … 7 — castle, iron, stone, wood, blacksmith — share the
    ///   *industry* workforce;
    /// * job 8, *Idle townsfolk*, has no share and takes whatever is left.
    ///
    /// Both defaults close: `FUN_004514F8` sets 33 / 50 / 17 and 0 / 0 / 0 /
    /// 100 / 0, and `FUN_0045158B` sets 33 / 50 / 17 and 40 / 15 / 15 / 15 / 15.
    /// Each half is exactly 100 in both.
    ///
    /// `docs/screens-county.md` §9 listed `+0x130`, `+0x134` and `+0x138` as
    /// "seen, not understood" and guessed they belonged to the field-painting
    /// brush. They are the first three of these eight.
    pub labour_share: [i32; JOB_COUNT - 1],
    /// `+0x08` — the percentage of the county's people the allocator gives to
/// **industry**.
    ///
    /// **`[D]`.** `FUN_0044F6E7` opens `industry = Pct(population, +0x08);
    /// farm = population - industry` and fills the two halves from their own
/// percentages. `FUN_0044FF4A` writes it back from what was
    /// assigned, counting **half** the idle as industry:
    /// `PctOf(pop - grain - cattle - reclamation - idle + idle/2, pop)`.
    /// A fresh county starts on 25 (`FUN_00451150`).
    pub industry_share: i32,
    /// `+0x90 + i*2` — reclamation progress of each field, 0..=800.
    pub field_progress: [u16; MAX_FIELDS],
/// `+0x15D` — 0..=5, the level `Ration_Apply` managed to feed.
    pub ration_achieved: i32,
    /// `+0x15E` — what the player asked for.
    pub ration_wanted: i32,
    /// `+0x15F` — percentage of the food requirement taken from livestock
/// Clamped to `0 ..= `[`MAX_RATION_SPLIT`].
    pub ration_split: i32,
    /// `+0x178` — sacks eaten. See `docs/kingdom.md` §4.3 for what does and
    /// does not reproduce.
    pub grain_eaten: i32,
    /// `+0x17C` — head slaughtered.
    pub herd_eaten: i32,
    /// `+0x180`, `+0x184` — the caps applied to the two above.
    pub grain_available: i32,
    pub herd_available: i32,
    /// `+0x198`, `+0x19C` — troops standing in the county; added to the food
    /// requirement when *Armies Eat* is on. Rebuilt from the unit array by
    /// [`crate::unit::Units::recount_county_troops`].
    pub friendly_troops: i32,
    pub enemy_troops: i32,
    /// `+0x1AD` — **the mercenary band on offer here**, 1..=12, or 0 for none.
    ///
/// A one-slot cache: `Mercenary_AdvanceAll` rewrites it
    /// every season with the lowest-numbered unhired band standing in the
    /// county, so if two land together the higher-numbered one is invisible.
    /// See [`crate::mercenary`].
    pub mercenary_offer: u8,
    /// `+0x1BC` — the unit slot of the army **garrisoning this county's
    /// castle**, or 0.
    ///
/// Half of the pair that decides whether a county can be walked
    /// into: `Army_AttackCounty`'s guard passes when the county has no castle,
    /// **or** no garrison, or a garrison belonging to the attacker. See
    /// [`crate::conquest::can_be_entered`].
    pub garrison_unit: usize,
    /// `+0x2F4` — the levy surcharge, added to the happiness cost of every
    /// subsequent levy raised in this county.
    ///
    /// [`crate::levy::create_army`] writes [`crate::tables::LEVY_SURCHARGE`] and
    /// [`crate::happiness::decay_levy_surcharge`] takes 5 off it a season, so
    /// raising a second army out of the same county inside three seasons costs
    /// noticeably more than the first. `docs/armies.md` §6.1 records the write
    /// and says the decay *"was not traced"*; it is traced now.
    pub levy_surcharge: i32,
    /// `+0x1C0` — 0 none, 1 palisade, 2 motte and bailey, 3 Norman keep,
    /// 4 stone castle, 5 royal castle.
    pub castle_type: u8,
    /// `+0x1C1` — **the castle that was standing when the current work was
    /// ordered**, and *not* the type being built.
    ///
    /// > **This field was backwards here until the castle chooser was written,
    /// > and every reader in the crate was already right.** `Castle_Order`
    /// > (`0x00436D02`) has exactly one write in the whole binary:
    /// >
    /// > ```c
    /// > if (county.castleType != 0) county.castleBuilding = county.castleType;
    /// > county.castleType = newType;          /* immediately, not on completion */
    /// > ```
    /// >
    /// > So `castleType` is the castle you are *getting* from the moment you
    /// > order it, and `castleBuilding` is the one you *had*. Nothing ever
    /// > clears it, which is safe because every reader is gated on
    /// > [`County::castle_degraded`] being non-zero.
    /// >
    /// > Read that way, four readers stop being odd and start being obvious:
    /// > `Tax_CollectAll` charges the **standing** castle while work is under
    /// > way and 0 if there was none ([`crate::tax`]);
    /// > [`crate::siege::assault_castle_level`] fights the **standing** castle,
    /// > not the scaffolding; [`crate::siege::begin_siege`] refuses when
/// > `degraded == 1 && castleBuilding == 0`, which is *building
    /// > the first castle on a bare plot — there is nothing there to besiege*;
    /// > and `Castle_BuildTick`'s free-archer top-up fires on
    /// > `castleBuilding < castleType`, an **upgrade**. Under the old reading
    /// > the siege refusal was unreachable and the assault fought the wrong
    /// > castle. `[V]` — one writer, four agreeing readers.
    pub castle_building: u8,
    /// `+0x1C3` — **castle work in progress**, and it is a *byte with three
    /// values*, not a flag.
    ///
    /// | value | meaning |
    /// |---:|---|
    /// | 0 | nothing under way |
    /// | 1 | a castle is being **built or upgraded** — [`crate::siege::CASTLE_DEGRADED_BUILDING`] |
    /// | 2 | a castle is being **repaired after a siege** — [`crate::siege::CASTLE_DEGRADED_DAMAGED`] |
    ///
    /// > **This was a `bool` and `docs/kingdom.md` §4.1 said the flag *"was not
    /// > traced"*.** Three readers settle it and each names a different value.
    /// > The castle-build season pass (`0x00450C48`) branches on **2** to send
/// > message `0xA3` variant 1 and to skip the
    /// > garrison top-up, so 2 is *repair* and 1 is *new work*.
    /// > [`crate::siege::assault_castle_level`] reads **1** as *"fight the
    /// > castle being built"* and **2** as *"fight what a previous siege left
    /// > standing"*. And the map's info panel (`0x00414...`) prints a different
    /// > line for each. `[D]`
    ///
    /// Every existing reader tests it against zero — `Tax_CollectAll` charges
    /// the *lower* of type and building while it is non-zero, and
    /// [`crate::labour::ceilings`] opens the castle job while it is non-zero —
    /// so widening the field changes no behaviour those two have.
    pub castle_degraded: u8,
    /// `+0x1C2` — **the castle is ruined**. `L2.eng` 165 names it, and
    /// `Army_BeginSiege` refuses to lay a siege while it is set: there is
    /// nothing left to besiege.
    pub castle_ruined: bool,
    /// `+0x1F9` — the castle level **left standing** after a siege knocked one
    /// down, read only when [`County::castle_degraded`] is 2.
    /// [`crate::siege::assault_castle_level`] is its one reader here.
    pub castle_level_left: u8,
    /// `+0x1E4` … `+0x1F1` — **what the last siege left**, written by
    /// [`crate::siege::record_castle_damage`] and read back by
    /// [`crate::siege::scars_for_assault`] when the next assault opens on the
    /// same castle. See [`crate::siege::SiegeScars`] for why the round trip is
    /// the point.
    pub siege_scars: crate::siege::SiegeScars,
    /// `+0x1B0` — **the castle-building switch**, thrown from the map the same
    /// way an industry is: [`crate::industry::toggle_from_map`] with
    /// [`crate::industry::MapToggle::Castle`].
    ///
    /// `Labour_Allocate` gates castle building on this *and* on
    /// [`County::castle_degraded`]. [`crate::labour::ceilings`] applies only
    /// the second, and deliberately: the switch has three UI writers in the
    /// original and no AI writer at all, so gating on it here would stop every
    /// AI realm building a castle — a rule that is right for the original's
    /// human player and wrong for everybody else in it. The field exists so the
    /// map button has something to move; the gate is not added until the
    /// original's own AI path for it is found.
    pub castle_switch: bool,
    /// `+0x1C4` — **how far the current castle work has got, 0…100.**
    ///
    /// Not a derived figure: `Castle_BuildTick` writes it every season and the
    /// tile stamp reads it (`< 50` is scaffolding, `>= 50` is a half-built
    /// castle), so two seasons of the same castle look different on the map.
    /// Completion is `> 99`, tested on the byte.
    pub castle_percent: u8,
    /// `+0x1CC` / `+0x1D8` — **man-work still outstanding**
    /// work was ordered at.
    ///
    /// The original counts *down*: [`crate::industry::order_castle`] sets both
    /// to `g_castleWorkforce[level]` and each season's castle labour is
    /// subtracted from the first. A siege adds to **both**, which is how a
    /// repair becomes a bigger job than the castle it is repairing.
    pub castle_work_left: i32,
    pub castle_work_total: i32,
    /// `+0x1D0` / `+0x1DC` — **stone still owed**
    /// costed at. `+0x1D4` / `+0x1E0` are the same pair for wood.
    ///
    /// > **The materials are drawn down over seasons, not paid up front**, and
    /// > this crate had it the other way round with a comment saying the choice
    /// > was not a finding. It is one now. `Castle_Order` takes whatever the
    /// > realm has *at the moment of the order* and leaves the rest owing;
    /// > `Castle_DeliverMaterials` (`0x00450CCD`) then takes whatever the realm
    /// > has at the top of every season until the debt is clear. Until it *is*
    /// > clear `Castle_BuildEstimate` gives the castle job a ceiling of **zero**
    /// > —
    /// > builders idle, and eats the realm's quarry output as it arrives.
    pub castle_stone_owed: i32,
    pub castle_stone_total: i32,
    pub castle_wood_owed: i32,
    pub castle_wood_total: i32,
    /// `+0x1FB`, `+0x1FC`, `+0x1FD` — percentage swings to population, grain
    /// and herd from a random event.
    ///
    /// The population byte is a percentage of the season's **deaths** (when
    /// negative) or **births** (when positive), not of the county — see
    /// [`County::event_population_swing`].
    pub event_population_pct: i32,
    pub event_grain_pct: i32,
    pub event_herd_pct: i32,
    /// `+0x2F8` — **the people a random event added to this season's births or
    /// deaths**, which is the number *Plague* and *Wedding fever*'s letters
    /// print: `Msg_DrawWindow` (`0x0047309E`) draws it with `Ui_DrawNumber`
    /// before `L2.eng` group 77 index 29, *"extra deaths."*, or index 30,
    /// *"extra births."*
    ///
    /// Written by `Population_UpdateAll` (`0x00449EF3`) in **every** county
    /// every season — zero unless `+0x1FB` is non-zero — as
    /// `Pct(deaths or births, |pct|) + 10`, capped at `Pct(population, 20)`.
    /// [`crate::population::update_one`] is that code. `[V]`: those two
    /// functions are the only readers and writers of the offset in the binary.
    ///
    /// **It outlives the percentage.** `+0x1FB` is spent and cleared inside the
    /// season, and this is not touched again until the next population pass, so
    /// a letter opened during the player's turn still reads it.
    pub event_population_swing: i32,
    /// `g_countyFieldTiles + county * 0x50 + slot * 4` (`0x0053EA00`) — **the
    /// twenty map tiles that are this county's fields**, or 0 for an empty
    /// slot.
    ///
    /// Not part of the county record in the original, but per-county and
    /// twenty-wide, so it lives here. The original stores a **byte offset**
    /// into its eight-byte-per-tile array; this stores the tile index, which is
    /// that offset divided by eight and is what [`crate::map`] indexes by.
    ///
    /// This is the primary state the five counts below are derived from — see
    /// [`crate::field`]. Save block 12 is 1,360 bytes = 17 × 80 exactly, which
    /// is where the width comes from.
    pub field_tiles: [u16; MAX_FIELDS],
    /// `+0x15A` — the round-robin slot the pasture sweeps last stopped on.
    ///
    /// `FUN_0046958F` (fallow → pasture) and `FUN_0046965A` (grain → pasture)
    /// share it, both under `County_EnsurePasture` (`0x0046921D`). Each
    /// advances it before reading a slot and wraps it at [`County::field_slots_used`]
    /// (`+0x205`), so which field a cattle purchase eats depends on where the
    /// last purchase left off. `[V]`
    pub pasture_cursor: u8,
    /// `+0x15B` — the same cursor for `FUN_00469A9C`, the field
    /// `Weather_UpdateAll` (`0x00449889`) floods or parches. Separate from
    /// `+0x15A` in the original and kept separate here.
    pub blight_cursor: u8,
    /// `+0x1FF` — read positively by the fertility rule.
    ///
    /// **Derived.** [`crate::field::recount`] is the only thing that should
    /// write any of the five counts; they are a cache over
    /// [`County::field_tiles`] and the map's terrain plane.
    pub fields_fallow: i32,
    /// `+0x200` — **not read by the fertility rule at all.**
    pub fields_cattle: i32,
    /// `+0x201` — the field count `Grain_Sow` multiplies by sacks-per-field,
    /// and the term the fertility rule subtracts.
    pub fields_grain: i32,
    /// `+0x203` — fields in no use: terrain `0`
    /// `0x17` and `0x18` a drought or a flood leaves behind for one season.
    pub fields_waste: i32,
    /// `+0x204` — fields under reclamation.
    ///
/// The one count with a *rule* attached: it is what
    /// `Field_SetType` tests to decide whether field reclamation gets a share
    /// of the farm workforce at all.
    pub fields_reclaiming: i32,
    /// `+0x208` — -100..=100.
    pub fertility: i32,
    /// `+0x21B`.
    pub weather: Weather,
    /// `+0x21D` — the accumulator the weather band is computed from.
    ///
    /// `docs/kingdom.md` types this `i8`. It is held as `i32` here and clamped
    /// to the `i8` range on every write, because the documented update can push
    /// a county past 127 in one season (`delta + localModifier` on top of the
    /// global `delta`) and an `i8` would wrap a drought into a flood.
    pub dryness: i32,
    /// `+0x224` — sacks in store.
    pub grain: i32,
    /// `+0x240`, `+0x244`, `+0x248` — **the seed, the standing crop, and this
    /// season's harvest.**
    ///
    /// This document used to call them *"the growing crop at its three
    /// stages"*, and they are not that. `Grain_SeasonTick` writes `crop[0]`
/// once a year, at sowing, as the sacks that went into the
    /// ground; `crop[1]` is the **one** word the whole year's crop lives in,
    /// rewritten in place by every `Grain_Grow`; and `crop[2]` is cleared at
    /// the top of every season and holds what the harvest brought in. So the
    /// crop is one running number with a sowing figure in front of it and a
    /// harvest figure behind it, not three stages. See [`crate::land`].
    pub crop: [i32; 3],
    /// `+0x202` — **how many grain fields were sown**, kept so that losing a
    /// field mid-year cuts the crop.
    ///
    /// `FUN_0044D281` scales the standing crop by `fieldsGrain / this`
    /// whenever the county now has *fewer* grain fields than it sowed, and it
    /// runs at the top of both `Grain_Grow` and `Grain_Harvest`. Repainting a
    /// grain field as pasture in July therefore costs a share of the standing
    /// crop, and repainting *more* land as grain does nothing until next
    /// year's sowing.
    ///
    /// used to say. The two are written together, once, by the sowing clause,
    /// but only `+0x206` is ever stepped down again — see
    /// [`County::fields_grain_standing`].
    pub fields_grain_sown: i32,
    /// `+0x206` — **the grain fields sown this year that are still standing**,
    /// and the divisor of the picture on every grain tile.
    ///
    /// `Grain_SeasonTick` (`0x0044C8AE`) writes it beside `+0x202` at sowing,
    /// `fieldsGrain` or `1` on a shortfall. Unlike `+0x202` it is then
    /// **decremented** by `County_DestroyField` (`0x00469E5B`) and by
    /// `FUN_0046965A` whenever `fieldsGrain <= +0x206`, and it is the byte
    /// all three of `Grain_SeasonTick`'s `FUN_0044CF6F` calls divide the crop
    /// by — so it decides which of the four wheat pictures a county's fields
    /// show. `docs/stored-fields.json` filed it as *"a second copy of +0x202
    /// that only the tile graphic reads"*; three functions read it and two
    /// write it. `[D]`, and not derivable from `+0x202` and `fieldsGrain`:
    /// destroy a field, then paint two more
    /// `docs/decisions.md` C195.
    pub fields_grain_standing: i32,
    /// `+0x1A7` — `Grain_Sow` could not afford one sack a field and fell back
    /// to sowing a token handful.
    ///
    /// `Grain_SeasonTick` reads it immediately afterwards and records the
/// county's field usage as **1** when it is set.
    /// It is deliberately *not* cleared on `Grain_Sow`'s two early exits — no
    /// store, or nobody on the fields —
    /// carries last year's flag. That is the original's; nothing observable
    /// depends on it, because the crop is 0 either way.
    pub sow_shortfall: bool,
    /// `+0x250` — head of livestock.
    pub herd: i32,
    /// `+0x25C` — how crowded the herd is: 10, 20, 30 or 40, which `L2.eng`
    /// group 77 names *"Low herd crowding."*, *"Average herd crowding."*,
    /// *"Herd overcrowded."* and *"Massive overcrowding!!"*
    /// (`docs/kingdom.md` §13.1).
    ///
/// **Stored**
    /// difference is observable: `FUN_0044D913` recomputes it at the *end* of
    /// the herd's tick,
    /// crowding the herd had when the season began. See
    /// [`crate::land::herd_crowding`].
    pub herd_crowding: i32,
    /// `+0x268`, `+0x26C` and `+0x258` — next season's forecast: group 77's
    /// *"Calf births expected"*, *"Cow deaths expected"* and *"Change due to
    /// farming"*. Written by [`crate::land::herd_preview`]
    /// numbers `crates/l2-kingdom/tests/reproduction.rs` holds against the
    /// England turn-one save.
    pub herd_births_expected: i32,
    pub herd_deaths_expected: i32,
    pub herd_change_expected: i32,
    /// `+0x24C` and `+0x278` — **what last season's weather and last season's
    /// random event did to the grain**: the two figures `Panel_JobGrain`
    /// (`0x00413590`) and `TileInfo_DrawGrain` (`0x0041CB3A`) print under
    /// `L2.eng` group 77.
    ///
    /// `grain_weather_change` is the season's stage **after** its weather band
    /// less **before** it — `crop[0]` at sowing, `crop[1]` while growing,
    /// `crop[2]` at harvest — signed, and drawn (advanced farming only) as 77/16
    /// *"gained last season, due to weather."*, 77/17 *"lost …"* or 77/18
    /// *"Weather had no effect last season."*
    ///
    /// `grain_event_change` is the **magnitude** of the event's percentage of the
    /// store. The painter chooses 77/25 *"eaten by rats."* or 77/26 *"found as
/// surplus."* from the event id, and
    /// `Msg_DrawWindow` (`0x0047309E`) prints the same number in the *Rats!!*
    /// and *Grain found.* letters.
    ///
    /// Both written by [`crate::land::grain_season_tick`], which is
    /// `Grain_SeasonTick` (`0x0044C8AE`): the event figure zeroed at the top of
    /// every county's tick, the weather figure in whichever season branch runs.
    /// **Nothing reads either back**, so neither can move the simulation; they
    /// are carried because without them a loaded game prints *"no effect"* for a
    /// season the original prints a number for. **Zero in every county of every
    /// save on this machine** — all eighteen stand in *Cloudy* weather with no
    /// grain event live — so the import is compared only against zero.
    pub grain_weather_change: i32,
    pub grain_event_change: i32,
    /// `+0x270` and `+0x274` — **the same pair for the herd**, printed by
    /// `Panel_JobCattle` (`0x00413B30`) and `TileInfo_DrawHerd` (`0x0041D299`).
    ///
    /// `herd_weather_change` is `Pct(herd, g_herdWeatherPct[weather])`, signed,
    /// and **forced to 0 by the No Bull event** — it is stored after that
    /// override. `herd_event_change` is the magnitude of the event's percentage
    /// of the herd, which the painter words as 77/20 *"died of disease."*, 77/21
    /// *"taken by wolves."*, 77/22 *"had to be put down."* or 77/23 *"born, over
    /// expectations."* by the event id; zero for No Bull, which has no figure.
    /// Written by [`crate::land::herd_season_tick`], which is `Herd_SeasonTick`
    /// (`0x0044D60D`). Zero in every save on this machine, for the same reason.
    pub herd_weather_change: i32,
    pub herd_event_change: i32,
    /// `+0x230`, `+0x2FC` and `+0x22C` — **the grain row's three forecasts**,
    /// and `crop[2]` is the fourth.
    ///
    /// A player: *"Sidebar doesn't show grain being planted as a negative
    /// number."* He is right, and he is describing **Spring**. These are the
    /// numbers that say so, and until now nothing in this workspace computed
/// any of them — so the sign question never arose.
    ///
    /// They are the **tail** of `Grain_LabourEstimate` (`0x0044D374`), after the
    /// search loop that [`crate::land::grain_labour_estimate`] reproduces:
    ///
    /// ```c
    /// staff = county.labour[0].workers; /* the real staffing */
    /// county.field_0x230 = Grain_Sow(county, staff, county.grain);
    /// if (season == 4)                county.crop[2]     = Grain_Harvest(county, staff, crop[1]);
    /// if (season == 2 || season == 3) county.field_0x2FC = Grain_Grow   (county, staff, crop[1]);
    ///
    /// if      (season == 1) county.field_0x22C = -county.field_0x230 - county.grainEaten;
    /// else if (season == 4) county.field_0x22C =  county.crop[2]     - county.grainEaten;
    /// else                  county.field_0x22C = -county.grainEaten;
    /// ```
    ///
    /// from it.** The loop calls `Grain_Sow(county, workers, grain − grainEaten)`
    /// and the tail calls `Grain_Sow(county, staff, grain)` — a different third
    /// argument and a different worker count. That is the whole reason
/// [`crate::land::grain_preview`] exists as a second pass
    /// value the estimate returns.
    ///
    /// **Encoded, deliberately
    /// *derived* — every estimate round recomputes them from state the digest
    /// already carries — so they cannot diverge on their own and could have been
    /// left out. They are in anyway, for the reason the three cattle fields
    /// above them are: **nothing re-runs the estimate round on load**,
    /// reloaded game would show a blank produce row until the next turn ended,
    /// and a blank row is exactly the defect this field exists to fix. A field
    /// that is cheap to carry and visible when absent is carried.
    pub grain_sown_expected: i32,
    pub grain_grown_expected: i32,
    pub grain_change_expected: i32,
    /// `+0x20C` and `+0x214` — **the reclamation row's two figures**, written by
    /// [`crate::land::reclaim_preview`], which is `Field_ReclaimEstimate`'s
    /// tail.
    ///
    /// A player: *"the figure is missing in the sidebar — it draws the serf
    /// reclaiming, but not the +1 I'm used to."* The `+1` is
    /// `reclaim_fields_finishing`, and it is a count of **fields that will be
    /// finished next season**, not of work done: the original simulates the
    /// coming season's labour over the twenty slots from the nearest-to-finished
    /// field and counts each one that crosses 800. A field that finishes hands
    /// its surplus to the next, so one gang can complete two.
    ///
    /// `reclaim_seasons_to_next` is the same row's second number, and it is
/// computed from the **full** staffing
    /// above had left over — the original re-reads `labour[2].workers`.
    ///
    /// Encoded on the same reasoning as the grain forecasts above: derived,
    /// recomputed by every estimate round, and visibly blank for a turn if a
    /// load defaulted them.
    pub reclaim_fields_finishing: i32,
    pub reclaim_seasons_to_next: i32,
    /// `+0x294 + c*0x18` — per-commodity production records.
    pub industry: [Industry; 4],
    /// **Engine state.** Which weapon the blacksmith is making.
    /// `docs/kingdom.md` §7.4 says weapons are credited to
    /// `realm +0x140 + type*4` without saying what picks `type`.
    pub weapon_type: usize,
    /// `+0x1FE` — **the farming style the county is farmed by**
/// piece of AI personality that lives on the county
    /// realm.
    ///
    /// `Ai_ManageCountyFarms` (`0x0049DD01`) writes the owning lord's
    /// `farmStyle` here every pass and dispatches on it. The unowned counties'
    /// pass, `AI_ManageFields(0)` (`0x0049DFC6`), only *reads* it —
    /// that has fallen out of a realm keeps farming the way its last lord
    /// farmed, and one whose last lord was a style-9 mixer is farmed by nobody
    /// at all, because the neutral pass dispatches only 0 and 1.
    ///
    /// **`[V]`** — `docs/symbols.md` records that `+0x1FE` holds only 0, 1 and
    /// 9 across the England turn-one fixture, which is exactly the set of
    /// values `AI_PERSONALITY_FARM_STYLE` can produce.
    /// [`crate::ai_farm`] is the whole rule.
    pub farm_style: u8,
    /// `+0x1A8` — an **untraced gate**: when non-zero, `Tax_CollectAll` takes
    /// nothing at all (`docs/kingdom.md` §4.1).
    pub tax_suppressed: bool,
}

impl Default for County {
    fn default() -> Self {
        County::new()
    }
}

impl County {
    /// An empty, unowned county. Everything is zero except the three values
    /// that have a documented non-zero default.
    pub fn new() -> County {
        County {
            event_fired: false,
            event_id: 0,
            owner: 0,
            health_band: 0,
            health_meter: 0,
            happiness: 0,
            happiness_last: 0,
            d_hap_tax: 0,
            d_hap_tax_local: 0,
            d_hap_health: 0,
            d_hap_ration: 0,
            shown_tax: 0,
            shown_ration: 0,
            shown_health: 0,
            shown_army: 0,
            tax_hap_other: 0,
            shown_events: 0,
            happiness_avg: 0,
            happiness_sum: 0,
            shown_ale: 0,
            ale_happiness_given: 0,
            unrest: 0,
            unrest_warned: false,
            population: 0,
            pop_last: 0,
            pop_change_pct: 0,
            births: 0,
            deaths: 0,
            army: 0,
            emigrants: 0,
            immigrants: 0,
            largest_inflow: 0,
            emigrant_destination: 0,
            largest_inflow_source: 0,
            inflow_sources: [0; MAX_INFLOW_SOURCES],
            neighbour_count: 0,
            neighbours: [0; MAX_NEIGHBOURS],
            change_reason: ChangeReason::None,
            pop_band: 0,
            anchor_x: 0,
            anchor_y: 0,
            tax_rate: 0,
            tax_collected: 0,
            purse: 0,
            merchant_count: 0,
            merchant_unit: 0,
            merchant_visits: 0,
            tax_shown: 0,
            labour: [0; JOB_COUNT],
            // Zero, not the sentinels: `FUN_00451150` sets up a fresh county
            // with `useful = 0; wanted = useful; workers = wanted;` for all
            // nine records, and only then runs the estimates.
            labour_wanted: [0; JOB_COUNT],
            labour_useful: [0; JOB_COUNT],
            // `FUN_004514F8`'s defaults: 33 / 50 / 17 across the farm and all
            // of the industry share on wood, each half summing to 100.
            labour_share: [33, 50, 17, 0, 0, 0, 100, 0],
            industry_share: 25,
            field_progress: [0; MAX_FIELDS],
            // Normal rations, all of it from livestock: the values every county
            // in the shipped lastturn.sav carries (docs/kingdom.md §4.3).
            ration_achieved: 3,
            ration_wanted: 3,
            ration_split: 100,
            grain_eaten: 0,
            herd_eaten: 0,
            grain_available: 0,
            herd_available: 0,
            friendly_troops: 0,
            enemy_troops: 0,
            mercenary_offer: 0,
            garrison_unit: 0,
            levy_surcharge: 0,
            castle_type: 0,
            castle_building: 0,
            castle_degraded: 0,
            castle_ruined: false,
            castle_level_left: 0,
            siege_scars: crate::siege::SiegeScars::default(),
            castle_switch: false,
            castle_percent: 0,
            castle_work_left: 0,
            castle_work_total: 0,
            castle_stone_owed: 0,
            castle_stone_total: 0,
            castle_wood_owed: 0,
            castle_wood_total: 0,
            event_population_pct: 0,
            event_population_swing: 0,
            event_grain_pct: 0,
            event_herd_pct: 0,
            field_tiles: [0; MAX_FIELDS],
            pasture_cursor: 0,
            blight_cursor: 0,
            fields_fallow: 0,
            fields_cattle: 0,
            fields_grain: 0,
            fields_waste: 0,
            fields_reclaiming: 0,
            fertility: 0,
            weather: Weather::Cloudy,
            dryness: 0,
            grain: 0,
            crop: [0; 3],
            fields_grain_sown: 0,
            fields_grain_standing: 0,
            sow_shortfall: false,
            herd: 0,
            // The lowest band: density 0 is at the bottom of it, and a county
            // with no pasture is pushed to the top band by `herd_crowding` the
            // first time the herd ticks.
            herd_crowding: crate::tables::HERD_CROWDING[0].1,
            herd_births_expected: 0,
            herd_deaths_expected: 0,
            herd_change_expected: 0,
            grain_weather_change: 0,
            grain_event_change: 0,
            herd_weather_change: 0,
            herd_event_change: 0,
            grain_sown_expected: 0,
            grain_grown_expected: 0,
            grain_change_expected: 0,
            reclaim_fields_finishing: 0,
            reclaim_seasons_to_next: 0,
            industry: [
                Industry::new(Commodity::Wood),
                Industry::new(Commodity::Iron),
                Industry::new(Commodity::Weapons),
                Industry::new(Commodity::Stone),
            ],
            weapon_type: 0,
            farm_style: 0,
            tax_suppressed: false,
        }
    }

    /// True when this county belongs to no realm. Unowned counties are a real
    /// case, not an edge case: they get their own happiness bonus (§4.4),
    /// halve their migration (§5.3) and are the ten counties whose food split
    /// reproduces in `docs/kingdom.md` §4.3.
    pub fn is_unowned(&self) -> bool {
        self.owner == 0
    }

    /// The sum of the three field-usage counts is the county's field total.
    /// Over the fourteen counties of the England map the totals run 8 to 16.
    ///
    /// **Three of five.** `County_RecountFields` fills five counts and this
    /// sums the three the AI's field ladder reads; a county's waste and
    /// reclaiming fields are not in it. [`County::field_slots_used`] is the
    /// count of tiles.
    pub fn field_total(&self) -> i32 {
        self.fields_fallow + self.fields_cattle + self.fields_grain
    }

    /// The map tile in one of the twenty field slots, or `None` for an empty
/// slot. Out-of-range slots are `None`, because the
    /// callers walk `0 .. MAX_FIELDS` and a bound check reads better there.
    pub fn field_tile(&self, slot: usize) -> Option<usize> {
        match self.field_tiles.get(slot) {
            Some(&0) | None => None,
            Some(&t) => Some(t as usize),
        }
    }

    /// Put a tile in a field slot, or clear it. Returns `false` if the slot is
    /// out of range.
    pub fn set_field_tile(&mut self, slot: usize, tile: Option<usize>) -> bool {
        let Some(cell) = self.field_tiles.get_mut(slot) else { return false };
        *cell = tile.unwrap_or(0) as u16;
        true
    }

    /// Which field slot holds this tile, if any. Linear over twenty entries in
    /// stored order — the original's own search.
    pub fn field_slot(&self, tile: usize) -> Option<usize> {
        (0..MAX_FIELDS).find(|&slot| self.field_tile(slot) == Some(tile))
    }

    /// How many of the twenty slots hold a tile.
    pub fn field_slots_used(&self) -> usize {
        (0..MAX_FIELDS).filter(|&slot| self.field_tile(slot).is_some()).count()
    }

/// The neighbour ids present, as a slice. Always walked in stored
    /// order — never sorted, never hashed.
    pub fn neighbours(&self) -> &[u8] {
        let n = (self.neighbour_count as usize).min(MAX_NEIGHBOURS);
        &self.neighbours[..n]
    }

    /// Record an adjacency. Returns `false` once [`MAX_NEIGHBOURS`] is reached,
/// — the original has a fixed slot count.
    pub fn add_neighbour(&mut self, id: u8) -> bool {
        let n = self.neighbour_count as usize;
        if n >= MAX_NEIGHBOURS {
            return false;
        }
        self.neighbours[n] = id;
        self.neighbour_count += 1;
        true
    }

    /// `popBand` = `(pop - 1) / 25 + 1` (`+0xB8`). Written
    /// document states it, including at population 0 where C's truncating
    /// division makes `(0 - 1) / 25` zero and the band 1.
    pub fn compute_pop_band(&self) -> i32 {
        (self.population - 1) / 25 + 1
    }

    /// A convenience for the ration and industry code: the ration level as an
    /// index, clamped into the table.
    pub fn ration_index(&self) -> usize {
        (self.ration_achieved.max(0) as usize).min(RATION_LEVEL_COUNT - 1)
    }

    /// The three minimap overlay ratings — `FUN_00451BBA` (`0x00451BBA`),
    /// which `Minimap_DrawOverlay` calls on **every** draw before it reads
    /// them. See [`MinimapBands`].
    pub fn minimap_bands(&self) -> MinimapBands {
        // +0x01. The original divides an `i8` happiness by 20 and stores an
        // `i8`
        // happiness of -20 or worse wraps past 5 and the county is left
// uncoloured. Reproduced with the same
        // cast, so the edge behaves the same if happiness ever goes negative.
        let happiness = ((self.happiness / 20) as i8) as u8;

        // +0x02. `DAT_00553E60` is a debug toggle, zeroed by the bulk global
        // reset at `0x00497500` and flipped only inside the command dispatcher
        // at `0x004B29BE`. With it clear — the shipped game — the food rating
        // is *binary*: red when the county did not achieve the ration it was
        // asked for, and **6, meaning draw nothing at all**, when it did. The
        // debug branch spreads `ration_achieved` over bands 1..=5 instead.
        let food = if self.ration_achieved < self.ration_wanted { 0 } else { 6 };

        // +0x03. Idle townsfolk, plus one for each of jobs 0..=7 carrying more
        // workers than it can use. Understaffing either farm job below its
        // wanted floor beats everything and gives band 0; otherwise a county
        // with no slack at all is 6 (draw nothing) and one with slack is 5.
        // So the labour overlay only ever paints the two ends of the ramp.
        let mut slack = self.labour[JOB_IDLE_TOWNSFOLK];
        for job in 0..JOB_IDLE_TOWNSFOLK {
            if self.labour_useful[job] < self.labour[job] {
                slack += 1;
            }
        }
        let short = self.labour[JOB_GRAIN_FARMING] < self.labour_wanted[JOB_GRAIN_FARMING]
            || self.labour[JOB_CATTLE_FARMING] < self.labour_wanted[JOB_CATTLE_FARMING];
        let labour = if short {
            0
        } else if slack == 0 {
            6
        } else {
            5
        };

        MinimapBands { labour, food, happiness }
    }

    /// Push one field's reclamation towards [`FIELD_PROGRESS_MAX`] by at most
    /// [`crate::tables::FIELD_RECLAIM_PER_SEASON`]. Returns the new progress.
    pub fn reclaim_field(&mut self, field: usize, by: i32) -> u16 {
        let p = self.field_progress[field] as i32;
        let next = (p + by.min(crate::tables::FIELD_RECLAIM_PER_SEASON)).min(FIELD_PROGRESS_MAX);
        self.field_progress[field] = next.max(0) as u16;
        self.field_progress[field]
    }
}

