// leads.js - mechanical naming LEADS for the functions nobody has named.
//
//   node tools/oracle/leads.js                 # -> docs/name-leads.json
//   node tools/oracle/leads.js --out <path>
//
// L2.eng and Lords2.exe come from $LORDS2_DIR (or --exe/--eng, passed through);
// the corpus from --decomp / $LORDS2_DECOMP / the main checkout.
//
// # What this is
//
// Every FUN_xxxxxxxx in the corpus that docs/symbols.json does not name, joined
// to whatever mechanical evidence five existing tools can produce about it, with
// a suggested name and a confidence. It runs those tools as subprocesses and
// parses their output; it reimplements none of them.
//
//   anchor.js strings   L2.eng groups drawn - a group with ONE consumer is that
//                       screen's vocabulary (CLAUDE.md rule 6), and its index-0
//                       label is the game's own word for the screen; the same
//                       mapping, by hand and with consumers, is eng.md 5.8
//   anchor.js stride    which record array the body indexes
//   anchor.js screens   screen id -> painter, for naming widget handlers
//   logstrings.js       Log_Write messages; eight name their own routine (C72)
//   kinds.js            the 24-byte input records: handler at +8, kind at +0x0F
//   xref.js build       the call graph, for "called by exactly one named caller"
//
// # What it does not claim
//
// A name. Every row here is a HYPOTHESIS and correction C3 is what happens when
// one is mistaken for a finding. docs/symbols.md takes a name only from an agent
// that read the body and had a check that could have failed (CLAUDE.md rule 4).
// Two of the joins below are themselves inferred and say so in the row:
// a record's table is the nearest table base at or below it, and a screen id is
// the nearest `g_screenId == N` above the call site that draws that table.

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const ORACLE = __dirname;
const ROOT = path.resolve(ORACLE, '..', '..');
const argOf = k => { const i = process.argv.indexOf(k); return i > 0 ? process.argv[i + 1] : null; };
const OUT = argOf('--out') || path.join(ROOT, 'docs', 'name-leads.json');

// Same resolution order as xref.js / logstrings.js: the corpus is gitignored, so
// a worktree has none of its own.
function findCorpus() {
  const explicit = argOf('--decomp') || process.env.LORDS2_DECOMP;
  if (explicit) return explicit;
  const here = path.join(ORACLE, 'decomp');
  if (fs.existsSync(here)) return here;
  try {
    const common = execFileSync('git', ['rev-parse', '--git-common-dir'],
      { cwd: ORACLE, encoding: 'utf8' }).trim();
    const there = path.join(path.resolve(ORACLE, common, '..'), 'tools', 'oracle', 'decomp');
    if (fs.existsSync(there)) return there;
  } catch { /* not a checkout */ }
  return here;
}
const DECOMP = findCorpus();
if (!fs.existsSync(DECOMP)) {
  console.error(`leads: no corpus at ${DECOMP}\n  run tools/oracle/decompile-all.ps1 first`);
  process.exit(1);
}

const pass = ['--exe', '--eng'].flatMap(k => (argOf(k) ? [k, argOf(k)] : []));
function run(script, args) {
  return execFileSync('node', [path.join(ORACLE, script), ...args, '--decomp', DECOMP, ...pass],
    { encoding: 'utf8', maxBuffer: 1 << 28, cwd: ORACLE });
}

const hex = a => '0x' + Number(a).toString(16).padStart(8, '0').toUpperCase();
const litNum = s => {
  s = String(s).trim();
  let m = /^'\\x([0-9a-f]{2})'$/i.exec(s); if (m) return parseInt(m[1], 16);
  m = /^'(.)'$/.exec(s); if (m) return s.charCodeAt(1);
  m = /^(0x[0-9a-f]+|\d+)$/i.exec(s); if (m) return Number(m[1]);
  return null;
};

// ---- who is unnamed --------------------------------------------------------

const symbols = JSON.parse(fs.readFileSync(path.join(ROOT, 'docs', 'symbols.json'), 'utf8'));
const namedAddrs = new Set(symbols.functions.map(f => f.addr.toUpperCase()));
const globalAddr = new Map(symbols.globals.map(g => [g.name, parseInt(g.addr, 16)]));

// xref.js's index is its own output; build it if this checkout has none.
const XREF = path.join(ORACLE, 'out', 'xref.json');
if (!fs.existsSync(XREF)) run('xref.js', ['build']);
const xfns = JSON.parse(fs.readFileSync(XREF, 'utf8')).functions;

// A candidate is spelled FUN_ in the corpus AND absent from symbols.json - the
// corpus goes stale the moment a name is added, and believing it would offer
// leads for functions somebody already named.
const cand = new Map();     // "FUN_00401234" -> row
for (const [name, f] of Object.entries(xfns)) {
  if (!name.startsWith('FUN_')) continue;
  const addr = hex(parseInt(f.addr, 16));
  if (namedAddrs.has(addr)) continue;
  cand.set(name, { addr, bytes: f.bytes, callers: f.callers, evidence: [] });
}
// Referenced but never defined: no body, so no size.
for (const f of Object.values(xfns)) {
  for (const c of f.calls) {
    if (!c.startsWith('FUN_') || xfns[c] || cand.has(c)) continue;
    const addr = hex(parseInt(c.slice(4), 16));
    if (namedAddrs.has(addr)) continue;
    cand.set(c, { addr, bytes: null, callers: [], evidence: [] });
  }
}
const byAddr = new Map([...cand].map(([n, r]) => [r.addr, Object.assign(r, { fun: n })]));
const at = va => byAddr.get(hex(va));

// ---- evidence 1: sole consumer of an L2.eng group ---------------------------
//
// anchor.js strings prints every function that draws a literal group, named or
// not, which is what makes "sole consumer" countable at all.

const engLabel = new Map();                 // group -> index-0 label
const engConsumers = new Map();             // group -> Set(key)
const engDrawn = new Map();                 // key -> Set(group)
{
  let key = null;
  for (const line of run('anchor.js', ['strings', '--limit', '100000']).split('\n')) {
    let m = /^(\S+)\s+(\d+)b\s*(\[unnamed\])?\s*$/.exec(line);
    if (m) { key = m[1]; continue; }
    m = /^ {4}group\s+(\d+)\s+label\s+(".*")$/.exec(line);
    if (!m || !key) continue;
    const g = +m[1];
    try { engLabel.set(g, JSON.parse(m[2])); } catch { /* label with an escape we do not need */ }
    if (!engConsumers.has(g)) engConsumers.set(g, new Set());
    engConsumers.get(g).add(key);
    if (!engDrawn.has(key)) engDrawn.set(key, new Set());
    engDrawn.get(key).add(g);
  }
}
// "Select a castle to build" -> "SelectCastleBuild". Words under three letters
// are articles and prepositions in every label in the file.
const camel = s => (s.match(/[A-Za-z]{3,}/g) || []).slice(0, 3)
  .map(w => w[0].toUpperCase() + w.slice(1).toLowerCase()).join('') || 'Eng';

for (const [g, set] of engConsumers) {
  if (set.size !== 1) continue;
  const only = [...set][0];
  const row = only.startsWith('0x') ? at(parseInt(only, 16)) : null;
  if (!row) continue;                       // the sole consumer is already named
  row.evidence.push({
    kind: 'eng-sole-consumer',
    detail: `the only consumer of L2.eng group ${g}, label ${JSON.stringify(engLabel.get(g) || '')}`,
    group: g, confidence: 'high', name: `${camel(engLabel.get(g))}_Draw`,
  });
}

// ---- evidence 2: a Log_Write message naming its own routine -----------------
//
// C72: "ERR:gen_frame bad data " is the C convention of putting the function's
// own name in its own error. The eight that do it are the exception - a message
// otherwise says what the CALLER was doing - so this demands the shape: an
// identifier with an underscore or an internal capital, then a failure word.
const SELF = /^(?:ERR|OK)\s*:\s*([A-Za-z][A-Za-z0-9_]*)\s+(bad data|no data|error|failed|no misc data)/i;
{
  let row = null;
  for (const line of run('logstrings.js', []).split('\n')) {
    let m = /^\*?\s*0x([0-9A-Fa-f]{8})\s+(\S+)\s+(\d+)b\s*$/.exec(line);
    if (m) { row = at(parseInt(m[1], 16)); continue; }
    m = /^ {6}(".*")$/.exec(line);
    if (!m || !row) continue;
    let msg; try { msg = JSON.parse(m[1]); } catch { continue; }
    const s = SELF.exec(msg);
    if (!s) continue;
    const id = s[1];
    if (!(id.includes('_') || /[a-z][A-Z]/.test(id))) continue;   // "ERR:screens no misc data"
    row.evidence.push({
      kind: 'logstring-self-name',
      detail: `its own Log_Write message names it: ${JSON.stringify(msg)}`,
      confidence: 'high', name: id,
    });
  }
}

// ---- evidence 3: an input record's handler ---------------------------------
//
// kinds.js walks both table regions and reports handler + kind byte + record
// address. What it cannot say is WHOSE table a record is in, so that is joined
// here from the call sites that draw and test the tables.

// 3a. table bases, and the screen id each is drawn under.
const tables = [];                // { va, count, fn, screen }
const screenOfFn = new Map();
{
  // Up to the end of the statement, not to the first `)`: Ghidra writes the
  // table argument as `(short *)&DAT_004dd640`, and a lazy match to the first
  // close paren stops inside that cast and finds no table at all.
  const CALL = /\b(Widget_Draw|Widget_Test|Hotspot_Draw|Hotspot_Test)\s*\(([^;]{0,240})/g;
  const HEAD = /^\/\/ ==== ([0-9a-f]{8})\s+(\S+)\s+params=(-?\d+)\s+bytes=(\d+)\s*$/gm;
  for (const file of fs.readdirSync(DECOMP).filter(f => f.endsWith('.c')).sort()) {
    // Our own prose in a comment names tables it is only discussing.
    const text = fs.readFileSync(path.join(DECOMP, file), 'utf8')
      .replace(/\/\*[\s\S]*?\*\//g, m => ' '.repeat(m.length))
      .replace(/^(?!\/\/ ====).*\/\/.*$/gm, m => ' '.repeat(m.length));
    const marks = [];
    HEAD.lastIndex = 0;
    let m;
    while ((m = HEAD.exec(text)) !== null) marks.push({ name: m[2], start: m.index });
    const fnAt = i => { let f = null; for (const k of marks) { if (k.start > i) break; f = k; } return f; };
    const screens = [...text.matchAll(/g_screenId\s*==\s*('[^']*'|0x[0-9a-f]+|\d+)/g)]
      .map(x => ({ i: x.index, id: litNum(x[1]) })).filter(x => x.id !== null);
    CALL.lastIndex = 0;
    while ((m = CALL.exec(text)) !== null) {
      const args = m[2];
      const t = /&\s*(DAT_[0-9a-f]{8}|[A-Za-z_]\w+)\s*,\s*([^,)]*)/.exec(args);
      if (!t) continue;
      const va = /^DAT_/.test(t[1]) ? parseInt(t[1].slice(4), 16) : globalAddr.get(t[1]);
      if (!va) continue;
      const count = litNum(t[2]);        // null when the count is a variable
      const fn = fnAt(m.index);
      // Nearest `g_screenId == N` above the call site and inside this function.
      let screen = null;
      for (const s of screens) {
        if (s.i > m.index) break;
        if (fn && s.i < fn.start) continue;
        if (m.index - s.i < 4000) screen = s.id;
      }
      tables.push({ va, count, fn: fn && fn.name, screen });
    }
  }
  tables.sort((a, b) => a.va - b.va);
}
// 3b. screen id -> painter, from anchor.js screens.
const painter = new Map();
for (const line of run('anchor.js', ['screens']).split('\n')) {
  const m = /^\s{2}0x([0-9a-f]{2})\s+(\S+)/.exec(line);
  if (m && m[2] !== '-') painter.set(parseInt(m[1], 16), m[2]);
}
const GESTURE = {
  'hotspot 1': 'Press', 'hotspot 2': 'Repeat', 'hotspot 3': 'Release',
  'widget 4': 'Held', 'widget 5': 'Button',
};
{
  const found = [];             // { row, kind, va, t }
  let kind = null;
  for (const line of run('kinds.js', []).split('\n')) {
    let m = /^(hotspot \d|widget \d)\s+\S+\s+\d+ records/.exec(line);
    if (m) { kind = m[1]; continue; }
    m = /^ {4}(FUN_[0-9a-f]{8})\s+x\s*\d+\s+(.*)$/.exec(line);
    if (!m || !kind) continue;
    const row = at(parseInt(m[1].slice(4), 16));
    if (!row) continue;
    for (const r of m[2].matchAll(/0x([0-9a-f]+)\((-?\d+),(-?\d+)\)f(-?\d+)id(-?\d+)/g)) {
      const va = parseInt(r[1], 16);
      // Nearest table base at or below the record; its count, when the call site
      // passed a literal one, has to cover it. INFERRED - the tables are laid
      // out contiguously and nothing marks where one ends.
      let t = null;
      for (const c of tables) {
        if (c.va > va) break;
        if (c.count && va >= c.va + 24 * c.count) continue;
        t = c;
      }
      if (!t || t.screen === null || t.screen === undefined) continue;
      found.push({ row, kind, va, t });
      break;    // one record is the lead; the rest are the same lead
    }
  }
  // Number a handler by its record's position among that SCREEN's records, in
  // address order. A screen owns several tables - the setup screen has one per
  // page - so numbering within a table collides and numbering within a screen
  // does not.
  const perScreen = new Map();
  for (const f of found.sort((a, b) => a.va - b.va)) {
    const n = (perScreen.get(f.t.screen) || 0) + 1;
    perScreen.set(f.t.screen, n);
    const p = painter.get(f.t.screen);
    const base = p && !p.startsWith('@')
      ? p.replace(/_(Draw|Screen|Panel)$/, '').replace(/^Screen_(?=.)/, '')
      : `Screen${hex(f.t.screen).slice(-2)}`;
    f.row.evidence.push({
      kind: 'input-record-handler',
      detail: `handler of the ${f.kind} record at 0x${f.va.toString(16)}, slot ` +
        `${Math.floor((f.va - f.t.va) / 24) + 1} of the table at 0x${f.t.va.toString(16)} that ` +
        `${f.t.fn || '?'} tests under g_screenId == 0x${f.t.screen.toString(16)} ` +
        `(${p || 'no painter'}); table membership and screen id are INFERRED`,
      screen: f.t.screen, gesture: f.kind, confidence: 'medium',
      name: `${base}_${GESTURE[f.kind] || 'Input'}${n}`,
    });
  }
}

// ---- evidence 4: called by exactly one named function ----------------------
for (const [name, row] of cand) {
  const cs = row.callers || [];
  if (cs.length !== 1 || cs[0].startsWith('FUN_')) continue;
  const caller = xfns[cs[0]];
  if (!caller) continue;
  // Which helper of that caller is it? Address order, so the number is stable.
  const sibs = caller.calls.filter(c => cand.has(c) && (xfns[c]?.callers || []).length === 1)
    .sort((a, b) => a.localeCompare(b));
  row.evidence.push({
    kind: 'sole-named-caller',
    detail: `called by ${cs[0]} and by nothing else in the corpus`,
    confidence: 'medium', name: `${cs[0]}_helper${sibs.indexOf(name) + 1}`,
  });
}

// ---- evidence 5: a record stride, plus a single named caller ---------------
{
  const strides = new Map();      // key -> "county×3 realm×1"
  let table = false;
  for (const line of run('anchor.js', ['stride', '--limit', '100000']).split('\n')) {
    if (/^  record\s+stride/.test(line)) { table = true; continue; }
    const m = /^ {2}(\S+)\s+(\d+)b\s+(\S.*)$/.exec(line);
    if (table && m) strides.set(m[1], m[3].trim());
  }
  for (const [key, tag] of strides) {
    if (!key.startsWith('0x')) continue;
    const row = at(parseInt(key, 16));
    if (!row) continue;
    const named = (row.callers || []).filter(c => !c.startsWith('FUN_'));
    if (named.length !== 1) continue;
    const kind = /^(\w+)/.exec(tag)[1];
    row.evidence.push({
      kind: 'stride-and-caller',
      detail: `indexes the ${tag} record stride and ${named[0]} is its only named caller`,
      confidence: 'low', name: `${named[0]}_${kind[0].toUpperCase()}${kind.slice(1)}Helper`,
    });
  }
}

// ---- emit ------------------------------------------------------------------

const RANK = { high: 0, medium: 1, low: 2 };
const rows = [];
let omitted = 0;
for (const [name, row] of [...cand].sort((a, b) => a[1].addr.localeCompare(b[1].addr))) {
  if (!row.evidence.length) { omitted++; continue; }
  row.evidence.sort((a, b) => RANK[a.confidence] - RANK[b.confidence]);
  const best = row.evidence[0];
  rows.push({
    addr: row.addr,
    corpus: name,
    bytes: row.bytes,
    suggested: best.name,
    confidence: best.confidence,
    evidence: row.evidence.map(e => ({ kind: e.kind, confidence: e.confidence, suggests: e.name, detail: e.detail })),
  });
}
const counts = { unnamed: cand.size, emitted: rows.length, omitted, high: 0, medium: 0, low: 0 };
for (const r of rows) counts[r.confidence]++;

fs.writeFileSync(OUT, JSON.stringify({
  _note: [
    'THESE ARE LEADS, NOT FINDINGS.',
    '',
    'Every row is a hypothesis generated mechanically by tools/oracle/leads.js from the',
    'decompiled corpus, L2.eng and Lords2.exe. A row says what a function is ABOUT - the',
    'group it alone draws, the message it prints about itself, the input record it handles,',
    'the one function that calls it. None of that says what it IS, and correction C3 is what',
    'happens when such a guess is filed as a fact.',
    '',
    'docs/symbols.md and docs/symbols.json take a name ONLY from an agent that read the body',
    'and had a check that could have failed (CLAUDE.md rule 4). Copying `suggested` into',
    'symbols.json without that check is exactly the defect rule 4 exists to prevent.',
    '',
    'Regenerate: node tools/oracle/leads.js. It is derived, so edit the script, never this.',
  ],
  generated: new Date().toISOString(),
  counts,
  leads: rows,
}, null, 1) + '\n');

console.log(`${counts.unnamed} unnamed functions in the corpus and not in docs/symbols.json`);
console.log(`${counts.emitted} rows emitted: ${counts.high} high, ${counts.medium} medium, ${counts.low} low`);
console.log(`${counts.omitted} omitted - no mechanical evidence of any of the five kinds`);
console.log(`-> ${OUT}`);
