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
| `+0x16` | i8 | taxHapOther | [V] | group 86 index 4, *"Other counties"*; summed across the realm into realm `+0x28`. |
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
| `+0xC4 + job*0x0C` | i32 | labour | [V] | workers assigned to each of ten jobs. §7. |
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
| `+0x290 + c*0x18` | — | industry | [D] | per-commodity production records; `+0x294` is the efficiency percentage. |

---

## 2. The realm record — `g_realms`, `0x0057BF00`, stride `0x160`

Six records; index 0 is unused and 1 … 5 are the players. `battle.md` already refers to
this array's `+0x05` without naming the array.

| Off | Type | Name | Ev | Meaning |
|---|---|---|---|---|
| `+0x00` | i32 | aiStep | [V] | 0 … 14 program counter through the AI's turn; **999 = finished**. §3.2. |
| `+0x04` | u8 | inPlay | [V] | realm exists. |
| `+0x05` | u8 | **isHuman** | [V] | when set, the AI turn machine is skipped entirely. This is the same byte `battle.md` §6.2 could not explain the meaning of; it means "a person is driving this realm". |
| `+0x07` | u8 | lord | [V] | 0 for the human, 1 … 5 for an AI lord, **6 when eliminated**. Indexes `g_aiPersonality` and `g_aiGoldGrant`. |
| `+0x28` | i8 | taxHapEmpire | [V] | sum of every owned county's `+0x16`; added to every county's tax happiness term. **A signed byte summed over up to 16 counties — it can overflow.** |
| `+0x29` | u8 | countyCount | [I] | owned counties; selects between the two AI gold-grant tables. |
| `+0x2B` | u8 | rank | [V] | 1 … 5 from `Score_RankRealms`. |
| `+0x50` | i32 | score | [V] | recomputed every turn. §8. |
| `+0xFC` | i32 | wages | [V] | this season's army bill. §7.4. |
| `+0x118` | i32 | **gold** | [V] | the treasury. |
| `+0x120` `+0x128` `+0x130` | i32 | iron, stone, wood | [V] | the realm-wide stockpiles `Industry_Produce` credits. `L2.eng` group 70 is *"Gold, Arms, Iron, Stone, Wood"*. |
| `+0x140 + t*4` | i32×6 | weapons | [V] | one counter per weapon type. |
| `+0x158` | u8 | bankruptStage | [D] | 0 … 5, the escalation in `Wages_PayAll`. |

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

`AI_RunTurnStep` (`0x0049A581`) picks the next in-play realm round-robin, and dispatches
on that realm's `+0x00` into fourteen handlers — tax, fields, food, industry, castles,
armies, diplomacy — incrementing it each time. When it passes `15 + 2 × realmIndex` the
realm is marked 999 and phase 4 ends for it. **[D]** on what the individual handlers do;
none was traced. **[V]** on the structure and the 999 sentinel, which `Turn_AllRealmsDone`
tests.

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
├── Industry_Produce x4         wood, iron, stone, weapons        §7.4
├── Castle_BuildTick            castle construction               §7.5
├── Migration_UpdateAll         emigrants and immigrants          §5.3
├── Population_UpdateAll        births, deaths, new population    §5
├── Score_RankRealms            scores and the ranking            §8.2
└── history ring, then Ration_Apply again as next season's preview
```

**[V]** the call list and its order; **[D]/[I]** the one-line descriptions, per the
sections they point at.

---

## 4. Happiness — the centre of the whole layer

### 4.1 Tax

`Tax_CollectAll` (`0x0044B59B`), per county:

```c
/* castleType is county +0x1C0, but when the flag at +0x1C3 is set the lower of
   +0x1C0 and the +0x1C1 castle-under-construction value is used instead, and a
   zero +0x1C1 forces type 0.  The flag was not traced; a siege or a partly razed
   castle would both fit and neither is established.                          [D] */
base = county.f1A8 != 0 ? 0                              /* another untraced gate */
     : castleType == 0 ? 320 : (int[]){480, 560, 640, 720, 800}[castleType - 1];
take  = Pct(Pct(population, base), taxRate);       /* Pct(x,p) = x*p/100 */
county.taxCollected = take;
realm.gold += take;
county.dHapTax = (5 - taxRate) + realm.taxHapEmpire;
```

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

> **A caution about the empire term.** `Tax_SumEmpireHappiness` sums signed bytes from up
> to sixteen counties **into a signed byte**. Nothing clamps it. Whether it actually wraps
> in play was not tested here. **[D]**

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
(`0x004D6520`)**: `≤10 → 0, ≤35 → 1, ≤65 → 2, ≤90 → 3, else 4`. The band then produces the
happiness term from **`g_healthHappiness` (`0x004D6548`)**:

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

> **What reproduces and what does not.** In the shipped `lastturn.sav`, an unowned county
> has population 456, herd 67, ration *Normal*, split 100 %. The model gives
> `456 − 67×5 = 121` people left to feed and `DivCeil(121, 10) = 13` head — and the stored
> `+0x17C` is **13**, for all ten unowned counties. The four player-owned counties do
> **not** reproduce: they store `+0x17C = 3` where the model gives 0. Those fields are
> written several times per turn from different call sites (the last write is a preview for
> the *next* season — the stored `rationAchieved` is likewise the next season's level, not
> the one that was applied), and this document did not untangle which write survives.
> Flagged rather than smoothed over.

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

**[V]** and reproduced exactly from the save — §9. The *army* and *ale* terms exist as
fields and as panel labels but are written elsewhere; the ale purchase path was not traced.

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
divisor is 5 with *Advanced Farming* on and 2 with it off.

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
delta = {Spring: 8, Summer: 24, Autumn: 12, Winter: -12}[g_season] - random/8;
for every county:            dryness += delta;
pick one county c:           dryness[c] += delta + localModifier(c);
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

### 7.4 Industry, weapons and wages

`Industry_Produce` (`0x0044EA92`) runs four times per county per season:

| commodity | job (`L2.eng` group 74) | divisor | base efficiency | credited to |
|---|---|---:|---:|---|
| 0 wood | 7 Wood cutting | 1 | **20 %** | realm `+0x130` |
| 1 iron | 5 Iron mining | 1 | 15 % | realm `+0x120` |
| 2 weapons | 8 Blacksmith | **4** | 15 % | realm `+0x140 + type*4` |
| 3 stone | 6 Stone quarrying | **2** | 15 % | realm `+0x128` |

`output = min(resourceLimit, Pct(workers / divisor, efficiency))`. **[V]** on the table;
**[V]** the 15 % base efficiency also appears verbatim in a published FAQ (*"30 serfs
working at 15 % efficiency"*), and *"iron and wood harvest at twice the quantity of stone"*
is exactly the divisor column.

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
five-stage bankruptcy escalation.

### 7.5 Castles

Five designs, and five parallel tables indexed by castle type 1 … 5:

| | palisade | motte & bailey | Norman keep | stone castle | royal castle |
|---|---:|---:|---:|---:|---:|
| wood, stone (`0x004D89C0`) | 400 / 40 | 800 / 80 | 200 / 1000 | 400 / 2000 | 800 / 3000 |
| workforce (`0x004D89E8`) | 200 | 400 | 800 | 1500 | 2500 |
| garrison cap (`0x004D8A10`) | 150 | 200 | 200 | 400 | 600 |
| tax bonus % (`0x004D8A28`) | 50 | 75 | 100 | 125 | 150 |
| free archers (`0x004D8A40`) | 50 | 150 | 150 | 200 | 300 |

**[V]** on the tables; the tax-bonus row is independently confirmed by `Tax_CollectAll`'s
own constants (§4.1) and by the manual's *"A new castle will automatically include a
garrison. Its size will vary according to the size of the castle."*

**[V] The default starting castle is the Norman keep.** Every player-owned county in the
shipped `lastturn.sav` has `castleType = 3`, and `L2.eng` group 103 index 22 — the value
word for the "Starting Castle" option — is `keep`.

---

## 8. Events, the AI and scoring

### 8.1 Random events

`Event_RollAll` (`0x00448819`) draws from **`g_eventTable` (`0x004D6108`)** for each
**human-owned** county once the year passes 1268, and dispatches one of 24 handlers
(ids `0x87` … `0x8E` and `0x12E` … `0x13D`). Most write a percentage into `+0x1FB`,
`+0x1FC` or `+0x1FD`, which `Population_UpdateAll`, `Grain_SeasonTick` and
`Herd_SeasonTick` then apply to births/deaths, the grain store and the herd; the rest move
happiness or health directly. The population modifier is capped at 20 % of the county.

**[V]** on the mechanism and on the guard: **the AI never draws random events.** The
message text is in `L2.eng` — *"Vermin have been discovered in the county's tithe barns"*,
*"Wolves are abroad in the county"*, *"The Black Death is spreading across the county"*.

### 8.2 The AI's advantages

`AI_SetTaxRates` (`0x0049D638`) does two things: it sets the county tax rate from happiness
on one of four ladders (one for unowned counties, three chosen by the AI lord's
personality), and, for AI realms only, it hands out free resources scaled by difficulty:

* gold from **`g_aiGoldGrant` (`0x004DC1E0`)** — `int[5][4]` by lord and difficulty, from
  all zeros for lord 0 up to `250, 600, 1100, 1800` — with a second, smaller table at
  `0x004DC230` for a realm holding fewer than three counties;
* free population, herd and grain: `difficulty × 20` people (and the same again booked as
  births), `difficulty × 5` head and `difficulty × 40` sacks, per county, per season.

**[V]** on the code and the tables; the tables are byte-identical to a published dump of
the same addresses. The population/herd/grain grant is gated on the county already having
some (`pop > 20`, `herd > 10`, `grain > 50`), so it compounds rather than rescues.

The human realm's `lord` byte is 0 and row 0 of the gold table is all zeros, so **the human
gets nothing from either mechanism**. **[V]**

### 8.3 Score

`Score_RankRealms` (`0x0049AA0E`) rebuilds each realm's score every season:

```
score = realm[+0x60]*10 + realm[+0x10]/10 + realm[+0x0C]*2 + realm[+0x58]*2
      + realm[+0x54]/5  + realm[+0x4C]*50
      + (gold > 10000 ? 200 : gold >= 5001 ? 100 : gold >= 2001 ? 50 : 0)
```

then bubble-sorts realms 1 … 5 into the table at `0x00565410` and writes the rank back to
`realm +0x2B`. **[D]** — the weights are read straight off the decompiler, but **none of the
six contributing realm fields was identified**, so the offsets are left bare rather than
guessed at. The gold bracket is the one term whose meaning is unambiguous.

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
   `shownTax = +5`, `shownHealth = +1`, `shownRation = +1`. Player-owned counties store
   happiness **72 = 65 + 5 + 1 + 1**; unowned counties store **77**, with
   `shownEvents = +5` — the unowned bonus in §4.4. Fourteen counties, two cases.
6. **The tax term reproduces.** `taxRate` is 0 everywhere, and `dHapTax = 5 − 0 = 5`.
7. **The health chain reproduces.** `healthMeter` is 67 and `healthBand` is 3, which is the
   ladder's `≤ 90 → 3`. A published dump of the new-game presets gives a starting health of
   **65** for a medium county — which is band **2** on the same ladder, since `65 ≤ 65` —
   and `g_healthDeltaTable[Normal][band 2]` is **+2**, giving `65 + 2 = 67`. One number
   pins the ladder's comparison sense and the delta table's indexing at once. **[I]** on
   the starting value, which comes from the prior art rather than from this binary.
8. **Births and deaths reproduce exactly, on two different happiness bands.** Every county
   started at `popLast = 417`, so `g_birthRateLadder` gives 20 %:

   | | happiness | factor | births | deaths | population |
   |---|---:|---:|---:|---:|---:|
   | owned (4 counties) | 72 | 75 % | `Pct(417, Pct(20,75)) + 1` = **63** | `Pct(417, 3+8)` = **45** | `417+63−45` = **435** |
   | unowned (10 counties) | 77 | 100 % | `Pct(417, 20) + 1` = **84** | **45** | **456** |

   and the stored values are 63/45/435 and 84/45/456. The deaths figure needs
   `g_deathRateByHealth[3] = 3` **and** `g_deathRateBySeason[4] = 8`, so it also confirms
   the season index independently.

That is a five-stage chain — ration → health meter → health band → happiness → birth rate →
population — reproducing on live data from a real game, with no free parameters. It is the
strongest evidence in this document and the reason most of §4 and §5 is marked **[V]**.

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

Two exceptions worth naming for a reimplementation:

* the **castle tax multipliers** in `Tax_CollectAll` are immediates in the instruction
  stream, not a table — the parallel `g_castleTaxBonus` table is used only by the UI;
* the **grain sow ladder** (10 down to 1) and the **ration happiness formula** (`3L − 8`)
  are likewise arithmetic, not data.

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
| "the county array holds counties 1..14" | RE write-up | 17 records; 14 is the county count *of the England map*, which is what `g_countyCount` reads in the shipped save |
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

* **The fourteen AI turn handlers** (§3.2). This is where the strategic AI lives — what it
  builds, who it attacks, how it trades. None was decompiled. It is the single largest
  remaining piece of the kingdom layer.
* **Ale.** There is a happiness field (`+0x194`), a panel label, a price of 1, and a UI
  string *"Buy ale for your county, as a gift for its people"*. The purchase path and the
  happiness formula were not traced. Published claims of "+1 per 20 % of the population, cap
  +5" are unverified here.
* **The army happiness term** (`+0x15`, group 85 *"From army"*). Field and label found,
  writer not found.
* **Trade.** `plane4.md` closed the merchant *movement*; the transaction is still open. The
  price table is §10; how a merchant's offer is generated from it, and what the second
  15-entry table at `0x004D8950` is, are not established.
* **Fertility's effect.** `+0x208` runs −100 … +100 and `L2.eng` group 22 names seven
  levels, but where the crop yield reads it was not found. The yield multipliers in
  `Grain_Grow` and `Grain_Harvest` were not decompiled beyond their weather branches.
* **The food split** (§4.3) — the unowned-county case reproduces exactly and the owned-county
  case does not.
* **Efficiency ramp.** `Industry_Produce` reads a per-commodity efficiency at `+0x294`
  computed by `FUN_0044F248`; the base values (15 %, 20 %) are known but the ramp is not.
* **Health's other inputs.** `g_healthDeltaTable` is indexed by ration and band only.
  Whether anything else moves the meter — plague events do, at least — was not enumerated.
* **About 150 county fields.** 201 offsets inside the record are referenced by the binary;
  52 are named above.
* **No runtime confirmation of anything dynamic.** Everything here is static: the binary,
  the shipped data files, `L2.eng`, the manual, and one turn-1 save. No game was run, per
  `decisions.md` D8 and the rule about processes an agent starts.

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
