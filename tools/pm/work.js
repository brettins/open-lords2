#!/usr/bin/env node
// The work ledger's tooling: a check that `docs/work.json` agrees with git, and
// the view derived from it.
//
//   node tools/pm/work.js --check             schema, then agreement with git
//   node tools/pm/work.js --check --schema    schema only -- what the test runs
//   node tools/pm/work.js --status            the derived view, as text
//   node tools/pm/work.js --html <path>       the same view as one page
//   ...                        --file <path>  read another ledger (ablations)
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
// merged into `main` (merged work leaves the file); no unmerged
// `worktree-agent-*` or `wip/*` branch with commits ahead of `main` goes without
// a row; no `queued-merge` branch carries `HANDOFF.md`.
//
// A fresh clone -- a CI runner -- has no agent branches, and comparing the
// ledger against refs that were never fetched would report every branch
// missing. So the git half **skips there, and says so, with the reason and the
// number of rows it did not compare**, the way the census reports what it
// skips. A skip that prints nothing is a pass that means nothing.
//
// # What "merged" means here, and where that breaks
//
// A branch is merged when its tip is reachable from `main` and is **not on
// `main`'s own first-parent line**. The second clause separates a merged branch
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
const rel = (p) => path.relative(repo, p).replace(/\\/g, '/') || p;

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

// ---- schema ---------------------------------------------------------------

function load() {
  let text;
  try {
    text = fs.readFileSync(LEDGER, 'utf8');
  } catch (e) {
    return { problems: [{ id: '(file)', msg: `cannot read ${rel(LEDGER)}: ${e.message}` }] };
  }
  try {
    return { j: JSON.parse(text), problems: [] };
  } catch (e) {
    return { problems: [{ id: '(file)', msg: `${rel(LEDGER)} is not valid JSON: ${e.message}` }] };
  }
}

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

// ---- git ------------------------------------------------------------------

function git(args) {
  try {
    return execFileSync('git', args, {
      cwd: repo,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      maxBuffer: 64 << 20,
    }).trim();
  } catch {
    return null;
  }
}

// Everything git can say about the ledger's branches, or the reason it cannot.
function gitFacts(rows) {
  if (git(['rev-parse', '--git-dir']) === null) return { skip: 'this is not a git checkout' };
  const listing = git(['for-each-ref', '--format=%(refname:short)%09%(objectname)', 'refs/heads']);
  const heads = new Map((listing || '').split('\n').filter(Boolean).map((l) => l.split('\t')));
  if (!heads.has('main')) {
    return { skip: 'there is no local main branch to compare against (a pull-request checkout, or a clone that never made one)' };
  }
  const agents = [...heads.keys()].filter((b) => AGENT_BRANCH.test(b));
  if (!agents.length) {
    return { skip: 'there are no local worktree-agent-* or wip/* branches, so this is not the clone the work happens in (a fresh clone, or a CI runner)' };
  }

  const mainSha = heads.get('main');
  const firstParent = new Set((git(['rev-list', '--first-parent', 'refs/heads/main']) || '').split('\n'));
  const reachable = new Set(
    (git(['for-each-ref', '--merged', 'refs/heads/main', '--format=%(refname:short)', 'refs/heads']) || '')
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
      const inMain = reachable.has(b);
      const ahead = inMain ? 0 : Number(git(['rev-list', '--count', `refs/heads/main..refs/heads/${b}`]));
      const last = ahead ? git(['log', '-1', '--format=%cI%x09%s', `refs/heads/main..refs/heads/${b}`]) : null;
      const [date, subject] = last ? last.split('\t') : [null, null];
      f = {
        branch: b,
        exists: true,
        sha,
        ahead,
        merged: inMain && !firstParent.has(sha),
        fresh: inMain && firstParent.has(sha),
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
    // Commits of its own: ahead of main AND of every branch it is built on.
    // A row stacked on another row's branch carries that branch's commits too,
    // and "12 ahead" would otherwise credit it with work it did not do.
    const bases = (Array.isArray(r.depends_on) ? r.depends_on : [])
      .map((d) => byId.get(d))
      .filter((d) => d && d.branch && d.branch !== r.branch && heads.has(d.branch));
    if (f.exists && f.ahead && bases.length) {
      f.own = Number(git(['rev-list', '--count', `refs/heads/${r.branch}`, '^refs/heads/main', ...bases.map((d) => `^refs/heads/${d.branch}`)]));
    } else if (f.exists) f.own = f.ahead;
    rowFacts.set(r.id, f);
  }

  const named = new Set(rows.map((r) => r.branch).filter(Boolean));
  const orphans = agents.filter((b) => !reachable.has(b) && !named.has(b)).map(facts);

  return {
    mainSha,
    branches: heads.size,
    agentBranches: agents.length,
    rowFacts,
    orphans,
  };
}

function agreement(rows, g) {
  const P = [];
  const bad = (id, msg) => P.push({ id, msg });
  const main = g.mainSha.slice(0, 7);
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
        `is ${r.state}, but ${r.branch} (${f.sha.slice(0, 7)}) is already merged into main (${main}). ` +
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
      `has ${f.ahead} commit${f.ahead === 1 ? '' : 's'} ahead of main (last ${day(f.date)}: "${f.subject}") and no row names it. ` +
        'Work nobody is tracking is what this ledger exists to prevent: write its row, or delete the branch if it is dead',
    );
  }
  return P;
}

const day = (iso) => (iso ? iso.slice(0, 10) : '-');
const stamp = (iso) => (iso ? iso.slice(0, 16).replace('T', ' ') : '-');

// ---- the systems rollup ---------------------------------------------------
//
// Computed from the inventories that are themselves checked, never typed. Each
// figure is quoted with its second column, because a count weights every row
// equally and a player does not (`CLAUDE.md`, the three inventories).
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
const readJson = (p) => JSON.parse(fs.readFileSync(path.join(repo, p), 'utf8'));

function inventories() {
  const out = [];
  const attempt = (meta, fn) => {
    try {
      out.push({ ...meta, ...fn() });
    } catch (e) {
      out.push({ ...meta, error: e.message });
    }
  };

  attempt({ track: 'screens', system: 'input', name: 'Input arms', source: 'docs/arms.json' }, () => {
    // The counting rule is figures.js's, required rather than restated.
    const { fromArms } = require(path.join(repo, 'tools', 'figures', 'figures.js'));
    const a = fromArms();
    const arms = readJson('docs/arms.json').arms;
    const missing = counts(arms.filter((x) => x.status === 'missing'), (x) => x.group || '(no group)');
    return {
      head: `${a.reproduced} of ${a.live}`,
      unit: 'live arms reproduced',
      pct: a.pct,
      second: [
        `${a.kinded} of the ${a.live} live arms are filed under one of the ${a.kinds} mouse gesture kinds -- which controls a screen answers is not how it answers them`,
        `${a.missing} missing, by group: ${listCounts(missing)}`,
        `${a.dead} dead in the binary and ${a.inventions} inventions of ours, both outside the denominator`,
      ],
    };
  });

  attempt({ track: 'presentation', system: 'audio', name: 'Sound triggers', source: 'docs/audio.json' }, () => {
    const sites = readJson('docs/audio.json').sites;
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
      return { of: of.length, files: files.size, fileSites: of.filter(oneFile).length, quoted };
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
    const p = path.join(repo, 'docs', 'stored-fields.json');
    if (!fs.existsSync(p)) {
      return { absent: 'not on this base -- it arrives with the stored-fields sweep, and this rollup reads it from then on' };
    }
    const fields = JSON.parse(fs.readFileSync(p, 'utf8')).fields;
    if (!Array.isArray(fields)) throw new Error('docs/stored-fields.json has no "fields" array');
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
    const src = fs.readFileSync(path.join(repo, 'crates', 'l2-game', 'tests', 'differential.rs'), 'utf8');
    const c = (name) => {
      const m = src.match(new RegExp(`const ${name}: usize = (\\d+);`));
      if (!m) throw new Error(`${name} is not in differential.rs any more -- this rollup reads it rather than restating it, so find where it went`);
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

  attempt({ track: 'instruments', system: 'tests', name: 'Install-gated tests', source: 'crates/l2-testkit/tests/census.rs' }, () => {
    const src = fs.readFileSync(path.join(repo, 'crates', 'l2-testkit', 'tests', 'census.rs'), 'utf8');
    const m = src.match(/const GATED_TOTAL: usize = (\d+);/);
    if (!m) throw new Error('GATED_TOTAL is not in census.rs any more');
    const files = new Set([...src.matchAll(/^\s*\("(crates\/[^"]+)",/gm)].map((x) => x[1]));
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
  const problems = schema(j);
  const rows = (Array.isArray(j.items) ? j.items : []).filter((r) => r && typeof r.id === 'string');

  let g = null;
  let skip = null;
  if (has('--schema')) skip = '--schema was given';
  else {
    g = gitFacts(rows);
    if (g.skip) {
      skip = g.skip;
      g = null;
    } else problems.push(...agreement(rows, g));
  }

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

  const inv = inventories();

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

  const head = git(['rev-parse', '--short', 'HEAD']);
  const dirty = git(['status', '--porcelain', '--', rel(LEDGER)]);
  return {
    generated: new Date().toISOString(),
    main: g ? g.mainSha.slice(0, 7) : (git(['rev-parse', '--short', 'refs/heads/main']) || 'unknown'),
    head,
    ledger: rel(LEDGER),
    ledgerDirty: !!dirty,
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

function gitLine(r, f) {
  if (!r.branch) return 'no branch';
  if (!f) return `${r.branch}  (git not compared)`;
  if (!f.exists) return `${r.branch}  BRANCH MISSING`;
  const bits = [];
  if (f.merged) bits.push(ON_A_BRANCH.includes(r.state) ? 'MERGED -- row is stale' : 'merged');
  else if (f.fresh) bits.push('no commits of its own yet');
  else {
    bits.push(`${f.ahead} ahead`);
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
  L.push(`lords2 work ledger -- derived ${stamp(m.generated)} UTC from main ${m.main}, ledger ${m.ledger}${m.ledgerDirty ? ' (uncommitted changes)' : ''}`);
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
          L.push(`      ${x.name}: ${x.head} ${x.unit}${x.pct !== undefined ? ` (${x.pct}%)` : ''}   [${x.source}]`);
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
    L.push(`${' '.repeat(prefix.length + 4)}${gitLine(r, m.facts(r))}`);
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

function html(m) {
  const inflight = m.inState('in-flight');
  const waiting = m.inState('awaiting-user');
  const open = m.inState('open');
  const parked = [...m.inState('deferred'), ...m.inState('abandoned')];
  const verdict = m.problems.length
    ? `<a class="chip bad" href="#disagreements">${m.problems.length} disagreement${m.problems.length === 1 ? '' : 's'}</a>`
    : m.skip
      ? chip('warn', 'schema clean, git not compared', m.skip)
      : chip('good', 'agrees with git');

  const gauges = (s) =>
    s.inventories
      .map((x) => {
        if (x.error) {
          return `<div class="gauge error"><div class="k"><span>${esc(x.name)}</span></div><div class="v">unreadable</div><p class="why">${esc(x.error)}</p><div class="src">${esc(x.source)}</div></div>`;
        }
        if (x.absent) {
          return `<div class="gauge absent"><div class="k"><span>${esc(x.name)}</span></div><p class="why">${esc(x.absent)}</p><div class="src">${esc(x.source)}</div></div>`;
        }
        const bar = x.pct !== undefined ? `<div class="bar" role="img" aria-label="${x.pct} percent"><i style="width:${Math.max(0, Math.min(100, x.pct))}%"></i></div>` : '';
        return `<div class="gauge"><div class="k"><span>${esc(x.name)}</span>${x.pct !== undefined ? `<span>${x.pct}%</span>` : ''}</div>
<div class="v">${esc(x.head)}<small>${esc(x.unit)}</small></div>${bar}
<ul>${x.second.map((l) => `<li>${esc(l)}</li>`).join('')}</ul><div class="src">${esc(x.source)}</div></div>`;
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

  footer { margin-top: 56px; padding-top: 16px; border-top: 1px solid var(--rule); font: 12px/1.6 var(--mono); color: var(--ink-faint); }
  @media (max-width: 720px) {
    .track { grid-template-columns: minmax(0, 1fr); }
  }
</style>

<div class="wrap">
  <header>
    <p class="eyebrow">lords2 &middot; the work ledger, derived</p>
    <h1>What is in flight, and what is left</h1>
    <p class="lede">Each row is intent the lead wrote in <code>${esc(m.ledger)}</code>. Everything beside it that git can answer &mdash; merged, commits ahead of <code>main</code>, a handoff note &mdash; was computed when this page was generated, and none of it is stored. If the time below is old, regenerate the page rather than trust it.</p>
    <div class="metabar">
      <span>generated <b>${esc(stamp(m.generated))} UTC</b></span>
      <span>main <b>${esc(m.main)}</b></span>
      <span>ledger at <b>${esc(m.head || 'unknown')}</b>${m.ledgerDirty ? ' <b>+ uncommitted</b>' : ''}</span>
      <span>${m.rows.length} rows</span>
      ${verdict}
    </div>
  </header>

  <h2>Systems</h2>
  <p class="intro">Figures are read from the inventories that are themselves checked, never typed. Each is quoted with its second column, because a count weights every row equally and a player does not.</p>
  ${systems}

  <h2>In flight <span class="n">${inflight.length}</span></h2>
  ${m.skip ? `<p class="skip">Git was not compared: ${esc(m.skip)}.</p>` : ''}
  ${inflight.length ? table(['Work', 'Branch', 'Last commit', 'Next'], inflight.map((r) => `<tr><td>${work(r)}</td><td>${gitCell(m, r)}</td><td>${lastCell(m, r)}</td><td class="prose">${esc(r.next) || '<span class="dim">-</span>'}</td></tr>`).join('')) : empty('No agent is working on anything.')}

  <h2>Merge queue <span class="n">${m.queue.length}</span></h2>
  <p class="intro">In the order they merge: the ledger's order, except that nothing merges ahead of a queued row it depends on.</p>
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

  <footer>Generated by <code>node tools/pm/work.js --html</code> from ${esc(m.ledger)}. Regenerate rather than edit: nothing on this page is stored anywhere.</footer>
</div>
`;
}

// ---- main -----------------------------------------------------------------

function main() {
  if (!has('--check') && !has('--status') && !has('--html')) {
    console.error('usage: node tools/pm/work.js --check [--schema] | --status | --html <path>   [--file <ledger>]');
    process.exit(2);
  }
  if (has('--html') && !opt('--html')) {
    console.error('work: --html needs a path to write, outside the tree or gitignored -- generated HTML is never committed');
    process.exit(2);
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
      const g = gitFacts(rows);
      if (g.skip) console.log(`work: SKIP git agreement: ${g.skip}; ${named} rows name a branch and were not compared`);
      else {
        const found = agreement(rows, g);
        console.log(`work: git: ${named} row branches and ${g.agentBranches} agent branches compared against main ${g.mainSha.slice(0, 7)}, ${found.length} problem(s)`);
        problems.push(...found);
      }
    }
    for (const p of problems) console.error(`work: ${p.id}: ${p.msg}`);
    process.exit(problems.length ? 1 : 0);
  }

  const m = derive();
  if (m.fatal) {
    for (const p of m.fatal) console.error(`work: ${p.id}: ${p.msg}`);
    process.exit(1);
  }
  const unreadable = m.inv.filter((x) => x.error);
  if (has('--status')) process.stdout.write(text(m));
  if (has('--html')) {
    const out = path.resolve(opt('--html'));
    fs.mkdirSync(path.dirname(out), { recursive: true });
    fs.writeFileSync(out, html(m));
    console.error(`work: wrote ${out} (main ${m.main}, ${m.rows.length} rows, ${m.problems.length} disagreement(s))`);
  }
  for (const x of unreadable) console.error(`work: rollup: ${x.name} could not be read: ${x.error}`);
  process.exit(unreadable.length ? 1 : 0);
}

main();
