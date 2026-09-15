#![allow(unused_imports)]

mod methods;
pub use methods::*;

use super::*;

use crate::tables::{
    Commodity, Weather, FIELD_PROGRESS_MAX, JOB_CATTLE_FARMING, JOB_COUNT, JOB_GRAIN_FARMING,
    JOB_IDLE_TOWNSFOLK, RATION_LEVEL_COUNT,
};

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
    ///
    /// `docs/kingdom.md` §6 describes the flag without giving its offset, and
    /// this said *engine state* for that reason: `Unrest_UpdateAll`
    /// (`0x0044AA41`) is the only reader and writer of `+0x21`, sets it under
    /// `0x1E`, and every save on this machine carries it set only in counties
    /// below thirty. `[V]`, `crates/l2-scenario/tests/import/main.rs`;
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
    /// **`[V]`
    ///
    /// * The banker is **`Tax_CollectAll` itself** (`0x0044B59B`), not
///   `FUN_0044B4F3` — it is inside
    ///   `Territory_BlockContains`. Its last statement is
    ///   `if (realm == 0) county.purse += county.taxCollected;` against the
    /// `else` that credits `realm.gold` and the two realm accumulators.
    ///
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
    ///
    /// * **Cattle** (`FUN_0044DD4D`) walks the same range through
    ///   `Herd_BirthsAndDeaths` and stores the **first count at which births
    ///   less deaths stops being negative** — break-even — or, if the herd
    ///   cannot break even at any staffing, the least-bad count.
    ///
    /// * **Every other job** writes [`LABOUR_NO_FLOOR`]: reclamation
    ///   (`Field_ReclaimEstimate`, `0x0044C278`), castle building
    /// (`Castle_BuildEstimate`, `0x00450E46`) and the four industries
    ///   (`FUN_0044F318`) all set it to −1, meaning *no requirement*.
    pub labour_wanted: [i32; JOB_COUNT],
    /// `+0xC4 + job*0x0C + 0x08` — **the useful ceiling**: the worker count
    /// past which more workers do the job no good.
    ///
    /// **`[D]`**, and unlike [`County::labour_wanted`] this one is not only a
    /// display hint: the labour allocator (`FUN_0044F6E7`) fills each of slots
    /// 0..=7 **up to this number and no further**, and drops what is left over
    /// into [`crate::tables::JOB_IDLE_TOWNSFOLK`].
    pub labour_useful: [i32; JOB_COUNT],
    /// `+0x130 + job*0x04` — **eight percentages, one per job**
    /// allocator's only instruction about where people should go.
    ///
    /// **`[V]`.** `FUN_00450000` recomputes them from the worker counts and
    /// indexes them as `(&DAT_0053FAE0)[i * 4]` for `i` in 0..3 and again for
    /// `i` in 3..8, which is the array written out. They are **two groups that
    /// each sum to 100**, not one that sums to 100:
    ///
    /// Both defaults close: `FUN_004514F8` sets 33 / 50 / 17 and 0 / 0 / 0 /
    /// 100 / 0, and `FUN_0045158B` sets 33 / 50 / 17 and 40 / 15 / 15 / 15 / 15.
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
    ///
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
    /// `+0x18C`, `+0x190` — **what the season's ration pass priced, held until
    /// the store is actually debited.** `Ration_Apply` (`0x0044DF5F`) never
    /// touches a store; `Ration_ApplyAll` (`0x0044BF04`) copies `+0x178` here
    /// and `+0x17C` here, and `Grain_SeasonTick` (`0x0044C8AE`) opens
    /// `grain -= +0x18C` while `Herd_SeasonTick` (`0x0044D60D`) opens
    /// `herd -= +0x190`. `+0x178`/`+0x17C` are overwritten by the *next*
    /// season's preview before either tick runs, which is why the shadow pair
    /// exists at all (`docs/decisions.md` C20).
    pub grain_eaten_shadow: i32,
    pub herd_eaten_shadow: i32,
    /// `+0x198`, `+0x19C` — troops standing in the county; added to the food
    /// requirement when *Armies Eat* is on. Rebuilt from the unit array by
    /// [`crate::unit::Units::recount_county_troops`].
    pub friendly_troops: i32,
    pub enemy_troops: i32,
    /// `+0x1AD` — **the mercenary band on offer here**, 1..=12, or 0 for none.
    pub mercenary_offer: u8,
    /// `+0x1BC` — the unit slot of the army **garrisoning this county's
    /// castle**, or 0.
    pub garrison_unit: usize,
    /// `+0x2F4` — the levy surcharge, added to the happiness cost of every
    /// subsequent levy raised in this county.
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
    ///
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
    /// > The castle-build season pass (`0x00450C48`) branches on **2** to send
/// > message `0xA3` variant 1 and to skip the
    /// > garrison top-up, so 2 is *repair* and 1 is *new work*.
    ///
    /// > [`crate::siege::assault_castle_level`] reads **1** as *"fight the
    /// > castle being built"* and **2** as *"fight what a previous siege left
    /// > standing"*. And the map's info panel (`0x00414...`) prints a different
    /// > line for each. `[D]`
    pub castle_degraded: u8,
    /// `+0x1C2` — **the castle is ruined**. `L2.eng` 165 names it, and
    /// `Army_BeginSiege` refuses to lay a siege while it is set: there is
    /// nothing left to besiege.
    pub castle_ruined: bool,
    /// `+0x1F9` — the castle level **left standing** after a siege knocked one
    /// down, read only when [`County::castle_degraded`] is 2.
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
    pub castle_switch: bool,
    /// `+0x1C4` — **how far the current castle work has got, 0…100.**
    pub castle_percent: u8,
    /// `+0x1CC` / `+0x1D8` — **man-work still outstanding**
    /// work was ordered at.
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
    ///
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
    pub fields_reclaiming: i32,
    /// `+0x208` — -100..=100.
    pub fertility: i32,
    /// `+0x21B`.
    pub weather: Weather,
    /// `+0x21D` — the accumulator the weather band is computed from.
    pub dryness: i32,
    /// `+0x224` — sacks in store.
    pub grain: i32,
    /// `+0x240`, `+0x244`, `+0x248` — **the seed, the standing crop, and this
    /// season's harvest.**
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
    ///
    /// destroy a field, then paint two more
    /// `docs/decisions.md` C195.
    pub fields_grain_standing: i32,
    /// `+0x1A7` — `Grain_Sow` could not afford one sack a field and fell back
    /// to sowing a token handful.
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
    /// numbers `crates/l2-kingdom/tests/reproduction/main.rs` holds against the
    /// England turn-one save.
    pub herd_births_expected: i32,
    pub herd_deaths_expected: i32,
    pub herd_change_expected: i32,
    /// `+0x24C` and `+0x278` — **what last season's weather and last season's
    /// random event did to the grain**: the two figures `Panel_JobGrain`
    /// (`0x00413590`) and `TileInfo_DrawGrain` (`0x0041CB3A`) print under
    /// `L2.eng` group 77.
    ///
    /// `grain_event_change` is the **magnitude** of the event's percentage of the
    /// store. The painter chooses 77/25 *"eaten by rats."* or 77/26 *"found as
/// surplus."* from the event id, and
    /// `Msg_DrawWindow` (`0x0047309E`) prints the same number in the *Rats!!*
    ///
    /// Both written by [`crate::land::grain_season_tick`], which is
    /// `Grain_SeasonTick` (`0x0044C8AE`): the event figure zeroed at the top of
    /// every county's tick, the weather figure in whichever season branch runs.
    pub grain_weather_change: i32,
    pub grain_event_change: i32,
    /// `+0x270` and `+0x274` — **the same pair for the herd**, printed by
    /// `Panel_JobCattle` (`0x00413B30`) and `TileInfo_DrawHerd` (`0x0041D299`).
    ///
    /// Written by [`crate::land::herd_season_tick`], which is `Herd_SeasonTick`
    /// (`0x0044D60D`). Zero in every save on this machine, for the same reason.
    pub herd_weather_change: i32,
    pub herd_event_change: i32,
    /// `+0x230`, `+0x2FC` and `+0x22C` — **the grain row's three forecasts**,
    /// and `crop[2]` is the fourth.
    ///
    /// They are the **tail** of `Grain_LabourEstimate` (`0x0044D374`), after the
    /// search loop that [`crate::land::grain_labour_estimate`] reproduces:
    pub grain_sown_expected: i32,
    pub grain_grown_expected: i32,
    pub grain_change_expected: i32,
    /// `+0x20C` and `+0x214` — **the reclamation row's two figures**, written by
    /// [`crate::land::reclaim_preview`], which is `Field_ReclaimEstimate`'s
    /// tail.
    pub reclaim_fields_finishing: i32,
    pub reclaim_seasons_to_next: i32,
    /// `+0x294 + c*0x18` — per-commodity production records.
    pub industry: [Industry; 4],
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

