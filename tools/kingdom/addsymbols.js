// Merge the kingdom-layer names into docs/symbols.json.  Idempotent: an entry
// whose address is already present is replaced, not duplicated.
//
//   node tools/kingdom/addsymbols.js
//   node tools/symbols/symbols_md.js
const fs = require('fs');
const path = require('path');
const p = path.resolve(__dirname, '..', '..', 'docs', 'symbols.json');
const data = JSON.parse(fs.readFileSync(p, 'utf8'));

if (!data.sections.some(s => s.id === 'kingdom'))
  data.sections.push({ id: 'kingdom', title: 'Kingdom layer (counties, seasons, economy)' });

const F = (addr, name, confidence, comment, signature) =>
  ({ addr, name, section: 'kingdom', confidence, ...(signature ? { signature } : {}), comment });
const G = (addr, name, confidence, comment) =>
  ({ addr, name, section: 'kingdom', confidence, comment });

const functions = [
  // --- turn machine ---
  F('0x0049A010', 'Turn_Tick', 'verified',
    'Per-frame dispatcher on g_turnPhase (1..7). Phase 1 neutral-county upkeep, 2 army movement, 3 supply transports, 4 the players\' turn, 5 peasant mobs, 6 merchants, 7 end of season. Increments g_turnPhaseStep every call.'),
  F('0x0049CE51', 'Turn_AdvancePhase', 'verified',
    'g_turnPhase++, wrapping 7 -> 1 (and then calling FUN_0049B6D3); resets g_turnPhaseStep and sets the next-phase preview at 0x0053F678.'),
  F('0x0049B762', 'Turn_AllRealmsDone', 'verified',
    'Returns non-zero once every in-play realm\'s AI step counter (realm +0x00) has reached 999. The exit condition of phase 4.'),
  F('0x0049A581', 'AI_RunTurnStep', 'verified',
    'One step of one realm\'s turn, round-robin over realms 1..5 through 0x0057C944. Realm +0x00 is a 1..14 program counter into fourteen sub-handlers; at 15 + 2*realm the realm is marked done (999). Skipped entirely when realm +0x05 (human) is set.'),
  F('0x00448440', 'Season_Advance', 'verified',
    'The end-of-season pipeline. Rolls g_seasonPrev/g_season/g_seasonNext and the year (when the season that ended was 4 = Winter), bumps g_turnCount, then calls 28 subsystem passes in a fixed order. This is the whole kingdom economy for one turn.'),
  F('0x00497CED', 'Game_NewGame', 'verified',
    'New-game setup: year 1267/1268, g_season = 3, g_seasonNext = 4, then Map_InitScenario, Merchant_SpawnAll and one immediate Season_Advance - which is why a new game begins in Winter 1268.'),
  F('0x004983B7', 'Rules_InitConstants', 'inferred',
    'Writes the tunable economy scalars into .data at startup: g_grainYieldPerSack = 12, g_grainMaxSacksPerField = 10, g_foodPerHead = 10, g_foodPerSack = 6, g_dairyPerHead = 5, g_grainLabourDivisorAdv = 5, g_grainLabourDivisor = 2.'),

  // --- taxation and happiness ---
  F('0x0044B59B', 'Tax_CollectAll', 'verified',
    'Per county: rate = 320 + castle bonus (480/560/640/720/800 for castle types 1..5, i.e. +50/75/100/125/150%); take = Pct(Pct(population, rate), taxRate); credited to the owner realm\'s gold. Also writes the tax happiness delta 5 - taxRate + realm[+0x28] into county +0x0E.',
    'void Tax_CollectAll(void)'),
  F('0x0044B99A', 'Tax_SumEmpireHappiness', 'verified',
    'realm[+0x28] = sum over the realm\'s counties of county +0x16 - the "Other counties" term of the tax happiness effect (L2.eng group 86 index 4). Sums signed bytes into a signed byte, so a large empire can overflow.',
    'void Tax_SumEmpireHappiness(uint realm)'),
  F('0x0044BAEA', 'Happiness_UpdateAll', 'verified',
    'happiness(+0x0C) = last(+0x0D) + tax(+0x0E) + health(+0x10) + ration(+0x11), copied to the display fields +0x12..+0x14, clamped 0..100, accumulated into +0x1C and averaged into +0x18. Unowned counties below 75 get a flat +5 recorded as "From events".',
    'void Happiness_UpdateAll(void)'),
  F('0x0044BD9F', 'Health_UpdateAll', 'verified',
    'health meter (+0x0B) += g_healthDeltaTable[rationLevel][healthBand], clamped 0..100; band (+0x09) = Table_Lookup(meter, g_healthBandLadder); happiness contribution (+0x10) = g_healthHappiness[band].',
    'void Health_UpdateAll(void)'),

  // --- population ---
  F('0x00449EF3', 'Population_UpdateAll', 'verified',
    'births = Pct(pop, Pct(g_birthRateLadder(pop), happinessFactor)); deaths = Pct(pop, g_deathRateByHealth[band] + g_deathRateBySeason[season]); pop += births - deaths - emigrants + immigrants. Reproduces the shipped lastturn.sav exactly.',
    'void Population_UpdateAll(void)'),
  F('0x0044A6BA', 'Migration_UpdateAll', 'verified',
    'Emigration to the happiest adjacent county: pct = Pct(dHappiness, (100 - happiness)/3), movers = min(100, Pct(pop, pct)), halved for unowned counties. Fills county +0x3C/+0x40/+0x58 and the 16-byte source list at +0x48.',
    'void Migration_UpdateAll(void)'),
  F('0x0044AA41', 'Unrest_UpdateAll', 'verified',
    'Counts county +0x20 up while happiness is low and down while it recovers; at 4 it calls FUN_004AC185, which raises the revolting-peasant army. Human-owned counties get warning messages 0x96..0x99 as the counter climbs.',
    'void Unrest_UpdateAll(void)'),

  // --- weather, land, crops, herd ---
  F('0x00449889', 'Weather_UpdateAll', 'verified',
    'Dryness (+0x21D) += seasonal delta (Spring +8, Summer +24, Autumn +12, Winter -12) minus rand/8, doubled for one random county and half again for its neighbours; banded into +0x21B: <5 Flooding, <20 Storms, <70 Cloudy, <95 Sunny, else Drought, with Drought becoming Frost in Winter and Spring. Forced to Cloudy when Advanced Farming is off.',
    'void Weather_UpdateAll(void)'),
  F('0x0044BFD5', 'Fertility_Update', 'verified',
    'county +0x208 += 6 * fallowFields(+0x1FF) - 3 * grainFields(+0x201), clamped to -100..100, forced to 0 when Advanced Farming is off. One fallow field per two grain fields is exactly break-even; cattle fields do not enter it.',
    'void Fertility_Update(int county)'),
  F('0x0044C093', 'Field_ReclaimTick', 'verified',
    'Advances the 20 per-county field-progress words at +0x90 towards 800, at most 200 per season - the manual\'s "never more than a quarter of a field in a single season". Rewrites the tile graphic at each quarter.',
    'void Field_ReclaimTick(int county)'),
  F('0x0044C278', 'Field_ReclaimEstimate', 'inferred',
    'The same walk without committing: fills +0xE4 (work remaining) and +0x214 (seasons to the next completed field) for the county panel.',
    'int Field_ReclaimEstimate(int county)'),
  F('0x0044C8AE', 'Grain_SeasonTick', 'verified',
    'The grain cycle. Entering Spring it sows (Grain_Sow, store -= seed); entering Summer and Autumn it grows; entering Winter it harvests into the store. Weather scales each stage. Also applies the rats/surplus random-event modifier at +0x1FC.',
    'void Grain_SeasonTick(void)'),
  F('0x0044CFE1', 'Grain_Sow', 'verified',
    'Largest sacksPerField in 10..1 such that grainFields*sacks fits both the grain store and the available labour (g_grainYieldPerSack * sown / labourDivisor). Sets county +0x1A7 when it had to fall back to a single field.',
    'int Grain_Sow(int county, int labour, int grainStore)'),
  F('0x0044D15A', 'Grain_Grow', 'inferred', 'Mid-season crop step, entering Summer and Autumn.',
    'int Grain_Grow(int county, int labour, int crop)'),
  F('0x0044D1E5', 'Grain_Harvest', 'inferred', 'Harvest step, entering Winter.',
    'int Grain_Harvest(int county, int labour, int crop)'),
  F('0x0044D60D', 'Herd_SeasonTick', 'verified',
    'Herd (+0x250) growth: births/deaths from FUN_0044DA99, plus Pct(herd, g_herdWeatherPct[weather]) and the disease/wolves random-event modifier at +0x1FD.',
    'void Herd_SeasonTick(void)'),

  // --- food ---
  F('0x0044DF5F', 'Ration_Apply', 'verified',
    'Picks the highest affordable ration level 0..5 (None/Quarter/Half/Normal/Double/Triple) by descending from the wanted level, requirement = DivCeil(pop, g_rationTable[l].div) * g_rationTable[l].mul plus garrisons when Armies Eat is on; spends dairy first, then splits the rest between livestock and grain by county +0x15F; writes the ration happiness delta 3*level - 8 to +0x11.',
    'void Ration_Apply(int county, int season)'),
  F('0x0044E5BB', 'Food_FromDairy', 'verified',
    'herd * g_dairyPerHead (5). The standing herd feeds five people per head per season without being slaughtered.',
    'int Food_FromDairy(int county)'),
  F('0x0044E603', 'Food_HeadsForPeople', 'verified',
    'DivCeil(people, g_foodPerHead) - one slaughtered animal feeds ten people.',
    'int Food_HeadsForPeople(int county, int people)'),
  F('0x0044E653', 'Food_SacksForPeople', 'verified',
    'DivCeil(people, g_foodPerSack) - one sack of grain feeds six people.',
    'int Food_SacksForPeople(int county, int people)'),
  F('0x0044E7B4', 'Food_Available', 'verified',
    'herd*5 + slaughterable*10 + grain*6, the county\'s total feeding capacity this season.',
    'int Food_Available(int county)'),

  // --- industry, castles, wages ---
  F('0x0044EA92', 'Industry_Produce', 'verified',
    'One industry pass. output = min(resourceLimit, Pct(workers / divisor, efficiency)); commodity 0 wood (job 7, divisor 1, base efficiency 20), 1 iron (job 5, divisor 1, 15), 2 weapons (job 8, divisor 4, 15), 3 stone (job 6, divisor 2, 15). Weapons debit g_weaponCost from the realm\'s wood and iron.',
    'void Industry_Produce(int county, int commodity, int job, int baseEfficiency, int divisor)'),
  F('0x004508DE', 'Castle_BuildTick', 'inferred',
    'Advances castle construction in every county, promoting the castle type and raising its free garrison when the work reaches 100%.'),
  F('0x00450E46', 'Castle_BuildEstimate', 'inferred',
    'Fills the castle panel: work left (+0xF0) and seasons remaining (+0x1A6) from the outstanding work and the labour assigned.',
    'void Castle_BuildEstimate(int county)'),
  F('0x004ACBD4', 'Wages_PayAll', 'verified',
    'realm[+0xFC] = Wages_ForRealm; if the treasury cannot cover it the realm goes through a five-stage bankruptcy escalation (messages 0xA0/0x10E, 0x11F, 0x10F, then FUN_004AD316) counted in realm +0x158.'),
  F('0x004AD495', 'Wages_ForRealm', 'verified',
    'Sum of Wages_ForUnit over the realm\'s armies (unit type 1).',
    'int Wages_ForRealm(int realm)'),
  F('0x004AD52B', 'Wages_ForUnit', 'verified',
    'men(+0x168) / 4 for a human owner; for an AI owner /3, /5, /10, /10 by difficulty. Matches the player-measured 250 men -> 62 crowns and 254 -> 63.',
    'int Wages_ForUnit(int unit)'),

  // --- events, AI, score ---
  F('0x00448819', 'Event_RollAll', 'verified',
    'Draws a random-event id from g_eventTable for each human-owned county after year 1268 and dispatches one of 24 handlers (0x87..0x8E, 0x12E..0x13D) that write the modifiers at county +0x1FB..+0x1FD or move happiness/health directly.'),
  F('0x0049D638', 'AI_SetTaxRates', 'verified',
    'Sets county +0xB9 from happiness on one of four ladders (neutral counties, then three by AI personality), and grants the AI its per-turn difficulty bonus: gold from g_aiGoldGrant, plus free population, herd and grain scaled by difficulty.',
    'void AI_SetTaxRates(uint realm)'),
  F('0x0049DFC6', 'AI_ManageFields', 'inferred',
    'For each of the realm\'s counties: start reclaiming when the field count is low for the population, then run one of two field-allocation strategies and Ration_Apply.',
    'void AI_ManageFields(uint realm)'),
  F('0x0049AA0E', 'Score_RankRealms', 'verified',
    'Recomputes each realm\'s score at +0x50 from counties, population, castles, armies and treasury, bubble-sorts realms 1..5 into the ranking table at 0x00565410 and writes the rank back to realm +0x2B.'),

  // --- persistence and multiplayer sync ---
  F('0x004ADE93', 'Save_Write', 'verified',
    'Writes every block listed in g_saveBlocks back to back, then appends the sixteen 12,800-byte castle plans from castles.dat. 267,028 + 16*12,800 = 471,828, the exact size of lastturn.sav.',
    'undefined4 Save_Write(char * path)'),
  F('0x0049A453', 'Save_RotateAndWrite', 'verified',
    'Rotates safeturn.sav <- old_turn.sav <- lastturn.sav and then calls Save_Write. Called from Game_NewGame and at each turn boundary.'),
  F('0x004AE8A9', 'Castles_CreateFile', 'verified',
    'Creates castles.dat as sixteen zeroed 0x3200-byte blocks, one castle plan per county slot.'),
  F('0x0043FAA4', 'Sync_CompareState', 'verified',
    'Multiplayer desync detector: walks three of the g_syncBlocks descriptors and reports the first record and byte that differ between the two state snapshots.'),
  F('0x0043FD2A', 'Sync_Checksum', 'verified',
    'Sums the same descriptor-selected bytes into a per-frame checksum for the network layer.'),

  // --- UI panels used as evidence ---
  F('0x004116FB', 'Panel_Happiness', 'verified',
    'Draws L2.eng group 85 ("Happiness in / Last season / From taxes / From ration / From health / From army / From ale / This Season / Average happiness / From events") next to county fields +0x0D, +0x12, +0x13, +0x14, +0x15, +0x194, +0x17, +0x0C and +0x18. This is what names those fields.'),
  F('0x004110B1', 'Panel_Population', 'verified',
    'Draws L2.eng group 73 next to county +0x28 (last season), +0x30 (births), +0x34 (deaths), +0x38 (army), +0x3C/+0x58 (emigrants to), +0x40 (immigrants) and +0x24 (this season).'),
  F('0x0041152F', 'Panel_Tax', 'verified',
    'Draws L2.eng group 86: "Tax rate" = county +0xB9, "People pay" = +0xC0, "This county" = realm +0x28 + county +0x0F, "Other counties" = county +0x16.'),

  // --- arithmetic helpers ---
  F('0x00404D6B', 'Pct', 'verified', 'x * p / 100. The whole economy is written in this.',
    'int Pct(int x, int p)'),
  F('0x00404DC1', 'PctOf', 'verified', 'a * 100 / b, 0 when b is 0.', 'int PctOf(int a, int b)'),
  F('0x00404E03', 'DivCeil', 'verified', '(a + b - 1) / b, 0 when b is 0.', 'int DivCeil(int a, int b)'),
];

const globals = [
  G('0x0053F9B0', 'g_counties', 'verified',
    'The county array: 17 records of 0x300 bytes, index 1..16 (0 unused). Base, count and stride all appear together in the multiplayer sync descriptor at 0x004D5B10 and in the save-block table at 0x004DE960.'),
  G('0x0057BF00', 'g_realms', 'verified',
    'Realm/player array: 6 records of 0x160 bytes, index 1..5. +0x00 AI step, +0x04 in play, +0x05 human, +0x07 lord, +0x28 empire tax happiness, +0x2B rank, +0x50 score, +0xFC army wages, +0x118 gold.'),
  G('0x0053EA00', 'g_countyFieldTiles', 'verified',
    '17 x 20 u32 tile indices - the map tiles that are this county\'s farm fields. Save block 12 is exactly 1360 bytes = 17 * 80.'),
  G('0x004DE960', 'g_saveBlocks', 'verified',
    '225 {u32 address, u32 length} records terminated by a zero length; Save_Write dumps each in order. Sums to 267,028 bytes.'),
  G('0x004D5B10', 'g_syncBlocks', 'verified',
    'Seven 12-byte {count, stride, firstComparedOffset} descriptors used by Sync_CompareState and Sync_Checksum: counties (17, 0x300, 5), realms (6, 0x160, 6), units (151, 0x1A4, 0), battle men (81, 0x1B0, 18), missiles (101, 0x4C, 4), battle units (81, 0x34, 0).'),
  G('0x004D5B70', 'g_stateArrayPtrs', 'verified',
    'Eight pointers in the same order as g_syncBlocks: counties, counties, realms, units, battle men, missiles, battle units, battlefield. No reader was found; it corroborates every base address.'),

  // clock
  G('0x0057C934', 'g_season', 'verified', 'Current season 1..4, used directly as the index into L2.eng group 29 (Spring, Summer, Autumn, Winter). Season_Advance sets it before running the economy, so the economy sees the season being entered.'),
  G('0x0057C92C', 'g_seasonNext', 'verified', 'The season after g_season; the year rolls when the season that just ended was 4.'),
  G('0x00554470', 'g_seasonPrev', 'verified', 'The previous value of g_season.'),
  G('0x00553EDC', 'g_year', 'verified', 'Displayed year. 1268 on turn 1.'),
  G('0x00553E74', 'g_yearNext', 'verified', 'g_year + 1, staged for the next roll.'),
  G('0x00553240', 'g_turnCount', 'verified', 'Turns elapsed; divides the happiness accumulator to give the running average.'),
  G('0x0057C93C', 'g_selectedCounty', 'verified', 'The county the county panels display.'),
  G('0x00554020', 'g_weatherCounty', 'verified', 'The county picked this season for the doubled local weather swing.'),

  // options
  G('0x0053F23C', 'g_optDifficulty', 'verified', 'New-game difficulty 0..3 (easy/normal/hard/impossible, L2.eng group 103 indices 11..14). Scales AI gold, AI free resources and AI army wages.'),
  G('0x0053F25C', 'g_optAdvancedFarming', 'verified', 'Advanced Farming (L2.eng group 102 index 0). When 0, weather is forced to Cloudy in every county and fertility is forced to 0.'),
  G('0x0053F260', 'g_optArmiesEat', 'verified', 'Armies Eat (L2.eng group 102 index 3). When 1, garrison and enemy troop counts are added to the county\'s food requirement.'),
  G('0x0053F26C', 'g_optTimeLimit', 'verified', 'Turn time limit in seconds, 0 for none.'),

  // economy scalars written by Rules_InitConstants
  G('0x0057C8E0', 'g_grainYieldPerSack', 'verified', '12. "Each sack planted will grow into 12 sacks" - the game\'s own FAQ text, L2.eng group 292 index 4.'),
  G('0x00552FFC', 'g_grainMaxSacksPerField', 'verified', '10. The top of Grain_Sow\'s descending search; the manual\'s "up to 5 sacks" is wrong.'),
  G('0x005533BC', 'g_grainLabourDivisorAdv', 'verified', '5. Labour divisor used when Advanced Farming is on.'),
  G('0x0057D34C', 'g_grainLabourDivisor', 'verified', '2. Labour divisor used when Advanced Farming is off.'),
  G('0x00553F60', 'g_dairyPerHead', 'verified', '5. People fed per head of the standing herd, without slaughter.'),
  G('0x00567594', 'g_foodPerHead', 'verified', '10. People fed by one slaughtered animal.'),
  G('0x0057CB30', 'g_foodPerSack', 'verified', '6. People fed by one sack of grain.'),

  // rule tables
  G('0x004D6308', 'g_birthRateLadder', 'verified',
    '20 {population, percent} pairs from (40, 100) down to (3000, 1): the base birth rate falls as a county fills up.'),
  G('0x004D63A8', 'g_deathRateByHealth', 'verified', 'Percent per season by health band 0..4: 35, 20, 8, 3, 0.'),
  G('0x004D63C0', 'g_deathRateBySeason', 'verified', 'Percent per season, indexed 1..4: Spring 4, Summer 0, Autumn 2, Winter 8.'),
  G('0x004D64A8', 'g_healthDeltaTable', 'verified',
    'int[6][5]: change to the health meter by ration level 0..5 and health band 0..4. Row 3 (Normal) is the first that is positive.'),
  G('0x004D6520', 'g_healthBandLadder', 'verified', 'Five {threshold, band} pairs: 10/0, 35/1, 65/2, 90/3, 100/4.'),
  G('0x004D6548', 'g_healthHappiness', 'verified',
    'Happiness per season by health band: -10, -5, 0, +1, +2 for Diseased, Sick, Average, Good, Perfect (L2.eng group 20).'),
  G('0x004D6560', 'g_herdWeatherPct', 'verified',
    'Percent change to the herd by weather 0..5: Frost -2, Drought -10, Sunny +5, Cloudy 0, Storms -5, Flooding -10.'),
  G('0x004D6738', 'g_rationTable', 'verified',
    'Six {divisor, multiplier} pairs: (1,0) None, (4,1) Quarter, (2,1) Half, (1,1) Normal, (1,2) Double, (1,3) Triple - exactly L2.eng group 21.'),
  G('0x004D6108', 'g_eventTable', 'verified', 'The random-event id ring Event_RollAll draws from; 0 means no event.'),
  G('0x004D8910', 'g_goodsPrice', 'verified',
    'Merchant base price per good id, 15 entries in L2.eng group 6 order: -, grain 2, cattle 12, sheep 0, ale 1, wool 0, iron 1, stone 2, timber 1, pikes 13, bows 16, maces 10, crossbows 24, swords 23, mail 44.'),
  G('0x004D8950', 'g_goodsStock', 'inferred',
    'A second 15-entry table on the same good ids: 1000 grain, 100 cattle, 200 sheep, 100 ale, 500 wool, 100 iron, 100 stone, 200 timber, 500 each weapon. Reads like the quantity a merchant carries.'),
  G('0x004D8990', 'g_weaponCost', 'verified',
    'Six {wood, iron} pairs: crossbow 6/10, mace 4/4, sword 3/10, pike 6/3, bow 13/0, armour 4/18. Debited by Industry_Produce.'),
  G('0x004D89C0', 'g_castleMaterial', 'verified',
    'Five {wood, stone} pairs: palisade 400/40, motte and bailey 800/80, Norman keep 200/1000, stone castle 400/2000, royal castle 800/3000.'),
  G('0x004D89E8', 'g_castleWorkforce', 'verified', 'Man-seasons per castle type, each value stored twice: 200, 400, 800, 1500, 2500.'),
  G('0x004D8A10', 'g_castleGarrisonCap', 'verified', 'Troops a castle can hold: 150, 200, 200, 400, 600.'),
  G('0x004D8A28', 'g_castleTaxBonus', 'verified',
    'Percent tax bonus by castle type: 50, 75, 100, 125, 150 - the same ratios as Tax_CollectAll\'s 480/560/640/720/800 over the castle-less 320.'),
  G('0x004D8A40', 'g_castleFreeArchers', 'verified', 'Archers a newly finished castle is given: 50, 150, 150, 200, 300.'),
  G('0x004D8A5C', 'g_aiPersonality', 'inferred', 'Per-AI-lord behaviour parameters, three 0x50-byte rows per lord; the first int of each row selects the tax ladder in AI_SetTaxRates.'),
  G('0x004DC1E0', 'g_aiGoldGrant', 'verified',
    'int[5][4] free gold per turn by AI lord and difficulty for a realm holding three or more counties: 0/0/0/0, 0/400/700/1200, 100/500/800/1400, 0/400/700/1200, 250/600/1100/1800.'),
  G('0x004DC230', 'g_aiGoldGrantSmall', 'verified', 'The same shape, used when the realm holds fewer than three counties.'),
];

const byAddr = new Map();
for (const kind of ['functions', 'globals'])
  data[kind].forEach((e, i) => byAddr.set(kind + e.addr, i));

let added = 0, replaced = 0;
for (const [kind, list] of [['functions', functions], ['globals', globals]]) {
  for (const e of list) {
    const k = kind + e.addr;
    if (byAddr.has(k)) { data[kind][byAddr.get(k)] = e; replaced++; }
    else { data[kind].push(e); added++; }
  }
}
fs.writeFileSync(p, JSON.stringify(data, null, 2) + '\n');
console.log('kingdom: ' + added + ' added, ' + replaced + ' replaced; now ' +
            data.functions.length + ' functions, ' + data.globals.length + ' globals');
