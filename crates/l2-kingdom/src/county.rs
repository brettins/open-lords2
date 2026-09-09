//! The county record — `docs/kingdom.md` §1.
//!
//! The original is 768 bytes at `0x0053F9B0`, 17 of them, and 52 of its 201
//! referenced offsets are identified. **This is not that layout.** We are
//! writing a new engine, not a memory-compatible clone, so what is reproduced
//! here is the *semantics* of the 52 identified fields, with the offset each
//! one came from recorded in a comment so a future differential test against
//! the original can find it again.
//!
//! What *is* reproduced exactly is the **array bounds**, because those are real
//! constraints the original enforces and a reimplementation that quietly allows
//! an eighteenth county or a twenty-first field is no longer simulating the
//! same game.
//!
//! Nothing here is `HashMap`-shaped and nothing is indexed by anything but a
//! small integer: the whole record is walked in index order, every season, on
//! every peer (`docs/netcode.md` §3).

use crate::tables::{Commodity, Weather, FIELD_PROGRESS_MAX, JOB_COUNT, RATION_LEVEL_COUNT};

/// `g_counties` is 17 records and **index 0 is never a county**
/// (`docs/kingdom.md` §1). So the usable ids are 1..=16 and the array bound is
/// 17.
pub const MAX_COUNTIES: usize = 17;

/// The highest county id that can be owned, played or referenced.
pub const MAX_COUNTY_ID: u8 = (MAX_COUNTIES - 1) as u8;

/// A county owns up to twenty fields — `g_countyFieldTiles` is
/// `17 x 20 x u32`, and save block 12 is 1,360 bytes = 17 x 80 exactly
/// (`docs/kingdom.md` §7.2).
pub const MAX_FIELDS: usize = 20;

/// The inflow list at `+0x48 … +0x57` is sixteen bytes (`docs/kingdom.md`
/// §1.2).
pub const MAX_INFLOW_SOURCES: usize = 16;

/// [`County::labour_wanted`] for a job that asks for nobody in particular.
///
/// **`[D]`.** Every writer but grain's and cattle's stores −1, and the two
/// readers both compare `labour < wanted`, which −1 can never satisfy.
pub const LABOUR_NO_FLOOR: i32 = -1;

/// [`County::labour_useful`] for a job that can always use one more.
///
/// **`[D]`.** `FUN_0044F318` writes 100,000 for iron, stone and wood — every
/// industry except the blacksmith, whose output is capped by the iron store.
/// It is a real number rather than a sentinel: the allocator compares against
/// it directly, and a county would have to hold a hundred thousand people
/// before it bound anything.
pub const LABOUR_UNBOUNDED: i32 = 100_000;

/// [`County::labour_useful`] for a job whose estimate has never run.
///
/// **`[D]`.** The allocator (`FUN_0044F6E7`) opens by reading all eight
/// ceilings and rewriting this value to **0** — so an unset ceiling allocates
/// nobody, rather than everybody.
pub const LABOUR_UNSET: i32 = 999_999;

/// A county cannot neighbour more counties than there are counties. The
/// original stores the count at `+0x5A` and the ids from `+0x5C`; the array
/// length is not stated, so this is the tightest bound the county array itself
/// implies.
pub const MAX_NEIGHBOURS: usize = MAX_COUNTIES - 2;

/// The `L2.eng` group 65 population-change reasons, in the order the five
/// strings appear. `docs/kingdom.md` §1.2, county `+0x5B`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum ChangeReason {
    #[default]
    None = 0,
    Births = 1,
    Deaths = 2,
    Emigration = 3,
    Immigration = 4,
}

impl ChangeReason {
    pub fn name(self) -> &'static str {
        match self {
            ChangeReason::None => "None",
            ChangeReason::Births => "Births",
            ChangeReason::Deaths => "Deaths",
            ChangeReason::Emigration => "Emigration",
            ChangeReason::Immigration => "Immigration",
        }
    }
}

/// A population change smaller than this percentage is not attributed to any
/// cause — `changeReason` stays `None`. `docs/kingdom.md` §1.2.
pub const CHANGE_REASON_MIN_PCT: i32 = 6;

/// One commodity's production record, county `+0x290 + c*0x18`.
/// `docs/kingdom.md` §1.3 and §7.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Industry {
    /// What the last `Industry_Produce` pass produced — the difference between
    /// the running total at `+0x2A0` and its snapshot at `+0x2A4`.
    pub output: i32,
    /// County `+0x294`, the efficiency percentage. It **ramps**: see
    /// [`crate::industry::efficiency_ramp`], which is `FUN_0044F248`.
    pub efficiency: i32,
    /// County `+0x29E` — the worker count this industry can absorb at full
    /// value. Above it the ramp's increment is scaled down by
    /// `capacity / workers`, so piling on more serfs raises the output but
    /// slows the improvement. `[D]` — the field is read by the ramp and
    /// written by the industry driver from county `+0x108`, whose meaning was
    /// not traced.
    pub capacity: i32,
    /// County `+0x295` — the county has this resource in the ground at all
    /// (ore, stone, forest). Weapons ignore it.
    pub has_resource: bool,
    /// County `+0x297` — this industry is switched on. Both this and
    /// [`Industry::has_resource`] gate the `resourceLimit`, so an industry
    /// missing either produces nothing however many workers it is given.
    pub enabled: bool,
    /// County `+0x296` — a countdown. While non-zero the pass produces nothing
    /// at all, zeroes the running total and decrements; at zero the industry is
    /// reinstated. `[D]` — a depleted seam or a razed workshop would both fit
    /// and neither is established.
    pub disabled_seasons: i32,
    /// The running total at `+0x2A0`, which the pass adds this season's output
    /// to rather than replacing.
    pub total: i32,
}

impl Industry {
    pub fn new(c: Commodity) -> Industry {
        Industry {
            output: 0,
            efficiency: c.base_efficiency(),
            capacity: 0,
            has_resource: true,
            enabled: true,
            disabled_seasons: 0,
            total: 0,
        }
    }
}

/// A county.
///
/// Field names follow `docs/kingdom.md`'s names, and each carries the original
/// offset it was identified at. Fields *not* in the document's table of 52 are
/// marked **engine state** — they are things the rules provably need that the
/// document does not place, and they are almost certainly among the ~150
/// untraced offsets rather than inventions.
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
    /// `+0x219` — the total happiness this county has **ever** been given by
    /// ale, which is what caps the bonus at five. Nothing in the binary resets
    /// it, so the cap is for the whole game rather than per season. See
    /// [`crate::tables::ALE_HAPPINESS_MAX`].
    pub ale_happiness_given: i32,
    /// `+0x20` — 0..=4. At 4 the county revolts (§6).
    pub unrest: u8,
    /// **Engine state.** The "warned" flag `Unrest_UpdateAll` clears at
    /// happiness >= 30, so message `0x92` fires once rather than every season.
    /// `docs/kingdom.md` §6 describes the flag without giving its offset.
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
    /// `+0xBC` — what the treasury actually banks.
    pub tax_collected: i32,
    /// `+0xC0` — group 86 index 2, *"People pay"*.
    pub tax_shown: i32,
    /// `+0xC4 + job*0x0C` — workers assigned to each of nine jobs.
    pub labour: [i32; JOB_COUNT],
    /// `+0xC4 + job*0x0C + 0x04` — **the wanted floor**: how many workers this
    /// job needs before it stops going backwards.
    ///
    /// **`[D]`.** The second word of the twelve-byte labour record, and the
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
    ///   (`Castle_BuildEstimate`, `0x00450E46`) and the four industries
    ///   (`FUN_0044F318`) all set it to −1, meaning *no requirement*.
    ///
    /// Two things read it, and both are interface: `Panel_JobDetail` colours
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
    /// away. [`LABOUR_UNSET`] means the estimate has never run and the
    /// allocator reads it as 0.
    pub labour_useful: [i32; JOB_COUNT],
    /// `+0x130 + job*0x04` — **eight percentages, one per job**, and the
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
    /// **industry** rather than to the farm.
    ///
    /// **`[D]`.** `FUN_0044F6E7` opens `industry = Pct(population, +0x08);
    /// farm = population - industry` and fills the two halves from their own
    /// percentages. `FUN_0044FF4A` writes it back from what was actually
    /// assigned, counting **half** the idle as industry:
    /// `PctOf(pop - grain - cattle - reclamation - idle + idle/2, pop)`.
    /// A fresh county starts on 25 (`FUN_00451150`).
    pub industry_share: i32,
    /// `+0x90 + i*2` — reclamation progress of each field, 0..=800.
    pub field_progress: [u16; MAX_FIELDS],
    /// `+0x15D` — 0..=5, the level `Ration_Apply` actually managed to feed.
    pub ration_achieved: i32,
    /// `+0x15E` — what the player asked for.
    pub ration_wanted: i32,
    /// `+0x15F` — percentage of the food requirement taken from livestock
    /// rather than grain.
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
    /// A one-slot cache rather than a list: `Mercenary_AdvanceAll` rewrites it
    /// every season with the lowest-numbered unhired band standing in the
    /// county, so if two land together the higher-numbered one is invisible.
    /// See [`crate::mercenary`].
    pub mercenary_offer: u8,
    /// `+0x1BC` — the unit slot of the army **garrisoning this county's
    /// castle**, or 0.
    ///
    /// Half of the pair that decides whether a county can simply be walked
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
    /// `+0x1C1` — the type under construction.
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
    /// > message `0xA3` variant 1 rather than variant 0 and to skip the
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
    /// **Engine state.** `Castle_BuildTick` needs somewhere to accumulate
    /// progress against [`crate::tables::CASTLE_WORKFORCE`]; the document names
    /// the pass and the table but not the counter.
    pub castle_progress: i32,
    /// `+0x1FB`, `+0x1FC`, `+0x1FD` — percentage swings to population, grain
    /// and herd from a random event.
    pub event_population_pct: i32,
    pub event_grain_pct: i32,
    pub event_herd_pct: i32,
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
    /// `+0x203` — fields in no use: terrain `0`, and the two blighted terrains
    /// `0x17` and `0x18` a drought or a flood leaves behind for one season.
    pub fields_waste: i32,
    /// `+0x204` — fields under reclamation.
    ///
    /// The one count with a *rule* attached rather than a display: it is what
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
    /// once a year, at sowing, as the sacks that actually went into the
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
    /// year's sowing. `+0x206` is a second copy of the same number, read only
    /// by the tile graphic.
    pub fields_grain_sown: i32,
    /// `+0x1A7` — `Grain_Sow` could not afford one sack a field and fell back
    /// to sowing a token handful.
    ///
    /// `Grain_SeasonTick` reads it immediately afterwards and records the
    /// county's field usage as **1** rather than `fieldsGrain` when it is set.
    /// It is deliberately *not* cleared on `Grain_Sow`'s two early exits — no
    /// store, or nobody on the fields — so a county that sowed nothing at all
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
    /// **Stored rather than derived**, because the original stores it and the
    /// difference is observable: `FUN_0044D913` recomputes it at the *end* of
    /// the herd's tick, so a season's births and deaths are worked out at the
    /// crowding the herd had when the season began. See
    /// [`crate::land::herd_crowding`].
    pub herd_crowding: i32,
    /// `+0x268`, `+0x26C` and `+0x258` — next season's forecast: group 77's
    /// *"Calf births expected"*, *"Cow deaths expected"* and *"Change due to
    /// farming"*. Written by [`crate::land::herd_preview`], and the three
    /// numbers `crates/l2-kingdom/tests/reproduction.rs` holds against the
    /// England turn-one save.
    pub herd_births_expected: i32,
    pub herd_deaths_expected: i32,
    pub herd_change_expected: i32,
    /// `+0x290 + c*0x18` — per-commodity production records.
    pub industry: [Industry; 4],
    /// **Engine state.** Which weapon the blacksmith is making.
    /// `docs/kingdom.md` §7.4 says weapons are credited to
    /// `realm +0x140 + type*4` without saying what picks `type`.
    pub weapon_type: usize,
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
            castle_switch: false,
            castle_progress: 0,
            event_population_pct: 0,
            event_grain_pct: 0,
            event_herd_pct: 0,
            field_tiles: [0; MAX_FIELDS],
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
            sow_shortfall: false,
            herd: 0,
            // The lowest band: density 0 is at the bottom of it, and a county
            // with no pasture is pushed to the top band by `herd_crowding` the
            // first time the herd ticks.
            herd_crowding: crate::tables::HERD_CROWDING[0].1,
            herd_births_expected: 0,
            herd_deaths_expected: 0,
            herd_change_expected: 0,
            industry: [
                Industry::new(Commodity::Wood),
                Industry::new(Commodity::Iron),
                Industry::new(Commodity::Weapons),
                Industry::new(Commodity::Stone),
            ],
            weapon_type: 0,
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
    /// slot. Out-of-range slots are `None` rather than a panic, because the
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

    /// The neighbour ids actually present, as a slice. Always walked in stored
    /// order — never sorted, never hashed.
    pub fn neighbours(&self) -> &[u8] {
        let n = (self.neighbour_count as usize).min(MAX_NEIGHBOURS);
        &self.neighbours[..n]
    }

    /// Record an adjacency. Returns `false` once [`MAX_NEIGHBOURS`] is reached,
    /// rather than growing — the original has a fixed slot count.
    pub fn add_neighbour(&mut self, id: u8) -> bool {
        let n = self.neighbour_count as usize;
        if n >= MAX_NEIGHBOURS {
            return false;
        }
        self.neighbours[n] = id;
        self.neighbour_count += 1;
        true
    }

    /// `popBand` = `(pop - 1) / 25 + 1` (`+0xB8`). Written exactly as the
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

    /// Push one field's reclamation towards [`FIELD_PROGRESS_MAX`] by at most
    /// [`crate::tables::FIELD_RECLAIM_PER_SEASON`]. Returns the new progress.
    pub fn reclaim_field(&mut self, field: usize, by: i32) -> u16 {
        let p = self.field_progress[field] as i32;
        let next = (p + by.min(crate::tables::FIELD_RECLAIM_PER_SEASON)).min(FIELD_PROGRESS_MAX);
        self.field_progress[field] = next.max(0) as u16;
        self.field_progress[field]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_county_is_unowned_and_on_normal_rations() {
        let c = County::new();
        assert!(c.is_unowned());
        assert_eq!(c.ration_wanted, 3);
        assert_eq!(c.ration_achieved, 3);
        assert_eq!(c.ration_split, 100);
        assert_eq!(c.weather, Weather::Cloudy);
    }

    /// `docs/kingdom.md` §9: `(435-1)/25 + 1 = 18` and `(456-1)/25 + 1 = 19`,
    /// which are the values stored in the England turn-one fixture.
    #[test]
    fn pop_band_reproduces_the_two_values_in_the_save() {
        let mut c = County::new();
        c.population = 435;
        assert_eq!(c.compute_pop_band(), 18);
        c.population = 456;
        assert_eq!(c.compute_pop_band(), 19);
        c.population = 417;
        assert_eq!(c.compute_pop_band(), 17);
    }

    #[test]
    fn neighbours_are_bounded_rather_than_growing() {
        let mut c = County::new();
        for i in 0..MAX_NEIGHBOURS {
            assert!(c.add_neighbour(i as u8 + 1), "neighbour {i} should fit");
        }
        assert!(!c.add_neighbour(99), "the seventeenth should be refused");
        assert_eq!(c.neighbours().len(), MAX_NEIGHBOURS);
    }

    #[test]
    fn reclaiming_a_field_takes_four_seasons_and_stops_at_eight_hundred() {
        let mut c = County::new();
        let mut seen = Vec::new();
        for _ in 0..6 {
            seen.push(c.reclaim_field(0, crate::tables::FIELD_RECLAIM_PER_SEASON));
        }
        assert_eq!(seen, vec![200, 400, 600, 800, 800, 800]);
    }

    #[test]
    fn a_season_cannot_reclaim_more_than_a_quarter_of_a_field() {
        let mut c = County::new();
        // Ask for the whole thing at once; the cap is the rule.
        assert_eq!(c.reclaim_field(0, 10_000), 200);
    }

    #[test]
    fn the_field_total_is_the_sum_of_the_three_usage_counts() {
        let mut c = County::new();
        c.fields_fallow = 3;
        c.fields_cattle = 4;
        c.fields_grain = 6;
        assert_eq!(c.field_total(), 13);
        // The England map's fourteen counties run 8 to 16 (docs/kingdom.md §7.2).
        assert!((8..=MAX_FIELDS as i32).contains(&c.field_total()));
    }
}
