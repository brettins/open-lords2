// The correction log's numbering, checked.
//
//   node tools/decisions/corrections.js            report
//   node tools/decisions/corrections.js --check    exit 1 on a duplicate or a dangling citation
//
// Two rules, both from things that actually happened:
//
//  1. NO DUPLICATE NUMBER in docs/decisions.md. Agents run concurrently, all
//     read the same log, and all reach for the next free number. This happened
//     FIVE times in one day — three agents on C22, two each on C25, C28, C30 —
//     and every time the integrator found it at merge rather than the agent
//     finding it in its own branch.
//
//  2. EVERY CITATION RESOLVES. Renumbering a heading without its citations is
//     worse than the collision, because the reference then silently points at
//     somebody else's correction. crates/l2-kingdom/src/tax.rs cited "C22" for
//     a correction that had never been written into the log at all; it stood
//     for weeks and was found by accident. See C26.
//
// WHAT COUNTS AS A CITATION. A bare "C12" is far too common to match on — it is
// a register, a cell reference, a hex tail. So a citation is a C-number that
// appears near a mention of `decisions.md`, or after the word "correction". The
// log's own cross-references count, because it cites itself constantly.
//
// THE ESCAPE HATCH IS PROSE, NOT A MAGIC COMMENT. A reference that is
// deliberately historical — pointing at what a number meant before a renumber,
// or at a correction that has since been withdrawn — is exempt when the text
// around it says so, in the words a reader would use anyway: "frozen",
// "historical", "superseded", "withdrawn", "at the time" or "used to be". That
// keeps the exemption visible to somebody reading the document, which a
// `<!-- ignore -->` marker would not be. If you need the hatch, write the
// sentence; if the sentence would be a lie, fix the citation instead.

const fs = require('fs');
const path = require('path');

const repo = path.resolve(__dirname, '..', '..');
const logPath = path.join(repo, 'docs', 'decisions.md');
const check = process.argv.includes('--check');

const fail = m => { console.error('corrections: ' + m); process.exit(1); };

// ---- rule 1: the headings themselves ---------------------------------------

const log = fs.readFileSync(logPath, 'utf8');
const lines = log.split(/\r?\n/);
const headings = new Map();          // number -> [{line, text}]
lines.forEach((L, i) => {
  const m = /^\*\*C(\d+) — (.*)$/.exec(L);
  if (!m) return;
  const n = Number(m[1]);
  if (!headings.has(n)) headings.set(n, []);
  headings.get(n).push({ line: i + 1, text: m[2].replace(/\*\*$/, '').trim() });
});

if (headings.size === 0) fail('found no **C<n> — ...** headings in docs/decisions.md at all');

const numbers = [...headings.keys()].sort((a, b) => a - b);
const nextFree = Math.max(...numbers) + 1;

const dupes = numbers.filter(n => headings.get(n).length > 1);
if (dupes.length) {
  console.error(`corrections: ${dupes.length} correction number(s) used more than once.\n`);
  for (const n of dupes) {
    console.error(`  C${n} is used ${headings.get(n).length} times:`);
    for (const h of headings.get(n))
      console.error(`    docs/decisions.md:${h.line}  ${h.text.slice(0, 84)}`);
  }
  console.error(`\nThe next free number is C${nextFree}.`);
  console.error('Renumber the LATER arrival: change its heading, then grep the tree for');
  console.error('citations of the old number and move them with it —');
  console.error(`    grep -rn "C${dupes[0]}" docs crates tools README.md`);
  console.error('A heading that moved without its citations is worse than the collision.');
  process.exit(1);
}

// A gap is not an error — a withdrawn correction leaves one — but say so.
const gaps = [];
for (let n = 1; n < nextFree; n++) if (!headings.has(n)) gaps.push('C' + n);

// ---- rule 2: citations anywhere in the tree --------------------------------

const SKIP_DIR = new Set(['.git', 'target', 'node_modules', 'decomp', 'out', '.claude']);
const EXT = /\.(rs|md|json|js|ps1|toml|html|java|yml|yaml)$/;

// Words that make a reference deliberately historical. See the header.
const HISTORICAL = /\b(frozen|historical|superseded|withdrawn|at the time|used to be|renumber)/i;

const citations = [];
(function walk(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (SKIP_DIR.has(e.name)) continue;
    const p = path.join(dir, e.name);
    if (e.isDirectory()) { walk(p); continue; }
    if (!EXT.test(e.name)) continue;
    let s;
    try { s = fs.readFileSync(p, 'utf8'); } catch { continue; }
    const rel = path.relative(repo, p).replace(/\\/g, '/');
    const isLog = rel === 'docs/decisions.md';
    for (const m of s.matchAll(/\bC(\d{1,3})\b/g)) {
      const n = Number(m[1]);
      const win = s.slice(Math.max(0, m.index - 220), m.index + 220);
      const cites = isLog || /decisions\.md/.test(win) || /\bcorrections?\b/i.test(win);
      if (!cites) continue;
      // its own heading is a definition, not a citation
      if (isLog && new RegExp(`^\\*\\*C${n} — `).test(s.slice(m.index - 2, m.index + 40))) continue;
      const line = s.slice(0, m.index).split(/\r?\n/).length;
      citations.push({ rel, line, n, historical: HISTORICAL.test(win) });
    }
  }
})(repo);

const dangling = citations.filter(c => !headings.has(c.n) && !c.historical);
if (dangling.length) {
  console.error(`corrections: ${dangling.length} citation(s) point at a correction that does not exist.\n`);
  for (const c of dangling)
    console.error(`  ${c.rel}:${c.line}  cites C${c.n}${gaps.includes('C' + c.n) ? ' (a gap in the log)' : ''}`);
  console.error(`\nThe log runs C1..C${Math.max(...numbers)}${gaps.length ? `, missing ${gaps.join(', ')}` : ''}.`);
  console.error('Either the correction was never written — write it, taking the next free');
  console.error(`number, C${nextFree} — or the citation is stale after a renumber, in which case`);
  console.error('point it at the number the correction actually has now.');
  console.error('If the reference is deliberately historical, say so in the surrounding text');
  console.error('("frozen", "superseded", "used to be") and this check will leave it alone.');
  process.exit(1);
}

// ---- report ----------------------------------------------------------------

const cited = new Set(citations.map(c => c.n));
const uncited = numbers.filter(n => !cited.has(n));
const hist = citations.filter(c => c.historical).length;

const summary = `${headings.size} corrections (C1..C${Math.max(...numbers)}), `
  + `${citations.length} citations, all resolve`;

if (check) {
  console.log('corrections: ' + summary);
} else {
  console.log(summary);
  if (gaps.length) console.log(`  gaps (withdrawn or never written): ${gaps.join(', ')}`);
  if (hist) console.log(`  ${hist} citation(s) exempt as deliberately historical`);
  console.log(`  cited nowhere outside the log: ${uncited.map(n => 'C' + n).join(' ') || '(none)'}`);
  console.log(`  next free number: C${nextFree}`);
}
