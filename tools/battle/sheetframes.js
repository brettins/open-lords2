// sheetframes.js - the frame count of every a2 and a3 troop sheet, and whether
// the identities docs/battle.md 13.5 and 14.4 claim actually hold.
//
//   node tools/battle/sheetframes.js "F:/games/Lords of the Realm II"
//
// This is the check that makes the a2/a3 story evidence rather than a story.
// Twelve animation handlers pair up six-and-six, each a3 twin being the same
// code with four constants changed - which is exactly the shape of correction
// C3 in docs/decisions.md, where three blitters matched three storage modes and
// the story assembled itself. So the pairing is anchored on numbers that do not
// come from the binary at all: the frame counts of the shipped art.
//
//   a2 sheets: 8 * N + 18 frames, N = 10, 13, 12, 12, 8, 13
//   a3 sheets: 8 * N + 13 frames, N =  6,  8,  7,  7, 5,  8
//
// and those two N rows are precisely the constants the a2 and a3 handlers use.
// Knights are the exception in both sets (a table, not a pose block); the horse
// sheets are 48 frames for a2 and 8 for a3, matching `horseFrame = dirc * 6`
// against `horseFrame = dirc`.
//
// A .pl8 is an 8-byte header whose u16 at +2 is the frame count (docs/formats/
// pl8.md). Nothing here decodes pixels, so it is fast and needs no install
// beyond the art.
//
// The `a2g_*` (green) sheets are reported separately and excluded from the
// check, and that is a fact rather than a fudge: docs/battle.md 13.4 has the
// sprite-bank table at 0x004DA550 holding six colours, `w r y k p b`, with `g`
// not among them. They ship on disk and the game never loads them. They are
// also the whole reason to print them: they are 98 and 82 frames - N = 10 for
// every troop that has a bigger sheet in the six live colours - so the unused
// green set is a smaller, presumably earlier art pass, and a reimplementation
// that globbed `a2*_mace.pl8` would silently pick one up and draw the wrong
// frame for every pose past 9. Running with them included is what a wrong N
// looks like: 28 mismatches.
const fs = require('fs');
const path = require('path');

const dir = process.argv[2] || process.env.LORDS2_DIR;
if (!dir) {
  console.log('usage: node tools/battle/sheetframes.js <install dir>');
  process.exit(1);
}

// TROOPS*.ENG column order, which is troop type order (docs/battle.md 8.2a).
const TROOPS = ['psnt', 'cros', 'mace', 'swor', 'pike', 'arch'];
const EXPECT = {
  a2: { shared: 18, n: { psnt: 10, cros: 13, mace: 12, swor: 12, pike: 8, arch: 13 } },
  a3: { shared: 13, n: { psnt: 6, cros: 8, mace: 7, swor: 7, pike: 5, arch: 8 } },
};
const HORSE = { a2: 48, a3: 8 };

function frames(file) {
  const fd = fs.openSync(file, 'r');
  const head = Buffer.alloc(8);
  fs.readSync(fd, head, 0, 8, 0);
  fs.closeSync(fd);
  return head.readUInt16LE(2);
}

const files = fs.readdirSync(dir);
let checked = 0, bad = 0;

for (const set of ['a2', 'a3']) {
  console.log('\n=== ' + set + ' ===');
  const re = new RegExp('^' + set + '([a-z])_(' + TROOPS.join('|') + ')\\.pl8$', 'i');
  const byTroop = {}, green = [];
  for (const f of files) {
    const m = f.match(re);
    if (!m) continue;
    if (m[1].toLowerCase() === 'g') { green.push(f); continue; }   // not in the bank table
    (byTroop[m[2].toLowerCase()] = byTroop[m[2].toLowerCase()] || []).push(f);
  }
  for (const t of TROOPS) {
    const list = (byTroop[t] || []).sort();
    if (!list.length) { console.log('  ' + t + ': no sheets'); continue; }
    const n = EXPECT[set].n[t];
    const want = 8 * n + EXPECT[set].shared;
    const counts = [...new Set(list.map(f => frames(path.join(dir, f))))];
    const ok = counts.length === 1 && counts[0] === want;
    checked += list.length;
    if (!ok) bad += list.length;
    console.log('  ' + t.padEnd(5) + list.length + ' sheets  frames ' + counts.join('/') +
      '   want 8*' + n + '+' + EXPECT[set].shared + ' = ' + want + '  ' + (ok ? 'OK' : 'MISMATCH'));
  }
  if (green.length) {
    const counts = [...new Set(green.map(f => frames(path.join(dir, f))))];
    console.log('  (' + green.length + ' ' + set + 'g_* sheets on disk, not in the bank table, frames ' +
      counts.join('/') + ' - excluded, see the header)');
  }
  const horse = files.find(f => new RegExp('^' + set + '_horse\\.pl8$', 'i').test(f));
  if (horse) {
    const got = frames(path.join(dir, horse));
    const ok = got === HORSE[set];
    checked++; if (!ok) bad++;
    console.log('  horse ' + horse + '  frames ' + got + '   want ' + HORSE[set] + '  ' + (ok ? 'OK' : 'MISMATCH'));
  }
}

console.log('\n' + checked + ' sheets checked, ' + bad + ' mismatches');
console.log(bad === 0
  ? 'The a2 and a3 frames-per-facing rows are the ones the two handler sets use.'
  : 'A mismatch means docs/battle.md 13.5 or 14.4 is wrong about that troop.');
process.exit(bad === 0 ? 0 : 1);
