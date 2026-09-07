// Read a Lords of the Realm II save file using the save-block table the game
// itself carries at 0x004DE960 in Lords2.exe.
//
//   node tools/kingdom/savedump.js layout  "F:/games/Lords of the Realm II"
//   node tools/kingdom/savedump.js county  "F:/games/Lords of the Realm II" [lastturn.sav]
//   node tools/kingdom/savedump.js realm   "F:/games/Lords of the Realm II"
//   node tools/kingdom/savedump.js raw     "F:/games/Lords of the Realm II" <va> <len>
//
// The table is 8-byte {u32 address, u32 length} records terminated by length 0.
// Lords2.exe writes those blocks back to back and then appends castles.dat
// (16 x 12800). The total is the invariant that validates the whole reading.
const fs = require('fs');
const path = require('path');
const pe = require(path.join(__dirname, '..', 'maps', 'pe.js'));

const SAVE_TABLE_VA = 0x004DE960;   // FUN_004ADE93 (Save_Write) walks this
const MAX_ENTRIES   = 0xE1;         // its loop bound
const CASTLE_BLOCKS = 16, CASTLE_BLOCK = 0x3200;

function blockTable(dir) {
  const x = pe.load(path.join(dir, 'Lords2.exe'));
  const out = [];
  let cum = 0;
  for (let i = 0; i < MAX_ENTRIES; i++) {
    const addr = x.u32(SAVE_TABLE_VA + i * 8), len = x.u32(SAVE_TABLE_VA + i * 8 + 4);
    if (len === 0) break;
    out.push({ addr, len, off: cum });
    cum += len;
  }
  out.total = cum;
  return out;
}

// Map a runtime VA to an offset in the save file, if it falls inside a block.
function saveOffset(tab, va) {
  for (const b of tab) if (va >= b.addr && va < b.addr + b.len) return b.off + (va - b.addr);
  return -1;
}

function openSave(dir, name) {
  return fs.readFileSync(path.join(dir, name || 'lastturn.sav'));
}

const COUNTY_BASE = 0x0053F9B0, COUNTY_STRIDE = 0x300, COUNTY_N = 17;
const REALM_BASE  = 0x0057BF00, REALM_STRIDE  = 0x160, REALM_N  = 6;

// Field maps.  Only fields whose meaning is established in docs/kingdom.md.
const COUNTY_FIELDS = [
  [0x05, 'u8',  'owner'],
  [0x09, 'i8',  'healthBand'],
  [0x0b, 'i8',  'healthMeter'],
  [0x0c, 'i8',  'happiness'],
  [0x0d, 'i8',  'happinessLast'],
  [0x0e, 'i8',  'dHapTax'],
  [0x10, 'i8',  'dHapHealth'],
  [0x11, 'i8',  'dHapRation'],
  [0x12, 'i8',  'shownTax'],
  [0x13, 'i8',  'shownRation'],
  [0x14, 'i8',  'shownHealth'],
  [0x15, 'i8',  'shownArmy'],
  [0x17, 'i8',  'shownEvents'],
  [0x194,'i32', 'shownAle'],
  [0x18, 'i8',  'happinessAvg'],
  [0x1c, 'i32', 'happinessSum'],
  [0x20, 'u8',  'unrest'],
  [0x24, 'i32', 'population'],
  [0x28, 'i32', 'popLast'],
  [0x30, 'i32', 'births'],
  [0x34, 'i32', 'deaths'],
  [0x3c, 'i32', 'emigrants'],
  [0x40, 'i32', 'immigrants'],
  [0x5a, 'u8',  'neighbourCount'],
  [0x6c, 'u8',  'anchorX'],
  [0x6d, 'u8',  'anchorY'],
  [0xb8, 'u8',  'popBand'],
  [0xb9, 'u8',  'taxRate'],
  [0xbc, 'i32', 'taxCollected'],
  [0x15d,'i8',  'rationAchieved'],
  [0x15e,'i8',  'rationWanted'],
  [0x15f,'i8',  'rationSplitPct'],
  [0x178,'i32', 'grainEaten'],
  [0x17c,'i32', 'herdEaten'],
  [0x1c0,'u8',  'castleType'],
  [0x1c1,'u8',  'castleBuilding'],
  [0x1ff,'u8',  'fieldsFallow'],
  [0x200,'u8',  'fieldsCattle'],
  [0x201,'u8',  'fieldsGrain'],
  [0x208,'i32', 'fertility'],
  [0x21b,'u8',  'weather'],
  [0x21d,'i8',  'dryness'],
  [0x224,'i32', 'grain'],
  [0x250,'i32', 'herd'],
];

const REALM_FIELDS = [
  [0x00, 'i32', 'aiStep'],
  [0x04, 'u8',  'inPlay'],
  [0x05, 'u8',  'isHuman'],
  [0x07, 'u8',  'lord'],
  [0x0c, 'u8',  'f0C'],
  [0x10, 'i32', 'f10'],
  [0x28, 'i8',  'taxHapEmpire'],
  [0x2b, 'u8',  'rank'],
  [0x50, 'i32', 'score'],
  [0x118,'i32', 'gold'],
  [0xfc, 'i32', 'wages'],
];

const GLOBALS = [
  [0x0056d5dc, 'i32', 'g_countyCount'],
  [0x0053f034, 'i32', 'g_scenarioIndex'],
  [0x0057c8cc, 'i32', 'g_localPlayer'],
  [0x0057c934, 'i32', 'g_season'],
  [0x0057c92c, 'i32', 'g_seasonNext'],
  [0x00553edc, 'i32', 'g_year'],
  [0x00553240, 'i32', 'g_turnCount'],
  [0x00569584, 'i32', 'g_turnPhase'],
  [0x0053f658, 'i32', 'g_turnPhaseStep'],
  [0x0053f23c, 'i32', 'g_optDifficulty'],
  [0x0053f25c, 'i32', 'g_optAdvancedFarming'],
  [0x0053f260, 'i32', 'g_optArmiesEat'],
  [0x0053f264, 'i32', 'g_optExploration'],
  [0x0053f26c, 'i32', 'g_optTimeLimit'],
  [0x005530b4, 'i32', 'g_merchantCount'],
  [0x00554020, 'i32', 'g_weatherCounty'],
];

function get(buf, off, ty) {
  switch (ty) {
    case 'u8':  return buf.readUInt8(off);
    case 'i8':  return buf.readInt8(off);
    case 'i16': return buf.readInt16LE(off);
    case 'i32': return buf.readInt32LE(off);
  }
}

function dumpArray(sav, tab, base, stride, n, fields, label) {
  const at = saveOffset(tab, base);
  if (at < 0) { console.log(label + ': not in the save'); return; }
  const cols = fields.map(f => f[2]);
  const w = cols.map(c => Math.max(c.length, 6));
  console.log(label + ' @ 0x' + base.toString(16) + '  stride 0x' + stride.toString(16) +
              '  n=' + n + '  save offset ' + at);
  console.log(['idx'.padStart(3)].concat(cols.map((c, i) => c.padStart(w[i]))).join(' '));
  for (let i = 0; i < n; i++) {
    const rec = sav.slice(at + i * stride, at + (i + 1) * stride);
    const vals = fields.map((f, k) => String(get(rec, f[0], f[1])).padStart(w[k]));
    console.log([String(i).padStart(3)].concat(vals).join(' '));
  }
}

function main() {
  const [mode, dir, a, b] = process.argv.slice(2);
  if (!mode || !dir) { console.log('usage: savedump.js layout|county|realm|raw <gameDir> [...]'); return; }
  const tab = blockTable(dir);

  if (mode === 'layout') {
    console.log('save-block table at 0x' + SAVE_TABLE_VA.toString(16) + ' in Lords2.exe');
    for (let i = 0; i < tab.length; i++)
      console.log(String(i).padStart(3) + '  0x' + tab[i].addr.toString(16).padStart(8, '0') +
                  '  ' + String(tab[i].len).padStart(7) + '  @' + tab[i].off);
    const expect = tab.total + CASTLE_BLOCKS * CASTLE_BLOCK;
    console.log('\n' + tab.length + ' blocks, ' + tab.total + ' bytes');
    console.log('+ castles.dat ' + CASTLE_BLOCKS + ' x ' + CASTLE_BLOCK + ' = ' + expect);
    for (const f of ['lastturn.sav', 'old_turn.sav', 'safeturn.sav']) {
      const p = path.join(dir, f);
      if (!fs.existsSync(p)) { console.log('  --   ' + f + ' (absent)'); continue; }
      const sz = fs.statSync(p).size;
      console.log((sz === expect ? '  OK   ' : '  FAIL ') + f + ' is ' + sz + ' bytes');
    }
    const cp = path.join(dir, 'Castles.dat');
    if (fs.existsSync(cp)) {
      const sz = fs.statSync(cp).size;
      console.log((sz === CASTLE_BLOCKS * CASTLE_BLOCK ? '  OK   ' : '  FAIL ') +
                  'Castles.dat is ' + sz + ' bytes');
    }
    return;
  }

  const sav = openSave(dir, [a,b].find(v => v && v.endsWith('.sav')));
  if (mode === 'county') dumpArray(sav, tab, COUNTY_BASE, COUNTY_STRIDE, COUNTY_N, COUNTY_FIELDS, 'counties');
  else if (mode === 'realm') dumpArray(sav, tab, REALM_BASE, REALM_STRIDE, REALM_N, REALM_FIELDS, 'realms');
  else if (mode === 'globals') {
    for (const [va, ty, name] of GLOBALS) {
      const off = saveOffset(tab, va);
      if (off < 0) { console.log(name + ': not saved'); continue; }
      console.log('0x' + va.toString(16) + '  ' + name.padEnd(20) + get(sav, off, ty));
    }
  }
  else if (mode === 'raw') {
    const va = parseInt(a, 16), len = parseInt(b, 10);
    const off = saveOffset(tab, va);
    if (off < 0) { console.log('not in a saved block'); return; }
    for (let i = 0; i < len; i += 16) {
      const row = sav.slice(off + i, off + Math.min(i + 16, len));
      console.log('0x' + (va + i).toString(16) + '  ' +
        [...row].map(x => x.toString(16).padStart(2, '0')).join(' ').padEnd(48) + ' |' +
        [...row].map(x => x >= 32 && x < 127 ? String.fromCharCode(x) : '.').join('') + '|');
    }
  }
}
main();
