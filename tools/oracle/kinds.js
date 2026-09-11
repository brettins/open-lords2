// kinds.js - enumerate the five gesture KINDS out of Lords2.exe.
//
// Why this exists, and why it is not widgets.js. `widgets.js` decodes ONE
// table, named by an address somebody read off a call site, and it is the right
// tool when you are reading a screen. This one walks the whole of both table
// regions and asks a different question: *how many records of each kind are
// there, and whose are they?*
//
// That question cannot be answered by reading call sites, because the kind byte
// at +0x0F is never mentioned in code - the two hit-testers compare it against
// literals and nothing else names it. It also cannot be answered by reading
// docs/arms.json, which counts arms somebody enumerated and is silent about the
// ones nobody has. So a count of kinds has exactly one source and this is it.
//
//   node tools/oracle/kinds.js              the five kinds, with every handler
//   node tools/oracle/kinds.js --counts     just the totals
//
// The exe is found via $LORDS2_DIR, or --exe <path>. Names come from
// docs/symbols.json.
//
// THE MODEL, read out of Widget_Test (0x0040DA1E) and Hotspot_Test (0x0040E3EE):
// both walk arrays of 24-byte records with a handler pointer at +0x08 and a
// kind byte at +0x0F, and THE NUMBERS DO NOT OVERLAP between the two testers.
// Hotspot_Test's 3 is a release; Widget_Test has no 3 that fires at all. So the
// tester is part of the question, and it is decided here by which region the
// record is in.
//
//   hotspot 1  fires on g_mouseLeftPressed, the down edge, nothing drawn
//   hotspot 2  down edge, then every 320 ms while held (DAT_0057D3C8)
//   hotspot 3  fires on g_mouseLeftReleased, the up edge
//   widget  4  down edge, pressed picture for 3 frames, ACCELERATING repeat
//   widget  5  down edge shows the pressed picture; handler runs 20 frames later
//
// THE REGION BOUNDS ARE THE ONE THING HERE THAT IS INFERRED. The tables are laid
// out contiguously at a 24-byte stride, and the split between the two testers is
// 0x004DD310 - g_confirmWidgets, the lowest table any Widget_Test call site
// names. A wrong bound would decode plausible rubbish rather than nothing, so
// two records are spot-checked against handlers named from call sites before
// anything below them is believed, and the script exits non-zero if either
// fails. crates/l2-game/tests/arms.rs makes the same two checks for the same
// reason.

const fs = require('fs');
const path = require('path');

function opt(name, dflt) {
  const i = process.argv.indexOf('--' + name);
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : dflt;
}
const EXE = opt('exe', path.join(process.env.LORDS2_DIR || 'F:/games/Lords of the Realm II', 'Lords2.exe'));
if (!fs.existsSync(EXE)) {
  console.error(`no Lords2.exe at ${EXE}\nset LORDS2_DIR, or pass --exe <path>`);
  process.exit(1);
}
const b = fs.readFileSync(EXE);

const pe = b.readUInt32LE(0x3c);
const nsec = b.readUInt16LE(pe + 6), optSize = b.readUInt16LE(pe + 20), optH = pe + 24;
const BASE = b.readUInt32LE(optH + 28), secOff = optH + optSize;
const secs = [];
for (let i = 0; i < nsec; i++) {
  const o = secOff + i * 40;
  secs.push({
    va: b.readUInt32LE(o + 12), vs: b.readUInt32LE(o + 8),
    raw: b.readUInt32LE(o + 20), rs: b.readUInt32LE(o + 16),
  });
}
const va2off = va => {
  const r = va - BASE;
  for (const s of secs) if (r >= s.va && r < s.va + Math.max(s.vs, s.rs)) return s.raw + (r - s.va);
  return -1;
};

const names = new Map();
try {
  const sym = JSON.parse(fs.readFileSync(path.join(__dirname, '..', '..', 'docs', 'symbols.json'), 'utf8'));
  for (const f of sym.functions || []) names.set(parseInt(f.addr, 16), f.name);
} catch { /* names are a convenience; the counts do not need them */ }
const nm = a => names.get(a) || ('FUN_' + a.toString(16).padStart(8, '0'));

// The gesture word docs/arms.json files each kind under. Kept spelled out
// rather than derived, because these five strings are the vocabulary two other
// artefacts are checked against.
const GESTURE = {
  'hotspot 1': 'left-press',
  'hotspot 2': 'left-press-held',
  'hotspot 3': 'left-release',
  'widget 4': 'left-press-repeat',
  'widget 5': 'left-press-delayed',
};
const REGIONS = [['hotspot', 0x004DC4D0, 0x004DD310], ['widget', 0x004DD310, 0x004DE400]];

const rows = [];
for (const [tester, lo, hi] of REGIONS) {
  for (let va = lo; va < hi; va += 24) {
    const o = va2off(va);
    if (o < 0) continue;
    const handler = b.readUInt32LE(o + 8);
    // A record whose +0x08 is not a plausible code address is not a record: the
    // regions hold a little padding and a few tables of other shapes.
    if (handler < 0x00401000 || handler >= 0x004D0000) continue;
    rows.push({
      key: tester + ' ' + b[o + 15], handler, va,
      x: b.readInt16LE(o), y: b.readInt16LE(o + 2),
      frame: b.readInt16LE(o + 4), size: b.readInt16LE(o + 6), id: b.readInt32LE(o + 16),
    });
  }
}

// --- the two spot checks, before a word of the above is believed ------------
const at = va => rows.find(r => r.va === va);
const must = (va, key, why) => {
  const r = at(va);
  if (!r || r.key !== key) {
    console.error(`SPOT CHECK FAILED: 0x${va.toString(16)} should be ${key} (${why}), ` +
      `got ${r ? r.key + ' ' + nm(r.handler) : 'no record'}.\n` +
      `The region bounds or the stride are wrong and every count below is rubbish.`);
    process.exit(1);
  }
};
must(0x004DC6F8, 'hotspot 1', 'Turn_End, the End Turn strip');
must(0x004DD310, 'widget 5', 'Ui_ConfirmClicked, the yes/no box');

const byKind = new Map();
for (const r of rows) {
  if (!byKind.has(r.key)) byKind.set(r.key, []);
  byKind.get(r.key).push(r);
}

const countsOnly = process.argv.includes('--counts');
console.log(`${rows.length} records with a handler, in 0x${REGIONS[0][1].toString(16)}..0x${REGIONS[1][2].toString(16)}\n`);
for (const key of Object.keys(GESTURE)) {
  const rs = byKind.get(key) || [];
  const handlers = new Map();
  for (const r of rs) {
    if (!handlers.has(r.handler)) handlers.set(r.handler, []);
    handlers.get(r.handler).push(r);
  }
  console.log(`${key.padEnd(10)} ${GESTURE[key].padEnd(20)} ` +
    `${String(rs.length).padStart(3)} records  ${String(handlers.size).padStart(2)} handlers`);
  if (countsOnly) continue;
  for (const [h, g] of handlers) {
    console.log(`    ${nm(h).padEnd(30)} x${String(g.length).padStart(2)}  ` +
      g.map(r => `0x${r.va.toString(16)}(${r.x},${r.y})f${r.frame}id${r.id}`).join(' '));
  }
  console.log('');
}
// Anything in the regions with a handler and a kind byte outside the five is
// worth seeing: it would mean a sixth kind, or a table of another shape sitting
// inside the bounds.
const other = rows.filter(r => !(r.key in GESTURE));
if (other.length) {
  console.log(`${other.length} record(s) with a handler and a kind byte that is NOT one of the five:`);
  for (const r of other) console.log(`    ${r.key.padEnd(10)} 0x${r.va.toString(16)}  ${nm(r.handler)}`);
}
