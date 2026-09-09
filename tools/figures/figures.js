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

function figures() {
  const sym = fromSymbols();
  const cargo = fromCargo();
  const pct = Math.round((sym.functions / BINARY_FUNCTIONS) * 100);
  return {
    tests: group(cargo.tests),
    'tests-l2-kingdom': group(cargo.testsKingdom),
    functions: group(sym.functions),
    globals: group(sym.globals),
    'functions-pct': String(pct),
    'binary-functions': group(BINARY_FUNCTIONS),
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

const TARGETS = ['README.md', path.join('docs', 'status.html')];
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
