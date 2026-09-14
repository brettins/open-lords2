#!/usr/bin/env node
// The work ledger's tooling: a check that `docs/work.json` agrees with git, and
// the view derived from it.
//
//   node tools/pm/work.js --check             schema, then agreement with git
//   node tools/pm/work.js --check --schema    schema only -- what the test runs
//   node tools/pm/work.js --status            the derived view, as text
//   node tools/pm/work.js --html <path>       the player's page: the game's features, then the work
//   node tools/pm/work.js --html-detail <path>  every row with its git facts, for agents
//   ...                        --file <path>  read another ledger file
//   ...                        --features <path>  read another feature list
//   ...                        --ref <ref>    compare and count against <ref> (default main)
//
// # Why this exists
//
// The project's plan and status lived in the lead's conversation.
// `docs/plan.md`'s in-flight section went stale while reading as current -- two
// "held for cause" verdicts were wrong by the time anybody acted on them -- and a
// machine restart needed an emergency dump of state that existed nowhere else.
// The player: *"your context window is not a good project management
// understanding -- you should be an interface for whatever project management
// setup you're running."*
//
// **The failure this file prevents is the one `plan.md` suffered: a status that
// looks current and is not.** So the ledger stores only what a person knows --
// intent -- and everything git knows is computed here, on every run, and never
// written anywhere. A stored "merged: false" is a claim with no timestamp; a
// derived one cannot be stale.
//
// # Two pages, for two readers
//
// The first page printed every row's prose in tables, and its systems section
// was gauges with commentary. The player: *"blabby rather than like a clear
// feature or project task list ... a bit illegible."* And it had no feature
// list at all, so nothing on it said how close the game is to the original,
// which is the whole goal. So `--html` is the player's page -- the game's
// features from `docs/features.json`, one line each, then what is in flight,
// what is waiting on him, and the rest folded away -- and `--html-detail` is
// the earlier page, for agents. **No source, note or next text reaches the
// player's page**; there a ledger row is its title and nothing else.
//
// # The feature list is a claim, so it is checked against evidence
//
// `docs/features.json` grades each feature of the original game done, partial,
// missing, not-assessed or out-of-scope. A feature marked done that is not is
// the failure this project keeps writing corrections about, so --check has
// three halves for it, and one report:
//
// * *schema*, which runs everywhere and in CI: every feature has every field;
//   a done feature cites at least one thing, and every citation is a form
//   somebody can check -- a correction `C123`, a path under crates/, docs/ or
//   tools/, or `arms:<id or group>` / `audio:<id or class>`; a partial feature
//   says in a few words what is missing;
// * *references*, full --check only: every ledger row a feature names is in
//   the ledger. A row leaves the ledger when its work merges, so a feature
//   still citing one has to be re-graded;
// * *evidence*, full --check where the ref resolves: every citation exists in
//   the ref -- the correction in its decisions.md, the path in its tree, the
//   arm or sound site in its inventory;
// * and a missing or partial feature that cites no ledger row is **reported
//   and not failed**. It is work nobody has written down, which is the lead's
//   to decide, not a check's to refuse.
//
// The page reads the list from the ref it names, `git show main:docs/features.json`,
// like every other figure; `--features <path>` reads a file instead, and the
// page says which file.
//
// # Every figure comes from the ref the view names, never from a working tree
//
// The first version of this tool read the systems inventories out of whatever
// checkout it sat in, while its header said `main 76a0437`. Run from an agent's
// worktree, the page quoted that worktree's differential (251 of 279) and
// census (399) under main's name, when main held 258 and 412, and said "stored
// fields: not on this base" of a file main had. That is this tool committing
// the exact failure it exists to prevent, and it was caught by a person reading
// the page. So:
//
// * the rollup reads every inventory with `git show <ref>:<path>`, and so does
//   the arms counting rule in `tools/figures/figures.js` wherever that ref's
//   copy can be required -- and says so on the page where it cannot;
// * the ledger's provenance is the ledger FILE's: its own checkout, branch and
//   the last commit that touched it, or plainly "uncommitted" -- never the
//   tool's HEAD, which is a different file's history in a different checkout.
//
// `crates/l2-testkit/tests/work_ledger.rs` holds both, in a scratch repository
// whose working tree disagrees with its main on every inventory.
//
// # What --check enforces
//
// *Schema*, which needs nothing but the file and runs everywhere: every row has
// every field and no other (a field git could answer is refused by name); state
// and track are declared at the top of the file; ids are unique; `depends_on`
// names rows that exist, and has no cycles; an `in-flight` or `queued-merge`
// row names a branch, since otherwise nothing about it can be checked.
//
// *Git agreement*, which needs the clone the work happens in: every branch a row
// names exists; no `in-flight` or `queued-merge` row names a branch already
// merged into the ref (merged work leaves the file); no unmerged
// `worktree-agent-*` or `wip/*` branch with commits ahead of the ref goes
// without a row; no `queued-merge` branch carries `HANDOFF.md`.
//
// A fresh clone -- a CI runner -- has no agent branches, and comparing the
// ledger against refs that were never fetched would report every branch
// missing. So the git half **skips there, and says so, with the reason and the
// number of rows it did not compare**, the way the census reports what it
// skips. A skip that prints nothing is a pass that means nothing.
//
// # What "merged" means here, and where that breaks
//
// A branch is merged when its tip is reachable from the ref and is **not on the
// ref's own first-parent line**. The second clause separates a merged branch
// from a freshly cut one, whose tip is an ordinary `main` commit: both have zero
// commits ahead, and only one of them is finished. Every merge on this project
// is a merge commit, which that rule reads correctly. A fast-forward merge would
// read as "no commits yet", and a squash or cherry-pick would read as unmerged;
// if the integrator ever merges that way, this rule has to change with it.

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const repo = path.resolve(__dirname, '..', '..');
const argv = process.argv.slice(2);
const has = (flag) => argv.includes(flag);
const opt = (flag) => {
  const i = argv.indexOf(flag);
  return i >= 0 && argv[i + 1] && !argv[i + 1].startsWith('--') ? argv[i + 1] : null;
};

const LEDGER = opt('--file') ? path.resolve(opt('--file')) : path.join(repo, 'docs', 'work.json');
const REF = opt('--ref') || 'main';
// The feature list: a file when named, otherwise the working tree's for
// --check and the ref's for the page.
const FEATURES_FLAG = opt('--features');
const FEATURES = FEATURES_FLAG ? path.resolve(FEATURES_FLAG) : path.join(repo, 'docs', 'features.json');
const FEATURES_IN_REF = 'docs/features.json';

// The row schema. Every field is required: an empty string, [] or null says
// "nothing to say" explicitly, which an absent field cannot.
const FIELDS = ['id', 'title', 'track', 'system', 'state', 'branch', 'depends_on', 'source', 'next', 'note'];
// Fields somebody will be tempted to type, and must not: git answers them.
const DERIVED = ['merged', 'ahead', 'behind', 'commits', 'handoff', 'last_commit', 'sha', 'exists', 'date', 'updated'];
// States in which a row is live work on a branch, so the branch is required and
// a merged branch means the row is stale.
const ON_A_BRANCH = ['in-flight', 'queued-merge'];
// Branch names that are agents' work, and so must have a row while unmerged.
const AGENT_BRANCH = /^(worktree-agent-|wip\/)/;

const hasOwn = (o, k) => Object.prototype.hasOwnProperty.call(o || {}, k);

function git(args, cwd = repo) {
  try {
    return execFileSync('git', args, {
      cwd,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      maxBuffer: 64 << 20,
    }).trim();
  } catch {
    return null;
  }
}

// The ref every git fact and every figure is computed from, resolved once.
function resolveBase() {
  if (git(['rev-parse', '--git-dir']) === null) return null;
  const sha = git(['rev-parse', '--verify', '--quiet', `${REF}^{commit}`]);
  return sha ? { name: REF, sha, short: sha.slice(0, 7) } : null;
}

// Where a file itself came from: its own checkout, branch, and the last commit
// that touched it -- or plainly that no commit holds what was read.
const ledgerSource = () => fileSource(LEDGER);
function fileSource(file) {
  const top = git(['rev-parse', '--show-toplevel'], path.dirname(file));
  if (top === null) {
    return { path: file, checkout: null, branch: null, commit: null, state: 'outside', label: `${file}, a file outside any git checkout` };
  }
  const p = path.relative(top, file).replace(/\\/g, '/');
  const head = git(['rev-parse', '--abbrev-ref', 'HEAD'], top);
  const branch = !head || head === 'HEAD' ? 'a detached HEAD' : head;
  const tracked = git(['ls-files', '--error-unmatch', '--', p], top) !== null;
  const last = tracked ? git(['log', '-1', '--format=%H', '--', p], top) : null;
  const commit = last ? last.slice(0, 7) : null;
  const dirty = tracked && !!git(['status', '--porcelain', '--', p], top);
  const place = `${p} on ${branch}`;
  if (!commit) return { path: p, checkout: top, branch, commit, state: 'uncommitted', label: `${place}, an uncommitted file that no commit holds` };
  if (dirty) return { path: p, checkout: top, branch, commit, state: 'modified', label: `${place}, with uncommitted changes since ${commit}` };
  return { path: p, checkout: top, branch, commit, state: 'committed', label: `${place}, last committed in ${commit}` };
}

// ---- schema ---------------------------------------------------------------

function loadJson(file) {
  let text;
  try {
    text = fs.readFileSync(file, 'utf8');
  } catch (e) {
    return { problems: [{ id: '(file)', msg: `cannot read ${file}: ${e.message}` }] };
  }
  try {
    return { j: JSON.parse(text), problems: [] };
  } catch (e) {
    return { problems: [{ id: '(file)', msg: `${file} is not valid JSON: ${e.message}` }] };
  }
}
const load = () => loadJson(LEDGER);

function schema(j) {
  const P = [];
  const bad = (id, msg) => P.push({ id, msg });
  if (!j || typeof j !== 'object' || Array.isArray(j)) {
    bad('(file)', 'the ledger must be a JSON object');
    return P;
  }
  if (typeof j.about !== 'string' || !j.about.trim()) bad('(file)', 'top-level "about" must state the rules');
  for (const k of ['states', 'tracks']) {
    if (!j[k] || typeof j[k] !== 'object' || Array.isArray(j[k])) {
      bad('(file)', `top-level "${k}" must be an object of name -> meaning`);
    }
  }
  if (!Array.isArray(j.items)) {
    bad('(file)', 'top-level "items" must be an array of rows');
    return P;
  }

  const seen = new Map();
  j.items.forEach((r, i) => {
    const id = r && typeof r.id === 'string' && r.id.trim() ? r.id : `(row ${i + 1})`;
    if (!r || typeof r !== 'object' || Array.isArray(r)) {
      bad(id, 'is not an object');
      return;
    }
    for (const f of FIELDS) {
      if (!hasOwn(r, f)) {
        bad(id, `missing field "${f}" -- every row carries all of: ${FIELDS.join(', ')} ("", [] or null where there is nothing to say)`);
      }
    }
    for (const f of Object.keys(r)) {
      if (FIELDS.includes(f)) continue;
      bad(
        id,
        DERIVED.includes(f)
          ? `carries "${f}", which git answers. It is derived by work.js --status on every run and must never be typed here -- a stored fact is a claim with no timestamp`
          : `unknown field "${f}" -- the row schema is: ${FIELDS.join(', ')}`,
      );
    }
    if (typeof r.id !== 'string' || !r.id.trim()) bad(id, '"id" must be a non-empty string');
    else if (seen.has(r.id)) bad(id, `duplicate id: rows ${seen.get(r.id) + 1} and ${i + 1} both claim it, and a merge keyed by id keeps only one`);
    else seen.set(r.id, i);
    for (const f of ['title', 'system']) {
      if (hasOwn(r, f) && (typeof r[f] !== 'string' || !r[f].trim())) bad(id, `"${f}" must be a non-empty string`);
    }
    for (const f of ['source', 'next', 'note']) {
      if (hasOwn(r, f) && typeof r[f] !== 'string') bad(id, `"${f}" must be a string`);
    }
    if (hasOwn(r, 'state') && !hasOwn(j.states, r.state)) {
      bad(id, `unknown state ${JSON.stringify(r.state)} -- declared: ${Object.keys(j.states || {}).join(', ')}`);
    }
    if (hasOwn(r, 'track') && !hasOwn(j.tracks, r.track)) {
      bad(id, `unknown track ${JSON.stringify(r.track)} -- declared: ${Object.keys(j.tracks || {}).join(', ')}`);
    }
    if (hasOwn(r, 'branch') && r.branch !== null && (typeof r.branch !== 'string' || !r.branch.trim())) {
      bad(id, '"branch" must be a branch name or null');
    }
    if (ON_A_BRANCH.includes(r.state) && !r.branch) {
      bad(id, `is ${r.state} with no branch, so nothing about it can be checked against git -- name the branch`);
    }
    if (hasOwn(r, 'depends_on') && (!Array.isArray(r.depends_on) || r.depends_on.some((d) => typeof d !== 'string'))) {
      bad(id, '"depends_on" must be an array of row ids');
    }
  });

  const rows = j.items.filter((r) => r && typeof r.id === 'string');
  const byId = new Map(rows.map((r) => [r.id, r]));
  const deps = (r) => (Array.isArray(r.depends_on) ? r.depends_on : []);
  for (const r of rows) {
    for (const d of deps(r)) {
      if (d === r.id) bad(r.id, 'depends on itself, so it can never be unblocked');
      else if (!byId.has(d)) {
        bad(r.id, `depends_on "${d}", which is not a row. If it merged, it left the ledger and the dependency is met: remove it from depends_on`);
      }
    }
  }
  // Cycles longer than one row. Reported once per cycle, on the row that
  // closes it, with the whole loop in the message.
  const colour = new Map();
  const reported = new Set();
  const visit = (id, stack) => {
    colour.set(id, 'grey');
    stack.push(id);
    for (const d of deps(byId.get(id))) {
      if (d === id || !byId.has(d)) continue;
      if (colour.get(d) === 'grey') {
        const loop = stack.slice(stack.indexOf(d));
        const key = [...loop].sort().join(' ');
        if (!reported.has(key)) {
          reported.add(key);
          bad(d, `dependency cycle: ${[...loop, d].join(' -> ')} -- none of these can ever be unblocked`);
        }
      } else if (!colour.has(d)) visit(d, stack);
    }
    stack.pop();
    colour.set(id, 'black');
  };
  for (const r of rows) if (!colour.has(r.id)) visit(r.id, []);
  return P;
}

// ---- the feature list -----------------------------------------------------

const FEATURE_FIELDS = ['id', 'area', 'name', 'status', 'evidence', 'gap', 'rows'];
// The page draws a mark for each of these and for nothing else.
const FEATURE_STATUSES = ['done', 'partial', 'missing', 'not-assessed', 'out-of-scope'];
// A feature is one line on the player's page, so its words are few; the
// ledger row carries the rest.
const NAME_MAX = 48;
const GAP_MAX = 40;

// What a citation is, or null when it is not one anybody can check.
function evidenceKind(e) {
  if (typeof e !== 'string') return null;
  let m;
  if ((m = /^C(\d+)$/.exec(e))) return { kind: 'correction', n: m[1] };
  if ((m = /^(arms|audio):(\S+)$/.exec(e))) return { kind: m[1], key: m[2] };
  if (/^(crates|docs|tools)\/[\w./#-]+$/.test(e) && !e.split('/').includes('..')) return { kind: 'path', path: e };
  return null;
}

const featureList = (j) => (j && Array.isArray(j.features) ? j.features.filter((f) => f && typeof f.id === 'string') : []);
const knownStatus = (s) => (FEATURE_STATUSES.includes(s) ? s : 'not-assessed');

function featureSchema(j) {
  const P = [];
  const bad = (id, msg) => P.push({ id, msg });
  if (!j || typeof j !== 'object' || Array.isArray(j)) {
    bad('(file)', 'the feature list must be a JSON object');
    return P;
  }
  if (typeof j.about !== 'string' || !j.about.trim()) bad('(file)', 'top-level "about" must say what the file is and how a feature is graded');
  if (!j.statuses || typeof j.statuses !== 'object' || Array.isArray(j.statuses)) {
    bad('(file)', `top-level "statuses" must be an object of status -> meaning, declaring exactly: ${FEATURE_STATUSES.join(', ')}`);
  } else {
    const keys = Object.keys(j.statuses);
    const extra = keys.filter((k) => !FEATURE_STATUSES.includes(k));
    const lacking = FEATURE_STATUSES.filter((k) => !keys.includes(k));
    if (extra.length || lacking.length) {
      bad('(file)', `"statuses" must declare exactly ${FEATURE_STATUSES.join(', ')} -- the page draws a mark for each of those and no other${extra.length ? `; not drawable: ${extra.join(', ')}` : ''}${lacking.length ? `; undeclared: ${lacking.join(', ')}` : ''}`);
    }
  }
  const areasOk = j.areas && typeof j.areas === 'object' && !Array.isArray(j.areas) && Object.keys(j.areas).length;
  if (!areasOk) bad('(file)', 'top-level "areas" must be an object of area id -> the name a player sees, in page order');
  else for (const [k, v] of Object.entries(j.areas)) if (typeof v !== 'string' || !v.trim()) bad('(file)', `area "${k}" must have a name`);
  if (!Array.isArray(j.features)) {
    bad('(file)', 'top-level "features" must be an array');
    return P;
  }

  const seen = new Map();
  j.features.forEach((f, i) => {
    const id = f && typeof f.id === 'string' && f.id.trim() ? f.id : `(feature ${i + 1})`;
    if (!f || typeof f !== 'object' || Array.isArray(f)) {
      bad(id, 'is not an object');
      return;
    }
    for (const k of FEATURE_FIELDS) {
      if (!hasOwn(f, k)) bad(id, `missing field "${k}" -- every feature carries all of: ${FEATURE_FIELDS.join(', ')} ("" or [] where there is nothing to say)`);
    }
    for (const k of Object.keys(f)) if (!FEATURE_FIELDS.includes(k)) bad(id, `unknown field "${k}" -- the feature schema is: ${FEATURE_FIELDS.join(', ')}`);
    if (typeof f.id !== 'string' || !/^[a-z0-9][a-z0-9-]*$/.test(f.id)) bad(id, '"id" must be lower-case words joined by hyphens');
    else if (seen.has(f.id)) bad(id, `duplicate id: features ${seen.get(f.id) + 1} and ${i + 1} both claim it`);
    else seen.set(f.id, i);
    if (hasOwn(f, 'area') && !(areasOk && hasOwn(j.areas, f.area))) {
      bad(id, `unknown area ${JSON.stringify(f.area)} -- declared: ${areasOk ? Object.keys(j.areas).join(', ') : 'none'}`);
    }
    if (hasOwn(f, 'name')) {
      if (typeof f.name !== 'string' || !f.name.trim()) bad(id, '"name" must be a non-empty string');
      else if (f.name.length > NAME_MAX) bad(id, `"name" is ${f.name.length} characters, and a feature gets one line on the player's page -- name it in a few words (${NAME_MAX} at most)`);
    }
    if (hasOwn(f, 'status') && !FEATURE_STATUSES.includes(f.status)) {
      bad(id, `unknown status ${JSON.stringify(f.status)} -- one of: ${FEATURE_STATUSES.join(', ')}`);
    }
    if (hasOwn(f, 'gap')) {
      if (typeof f.gap !== 'string') bad(id, '"gap" must be a string');
      else if (f.gap.length > GAP_MAX) bad(id, `"gap" is ${f.gap.length} characters -- say what is missing in a few words (${GAP_MAX} at most); the ledger row carries the rest`);
      else if (f.status === 'partial' && !f.gap.trim()) bad(id, 'is partial and does not say what is missing -- put it in "gap", in a few words');
    }
    if (hasOwn(f, 'evidence')) {
      if (!Array.isArray(f.evidence)) bad(id, '"evidence" must be an array of citations');
      else {
        for (const e of f.evidence) {
          if (!evidenceKind(e)) {
            bad(id, `evidence ${JSON.stringify(e)} is not a citation anybody can check -- use a correction (C123), a path under crates/, docs/ or tools/, or arms:<id or group> / audio:<id or class>`);
          }
        }
        if (f.status === 'done' && !f.evidence.length) {
          bad(id, 'is done and cites nothing -- a done feature names the correction, test or inventory entry that shows it, because a feature marked done that is not is the failure this file exists to prevent');
        }
      }
    }
    if (hasOwn(f, 'rows') && (!Array.isArray(f.rows) || f.rows.some((r) => typeof r !== 'string'))) {
      bad(id, '"rows" must be an array of docs/work.json row ids');
    }
  });
  return P;
}

// Every ledger row a feature names is in the ledger.
function featureReferences(j, rows, ledgerLabel) {
  const ids = new Set(rows.map((r) => r.id));
  const P = [];
  for (const f of featureList(j)) {
    for (const r of Array.isArray(f.rows) ? f.rows : []) {
      if (typeof r === 'string' && !ids.has(r)) {
        P.push({
          id: f.id,
          msg: `cites ledger row "${r}", which is not in ${ledgerLabel}. If it merged, its work landed: re-grade this feature and cite the correction that landed it; if the row was renamed, follow it`,
        });
      }
    }
  }
  return P;
}

// Every citation exists in the ref: the correction in its decisions.md, the
// path in its tree, the arm or sound site in its inventory.
function featureEvidence(j, base) {
  const src = at(base);
  const problems = [];
  let citations = 0;
  let files = null;
  let corrections = null;
  let arms = null;
  let audio = null;
  const keysOf = (p, list, fields) => {
    if (!src.exists(p)) return new Set();
    const got = src.json(p)[list];
    return new Set((Array.isArray(got) ? got : []).flatMap((r) => fields.map((k) => r && r[k]).filter((v) => typeof v === 'string')));
  };
  for (const f of featureList(j)) {
    for (const e of Array.isArray(f.evidence) ? f.evidence : []) {
      const k = evidenceKind(e);
      if (!k) continue; // the schema names it
      citations++;
      let why = null;
      if (k.kind === 'path') {
        files = files || new Set((git(['ls-tree', '-r', '--name-only', base.sha]) || '').split('\n'));
        if (!files.has(k.path)) why = `is not a file in ${src.where}`;
      } else if (k.kind === 'correction') {
        const log = src.exists('docs/decisions.md') ? src.read('docs/decisions.md') : '';
        corrections = corrections || new Set([...log.matchAll(/^\*\*C(\d+)\b/gm)].map((x) => x[1]));
        if (!corrections.has(k.n)) why = `is not a correction in docs/decisions.md at ${src.where}`;
      } else if (k.kind === 'arms') {
        arms = arms || keysOf('docs/arms.json', 'arms', ['id', 'group']);
        if (!arms.has(k.key)) why = `is neither an arm id nor a group in docs/arms.json at ${src.where}`;
      } else {
        audio = audio || keysOf('docs/audio.json', 'sites', ['id', 'class']);
        if (!audio.has(k.key)) why = `is neither a site id nor a class in docs/audio.json at ${src.where}`;
      }
      if (why) problems.push({ id: f.id, msg: `evidence "${e}" ${why} -- cite something the ref holds, or re-grade the feature` });
    }
  }
  return { problems, citations };
}

// Missing or partial, and no ledger row covers it: work nobody has written down.
const unrecorded = (list) => list.filter((f) => (f.status === 'missing' || f.status === 'partial') && !(Array.isArray(f.rows) && f.rows.length));

// "N of M done", where M leaves out what is deliberately out of scope.
function featureCounts(list) {
  const c = { total: 0 };
  for (const s of FEATURE_STATUSES) c[s] = 0;
  for (const f of list) {
    c[knownStatus(f.status)]++;
    c.total++;
  }
  c.graded = c.total - c['out-of-scope'];
  return c;
}

// The list the page draws: the ref's copy, or the file --features names.
function featuresForView(base, rows, ledgerLabel) {
  let text;
  let label;
  if (FEATURES_FLAG) {
    label = fileSource(FEATURES).label;
    try {
      text = fs.readFileSync(FEATURES, 'utf8');
    } catch (e) {
      return { error: `cannot read ${FEATURES}: ${e.message}`, label };
    }
  } else {
    const src = at(base);
    label = `${FEATURES_IN_REF} at ${src.where}`;
    if (!src.exists(FEATURES_IN_REF)) return { absent: `${FEATURES_IN_REF} is not in ${src.where}`, label };
    text = src.read(FEATURES_IN_REF);
  }
  let j;
  try {
    j = JSON.parse(text);
  } catch (e) {
    return { error: `${label} is not valid JSON: ${e.message}`, label };
  }
  const problems = [...featureSchema(j), ...featureReferences(j, rows, ledgerLabel)];
  try {
    problems.push(...featureEvidence(j, base).problems);
  } catch (e) {
    problems.push({ id: '(file)', msg: `the evidence could not be checked: ${e.message}` });
  }
  const list = featureList(j);
  const declared = j.areas && typeof j.areas === 'object' && !Array.isArray(j.areas) ? j.areas : {};
  const areaIds = Object.keys(declared);
  for (const f of list) if (!areaIds.includes(f.area)) areaIds.push(f.area);
  const areas = areaIds
    .map((a) => ({ id: a, name: typeof declared[a] === 'string' && declared[a] ? declared[a] : String(a), features: list.filter((f) => f.area === a) }))
    .filter((a) => a.features.length);
  return { label, problems, list, areas, counts: featureCounts(list), unrecorded: unrecorded(list) };
}

// --check's half for the feature list. Prints what it did and what it
// skipped, and returns the problems.
function checkFeatures(rows) {
  const { j, problems } = loadJson(FEATURES);
  if (!j) return problems;
  const list = featureList(j);
  const found = featureSchema(j);
  const areaCount = j.areas && typeof j.areas === 'object' ? Object.keys(j.areas).length : 0;
  console.log(`work: features: schema: ${list.length} features in ${areaCount} areas, ${found.length} problem(s)`);
  if (has('--schema')) {
    console.log(`work: SKIP feature references and evidence: --schema was given; ${list.length} features were not compared with the ledger or with ${REF}`);
  } else {
    const refs = featureReferences(j, rows, ledgerSource().label);
    const cited = list.reduce((n, f) => n + (Array.isArray(f.rows) ? f.rows.length : 0), 0);
    console.log(`work: features: references: ${cited} ledger row citation(s) checked against the ledger, ${refs.length} problem(s)`);
    found.push(...refs);
    const base = resolveBase();
    if (!base) {
      console.log(`work: SKIP feature evidence: ${REF} does not resolve here, so no citation was checked`);
    } else {
      try {
        const ev = featureEvidence(j, base);
        console.log(`work: features: evidence: ${ev.citations} citation(s) checked against ${base.name} ${base.short}, ${ev.problems.length} problem(s)`);
        found.push(...ev.problems);
      } catch (e) {
        found.push({ id: '(file)', msg: `the evidence could not be checked against ${base.name} ${base.short}: ${e.message}` });
      }
    }
  }
  const un = unrecorded(list);
  console.log(`work: features: ${un.length} missing or partial feature(s) cite no ledger row -- work nobody has written down; reported, not failed${un.length ? ':' : ''}`);
  for (const f of un) console.log(`work:   ${f.id} (${f.status}) ${f.name}${f.gap ? ` -- ${f.gap}` : ''}`);
  return found;
}

// ---- git ------------------------------------------------------------------

// Everything git can say about the ledger's branches, or the reason it cannot.
function gitFacts(rows, base) {
  if (git(['rev-parse', '--git-dir']) === null) return { skip: 'this is not a git checkout' };
  if (!base) return { skip: `the ref ${REF} does not resolve here (a pull-request checkout, or a clone that never made it)` };
  const listing = git(['for-each-ref', '--format=%(refname:short)%09%(objectname)', 'refs/heads']);
  const heads = new Map((listing || '').split('\n').filter(Boolean).map((l) => l.split('\t')));
  const agents = [...heads.keys()].filter((b) => AGENT_BRANCH.test(b));
  if (!agents.length) {
    return { skip: 'there are no local worktree-agent-* or wip/* branches, so this is not the clone the work happens in (a fresh clone, or a CI runner)' };
  }

  const firstParent = new Set((git(['rev-list', '--first-parent', base.sha]) || '').split('\n'));
  const reachable = new Set(
    (git(['for-each-ref', '--merged', base.sha, '--format=%(refname:short)', 'refs/heads']) || '')
      .split('\n')
      .filter(Boolean),
  );

  const cache = new Map();
  const facts = (b) => {
    if (cache.has(b)) return cache.get(b);
    let f;
    if (!heads.has(b)) f = { branch: b, exists: false };
    else {
      const sha = heads.get(b);
      const inBase = reachable.has(b);
      const ahead = inBase ? 0 : Number(git(['rev-list', '--count', `${base.sha}..refs/heads/${b}`]));
      const last = ahead ? git(['log', '-1', '--format=%cI%x09%s', `${base.sha}..refs/heads/${b}`]) : null;
      const [date, subject] = last ? last.split('\t') : [null, null];
      f = {
        branch: b,
        exists: true,
        sha,
        ahead,
        merged: inBase && !firstParent.has(sha),
        fresh: inBase && firstParent.has(sha),
        handoff: git(['cat-file', '-e', `refs/heads/${b}:HANDOFF.md`]) !== null,
        date,
        subject,
      };
    }
    cache.set(b, f);
    return f;
  };

  const rowFacts = new Map();
  const byId = new Map(rows.map((r) => [r.id, r]));
  for (const r of rows) {
    if (!r.branch) continue;
    const f = { ...facts(r.branch) };
    // Commits of its own: ahead of the ref AND of every branch it is built on.
    // A row stacked on another row's branch carries that branch's commits too,
    // and "12 ahead" would otherwise credit it with work it did not do.
    const bases = (Array.isArray(r.depends_on) ? r.depends_on : [])
      .map((d) => byId.get(d))
      .filter((d) => d && d.branch && d.branch !== r.branch && heads.has(d.branch));
    if (f.exists && f.ahead && bases.length) {
      f.own = Number(git(['rev-list', '--count', `refs/heads/${r.branch}`, `^${base.sha}`, ...bases.map((d) => `^refs/heads/${d.branch}`)]));
    } else if (f.exists) f.own = f.ahead;
    rowFacts.set(r.id, f);
  }

  const named = new Set(rows.map((r) => r.branch).filter(Boolean));
  const orphans = agents.filter((b) => !reachable.has(b) && !named.has(b)).map(facts);

  return { base, branches: heads.size, agentBranches: agents.length, rowFacts, orphans };
}

function agreement(rows, g) {
  const P = [];
  const bad = (id, msg) => P.push({ id, msg });
  const into = `${g.base.name} (${g.base.short})`;
  for (const r of rows) {
    const f = g.rowFacts.get(r.id);
    if (!f) continue;
    if (!f.exists) {
      bad(r.id, `names branch ${r.branch}, which does not exist in this clone. If it merged, the row should have left; if it was deleted, say so in note and set branch to null`);
      continue;
    }
    if (ON_A_BRANCH.includes(r.state) && f.merged) {
      const dependents = rows.filter((x) => Array.isArray(x.depends_on) && x.depends_on.includes(r.id)).map((x) => x.id);
      bad(
        r.id,
        `is ${r.state}, but ${r.branch} (${f.sha.slice(0, 7)}) is already merged into ${into}. ` +
          `Merged work leaves the ledger: delete this row` +
          (dependents.length ? `, and remove "${r.id}" from depends_on in ${dependents.join(', ')}` : ''),
      );
    }
    if (r.state === 'queued-merge' && f.handoff) {
      bad(r.id, `is queued-merge, but ${r.branch} carries HANDOFF.md -- the note an agent leaves when it is stopped mid-work, so the branch is not finished. Read it, then move the row back to in-flight or awaiting-user, or have the handoff removed on the branch`);
    }
  }
  for (const f of g.orphans) {
    bad(
      `branch ${f.branch}`,
      `has ${f.ahead} commit${f.ahead === 1 ? '' : 's'} ahead of ${into} (last ${day(f.date)}: "${f.subject}") and no row names it. ` +
        'Work nobody is tracking is what this ledger exists to prevent: write its row, or delete the branch if it is dead',
    );
  }
  return P;
}

const day = (iso) => (iso ? iso.slice(0, 10) : '-');
const stamp = (iso) => (iso ? iso.slice(0, 16).replace('T', ' ') : '-');

// ---- the systems rollup ---------------------------------------------------
//
// Computed from the inventories that are themselves checked, never typed, and
// read from the ref the view names -- never from the working tree. Each figure
// is quoted with its second column, because a count weights every row equally
// and a player does not (`CLAUDE.md`, the three inventories).
//
// A figure that cannot be read is an error on the page, in red, and a non-zero
// exit -- not a blank. A tool that degrades silently is worse the more people
// use it.

function counts(list, keyFn) {
  const out = {};
  for (const x of list) {
    const k = keyFn(x);
    out[k] = (out[k] || 0) + 1;
  }
  return out;
}
const listCounts = (o, order) =>
  Object.entries(o)
    .sort((a, b) => (order ? order.indexOf(a[0]) - order.indexOf(b[0]) : b[1] - a[1] || (a[0] < b[0] ? -1 : 1)))
    .map(([k, v]) => `${v} ${k}`)
    .join(', ');

// Reads files as they are in one commit.
function at(base) {
  const where = `${base.name} ${base.short}`;
  const exists = (p) => git(['cat-file', '-e', `${base.sha}:${p}`]) !== null;
  const read = (p) => {
    const t = git(['show', `${base.sha}:${p}`]);
    if (t === null) throw new Error(`${p} is not in ${where}`);
    return t;
  };
  return { where, exists, read, json: (p) => JSON.parse(read(p)) };
}

// The arms counting rule, from the same commit as the data where that commit's
// `figures.js` can be required. A copy from before it exported `fromArms(j)`
// would run the whole suite if loaded, so that case uses this checkout's rule
// and the page says so rather than pretending.
function armsRule(src) {
  const p = 'tools/figures/figures.js';
  const text = src.exists(p) ? src.read(p) : '';
  if (/module\.exports = \{ fromArms \}/.test(text) && /function fromArms\(j\b/.test(text)) {
    const Module = require('module');
    const filename = path.join(repo, p);
    const m = new Module(filename, module);
    m.filename = filename;
    m.paths = Module._nodeModulePaths(path.dirname(filename));
    m._compile(text, filename);
    return { fromArms: m.exports.fromArms, borrowed: null };
  }
  return {
    fromArms: require(path.join(repo, p)).fromArms,
    borrowed: `the counting rule is this checkout's tools/figures/figures.js, because the copy in ${src.where} predates the export; the data is ${src.where}'s`,
  };
}

function inventories(base) {
  const src = at(base);
  const out = [];
  const attempt = (meta, fn) => {
    try {
      out.push({ ...meta, where: src.where, ...fn() });
    } catch (e) {
      out.push({ ...meta, where: src.where, error: e.message });
    }
  };

  attempt({ track: 'screens', system: 'input', name: 'Input arms', source: 'docs/arms.json' }, () => {
    const rule = armsRule(src);
    const j = src.json('docs/arms.json');
    const a = rule.fromArms(j);
    const missing = counts(j.arms.filter((x) => x.status === 'missing'), (x) => x.group || '(no group)');
    return {
      head: `${a.reproduced} of ${a.live}`,
      unit: 'live arms reproduced',
      pct: a.pct,
      second: [
        `${a.kinded} of the ${a.live} live arms are filed under one of the ${a.kinds} mouse gesture kinds -- which controls a screen answers is not how it answers them`,
        `${a.missing} missing, by group: ${listCounts(missing) || 'none'}`,
        `${a.dead} dead in the binary and ${a.inventions} inventions of ours, both outside the denominator`,
        ...(rule.borrowed ? [rule.borrowed] : []),
      ],
    };
  });

  attempt({ track: 'presentation', system: 'audio', name: 'Sound triggers', source: 'docs/audio.json' }, () => {
    const sites = src.json('docs/audio.json').sites;
    const order = ['reproduced', 'missing', 'blocked', 'dead'];
    const byStatus = counts(sites, (s) => s.status);
    for (const s of Object.keys(byStatus)) if (!order.includes(s)) order.push(s);
    const classes = [...new Set(sites.map((s) => s.class))];
    const byClass = classes
      .map((c) => {
        const of = sites.filter((s) => s.class === c);
        return [c, of.filter((s) => s.status === 'reproduced').length, of.length];
      })
      .sort((a, b) => b[2] - a[2])
      .map(([c, fired, total]) => `${c} ${fired}/${total}`)
      .join(', ');
    // "By file": a site whose `sound` names one .wav plays that file; a site
    // whose `sound` is prose ("530 files by group and variant") is a table.
    // The table's reach is quoted from its row, never summed -- summing prose
    // would be typing a number and calling it derived.
    const oneFile = (s) => typeof s.sound === 'string' && /^[\w.-]+\.wav$/i.test(s.sound);
    const describe = (status) => {
      const of = sites.filter((s) => s.status === status);
      const files = new Set(of.filter(oneFile).map((s) => s.sound.toLowerCase()));
      const tables = counts(of.filter((s) => s.sound && !oneFile(s)), (s) => s.sound);
      const quoted = Object.entries(tables)
        .sort((a, b) => b[1] - a[1])
        .map(([t, n]) => `"${t}" (${n} site${n === 1 ? '' : 's'})`)
        .join('; ');
      return { files: files.size, fileSites: of.filter(oneFile).length, quoted };
    };
    const fired = describe('reproduced');
    const blocked = describe('blocked');
    return {
      head: `${byStatus.reproduced || 0} of ${sites.length}`,
      unit: 'trigger sites fire',
      pct: Math.round(((byStatus.reproduced || 0) / sites.length) * 100),
      second: [
        `by status: ${listCounts(byStatus, order)}`,
        `by class, fired of total: ${byClass}`,
        `by file: ${fired.fileSites} fired sites play ${fired.files} distinct named files; the rest of the fired sites are tables, whose rows say ${fired.quoted || 'nothing'}`,
        `blocked tables, as their rows say: ${blocked.quoted || 'none'}`,
      ],
    };
  });

  attempt({ track: 'play', system: 'save-load', name: 'Stored fields', source: 'docs/stored-fields.json' }, () => {
    const p = 'docs/stored-fields.json';
    if (!src.exists(p)) {
      return { absent: `not in ${src.where} -- it arrives with the stored-fields sweep, and this rollup reads it from then on` };
    }
    const fields = src.json(p).fields;
    if (!Array.isArray(fields)) throw new Error(`${p} in ${src.where} has no "fields" array`);
    const order = ['imported', 'derived', 'excluded'];
    const byStatus = counts(fields, (f) => f.status);
    for (const s of Object.keys(byStatus)) if (!order.includes(s)) order.push(s);
    const elements = {};
    for (const f of fields) elements[f.status] = (elements[f.status] || 0) + (Number(f.count) || 1);
    const byRecord = counts(fields, (f) => String(f.id).split('+')[0]);
    const carried = (byStatus.imported || 0) + (byStatus.derived || 0);
    return {
      head: `${carried} of ${fields.length}`,
      unit: 'stored fields carried by a load',
      pct: Math.round((carried / fields.length) * 100),
      second: [
        `by status: ${listCounts(byStatus, order)}`,
        `by element, arrays counted per element: ${listCounts(elements, order)}`,
        `by record: ${listCounts(byRecord)}`,
      ],
    };
  });

  attempt({ track: 'play', system: 'turns', name: 'End Turn differential', source: 'crates/l2-game/tests/differential.rs' }, () => {
    const text = src.read('crates/l2-game/tests/differential.rs');
    const c = (name) => {
      const m = text.match(new RegExp(`const ${name}: usize = (\\d+);`));
      if (!m) throw new Error(`${name} is not in differential.rs in ${src.where} -- this rollup reads it rather than restating it, so find where it went`);
      return Number(m[1]);
    };
    const [compared, agree, moved, movedAgree] = ['COMPARED_TOTAL', 'AGREE_TOTAL', 'MOVED_TOTAL', 'MOVED_AGREE_TOTAL'].map(c);
    return {
      head: `${movedAgree} of ${moved}`,
      unit: "fields the original's End Turn moved, agreeing",
      pct: Math.round((movedAgree / moved) * 100),
      second: [
        `${agree} of ${compared} comparisons agree overall -- most of a record is inert across a season, so that number is mostly free; quote the one above`,
      ],
    };
  });

  attempt({ track: 'instruments', system: 'tests', name: 'Install-gated tests', source: 'crates/l2-testkit/tests/census/main.rs' }, () => {
    const text = src.read('crates/l2-testkit/tests/census/main.rs');
    const m = text.match(/const GATED_TOTAL: usize = (\d+);/);
    if (!m) throw new Error(`GATED_TOTAL is not in census.rs in ${src.where}`);
    const files = new Set([...text.matchAll(/^\s*\("(crates\/[^"]+)",/gm)].map((x) => x[1]));
    return {
      head: m[1],
      unit: 'tests that do not exist without a copy of the game',
      second: [`across ${files.size} test files; a clone without the game, and CI, run none of them`],
    };
  });
  return out;
}

// ---- the derived model ----------------------------------------------------

function derive() {
  const { j, problems: loadProblems } = load();
  if (!j) return { fatal: loadProblems };
  const base = resolveBase();
  if (!base) {
    return {
      fatal: [
        {
          id: '(ref)',
          msg: `${REF} does not resolve in this clone. The view computes every git fact and every figure from a ref, never from the working tree the tool sits in, so name one that exists: --ref <branch or sha>`,
        },
      ],
    };
  }
  const problems = schema(j);
  const rows = (Array.isArray(j.items) ? j.items : []).filter((r) => r && typeof r.id === 'string');

  let g = gitFacts(rows, base);
  let skip = null;
  if (g.skip) {
    skip = g.skip;
    g = null;
  } else problems.push(...agreement(rows, g));

  const byId = new Map(rows.map((r) => [r.id, r]));
  const facts = (r) => (g ? g.rowFacts.get(r.id) : null);
  const inState = (s) => rows.filter((r) => r.state === s);

  // The merge queue: file order, except that a row never precedes a queued row
  // it depends on.
  const queued = inState('queued-merge');
  const queue = [];
  const placed = new Set();
  while (queue.length < queued.length) {
    const next =
      queued.find((r) => !placed.has(r.id) && (r.depends_on || []).every((d) => placed.has(d) || !queued.some((q) => q.id === d))) ||
      queued.find((r) => !placed.has(r.id));
    queue.push(next);
    placed.add(next.id);
  }

  const blocked = rows
    .filter((r) => Array.isArray(r.depends_on) && r.depends_on.some((d) => byId.has(d)))
    .map((r) => ({ row: r, on: r.depends_on.filter((d) => byId.has(d)).map((d) => byId.get(d)) }));

  const inv = inventories(base);
  const ledger = ledgerSource();
  const features = featuresForView(base, rows, ledger.label);

  // Tracks in the ledger's own order; systems alphabetical within each.
  const trackNames = [...Object.keys(j.tracks || {})];
  for (const x of [...rows, ...inv]) if (!trackNames.includes(x.track)) trackNames.push(x.track);
  const tracks = trackNames.map((t) => {
    const systems = [...new Set([...rows, ...inv].filter((x) => x.track === t).map((x) => x.system))].sort();
    return {
      name: t,
      about: (j.tracks || {})[t] || '',
      systems: systems.map((s) => ({
        name: s,
        states: counts(rows.filter((r) => r.track === t && r.system === s), (r) => r.state),
        inventories: inv.filter((x) => x.track === t && x.system === s),
      })),
    };
  });

  return {
    generated: new Date().toISOString(),
    base,
    ledger,
    features,
    stateOrder: Object.keys(j.states || {}),
    states: j.states || {},
    git: g ? { branches: g.branches, agentBranches: g.agentBranches, orphans: g.orphans.length } : null,
    skip,
    problems,
    rows,
    facts,
    queue,
    blocked,
    tracks,
    inv,
    inState,
  };
}

// ---- text -----------------------------------------------------------------

function gitLine(m, r, f) {
  if (!r.branch) return 'no branch';
  if (!f) return `${r.branch}  (git not compared)`;
  if (!f.exists) return `${r.branch}  BRANCH MISSING`;
  const bits = [];
  if (f.merged) bits.push(ON_A_BRANCH.includes(r.state) ? 'MERGED -- row is stale' : 'merged');
  else if (f.fresh) bits.push('no commits of its own yet');
  else {
    bits.push(`${f.ahead} ahead of ${m.base.name}`);
    if (f.own !== f.ahead) bits.push(`${f.own} beyond the branch it is built on`);
    bits.push(`last ${stamp(f.date)} "${f.subject}"`);
  }
  if (f.handoff) bits.push('HANDOFF.md');
  return `${r.branch}  ${bits.join(' | ')}`;
}

function text(m) {
  const L = [];
  const wrap = (s, indent) => {
    const width = 100 - indent.length;
    const words = String(s).split(/\s+/);
    const out = [];
    let line = '';
    for (const w of words) {
      if (line && line.length + 1 + w.length > width) {
        out.push(indent + line);
        line = w;
      } else line = line ? `${line} ${w}` : w;
    }
    if (line) out.push(indent + line);
    return out;
  };
  L.push(`lords2 work ledger -- derived ${stamp(m.generated)} UTC`);
  L.push(`figures and git facts: ${m.base.name} ${m.base.short}`);
  L.push(`ledger: ${m.ledger.label}${m.ledger.checkout ? ` (checkout ${m.ledger.checkout})` : ''}`);
  L.push(
    m.skip
      ? `git half SKIPPED: ${m.skip}; ${m.rows.filter((r) => r.branch).length} rows name a branch and were not compared`
      : `git half ran against ${m.git.branches} local branches (${m.git.agentBranches} agent branches)`,
  );
  L.push(m.problems.length ? `check: ${m.problems.length} disagreement(s) -- listed at the end` : 'check: the ledger agrees with its schema' + (m.skip ? '' : ' and with git'));

  L.push('', 'SYSTEMS');
  for (const t of m.tracks) {
    if (!t.systems.length) continue;
    L.push(`  ${t.name} -- ${t.about}`);
    for (const s of t.systems) {
      const st = listCounts(s.states, m.stateOrder);
      L.push(`    ${s.name.padEnd(14)} ${st ? `rows: ${st}` : 'no rows'}`);
      for (const x of s.inventories) {
        if (x.error) L.push(`      ${x.name}: ERROR ${x.error}`);
        else if (x.absent) L.push(`      ${x.name}: ${x.absent}`);
        else {
          L.push(`      ${x.name}: ${x.head} ${x.unit}${x.pct !== undefined ? ` (${x.pct}%)` : ''}   [${x.source} @ ${x.where}]`);
          for (const s2 of x.second) L.push(...wrap(s2, '        '));
        }
      }
    }
  }

  const section = (title, list, fn) => {
    L.push('', `${title} (${list.length})`);
    if (!list.length) L.push('  none');
    list.forEach(fn);
  };
  const row = (r, prefix = '  ') => {
    L.push(`${prefix}${r.id}  [${r.track}/${r.system}]  ${r.title}`);
    L.push(`${' '.repeat(prefix.length + 4)}${gitLine(m, r, m.facts(r))}`);
    if (r.next) L.push(...wrap(`next: ${r.next}`, ' '.repeat(prefix.length + 4)));
  };
  section('IN FLIGHT', m.inState('in-flight'), (r) => row(r));
  section('MERGE QUEUE, in order', m.queue, (r, i) => row(r, `  ${i + 1}. `));
  section('WAITING ON THE PLAYER', m.inState('awaiting-user'), (r) => row(r));
  section('OPEN', m.inState('open'), (r) => L.push(`  ${r.id}  [${r.track}/${r.system}]  ${r.title}`));
  section('DEFERRED AND ABANDONED', [...m.inState('deferred'), ...m.inState('abandoned')], (r) =>
    L.push(`  ${r.id}  ${r.state}  ${r.title}`, ...(r.note ? wrap(`why: ${r.note}`, '      ') : [])),
  );
  // A dependency whose branch already merged is not blocking anything; the row
  // saying so is stale, and the line says that rather than repeating it.
  const depState = (d) => `${d.id} (${d.state}${m.facts(d) && m.facts(d).merged ? ', but its branch is merged' : ''})`;
  section('BLOCKED BY', m.blocked, (b) => L.push(`  ${b.row.id} waits on ${b.on.map(depState).join(', ')}`));
  if (m.problems.length) {
    L.push('', `DISAGREEMENTS (${m.problems.length})`);
    for (const p of m.problems) L.push(...wrap(`${p.id}: ${p.msg}`, '  '));
  }
  return L.join('\n') + '\n';
}

// ---- html -----------------------------------------------------------------

const esc = (s) =>
  String(s == null ? '' : s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

const STATE_LABEL = {
  'in-flight': 'in flight',
  'queued-merge': 'queued',
  'awaiting-user': 'needs the player',
  open: 'open',
  deferred: 'deferred',
  abandoned: 'abandoned',
};
const chip = (cls, label, title) => `<span class="chip ${esc(cls)}"${title ? ` title="${esc(title)}"` : ''}>${esc(label)}</span>`;
const stateChip = (m, s) => chip(s, STATE_LABEL[s] || s, m.states[s]);

function gitCell(m, r) {
  if (!r.branch) return '<span class="dim">no branch</span>';
  const f = m.facts(r);
  const name = `<span class="branch">${esc(r.branch)}</span>`;
  if (!f) return `${name}<div class="facts">${chip('quiet', 'git not compared', m.skip)}</div>`;
  if (!f.exists) return `${name}<div class="facts">${chip('bad', 'branch missing')}</div>`;
  const bits = [];
  if (f.merged) bits.push(ON_A_BRANCH.includes(r.state) ? chip('bad', 'merged: row is stale') : chip('good', 'merged'));
  else if (f.fresh) bits.push(chip('quiet', 'no commits yet'));
  else {
    bits.push(`<span class="m">${f.ahead} ahead</span>`);
    if (f.own !== f.ahead) bits.push(`<span class="m dim" title="commits beyond the branch it is built on">${f.own} own</span>`);
  }
  if (f.handoff) bits.push(chip('warn', 'HANDOFF.md', 'the branch carries a handoff note: an agent was stopped mid-work'));
  return `${name}<div class="facts">${bits.join('')}</div>`;
}

function lastCell(m, r) {
  const f = m.facts(r);
  if (!f || !f.exists || !f.date) return '<span class="dim">-</span>';
  return `<span class="m">${esc(stamp(f.date))}</span><div class="subject">${esc(f.subject)}</div>`;
}

const work = (r) => `<div class="title">${esc(r.title)}</div><span class="id">${esc(r.id)} &middot; ${esc(r.track)}/${esc(r.system)}</span>`;
const waits = (m, r) => {
  const on = (r.depends_on || []).filter((d) => m.rows.some((x) => x.id === d));
  if (!on.length) return '<span class="dim">-</span>';
  return on
    .map((d) => {
      const x = m.rows.find((y) => y.id === d);
      const f = m.facts(x);
      const stale = f && f.merged ? chip('bad', 'branch merged', 'this dependency has landed; its row should have left the ledger') : '';
      return `<div class="dep"><span class="m">${esc(d)}</span> ${stateChip(m, x.state)}${stale}</div>`;
    })
    .join('');
};
const table = (head, body, cls = '') =>
  `<div class="scroll"><table class="${cls}"><thead><tr>${head.map((h) => `<th>${esc(h)}</th>`).join('')}</tr></thead><tbody>${body}</tbody></table></div>`;
const empty = (s) => `<p class="empty">${esc(s)}</p>`;

// ---- the player's page ----------------------------------------------------
//
// One screen of reading for the player, who is not an agent: the game's
// features as a checklist, what is in flight, what waits on him, and
// everything else folded away. A ledger row is its title and nothing else, and
// every row whose branch has not merged appears exactly once -- a merged one
// is stale, and the disagreement line already counts it. `work_ledger.rs`
// counts both.

const FEATURE_LABEL = { done: 'done', partial: 'partial', missing: 'missing', 'not-assessed': 'not assessed', 'out-of-scope': 'out of scope' };
const PLAYER_TRACKS = ['play', 'screens', 'presentation'];
const TRACK_NAME = {
  play: 'Playing the game',
  screens: 'Screens and controls',
  presentation: 'Sound, films and animation',
  instruments: 'Checks that prove parity',
  process: 'How the project runs',
};
const PARKED = ['open', 'deferred', 'abandoned'];

function placeRow(m, r) {
  const f = m.facts(r);
  if (f && f.merged) return null;
  if (r.state === 'in-flight') return 'now';
  if (r.state === 'queued-merge') return 'next';
  if (r.state === 'awaiting-user') return 'asks';
  return PLAYER_TRACKS.includes(r.track) && PARKED.includes(r.state) ? 'backlog' : 'behind';
}

// Five marks that differ in shape, not only in colour: a filled disc with a
// tick, a half-filled ring, a crossed ring, a dashed ring, and a bar.
const statusMark = (s) => `<svg class="mark" viewBox="0 0 16 16" aria-hidden="true" focusable="false"><use href="#mark-${esc(knownStatus(s))}"></use></svg>`;
const MARK_DEFS = `<svg class="defs" aria-hidden="true" focusable="false"><defs>
<symbol id="mark-done" viewBox="0 0 16 16"><circle cx="8" cy="8" r="7" fill="currentColor"/><path d="M4.7 8.2l2.2 2.3 4.4-4.8" fill="none" style="stroke:var(--raised)" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/></symbol>
<symbol id="mark-partial" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M8 1.8a6.2 6.2 0 0 0 0 12.4z" fill="currentColor"/></symbol>
<symbol id="mark-missing" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M5.8 5.8l4.4 4.4m0-4.4l-4.4 4.4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></symbol>
<symbol id="mark-not-assessed" viewBox="0 0 16 16"><circle cx="8" cy="8" r="6.2" fill="none" stroke="currentColor" stroke-width="1.5" stroke-dasharray="2.2 2.1"/></symbol>
<symbol id="mark-out-of-scope" viewBox="0 0 16 16"><path d="M3.5 8h9" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></symbol>
</defs></svg>`;

function meter(c, cls = '') {
  const parts = ['done', 'partial', 'missing', 'not-assessed'].filter((s) => c[s]);
  if (!parts.length) return '';
  const said = parts.map((s) => `${c[s]} ${FEATURE_LABEL[s]}`).join(', ');
  return `<div class="meter ${cls}" role="img" aria-label="${esc(said)}">${parts.map((s) => `<i class="${s}" style="flex-grow:${c[s]}"></i>`).join('')}</div>`;
}

const featureLine = (f) => {
  const s = knownStatus(f.status);
  const gap = s === 'partial' && f.gap ? `<span class="gap">${esc(f.gap)}</span>` : '';
  return `<li data-feature="${esc(f.id)}" data-status="${esc(f.status)}" class="${s}">${statusMark(s)}<span class="label"><span class="sr">${esc(FEATURE_LABEL[s])}: </span><span class="fname">${esc(f.name)}</span>${gap}</span></li>`;
};

const PLAYER_CSS = `
  /* The tokens are docs/status.html's and the detailed page's, so the pages
     read as one project. Colour is never the only signal: every status mark
     and every line marker also has a shape. */
  :root {
    --ground: #E7E6E1; --surface: #F2F1ED; --raised: #FBFAF8;
    --ink: #1A1C1F; --ink-dim: #575C63; --ink-faint: #80858C;
    --rule: #CECCC4; --rule-soft: #DEDCD5;
    --gules: #A32C33; --or: #8A6512; --azure: #2E5C8A; --vert: #3B6B45;
    --shadow: 0 1px 2px rgba(26,28,31,.06), 0 3px 10px rgba(26,28,31,.04);
    --sans: "Archivo", ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
    --mono: "IBM Plex Mono", ui-monospace, "Cascadia Mono", Consolas, monospace;
    color-scheme: light;
  }
  @media (prefers-color-scheme: dark) {
    :root:not([data-theme="light"]) {
      --ground: #15171B; --surface: #1D2026; --raised: #23272E;
      --ink: #E4E6E9; --ink-dim: #9AA1AA; --ink-faint: #737A83;
      --rule: #333842; --rule-soft: #2A2F37;
      --gules: #E0666D; --or: #D8AA4E; --azure: #6C9FD6; --vert: #79B183;
      --shadow: 0 1px 2px rgba(0,0,0,.4), 0 4px 16px rgba(0,0,0,.22);
      color-scheme: dark;
    }
  }
  :root[data-theme="dark"] {
    --ground: #15171B; --surface: #1D2026; --raised: #23272E;
    --ink: #E4E6E9; --ink-dim: #9AA1AA; --ink-faint: #737A83;
    --rule: #333842; --rule-soft: #2A2F37;
    --gules: #E0666D; --or: #D8AA4E; --azure: #6C9FD6; --vert: #79B183;
    --shadow: 0 1px 2px rgba(0,0,0,.4), 0 4px 16px rgba(0,0,0,.22);
    color-scheme: dark;
  }

  * { box-sizing: border-box; }
  body {
    margin: 0; background: var(--ground); color: var(--ink);
    font-family: var(--sans); font-size: 15px; line-height: 1.5;
    -webkit-font-smoothing: antialiased;
  }
  .wrap { max-width: 1120px; margin: 0 auto; padding-block: 28px 64px; padding-inline: clamp(16px, 4vw, 32px); }
  .defs { position: absolute; width: 0; height: 0; overflow: hidden; }
  .sr { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); white-space: nowrap; }
  a { color: var(--azure); }
  a:focus-visible, summary:focus-visible { outline: 2px solid var(--azure); outline-offset: 3px; }

  /* ---- the header: one line, then one sentence ---- */
  .top { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: 6px 24px; padding-bottom: 12px; border-bottom: 2px solid var(--ink); }
  h1 { margin: 0; font-size: 22px; font-weight: 700; letter-spacing: -.01em; line-height: 1.2; }
  h1 span { font-weight: 500; color: var(--ink-dim); }
  .meta { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 18px; margin: 0; font: 12px/1.5 var(--mono); color: var(--ink-faint); }
  .meta b { color: var(--ink); font-weight: 500; }
  .check { --c: var(--ink-faint); display: inline-flex; align-items: center; gap: 7px; color: var(--c); text-decoration: none; }
  .check::before { content: ""; width: 7px; height: 7px; background: currentColor; flex: none; }
  .check.good { --c: var(--vert); }
  .check.good::before { border-radius: 50%; }
  .check.warn { --c: var(--or); }
  .check.warn::before { transform: rotate(45deg); }
  .check.bad { --c: var(--gules); }
  .check.bad::before { width: 2px; height: 10px; }
  .lede { margin: 10px 0 0; max-width: 70ch; color: var(--ink-dim); text-wrap: pretty; }

  .alert { margin-top: 16px; padding: 8px 14px; border: 1px solid var(--gules); border-radius: 3px; font-size: 13.5px; color: var(--ink-dim); }
  .alert summary { cursor: pointer; color: var(--gules); font-weight: 600; }
  .alert ul { margin: 8px 0 4px; padding-left: 18px; display: grid; gap: 6px; overflow-wrap: anywhere; }
  .alert b { color: var(--ink); font: 500 12px var(--mono); }

  /* ---- sections ---- */
  .block { margin-top: 36px; }
  .section-head { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: 4px 16px; padding-bottom: 8px; border-bottom: 1px solid var(--rule); }
  h2 { margin: 0; font: 600 12px/1.3 var(--mono); letter-spacing: .14em; text-transform: uppercase; color: var(--ink-dim); }
  .aside, .n { font: 400 12px/1.3 var(--mono); letter-spacing: 0; text-transform: none; color: var(--ink-faint); font-variant-numeric: tabular-nums; }

  /* ---- the checklist ---- */
  .tally-row { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: 8px 28px; margin-top: 18px; }
  .tally { margin: 0; font-size: 17px; color: var(--ink-dim); }
  .tally b { margin-right: 4px; font: 600 36px/1 var(--mono); letter-spacing: -.02em; color: var(--ink); font-variant-numeric: tabular-nums; }
  .legend { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 6px 18px; font-size: 13px; color: var(--ink-dim); }
  .legend li { display: inline-flex; align-items: center; gap: 6px; }
  .legend b { font: 600 12.5px var(--mono); color: var(--ink); font-variant-numeric: tabular-nums; }

  .mark { width: 15px; height: 15px; flex: none; color: var(--ink-faint); }
  li.done .mark { color: var(--vert); }
  li.partial .mark { color: var(--or); }
  li.missing .mark { color: var(--gules); }

  .meter { display: flex; gap: 2px; height: 4px; margin: 7px 0 9px; }
  .meter.wide { height: 10px; margin: 14px 0 0; }
  .meter i { flex: 1 1 0; min-width: 2px; border-radius: 1px; }
  .meter i.done { background: var(--vert); }
  .meter i.partial { background: var(--or); }
  .meter i.missing { background: var(--gules); }
  .meter i.not-assessed { background: repeating-linear-gradient(135deg, var(--ink-faint) 0 1.5px, transparent 1.5px 4px); }

  .areas { columns: 3 290px; column-gap: 44px; margin-top: 28px; }
  .area { break-inside: avoid; padding-bottom: 24px; }
  .area h3 { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; margin: 0; font-size: 15px; font-weight: 600; text-wrap: balance; }
  .checklist { list-style: none; margin: 0; padding: 0; display: grid; gap: 4px; }
  .checklist li { display: flex; align-items: flex-start; gap: 8px; font-size: 14px; line-height: 1.35; }
  .checklist .mark { margin-top: 1px; }
  .checklist .label { min-width: 0; }
  .checklist .gap { margin-left: 7px; font-size: 12.5px; color: var(--ink-faint); }
  .checklist li.not-assessed .fname, .checklist li.out-of-scope .fname { color: var(--ink-dim); }

  /* ---- the work ---- */
  .work { display: grid; grid-template-columns: minmax(0, 1.5fr) minmax(0, 1fr); gap: 0 48px; align-items: start; margin-top: 12px; }
  .work > section { margin-top: 24px; }
  .work h3, .track h3 { display: flex; align-items: baseline; gap: 10px; margin: 16px 0 8px; font: 600 11.5px/1.3 var(--mono); letter-spacing: .1em; text-transform: uppercase; color: var(--ink-faint); }
  .asks { padding: 14px 18px 18px; background: var(--raised); border: 1px solid var(--rule-soft); border-radius: 3px; box-shadow: var(--shadow); }
  .asks .lines, .asks .none { margin-top: 12px; }
  @media (max-width: 760px) { .work { grid-template-columns: minmax(0, 1fr); } }

  .lines { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; font-size: 14px; line-height: 1.4; }
  .lines li { position: relative; padding-left: 20px; text-wrap: pretty; }
  .lines li::before { content: ""; position: absolute; left: 3px; top: .45em; width: 7px; height: 7px; background: var(--ink-faint); }
  .lines.flight li::before { border-radius: 50%; background: var(--or); }
  .lines.ask li::before { background: var(--gules); transform: rotate(45deg) scale(.95); }
  .lines.queue { counter-reset: q; }
  .lines.queue li { counter-increment: q; padding-left: 26px; }
  .lines.queue li::before { content: counter(q); top: 0; left: 0; width: auto; height: auto; background: none; font: 600 12.5px/1.55 var(--mono); color: var(--azure); }
  .lines.parked li::before { background: transparent; box-shadow: inset 0 0 0 1.5px var(--ink-faint); border-radius: 50%; }
  .lines.parked li.st-deferred::before, .lines.parked li.st-abandoned::before { top: .7em; height: 2px; background: var(--ink-faint); box-shadow: none; border-radius: 0; }
  .lines.figures li::before { top: .7em; width: 9px; height: 2px; }
  .lines li.bad { color: var(--gules); }
  .lines b { font: 600 13.5px var(--mono); font-variant-numeric: tabular-nums; }
  .tag { margin-left: 6px; padding: 1px 5px; border: 1px solid var(--rule); border-radius: 2px; font: 600 10px/1.4 var(--mono); letter-spacing: .06em; text-transform: uppercase; color: var(--ink-faint); white-space: nowrap; }
  .tag.deferred { border-style: dashed; }
  .tag.abandoned { text-decoration: line-through; }
  .none { margin: 0; font-size: 13.5px; color: var(--ink-faint); }
  .note { margin: 16px 0 0; padding: 12px 14px; border: 1px dashed var(--rule); border-radius: 3px; font-size: 13.5px; color: var(--ink-dim); }
  .note.bad { border-color: var(--gules); color: var(--gules); }

  /* ---- folded ---- */
  .fold { margin-top: 32px; border-top: 1px solid var(--rule); }
  .fold + .fold { margin-top: 0; }
  .fold > summary { display: flex; flex-wrap: wrap; align-items: baseline; gap: 4px 14px; padding: 14px 0; cursor: pointer; list-style: none; }
  .fold > summary::-webkit-details-marker { display: none; }
  .fold > summary::before { content: ""; align-self: center; width: 0; height: 0; border-style: solid; border-width: 5px 0 5px 7px; border-color: transparent transparent transparent var(--ink-dim); transition: transform .15s; }
  .fold[open] > summary::before { transform: rotate(90deg); }
  .fold-title { font: 600 12px/1.3 var(--mono); letter-spacing: .14em; text-transform: uppercase; color: var(--ink-dim); }
  .fold-meta { font-size: 13px; color: var(--ink-faint); }
  .fold-body { padding: 0 0 18px 21px; }
  .tracks { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 280px), 1fr)); gap: 4px 44px; }
  .track h3 { margin-top: 6px; }
  .sources { margin: 20px 0 0; font: 12px/1.6 var(--mono); color: var(--ink-faint); overflow-wrap: anywhere; }
  @media (max-width: 520px) { .fold-body { padding-left: 0; } }
  @media (prefers-reduced-motion: reduce) { .fold > summary::before { transition: none; } }
`;

function html(m) {
  const inPlace = (p) => m.rows.filter((r) => placeRow(m, r) === p);
  const now = inPlace('now');
  const next = m.queue.filter((r) => placeRow(m, r) === 'next');
  const asks = inPlace('asks');
  const backlog = inPlace('backlog');
  const behind = inPlace('behind');
  const refName = `${m.base.name} ${m.base.short}`;
  const F = m.features;

  const problems = [...m.problems, ...(F.problems || []).map((p) => ({ id: p.id === '(file)' ? 'features.json' : `feature ${p.id}`, msg: p.msg }))];
  const check = problems.length
    ? `<a class="check bad" href="#disagreements">${problems.length} disagreement${problems.length === 1 ? '' : 's'}</a>`
    : m.skip
      ? `<span class="check warn" title="${esc(m.skip)}">git not compared</span>`
      : '<span class="check good">ledger agrees with git</span>';

  const title = (r) => `<li data-row="${esc(r.id)}">${esc(r.title)}</li>`;
  const lines = (list, cls, none) => {
    const tag = cls === 'queue' ? 'ol' : 'ul';
    return list.length ? `<${tag} class="lines ${cls}">${list.map(title).join('')}</${tag}>` : `<p class="none">${esc(none)}</p>`;
  };
  const parked = (r) =>
    `<li data-row="${esc(r.id)}" class="st-${esc(r.state)}">${esc(r.title)}${r.state === 'open' ? '' : ` <span class="tag ${esc(r.state)}">${esc(STATE_LABEL[r.state] || r.state)}</span>`}</li>`;
  const trackOrder = [...new Set([...m.tracks.map((t) => t.name), ...m.rows.map((r) => r.track)])];
  const byTrack = (list) => trackOrder.map((t) => ({ t, rows: list.filter((r) => r.track === t) })).filter((g) => g.rows.length);
  const trackName = (t) => TRACK_NAME[t] || String(t);
  const trackBlocks = (list) =>
    byTrack(list)
      .map((g) => `<section class="track"><h3>${esc(trackName(g.t))} <span class="n">${g.rows.length}</span></h3><ul class="lines parked">${g.rows.map(parked).join('')}</ul></section>`)
      .join('');

  let featureBody;
  if (F.absent) featureBody = `<p class="note">There is no feature checklist to show: ${esc(F.absent)}.</p>`;
  else if (F.error) featureBody = `<p class="note bad">The feature checklist could not be read: ${esc(F.error)}</p>`;
  else {
    const c = F.counts;
    const legend = FEATURE_STATUSES.map((s) => `<li class="${s}">${statusMark(s)}${esc(FEATURE_LABEL[s])} <b>${c[s]}</b></li>`).join('');
    const areas = F.areas
      .map((a) => {
        const ac = featureCounts(a.features);
        return `<section class="area"><h3><span>${esc(a.name)}</span><span class="n">${ac.done} of ${ac.graded}</span></h3>${meter(ac)}<ul class="checklist">${a.features.map(featureLine).join('')}</ul></section>`;
      })
      .join('');
    featureBody = `<div class="tally-row"><p class="tally" data-done="${c.done}" data-of="${c.graded}"><b>${c.done}</b> of ${c.graded} features done</p><ul class="legend">${legend}</ul></div>
    ${meter(c, 'wide')}
    <div class="areas">${areas}</div>`;
  }

  const figures = m.inv
    .filter((x) => x.system !== 'tests')
    .map((x) =>
      x.error
        ? `<li class="bad">${esc(x.name)}: could not be read</li>`
        : x.absent
          ? `<li>${esc(x.name)}: not on ${esc(refName)}</li>`
          : `<li>${esc(x.name)}: <b>${esc(x.head)}</b> ${esc(x.unit)}</li>`,
    )
    .join('');
  const unrec = F.unrecorded || [];
  const backlogMeta = byTrack(backlog)
    .map((g) => ` &middot; ${g.rows.length} ${esc(trackName(g.t).toLowerCase())}`)
    .join('');

  return `<title>lords2 Status</title>
<meta name="generator" content="tools/pm/work.js --html">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Archivo:wght@400;500;600;700&family=IBM+Plex+Mono:wght@400;500;600&display=swap">
<style>${PLAYER_CSS}</style>
${MARK_DEFS}
<div class="wrap">
  <header class="top">
    <h1>lords2 <span>status</span></h1>
    <p class="meta"><span>${esc(m.base.name)} <b>${esc(m.base.short)}</b></span><span>generated <b>${esc(stamp(m.generated))} UTC</b></span>${check}</p>
  </header>
  <p class="lede">How close this rebuild of Lords of the Realm II is to the original game, feature by feature, and what is being worked on.</p>
  ${
    problems.length
      ? `<details class="alert" id="disagreements"><summary>${problems.length} place${problems.length === 1 ? '' : 's'} where the ledger, the checklist and git disagree</summary><ul>${problems.map((p) => `<li><b>${esc(p.id)}</b> ${esc(p.msg)}</li>`).join('')}</ul></details>`
      : ''
  }

  <section class="block" aria-labelledby="h-features">
    <div class="section-head"><h2 id="h-features">Game features</h2><span class="aside">graded against the original</span></div>
    ${featureBody}
  </section>

  <div class="work">
    <section aria-labelledby="h-now">
      <div class="section-head"><h2 id="h-now">Now</h2></div>
      <h3>In progress <span class="n">${now.length}</span></h3>
      ${lines(now, 'flight', 'No agent is working on anything.')}
      <h3>Up next <span class="n">merging in this order</span></h3>
      ${lines(next, 'queue', 'Nothing is waiting to merge.')}
    </section>
    <section class="asks" aria-labelledby="h-asks">
      <div class="section-head"><h2 id="h-asks">Waiting on you</h2><span class="n">${asks.length}</span></div>
      ${lines(asks, 'ask', 'Nothing is waiting on you.')}
    </section>
  </div>

  <details class="fold">
    <summary><span class="fold-title">Backlog</span><span class="fold-meta">${backlog.length} known, not started${backlogMeta}</span></summary>
    <div class="fold-body tracks">${backlog.length ? trackBlocks(backlog) : '<p class="none">The backlog is empty.</p>'}</div>
  </details>

  <details class="fold">
    <summary><span class="fold-title">Behind the scenes</span><span class="fold-meta">the numbers, the checks and the tooling</span></summary>
    <div class="fold-body">
      <div class="tracks">
        <section class="track"><h3>The numbers <span class="n">${esc(refName)}</span></h3><ul class="lines figures">${figures}</ul></section>
        <section class="track"><h3>Gaps with no ledger row <span class="n">${unrec.length}</span></h3>${
          unrec.length
            ? `<ul class="lines parked">${unrec.map((f) => `<li>${esc(f.name)} <span class="tag">${esc(FEATURE_LABEL[knownStatus(f.status)])}</span></li>`).join('')}</ul>`
            : '<p class="none">Every gap has a ledger row.</p>'
        }</section>
        ${trackBlocks(behind)}
      </div>
      <p class="sources">Ledger: ${esc(m.ledger.label)}. Checklist: ${esc(F.label || FEATURES_IN_REF)}. Figures: ${esc(m.base.name)} ${esc(m.base.sha)}.${m.skip ? ` Git not compared: ${esc(m.skip)}.` : ''} Regenerate with node tools/pm/work.js --html; every row in full is --html-detail.</p>
    </div>
  </details>
</div>
`;
}

// ---- the detailed page, for agents ----------------------------------------

function htmlDetail(m) {
  const inflight = m.inState('in-flight');
  const waiting = m.inState('awaiting-user');
  const open = m.inState('open');
  const parked = [...m.inState('deferred'), ...m.inState('abandoned')];
  const refName = `${m.base.name} ${m.base.short}`;
  const led = m.ledger;
  const verdict = m.problems.length
    ? `<a class="chip bad" href="#disagreements">${m.problems.length} disagreement${m.problems.length === 1 ? '' : 's'}</a>`
    : m.skip
      ? chip('warn', 'schema clean, git not compared', m.skip)
      : chip('good', 'agrees with git');
  const ledgerMeta = led.checkout
    ? `<span>ledger <b>${esc(led.path)}</b> on <b>${esc(led.branch)}</b>${led.commit ? ` @ <b>${esc(led.commit)}</b>` : ''}</span>${
        led.state === 'modified' ? chip('warn', 'uncommitted changes', led.label) : led.state === 'uncommitted' ? chip('warn', 'uncommitted file', led.label) : ''
      }`
    : `<span>ledger <b>${esc(led.path)}</b></span>${chip('warn', 'outside git', led.label)}`;
  const ledgerLede = led.checkout
    ? `<code>${esc(led.path)}</code> on <code>${esc(led.branch)}</code>${
        led.state === 'committed' ? `, last committed in <code>${esc(led.commit)}</code>` : led.state === 'modified' ? `, with uncommitted changes since <code>${esc(led.commit)}</code>` : ', an uncommitted file'
      }`
    : `<code>${esc(led.path)}</code>, outside any git checkout`;

  const gauges = (s) =>
    s.inventories
      .map((x) => {
        const from = `<div class="src">${esc(x.source)} @ ${esc(x.where)}</div>`;
        if (x.error) {
          return `<div class="gauge error"><div class="k"><span>${esc(x.name)}</span></div><div class="v">unreadable</div><p class="why">${esc(x.error)}</p>${from}</div>`;
        }
        if (x.absent) {
          return `<div class="gauge absent"><div class="k"><span>${esc(x.name)}</span></div><p class="why">${esc(x.absent)}</p>${from}</div>`;
        }
        const bar = x.pct !== undefined ? `<div class="bar" role="img" aria-label="${x.pct} percent"><i style="width:${Math.max(0, Math.min(100, x.pct))}%"></i></div>` : '';
        return `<div class="gauge"><div class="k"><span>${esc(x.name)}</span>${x.pct !== undefined ? `<span>${x.pct}%</span>` : ''}</div>
<div class="v">${esc(x.head)}<small>${esc(x.unit)}</small></div>${bar}
<ul>${x.second.map((l) => `<li>${esc(l)}</li>`).join('')}</ul>${from}</div>`;
      })
      .join('');

  const systems = m.tracks
    .filter((t) => t.systems.length)
    .map((t) => {
      const withInv = t.systems.filter((s) => s.inventories.length);
      const list = t.systems
        .map(
          (s) =>
            `<li><span class="name">${esc(s.name)}</span>${
              Object.keys(s.states).length
                ? m.stateOrder.filter((st) => s.states[st]).map((st) => `<span class="count ${esc(st)}" title="${esc(STATE_LABEL[st] || st)}">${s.states[st]} ${esc(STATE_LABEL[st] || st)}</span>`).join('')
                : '<span class="count none">no rows</span>'
            }</li>`,
        )
        .join('');
      return `<div class="track"><div><div class="track-name">${esc(t.name)}</div><div class="track-about">${esc(t.about)}</div></div>
<div>${withInv.length ? `<div class="gauges">${withInv.map(gauges).join('')}</div>` : ''}<ul class="systems">${list}</ul></div></div>`;
    })
    .join('');

  const openByTrack = m.tracks
    .map((t) => {
      const of = open.filter((r) => r.track === t.name);
      if (!of.length) return '';
      return `<tr class="group"><th colspan="4">${esc(t.name)} <span>${of.length}</span></th></tr>${of
        .map((r) => `<tr><td>${work(r)}</td><td class="prose">${esc(r.source) || '<span class="dim">-</span>'}</td><td class="prose">${esc(r.note) || '<span class="dim">-</span>'}</td><td>${waits(m, r)}</td></tr>`)
        .join('')}`;
    })
    .join('');

  return `<title>lords2 Work Ledger</title>
<meta name="generator" content="tools/pm/work.js">
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Archivo:wght@400;500;600;700&family=IBM+Plex+Mono:wght@400;500;600&display=swap">
<style>
  /* The tokens are docs/status.html's, so the two pages read as one project.
     Semantic colour, never the only signal: every chip also has a shape. */
  :root {
    --ground: #E7E6E1; --surface: #F2F1ED; --raised: #FBFAF8;
    --ink: #1A1C1F; --ink-dim: #575C63; --ink-faint: #80858C;
    --rule: #CECCC4; --rule-soft: #DEDCD5;
    --gules: #A32C33; --or: #8A6512; --azure: #2E5C8A; --vert: #3B6B45;
    --shadow: 0 1px 2px rgba(26,28,31,.06), 0 3px 10px rgba(26,28,31,.04);
    --sans: "Archivo", ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif;
    --mono: "IBM Plex Mono", ui-monospace, "Cascadia Mono", Consolas, monospace;
    color-scheme: light;
  }
  @media (prefers-color-scheme: dark) {
    :root:not([data-theme="light"]) {
      --ground: #15171B; --surface: #1D2026; --raised: #23272E;
      --ink: #E4E6E9; --ink-dim: #9AA1AA; --ink-faint: #737A83;
      --rule: #333842; --rule-soft: #2A2F37;
      --gules: #E0666D; --or: #D8AA4E; --azure: #6C9FD6; --vert: #79B183;
      --shadow: 0 1px 2px rgba(0,0,0,.4), 0 4px 16px rgba(0,0,0,.22);
      color-scheme: dark;
    }
  }
  :root[data-theme="dark"] {
    --ground: #15171B; --surface: #1D2026; --raised: #23272E;
    --ink: #E4E6E9; --ink-dim: #9AA1AA; --ink-faint: #737A83;
    --rule: #333842; --rule-soft: #2A2F37;
    --gules: #E0666D; --or: #D8AA4E; --azure: #6C9FD6; --vert: #79B183;
    --shadow: 0 1px 2px rgba(0,0,0,.4), 0 4px 16px rgba(0,0,0,.22);
    color-scheme: dark;
  }

  * { box-sizing: border-box; }
  body {
    margin: 0; background: var(--ground); color: var(--ink);
    font-family: var(--sans); font-size: 15px; line-height: 1.55;
    -webkit-font-smoothing: antialiased;
  }
  .wrap { max-width: 1140px; margin: 0 auto; padding-block: 40px 72px; padding-inline: clamp(16px, 4vw, 28px); }
  code, .m { font-family: var(--mono); font-variant-numeric: tabular-nums; }
  .m { font-size: 12px; }
  .dim { color: var(--ink-faint); }
  a { color: var(--azure); }
  a:focus-visible { outline: 2px solid var(--azure); outline-offset: 2px; }

  .eyebrow { font: 500 11px/1.4 var(--mono); letter-spacing: .16em; text-transform: uppercase; color: var(--ink-faint); margin: 0 0 10px; }
  h1 { font-size: clamp(28px, 4vw, 40px); font-weight: 700; letter-spacing: -.02em; line-height: 1.1; margin: 0; text-wrap: balance; }
  .lede { margin: 12px 0 0; max-width: 66ch; color: var(--ink-dim); }
  .metabar {
    display: flex; flex-wrap: wrap; align-items: center; gap: 8px 22px;
    margin-top: 22px; padding-top: 16px; border-top: 1px solid var(--rule);
    font: 12px/1.5 var(--mono); color: var(--ink-faint);
  }
  .metabar b { color: var(--ink); font-weight: 500; }

  h2 {
    display: flex; align-items: baseline; gap: 10px;
    font: 600 12px/1.3 var(--mono); letter-spacing: .14em; text-transform: uppercase;
    color: var(--ink-faint); margin: 52px 0 14px; padding-bottom: 9px; border-bottom: 1px solid var(--rule);
  }
  h2 .n { color: var(--ink-dim); font-weight: 500; letter-spacing: .02em; }
  .intro { margin: -2px 0 14px; max-width: 72ch; color: var(--ink-dim); font-size: 13.5px; }

  /* ---- chips: colour and shape ---- */
  .chip {
    --c: var(--ink-faint);
    display: inline-flex; align-items: center; gap: 6px;
    font: 600 10.5px/1 var(--mono); letter-spacing: .06em; text-transform: uppercase;
    padding: 4px 7px 4px 6px; border-radius: 2px; white-space: nowrap; text-decoration: none;
    color: var(--c); background: color-mix(in srgb, var(--c) 14%, transparent);
  }
  .chip::before { content: ""; width: 6px; height: 6px; background: currentColor; flex: none; }
  .chip.in-flight { --c: var(--or); }
  .chip.in-flight::before { border-radius: 50%; }
  .chip.queued-merge { --c: var(--azure); }
  .chip.awaiting-user { --c: var(--gules); }
  .chip.awaiting-user::before { transform: rotate(45deg) scale(.9); }
  .chip.open::before { background: transparent; box-shadow: inset 0 0 0 1.5px currentColor; border-radius: 50%; }
  .chip.deferred::before { height: 2px; }
  .chip.abandoned { text-decoration: line-through; }
  .chip.abandoned::before { height: 2px; }
  .chip.good { --c: var(--vert); }
  .chip.warn { --c: var(--or); }
  .chip.warn::before { transform: rotate(45deg) scale(.9); }
  .chip.bad { --c: var(--gules); }
  .chip.bad::before { width: 2px; height: 9px; }
  .chip.quiet { --c: var(--ink-faint); }
  .chip.quiet::before { background: transparent; box-shadow: inset 0 0 0 1.5px currentColor; }

  /* ---- systems ---- */
  .track { display: grid; grid-template-columns: 190px minmax(0, 1fr); gap: 12px 28px; padding: 20px 0; border-bottom: 1px solid var(--rule-soft); }
  .track:last-child { border-bottom: 0; }
  .track-name { font: 600 12.5px/1.3 var(--mono); letter-spacing: .08em; text-transform: uppercase; }
  .track-about { margin-top: 5px; font-size: 12.5px; line-height: 1.45; color: var(--ink-dim); }
  .gauges { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 330px), 1fr)); gap: 12px; margin-bottom: 14px; }
  .gauge { background: var(--raised); border: 1px solid var(--rule-soft); border-radius: 3px; padding: 14px 16px 12px; box-shadow: var(--shadow); }
  .gauge .k { display: flex; justify-content: space-between; gap: 8px; font: 500 11px/1.3 var(--mono); letter-spacing: .08em; text-transform: uppercase; color: var(--ink-faint); }
  .gauge .v { margin-top: 6px; font: 600 25px/1.2 var(--mono); font-variant-numeric: tabular-nums; letter-spacing: -.01em; }
  .gauge .v small { display: block; margin-top: 2px; font: 400 12.5px/1.4 var(--sans); letter-spacing: 0; color: var(--ink-dim); }
  .bar { height: 5px; margin-top: 10px; background: var(--rule-soft); border-radius: 99px; overflow: hidden; }
  .bar > i { display: block; height: 100%; background: var(--vert); border-radius: 99px; }
  .gauge ul { list-style: none; margin: 12px 0 0; padding: 10px 0 0; border-top: 1px dashed var(--rule-soft); display: grid; gap: 6px; font-size: 12.5px; line-height: 1.45; color: var(--ink-dim); }
  .gauge .src, .gauge .why { font: 11px/1.4 var(--mono); color: var(--ink-faint); margin: 10px 0 0; }
  .gauge.absent { background: transparent; box-shadow: none; border-style: dashed; }
  .gauge.error { border-color: var(--gules); }
  .gauge.error .v { color: var(--gules); }
  .systems { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 6px 20px; }
  .systems li { display: inline-flex; align-items: baseline; flex-wrap: wrap; gap: 4px 8px; }
  .systems .name { font: 500 12.5px var(--mono); color: var(--ink); }
  .count { font: 11.5px var(--mono); color: var(--ink-dim); font-variant-numeric: tabular-nums; }
  .count + .count::before { content: "\\00b7"; margin-right: 8px; color: var(--ink-faint); }
  .count.in-flight { color: var(--or); }
  .count.queued-merge { color: var(--azure); }
  .count.awaiting-user { color: var(--gules); }
  .count.none { color: var(--ink-faint); }

  /* ---- tables ---- */
  .scroll { overflow-x: auto; border: 1px solid var(--rule-soft); border-radius: 3px; background: var(--raised); }
  table { border-collapse: collapse; width: 100%; min-width: 820px; font-size: 13.5px; }
  th, td { text-align: left; vertical-align: top; padding: 11px 14px; border-bottom: 1px solid var(--rule-soft); }
  thead th { font: 600 10.5px/1.3 var(--mono); letter-spacing: .1em; text-transform: uppercase; color: var(--ink-faint); background: var(--surface); white-space: nowrap; }
  tbody tr:last-child td { border-bottom: 0; }
  tr.group th { background: var(--surface); font: 600 11px/1.3 var(--mono); letter-spacing: .1em; text-transform: uppercase; color: var(--ink-dim); padding-block: 8px; }
  tr.group th span { color: var(--ink-faint); font-weight: 500; margin-left: 6px; }
  td:first-child { width: 30%; }
  .title { font-weight: 500; line-height: 1.4; text-wrap: pretty; }
  .id { display: block; margin-top: 3px; font: 11.5px/1.4 var(--mono); color: var(--ink-faint); }
  .branch { display: block; font: 11.5px/1.4 var(--mono); color: var(--ink-dim); word-break: break-all; }
  .facts { display: flex; flex-wrap: wrap; align-items: center; gap: 5px 8px; margin-top: 6px; }
  .subject { margin-top: 3px; font-size: 12.5px; line-height: 1.4; color: var(--ink-dim); max-width: 34ch; }
  .prose { font-size: 13px; line-height: 1.5; color: var(--ink-dim); max-width: 46ch; }
  .dep { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 8px; }
  .dep + .dep { margin-top: 6px; }
  td.pos { width: 1%; font: 600 14px/1.4 var(--mono); color: var(--azure); font-variant-numeric: tabular-nums; }
  table.queue td:nth-child(2) { width: 28%; }
  .empty { margin: 0; padding: 14px 16px; border: 1px dashed var(--rule); border-radius: 3px; color: var(--ink-faint); font-size: 13.5px; }
  .problems td:first-child { width: 26%; font: 500 12px/1.45 var(--mono); color: var(--gules); word-break: break-all; }
  .skip { margin: 0 0 12px; padding: 10px 14px; border: 1px dashed var(--or); border-radius: 3px; font-size: 13px; color: var(--ink-dim); }

  footer { margin-top: 56px; padding-top: 16px; border-top: 1px solid var(--rule); font: 12px/1.6 var(--mono); color: var(--ink-faint); overflow-wrap: anywhere; }
  @media (max-width: 720px) {
    .track { grid-template-columns: minmax(0, 1fr); }
  }
</style>

<div class="wrap">
  <header>
    <p class="eyebrow">lords2 &middot; the work ledger, derived</p>
    <h1>What is in flight, and what is left</h1>
    <p class="lede">Each row is intent the lead wrote in ${ledgerLede}. Everything beside it that git can answer &mdash; merged, commits ahead, a handoff note &mdash; and every systems figure was computed from <code>${esc(refName)}</code> when this page was generated. None of it is read from a working tree, and none of it is stored. If the time below is old, regenerate the page rather than trust it.</p>
    <div class="metabar">
      <span>generated <b>${esc(stamp(m.generated))} UTC</b></span>
      <span>${esc(m.base.name)} <b>${esc(m.base.short)}</b></span>
      ${ledgerMeta}
      <span>${m.rows.length} rows</span>
      ${verdict}
    </div>
  </header>

  <h2>Systems</h2>
  <p class="intro">Read from <code>${esc(refName)}</code>&rsquo;s copies of the inventories that are themselves checked, never typed and never from a working tree. Each figure is quoted with its second column, because a count weights every row equally and a player does not.</p>
  ${systems}

  <h2>In flight <span class="n">${inflight.length}</span></h2>
  ${m.skip ? `<p class="skip">Git was not compared: ${esc(m.skip)}.</p>` : ''}
  ${inflight.length ? table(['Work', 'Branch', 'Last commit', 'Next'], inflight.map((r) => `<tr><td>${work(r)}</td><td>${gitCell(m, r)}</td><td>${lastCell(m, r)}</td><td class="prose">${esc(r.next) || '<span class="dim">-</span>'}</td></tr>`).join('')) : empty('No agent is working on anything.')}

  <h2>Merge queue <span class="n">${m.queue.length}</span></h2>
  <p class="intro">In the order they merge: the ledger&rsquo;s order, except that nothing merges ahead of a queued row it depends on.</p>
  ${m.queue.length ? table(['#', 'Work', 'Branch', 'Waits on', 'Next'], m.queue.map((r, i) => `<tr><td class="pos">${i + 1}</td><td>${work(r)}</td><td>${gitCell(m, r)}</td><td>${waits(m, r)}</td><td class="prose">${esc(r.next) || '<span class="dim">-</span>'}</td></tr>`).join(''), 'queue') : empty('Nothing is waiting to merge.')}

  <h2>Waiting on the player <span class="n">${waiting.length}</span></h2>
  ${waiting.length ? table(['Work', 'What is asked', 'Branch', 'Note'], waiting.map((r) => `<tr><td>${work(r)}</td><td class="prose">${esc(r.next) || '<span class="dim">-</span>'}</td><td>${gitCell(m, r)}</td><td class="prose">${esc(r.note) || '<span class="dim">-</span>'}</td></tr>`).join('')) : empty('Nothing is waiting on a decision or an observation.')}

  <h2>Open backlog <span class="n">${open.length}</span></h2>
  <p class="intro">Known, understood, and not started, by track.</p>
  ${open.length ? table(['Work', 'Where it came from', 'Note', 'Waits on'], openByTrack) : empty('The backlog is empty.')}

  <h2>Deferred and abandoned <span class="n">${parked.length}</span></h2>
  ${parked.length ? table(['Work', 'State', 'Why', 'Branch'], parked.map((r) => `<tr><td>${work(r)}</td><td>${stateChip(m, r.state)}</td><td class="prose">${esc(r.note) || '<span class="dim">-</span>'}</td><td>${gitCell(m, r)}</td></tr>`).join('')) : empty('Nothing is parked.')}

  <h2 id="disagreements">Where the ledger disagrees <span class="n">${m.problems.length}</span></h2>
  ${m.problems.length ? table(['Row or branch', 'What the schema or git says'], m.problems.map((p) => `<tr><td>${esc(p.id)}</td><td class="prose">${esc(p.msg)}</td></tr>`).join(''), 'problems') : empty(m.skip ? `The schema is clean. Git was not compared: ${m.skip}.` : 'The ledger agrees with its schema and with git.')}

  <footer>Generated by <code>node tools/pm/work.js --html-detail</code>. Ledger: ${esc(led.label)}${led.checkout ? ` (checkout ${esc(led.checkout)})` : ''}. Figures and git facts: ${esc(m.base.name)} ${esc(m.base.sha)}. Regenerate rather than edit: nothing on this page is stored anywhere.</footer>
</div>
`;
}

// ---- main -----------------------------------------------------------------

function main() {
  const pages = ['--html', '--html-detail'];
  if (!has('--check') && !has('--status') && !pages.some(has)) {
    console.error('usage: node tools/pm/work.js --check [--schema] | --status | --html <path> | --html-detail <path>   [--file <ledger>] [--features <list>] [--ref <ref>]');
    process.exit(2);
  }
  for (const p of pages) {
    if (has(p) && !opt(p)) {
      console.error(`work: ${p} needs a path to write, outside the tree or gitignored -- generated HTML is never committed`);
      process.exit(2);
    }
  }

  if (has('--check')) {
    const { j, problems: loadProblems } = load();
    if (!j) {
      for (const p of loadProblems) console.error(`work: ${p.id}: ${p.msg}`);
      process.exit(1);
    }
    const problems = schema(j);
    const rows = (Array.isArray(j.items) ? j.items : []).filter((r) => r && typeof r.id === 'string');
    const named = rows.filter((r) => r.branch).length;
    console.log(`work: schema: ${rows.length} rows, ${problems.length} problem(s)`);
    if (has('--schema')) {
      console.log(`work: SKIP git agreement: --schema was given; ${named} rows name a branch and were not compared`);
    } else {
      const g = gitFacts(rows, resolveBase());
      if (g.skip) console.log(`work: SKIP git agreement: ${g.skip}; ${named} rows name a branch and were not compared`);
      else {
        const found = agreement(rows, g);
        console.log(`work: git: ${named} row branches and ${g.agentBranches} agent branches compared against ${g.base.name} ${g.base.short}, ${found.length} problem(s)`);
        problems.push(...found);
      }
    }
    const featureProblems = checkFeatures(rows);
    for (const p of problems) console.error(`work: ${p.id}: ${p.msg}`);
    for (const p of featureProblems) console.error(`work: ${p.id === '(file)' ? 'features' : `feature ${p.id}`}: ${p.msg}`);
    process.exit(problems.length || featureProblems.length ? 1 : 0);
  }

  const m = derive();
  if (m.fatal) {
    for (const p of m.fatal) console.error(`work: ${p.id}: ${p.msg}`);
    process.exit(1);
  }
  const unreadable = m.inv.filter((x) => x.error);
  if (has('--status')) process.stdout.write(text(m));
  const write = (flag, render, what) => {
    if (!has(flag)) return;
    const out = path.resolve(opt(flag));
    fs.mkdirSync(path.dirname(out), { recursive: true });
    fs.writeFileSync(out, render(m));
    console.error(`work: wrote ${out} (${m.base.name} ${m.base.short}, ${m.rows.length} rows, ${m.problems.length} disagreement(s)${what})`);
  };
  const F = m.features;
  write('--html', html, F.list ? `, ${F.list.length} features, ${F.problems.length} feature problem(s)` : `, no features: ${F.absent || F.error}`);
  write('--html-detail', htmlDetail, '');
  for (const x of unreadable) console.error(`work: rollup: ${x.name} could not be read: ${x.error}`);
  const featuresBroken = has('--html') && F.error;
  if (featuresBroken) console.error(`work: features: ${F.error}`);
  process.exit(unreadable.length || featuresBroken ? 1 : 0);
}

main();
