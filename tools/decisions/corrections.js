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
const crypto = require('crypto');

const repo = path.resolve(__dirname, '..', '..');
const logPath = path.join(repo, 'docs', 'decisions.md');
const lockPath = path.join(__dirname, 'citations.lock');
const check = process.argv.includes('--check');
const relock = process.argv.includes('--relock');

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
const placeholders = [];
(function walk(dir) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    if (SKIP_DIR.has(e.name)) continue;
    const p = path.join(dir, e.name);
    if (e.isDirectory()) { walk(p); continue; }
    if (!EXT.test(e.name)) continue;
    let s;
    try { s = fs.readFileSync(p, 'utf8'); } catch { continue; }
    // **Normalise line endings before anything measures an offset.**
    // `.gitattributes` pins `*.ps1` to CRLF and everything else to LF, so the
    // same citation has different byte offsets in a `.ps1` than in a `.md` —
    // and the fingerprint below slices a fixed *character* window, so a stray
    // `\r` shifts which words fall inside it. Two agents relocking the same
    // unchanged tree produced four different fingerprints in `tools/oracle/*.ps1`
    // because of this, twice. A fingerprint must be a property of the text, not
    // of the checkout. This repository has been bitten by line endings before —
    // see the comment at the top of `.gitattributes`.
    s = s.replace(/\r\n/g, '\n');
    const rel = path.relative(repo, p).replace(/\\/g, '/');
    const isLog = rel === 'docs/decisions.md';

    // ---- rule 4: no unassigned placeholder may reach `main` ----------------
    //
    // An agent writing on a branch cannot know which number its correction will
    // get: C61 had SIX claimants in one day, from six branches that all
    // correctly read C60 as the highest. So a branch writes `CNEW-<slug>` (or
    // `BNEW-<slug>` for `docs/bugs.md`) and the integrator assigns a number at
    // merge — which is the one moment serialisation is free.
    //
    // **This rule exists because the scheme was proposed, approved, not built,
    // and then failed within hours in exactly the predicted way.** A branch's
    // own `CNEW-withdrawal` entry was merged to `main` unassigned, and a second
    // correction about the same work was appended beside it, because the
    // integrator did not notice the first. Two entries, one piece of work, and
    // nothing to catch it. The convention travelled — agents adopted `CNEW-`
    // before the check existed — which is exactly what makes the check
    // necessary rather than redundant: a convention people follow produces
    // artefacts that need collecting.
    //
    // Skipped inside this tool and inside `docs/agents.md`, which have to be
    // able to *describe* the convention.
    if (rel !== 'tools/decisions/corrections.js' && rel !== 'docs/agents.md') {
      for (const m of s.matchAll(/\b([CB])NEW-([a-z0-9-]+)/g)) {
        const before = s.slice(Math.max(0, m.index - 200), m.index);
        placeholders.push({ file: rel, tag: `${m[1]}NEW-${m[2]}`, kind: m[1], before });
      }
    }
    for (const m of s.matchAll(/\bC(\d{1,3})\b/g)) {
      const n = Number(m[1]);
      const win = s.slice(Math.max(0, m.index - 220), m.index + 220);
      const cites = isLog || /decisions\.md/.test(win) || /\bcorrections?\b/i.test(win);
      if (!cites) continue;
      // its own heading is a definition, not a citation
      if (isLog && new RegExp(`^\\*\\*C${n} — `).test(s.slice(m.index - 2, m.index + 40))) continue;
      const line = s.slice(0, m.index).split(/\r?\n/).length;
      // The words either side, with every C-number blanked so the fingerprint
      // does NOT move when the number does. That is the whole trick of rule 3.
      const ctx = (s.slice(Math.max(0, m.index - 120), m.index) + '|'
        + s.slice(m.index + m[0].length, m.index + 120))
        .replace(/\bC\d{1,3}\b/g, '#').replace(/\s+/g, ' ').trim().toLowerCase();
      const fp = crypto.createHash('sha1').update(ctx).digest('hex').slice(0, 12);
      citations.push({ rel, line, n, fp, historical: HISTORICAL.test(win) });
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

if (placeholders.length) {
  const byTag = new Map();
  for (const h of placeholders) {
    if (!byTag.has(h.tag)) byTag.set(h.tag, []);
    byTag.get(h.tag).push(h.file);
  }
  console.error(
    `corrections: ${byTag.size} unassigned placeholder(s) reached the tree.\n`,
  );
  for (const [tag, files] of byTag) {
    const uniq = [...new Set(files)];
    console.error(`  ${tag}  in ${uniq.length} file(s): ${uniq.slice(0, 6).join(', ')}`);
  }
  const kinds = new Set(placeholders.map(h => h.kind));
  const log = kinds.has('C') ? 'docs/decisions.md' : 'docs/bugs.md';
  console.error(
    `\nA placeholder is how a BRANCH writes a correction whose number it cannot\n` +
    `know: several branches routinely pick the same next number. Assigning it is\n` +
    `the integrator's job at merge, and it is one command per tag:\n\n` +
    `    the next free number in ${log}, then across the whole tree\n` +
    `    (headings, prose, symbols.json comments, Rust doc comments, tests)\n\n` +
    `Assign it or the entry is invisible to every citation this tool checks.`,
  );
  process.exit(1);
}

// ---- rule 3: a citation whose number moved while its words did not ---------
//
// Rules 1 and 2 verify that a citation *resolves*. They cannot see whether it
// resolves to the RIGHT correction, and a renumber done by search-and-replace
// drags unrelated citations along: they all resolve, and they are all wrong.
// That happened FOUR times in one session — once for real, and three more times
// while renumbering colliding corrections — with rule 2 reporting "all citations
// resolve" every time. Every one was found by a person reading.
//
// So each citation is fingerprinted by the words around it, with C-numbers
// blanked. A dragged citation keeps its fingerprint and changes its number,
// which is precisely what this compares. Replayed against the real failure it
// flagged that one citation and nothing else, across a merge that moved seven
// citations and drifted every line number in the tree.

const lockLine = c => `${c.rel}\tC${c.n}\t${c.fp}`;
const LOCK_HEADER = [
  '# Citation fingerprints — see tools/decisions/corrections.js, rule 3.',
  '#',
  '# One line per correction citation: file, the number it cites, and a hash of',
  '# the words around it with every C-number blanked. A citation whose number',
  '# changes while its words do not is a citation dragged along by somebody',
  '# renumbering a heading, and that is what this file exists to catch.',
  '#',
  '# Regenerate with:  node tools/decisions/corrections.js --relock',
  '# Generated. Do not hand-edit except to accept a single deliberate change.',
  '',
].join('\n');

const current = citations.map(lockLine).sort();

if (relock) {
  // Show any drag being recorded rather than swallowing it: --relock is how a
  // deliberate correction is accepted, so it must not be a silent way past a
  // real one.
  if (fs.existsSync(lockPath)) {
    const was = new Map(fs.readFileSync(lockPath, 'utf8').split(/\r?\n/)
      .filter(l => l && !l.startsWith('#'))
      .map(l => { const [rel, n, fp] = l.split('\t'); return [rel + '\t' + fp, n]; }));
    const moved = citations.filter(c => was.has(c.rel + '\t' + c.fp) && was.get(c.rel + '\t' + c.fp) !== 'C' + c.n);
    if (moved.length) {
      console.log(`corrections: recording ${moved.length} citation(s) whose number changed`);
      console.log('  while the words around them did not. If any of these was not deliberate,');
      console.log('  it is a dragged citation — undo it rather than keeping this lockfile.\n');
      for (const c of moved)
        console.log(`  ${c.rel}:${c.line}  ${was.get(c.rel + '\t' + c.fp)} -> C${c.n}`);
      console.log('');
    }
  }
  fs.writeFileSync(lockPath, LOCK_HEADER + current.join('\n') + '\n');
  console.log(`corrections: relocked ${current.length} citations`);
  process.exit(0);
}

if (!fs.existsSync(lockPath)) {
  fail('tools/decisions/citations.lock is missing.\n'
    + '  Create it with:  node tools/decisions/corrections.js --relock');
}

{
  const lockLines = fs.readFileSync(lockPath, 'utf8').split(/\r?\n/).filter(l => l && !l.startsWith('#'));
  const locked = new Map(lockLines.map(l => {
    const [rel, n, fp] = l.split('\t');
    return [rel + '\t' + fp, n];
  }));

  const dragged = [];
  for (const c of citations) {
    const key = c.rel + '\t' + c.fp;
    if (locked.has(key) && locked.get(key) !== 'C' + c.n)
      dragged.push({ ...c, was: locked.get(key) });
  }

  if (dragged.length) {
    console.error(`corrections: ${dragged.length} citation(s) changed number while their surrounding words did not.\n`);
    console.error('  **This looks like a dragged citation** — a renumber that took an unrelated');
    console.error('  reference with it. Such a citation still resolves, so rule 2 passes and the');
    console.error('  reference now silently points at somebody else\'s correction.\n');
    for (const d of dragged) {
      console.error(`  ${d.rel}:${d.line}   ${d.was} -> C${d.n}`);
      console.error(`      was: ${d.rel}\t${d.was}\t${d.fp}`);
      console.error(`      now: ${lockLine(d)}`);
    }
    console.error('\n  Read each one and decide which correction it MEANS.');
    console.error('  If the old number was right, put it back.');
    console.error('  If this is a deliberate correction of a citation that was wrong, accept it:');
    console.error('      node tools/decisions/corrections.js --relock');
    console.error('  (or edit the line above in tools/decisions/citations.lock by hand).');
    process.exit(1);
  }

  const lockedSet = new Set(lockLines);
  const currentSet = new Set(current);
  const added = current.filter(l => !lockedSet.has(l));
  const gone = lockLines.filter(l => !currentSet.has(l));
  if (added.length || gone.length) {
    console.error('corrections: tools/decisions/citations.lock is out of date '
      + `(${added.length} added, ${gone.length} removed).`);
    console.error('  No dragged citation was found — this is the ordinary case of citations');
    console.error('  being written, moved or reworded. A stale lockfile protects nothing, so:');
    console.error('      node tools/decisions/corrections.js --relock');
    process.exit(1);
  }
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
