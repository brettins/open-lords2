// The numbering of the project's two numbered logs, checked — and assigned.
//
//   node tools/decisions/corrections.js                 report
//   node tools/decisions/corrections.js --check         exit 1 on any rule below
//   node tools/decisions/corrections.js --relock        accept the current citations (rules 3, 5)
//   node tools/decisions/corrections.js --assign TAG    number one placeholder across the whole tree
//
//   --root DIR   run against another tree instead of this checkout. The tests
//                in crates/l2-testkit/tests/corrections_tool.rs build small
//                trees and point the tool at them.
//
// Seven rules, all from things that actually happened:
//
//  1. NO DUPLICATE NUMBER in any numbered series. Agents run concurrently, all
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
//  3. A CITATION WAS NOT DRAGGED BY A RENUMBER. 4. NO PLACEHOLDER REACHES MAIN.
//  5. A CITATION WAS NOT LEFT BEHIND BY ONE. Each is described where it runs.
//
//  6. EVERY PLACEHOLDER FAMILY IS KNOWN. This tool once recognised `CNEW-` and
//     `BNEW-` and nothing else, and a branch wrote a `DNEW-` placeholder for a
//     dead-code entry in docs/bugs.md: it was neither assigned nor reported, and
//     the integrator numbered it by hand. The families are now read off the
//     series the logs actually carry (see SERIES), and a placeholder of a letter
//     no log numbers is reported rather than ignored.
//
//  7. A HEADING THIS TOOL CANNOT READ IS AN ERROR, NOT A SKIP. A line that looks
//     like an entry and is not in the form the log writes — `## CNEW-slug`, a
//     hyphen where the em-dash belongs, an em-dash encoded twice — used to be
//     passed over in silence, so the entry did not exist to any rule here. That
//     is how C146 vanished (its em-dash was double-encoded, C147) and how two
//     branches' `## CNEW-slug` headings reached a merge unseen. An invisible
//     heading is worse than a missing one: the next free number is computed
//     without it, so the next assignment collides with it.
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

const argv = process.argv.slice(2);
const option = name => {
  const i = argv.indexOf(name);
  return i < 0 ? undefined : (argv[i + 1] === undefined || argv[i + 1].startsWith('--') ? '' : argv[i + 1]);
};

const repo = option('--root') ? path.resolve(option('--root')) : path.resolve(__dirname, '..', '..');
const lockPath = path.join(repo, 'tools', 'decisions', 'citations.lock');
const check = argv.includes('--check');
const relock = argv.includes('--relock');
const assignTag = option('--assign');

const fail = m => { console.error('corrections: ' + m); process.exit(1); };

// ---- the numbered series ---------------------------------------------------
//
// Two logs number their entries, and between them they carry SIX series:
//
//   docs/decisions.md   D  decisions            **D1 — title**
//                       C  corrections          **C1 — title**
//   docs/bugs.md        B  §2, reproduced       ### B1 — title   or   | **B10** |
//                       N  §3, not reproduced   (the same two forms)
//                       S  §4, not a bug
//                       D  §5, dead code
//
// **The D-series exists twice, and the two are unrelated.** D5 in decisions.md
// is the licence decision; D5 in bugs.md is an unreachable zoom level. So a
// series is a (log, letter) pair and never a letter alone, and a `D`
// placeholder is numbered in whichever log its entry is written in. Numbering
// by letter would have given the first dead-code placeholder D12, which is the
// next free DECISION.
//
// A retracted bug row wears strikethrough, `| ~~**B11**~~ |`, and is still an
// entry. An id may carry one lower-case suffix: `B11a`, `D5a`.

const SLUG = '[A-Za-z0-9_]+(?:-[A-Za-z0-9_]+)*';
const ID = `[A-Z](?:\\d+[a-z]?|NEW-${SLUG})`;

const LOGS = [
  {
    file: 'docs/decisions.md',
    series: { C: 'correction', D: 'decision' },
    forms: [
      { shape: '**C<n> — title**', re: new RegExp(`^\\*\\*(${ID}) — `) },
    ],
  },
  {
    file: 'docs/bugs.md',
    series: { B: 'reproduced bug', N: 'bug not reproduced', S: 'surprising, not a bug', D: 'dead code' },
    forms: [
      { shape: '### B<n> — title', re: new RegExp(`^### (${ID}) — `) },
      { shape: '| **B<n>** |', re: new RegExp(`^\\| (?:~~)?\\*\\*(${ID})\\*\\*(?:~~)? \\|`) },
    ],
  },
];

// **What "looks like an entry" means** (rule 7). Three prefixes can open one:
// a Markdown heading, bold, and a table row's first cell. After the id:
//
//  * a `#` heading that opens with an id is always an attempt at an entry;
//  * bold or a row cell is one only when a dash, a colon, a cell bar, the start
//    of a mangled dash, or the end of the line follows — because both logs
//    open ordinary prose with a bold id, and those must stay quiet:
//    `**C12's failure mode…`, `**C125 was right…**`, `**B4** the empire tax`,
//    `**D1, the score…`, and the pointer row `| **D1 →** |`;
//  * a placeholder in any of the three positions is always an attempt.
const SUSPECT = new RegExp(
  `^(#{1,6}[ \\t]*(?:\\*\\*|~~|__)*|\\*\\*|\\|[ \\t]*(?:~~)?\\*\\*)[ \\t]*(${ID}|[A-Z]NEW(?![A-Za-z])\\S*)(.*)$`,
);
// U+00E2 and U+00C3 open the mojibake of an em-dash encoded twice or three times.
const DASHLIKE_AFTER_BOLD = /^(?:\*\*|~~)?\s*(?:[-–—:]|â|Ã|$)/;
const DASHLIKE_AFTER_CELL = /^(?:\*\*)?(?:~~)?\s*(?:[-–—:|]|â|Ã|$)/;

function diagnose(log, prefix, rest) {
  const opener = rest.replace(/^(?:\*\*|~~)*\s*/, '');
  if (/â€|Ã¢/.test(rest))
    return 'its em-dash is double-encoded (UTF-8 read as CP1252 and written back) — the bytes should be e2 80 94';
  if (prefix.startsWith('#') && !log.forms.some(f => f.shape.startsWith('#')))
    return `a Markdown heading; ${log.file} does not write entries as headings`;
  if (prefix.startsWith('#') && prefix.replace(/[^#]/g, '').length !== 3)
    return `a level-${prefix.replace(/[^#]/g, '').length} heading; entries are level 3`;
  if (/[*_~]/.test(prefix.replace(/^#+/, '').replace(/^\|/, '')) && prefix.startsWith('#'))
    return 'a heading wrapped in emphasis; the id opens the heading bare';
  if (opener.startsWith('–')) return 'an en-dash where the em-dash belongs';
  if (opener.startsWith('-')) return 'a hyphen where the em-dash belongs';
  if (opener.startsWith(':')) return 'a colon where the em-dash belongs';
  if (opener.startsWith('—')) return 'the em-dash is there but the spacing or emphasis around it is not';
  if (opener === '' || opener.startsWith('|')) return 'nothing after the id — no " — title", or a cell not closed the way the log closes it';
  return 'not in any form this log writes';
}

function readLogs() {
  const defs = [];          // {file, line, id, letter, title}
  const malformed = [];     // {file, line, text, why}
  for (const log of LOGS) {
    const abs = path.join(repo, log.file);
    if (!fs.existsSync(abs)) fail(`${log.file} is missing, and it is one of the two logs this tool numbers`);
    const lines = fs.readFileSync(abs, 'utf8').split(/\r?\n/);
    lines.forEach((L, i) => {
      for (const form of log.forms) {
        const m = form.re.exec(L);
        if (!m) continue;
        const letter = m[1][0];
        if (!(letter in log.series)) {
          malformed.push({
            file: log.file, line: i + 1, text: L,
            why: `${letter} is not a series ${log.file} numbers (it numbers ${Object.keys(log.series).join(', ')})`,
          });
        } else {
          defs.push({ file: log.file, line: i + 1, id: m[1], letter, title: L.slice(m[0].length) });
        }
        return;
      }
      const s = SUSPECT.exec(L);
      if (!s) return;
      const [, prefix, id, rest] = s;
      const placeholder = /^[A-Z]NEW/.test(id);
      const looks = prefix.startsWith('#') || placeholder
        || (prefix.startsWith('|') ? DASHLIKE_AFTER_CELL : DASHLIKE_AFTER_BOLD).test(rest);
      if (!looks) return;
      malformed.push({ file: log.file, line: i + 1, text: L, why: diagnose(log, prefix, rest) });
    });
  }
  return { defs, malformed };
}

// ---- the tree ----------------------------------------------------------------

const SKIP_DIR = new Set(['.git', 'target', 'node_modules', 'decomp', 'out', '.claude']);
// Citations are looked for in these, and the lockfile's contents depend on the
// list, so it does not grow casually.
const EXT = /\.(rs|md|json|js|ps1|toml|html|java|yml|yaml)$/;
// Placeholders are looked for — and replaced — in every text type the tree
// tracks. A placeholder in a file this scan skips is one `--assign` would leave
// behind and `--check` would never report.
const PLACEHOLDER_EXT = /\.(rs|md|json|js|ps1|toml|html|java|yml|yaml|txt|lock|cpp|h|def|sh|cmd|py|css|csv)$/;

// Files that have to be able to *describe* the convention.
const PLACEHOLDER_EXEMPT = new Set(['tools/decisions/corrections.js', 'docs/agents.md']);

// Words that make a reference deliberately historical. See the header.
const HISTORICAL = /\b(frozen|historical|superseded|withdrawn|at the time|used to be|renumber)/i;

const PLACEHOLDER = new RegExp(`\\b([A-Z])NEW-(${SLUG})`, 'g');

function analyse() {
  const { defs, malformed } = readLogs();

  // Rule 1, for every series: a duplicate id is an error wherever it is.
  const byId = new Map();
  for (const d of defs) {
    const k = d.file + '\t' + d.id;
    if (!byId.has(k)) byId.set(k, []);
    byId.get(k).push(d);
  }
  const dupes = [...byId.values()].filter(v => v.length > 1);

  // Each series, and its next free number.
  const series = new Map();    // "file\tletter" -> {file, letter, name, max, numbers}
  for (const log of LOGS)
    for (const [letter, name] of Object.entries(log.series))
      series.set(log.file + '\t' + letter, { file: log.file, letter, name, max: 0, count: 0 });
  for (const d of defs) {
    const m = /^[A-Z](\d+)/.exec(d.id);
    if (!m) continue;
    const s = series.get(d.file + '\t' + d.letter);
    s.max = Math.max(s.max, Number(m[1]));
    s.count++;
  }

  // The corrections themselves, which every citation rule is about.
  const headings = new Map();          // number -> [{line, text}]
  for (const d of defs) {
    if (d.file !== 'docs/decisions.md' || !/^C\d+$/.test(d.id)) continue;
    const n = Number(d.id.slice(1));
    if (!headings.has(n)) headings.set(n, []);
    headings.get(n).push({ line: d.line, text: d.title.replace(/\*\*$/, '').trim() });
  }

  // **What each number currently NAMES**, hashed from the heading's own words.
  // Rule 5 compares a citation against this rather than against the number, which
  // is the difference between "does it resolve?" and "does it resolve to the
  // thing it meant?".
  const headingFp = new Map();
  for (const [n, hs] of headings) {
    const text = hs[0].text.replace(/\s+/g, ' ').trim().toLowerCase();
    headingFp.set(n, crypto.createHash('sha1').update(text).digest('hex').slice(0, 12));
  }

  const numbers = [...headings.keys()].sort((a, b) => a - b);
  const nextFree = numbers.length ? Math.max(...numbers) + 1 : 1;

  // A gap is not an error — a withdrawn correction leaves one — but say so.
  const gaps = [];
  for (let n = 1; n < nextFree; n++) if (!headings.has(n)) gaps.push('C' + n);

  // ---- rule 2: citations anywhere in the tree ------------------------------

  const citations = [];
  const placeholders = [];
  (function walk(dir) {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      if (SKIP_DIR.has(e.name)) continue;
      const p = path.join(dir, e.name);
      if (e.isDirectory()) { walk(p); continue; }
      const cites = EXT.test(e.name);
      if (!cites && !PLACEHOLDER_EXT.test(e.name)) continue;
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

      // ---- rule 4: no unassigned placeholder may reach `main` --------------
      //
      // An agent writing on a branch cannot know which number its correction will
      // get: C61 had SIX claimants in one day, from six branches that all
      // correctly read C60 as the highest. So a branch writes `CNEW-<slug>` (or
      // `BNEW-<slug>`, `DNEW-<slug>`, … for the other series) and the integrator
      // assigns a number at merge — which is the one moment serialisation is free.
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
      // Skipped inside PLACEHOLDER_EXEMPT, which have to be able to *describe*
      // the convention.
      if (!PLACEHOLDER_EXEMPT.has(rel)) {
        for (const m of s.matchAll(PLACEHOLDER)) {
          const line = s.slice(0, m.index).split('\n').length;
          placeholders.push({ file: rel, line, tag: m[0], letter: m[1] });
        }
      }
      if (!cites) continue;
      for (const m of s.matchAll(/\bC(\d{1,3})\b/g)) {
        const n = Number(m[1]);
        const win = s.slice(Math.max(0, m.index - 220), m.index + 220);
        const isCite = isLog || /decisions\.md/.test(win) || /\bcorrections?\b/i.test(win);
        if (!isCite) continue;
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

  return { defs, malformed, dupes, series, headings, headingFp, numbers, nextFree, gaps, citations, placeholders };
}

// ---- rule 7: every entry-shaped line is an entry ------------------------------

function reportMalformed(A) {
  if (!A.malformed.length) return;
  console.error(`corrections: ${A.malformed.length} line(s) look like a numbered entry and are not in a form this tool reads.\n`);
  for (const m of A.malformed) {
    console.error(`  ${m.file}:${m.line}  ${m.why}`);
    console.error(`      ${m.text.slice(0, 100)}`);
  }
  console.error('\nThe forms each log writes:');
  for (const log of LOGS)
    console.error(`  ${log.file.padEnd(18)} ${log.forms.map(f => f.shape).join('   or   ')}`);
  console.error(
    '\nAn entry this tool cannot read does not exist to any rule it enforces: its\n' +
    'citations dangle, its number is free to be taken again, and a placeholder\n' +
    'on it is never assigned. Rewrite the line in the form above — with a real\n' +
    'em-dash, e2 80 94, if the complaint is its encoding. If the line is prose\n' +
    'that happens to open with an id, reword its opening.',
  );
  process.exit(1);
}

function reportDupes(A) {
  if (!A.dupes.length) return;
  console.error(`corrections: ${A.dupes.length} entry id(s) used more than once.\n`);
  for (const ds of A.dupes) {
    const s = A.series.get(ds[0].file + '\t' + ds[0].letter);
    console.error(`  ${ds[0].id} (${s.name}) is used ${ds.length} times:`);
    for (const d of ds)
      console.error(`    ${d.file}:${d.line}  ${d.title.slice(0, 84)}`);
    if (!/NEW-/.test(ds[0].id)) console.error(`    the next free number in that series is ${s.letter}${s.max + 1}`);
  }
  console.error('\nRenumber the LATER arrival: change its heading, then grep the tree for');
  console.error('citations of the old number and move them with it —');
  console.error(`    grep -rn "${A.dupes[0][0].id}" docs crates tools README.md`);
  console.error('A heading that moved without its citations is worse than the collision.');
  console.error('A placeholder used twice is two entries sharing a slug: rename one.');
  process.exit(1);
}

function reportDangling(A) {
  const dangling = A.citations.filter(c => !A.headings.has(c.n) && !c.historical);
  if (!dangling.length) return;
  console.error(`corrections: ${dangling.length} citation(s) point at a correction that does not exist.\n`);
  for (const c of dangling)
    console.error(`  ${c.rel}:${c.line}  cites C${c.n}${A.gaps.includes('C' + c.n) ? ' (a gap in the log)' : ''}`);
  console.error(`\nThe log runs C1..C${Math.max(...A.numbers)}${A.gaps.length ? `, missing ${A.gaps.join(', ')}` : ''}.`);
  console.error('Either the correction was never written — write it, as a placeholder on a');
  console.error(`branch or taking the next free number, C${A.nextFree}, at merge — or the citation`);
  console.error('is stale after a renumber, in which case point it at the number the');
  console.error('correction actually has now.');
  console.error('If the reference is deliberately historical, say so in the surrounding text');
  console.error('("frozen", "superseded", "used to be") and this check will leave it alone.');
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

const lockLine = (A, c) =>
  `${c.rel}\tC${c.n}\t${c.fp}\t${A.headingFp.get(c.n) || '-'}`;
const LOCK_HEADER = [
  '# Citation fingerprints — see tools/decisions/corrections.js, rules 3 and 5.',
  '#',
  '# One line per correction citation, with FOUR fields:',
  '#',
  '#   file, the number it cites, a hash of the words around it with every',
  '#   C-number blanked, and a hash of the HEADING that number currently names.',
  '#',
  '# The third field catches a citation DRAGGED by a renumber: its number moved',
  '# and its words did not (rule 3). The fourth catches the opposite and rarer',
  '# case, a citation LEFT BEHIND by one: nothing about the citation changed at',
  '# all, but the entry sitting on that number is now a different entry (rule 5).',
  '# Both still resolve, so rule 2 passes on either.',
  '#',
  '# Regenerate with:  node tools/decisions/corrections.js --relock',
  '# Generated. Do not hand-edit except to accept a single deliberate change.',
  '',
].join('\n');

function readLock() {
  if (!fs.existsSync(lockPath)) return null;
  const lines = fs.readFileSync(lockPath, 'utf8').split(/\r?\n/).filter(l => l && !l.startsWith('#'));
  const number = new Map();
  const heading = new Map();
  for (const l of lines) {
    const [rel, n, fp, hfp] = l.split('\t');
    number.set(rel + '\t' + fp, n);
    heading.set(rel + '\t' + fp, hfp || '-');
  }
  return { lines, number, heading };
}

function findDragged(A, lock) {
  const dragged = [];
  for (const c of A.citations) {
    const key = c.rel + '\t' + c.fp;
    if (lock.number.has(key) && lock.number.get(key) !== 'C' + c.n)
      dragged.push({ ...c, was: lock.number.get(key) });
  }
  return dragged;
}

// ---- rule 5: a citation left behind by a renumber ----------------------
//
// Rule 3 catches the citation a renumber DRAGGED. This catches the one it
// LEFT: the file, the number and the words are all untouched, and the entry
// that number names is now somebody else's.
//
// It has happened. A `C61 -> C63` renumber during a merge left `campaign.rs`
// and `village.rs` still saying `C61`, which on `main` is now a different
// correction entirely — and this tool reported "all citations resolve"
// throughout, because they do. The lint checked that a citation resolves, not
// that it resolves to the thing it meant.
//
// The fix is cheap and it is the fourth field of the lockfile: remember which
// HEADING each citation was pointing at, and object when the heading under it
// changes. It also argues for a habit — **assign placeholders before
// renumbering anything, not during**, because the two operations interleave
// badly and this check cannot tell you which of them was the mistake.
function findBehind(A, lock) {
  const behind = [];
  for (const c of A.citations) {
    const key = c.rel + '\t' + c.fp;
    const was = lock.heading.get(key);
    if (!was || was === '-') continue;
    if (lock.number.get(key) !== 'C' + c.n) continue;   // rule 3's case, already reported
    const now = A.headingFp.get(c.n);
    if (now && now !== was) behind.push({ ...c, wasFp: was, nowFp: now });
  }
  return behind;
}

function writeLock(A) {
  // Show any drag being recorded rather than swallowing it: --relock is how a
  // deliberate correction is accepted, so it must not be a silent way past a
  // real one.
  const lock = readLock();
  if (lock) {
    const moved = findDragged(A, lock);
    if (moved.length) {
      console.log(`corrections: recording ${moved.length} citation(s) whose number changed`);
      console.log('  while the words around them did not. If any of these was not deliberate,');
      console.log('  it is a dragged citation — undo it rather than keeping this lockfile.\n');
      for (const c of moved)
        console.log(`  ${c.rel}:${c.line}  ${c.was} -> C${c.n}`);
      console.log('');
    }
  }
  const current = A.citations.map(c => lockLine(A, c)).sort();
  fs.mkdirSync(path.dirname(lockPath), { recursive: true });
  fs.writeFileSync(lockPath, LOCK_HEADER + current.join('\n') + '\n');
  console.log(`corrections: relocked ${current.length} citations`);
}

function checkLock(A) {
  const lock = readLock();
  if (!lock) {
    fail('tools/decisions/citations.lock is missing.\n'
      + '  Create it with:  node tools/decisions/corrections.js --relock');
  }

  const dragged = findDragged(A, lock);
  if (dragged.length) {
    console.error(`corrections: ${dragged.length} citation(s) changed number while their surrounding words did not.\n`);
    console.error('  **This looks like a dragged citation** — a renumber that took an unrelated');
    console.error('  reference with it. Such a citation still resolves, so rule 2 passes and the');
    console.error('  reference now silently points at somebody else\'s correction.\n');
    for (const d of dragged) {
      console.error(`  ${d.rel}:${d.line}   ${d.was} -> C${d.n}`);
      console.error(`      was: ${d.rel}\t${d.was}\t${d.fp}`);
      console.error(`      now: ${lockLine(A, d)}`);
    }
    console.error('\n  Read each one and decide which correction it MEANS.');
    console.error('  If the old number was right, put it back.');
    console.error('  If this is a deliberate correction of a citation that was wrong, accept it:');
    console.error('      node tools/decisions/corrections.js --relock');
    console.error('  (or edit the line above in tools/decisions/citations.lock by hand).');
    process.exit(1);
  }

  const behind = findBehind(A, lock);
  if (behind.length) {
    console.error(
      `corrections: ${behind.length} citation(s) still name C-numbers whose ENTRY has changed.\n`,
    );
    console.error('  **This looks like a citation left behind by a renumber.** Nothing about');
    console.error('  the citation moved — same file, same number, same words — but the');
    console.error('  correction sitting on that number is not the one it was written against.');
    console.error('  It still resolves, which is why rule 2 is silent.\n');
    for (const b of behind) {
      const h = A.headings.get(b.n);
      console.error(`  ${b.rel}:${b.line}  cites C${b.n}, which now reads:`);
      console.error(`      ${(h ? h[0].text : '(missing)').slice(0, 90)}`);
    }
    console.error('\n  Read each one and point it at the number its correction actually has now.');
    console.error('  If the entry was legitimately rewritten in place and the citation is still');
    console.error('  right, accept it:');
    console.error('      node tools/decisions/corrections.js --relock');
    process.exit(1);
  }

  const current = A.citations.map(c => lockLine(A, c)).sort();
  const lockedSet = new Set(lock.lines);
  const currentSet = new Set(current);
  const added = current.filter(l => !lockedSet.has(l));
  const gone = lock.lines.filter(l => !currentSet.has(l));
  if (added.length || gone.length) {
    console.error('corrections: tools/decisions/citations.lock is out of date '
      + `(${added.length} added, ${gone.length} removed).`);
    console.error('  No dragged citation was found — this is the ordinary case of citations');
    console.error('  being written, moved or reworded. A stale lockfile protects nothing, so:');
    console.error('      node tools/decisions/corrections.js --relock');
    process.exit(1);
  }
}

// ---- rule 4, reported: the placeholders still in the tree ------------------
//
// Reported LAST, after every other rule has passed, so that a branch — which
// carries its own placeholders by design — still learns whether anything else is
// wrong. On a branch this is the expected final failure; on `main` it is fatal.

function placeholderPlan(A) {
  // One row per distinct tag, in the order a person should assign them: the
  // logs' own order first, then anything no entry defines.
  const next = new Map([...A.series].map(([k, s]) => [k, s.max]));
  const rows = [];
  const seen = new Set();
  for (const d of A.defs) {
    if (!/^[A-Z]NEW-/.test(d.id) || seen.has(d.id)) continue;
    seen.add(d.id);
    const key = d.file + '\t' + d.letter;
    next.set(key, next.get(key) + 1);
    rows.push({ tag: d.id, def: d, series: A.series.get(key), becomes: d.letter + next.get(key) });
  }
  for (const p of A.placeholders) {
    if (seen.has(p.tag)) continue;
    seen.add(p.tag);
    rows.push({ tag: p.tag, def: null, letter: p.letter });
  }
  for (const r of rows) r.uses = A.placeholders.filter(p => p.tag === r.tag);
  return rows;
}

function reportPlaceholders(A) {
  const rows = placeholderPlan(A);
  if (!rows.length) return;
  console.error(`corrections: ${rows.length} unassigned placeholder(s) in the tree.\n`);
  for (const r of rows) {
    const files = [...new Set(r.uses.map(u => u.file))];
    const where = files.length ? `used in ${files.length} file(s): ${files.slice(0, 5).join(', ')}${files.length > 5 ? ', …' : ''}` : 'used nowhere else';
    if (r.def) {
      console.error(`  ${r.tag}  — a ${r.series.name}, ${r.def.file}:${r.def.line}; ${where}`);
      console.error(`      node tools/decisions/corrections.js --assign ${r.tag}      (it would take ${r.becomes})`);
    } else {
      const homes = LOGS.filter(l => r.letter in l.series).map(l => `${l.file} (${l.series[r.letter]})`);
      console.error(`  ${r.tag}  — ${where}`);
      console.error(homes.length
        ? `      no entry defines it. A ${r.letter}-placeholder is an entry in ${homes.join(' or ')}; write the entry, or this is a stale citation.`
        : `      ${r.letter} is not a series either log numbers (${LOGS.map(l => Object.keys(l.series).join('')).join(', ')}), so nothing can assign it.`);
    }
  }
  console.error(
    `\nA placeholder is how a BRANCH writes an entry whose number it cannot know:\n` +
    `several branches routinely pick the same next number. Every other rule passed.\n` +
    `On a branch that is the expected state and this is the last thing standing;\n` +
    `on main it is fatal. The integrator assigns them at merge, one command per\n` +
    `tag, in the order listed — each takes the next free number in its own series,\n` +
    `rewrites the tag byte-for-byte across the tree, and relocks the citations.`,
  );
  process.exit(1);
}

// ---- --assign: the one moment numbering is serial ----------------------------
//
// It used to be done with `git grep -l | perl -pi`, and a tool without an
// encoding layer is exactly what double-encoded C146's em-dash. So this reads
// and writes BYTES: a tag is ASCII, so it can be found and replaced in a Buffer
// without decoding anything else in the file, and a byte this tool does not
// understand — a stray CP1252 byte, a BOM, a CRLF — goes back out as it came in.

const TAG = new RegExp(`^([A-Z])NEW-${SLUG}$`);

const isWordByte = b => (b >= 0x30 && b <= 0x39) || (b >= 0x41 && b <= 0x5a) || (b >= 0x61 && b <= 0x7a) || b === 0x5f;

// The same boundaries as PLACEHOLDER: not preceded by a word character, and not
// followed by one or by a hyphen that continues the slug. So `CNEW-foo` is not
// found inside `CNEW-foo-bar` or `XCNEW-foo`.
function replaceBytes(buf, tag, id) {
  const needle = Buffer.from(tag, 'ascii');
  const rep = Buffer.from(id, 'ascii');
  const parts = [];
  let from = 0, count = 0, i = 0;
  while ((i = buf.indexOf(needle, i)) !== -1) {
    const j = i + needle.length;
    const before = i > 0 && isWordByte(buf[i - 1]);
    const after = j < buf.length && (isWordByte(buf[j]) || (buf[j] === 0x2d && j + 1 < buf.length && isWordByte(buf[j + 1])));
    if (before || after) { i += 1; continue; }
    parts.push(buf.subarray(from, i), rep);
    from = i = j;
    count++;
  }
  parts.push(buf.subarray(from));
  return { out: Buffer.concat(parts), count };
}

function refuse(m) {
  console.error('corrections: --assign refused, and nothing was changed.\n  ' + m.split('\n').join('\n  '));
  process.exit(1);
}

function assign(A, tag) {
  if (!tag) refuse('--assign needs the placeholder to number, e.g.\n    node tools/decisions/corrections.js --assign CNEW-hover');
  if (!TAG.test(tag)) refuse(`"${tag}" is not a placeholder. A placeholder is a series letter, NEW-, and a slug: CNEW-hover, BNEW-forecast.`);

  const uses = A.placeholders.filter(p => p.tag === tag);
  const defs = A.defs.filter(d => d.id === tag);
  if (!uses.length && !defs.length) {
    const present = [...new Set(A.placeholders.map(p => p.tag))];
    refuse(`${tag} is not in the tree.` + (present.length ? `\nThe placeholders that are: ${present.join(', ')}` : '\nThere are no placeholders in the tree.'));
  }
  if (!defs.length) {
    refuse(`${tag} is used ${uses.length} time(s) — first at ${uses[0].file}:${uses[0].line} — and no entry defines it,\n`
      + 'so there is no telling which log and series it belongs to. Write the entry first.');
  }
  const def = defs[0];                       // rule 1 has already refused two
  const s = A.series.get(def.file + '\t' + def.letter);
  const id = def.letter + (s.max + 1);

  // --assign relocks, and a relock is how a drag is accepted. So a drag already
  // in the tree has to be looked at by a person first, not swept into the lock.
  const lock = readLock();
  if (lock) {
    const dragged = findDragged(A, lock);
    const behind = findBehind(A, lock);
    if (dragged.length || behind.length) {
      refuse(`the tree already has ${dragged.length} dragged and ${behind.length} left-behind citation(s), and --assign\n`
        + 'would relock over them. Run --check, resolve them, then assign.');
    }
  }

  // Plan every write before making any, and check the byte replacement found
  // exactly what the scan found. A disagreement means one of the two is wrong,
  // and a half-applied rename is worse than none.
  const perFile = new Map();
  for (const u of uses) perFile.set(u.file, (perFile.get(u.file) || 0) + 1);
  const writes = [];
  for (const [file, expected] of perFile) {
    const abs = path.join(repo, file);
    const { out, count } = replaceBytes(fs.readFileSync(abs), tag, id);
    if (count !== expected)
      refuse(`${file}: the scan found ${tag} ${expected} time(s) and the byte replacement ${count}. Fix this tool before trusting it.`);
    writes.push({ abs, out });
  }
  for (const w of writes) fs.writeFileSync(w.abs, w.out);

  const B = analyse();
  const problems = [];
  if (B.placeholders.some(p => p.tag === tag)) problems.push(`${tag} is still in the tree`);
  if (!B.defs.some(d => d.file === def.file && d.id === id)) problems.push(`${def.file} has no ${id} entry afterwards`);
  if (B.malformed.length || B.dupes.length) problems.push('the logs no longer parse cleanly');
  if (problems.length) fail(`--assign wrote ${writes.length} file(s) and then found: ${problems.join('; ')}. Inspect with git diff.`);

  console.log(`corrections: ${tag} -> ${id}, the next free ${s.name} number in ${def.file}: `
    + `${uses.length} occurrence(s) in ${writes.length} file(s)`);
  for (const [file, n] of perFile) console.log(`  ${file}  ×${n}`);
  writeLock(B);
  const left = placeholderPlan(B);
  if (left.length) console.log(`  ${left.length} placeholder(s) still to assign: ${left.map(r => r.tag).join(', ')}`);
  process.exit(0);
}

// ---- main --------------------------------------------------------------------

const A = analyse();

reportMalformed(A);
if (A.defs.filter(d => d.file === 'docs/decisions.md' && d.letter === 'C').length === 0)
  fail('found no **C<n> — ...** headings in docs/decisions.md at all');
reportDupes(A);
reportDangling(A);

if (assignTag !== undefined) assign(A, assignTag);

if (relock) {
  writeLock(A);
  const left = placeholderPlan(A);
  if (left.length) console.log(`  (${left.length} unassigned placeholder(s) in the tree — --check will still report them)`);
  process.exit(0);
}

checkLock(A);
reportPlaceholders(A);

// ---- report ----------------------------------------------------------------

const cited = new Set(A.citations.map(c => c.n));
const uncited = A.numbers.filter(n => !cited.has(n));
const hist = A.citations.filter(c => c.historical).length;

const summary = `${A.headings.size} corrections (C1..C${Math.max(...A.numbers)}), `
  + `${A.citations.length} citations, all resolve`;

if (check) {
  console.log('corrections: ' + summary);
} else {
  console.log(summary);
  if (A.gaps.length) console.log(`  gaps (withdrawn or never written): ${A.gaps.join(', ')}`);
  if (hist) console.log(`  ${hist} citation(s) exempt as deliberately historical`);
  console.log(`  cited nowhere outside the log: ${uncited.map(n => 'C' + n).join(' ') || '(none)'}`);
  console.log(`  next free number: C${A.nextFree}`);
  const others = [...A.series.values()].filter(s => !(s.file === 'docs/decisions.md' && s.letter === 'C'));
  console.log('  and in the other series: ' + others.map(s => `${s.letter}${s.max + 1} (${s.name}, ${s.count} entries)`).join(', '));
}
