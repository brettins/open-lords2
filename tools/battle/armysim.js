#!/usr/bin/env node
// armysim.js - reproduce Lords2.exe's battlefield sizing and figure allocation.
//
// Everything here is read out of Lords2.exe (see docs/battle.md for addresses):
//   FUN_0047efee  Battle_InitArmies   - battlefield size class
//   FUN_0047fea7  Battle_RaiseSide    - split an army into units
//   FUN_00480662  BattleUnit_Create   - split a unit into figures
//
// It exists to test one hard invariant: g_battleFigures has exactly 80 usable
// slots (index 1..80), and BattleMan_Create returns 0 - i.e. the army is
// silently truncated - the moment allocation runs past it.
//
//   node tools/battle/armysim.js troops "F:/games/Lords of the Realm II"
//   node tools/battle/armysim.js skr    "F:/games/Lords of the Realm II/USER.SKR"
//   node tools/battle/armysim.js one 200 25 100 75 25 150 25 -- 100 50 50 50 100 100 0

const fs = require('fs');

// ---- tables lifted from the binary -----------------------------------------
const MEN_PER_FIGURE = [4, 8, 16, 32, 64, 128, 256, 512, 1024];      // 0x004D9658
const SIZE_LADDER = [[0x131, 0], [0x261, 1], [0x4c1, 2], [0x981, 3], // 0x004D95D8
                     [0x1301, 4], [0x2601, 5], [0x4c01, 6], [0x9801, 7]];
const SIEGE_LADDER = [[5, 4], [9, 8], [0x11, 16], [0x21, 32],        // 0x004D9618
                      [0x41, 64], [0x81, 128], [0x101, 256], [0x201, 512]];
// 0x004D96D0, stride 0x14: maxFigures, footprint, maxPerRow, weaponClass, moveDelay
const TROOP = [
  { name: 'Peasants',    maxFigures: 12, foot: 1, perRow: 6, wclass: 0, moveDelay: 2 },
  { name: 'Crossbowmen', maxFigures:  8, foot: 1, perRow: 6, wclass: 2, moveDelay: 2 },
  { name: 'Macemen',     maxFigures:  8, foot: 1, perRow: 4, wclass: 0, moveDelay: 1 },
  { name: 'Swordsmen',   maxFigures:  8, foot: 1, perRow: 4, wclass: 0, moveDelay: 3 },
  { name: 'Pikemen',     maxFigures: 10, foot: 1, perRow: 5, wclass: 0, moveDelay: 4 },
  { name: 'Archers',     maxFigures: 12, foot: 1, perRow: 6, wclass: 1, moveDelay: 2 },
  { name: 'Knights',     maxFigures:  6, foot: 1, perRow: 6, wclass: 0, moveDelay: 0 },
  { name: 'Catapults',   maxFigures:  2, foot: 3, perRow: 2, wclass: 3, moveDelay: 5 },
  { name: 'SiegeTowers', maxFigures:  2, foot: 3, perRow: 2, wclass: 0, moveDelay: 5 },
  { name: 'Rams',        maxFigures:  2, foot: 3, perRow: 2, wclass: 0, moveDelay: 5 },
  { name: 'Oil',         maxFigures:  1, foot: 2, perRow: 1, wclass: 0, moveDelay: 5 },
];
const RAISE_ORDER = [9, 10, 6, 3, 2, 4, 1, 5, 0, 8, 7];              // 0x004D9870 (field)
const RAISE_ORDER_SIEGE = [10, 5, 1, 6, 0, 3, 2, 4];                 // 0x004D98A0 (defender)
const FIGURE_SLOTS = 80;                                             // g_battleFigures 1..80
// FUN_0042AC0C overwrites difficulty groups 0,1,3,4 with Normal scaled, types 0..6 only.
const DIFFICULTY_PCT = [116, 108, 100, 92, 84];

function ladder(x, table, dflt) {          // FUN_00404E4B
  for (const pair of table) if (x < pair[0]) return pair[1];
  return dflt;
}

function sizeClass(armyA, armyB) {
  const totalA = armyA.slice(0, 7).reduce((a, b) => a + b, 0);
  const totalB = armyB.slice(0, 7).reduce((a, b) => a + b, 0);
  const siege = armyA.slice(7, 11).concat(armyB.slice(7, 11)).reduce((a, b) => a + b, 0);
  const scale = ladder(Math.floor((totalA + totalB) / 0x4c), SIEGE_LADDER, 0x400);
  return { cls: ladder(totalA + totalB + scale * siege, SIZE_LADDER, 8), totalA, totalB, siege };
}

// FUN_0047FEA7 (field) / FUN_004801F8 (siege defender) + FUN_00480662, for one side.
function raiseSide(counts, total, cls, siege) {
  let mpf = MEN_PER_FIGURE[cls];
  if (Math.floor(total / mpf) < 9 && mpf > 7) {        // small army: finer figures
    mpf = mpf < 0x10 ? 4 : mpf < 0x20 ? 8 : mpf < 0x40 ? 16 : mpf < 0x80 ? 32 : 64;
  }
  const units = [];
  for (const t of (siege ? RAISE_ORDER_SIEGE : RAISE_ORDER)) {
    let men = counts[t] || 0;
    if (t > 6) men *= mpf;                             // siege engines count as whole figures
    if (men === 0) continue;
    // FUN_004801F8 forces max 3 figures per unit for archers and crossbowmen
    const maxFig = (siege && (t === 5 || t === 1)) ? 3 : TROOP[t].maxFigures;
    const cap = maxFig * mpf;
    while (men > 0) {
      const inUnit = Math.min(men, cap);
      units.push({ type: t, men: inUnit, figures: Math.ceil(inUnit / mpf) });
      men -= inUnit;
    }
  }
  return { mpf, units, figures: units.reduce((a, u) => a + u.figures, 0) };
}

function battle(armyA, armyB, label) {
  const s = sizeClass(armyA, armyB);
  const a = raiseSide(armyA, s.totalA, s.cls);
  const b = raiseSide(armyB, s.totalB, s.cls);
  const figures = a.figures + b.figures;
  return {
    label, cls: s.cls, mpf: a.mpf, mpfB: b.mpf, totalA: s.totalA, totalB: s.totalB,
    unitsA: a.units.length, unitsB: b.units.length,
    figA: a.figures, figB: b.figures, figures, over: figures > FIGURE_SLOTS,
  };
}

// ---- inputs ----------------------------------------------------------------
function readTroops(path) {                            // FUN_0042AC0C tokeniser
  const text = fs.readFileSync(path, 'latin1');
  const body = text.slice(text.indexOf('*'));
  const nums = (body.match(/\d+/g) || []).map(Number);
  if (nums.length !== 3885) throw new Error(path + ': ' + nums.length + ' numbers, expected 3885');
  const rows = [];
  let p = 0;
  for (let r = 0; r < 35; r++) {
    const advantage = nums[p++];
    const groups = [];
    for (let g = 0; g < 5; g++) {
      const sides = [];
      for (let s = 0; s < 2; s++) { sides.push(nums.slice(p, p + 11)); p += 11; }
      groups.push(sides);
    }
    rows.push({ advantage, groups });
  }
  return rows;
}

function readSkr(path) {
  const buf = fs.readFileSync(path);
  const maps = [];
  for (let m = 0; m < 20; m++) {
    const rec = (i) => Array.from({ length: 11 },
      (_, k) => buf.readUInt32LE(m * 0x58 + i * 0x2c + k * 4));
    maps.push([rec(0), rec(1)]);
  }
  return maps;
}

// ---- entry points ----------------------------------------------------------
const argv = process.argv;
const cmd = argv[2];
const rest = argv.slice(3);
const pad = (s, n) => String(s).padStart(n);

// The in-memory table after FUN_0042AC0C: only "Normal" (group 2) comes from the
// file; the other four are Normal * DIFFICULTY_PCT on troop types 0..6 only.
function derive(row) {
  const normal = row.groups[2];
  return DIFFICULTY_PCT.map((pct) => normal.map((side) =>
    side.map((v, t) => (t < 7 ? Math.floor(v * pct / 100) : v))));
}

if (cmd === 'troops') {
  const dir = rest[0];
  let worst = null, over = 0, n = 0;
  const overs = [];
  for (const f of ['TROOPS.ENG', 'TROOPS2.ENG', 'TROOPS3.ENG']) {
    const rows = readTroops(dir + '/' + f);
    rows.forEach((row, r) => derive(row).forEach((g, gi) => {
      const res = battle(g[0], g[1], f + ' row ' + r + ' difficulty ' + gi);
      n++;
      if (res.over) { over++; overs.push(res); }
      if (!worst || res.figures > worst.figures) worst = res;
    }));
  }
  console.log('checked ' + n + ' (battle, difficulty) pairs, difficulty groups derived as the game does');
  console.log('  ' + (over ? 'note' : 'OK  ') +
    ' field-battle allocations exceeding the ' + FIGURE_SLOTS + '-figure array: ' + over);
  for (const o of overs) {
    console.log('       ' + o.label + ' -> class ' + o.cls + ', ' + o.mpf + ' men/figure, ' +
      o.figA + '+' + o.figB + ' = ' + o.figures);
  }
  console.log('  worst: ' + worst.label + ' -> size class ' + worst.cls + ', ' + worst.mpf +
    ' men/figure, ' + worst.figA + '+' + worst.figB + ' = ' + worst.figures + ' figures');
} else if (cmd === 'skr') {
  const maps = readSkr(rest[0]);
  let over = 0;
  maps.forEach((pairArmies, m) => {
    const r = battle(pairArmies[0], pairArmies[1], 'map ' + m);
    if (r.over) over++;
    console.log('map ' + pad(m, 2) + '  ' + pad(r.totalA, 5) + ' vs ' + pad(r.totalB, 5) +
      ' men  class ' + r.cls + '  ' + pad(r.mpf, 4) + ' men/figure  units ' +
      pad(r.unitsA, 2) + '+' + pad(r.unitsB, 2) + '  figures ' + pad(r.figA, 2) + '+' +
      pad(r.figB, 2) + ' = ' + pad(r.figures, 3) + (r.over ? '  <-- OVER 80' : ''));
  });
  console.log('  ' + (over ? 'FAIL' : 'OK  ') +
    ' allocations exceeding the ' + FIGURE_SLOTS + '-figure array: ' + over);
} else if (cmd === 'one') {
  const sep = rest.indexOf('--');
  const A = rest.slice(0, sep).map(Number), B = rest.slice(sep + 1).map(Number);
  while (A.length < 11) A.push(0);
  while (B.length < 11) B.push(0);
  const s = sizeClass(A, B);
  const a = raiseSide(A, s.totalA, s.cls), b = raiseSide(B, s.totalB, s.cls);
  console.log('size class ' + s.cls + ', ' + a.mpf + ' men per figure');
  for (const entry of [['A', a], ['B', b]]) {
    console.log(' side ' + entry[0] + ': ' + entry[1].units.length + ' units, ' +
      entry[1].figures + ' figures');
    for (const u of entry[1].units) {
      console.log('   ' + TROOP[u.type].name.padEnd(12) + pad(u.men, 5) + ' men  ' +
        pad(u.figures, 2) + ' figures');
    }
  }
  console.log(' total figures ' + (a.figures + b.figures) + ' of ' + FIGURE_SLOTS);
} else {
  console.log('usage: node armysim.js troops <gamedir> | skr <USER.SKR> | one <7 counts> -- <7 counts>');
}
