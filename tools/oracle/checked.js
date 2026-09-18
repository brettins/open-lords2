// Which cited lines in crates/ an oracle check has read against the binary.
//
//   node tools/oracle/checked.js
//   node tools/oracle/checked.js --json
//
// A *cited line* is a line of Rust naming an address of Lords2.exe, in any of the
// forms the crates use: `0x0043BF07`, `FUN_0043BF07`, `DAT_00553EE0`, `LAB_...`.
// A record in docs/oracle-checks/*.json covers a line when it carries an entry with
// the same file and one of the line's addresses. The line is *stale* when its text
// is not in that file at the record's `reviewed` commit: the check read something
// else.

const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const { crateFiles, ROOT } = require('./dossier.js');

const DIR = path.join(ROOT, 'docs', 'oracle-checks');
const CITE = /(?:0x|FUN_|DAT_|LAB_)(00[0-9a-fA-F]{6})/g;

function records() {
  let names;
  try { names = fs.readdirSync(DIR).filter((n) => n.endsWith('.json')); } catch (e) { return []; }
  return names.sort().map((n) => ({ name: n.replace(/\.json$/, ''), ...JSON.parse(fs.readFileSync(path.join(DIR, n), 'utf8')) }));
}

function addrsOf(text) {
  const out = new Set();
  CITE.lastIndex = 0;
  let m;
  while ((m = CITE.exec(text))) out.add('0x' + m[1].toUpperCase());
  return out;
}

// file -> { addr -> [reviewed commits] }, from every record
function coverage(recs) {
  const by = new Map();
  for (const r of recs) {
    for (const b of r.behaviours || []) {
      if (!by.has(b.file)) by.set(b.file, new Map());
      const m = by.get(b.file);
      const a = String(b.address).toUpperCase().replace(/^0X/, '0x');
      if (!m.has(a)) m.set(a, []);
      m.get(a).push(r.reviewed);
    }
  }
  return by;
}

const blobs = new Map();
function blob(rev, file) {
  const k = rev + ':' + file;
  if (!blobs.has(k)) {
    let t = null;
    try { t = cp.execFileSync('git', ['show', `${rev}:${file}`], { cwd: ROOT, maxBuffer: 1 << 28 }).toString(); } catch (e) { t = null; }
    blobs.set(k, t === null ? null : new Set(t.split(/\r?\n/).map((l) => l.trim())));
  }
  return blobs.get(k);
}

function crateOf(rel) {
  const m = /^crates\/([^/]+)\//.exec(rel);
  return m ? m[1] : '(outside crates)';
}

function scan() {
  const recs = records();
  const cov = coverage(recs);
  const per = new Map();
  const unchecked = [];
  const staleLines = [];
  for (const f of crateFiles()) {
    if (!f.endsWith('.rs')) continue;
    const rel = path.relative(ROOT, f).replace(/\\/g, '/');
    const lines = fs.readFileSync(f, 'utf8').split(/\r?\n/);
    const c = crateOf(rel);
    if (!per.has(c)) per.set(c, { crate: c, cited: 0, checked: 0, stale: 0, unchecked: 0 });
    const p = per.get(c);
    for (let i = 0; i < lines.length; i++) {
      const addrs = addrsOf(lines[i]);
      if (!addrs.size) continue;
      p.cited++;
      const m = cov.get(rel);
      const revs = m ? [...addrs].flatMap((a) => m.get(a) || []) : [];
      if (!revs.length) {
        p.unchecked++;
        unchecked.push({ file: rel, line: i + 1, addresses: [...addrs] });
        continue;
      }
      p.checked++;
      const text = lines[i].trim();
      if (!revs.some((rev) => { const s = blob(rev, rel); return s && s.has(text); })) {
        p.stale++;
        staleLines.push({ file: rel, line: i + 1, addresses: [...addrs] });
      }
    }
  }
  const crates = [...per.values()].filter((p) => p.cited).sort((a, b) => b.cited - a.cited);
  const total = crates.reduce((t, p) => ({ cited: t.cited + p.cited, checked: t.checked + p.checked, stale: t.stale + p.stale, unchecked: t.unchecked + p.unchecked }), { cited: 0, checked: 0, stale: 0, unchecked: 0 });
  return { records: recs.map((r) => ({ name: r.name, reviewed: r.reviewed, date: r.date, behaviours: (r.behaviours || []).length })), crates, total, stale: staleLines, uncheckedLines: unchecked };
}

// Every record entry names a file that exists and an address that file cites.
function verify() {
  const bad = [];
  const cache = new Map();
  for (const r of records()) {
    if (!/^[0-9a-f]{7,40}$/.test(String(r.reviewed || ''))) bad.push(`${r.name}: reviewed "${r.reviewed}" is not a commit`);
    if (!/^\d{4}-\d\d-\d\d$/.test(String(r.date || ''))) bad.push(`${r.name}: date "${r.date}" is not YYYY-MM-DD`);
    for (const b of r.behaviours || []) {
      if (!['match', 'mismatch', 'departure'].includes(b.verdict)) bad.push(`${r.name}: ${b.file}:${b.line} verdict "${b.verdict}"`);
      const p = path.join(ROOT, b.file);
      if (!cache.has(b.file)) cache.set(b.file, fs.existsSync(p) ? fs.readFileSync(p, 'utf8') : null);
      const t = cache.get(b.file);
      if (t === null) { bad.push(`${r.name}: ${b.file} does not exist`); continue; }
      const hex = String(b.address).replace(/^0x/i, '');
      if (!new RegExp(`(?:0x|FUN_|DAT_|LAB_)${hex}`, 'i').test(t)) bad.push(`${r.name}: ${b.file} does not cite ${b.address}`);
    }
  }
  return bad;
}

module.exports = { scan, records, addrsOf, verify };
if (require.main !== module) return;

if (process.argv.includes('--verify')) {
  const bad = verify();
  for (const b of bad) console.log(b);
  process.exit(bad.length ? 1 : 0);
}

const out = scan();
if (process.argv.includes('--json')) {
  console.log(JSON.stringify(out, null, 1));
  return;
}
const w = Math.max(10, ...out.crates.map((c) => c.crate.length));
console.log(`${'crate'.padEnd(w)}  cited  checked   stale  unchecked`);
for (const c of out.crates) {
  console.log(`${c.crate.padEnd(w)}  ${String(c.cited).padStart(5)}  ${String(c.checked).padStart(7)}  ${String(c.stale).padStart(5)}  ${String(c.unchecked).padStart(9)}`);
}
const t = out.total;
console.log(`${'total'.padEnd(w)}  ${String(t.cited).padStart(5)}  ${String(t.checked).padStart(7)}  ${String(t.stale).padStart(5)}  ${String(t.unchecked).padStart(9)}`);
console.log(`\n${out.records.length} record(s) in docs/oracle-checks: ${out.records.map((r) => `${r.name} (${r.behaviours})`).join(', ')}`);
