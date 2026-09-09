# The kingdom layer

The turn-based half of `Lords2.exe`: counties, seasons, population, food, happiness,
taxation, crops, livestock, industry, castles and wages. Where [`battle.md`](battle.md)
covers what happens when two armies meet, this covers everything that decides *which*
armies exist.

Rules are much harder to validate than formats, so the status legend from `battle.md`
applies here unchanged and is meant literally:

* **[V] verified** — read out of the binary *and* cross-checked against a second,
  independent source: an `L2.eng` UI string, a statement in the game's own in-game help,
  the printed manual, an exact invariant over shipped data, or a reproduction from the
  shipped `lastturn.sav`.
* **[D] decompiler-only** — a straightforward reading of decompiled C with no second
  source. Probably right about *what the code does*; the *name* may be wrong.
* **[I] inferred** — consistent with everything measured, not proven.

`docs/decisions.md` C3 and C4 are the failure modes: a plausible story assembled from
decompiler output, and a number quoted across a scope boundary without re-counting.
Every count below was counted here, and where a sample is small this document says so.

Addresses are from the GOG Windows build (1,031,680 bytes, `ImageBase 0x400000`, no
ASLR). Every name used here is in [`symbols.json`](symbols.json) and applied to the
Ghidra database.

---

## 0. The headline

**A kingdom is 17 county records of 768 bytes at `0x0053F9B0` and 6 realm records of 352
bytes at `0x0057BF00`.**

| | Address | Stride | Count | What it is |
|---|---|---|---|---|
| **counties** | `0x0053F9B0` | `0x300` | 0 … 16 | the thing the county panels show; index 0 unused |
| **realms** | `0x0057BF00` | `0x160` | 0 … 5 | one per player; index 0 unused, 1 … 5 are the players |
| county field tiles | `0x0053EA00` | `0x50` | 17 × 20 × u32 | which map tiles are this county's farm fields |
| units (armies, merchants, transports) | `0x0052F0B0` | `0x1A4` | 1 … 150 | already documented in [`plane4.md`](formats/plane4.md) |

### The two tables that pin all of this

This subsystem has no equivalent of the battle's debug overlay, but it has something
almost as good: **two descriptor tables that the binary itself carries, listing the game's
state arrays with their addresses, counts and strides.**

**`g_saveBlocks` (`0x004DE960`)** — 225 `{u32 address, u32 length}` records terminated by a
zero length. `Save_Write` (`0x004ADE93`) writes each block to the save file back to back
and then appends `castles.dat`. **[V]**

```
  0  0x00522f90    32768   g_tiles          4096 x 8
  1  0x005440e0    51200   g_battlefield    6400 x 8
  2  0x00553d50      264                       6 x 0x2c
  3  0x0057bf00     2112   g_realms            6 x 0x160
  4  0x0053f9b0    13056   g_counties         17 x 0x300
  5  0x0052f0b0    63420   g_units           151 x 0x1a4
  ...
```

**`g_syncBlocks` (`0x004D5B10`)** — seven 12-byte `{count, stride, firstComparedOffset}`
records used by the multiplayer desync detector `Sync_CompareState` (`0x0043FAA4`), with a
parallel pointer table at **`0x004D5B70`**:

| # | count | stride | compare from | pointer | array |
|---|---:|---:|---:|---|---|
| 1 | **17** | **0x300** | 5 | `0x0053F9B0` | counties |
| 2 | 6 | 0x160 | 6 | `0x0057BF00` | realms |
| 3 | 151 | 0x1A4 | 0 | `0x0052F0B0` | units |
| 4 | 81 | 0x1B0 | 18 | `0x00554480` | battle figures |
| 5 | 101 | 0x4C | 4 | `0x0057A100` | missiles |
| 6 | 81 | 0x34 | 0 | `0x00566520` | battle units |
| 7 | — | — | — | `0x005440E0` | battlefield |

**[V]** Rows 4, 5 and 6 are exactly the counts and strides `battle.md` derived from loop
bounds — an independent confirmation of that work from a completely different part of the
binary. Rows 1 and 2 are this document's headline, stated by the binary rather than
inferred.

### The invariant that closes it

```
$ node tools/kingdom/savedump.js layout "F:/games/Lords of the Realm II"
225 blocks, 267028 bytes
+ castles.dat 16 x 12800 = 471828
  OK   lastturn.sav is 471828 bytes
  OK   Castles.dat is 204800 bytes
```

**[V]** The block table sums to 267,028; plus the sixteen 12,800-byte castle plans that is
471,828, which is byte for byte the size of the `lastturn.sav` in the install. That is the
end-offset invariant this layer was missing: if any address or length in the table were
misread the total would not close.

It also means **a save file is a raw dump of the live `.data` state**, so every claim in
this document about a county field can be tested against a real game — and §9 does that.

---

## 1. The county record — `g_counties`, `0x0053F9B0`, stride `0x300`

201 distinct byte offsets inside a county record are referenced somewhere in the binary,
the highest being `+0x2FC`; nothing spills past `+0x300`. Fifty-two are identified below.
The rest are untraced.

The county array is indexed **1 … `g_countyCount`**; record 0 is never a county, and
`Sync_CompareState` skips bytes `+0x00 … +0x04` of every record, so those five bytes are
local UI state rather than simulation state. **[V]**

### 1.1 Identity, happiness and health

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x00` | u8 | eventFired | [D] | set by `Event_RollAll` when this county drew a random event. |
| `+0x05` | u8 | **owner** | [V] | realm index 1 … 5; **0 = unowned**. The first byte the desync comparator checks. |
| `+0x09` | i8 | **healthBand** | [V] | 0 … 4 = *Diseased, Sick, Average, Good, Perfect* — `L2.eng` group 20, five strings. |
| `+0x0B` | i8 | healthMeter | [V] | 0 … 100. Banded into `+0x09` through `g_healthBandLadder`. |
| `+0x0C` | i8 | **happiness** | [V] | 0 … 100. Group 85 index 7, *"This Season"*. |
| `+0x0D` | i8 | **happinessLast** | [V] | group 85 index 1, *"Last season"*. |
| `+0x0E` | i8 | dHapTax | [V] | the tax term, applied this turn. |
| `+0x0F` | i8 | dHapTaxLocal | [D] | the "this county" half of the tax effect, for the tax panel. |
| `+0x10` | i8 | dHapHealth | [V] | the health term. |
| `+0x11` | i8 | dHapRation | [V] | the ration term. |
| `+0x12` … `+0x15` | i8 | shown tax / ration / health / army | [V] | display copies taken at the moment the update ran — group 85 indices 2, 3, 4, 5. |
| `+0x16` | i8 | taxHapOther | [V] | group 86 index 4, *"Other counties"*; summed across the realm into realm `+0x28`. **`g_taxHappinessOther[rate]`, not `5 − rate`** — §4.1. |
| `+0x17` | i8 | shownEvents | [V] | group 85 index 9, *"From events"*. |
| `+0x18` | i8 | happinessAvg | [V] | group 85 index 8, *"Average happiness"* = `+0x1C / g_turnCount`. |
| `+0x1C` | i32 | happinessSum | [V] | running total of `+0x0C` over all turns. |
| `+0x194` | i32 | shownAle | [V] | group 85 index 6, *"From ale"*. |
| `+0x20` | u8 | **unrest** | [V] | 0 … 4. At 4 the county revolts. §6. |

**[V] What names these fields is the game's own panel.** `Panel_Happiness` (`0x004116FB`)
draws `L2.eng` group 85 — *"Happiness in / Last season / From taxes / From ration / From
health / From army / From ale / This Season / Average happiness / From events"* — next to
exactly these offsets, in that order. It is the closest thing this half of the game has to
the battle debug overlay.

### 1.2 Population

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x24` | i32 | **population** | [V] | group 73 index 7, *"This Season"*. |
| `+0x28` | i32 | popLast | [V] | *"Last season"*. |
| `+0x2C` | i32 | popChangePct | [D] | `|pop − popLast| * 100 / popLast`. |
| `+0x30` | i32 | births | [V] | *"Births"*. |
| `+0x34` | i32 | deaths | [V] | *"Deaths"*, drawn negated. |
| `+0x38` | i32 | army | [V] | *"Army"*. Zeroed by `Population_UpdateAll` and filled elsewhere. |
| `+0x3C` | i32 | emigrants | [V] | *"Emigrants to"*, with the destination county in `+0x58`. |
| `+0x40` | i32 | immigrants | [V] | *"Total immigrants"*. |
| `+0x44` | i32 | largestInflow | [D] | biggest single incoming stream, source county in `+0x59`. |
| `+0x48` … `+0x57` | u8×16 | inflowSources | [D] | see the bug note in §5.3. |
| `+0x5A` | u8 | neighbourCount | [V] | number of adjacent counties. |
| `+0x5C` … | u8×n | neighbours | [V] | their ids. Read by migration and by the regional weather swing. |
| `+0x5B` | u8 | changeReason | [V] | 0 none, 1 births, 2 deaths, 3 emigration, 4 immigration; suppressed below a 6 % change. `L2.eng` group 65 has exactly five strings in that order. |
| `+0xB8` | u8 | popBand | [V] | `(pop − 1) / 25 + 1`. |
| `+0x6C` `+0x6D` | u8 | anchorX, anchorY | [V] | the county's anchor tile; already named in `symbols.md`. |

### 1.3 Money, food and land

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0xB9` | u8 | **taxRate** | [V] | group 86 index 1, *"Tax rate"*. |
| `+0xBC` | i32 | taxCollected | [V] | what the treasury actually banks. |
| `+0xC0` | i32 | taxShown | [V] | group 86 index 2, *"People pay"*. |
| `+0xC4 + job*0x0C` | i32 | labour | [V] | workers assigned to each of **nine** jobs. §7, §14. |
| `+0xC4 + job*0x0C + 4` | i32 | labourWanted | [V] | the job's **wanted floor** — below it the count is drawn red and the shortfall appears as unselectable icons. −1 means no floor. §14. |
| `+0xC4 + job*0x0C + 8` | i32 | labourUseful | [V] | its **useful ceiling** — the allocator fills the job to it and no further. 100,000 means none; 0 means the county has no such resource. §14. |
| `+0x130 + job*4` | i32×8 | labourShare | [V] | eight percentages, jobs 0 … 7, in **two groups that each sum to 100**: farm (0 … 2) and industry (3 … 7). §14. |
| `+0x08` | i8 | industryShare | [D] | the percentage of the county given to industry rather than to the farm. Starts at 25. §14. |
| `+0x90 + i*2` | u16×20 | fieldProgress | [V] | reclamation progress of each field, 0 … 800. §7.2. |
| `+0x15D` | i8 | **rationAchieved** | [V] | 0 … 5 = *None, Quarter, Half, Normal, Double, Triple* — `L2.eng` group 21, six strings. |
| `+0x15E` | i8 | rationWanted | [V] | what the player asked for; group 87, *"Wanted:" / "Achieved:"*. |
| `+0x15F` | i8 | rationSplit | [V] | percentage of the food requirement taken from livestock rather than grain. |
| `+0x178` | i32 | grainEaten | [D] | sacks; see §4.3 for what does and does not reproduce. |
| `+0x17C` | i32 | herdEaten | [D] | head slaughtered. |
| `+0x180` `+0x184` | i32 | grain / herd available | [D] | the caps applied to the two above. |
| `+0x198` `+0x19C` | i32 | friendly / enemy troops | [V] | troops standing in the county; added to the food requirement when *Armies Eat* is on. |
| `+0x1C0` | u8 | **castleType** | [V] | 0 none, 1 wooden palisade, 2 motte and bailey, 3 Norman keep, 4 stone castle, 5 royal castle — `L2.eng` group 71 lists exactly those five, and group 103 indices 19–24 name them for the options screen. |
| `+0x1C1` | u8 | castleBuilding | [D] | the type under construction. |
| `+0x1FB` `+0x1FC` `+0x1FD` | i8 | event modifiers | [V] | percentage swings to population, grain and herd from a random event. Group 77 indices 19–26 name them (*"eaten by rats"*, *"taken by wolves"*, *"found as surplus"* …). |
| `+0x1FF` | u8 | fieldsFallow | [I] | read positively by the fertility rule. |
| `+0x200` | u8 | fieldsCattle | [I] | not read by the fertility rule at all. |
| `+0x201` | u8 | **fieldsGrain** | [V] | the field count `Grain_Sow` multiplies by sacks-per-field, and the term the fertility rule subtracts. |
| `+0x208` | i32 | fertility | [V] | −100 … +100. §7.1. |
| `+0x21B` | u8 | **weather** | [V] | 0 … 5 = *Frost, Drought, Sunny, Cloudy, Storms, Flooding* — `L2.eng` group 66, six name/effect pairs. |
| `+0x21D` | i8 | dryness | [V] | the accumulator the weather band is computed from. §7.3. |
| `+0x224` | i32 | **grain** | [V] | sacks in store. |
| `+0x240` `+0x244` `+0x248` | i32 | crop | [D] | the growing crop at its three stages. |
| `+0x250` | i32 | **herd** | [V] | head of livestock. |
| `+0x290 + c*0x18` | — | industry | [D] | per-commodity production records. `+0x294` efficiency (`i8`), `+0x295` the county has this resource, `+0x296` a disablement countdown, `+0x297` the industry is switched on, `+0x29C` last season's efficiency, `+0x29E` (`i16`) the worker capacity the ramp scales against, `+0x2A0` a running output total and `+0x2A4` its snapshot. §7.4. |
| `+0x290` | u8 | **weaponType** | [D] | which weapon the blacksmith makes, indexing `g_weaponCost`. It shares its address with industry record 0's first byte, which is odd and is what the code does. |
| `+0x1A7` | u8 | sowShortfall | [D] | set when `Grain_Sow` could not afford one sack a field and fell back. §7.1. |
| `+0x1A8` | u8 | **taxRobbed** | [V] | non-zero after the *"Stop thief!"* event; `Tax_CollectAll` then takes nothing. §4.1, §8.1. |
| `+0x1AA` | i16 | **eventId** | [V] | the id of the random event that fired, which is also its `L2.eng` group. §8.1. |
| `+0x1F4` | i32 | neutralPurse | [D] | where an unowned county's tax goes. §4.1. |
| `+0x1FE` | u8 | farmStyle | [D] | the AI lord's farming style, copied in by AI step 5. §8.2. |
| `+0x219` | u8 | **aleGiven** | [V] | the total happiness this county has ever been given by ale; caps the bonus at 5 and is never reset. §4.4. |
| `+0x21C` | u8 | weatherLast | [D] | the previous season's band, saved at the top of `Weather_UpdateAll`. |

---

## 2. The realm record — `g_realms`, `0x0057BF00`, stride `0x160`

Six records; index 0 is unused and 1 … 5 are the players. `battle.md` already refers to
this array's `+0x05` without naming the array.

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x00` | i32 | aiStep | [V] | program counter through the AI's turn: 0 is the initialisation, 1 … 14 the handlers, then idle steps up to `15 + 2×realm`; **999 is written when it finishes and 1000 is what lands in the record**. §3.2. |
| `+0x04` | u8 | **strength** | [V] | **not a flag.** `3 × ownedCounties + 1 × armies`, rebuilt by AI step 0; the realm is eliminated when it is zero, and every other site only tests it against zero. §8.3. |
| `+0x05` | u8 | **isHuman** | [V] | when set, the AI turn machine is skipped entirely. This is the same byte `battle.md` §6.2 could not explain the meaning of; it means "a person is driving this realm". |
| `+0x07` | u8 | **lord** | [V] | 0 for the human and **1 … 4 for the four AI lords — the Knight, the Baron, the Countess and the Bishop**, which is `L2.eng` group 7 exactly; **6 when eliminated**. Indexes `g_aiPersonality` (four records) and `g_aiGoldGrant` (five rows, row 0 being the human). **This is not the realm index**: setup draws the lord from `g_lordChoice` and the realm's colour from `+0x0A` separately. [`diplomacy.md`](diplomacy.md) §0. |
| `+0x0C` | i32 | meanHappiness | [V] | mean over the realm's counties, rebuilt by AI step 14. Score input ×2. |
| `+0x10` `+0x14` `+0x18` | i32 | population total, mean, previous total | [V] | rebuilt by AI step 14. `+0x10` is a score input ÷10. |
| `+0x28` | i8 | taxHapEmpire | [V] | sum of every owned county's `+0x16`; added to every county's tax happiness term. **A signed byte summed over up to 16 counties — it can overflow.** |
| `+0x29` | u8 | countyCount | [V] | owned counties, rebuilt by AI step 14. Selects between the two AI gold-grant tables **and** the goods-grant tier. §8.2. |
| `+0x2B` | u8 | rank | [V] | 1 … 5 from `Score_RankRealms`. |
| `+0x2C` | u8 | armyCount | [V] | rebuilt by AI step 14. |
| `+0x4C` | i32 | — | — | the sixth score input, **unidentified**, and the heaviest weighted. §8.3. |
| `+0x50` | i32 | score | [V] | recomputed every turn. §8.3. |
| `+0x54` | i32 | totalMen | [V] | over the realm's armies, rebuilt by AI step 14. Score input ÷5. |
| `+0x58` | i32 | meanHealth | [V] | mean health meter over the realm's counties. Score input ×2. |
| `+0x60` | i32 | shareOfMapPct | [V] | `PctOf(ownedCounties, g_countyCount)`. Score input ×10 — the heaviest identified term. |
| `+0xFC` | i32 | wages | [V] | this season's army bill. §7.4. |
| `+0x118` | i32 | **gold** | [V] | the treasury. |
| `+0x120` `+0x128` `+0x130` | i32 | iron, stone, wood | [V] | the realm-wide stockpiles `Industry_Produce` credits. `L2.eng` group 70 is *"Gold, Arms, Iron, Stone, Wood"*. |
| `+0x124` `+0x12C` `+0x134` | i32 | iron, stone, wood at the start of the season | [D] | snapshotted by the industry driver before the four passes run. |
| `+0x140 + t*4` | i32×6 | weapons | [V] | one counter per weapon type. |
| `+0x158` | u8 | bankruptStage | [V] | 0 … 5, the escalation in `Wages_PayAll`. **Wraps to 0 after the mutiny rather than saturating.** §7.4. |

---

## 3. The turn machine

### 3.1 Seven phases

`Turn_Tick` (`0x0049A010`) runs once per frame and dispatches on `g_turnPhase`
(`0x00569584`), counting `g_turnPhaseStep` (`0x0053F658`) up on every call. Each phase
kicks off work on its first step and then waits for the units it started to stop moving;
`Turn_AdvancePhase` (`0x0049CE51`) moves on, wrapping 7 → 1.

| Phase | First step | Waits for | What it is |
|---:|---|---|---|
| 1 | `AI_SetTaxRates(0)`, then `AI_ManageFields(0)` | 3 steps | **Neutral counties.** Realm 0 — the unowned counties — get their tax rates and fields set. |
| 2 | `FUN_004A82B9` | armies (unit type 1) | **Army movement**, including battle resolution. |
| 3 | `FUN_00429418` | transports (type 4) | **Supply transports** are re-targeted at their destination county's anchor and walk. |
| 4 | — | every realm's `aiStep` = 999 | **The players' turn.** `AI_RunTurnStep` drives the AI realms; the human realm is driven by the UI. Also where the turn timer runs. |
| 5 | `FUN_004AC499` | peasant mobs (type 2) | **Revolting peasants** move. |
| 6 | `Merchant_AdvanceAll` | merchants (type 3) | **Merchants**, already documented in [`plane4.md`](formats/plane4.md) §2.3. |
| 7 | `Season_Advance` | — | **End of season.** Runs once and advances straight to phase 1. |

**[V]** on the phase numbering and dispatch; **[V]** on phase 6, which is where
`plane4.md` independently found `Merchant_AdvanceAll`; **[D]** on the one-word
descriptions of phases 2, 3 and 5, which come from the unit type each phase waits on
rather than from tracing the handlers.

### 3.2 The AI realm's turn is a fourteen-step program

`AI_RunTurnStep` (`0x0049A581`) picks the next in-play realm round-robin and dispatches on
that realm's `+0x00`. All fourteen handlers have now been decompiled:

| step | address | what it does |
|---:|---|---|
| *0* | `0x0049B42B` | **not a handler** — recount realm strength, eliminate the realm if it is zero, `Score_RankRealms`, then set the counter to 1 |
| 1 | `Diplo_AnswerInbox` `0x004A277D` | answer the five pending messages in the realm's inbox (`g_diploInbox`, `0x0053F0F0`), then empty it. [`diplomacy.md`](diplomacy.md) §2.1 |
| 2 | `AI_Diplomacy` `0x004A0C1D` | **the whole diplomacy driver.** Heal standing +1 a turn towards every non-human realm, age the alliance grudge and break it past the lord's threshold, and court an ally. [`diplomacy.md`](diplomacy.md) §4.1 |
| 3 | `0x0049D638` | `AI_SetTaxRates` — §8.2 |
| 4 | `0x0049E1BF` | total what the realm can sell and what it needs to buy, into realm `+0x70 … +0x7C` |
| 5 | `0x0049DD01` | `AI_ManageFields` — add fields as the county grows, then apply the lord's farming style |
| 6 | `AI_BuildCastles` `0x0049EDC7` | order the largest castle the treasury clears, from five per-lord gold thresholds at personality `+0xCC … +0xDC`, capped at personality `+0x90` concurrent builds. [`diplomacy.md`](diplomacy.md) §8.1 |
| 7 | `0x0049F93D` | three army-management sub-passes |
| 8 | `0x0049F96C` | **nothing — the function is empty** |
| 9 | `0x0049F977` | raise men in the realm's chosen county and march them |
| 10 | `0x004A0015` | create a type-7 unit and send it out |
| 11 | `0x004A5667` | walk every army towards its target tile |
| 12 | `0x0049E77D` | set every county's weapon type from a ten-step per-lord rota, switch the four industries on or off, reallocate labour |
| 13 | `AI_Taunt` `0x004A13A6` | **not alliances** — a taunt timer. A realm ranked better than 2nd sends *"How are you doing?"* above 39 % of the map and *"Helpful advice."* to the last-placed human above 27 %. [`diplomacy.md`](diplomacy.md) §6 |
| 14 | `0x0049D1E0` | recompute the realm's totals — §8.3 |

**[V]** on the fourteen addresses and the dispatch. **[D]** on the one-line descriptions
of 4, 7, 9, 10, 11 and 12, which are read off decompiled C with no second source. Steps 3, 5
and 14 are **[V]** and are written out in §8.2 and §8.3; steps 1, 2, 6 and 13 are written out
in [`diplomacy.md`](diplomacy.md).

**Steps 2 and 13 were the wrong way round here until the diplomacy work.** This section had
step 2 as a grudge counter and step 13 as *"offer an alliance, or break one"*. Step 2 does
both of those; step 13 does neither and is a taunt timer. The rows are corrected above.

Three corrections to what this section used to say:

* **Step 0 is an initialisation, not a handler.** The `if (aiStep == 0)` branch above the
  dispatch runs `FUN_0049B42B` and `FUN_0049D1E0`, clears realm `+0x1C`, and sets the
  counter to 1. The dispatch covers 1 … 14.
* **The finish test is `15 + 2 × realmIndex <= aiStep`, and it is not sufficient on its
  own.** The realm is marked done only if `FUN_004A4E3D(1, realm)` *also* returns 0 — a
  call that starts every one of the realm's idle armies moving and reports whether it
  found any. A realm with armies still to launch keeps stepping past the threshold.
* **The stored sentinel is 1000, not 999.** The increment at the bottom of the function
  is outside the `if` that writes 999. Nothing breaks, because every later test is
  `aiStep < 999` — but a reader comparing a save against `== 999` will find nothing.

### 3.3 Seasons and the year

```c
/* inside Season_Advance, 0x00448440 */
int ended = g_seasonNext;
g_turnCount++;
g_seasonPrev = g_season;
g_season     = g_seasonNext;      /* the season now beginning */
g_seasonNext = ended + 1; if (g_seasonNext > 4) g_seasonNext = 1;
if (ended == 4) { g_year = g_yearNext; g_yearNext++; }
... 28 subsystem passes ...
```

**[V] `g_season` (`0x0057C934`) is 1 … 4 and is used directly as the index into `L2.eng`
group 29** — `No Season, Spring, Summer, Autumn, Winter` — at `0x0041A113`. The year rolls
after season 4, so **season 4 is Winter and the year runs Spring → Winter**.

The important subtlety, and the thing that makes every seasonal rule read correctly:
`Season_Advance` sets `g_season` to the season *about to begin* **before** running the
economy. So a rule guarded by `g_season == 1` fires at the **end of Winter**.

That is not an interpretation; the game's own in-game help says so. `L2.eng` group 292
index 5:

> *"Grain is only planted at the end of the winter turn, and is harvested at the end of
> the Autumn. You will see the extra grain in your county at the start of each Winter
> turn."*

`Grain_SeasonTick` sows when `g_season == 1` (entering Spring, i.e. at the end of Winter),
grows at 2 and 3, and harvests when `g_season == 4` (entering Winter, i.e. at the end of
Autumn, with the grain appearing in the store during that same update). **[V]** — three
separate clauses of one sentence, all matching.

Two more consequences fall out and both check:

* `g_deathRateBySeason` is `Spring 4, Summer 0, Autumn 2, Winter 8` percent. Winter is the
  deadliest season and summer the safest. **[V]**
* the weather accumulator moves `+8, +24, +12, −12` for Spring, Summer, Autumn, Winter,
  and *Drought* is rewritten to *Frost* only in Winter and Spring. **[V]**

**[V] A new game starts in Winter 1268.** `Game_NewGame` (`0x00497CED`) sets
`g_season = 3`, `g_seasonNext = 4`, `g_year = 1267`, `g_yearNext = 1268` and then calls
`Season_Advance` once, which rolls the year. The shipped `lastturn.sav` reads back
`g_season = 4`, `g_year = 1268`, `g_turnCount = 1` — §9.

### 3.4 The end-of-season pipeline

`Season_Advance` calls 28 functions in a fixed order. **The order is the rule**: taxation
reads the happiness that migration has not yet changed, population growth reads the
happiness that this turn's update has already written, and so on. Abridged to the passes
this document identifies:

```
Season_Advance
├── clock: season, year, turn counter
├── Event_RollAll               random events per county          §8.1
├── Weather_UpdateAll           dryness -> weather band           §7.3
├── Tax_CollectAll              gold, and the tax happiness term  §4.1
├── Wages_PayAll                army wages, bankruptcy            §7.4
├── (food demand recomputed)    Ration_Apply per county           §4.3
├── Health_UpdateAll            health meter and band             §4.2
├── Happiness_UpdateAll         sum the four terms                §4.4
├── Unrest_UpdateAll            revolt counter                    §6
├── Fertility_Update            fertility, then field reclamation §7.2
├── Grain_SeasonTick            sow / grow / harvest              §7.1
├── Herd_SeasonTick             livestock                         §7.1
├── Industry_Produce x4         weapons, then iron, stone, wood   §7.4
├── Castle_BuildTick            castle construction               §7.5
├── Migration_UpdateAll         emigrants and immigrants          §5.3
├── Population_UpdateAll        births, deaths, new population    §5
├── (clear the event modifiers a second time)                     §8.1
├── history ring                400 seasons x 16 counties         below
└── Ration_Apply again, as next season's preview                  §4.3
```

**[V]** the call list and its order; **[D]/[I]** the one-line descriptions, per the
sections they point at.

**Two corrections to an earlier revision of this list.**

**`Score_RankRealms` is not one of the 28 calls.** It was listed here and it is not
there. Its five callers are `Turn_Tick` (`0x0049A010`), `Game_NewGame` (`0x00497CED`),
`0x0049CEBE`, the AI turn's step 0 (`FUN_0049B42B`, §3.2) and one UI path at
`0x00435211`. So the ranking is rebuilt when a realm's strength is recounted and when the
turn phase advances, not at the end of the season. **[V]**

**The four `Industry_Produce` runs are weapons, iron, stone, wood.** The driver
(`FUN_0044E852`) makes two loops over the counties: the first runs the blacksmith over
every county, the second runs iron, stone and wood per county. So a season's weapons are
paid for out of the **previous** season's ore, because this season's has not been mined
yet. This is neither of the two orders an earlier revision gave. **[V]**

**The history ring** (`FUN_004AE7DD`) is `0x0056D8C0`, **400 seasons × 16 counties ×
8 bytes**, each entry `{i32 population, i8 happiness}`. It is written unconditionally for
counties 1 … 16 whatever `g_countyCount` is. Three globals go with it: the write head
`0x0055300C`, the oldest entry `0x00568DA8` and the count held `0x00553F30`, saturating
at 400. **[V]** — and the arithmetic closes: save block 10 is exactly 51,200 bytes and
`400 × 16 × 8` is 51,200, with each of the three globals its own four-byte block.

---

## 4. Happiness — the centre of the whole layer

### 4.1 Tax

`Tax_CollectAll` (`0x0044B59B`), per county:

```c
/* castleType is county +0x1C0, but when the flag at +0x1C3 is set the lower of
   +0x1C0 and the +0x1C1 castle-under-construction value is used instead, and a
   zero +0x1C1 forces type 0.  The flag was not traced; a siege or a partly razed
   castle would both fit and neither is established.                          [D] */
base = county.taxRobbed != 0 ? 0                    /* +0x1A8 — see below */
     : castleType == 0 ? 320 : (int[]){480, 560, 640, 720, 800}[castleType - 1];
take  = Pct(Pct(population, base), taxRate);       /* Pct(x,p) = x*p/100 */
county.taxCollected = take;
if (owner == 0) county.f1F4 += take;               /* an unowned county keeps its own */
else { realm.gold += take; realm.f0F4 += take; realm.f0F8 += take; }
county.dHapTax = (5 - taxRate) + realm.taxHapEmpire;   /* +0x0E; +0x16 is a table, below */
```

**[V] `+0x1A8` is the *"Stop thief!"* random event**, and it is no longer an untraced
gate. `L2.eng` group 315 is *"Highwaymen waylay your tax collectors. Lose all tax revenues
this season."*, its handler (`FUN_0044975D`, event id `0x13B`) sets `+0x1A8 = 100` and
does nothing else, and `Event_RollAll` clears the byte at the start of every season. The
handler is guarded on the county's shown tax being at least 10 — there is no point robbing
a collector who is carrying nothing. §8.1.

**[V] An unowned county banks its own tax** into `+0x1F4` rather than into any treasury,
which is why the neutral tax ladder in §8.2 exists at all.

**[V] The castle multipliers are exactly the published castle tax bonuses.**
`g_castleTaxBonus` (`0x004D8A28`) holds `50, 75, 100, 125, 150` — and
`480/320 = 1.50`, `560/320 = 1.75`, `640/320 = 2.00`, `720/320 = 2.25`, `800/320 = 2.50`.
Two tables in the same binary, written in different units, agreeing exactly. `L2.eng`
group 71 index 16 is *"Boosts tax revenues by"*, and the castle-upgrade messages say
*"The addition of this castle will boost the county's tax revenues"* and *"This lesser
castle will reduce the tax collected in the county"*.

So a county with no castle yields `pop × 3.2 × taxRate / 100` crowns a season, and a royal
castle yields two and a half times that.

**[V] Tax costs happiness at one point per point of rate above 5.** `dHapTax = 5 − rate`,
so a rate of 5 is free, and every point above it costs one happiness per season. The empire
term `realm.taxHapEmpire` is the sum of every owned county's `+0x16`, which is the
mechanism behind the manual's *"if you set taxes outrageously high in one county, this will
damage the happiness ratings of all your other counties."*

**[V] `+0x16` is a table lookup and *not* `5 − rate`, and this document said otherwise.**
`Tax_RecomputePreview` (`0x0044B80B`) is the only writer of `+0x16` anywhere in the binary,
and it writes two different things to two different fields:

```c
county[+0x0F] = 5 - taxRate;                       /* the local half        */
county[+0x16] = g_taxHappinessOther[taxRate];      /* the empire half       */
```

`g_taxHappinessOther` (`0x004D63D8`) is 51 `i32` entries, one per rate `0 … 50`:

| rate | 0 – 19 | 20–23 | 24–27 | 28–31 | 32–34 | 35–37 | 38–39 | 40–41 | 42–43 | 44 | 45 | 46 | 47 | 48 | 49 | **50** |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| `+0x16` | 0 | −1 | −2 | −3 | −4 | −5 | −6 | −7 | −8 | −9 | −10 | −11 | −12 | −13 | −14 | **−15** |

So **taxing at 19% costs the rest of the realm nothing whatever**, and even a punitive 50%
costs it 15 — an order of magnitude gentler than the shape of `5 − rate` suggests. Reading
`+0x16` as `5 − rate` and then, when the save refused it, as `min(5 − rate, 0)`, gives the
right answer at six of the 51 rates. One of those six is rate 0, which is every rate in
`lastturn.sav`, so the whole of `crates/l2-kingdom`'s test suite passed on the wrong rule.
The table is now `l2_kingdom::tables::TAX_HAPPINESS_OTHER` and
`tools/oracle/kingdom.ps1` checks all 51 entries against the executable.

**[V] The rate is capped at 50, and the table's length is the second reading of it.**
`Tax_IncreaseCounty` (`0x0043AA83`) guards `taxRate < 0x32`; `0x004D63D8 + 52 × 4` is
exactly `0x004D64A8`, where `g_healthDeltaTable` begins, and the 52nd word is a zero no
rate can index. A rate that could reach 100 would need 101 rows.

> **A caution about the empire term.** `Tax_SumEmpireHappiness` sums signed bytes from up
> to sixteen counties **into a signed byte**. Nothing clamps it. With the real table the
> worst case is sixteen counties at −15, which is −240 and *does* wrap; under the old
> reading it was −720 and wrapped much sooner. Whether it wraps in play was not tested
> here. **[D]**

### 4.2 Health and rations

Two coupled ladders.

**`g_rationTable` (`0x004D6738`)** — six `{divisor, multiplier}` pairs. The requirement is
`DivCeil(population, divisor) × multiplier`:

| level | pair | food needed | happiness `3L − 8` | `L2.eng` group 21 |
|---:|---|---|---:|---|
| 0 | (1, 0) | none | **−8** | None |
| 1 | (4, 1) | pop / 4 | **−5** | Quarter |
| 2 | (2, 1) | pop / 2 | **−2** | Half |
| 3 | (1, 1) | pop | **+1** | Normal |
| 4 | (1, 2) | pop × 2 | **+4** | Double |
| 5 | (1, 3) | pop × 3 | **+7** | Triple |

**[V]** Six table entries, six strings in group 21, and the ratios match the names one for
one. The happiness column is the single expression `rationLevel * 3 - 8` at the end of
`Ration_Apply`. The manual: *"A ration of Normal or above will improve happiness while half
or quarter rations will decrease happiness"* — Normal is the first positive row.

**`g_healthDeltaTable` (`0x004D64A8`)** — `int[6][5]`, indexed `[rationLevel][healthBand]`,
added to the health meter each season:

| ration \ band | 0 Diseased | 1 Sick | 2 Average | 3 Good | 4 Perfect |
|---|---:|---:|---:|---:|---:|
| None | −8 | −10 | −13 | −16 | −20 |
| Quarter | −4 | −6 | −9 | −12 | −15 |
| Half | −2 | −4 | −6 | −8 | −12 |
| **Normal** | **+8** | +4 | +2 | +1 | −1 |
| Double | +12 | +8 | +4 | +2 | 0 |
| Triple | +20 | +12 | +6 | +3 | +1 |

The meter is clamped 0 … 100 and re-banded through **`g_healthBandLadder`
(`0x004D6520`)**, which is **five `{threshold, band}` pairs** — `(10,0) (35,1) (65,2)
(90,3) (100,4)` — and not the bare threshold array an earlier revision described. The
*"else 4"* is an explicit fifth row, not a fallthrough. **[V]**, by two independent checks:
five pairs of `int` is 40 bytes and `0x004D6520 + 40` is exactly `0x004D6548`, where
`g_healthHappiness` begins; and the interleaved `0,1,2,3,4` cannot be thresholds, because
thresholds do not decrease. Nothing behaves differently — the meter is clamped before it is
banded, so the last row is never fallen off — but a reimplementation that reads the table
as one array reads garbage.

The band then produces the happiness term from **`g_healthHappiness` (`0x004D6548`)**,
which has **six** slots with the sixth zero (`0x004D6548 + 24` is `0x004D6560`, where
`g_herdWeatherPct` begins):

| band | 0 Diseased | 1 Sick | 2 Average | 3 Good | 4 Perfect |
|---|---:|---:|---:|---:|---:|
| happiness / season | **−10** | **−5** | **0** | **+1** | **+2** |

**[V]** Five entries, and `L2.eng` group 20 is exactly `Diseased, Sick, Average, Good,
Perfect`. Recovering a starved county is fast at the bottom (+8 to +20 a season at band 0)
and slow at the top, which is what the sign pattern in the table's last column encodes:
**Perfect health decays under anything less than Double rations.**

### 4.3 Where the food comes from

`Ration_Apply` (`0x0044DF5F`) descends from `rationWanted + 1` until the requirement fits
what the county can feed, then spends, in order:

1. **Dairy, free.** `Food_FromDairy` = `herd × g_dairyPerHead`, and
   **`g_dairyPerHead` (`0x00553F60`) is 5**. The standing herd feeds five people per head
   per season without being slaughtered. **[V]** — and independently: a strategy guide
   states *"each portion of cheese being enough to feed 5 people"*, and a player measuring
   a save reported 80 cows feeding 400 people.
2. The remainder is split by `county.rationSplit` percent between
   **`Food_HeadsForPeople` = `DivCeil(people, 10)`** (`g_foodPerHead` = 10, one slaughtered
   animal feeds ten) and **`Food_SacksForPeople` = `DivCeil(people, 6)`** (`g_foodPerSack`
   = 6, one sack of grain feeds six). **[V]** on the constants and the rounding;
   **[D]** on the commodity each side denotes.
3. Each side is capped at what is actually in store, and the loop drops a ration level and
   retries if the total will not fit.

> **What reproduces — all of it, now. Corrected twice.** An earlier revision of this box
> said the nine unowned counties reproduced, the *"four player-owned"* ones did not, and
> that they stored `+0x17C = 3` where the model gives 0. Two things were wrong with that.
> There are **five** owned counties, not four (§9). And they store `+0x17C = 0`, not 3 —
> the 3 is `rationAchieved` at `+0x15D`, one field along.
>
> Read correctly, **every county in the file reproduces**, and
> `crates/l2-kingdom/tests/reproduction.rs` asserts it for all fourteen:
>
> | | population | herd | grain | split | achieved | `+0x178` | `+0x17C` |
> |---|---:|---:|---:|---:|---:|---:|---:|
> | nine unowned | 456 | 67 | 100 | 100 % | Normal | 0 | **13** |
> | four of the five owned | 435 | 93 … 110 | 0 | 0 or 100 % | Normal | 0 | **0** |
> | **realm 5's county** | 435 | 74 | 0 | 0 % | **Half** | 0 | **0** |
>
> The unowned row is the worked example: `456 − 67×5 = 121` people left to feed and
> `DivCeil(121, 10) = 13` head. The middle row keeps a herd large enough that five people
> per head covers the whole county, so nothing is slaughtered and nothing is sown. The last
> county splits its ration entirely onto grain and has none, so it drops a level — which is
> why its `dHapRation` is **−2** while its `shownRation` is **+1**.
>
> **Which county that is, is rolled per game; which realm it is, is not.** This table used
> to name the last two rows "counties 4, 8, 11, 13" and "county 1", read off one saved
> game. Two independently created England turn-one saves put the five starting counties at
> the same five indices — 1, 4, 8, 11, 13 — and hand them to realms 1 to 5 in a **different
> order each time**. In the first save realm 5 held county 1; in the second it holds county
> 8. The hungry county is realm 5's in both. **One lord always begins short of food, and it
> is always realm 5** — a piece of scenario design that had been written down here, and
> asserted in three test files, as a fact about county 1.
>
> **The `shownRation`/`dHapRation` disagreement settles which write survives.**
> `shownRation` is the copy taken while happiness was computed, so the *first* call fed
> that county at Normal; `dHapRation` is the *second* call, next season's preview, and it
> says Half. And the first call must have **debited the store**: feeding it at Normal on an
> all-grain split costs `DivCeil(417 − 74×5, 6) = 8` sacks, and the county holds none. A
> call that did not spend would have left those eight sacks behind.
>
> The same inversion recovers the opening stores the file does not record, uniquely: the
> unowned counties began the season on **73 head** and closed on 67, and realm 5's county
> began on **8 sacks**. Put those back and the whole map reproduces every stored field.

### 4.4 Putting happiness together

`Happiness_UpdateAll` (`0x0044BAEA`):

```c
county.happinessLast = county.happiness;                      /* +0x0D */
county.happiness += county.dHapTax + county.dHapHealth + county.dHapRation;
county.shownTax = dHapTax; county.shownHealth = dHapHealth; county.shownRation = dHapRation;
county.shownArmy = county.shownEvents = county.shownAle = 0;
if (population == 0 && owner is AI)      county.happiness = 50;
if (county.happiness < 75 && owner == 0) { county.happiness += 5; county.shownEvents = 5; }
clamp 0 .. 100;
county.happinessSum += county.happiness;
county.happinessAvg  = county.happinessSum / g_turnCount;
```

**[V]** and reproduced exactly from the save — §9.

The *army* and *ale* terms are **zeroed** by this pass, because they are written when the
player acts rather than once a season. Both writers are traced now.

**Ale — `FUN_00428C42`.** Buying ale is worth one happiness per **10 %** of the county's
population in crowns spent, up to a cap of five:

```c
if (crowns <= 0) return;
tenth = population / 10;
bonus = crowns >= 5*tenth ? 5 : crowns >= 4*tenth ? 4 : ... : crowns >= tenth ? 1 : 0;
if (bonus > 5 - county.aleGiven) bonus = 5 - county.aleGiven;   /* +0x219 */
if (bonus < 0)                   bonus = 0;
county.aleGiven += bonus;  county.happiness += bonus;  county.shownAle += bonus;
if (county.happiness > 99) county.happiness = 100;
```

**[V]**, and it settles the published claim §12 used to list as unverified: the guides say
*"+1 per 20 % of the population, cap +5"*, and the cap is right but the step is twice too
coarse. The panel's own preview (`FUN_00435673`) computes the identical ladder, which is
the second source. `crowns` is `price × quantity` at the call site, and ale's base price is
1 (§10), so a barrel is a crown.

**The cap is cumulative and nothing resets it.** `+0x219` holds the total already granted
and is only ever added to; a cross-reference of every instruction touching the offset found
no other write. So a county can be given **five happiness from ale for the whole game**,
not five a season. **[D]** — a negative, and negatives are hard to prove.

**Army — `FUN_004A9A9A`'s tail.** Raising men costs the county happiness, and the cost is a
table lookup on **the share of the county being taken**, not on the number of men:

```c
share = PctOf(men, population);              /* 50 men of 500 is 10 */
cost  = g_armyHappinessCost[share];          /* 0x004D8778 */
if (happiness < cost) { shownArmy -= happiness; happiness = 0; }
else                  { happiness -= cost;     shownArmy -= cost; }
```

**`g_armyHappinessCost` (`0x004D8778`)** is 102 ints, `0x004D8778 … 0x004D8910`, which is
exactly where the merchant price table begins. It is steeply progressive:

| share taken | 5 % | 10 % | 20 % | 25 % | 33 % | 50 % | 60 % + |
|---|---:|---:|---:|---:|---:|---:|---:|
| happiness cost | 2 | 5 | 10 | 19 | 37 | 90 | 101 |

**[V]**. The AI's own caller computes `share = PctOf(50, population)` and raises
`Pct(population, share)` men, so an AI raising fifty men from a thousand-person county pays
2 and from a hundred-person county pays 90. Note the index is **not bounded**: any county
under 50 people gives a share above 101 and the read lands on the first entry of the
merchant price table, which is 0 — a free army. Whether that path is reachable was not
established; the AI's is guarded on the county having 400 people.

The three terms mean the whole steady state is easy to state: **a county holds its
happiness when `(5 − taxRate) + healthHappiness + (3×ration − 8) = 0`.** At Perfect health
and Normal rations that is `5 − rate + 2 + 1 = 0`, i.e. **rate 8**. At Good health,
**rate 7**. A published FAQ says exactly that, in those words: *"a county in good health
(+1 happiness) with normal rations (+1) and 100 happiness can pay 7 % taxes (−2) and remain
at 100 %. At perfect health, they can pay 8 %."* **[V]** — the arithmetic in the guide and
the arithmetic in the binary are the same arithmetic.

---

## 5. Population

`Population_UpdateAll` (`0x00449EF3`), per county, in this order:

```c
popLast = pop;
cap     = Pct(pop, 20);
base    = Table_Lookup(pop, g_birthRateLadder, 20, 1);      /* percent */
death   = g_deathRateByHealth[healthBand] + g_deathRateBySeason[g_season];
factor  = happiness < 26 ? 25 : happiness < 51 ? 50
        : happiness < 76 ? 75 : happiness < 100 ? 100 : 120;
births  = Pct(pop, Pct(base, factor));
deaths  = Pct(pop, death);
if (births == 0 && birthRate != 0) births = 1;
if (deaths == 0 && death     != 0) deaths = 1;
if (healthBand == 0)        deaths += 2;                    /* Diseased */
if (birthRate < death)      deaths += 1; else births += 1;
/* random-event modifier, capped at 20 % of the population */
pop += births - deaths;
if (pop < 1) { births = 0; deaths = pop; pop = 0; }
pop -= emigrants; pop += immigrants;
```

**[V]** and reproduced exactly from the save — §9 gets both births and deaths right, twice
each, on two different happiness bands.

### 5.1 The birth-rate ladder

**`g_birthRateLadder` (`0x004D6308`)** — twenty `{population, percent}` pairs:

| pop ≤ | 40 | 80 | 100 | 250 | 500 | 700 | 800 | 900 | 1000 | 1100 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| % | 100 | 70 | 50 | 30 | 20 | 15 | 14 | 13 | 12 | 11 |

| pop ≤ | 1200 | 1300 | 1400 | 1500 | 1600 | 1700 | 1800 | 1900 | 2000 | 3000 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| % | 10 | 9 | 8 | 7 | 6 | 5 | 4 | 3 | 2 | 1 |

**[V]** as a table in the binary; the pairing is confirmed by the reproduction in §9. This
is the "soft population cap" players describe: at 2,000 people the birth rate is 2 %, which
at Winter's 8 % death rate cannot keep up.

### 5.2 Death rates

`g_deathRateByHealth` = **35, 20, 8, 3, 0** percent for bands 0 … 4, and
`g_deathRateBySeason` = **Spring 4, Summer 0, Autumn 2, Winter 8**. They add. A Diseased
county in winter loses 43 % of its people in one season. **[V]**

### 5.3 Migration

`Migration_UpdateAll` (`0x0044A6BA`) finds each county's happiest neighbour and moves
people towards it:

```c
if (bestNeighbourHappiness > happiness) {
    pct     = Pct(bestNeighbourHappiness - happiness, (100 - happiness) / 3);
    movers  = Pct(population, pct);
    if (movers > 100) movers = 100;
    if (owner == 0)   movers /= 2;
}
```

**[V]** on the formula. Two consequences worth stating: **migration is capped at 100 people
per county per season**, and it is driven by the *difference* scaled by how unhappy the
source is — a county at 100 happiness never emigrates, whatever its neighbours do.

> **A likely bug.** The loop that records the source county in the destination's
> 16-byte inflow list has no `break`: it writes the source id into **every** free slot
> rather than the first. The list therefore ends up holding one repeated value, and the
> pass that reads it back only takes a maximum, so the visible effect is limited to the
> *"arrive from"* line of the population panel naming the wrong county. **[D]** — read
> from decompiled C, not observed.

---

## 6. Unrest and revolt

`Unrest_UpdateAll` (`0x0044AA41`) walks `county.unrest` (`+0x20`) up and down against
happiness, on two different ladders depending on whether the owner is human:

* **AI-owned:** happiness ≥ 41 resets it to 0; 11 … 40 walks it down; below 1 walks it up.
* **Human-owned:** happiness ≥ 30 clears a "warned" flag; below 30 fires message `0x92`;
  below 25 walks the counter up and fires messages `0x96`, `0x97`, `0x98`, `0x99` as it
  passes 1, 2, 3 and 4.

At 4 it calls `FUN_004AC185`, which raises the revolting-peasant army (unit type 2, the one
phase 5 moves), and resets the counter.

**[V] against the manual**, which is unusually specific here: *"When any county's happiness
rating drops below 25 and stays there for more than four seasons, its population will
revolt."* The threshold is 25 for a human-owned county and the counter needs four seasons
below it. Both halves of the sentence are in the code.

---

## 7. Land, industry and the treasury

### 7.1 Grain and livestock

The grain cycle is one number moving through three fields:

```
end of Winter   Grain_Sow      sown  = fields × sacksPerField ; store -= sown
                               crop  = sown × g_grainYieldPerSack
end of Spring   Grain_Grow     crop adjusted
end of Summer   Grain_Grow     crop adjusted
end of Autumn   Grain_Harvest  store += crop
```

**[V] `g_grainYieldPerSack` (`0x0057C8E0`) is 12** — and `L2.eng` group 292 index 4, the
game's own frequently-asked-questions page, says *"Each sack planted will grow into 12
sacks so long as there is enough labor to tend and harvest it during the year."*

`Grain_Sow` (`0x0044CFE1`) chooses sacks-per-field by descending from
**`g_grainMaxSacksPerField` (`0x00552FFC`) = 10** to 1, taking the first value for which
both `grainStore ≥ fields × sacks` and `labour ≥ 12 × fields × sacks / divisor` hold. The
divisor is 5 with *Advanced Farming* on and 2 with it off (`0x005533BC` and `0x0057D34C`,
both written by the constant initialiser `FUN_004983B7`).

**[V] The labour figure is job slot 0**, which is *"Grain farming"*. `Grain_SeasonTick`
(`0x0044C8AE`) passes county `+0xC4` with no job stride to all three of `Grain_Sow`,
`Grain_Grow` and `Grain_Harvest` — see §7.4 for why slot 0 is group 74's *string 1*.

**[V] There is a fallback path.** If even one sack a field cannot be afforded or worked,
the function sets a shortfall flag at county `+0x1A7` and retries against the *sack count
alone*, ignoring the field count: it returns the largest `s ≤ 10` with `s ≤ grainStore` and
`12 × s / divisor ≤ labour`, and sows that as the whole county's seed. So a county too poor
to sow properly plants a token handful rather than nothing, and `Grain_SeasonTick` then
records its field usage as 1 rather than `fieldsGrain`.

> **This contradicts the manual, and the manual is wrong.** The manual says twice that *"up
> to 5 sacks of grain can be planted in one field"*. The constant is **10**, and two
> independent players measuring their own saves reported 6 fields → 60 sacks and 9 fields →
> 90 sacks. `docs/decisions.md` C8's rule — find prior art, then check it against the data —
> cuts both ways: here the published guides are right and the shipped manual is not. The
> game's own readme reportedly warns that the manual predates late changes. **[V]** on the
> constant, **[I]** that 10 is what a player sees.

Each stage is scaled by the county's weather band (`Grain_SeasonTick`): entering Spring,
*Flooding* quarters the sowing and *Frost* or *Storms* halve it; entering Summer or Autumn,
*Sunny* multiplies the crop by 3/2 while *Drought* and *Flooding* halve it; at harvest,
*Sunny* again gives 3/2 while *Flooding* quarters and *Frost* or *Storms* halve. **[D]** —
the branch structure is unambiguous, but nothing outside the binary confirms the factors.

Livestock (`Herd_SeasonTick`, `0x0044D60D`) gets a percentage swing from
**`g_herdWeatherPct` (`0x004D6560`)**:

| weather | Frost | Drought | Sunny | Cloudy | Storms | Flooding |
|---|---:|---:|---:|---:|---:|---:|
| herd change | −2 % | −10 % | **+5 %** | 0 % | −5 % | −10 % |

**[V]** Six entries for six weather names, and the signs match `L2.eng` group 66's own
descriptions: only *Sunny* is *"Boosts growing crops"*, only *Cloudy* is *"Little effect on
farming"*, and the other four all say crops are lost.

### 7.2 Fields and fertility

A county owns up to twenty fields; their map tile ids live in `g_countyFieldTiles`
(`0x0053EA00`, 17 × 20 × u32 — save block 12 is 1,360 bytes = 17 × 80, exactly) and their
condition in the twenty words at county `+0x90`.

**Reclamation.** `Field_ReclaimTick` (`0x0044C093`) pushes a field's progress towards 800
by **at most 200 per season**, redrawing the tile at each quarter. The manual: *"you will
never be able to reclaim more than a quarter of a field in a single season."* 200 / 800 is
exactly a quarter. **[V]**

**Fertility.** `Fertility_Update` (`0x0044BFD5`) is three lines:

```c
county.fertility += 6 * county.fieldsFallow - 3 * county.fieldsGrain;
clamp -100 .. 100;
if (!g_optAdvancedFarming) county.fertility = 0;
```

**[V] One fallow field per two grain fields is exactly break-even**, and **cattle fields do
not enter the formula at all**. That contradicts the manual's *"try to keep at least a
third of your fields fallow"* and matches, precisely, a player's claim that the real rule
is one fallow per two wheat fields with cattle fields excluded. `L2.eng` group 22 names
seven fertility levels from *"Infertile — almost no production"* to *"Excellent fertility —
bumper crop!"*; the mapping from the −100 … 100 scalar to those seven was not traced. **[I]**

Field usage counts live at `+0x1FF`, `+0x200` and `+0x201` and their sum is the county's
field total. Over the fourteen counties of the England map in `lastturn.sav` the totals run
**8 to 16** — which is the range a published guide gives (*"usually they number at least 8
and as high as 16"*). Sample: fourteen counties of one map. **[V]** for the count,
**[I]** for generalising it.

### 7.3 Weather

`Weather_UpdateAll` (`0x00449889`) keeps a per-county **dryness** accumulator at `+0x21D`:

```c
delta = {Spring: 8, Summer: 24, Autumn: 12, Winter: -12}[g_season] - (randomB & 0x7F) / 8;
for every county:            dryness += delta;
c = randomA & 0xF;                            /* 0 .. 15, whatever the map holds */
if (c > g_countyCount) c = g_weatherCounty + 1;
if (c > g_countyCount) c = 1;
if (c < 1)             c = 1;
g_weatherCounty = c;
                             dryness[c] += delta + localModifier(c);
for each neighbour n of c:   dryness[n] += delta/2 + localModifier(n);

band =  dryness <  5 ? 5 Flooding   /* and dryness is pulled up to 30 */
      : dryness < 20 ? 4 Storms
      : dryness < 70 ? 3 Cloudy
      : dryness < 95 ? 2 Sunny
      :                1 Drought;   /* and dryness is pulled down to 70 */
if (g_season == 4 || g_season == 1) {          /* Winter or Spring */
    if (band == 1) band = 0;                                  /* Drought -> Frost */
    else if (band == 2 && dryness > 74) band = 0;             /* Sunny   -> Frost */
}
if (!g_optAdvancedFarming) band = 3;                          /* Cloudy everywhere */
```

**[V]** The six bands are exactly `L2.eng` group 66's six name/effect pairs, in the order
`Frost, Drought, Sunny, Cloudy, Storms, Flooding` at indices 0 … 5. The clamps at the two
extremes are mean reversion: a flood pulls the county back to 30, a drought back to 70.
**Weather is regional, not per-county**: every county gets the same seasonal push, and one
randomly chosen county plus its neighbours get an extra swing.

A player observation — *"Floods happen during winter and spring, while droughts happen
during summer"* — falls straight out: dryness rises fastest in Summer (+24) and falls only
in Winter (−12), and Drought is rewritten to Frost in Winter and Spring so it cannot be
*seen* in those two seasons at all. **[V]**

**`random` is 0 … 127, so the jitter is 0 … 15.** An earlier revision wrote the term as
`random/8` with no stated range, which made it unimplementable and left
`WEATHER_JITTER_BOUND` in `crates/l2-kingdom` as the one constant in that crate with no
evidence behind it (`decisions.md`, open questions). The generator is **`FUN_00404A46`**,
called once at the top of `Season_Advance`: it steps **two 31-bit LFSRs** — taps at bits 0
and 4, feeding bit 30, thirty-one iterations a call — and publishes six masked values from
them, `& 0x7FFF`, `& 0x7F` and `& 7` from each. `Weather_UpdateAll` reads the two `& 0x7F`
values and `Event_RollAll` reads one of them (§8.1). **[V]**

That matters, because a jitter of 0 … 15 **can cancel Spring's +8 and Autumn's +12
outright**: a wet spring is a real outcome, not an edge case. Only Summer's +24 is safe.

The county that gets the local swing is picked by a flat `& 0xF` that takes no account of
how many counties the map has, so on the fourteen-county England map the two out-of-range
draws fall through to *"the county after last season's"*. The local swing therefore walks
steadily around the map about an eighth of the time rather than jumping. `g_weatherCounty`
(`0x00554020`) is its own four-byte save block. **[V]**

`localModifier` is `FUN_00449D6E` and is **still not traced**.

**`dryness` is a signed byte and the accumulation wraps.** Nothing clamps it before the
band ladder reads it, so a long enough run of Summers rolls it through +127 into −128 and
turns a drought into a flood. Whether that is reachable in play was not tested. **[D]**

### 7.4 Industry, weapons and wages

`Industry_Produce` (`0x0044EA92`) runs four times per county per season. The driver is
`FUN_0044E852`, and its argument list is where the job slots and the bases come from:

| order | commodity | job record | `L2.eng` group 74 | divisor | base | credited to |
|---:|---|---:|---|---:|---:|---|
| 1st | 2 weapons | **7** | 8 Blacksmith | **4** | 15 % | realm `+0x140 + type*4` |
| 2nd | 1 iron | **4** | 5 Iron mining | 1 | 15 % | realm `+0x120` |
| 3rd | 3 stone | **5** | 6 Stone quarrying | **2** | 15 % | realm `+0x128` |
| 4th | 0 wood | **6** | 7 Wood cutting | 1 | **20 %** | realm `+0x130` |

`output = min(resourceLimit, Pct(workers / divisor, efficiency))`. **[V]** on the table;
**[V]** the 15 % base efficiency also appears verbatim in a published FAQ (*"30 serfs
working at 15 % efficiency"*), and *"iron and wood harvest at twice the quantity of stone"*
is exactly the divisor column.

**[V] The job column an earlier revision gave was one too high throughout**, because it
read the numbers straight off `L2.eng` group 74. Group 74 has **ten** strings and the
labour array has **nine** records: record *r* is group-74 string *r + 1*, because string 0,
*"Idle people"*, is the remainder rather than a job anyone is assigned to. So the records
are

```
0 Grain farming   1 Cattle farming   2 Field reclamation   3 Castle building
4 Iron mining     5 Stone quarrying  6 Wood cutting        7 Blacksmith
8 Idle townsfolk
```

Three independent facts fix it and they agree: the driver passes 7, 4, 5 and 6; the labour
allocator `FUN_0044F6E7` gates record 6 on the *wood* industry's enable flag, 4 on *iron*,
5 on *stone* and 7 on the *blacksmith*; and that allocator clears exactly nine records.
Grain sowing reads record 0 (§7.1), which is *"Grain farming"*.

**[V] The efficiency is not the base — it ramps.** `FUN_0044F248` is:

```c
if (!g_optAdvancedFarming) return 80;                /* a flat 80 %, and no ramp at all */
if (workers == 0)          return 0;
increment = base;                                     /* 15, or 20 for wood */
if (capacity < workers) increment = Pct(base, PctOf(capacity, workers));
eff = lastEfficiency + increment;                     /* county +0x29C, last season's */
if (eff > 100)  eff = 100;
if (eff < base) eff = base;
```

Three things follow. The efficiency **compounds**, so a new iron mine climbs 15 points a
season and takes seven seasons to reach 100 — a county that changes hands starts again.
**Overstaffing is self-defeating**: past the capacity at `+0x29E` the increment is scaled
by `capacity / workers`, so twice the workers improve at half the rate. And with *Advanced
Farming* off none of it happens and every industry sits at **80 %**, which is the shipped
save's setting and more than five times the base — so the published *"15 % efficiency"*
describes the advanced game only.

**[V] `resourceLimit` is `FUN_0044EF4E`.** For wood, iron and stone it is a flag test
rather than a quantity: the industry must be enabled (`+0x297`), must have its resource in
the ground (`+0x295`) and must not be counting down a disablement (`+0x296`); if all three
hold the limit is a literal **999** and otherwise **0**. For weapons it is the realm's
share of the stock the weapon costs — `(stock × cost / denominator) / cost` for wood and
iron, taking the smaller. **[D]** on the two denominators (`0x0057C904` and `0x0056D628`,
written by `FUN_0044F15B`), which were not traced.

A pass on a disabled industry produces nothing, zeroes the running total at `+0x2A0`,
decrements `+0x296` and reinstates the industry when it reaches zero.

Weapons debit **`g_weaponCost` (`0x004D8990`)**, six `{wood, iron}` pairs:

| | crossbow | mace | sword | pike | bow | armour |
|---|---:|---:|---:|---:|---:|---:|
| wood | 6 | 4 | 3 | 6 | 13 | 4 |
| iron | 10 | 4 | 10 | 3 | 0 | 18 |

**[V]** — identical to two independently published tables.

**Wages.** `Wages_ForUnit` (`0x004AD52B`) is the whole army upkeep rule:

```c
if (owner is human) wage = men / 4;
else wage = men / (difficulty == 0 ? 3 : difficulty == 1 ? 5 : 10);
```

where `men` is campaign unit `+0x168`, the field `battle.md` §4.1 identified as the total
over troop types 0 … 6. **[V] Troop type does not enter it: a knight and a peasant cost the
same.** A player measured 250 men → 62 crowns, 252 → 63, 254 → 63, all of them
`floor(men/4)`, across armies of knights, of peasants and of mixed troops. `Wages_PayAll`
(`0x004ACBD4`) sums it per realm into `+0xFC` and, if the treasury cannot cover it, runs a
five-stage bankruptcy escalation on `+0x158`.

**The five stages, and what each does.** Each raises a message whose id is its own `L2.eng`
group, and every one of those five strings describes, in prose, exactly what its handler
does — which is the second source that makes this **[V]** rather than **[D]**:

| stage | effect | message | `L2.eng` |
|---:|---|---|---|
| 0 → 1 | `FUN_004AD230` — every mercenary in every army walks off; an army left with no men is destroyed | `0xA0` | 160 *"Mercenaries desert! You have insufficient crowns … The mercenaries promptly deserted"* |
| 0 → 1 | …or, if there were no mercenaries to lose, a warning and nothing else | `0x10E` | 270 *"Unpaid troops. … your men will not tolerate this situation for long!"* |
| 1,2,3 → 2,3,4 | `FUN_004AD0E8` — every army loses men | `0x11F` | 287 *"Angry troops. … your armies are losing men."* |
| 4 → 5 | the same desertion, with a final warning | `0x10F` | 271 *"Mutinous troops. … it has been over a year since your men received any wages."* |
| 5 → **0** | `FUN_004AD316` — every army is destroyed, and the counter **resets** | `0x110` / `0x111` | 272 *"Mutiny!!!. … All your armies are disbanded."*, and to everyone else 273 *"FREE. … this noble's troops, unpaid for months, have mutinied"* |

Group 271's *"over a year"* is a check on the count: stage 4 is reached on the fourth
unpaid season, and four seasons is a year.

**[V] A realm that cannot pay keeps its gold.** The `else` branch is the only place the
treasury is debited, so an unpayable bill costs nothing at all — a realm one crown short
pays no wages and loses no crowns. And the counter **wraps** rather than saturating: after
the mutiny it is 0 again, so a realm that never pays loses its armies once every six
seasons.

### 7.5 Castles

Five designs, and five parallel tables indexed by castle type 1 … 5:

| | palisade | motte & bailey | Norman keep | stone castle | royal castle |
|---|---:|---:|---:|---:|---:|
| wood, stone (`0x004D89C0`) | 400 / 40 | 800 / 80 | 200 / 1000 | 400 / 2000 | 800 / 3000 |
| workforce (`0x004D89E8`) | 200 | 400 | 800 | 1500 | 2500 |
| garrison cap (`0x004D8A10`) | 150 | 200 | 200 | 400 | 600 |
| tax bonus % (`0x004D8A28`) | 50 | 75 | 100 | 125 | 150 |
| free archers (`0x004D8A40`) | 50 | 150 | 150 | 200 | 300 |

**[V]** on the values; the tax-bonus row is independently confirmed by `Tax_CollectAll`'s
own constants (§4.1) and by the manual's *"A new castle will automatically include a
garrison. Its size will vary according to the size of the castle."*

**Two layout notes**, because the values are right and the strides above are not.

**`g_castleWorkforce` is two ints per castle level, not one**: the bytes read
`200,200 400,400 800,800 1500,1500 2500,2500`. Ten ints is 40 bytes and `0x004D89E8 + 40`
is exactly `0x004D8A10`, where the garrison caps begin, so the stride is not in doubt.
**What the second column means is not established** — both columns hold the same number in
all five rows, so nothing here can tell a duplicate from a second quantity that happens to
match. **[V]** on the layout, **unknown** on the meaning.

**The last three tables are six slots each, five used and a trailing zero.** That 24-byte
stride is what puts the tax bonus at `0x004D8A28`, the free archers at `0x004D8A40`, and
`g_aiPersonality` (§8.2) at `0x004D8A58`. `g_healthHappiness` (§4.2) has the same shape.
**[V]**

**[V] The default starting castle is the Norman keep.** Every player-owned county in the
shipped `lastturn.sav` has `castleType = 3`, and `L2.eng` group 103 index 22 — the value
word for the "Starting Castle" option — is `keep`.

---

## 8. Events, the AI and scoring


### 8.1 Random events

`Event_RollAll` (`0x00448819`) deals from **`g_eventTable` (`0x004D6108`)** for each
**human-owned** county once the year passes 1268, and dispatches one of 24 handlers
(ids `0x87` … `0x8E` and `0x12E` … `0x13D`).

**`g_eventTable` is not a 24-entry table.** It is a **256-slot deck of `i16` ids, 230 of
them zero**, and a zero slot means "no event this season". The 26 non-zero slots hold the
24 distinct ids, two of them twice — **`0x8A` and `0x8B`, which are therefore twice as
likely to fire as any other event.** The deck is weighted, not uniform, which is precisely
what calling it "a 24-entry table" hides. Every non-zero slot sits at an index `≡ 7 (mod 8)`, so
the deck is 32 groups of eight with at most one event in each group's last slot. **[V]**,
and three things close it: 256 `i16` is 512 bytes and `0x004D6108 + 512` is exactly
`0x004D6308`, where `g_birthRateLadder` begins; the 24 distinct ids are exactly the 24 the
dispatch tests, with nothing unreachable on either side; and **each id is its own `L2.eng`
group number**, so each of the 24 comes with a sentence of shipped prose that matches what
its handler does.

The draw is a **ring, not a probability**:

```c
index = g_seasonRandom * 2;                 /* 0 .. 254, drawn once per season */
for (county = 1; county <= g_countyCount; county++) {
    county.eventPct[0..2] = 0;  county.taxRobbed = 0;
    index++;  if (index > 255) index = 0;
    if (realms[county.owner].isHuman && g_year > 1268 && g_eventTable[index] != 0) {
        county.eventFired = 1;  county.eventId = g_eventTable[index];
        dispatch(county);
    }
}
```

One number is drawn per **season**, not per county, and the counties then walk consecutive
slots. So the deck's density — 26 in 256 — is the whole frequency rule, about one event per
ten eligible county-seasons, and two adjacent counties can never both draw, because the
closest pair of dealt slots is eight apart.

> **A bug: only odd-numbered counties can ever draw an event.** The seed is
> `g_seasonRandom * 2` and so always even; the wrap resets the index to 0, which is also
> even, so the parity survives it; county *k* therefore always lands on a slot of parity
> *k*. Every dealt slot is at `index ≡ 7 (mod 8)`, which is odd. Counties 2, 4, 6 … 16 are
> permanently exempt, and nothing in play would reveal it, because the counties that do
> draw behave exactly as they should. **[V]** — arithmetic over the dumped deck, checked
> exhaustively across all 128 seeds.

**The 24 handlers.** Each id is its own `L2.eng` group; the *name* column below is that
group's index 0. Almost every handler is **guarded**, and a handler whose guard fails
clears `eventFired` and the stored id, so the player is told nothing. Four are seasonal —
they read `g_season` and pick one of four percentages, which no earlier revision of this
document mentioned.

| id | group | name | guard | effect |
|---|---:|---|---|---|
| `0x87` | 135 | Rats!! | grain ≥ 50 | grain % by season: Sp −30, Su −25, Au **−45**, Wi −40 |
| `0x88` | 136 | Mad Cows !! | herd ≥ 40 | herd −20 % |
| `0x89` | 137 | Wolves. | herd ≥ 40 | herd −40 % |
| `0x8A` | 138 | Plague. | pop ≥ 100 | pop % by season: Sp −30, Su −20, Au −30, Wi **−40**; **and** health meter −20, then clamped to at most 25 |
| `0x8B` | 139 | Grain found. | grain ≥ 50 | grain % by season: Sp **+40**, Su +35, Au +25, Wi +15 |
| `0x8C` | 140 | Bad cattle stock | herd ≥ 40 | herd −10 % |
| `0x8D` | 141 | Cow bonanza!! | herd ≥ 40 | herd +25 % |
| `0x8E` | 142 | Wedding fever. | pop ≥ 100 **and** happiness ≥ 30 | pop % by season: Sp **+60**, Su +50, Au +40, Wi +30 |
| `0x12E` | 302 | Healthy eating. | health meter < 81 | health meter +20 |
| `0x12F` | 303 | Mother nature. | a barren field tile exists | one barren field becomes fallow |
| `0x130` | 304 | Weapons found. | none | realm weapons `[(countyId & 3) + 1]` **+25** |
| `0x131` | 305 | Donation. | none | gold +500 |
| `0x132` | 306 | Treasure | none | gold +1000 |
| `0x133` | 307 | Holy Relic. | happiness < 96 | happiness +5 |
| `0x134` | 308 | Witch !! | happiness < 91 | happiness +10 |
| `0x135` | 309 | Stone found. | none | stone +100 |
| `0x136` | 310 | Pests. (locusts) | a field tile of type 3 … 14 exists | one grain field is made barren |
| `0x137` | 311 | Hags curse. | health meter ≥ 20 **and** herd ≥ 20 | health meter −20 |
| `0x138` | 312 | Fraud. | gold ≥ 500 | gold −500 |
| `0x139` | 313 | Corruption. | that weapon ≥ 25 | realm weapons `[(countyId & 3) + 1]` **−25** |
| `0x13A` | 314 | No bull. | herd ≥ 20 | herd modifier **= 99**, a sentinel meaning "no growth" |
| `0x13B` | 315 | Stop thief!. | shown tax ≥ 10 | county `+0x1A8` = 100 — no tax this season (§4.1) |
| `0x13C` | 316 | Pests. (termites) | wood ≥ 300 | wood −300 |
| `0x13D` | 317 | No songs. | happiness ≥ 7 | happiness −7 |

**[V]** throughout: the guard and the effect are read off the decompiled handler, and the
group's own text says the same thing. Two entries are worth singling out.

**99 is a sentinel, not a percentage.** `Herd_SeasonTick` tests the herd modifier for 99
*before* it tests its sign, and on a match sets both the weather swing and the herd's
births to zero — which is exactly group 314's *"Cattle will not reproduce this season due
to the death of your prize bull. Deaths, however, occur normally."*

> **A second bug: the weapon a county finds is chosen by its id.** `FUN_0044938C` indexes
> the realm's weapon array with `(countyId & 3) + 1` rather than with the county's own
> weapon type at `+0x290`, and `FUN_00449688` computes the same index the same way — so it
> is a shared idiom rather than a slip in one place. A county therefore finds, and has
> embezzled, a weapon that has nothing to do with what its blacksmith makes, and the
> crossbow (type 0) can never be found or stolen at all. **[D]**

The population modifier is capped at 20 % of the county. **[V]** on the guard that matters
most: **the AI never draws random events** — the test is on the *owner realm's* `isHuman`
byte.

### 8.2 The AI's advantages

`AI_SetTaxRates` (`0x0049D638`) does two things: it sets the county tax rate from happiness
on one of four ladders, and, for AI realms only, it hands out free resources.

**The four ladders.** Each is a descending `if`/`else if` chain on the county's happiness;
`AI_SetTaxRates(0)`, which phase 1 runs every turn, uses the first.

| happiness < | 20 | 30 | 40 | 50 | 60 | 65 | 70 | 80 | 90 | 95 | else |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| **neutral** (realm 0) | 0 | | 1 | 2 | 3 | | 4 | 6 | 8 | | 12 |
| **ladder 0** | | 0 | | 2 | | 4 | | 10 | | | 15 |
| **ladder 1** | | 0 | | 1 | | 3 | | 7 | | | 12 |
| **ladder 2** | | | | | 0 | | 1 | 2 | 3 | 8 | 10 |

**[V]**. Ladder 0 is the greediest and ladder 2 the gentlest — nothing at all below 60
happiness. Note that a *neutral* county is taxed harder at low happiness than any AI taxes
its own, though nobody collects it: an unowned county banks its take into itself (§4.1).

All four ladders are now checked against the executable by `tools/oracle/kingdom.ps1` —
not by reading a table, because there is no table. The chains compile to 20 `CMP EAX,
imm8` thresholds interleaved with 24 `MOV byte ptr [county+0xB9], imm8` rate stores, and
the oracle reads that ordered sequence out of the instruction stream. See §13.

The ladder is chosen by the first `int` of the AI lord's **`g_aiPersonality`** record
(`0x004D8A58`, stride `3 × 0x50` = 240; the code addresses it as
`base + (lord × 3 − 3) × 0x50` and only ever uses the first of the three rows). The
records hold, at `+0x00` and `+0x04`:

| lord | 1 | 2 | 3 | 4 |
|---|---:|---:|---:|---:|
| farming style (`+0x00`) | 1 | 1 | 0 | 9 |
| tax ladder (`+0x04`) | 2 | 2 | 2 | 1 |

**There are four records, not five.** §2 says the lord byte runs 1 … 5; a fifth record
would begin at `0x004D8E18`, and what is there fits no pattern — 17 and 0 where every real
record has a farming style of 0, 1 or 9, and 5000 where the four records hold 100, 100,
200 and 50. So `0x004D8E18` is taken to be past the end. **This is now [V] and closed**: it is the
campaign-progression table `FUN_00499E5D` reads with a `0x20` stride, and the four lords are
independently named by `L2.eng` group 7 — the Knight, the Baron, the Countess and the Bishop.
See [`diplomacy.md`](diplomacy.md) §0. Note also that both this table and `g_aiGoldGrant` are
indexed by the **lord byte** (realm `+0x07`), not by the realm index; row 0 of the gold tables
is lord 0, the human. The farming style is copied into county `+0x1FE` by step 5
and dispatched on; 0, 1 and 9 are exactly the three values the dispatch tests, which is a
check on the field's identity. **What each style does was not traced.**

**The grants**, gated on the realm being in play, not human, and holding at least one
county:

* gold from **`g_aiGoldGrant` (`0x004DC1E0`)**, `int[5][4]` by lord and difficulty, when
  the realm holds **three or more** counties — and from the smaller
  **`g_aiGoldGrantSmall` (`0x004DC230`)** when it holds fewer:

  | lord | `g_aiGoldGrant` | `g_aiGoldGrantSmall` |
  |---:|---|---|
  | 0 (the human) | 0, 0, 0, 0 | 0, 0, 0, 0 |
  | 1 | 0, 400, 700, 1200 | 0, 160, 250, 400 |
  | 2 | 100, 500, 800, 1400 | 40, 180, 300, 500 |
  | 3 | 0, 400, 700, 1200 | 0, 160, 250, 400 |
  | 4 | 250, 600, 1100, 1800 | 100, 240, 400, 600 |

* free population, herd and grain, **tiered by the realm's county count** (`+0x29`):

  | realm counties | people | head | sacks |
  |---|---:|---:|---:|
  | 1 … 2 | `d × 20` | `d × 5` | `d × 40` |
  | 3 … 4 | `d × 10` | `d × 2` | `d × 20` |
  | 5 or more | **0** | **0** | **0** |

**[V]** on the code and both tables; the values are byte-identical to a published dump of
the same addresses. An earlier revision of this section gave the goods grant as flat
`d × 20 / d × 5 / d × 40`; those are the **smallest** realm's figures, and a realm with
five counties gets no goods grant at all. Each county's share is still gated on the county
already having some (`pop > 20`, `herd > 10`, `grain > 50`), so it compounds rather than
rescues, and the people are booked as births as well as added to the population.

Both grants therefore **reward a realm that is already ahead**: the small gold table is
uniformly *smaller*, and the goods stop entirely once a realm is doing well. That is the
opposite of rubber-banding.

The human realm's `lord` byte is 0 and row 0 of both gold tables is all zeros, so **the
human gets nothing from either mechanism**. **[V]**

### 8.3 Score

`Score_RankRealms` (`0x0049AA0E`) rebuilds each realm's score:

```
score = realm[+0x60]*10 + realm[+0x10]/10 + realm[+0x0C]*2 + realm[+0x58]*2
      + realm[+0x54]/5  + realm[+0x4C]*50
      + (gold > 10000 ? 200 : gold >= 5001 ? 100 : gold >= 2001 ? 50 : 0)
```

then bubble-sorts realms 1 … 5 into the table at `0x00565410` and writes the rank back to
`realm +0x2B`. It is **not** called from `Season_Advance` — see §3.4.

**Five of the six contributing fields are identified**, from `FUN_0049D1E0`, the AI turn's
fourteenth step, which recomputes exactly these once a turn:

| offset | weight | what step 14 writes there |
|---|---|---|
| `+0x60` | ×10 | `PctOf(ownedCounties, g_countyCount)` — the **share of the map**, 0 … 100 |
| `+0x10` | ÷10 | total population over the realm's counties |
| `+0x0C` | ×2 | mean happiness over the realm's counties |
| `+0x58` | ×2 | mean health meter over the realm's counties |
| `+0x54` | ÷5 | total men over the realm's armies |
| `+0x4C` | ×50 | **still unidentified** |

**[V]** on the five. The score reads, in order of weight, as *territory, then people, then
how well they are doing, then the army*. `+0x4C` carries the heaviest weight of the six and
is not written by that pass; it is left unnamed rather than guessed at (`decisions.md` C3).
The same step also writes `+0x14` (mean population per county), `+0x18` (last turn's
total), `+0x29` (the county count the grant tiers turn on) and `+0x2C` (the army count),
and every division is guarded on the county count being non-zero.

**`+0x04` is not `inPlay`.** §2 calls it that and marks it **[V]**; `FUN_0049B42B` — AI
step 0 — rebuilds it as **`3 × ownedCounties + 1 × armies`**, and the realm is eliminated
when that comes out zero. Every other site only tests it against zero, which is why
"inPlay" fits everything except the write. **[V]**

---

## 9. Reading it back out of a save — the validation

`tools/kingdom/savedump.js` parses `g_saveBlocks` out of `Lords2.exe` and uses it to locate
any global inside a `.sav`. Run against the `lastturn.sav` in the install — a turn-1
autosave of the England map:

```
$ node tools/kingdom/savedump.js globals "F:/games/Lords of the Realm II"
g_countyCount       14      g_season            4        g_turnCount     1
g_scenarioIndex     0       g_seasonNext        1        g_turnPhase     1
g_localPlayer       1       g_year              1268     g_merchantCount 6
g_optDifficulty     0       g_optAdvancedFarming 0       g_optArmiesEat  0
```

Eight independent predictions land:

1. **`g_season` = 4, `g_year` = 1268, `g_turnCount` = 1** — §3.3 predicted a new game
   starts in Winter 1268 from `Game_NewGame`'s constants plus one `Season_Advance`.
2. **`g_countyCount` = 14** on the England map, and county records 15 and 16 are all zero
   while 1 … 14 are populated — the 17-record array with an unused index 0.
3. **`g_optAdvancedFarming` = 0, and every county's weather byte is 3 (Cloudy) and every
   fertility is 0** — exactly the two overrides §7.2 and §7.3 say that option forces.
4. **`g_merchantCount` = 6.** [`plane4.md`](formats/plane4.md) §5 lists "six merchants on
   England" as a falsifiable, unobserved prediction. **It is now observed.**
5. **Happiness reproduces exactly.** Every county has `happinessLast = 65`,
   `shownTax = +5`, `shownHealth = +1`, `shownRation = +1`. Owned counties store
   happiness **72 = 65 + 5 + 1 + 1**; unowned counties store **77**, with
   `shownEvents = +5` — the unowned bonus in §4.4. Fourteen counties, two cases.

   **Corrected: there are five owned counties, not four, and they belong to five different
   realms.** The owner bytes are `5` at index 1, `4` at 4, `1` at 8, `3` at 11 and `2` at
   13 — one county for each of realms 1 … 5, nine unowned, and the realm records agree
   from the other side with `+0x29 = 1` apiece. The person is realm 1 and holds county 8
   alone. An earlier revision of this section said *"four counties owned by the human
   realm, ten unowned"*, and `crates/l2-kingdom/tests/reproduction.rs` was built on that
   invented scenario rather than on the file — `docs/decisions.md` C12 a second time. Both
   are fixed: the test now imports the save through `crates/l2-scenario` and compares
   against the stored bytes.
6. **The tax term reproduces.** `taxRate` is 0 everywhere, and `dHapTax = 5 − 0 = 5`.
7. **The health chain reproduces.** `healthMeter` is 67 and `healthBand` is 3, which is the
   ladder's `≤ 90 → 3`. A published dump of the new-game presets gives a starting health of
   **65** for a medium county — which is band **2** on the same ladder, since `65 ≤ 65` —
   and `g_healthDeltaTable[Normal][band 2]` is **+2**, giving `65 + 2 = 67`. One number
   pins the ladder's comparison sense and the delta table's indexing at once. It was
   **[I]**, taken from prior art rather than from this binary; it is **[V]** now — the
   starting-position table of §13.2 holds `65` in the health column of the row whose herd
   and population the save also carries.
8. **Births and deaths reproduce exactly, on two different happiness bands.** Every county
   started at `popLast = 417`, so `g_birthRateLadder` gives 20 %:

   | | happiness | factor | births | deaths | population |
   |---|---:|---:|---:|---:|---:|
   | owned (5 counties) | 72 | 75 % | `Pct(417, Pct(20,75)) + 1` = **63** | `Pct(417, 3+8)` = **45** | `417+63−45` = **435** |
   | unowned (9 counties) | 77 | 100 % | `Pct(417, 20) + 1` = **84** | **45** | **456** |

   and the stored values are 63/45/435 and 84/45/456. The deaths figure needs
   `g_deathRateByHealth[3] = 3` **and** `g_deathRateBySeason[4] = 8`, so it also confirms
   the season index independently.

That is a five-stage chain — ration → health meter → health band → happiness → birth rate →
population — reproducing on live data from a real game, with no free parameters. It is the
strongest evidence in this document and the reason most of §4 and §5 is marked **[V]**.

**And it is now a test rather than a paragraph.** `crates/l2-scenario` imports
`lastturn.sav` into a live `l2_kingdom::Kingdom` — the seam exists because `l2-kingdom` may
not know what a file is and `l2-formats` may not know what a county is — and
`crates/l2-kingdom/tests/reproduction.rs` rewinds it one season, runs `Season_Advance`, and
compares **twenty-six stored fields across all fourteen counties**. Nothing in that file is
quoted from this document any more.

The rewind needs one input the save does not hold: the herd and grain the ration pass ate,
since only the remainder survives. §4.3 shows it is recoverable and unique.

`popBand` (`+0xB8`) also checks: `(435−1)/25 + 1 = 18` and `(456−1)/25 + 1 = 19`, which are
the stored values.

---

## 10. Where the numbers live — and what a mod can reach

**Everything in this document is a constant inside `Lords2.exe`.** No kingdom-layer rule is
loaded from a data file.

`Lords2.exe` references exactly seventeen file names, and they divide cleanly:

| file | what it feeds |
|---|---|
| `l2_maps.dat` | the campaign map, county partition, merchant routes ([`maps.md`](formats/maps.md)) |
| `l2.eng` | all UI text, including the strings that name the enumerations above ([`eng.md`](formats/eng.md)) |
| `troops.eng`, `troops2.eng`, `troops3.eng`, `battles.eng` | **skirmish only** — see `battle.md` §10 |
| `castles.dat` | sixteen 12,800-byte castle *plans*, one per county slot. Working state, not rules: `Castles_CreateFile` writes it out zeroed at the start of a new game |
| `lastturn.sav`, `old_turn.sav`, `safeturn.sav`, `net1game.sav`, `testgame.sav` | state snapshots, per §0 |
| `score.dat`, `l2.ini`, `sierra.ini`, `lords2.inf`, `l2help.hlp` | scores, settings, help |

So **a mod that wants to change an economy rule must patch the executable**, in the same
way `battle.md` found for the real-time battle. The consolation is that the constants are
well-clustered and mostly in one place: `0x004D6300 … 0x004D6800` holds the population,
health, ration and weather tables, and `0x004D8900 … 0x004D8A60` holds prices, weapon
costs and the five castle tables. Both regions are contiguous, aligned int arrays with no
code interleaved.

Several exceptions are worth naming for a reimplementation, because they are the rules a
reader looking for a table will never find one for:

* the **castle tax multipliers** in `Tax_CollectAll` are immediates in the instruction
  stream, not a table — the parallel `g_castleTaxBonus` table is used only by the UI;
* the **grain sow ladder** (10 down to 1) and the **ration happiness formula** (`3L − 8`)
  are likewise arithmetic, not data;
* the **four AI tax ladders** of §8.2 are four `if`/`else if` chains inside
  `AI_SetTaxRates` — 20 threshold `CMP`s and 24 rate stores, and no table anywhere;
* the **ale ladder** of §12 is `population / 10` against five rungs, with the divisor and
  the cap as `MOV` immediates in `FUN_00428C42`;
* the **efficiency ramp's** flat 80 and ceiling of 100 are immediates in `FUN_0044F248`.

The last three were exactly the rules `crates/l2-mods` was last to reach, which is not a
coincidence: there is nothing to transcribe, so nobody transcribed anything. Those three
are now checked against the binary by reading the instruction stream — see §13. The first
two are not yet, and that is a gap rather than a decision.

`0x004D8910` is worth one more note: it is the merchant base-price table, and indexing it
by `L2.eng` group 6's fifteen good ids explains two slots that a published dump listed as
unlabelled gaps — they are **sheep** and **wool**, both priced 0.

| good | 1 grain | 2 cattle | 3 sheep | 4 ale | 5 wool | 6 iron | 7 stone | 8 timber |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| price | 2 | 12 | 0 | 1 | 0 | 1 | 2 | 1 |

| good | 9 pikes | 10 bows | 11 maces | 12 crossbows | 13 swords | 14 mail |
|---|---:|---:|---:|---:|---:|---:|
| price | 13 | 16 | 10 | 24 | 23 | 44 |

**[V]** Fifteen table entries, fifteen strings in group 6, and the two zeros land exactly on
the two goods a county cannot produce or trade in the base game.

---

## 11. Prior art, and where it was wrong

Searched first, per `CLAUDE.md` rule 3 and `decisions.md` C5, then checked against the
binary, per C8.

**OpenLotR2 has nothing on this layer.** Its "Game" chapter is a reflow of the printed
manual and its "Technical" chapter covers graphics and `.skr` only. There is no GPL-3
exposure here because there is no material.

**Two independent projects do have real data**, and both agree with what is derived above:
a Ghidra-based write-up gives `g_counties` at `0x0053F9B0` stride `0x300`, realm stride
`0x160`, realm `+0x05` human, `+0x28` happiness, county `+0x24` population, `+0xBC`/`+0xC0`
tax collected/shown, and the same wage divisors; a binary-editor plugin dumps
`0x004D49B8 … 0x004DE8C8` at 4-byte granularity and its values match this binary
byte for byte at every address checked here. Independently derived first, then confirmed —
which is the useful order.

Three published claims are **wrong**, and the binary says so:

| claim | source | what the binary says |
|---|---|---|
| "the county array holds counties 1..14" | RE write-up | 17 records; 14 is the county count *of the England map*, which is what `g_countyCount` reads in the England turn-one fixture |
| "up to 5 sacks of grain per field" | **the printed manual**, twice | `g_grainMaxSacksPerField` = 10 (§7.1) |
| "keep at least a third of your fields fallow" | the printed manual | the rule is one fallow per **two** grain fields, and cattle fields are excluded (§7.2) |

Two published claims are **right against the manual**, which is worth recording because the
manual has been the more trusted source on this project: the 10-sack figure and the
one-in-two fallow ratio both come from players measuring their own saves.

One published disagreement is **resolved**: a guide's merchant prices are uniformly twice
the table's, and the manual explains why — the merchant scroll shows *"such as 30/60. The
left number is the selling price… the right number is the buying price"*. The table holds
the sell price.

One is **not resolved and is left open**: a mercenary table carries a third column at
exactly 10 % of the hire price, which reads like a seasonal wage, but a player measuring a
garrison containing 150 mercenaries got `floor(men/4)` with no premium — and §7.4 confirms
`Wages_ForUnit` has no mercenary term. Either the 10 % column is not a wage, or it is
charged somewhere this document did not look.

---

## 12. What is still unknown

The list below is what remains after the pass that traced the fourteen AI handlers, the
event table, the bankruptcy stages, the efficiency ramp, `resourceLimit`, both AI gold
tables, the history ring, the ale and army happiness terms and `Grain_Sow`'s labour slot.
Each of those is now written out in the section it belongs to.

**Still unknown, and named rather than guessed at:**

* **What ten of the fourteen AI handlers do in detail** (§3.2). All fourteen are located
  and each has a one-line description, but steps 1, 2, 4, 6, 7, 9, 10, 11, 12 and 13 drive
  armies, merchants, diplomacy and map tiles and were read only far enough to say what they
  are for. This is still the largest remaining piece of the kingdom layer, though it is now
  a bounded one: ten named functions rather than an unexplored region.
* **The three AI farming styles.** `AI_ManageFields` copies the lord's style into county
  `+0x1FE` and dispatches on 0, 1 or 9 into `FUN_004A4052`, `FUN_004A42E3` and
  `FUN_004A440F`. None was traced. This is why `kingdom.ai.personality.*.farm_style` in
  `crates/l2-mods` loads and changes nothing: the value is carried and there is no
  behaviour to attach to it. `docs/modding.md` §11 says so in those words.
* **A fifth AI lord's personality record.** §8.2 finds four; `0x004D8E18` is where a fifth
  would be and what is there fits no pattern. **[I]**, and now held by the oracle: it reads
  those six ints and expects `17, 0, 5000, 1, 1, 1`, so if the reading is ever wrong the
  check is where it shows.
* **The sixth score input**, realm `+0x4C` (§8.3), which carries the heaviest weight of the
  six and is not written by the pass that writes the other five.
* **The two denominators the weapons `resourceLimit` divides by** — `0x0057C904` and
  `0x0056D628`, written by `FUN_0044F15B` (§7.4).
* **The second column of `g_castleWorkforce`** (§7.5). Both columns hold the same number in
  all five rows, so nothing here distinguishes a duplicate from a second quantity.
* **`localModifier`** (`FUN_00449D6E`, §7.3), the per-county weather swing.
* **Trade.** `plane4.md` closed the merchant *movement*; the transaction is still open. The
  price table is §10; how a merchant's offer is generated from it, and what the second
  15-entry table at `0x004D8950` is, are not established.
* **Fertility's effect.** `+0x208` runs −100 … +100 and `L2.eng` group 22 names seven
  levels, but where the crop yield reads it was not found. The yield multipliers in
  `Grain_Grow` and `Grain_Harvest` were not decompiled beyond their weather branches.
* **The food split** (§4.3) — the unowned-county case reproduces exactly and the
  owned-county case does not.
* **Health's other inputs.** `g_healthDeltaTable` is indexed by ration and band only. Three
  random events also move the meter (§8.1) and nothing else was enumerated.
* **About 140 county fields.** 201 offsets inside the record are referenced by the binary.
  52 were named when this section was first written; sections 4.1, 7.1, 7.4 and 8.1 have
  since added +0x1A7, +0x1A8, +0x1AA, +0x1F4, +0x1FE, +0x219 and the five industry-record
  fields at +0x295 … +0x2A0.
* **No runtime confirmation of anything dynamic.** Everything here is static: the binary,
  the shipped data files, `L2.eng`, the manual, and one turn-1 save. No game was run, per
  `decisions.md` D8 and the rule about processes an agent starts.

**Bugs, which are findings rather than gaps.** The first three are reproduced in
`crates/l2-kingdom` with a comment saying why; the rest are recorded here and not
reproduced, for the reason each line gives.

* **Only odd-numbered counties can draw a random event** (§8.1) — the deck's seed is always
  even and every dealt slot is odd. *Reproduced.*
* **A found or embezzled weapon is chosen by `(countyId & 3) + 1`** (§8.1) rather than by
  the county's weapon type, so the crossbow is unreachable. *Reproduced.*
* **The empire tax term is summed into a signed byte** with nothing clamping it (§4.1).
  *Reproduced.*
* **The AI turn's sentinel is stored as 1000**, because the increment below the dispatch
  sits outside the `if` that writes 999 (§3.2). *Reproduced, and every reader in the
  original tests `< 999`, so nothing in the original depends on it.*
* **`Grain_Harvest`'s weather branches overwrite the labour-limited result** with a
  multiple of the raw crop at `+0x244`, rather than scaling the value the labour check just
  returned — so in any weather but Drought or Cloudy the labour limit is discarded
  entirely. **[D]**. *Not reproduced: `crates/l2-kingdom` does not model the labour limit on
  growing and harvesting at all, so there is nothing for the overwrite to discard. Both
  halves are open.*
* **The migration inflow list is written without a `break`**, so it holds one repeated
  value (§5.3). **[D]**
* **`dryness` is a signed byte that nothing clamps** (§7.3), so a long enough dry run wraps
  a drought into a flood. **[D]**, and unreachability was not shown.


---

## 13. Reproduction

```bash
# the save-block table, and the size invariant that validates it
node tools/kingdom/savedump.js layout  "F:/games/Lords of the Realm II"

# every county and realm record in a save, decoded with the field map above
node tools/kingdom/savedump.js county  "F:/games/Lords of the Realm II"
node tools/kingdom/savedump.js realm   "F:/games/Lords of the Realm II"
node tools/kingdom/savedump.js globals "F:/games/Lords of the Realm II"
node tools/kingdom/savedump.js raw     "F:/games/Lords of the Realm II" 53f9b0 768

# push the kingdom names into docs/symbols.json, then regenerate the tables
node tools/kingdom/addsymbols.js
node tools/symbols/symbols_md.js
```

```powershell
# decompile, cross-reference, dump and locate strings, into tools/kingdom/out/
powershell -File tools/kingdom/ghraw.ps1 -postScript KDecomp  turn.c 0049a010
powershell -File tools/kingdom/ghraw.ps1 -postScript KRefs    r.txt range 53f9b0 53fcb0
powershell -File tools/kingdom/ghraw.ps1 -postScript KDump    t.txt bytes 4d6738 48
powershell -File tools/kingdom/ghraw.ps1 -postScript KCallArg e.txt 00402d37
powershell -File tools/kingdom/ghraw.ps1 -postScript KStrRefs castles.dat
```

Ghidra scripts live in `tools/kingdom/ghidra/` (`KDecomp`, `KRefs`, `KDump`, `KCallArg`,
`KStrRefs`), kept separate from `ghidra_scripts/` so parallel agents do not edit the same
files. Their output goes to `tools/kingdom/out/`, which is gitignored — it is derived from
the game binary. The names are pushed into the Ghidra database with
`ghidra_scripts/ApplySymbols.java` as described in [`symbols.md`](symbols.md).

`tools/kingdom2/` is a second copy of the same five scripts against its own Ghidra project
(`E:\dev\ghidra-projects-kingdom2`, project `l2k2`), created so the work in this revision
could run without holding the shared `lords2` project. Either will do; use whichever is
free.

**Checking the tables against the binary rather than against this document:**

```powershell
powershell -File tools/oracle/kingdom.ps1 -Source File
```

It runs **29 checks** straight out of `Lords2.exe` at the addresses named above and
compares them against the values written here. It needs no running process. That is the
check that caught two of this document's layout errors — `g_healthBandLadder`'s pairs and
`g_castleWorkforce`'s stride — and it should gain a row whenever a table is added.

Twenty-four of the 29 read initialised `.data`. **Five read `.text`**, because the rule
they check is not a table at all:

| check | what it reads | why there is no table |
|---|---|---|
| `AI_SetTaxRates ladders` | 20 `CMP EAX, imm8` thresholds interleaved with 24 `MOV byte ptr [county+0xB9], imm8` rates | the four ladders of §8.2 are four `if`/`else if` chains |
| `ale ladder` | `MOV EAX, 5` (the cap), `MOV ECX, 10` (the step divisor), and the `MOV dword ptr [ebp-8], imm32` rungs | §12's ale term is arithmetic |
| `efficiency ramp` | `MOV EAX, 0x50` and the `CMP`/`MOV` pair holding 100 | §7.4's ramp bounds are immediates |
| `herd crowding bands` | the `MOV dword ptr [county+0x25C], imm32` stores and the `CMP [ebp-8], imm8` densities they hang off | §13.1's four bands are an `if`/`else if` chain, twice over — once for the level and once for the map graphic |
| `herd births and deaths` | 31 tagged immediates: the staffing cap, the `/ 3` shortfall divisor, the four death rates, the four birth rates, the three small-herd bonuses and the two season codes | §13's whole rule is `FUN_0044DA99`'s instruction stream and there is no table anywhere in it |

This is `decisions.md` **C16** applied to rules rather than to `Rules_InitConstants`'
globals: *when a value is absent from the data, read the code that produces it.* These
three were the last rules `crates/l2-mods` could not reach, and that is not a
coincidence — there was no table to transcribe, so nobody had transcribed one.

The scan is a byte-pattern walk and not a disassembler, so it cannot tell an instruction
boundary from a byte inside an operand. That is safe here because each check compares the
**whole ordered list** of tagged immediates: a stray match fails the check loudly rather
than passing it quietly.

Three data checks were also widened at the same time. `g_armyHappinessCost` is checked
over all 102 entries rather than the first 32; the AI personality records for lords 2 and
3 are checked rather than only 1 and 4; and `0x004D8E18` — where a fifth record would
begin — is pinned at `17, 0, 5000, …`, which is the evidence that the table holds four
lords and not the five §2 implies.

---

## 13. The herd needs tending  **[V]**

A player's description of the game named three cattle mechanics. Two we had; one we did
not, and it is not small. This section was first written as a **gap in `crates/l2-kingdom`**;
it is now the rule the crate implements.

`Herd_SeasonTick` (`0x0044D60D`) calls `FUN_0044DA99` with five arguments, and the third is
the giveaway:

```c
FUN_0044DA99(county,
             county[0x250],      /* herd, less what the ration pass ate  */
             county[0x0D0],      /* labour: record 1 of the +0xC4 block  */
             county[0x25C],      /* crowding - §13.1                     */
             g_season);
```

`+0xD0` sits inside the labour block at `+0xC4`, so **the herd's births and deaths take a
labour argument.** The whole of it:

```c
if (herd == 0) return;                                 /* nothing to do        */
if (fieldsCattle == 0) {                               /* +0x200 - no pasture  */
    deaths = (herd < 6) ? herd : herd / 2;             /* half, or all of it   */
    return;                                            /* and nothing else     */
}
staffing = PctOf(labour, herd * 3);                    /* labour*100/(herd*3)  */
if (staffing >= 200) staffing = 200;                   /* twice is the ceiling */

deathRate = {10:1, 20:3, 30:5, else:7}[crowding];
if (staffing < 100) deathRate += -((staffing - 100) / 3);

birthRate = Pct({10:1400, 20:900, 30:500, else:200}[crowding], staffing);
if (staffing >= 100) {                                 /* a small herd breeds  */
    if      (herd <  5) birthRate += 10000;            /* faster - but only if */
    else if (herd < 10) birthRate +=  5000;            /* somebody is tending  */
    else if (herd < 25) birthRate +=  2000;            /* it                   */
}

deaths = (herd * 100) * deathRate / 10000;             /* FUN_00404D96         */
if (season == 4) deaths = deaths * 3 / 2;              /* Winter kills         */
births = herd * birthRate / 10000;
if (season == 1) births = births * 3 / 2;              /* Spring calves        */

if (births == 0) {                                     /* a rate that rounds   */
    if (birthRate != 0)                 births = 1;    /* away still moves one */
    else if (deaths == 0 && deathRate)  deaths = 1;    /* animal               */
}
if (deaths > herd) deaths = herd;
```

**Three labourers per head is full staffing.** Below it the shortfall is divided by three
and **added to the death rate**, so a herd nobody is working loses `crowding + 33` per ten
thousand a season; above it the benefit is the birth rate only, and it stops at 200%. That
is exactly the reported behaviour — *"cows require more peasants to tend them depending on
how many cows there are"* and *"some cows die by being unattended"* — arrived at
independently from play and from the instruction stream.

> An earlier revision of this section annotated `-((staffing - 100) / 3)` as
> *"understaffed: negative"*. It is **positive**: the operand is negated after a truncating
> division, and the result is added to the deaths rather than to the growth.

**A county with no pasture is not lightly penalised.** The `fieldsCattle == 0` branch above
everything else halves the herd — or kills all of it below six head — and no other term
runs. It is what makes `+0x200` load-bearing rather than decorative.

### What this means for us — **done**

`land::herd_growth` is the function above, `land::herd_crowding` is §13.1, and
`land::herd_season_tick` takes the season and next season as `Herd_SeasonTick` does.
The labour comes from `County::labour[JOB_CATTLE_FARMING]`, which `l2-scenario` now
imports; it did not before, and a county with no imported labour would have lost cattle
every season for want of a field nobody had read.

**The labour import checks itself.** The nine records are `+0xC4 + job * 0x0C`, worker count
at word 0 — fixed by the allocator `FUN_0044F6E7`, which clears them with
`for (c = 0; c < 9; c++) *(int *)(county + 0xC4 + c * 0x0C) = 0;` — and in the England turn-one fixture
**every county's nine records sum to its population exactly**: 218 + 217 = 435 in county 1,
323 + 133 = 456 in county 2, and so on for all fourteen. No wrong stride does that
fourteen times running.

### How the rule was checked, and what it cost the reproduction

`Herd_SeasonTick` ends by calling `FUN_0044DD4D`, which writes next season's forecast into
`+0x268`, `+0x26C` and `+0x258` — group 77's *"Calf births expected"*, *"Cow deaths
expected"* and *"Change due to farming"* — from state the save also holds. So the whole
function can be run against fourteen counties' worth of stored answers with **no inversion
at all**, and all fifty-six numbers reproduce: county 1 through the understaffed arm at 98%
staffing, county 2 through the capped arm at 199%, three of the four crowding bands, the
Spring bonus, and the second subtraction of `herdEaten` that makes *"change due to
farming"* what it is.

`change = (births - deaths) - herdEaten` really does count the eating twice: the ration pass
has already taken this season's animals, and the panel assumes next season takes as many
again.

The same function also searches `labour = 0 … population` for the worker count that best
suits the herd, writing a suggestion to `+0xD4` and the growth-maximising figure to `+0xD8`
— the second and third words of labour record 1, which the labour allocator then fills up
to. `crates/l2-kingdom` does not reproduce the search (its labour is one integer a job), but
the search is a full sweep of `FUN_0044DA99` over the labour domain and it lands on the
stored bytes: 302 and 302 for county 1, **106 and 323** for county 2.

**What it cost.** `crates/l2-kingdom/tests/reproduction.rs` used to reproduce `herd` and
`herd_eaten` and no longer does, and the reason is worth stating plainly: *they reproduced
because the rule was missing.* With the herd moving only on this map's neutral weather,
"put back what the ration pass ate" was the whole of it. The file disagrees — `+0x254` is
the herd as `Herd_SeasonTick` found it, and it is **95 in every one of the fourteen
counties**, against the 73 the inversion recovers. Getting from 95 to the stored herd needs
the *pre-season* cattle labour, and the save holds only the post-season allocation; a search
over openings finds two or three for each owned county and **none at all** for the nine
unowned ones. Modelling `FUN_0044F6E7` would bring both fields back.

Also confirmed from the same description, and already correct: sowing debits the store
(`Grain_Sow`'s `store -= sown`), dairy feeds people at `g_dairyPerHead`, cattle are eaten,
and a population icon stands for `popBand` people rather than one.

### 13.1 Crowding is `herd / fieldsCattle`, in four bands  **[V]**

The untraced `+0x25C` above is the crowding level, and a player naming four levels is what
found it. `FUN_0044D913(county)`:

```c
density = (herd < 1) ? 0 : (fieldsCattle == 0) ? 1000 : herd / fieldsCattle;
if      (density < 11) crowding = 10;
else if (density < 21) crowding = 20;
else if (density < 31) crowding = 30;
else                   crowding = 40;
if (fieldsCattle == 0) crowding = 40;      /* no pasture is maximum crowding */
```

> An earlier revision of this listing put the `herd < 1` test on the *bands* rather than on
> the density. It is on the density (and on the map graphic); the level is written
> unconditionally, so an empty herd on real pasture sits at density 0 and therefore in the
> **lowest** band, not in none of them.

**The level is a stored field, not a derived one**, and the difference is observable:
`FUN_0044D913` runs at the *end* of `Herd_SeasonTick`, so a season's births and deaths are
worked out at the crowding the herd had when the season began. `crates/l2-kingdom` stores it
for the same reason.

and `L2.eng` group **77** names them in the game's own words — the panel draws slots 8…11
from exactly these four values:

| density (head per field) | `+0x25C` | `L2.eng` 77 | into `FUN_0044DA99` |
|---|---:|---|---:|
| 1 … 10 | 10 | *"Low herd crowding."* | 1 |
| 11 … 20 | 20 | *"Average herd crowding."* | 3 |
| 21 … 30 | 30 | *"Herd overcrowded."* | 5 |
| 31 + , or no pasture | 40 | *"Massive overcrowding!!"* | 7 |

The same function also selects a map graphic (`0x13`…`0x16`) from the same bands, so
**crowding is visible on the field art**, which is how a player sees it without opening a
panel. Group 77 also carries *"Calf births expected"*, *"Cow deaths expected"* and *"Change
due to farming"*, which is the panel this whole calculation feeds.

**How this was found is the point.** A player said cattle are affected by crowding, in about
four levels. The binary had a four-way branch on an argument nobody had traced, `L2.eng` had
four strings, and the bands closed. None of the three would have been conclusive alone:
recollection does not give `herd / fieldsCattle` or the boundary at 11, and no amount of
reading `FUN_0044DA99` volunteers that the four constants are *crowding* rather than a
ration level — which is what the earlier `[I]` in §13 guessed, wrongly.

### 13.2 The new-game starting position is a table  **[V]**

Found while working out why §13's rule stopped the reproduction reproducing the herd, and
worth its own heading because of what it settles. `FUN_0049BD99` sets a new game up from
`0x004DC0D0 + startingWealth * 0x14`, five `i32` a row:

| row | grain | herd | population | health | health |
|---|---:|---:|---:|---:|---:|
| 0 | 10 | 40 | 167 | 45 | 41 |
| **1** | **0** | **95** | **417** | **65** | **65** |
| 2 | 500 | 330 | 1181 | 85 | 85 |

Both the live field and its `…Last` twin are written from the same column, which is why the
England turn-one save still carries row 1 in three places: `popLast` is 417 in all fourteen counties,
`+0x254` is 95 in all fourteen, and **65 is the health meter** `crates/l2-scenario` carried
as `STARTING_HEALTH_METER` with an `[I]` saying it was *the one number in the whole
reproduction that comes from prior art rather than from the binary.* It is in the binary,
and this is where. The same function then hands an unowned county +100 grain, which is the
100 sacks the nine unowned counties still hold.

A fourth row of zeros follows, so there are three starting-wealth settings and not four.

---

## 14. Labour — the record is three integers, and there is an allocator  **[V]**

`docs/screens-county.md` §8.3 said the labour record was three integers a job and that only
the first was imported. All three are now read, and the pass that writes the other two and
then acts on them is reproduced.

### 14.1 The record

`+0xC4 + job*0x0C`, nine records of twelve bytes:

| word | off | meaning |
|---|---|---|
| 0 | `+0x00` | workers assigned. The nine sum to the population, exactly, always |
| 1 | `+0x04` | the **wanted floor** — `-1` for "no floor" |
| 2 | `+0x08` | the **useful ceiling** — `100000` for "none", `999999` for "never computed" |

Only two passes ever write a real floor, and both find it the same way — by trying every
staffing from nobody to everybody and taking the first that works:

* `Grain_LabourEstimate` (`0x0044D374`) runs `Grain_Sow` / `Grain_Grow` / `Grain_Harvest`
  for `workers = 0 … population` and stores the **first count that reaches the best result**
  in *both* words. Grain's floor and ceiling are therefore equal.
* `Herd_LabourEstimate` (`0x0044DD4D`) runs `Herd_BirthsAndDeaths` over the same range and
  stores the **first staffing at which births less deaths stops being negative** as the
  floor, and the staffing that **maximises** it as the ceiling. If the herd cannot break
  even at any staffing it stores the least-bad count instead.

Everything else writes `-1` and a ceiling: `Field_ReclaimEstimate` the work left in the
county's reclaimable fields, `Castle_BuildEstimate` the work the current build still needs,
and `Industry_LabourEstimate` (`0x0044F318`) either the output-maximising smith count, or
**100,000** for iron, stone and wood, or **0** where the county has no such resource.

### 14.2 The allocator

`Labour_Allocate` (`0x0044F6E7`), 2,147 bytes, and the only writer of word 0:

1. `industry = Pct(population, +0x08)`, `farm = population - industry`.
2. Each farm job takes `Pct(farm, +0x130 + job*4)` people, or as many as its ceiling allows,
   **cattle first**, then grain, then reclamation.
3. The leftover farm people are walked round the three in an uneven rota — grain gets two
   places and cattle three before reclamation gets one — until the pool empties or every job
   is full.
4. Farm leftovers **smaller than one peasant icon** (`+0xB8`) are handed to industry.
5. The industry half repeats it: wood, stone, iron, blacksmith, castle by quota, then one
   place each for the first four before castle building gets one.
6. Whatever neither half could spend becomes **Idle townsfolk** (record 8).

`Labour_RecomputeShares` (`0x00450000`) then rewrites the eight percentages from what
actually happened, in two groups each renormalised to exactly 100, and
`Labour_RecomputeIndustryShare` (`0x0044FF4A`) rewrites `+0x08` counting **half the idle
count as industry**. `Labour_Move` — the village screen's drag — calls both.

`Season_Advance` calls the allocator for every county **twice**: after `Castle_BuildTick`
and before migration, and again after the armies are recounted. Both are immediately after
something changed how many people there are, which is what keeps the nine records summing to
the population every season.

### 14.3 What checks it

The shipped save is the state right after the pass ran, so its own worker counts are the
answer. `l2_kingdom::labour::allocate` rebuilds them from the population, `+0x08`, the eight
percentages and the eight ceilings, and **all fourteen counties come back exactly** —
county 1's 218 dairy maids and 217 foresters, county 8's 327 and 108, and 323 with 133 idle
in each of the nine unowned ones.

That last contrast is the ceiling doing all the work: an owned county's wood ceiling is
100,000 and an unowned one's is 0, so identical populations end up as a county full of
foresters or a county full of idlers.

**Not wired into `phase.rs`.** The allocator's inputs are the seven estimate passes
`County_RefreshEstimates` (`0x004485A5`) runs, and this crate has none of them. Running the
allocator every season against a stale ceiling would move people on evidence that had
stopped being true. `l2_kingdom::labour::SEASON_CALL_SITES` records where it belongs.
