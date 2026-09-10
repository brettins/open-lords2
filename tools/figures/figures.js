// The figures README.md and docs/status.html quote about the tree, generated.
//
//   node tools/figures/figures.js            rewrite the marked figures in place
//   node tools/figures/figures.js --check    exit 1 if any quoted figure is stale
//
// Why this exists. A document that states a measurement nothing recomputes is a
// test that cannot fail. README said "958 tests" when there were 1,221;
// status.html said "230 functions named" when symbols.json held 465 and "542
// tests" against the same 1,221. Nobody was careless - there is simply no
// mechanism, and four agents appending to symbols.json in an afternoon move the
// counts faster than anyone edits prose.
//
// This follows tools/symbols/symbols_md.js: one source of truth, a generated
// region, and a --check that fails loudly in CI.
//
// The markers are HTML comments, so they are invisible in rendered Markdown and
// in the browser:
//
//     <!--fig:tests-->1,221<!--/fig-->
//
// A figure inside a fenced code block cannot be marked this way - the comment
// would be shown literally - so code samples must not quote counts. If you want
// one there, write the command, not its output.
//
// NOT EVERY NUMBER IN THE DOCS BELONGS HERE. A frozen measurement - "932
// passing tests" is the moment C26 describes, plan-review.md's 727-vs-711 is a
// dated review whose subject is the discrepancy - records a past state and must
// keep its old value. Marking one of those would destroy what it records. Only
// present-tense claims about the tree as it stands go in here.

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const repo = path.resolve(__dirname, '..', '..');
const check = process.argv.includes('--check');

const fail = m => { console.error('figures: ' + m); process.exit(1); };

// The function count of Lords2.exe itself. Not derived from anything in the
// tree because it is a property of the shipped binary, not of our work:
// DecompileAll reports "2452 decompiled, 0 failed" on every run. It moves only
// if the game does.
const BINARY_FUNCTIONS = 2452;

const group = n => n.toLocaleString('en-US');

// ---- sources of truth ------------------------------------------------------

function fromSymbols() {
  const p = path.join(repo, 'docs', 'symbols.json');
  const d = JSON.parse(fs.readFileSync(p, 'utf8'));
  return { functions: (d.functions || []).length, globals: (d.globals || []).length };
}

// The count of install-gated tests is deliberately NOT a figure here.
// crates/l2-testkit/tests/census.rs owns it, checks it against its own
// inventory, and prints it on every run; docs/environment.md used to restate it
// and now points at the census instead. A number with one home does not need a
// generator.

// The suite is measured by running it, not by counting `#[test]` in the source.
// The two differ - doc-tests, `#[ignore]`, tests behind a cfg - and the claim
// these documents make is "this many pass", which is a claim about a run.
//
// Deliberately scrubbed of LORDS2_DIR and LORDS2_FIXTURES: the figure quoted is
// what a reader with no copy of the game gets, which is also what CI gets.
function fromCargo() {
  const env = { ...process.env };
  delete env.LORDS2_DIR;
  delete env.LORDS2_FIXTURES;

  const run = args => {
    let out;
    try {
      out = execFileSync('cargo', args, { cwd: repo, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 64 << 20 });
    } catch (e) {
      // A failing suite still prints its results; a missing cargo does not.
      if (e.stdout === undefined) fail(`could not run "cargo ${args.join(' ')}": ${e.message}`);
      out = e.stdout;
    }
    let passed = 0, failed = 0, seen = 0;
    for (const m of out.matchAll(/^test result: \w+\. (\d+) passed; (\d+) failed/gm)) {
      passed += Number(m[1]); failed += Number(m[2]); seen++;
    }
    if (!seen) fail(`"cargo ${args.join(' ')}" produced no test results`);
    return { passed, failed };
  };

  const all = run(['test', '--workspace']);
  if (all.failed) fail(`the suite is red (${all.failed} failing) - fix that before regenerating figures`);
  const kingdom = run(['test', '-p', 'l2-kingdom']);
  return { tests: all.passed, testsKingdom: kingdom.passed };
}

// ---- the figures -----------------------------------------------------------

// **The input-arm inventory, which is the 1:1 measurement.**
//
// `docs/plan.md` revision 5 leads with this number, so it is generated rather
// than typed — the same rule the rest of this file exists for, and the same
// reason: an earlier brief quoted 61/32/10 for the quirk counts when the live
// numbers were 67/34/12, and a stale headline is worse than an absent one.
//
// The denominator is deliberately **live arms**: `dead` records are in the
// binary and cannot run, so counting them would inflate the gap with work no
// player could ever see. `invention` is not in the denominator either — it is
// ours, and it is reported separately because it is the other half of 1:1 and
// the half nobody was counting.
function fromArms() {
  const j = JSON.parse(fs.readFileSync(path.join(repo, 'docs', 'arms.json'), 'utf8'));
  const by = k => j.arms.filter(a => a.status === k).length;
  const reproduced = by('reproduced');
  const missing = by('missing');
  const live = reproduced + missing;
  const groups = Object.values(j.groups || {});
  return {
    reproduced,
    missing,
    dead: by('dead'),
    inventions: by('invention'),
    live,
    pct: live ? Math.round((reproduced / live) * 100) : 0,
    groups: groups.length,
    groupsDone: groups.filter(g => g && g.complete).length,
  };
}

// How many screens are still shells — the honest measure of how much of the
// interface is a placeholder, and one a player can feel: he described seven of
// them as "placeholder everywhere" without being told which were which.
function fromShells() {
  const src = fs.readFileSync(
    path.join(repo, 'crates', 'l2-game', 'src', 'screens', 'shells.rs'),
    'utf8',
  );
  const at = src.indexOf('SHELLS');
  const body = at < 0 ? src : src.slice(at);
  return (body.match(/^\s{4}Shell \{$/gm) || []).length;
}

// **The campaign map's draw calls** -- the instrument for `docs/plan.md` §0's
// third falsification row, which read "instrument: none" from the day it was
// written until this audit gave it one.
//
// Shelled out to rather than reimplemented: `mapdraws.js` holds the listing AND
// cross-checks it against the area table in `docs/draws-map.md`, and a second
// copy of that arithmetic here would be the duplicate-rule failure this project
// has logged twice.
function fromMapDraws() {
  const out = execFileSync(process.execPath, [
    path.join(repo, 'tools', 'draws', 'mapdraws.js'), '--figures',
  ], { encoding: 'utf8' });
  const last = out.trim().split(String.fromCharCode(10)).pop().trim();
  return JSON.parse(last);
}

// The draw-call audit — `docs/draws.md`, and the instrument `docs/plan.md` §0's
// third falsification condition did not have.
//
// **Two halves, and they are measured differently on purpose.**
//
// The *original's* count cannot be recomputed here: the decompiled corpus is
// gitignored and CI has none. So it is stored in `tools/draws/screens.json` by
// `node tools/draws/screendraws.js --write`, whose `--check` recomputes it
// against the corpus for anybody who has one and skips loudly for anybody who
// does not. This file's job is only that no document quotes a number
// `screens.json` does not hold.
//
// *Our* half needs nothing but `crates/`, so it is recomputed here every run —
// including the split that the draw count cannot see: **how many of our marks
// go through the game's own artwork, and how many are our 5 x 7 debug font and
// our own rectangles.** A screen can reproduce every draw call and still be
// entirely placeholder, which is what a player meant by *"placeholder shit
// everywhere"* on screens whose counts were fine.
function fromScreenDraws() {
  const p = path.join(repo, 'tools', 'draws', 'screens.json');
  const inv = JSON.parse(fs.readFileSync(p, 'utf8'));
  const original = inv.reduce((n, r) => n + (r.original || 0), 0);
  const missing = inv.reduce((n, r) => n + (r.missing || []).length, 0);
  const invented = inv.reduce((n, r) => n + (r.invented || []).length, 0);

  const { ourSites, ourKinds } = require(path.join(repo, 'tools', 'draws', 'screendraws.js'));
  // A module can serve two screens (`0x35`/`0x36` are one painter and one
  // flag), and its marks are a property of the file. Count each file once.
  const modules = [...new Set(inv.map(r => r.module).filter(Boolean))];
  let ours = 0, real = 0, placeholder = 0, literals = 0;
  for (const m of modules) {
    ours += (ourSites(m) || []).length;
    const k = ourKinds(m);
    if (k) { real += k.real; placeholder += k.placeholder; literals += k.literals.length; }
  }
  return {
    screens: inv.length,
    original,
    ours,
    missing,
    invented,
    real,
    placeholder,
    literals,
    pct: original ? Math.round((Math.min(ours, original) / original) * 100) : 0,
    realPct: real + placeholder ? Math.round((real / (real + placeholder)) * 100) : 0,
  };
}

function figures() {
  const sym = fromSymbols();
  const cargo = fromCargo();
  const arms = fromArms();
  const mapDraws = fromMapDraws();
  const draws = fromScreenDraws();
  const shells = fromShells();
  const pct = Math.round((sym.functions / BINARY_FUNCTIONS) * 100);
  return {
    tests: group(cargo.tests),
    'tests-l2-kingdom': group(cargo.testsKingdom),
    functions: group(sym.functions),
    globals: group(sym.globals),
    'functions-pct': String(pct),
    'binary-functions': group(BINARY_FUNCTIONS),
    'arms-reproduced': group(arms.reproduced),
    'arms-missing': group(arms.missing),
    'arms-live': group(arms.live),
    'arms-dead': group(arms.dead),
    'arms-inventions': group(arms.inventions),
    'arms-pct': String(arms.pct),
    'arms-groups': group(arms.groups),
    'arms-groups-done': group(arms.groupsDone),
    shells: group(shells),
    'map-draws': group(mapDraws.total),
    'map-draws-live': group(mapDraws.live),
    'map-draws-ours': group(mapDraws.ours),
    'map-draws-pct': String(mapDraws.pct),
    'draws-screens': group(draws.screens),
    'draws-original': group(draws.original),
    'draws-ours': group(draws.ours),
    'draws-missing': group(draws.missing),
    'draws-inventions': group(draws.invented),
    'draws-pct': String(draws.pct),
    'draws-real': group(draws.real),
    'draws-placeholder': group(draws.placeholder),
    'draws-real-pct': String(draws.realPct),
    'draws-literals': group(draws.literals),
    // A date that does not move while the content does is worse than no date,
    // so the stamp is regenerated with everything else. Spelled out rather than
    // taken from toLocaleDateString, which gives "Sept" on some ICU versions
    // and "Sep" on others - a figure that changes with the Node build would
    // defeat the point of checking it.
    updated: (() => {
      const d = new Date();
      const mon = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
      return `${d.getDate()} ${mon[d.getMonth()]} ${d.getFullYear()}`;
    })(),
  };
}

// ---- rewriting -------------------------------------------------------------

const TARGETS = [
  'README.md',
  path.join('docs', 'status.html'),
  path.join('docs', 'method.md'),
  // docs/plan.md quotes the test count and the naming coverage as *present-tense*
  // claims about the tree, so they belong here. Everything else in that file is a
  // frozen measurement and says so - plan-review.md's numbers especially, which
  // are the evidence for a dated review and must never be rewritten.
  path.join('docs', 'plan.md'),
];
const MARKER = /<!--fig:([a-z0-9-]+)-->([\s\S]*?)<!--\/fig-->/g;

const values = figures();
let stale = [];
let counted = 0;
const used = new Set();

// An HTML comment inside an attribute value is not a comment - it is literal
// text - so status.html's progress bar cannot carry a marker. It gets the one
// special case in this file: the width of `id="fn-bar"` is the same percentage
// the tile prints beside it, and is rewritten and checked the same way.
const BAR = /(<i id="fn-bar" style="width:)(\d+)(%")/;

for (const rel of TARGETS) {
  const p = path.join(repo, rel);
  const before = fs.readFileSync(p, 'utf8');
  let after = before.replace(MARKER, (whole, name, current) => {
    counted++;
    used.add(name);
    if (!(name in values)) fail(`${rel} marks an unknown figure "${name}" - known: ${Object.keys(values).join(', ')}`);
    const want = values[name];
    if (current !== want) stale.push({ rel, name, current, want });
    return `<!--fig:${name}-->${want}<!--/fig-->`;
  });
  if (rel.endsWith('status.html') && (before.match(new RegExp(BAR.source, 'g')) || []).length !== 1)
    fail('docs/status.html must contain exactly one <i id="fn-bar" style="width:N%"> - the coverage bar');
  after = after.replace(BAR, (whole, head, current, tail) => {
    counted++;
    const want = values['functions-pct'];
    if (current !== want) stale.push({ rel, name: 'functions-pct (bar width)', current: current + '%', want: want + '%' });
    return head + want + tail;
  });
  if (!check && after !== before) fs.writeFileSync(p, after);
}

for (const name of Object.keys(values))
  if (!used.has(name)) fail(`figure "${name}" is computed but quoted nowhere - remove it or use it`);

if (check) {
  if (stale.length) {
    console.error(`figures: ${stale.length} quoted figure(s) are stale.\n`);
    for (const s of stale)
      console.error(`  ${s.rel}  ${s.name}:  says ${s.current || '(empty)'}, should be ${s.want}`);
    console.error('\nRun:  node tools/figures/figures.js');
    process.exit(1);
  }
  console.log(`figures: ${counted} quoted figures are current (${values.tests} tests, ${values.functions} functions, ${values.globals} globals)`);
} else if (stale.length) {
  console.log(`figures: updated ${stale.length} of ${counted}`);
  for (const s of stale) console.log(`  ${s.rel}  ${s.name}:  ${s.current || '(empty)'} -> ${s.want}`);
} else {
  console.log(`figures: all ${counted} already current`);
}
