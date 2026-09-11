#!/usr/bin/env node
// A git merge driver for the keyed symbol databases.
//
// # Why this exists
//
// `docs/symbols.json` once came up with nine conflict hunks in which **the
// `ours` body of every hunk sat under the wrong entry's `name`** — git had
// aligned two arrays that were ordered differently, so the diff paired
// unrelated records and offered `County_PlaceBlacksmith`'s comment as a
// candidate body for `Move_BuildCostMap`. Resolving those textually — taking a
// side, taking the union, taking the newer — would have produced a symbol
// database that parses, reads plausibly, and lies about what functions do, in
// the file this project consults to decide what the binary is.
//
// Nothing was wrong with the data. The two branches had touched different
// addresses; only the *order* differed. Merged by address there was no conflict
// at all.
//
// What caught it was `JSON.parse` throwing on the union — a syntactic check
// catching a semantic disaster by luck, which is not a defence to rely on
// twice. `docs/agents.md`, *A file that looks like data is usually a claim*.
//
// # What it does
//
// A three-way merge of every top-level array whose entries carry a key:
//
// * an entry only one side changed  -> take that side
// * an entry both sides changed the same way -> take it
// * an entry only one side added    -> keep it
// * an entry one side deleted and the other left alone -> delete it
// * an entry **both sides changed differently** -> a real conflict. Reported by
//   address and name, and the merge is refused.
//
// Output is sorted by key, so the file has one canonical order and the next
// merge has nothing to misalign. That is half the fix: the driver handles today,
// the sort stops tomorrow's diff from being nonsense in the first place.
//
// **Except where a file says its order is intent** — `FILE_POLICY` below. The
// work ledger's rows are read top to bottom (the merge queue is an order), so
// sorting them by id would merge the data and destroy the meaning.
//
// # It never changes a file's layout, and refuses rather than try
//
// On the mercenaries merge (C164) this driver rewrote `docs/stored-fields.json`
// from 274 lines to 1,617 -- pretty-printed -- and reported the merge clean. The
// content was the correct union. The layout was not, and that file's contract is
// one row per line, so three tests that scan it went red and the integrator
// restored the layout by hand. A clean report on a merge that broke the file is
// the silent-degradation failure this project keeps logging.
//
// So the layout a file is written back in is part of its policy, and two checks
// hold it:
//
// * `--check` fails any registered file whose bytes are not exactly what this
//   driver would write for it, so a file with a layout the driver does not know
//   is found by the test suite, before anyone merges it;
// * a merge whose OUR side is not in that layout is refused, and git is left to
//   show the conflict, rather than writing a "clean" result in a different
//   shape. With our side in the driver's layout, the output is in the same
//   layout by construction, so a clean merge cannot reformat a file.
//
// # Wiring
//
//     git config merge.l2json.name   "keyed merge for the symbol databases"
//     git config merge.l2json.driver "node tools/symbols/merge-json.js %O %A %B %P"
//
// and `.gitattributes` names the files. The `git config` half is per-clone —
// git will not run a driver a repository merely asks for, which is a sensible
// refusal to execute code on checkout. `tools/symbols/install-merge-driver.sh`
// is the one-liner; without it git falls back to the ordinary text merge and
// the conflict markers appear as before, which is safe because it is loud.

const fs = require('fs');

// **Every array carries its own key, and guessing one wrong is the failure this
// file exists to prevent**, so the key is derived from the entry rather than
// assumed: `id` where there is one; else `addr` for functions, globals and
// record arrays; `target` for corrections; `record`+`off` for a struct field;
// `name` for structs. An array whose entries match none of these is NOT merged
// by key -- it falls through to 'conflict if both sides changed it', which is
// loud and correct.
//
// **`id` outranks `addr` and that ordering is load-bearing.** `docs/arms.json`
// carries both: 42 records across 23 addresses, because one function can hold
// several input arms -- `Screen_FrameInput` alone holds six. Keyed on `addr`
// this driver would have collapsed 42 records into 23 and thrown away 19 of
// them, silently, in a file three agents are writing at once. The
// `--check` mode below exists so that a key which is not unique is a loud
// failure rather than a quiet deletion, and `crates/l2-testkit/tests/keyed_json.rs`
// runs it over every file `.gitattributes` hands to this driver.
const KEY_FIELDS = [['id'], ['addr'], ['target'], ['record', 'off'], ['name']];

function keyOf(e) {
  if (!e || typeof e !== 'object') return null;
  for (const fields of KEY_FIELDS) {
    if (fields.every((f) => e[f] !== undefined)) {
      return fields.map((f) => String(e[f]).toLowerCase()).join(' ');
    }
  }
  return null;
}

// **Per-file policy, for what the entries themselves cannot say.**
//
// A file with no entry here is written back as plain two-space JSON, with its
// keyed arrays sorted by the derived key. `docs/symbols.json`,
// `docs/hypotheses.json`, `docs/records.json`, `docs/arms.json` and
// `docs/audio.json` are all exactly that today -- `--check` holds each of them to
// it byte for byte -- so they need no entry.
//
// A file whose shape differs says how, and each part is stated rather than
// inferred, because an inferred key is exactly how `arms.json` nearly lost 19
// records:
//
// * **`key`** — the fields a row is keyed by, and nothing else; a row without
//   them is refused, not re-keyed.
// * **`order`** — `'key'` sorts by that key, like every other file; `'file'`
//   keeps OUR order and slots each row only THEIRS added in after the row it
//   followed there, for a file whose order is intent. `--check` demands key
//   order only of `'key'`.
// * **`rows: true`** — one record per line. The driver writes the file back in
//   that shape, and `--check` requires the file to already be in it. It is also
//   what keeps a text merge honest when the driver is not registered: a
//   misaligned hunk can only pair whole rows, never one row's field under
//   another row's id -- which was the whole of the `symbols.json` disaster.
const FILE_POLICY = {
  // The work ledger. Its rows sit under `items`, beside `about`, `states` and
  // `tracks`, and their order is the merge queue's, so it is kept.
  'docs/work.json': { items: { key: ['id'], order: 'file', rows: true } },
  // The stored-fields inventory. **One object per line is its contract**:
  // `crates/l2-scenario/tests/stored_fields.rs` scans it line by line rather than
  // take a JSON dependency, and loses every row of a reformatted file -- which is
  // what the C164 merge did to it. Its order is County then Realm, each by
  // offset, which the same test requires; for ids of the form `County+0x0C0`
  // that is key order, so the ordinary sort is the right one and `--check`
  // keeps asking for it.
  'docs/stored-fields.json': { fields: { key: ['id'], order: 'key', rows: true } },
};

function policyFor(file) {
  const rel = String(file || '').replace(/\\/g, '/').replace(/^\.\//, '');
  for (const [p, v] of Object.entries(FILE_POLICY)) {
    if (rel === p || rel.endsWith('/' + p)) return v;
  }
  return {};
}

// The key function for one member of one file: the policy's, where it names
// that member, and otherwise the entry-derived rule above.
function keyFnFor(policy, which) {
  const a = policy[which];
  if (!a) return keyOf;
  return (e) =>
    e && typeof e === 'object' && a.key.every((f) => e[f] !== undefined && e[f] !== null)
      ? a.key.map((f) => String(e[f]).toLowerCase()).join(' ')
      : null;
}

// One value on one line, spaced the way the one-row-per-line files are written.
function inline(v) {
  if (Array.isArray(v)) return '[' + v.map(inline).join(', ') + ']';
  if (v && typeof v === 'object') {
    return '{' + Object.entries(v).map(([k, x]) => JSON.stringify(k) + ': ' + inline(x)).join(', ') + '}';
  }
  return JSON.stringify(v);
}

// The canonical text of a file under its policy: exactly the bytes this driver
// writes. Without a `rows` member it is the plain two-space form.
function serialize(j, policy) {
  if (!Object.values(policy).some((a) => a.rows)) return JSON.stringify(j, null, 2) + '\n';
  const parts = Object.keys(j).map((k) => {
    const v = j[k];
    const a = policy[k];
    if (a && a.rows && Array.isArray(v)) {
      const body = v.length ? '[\n' + v.map((e) => '    ' + inline(e)).join(',\n') + '\n  ]' : '[]';
      return '  ' + JSON.stringify(k) + ': ' + body;
    }
    return '  ' + JSON.stringify(k) + ': ' + JSON.stringify(v, null, 2).replace(/\n/g, '\n  ');
  });
  return '{\n' + parts.join(',\n') + '\n}\n';
}

// Where two texts first differ, as a 1-based line number.
function firstDifference(a, b) {
  const x = a.split('\n');
  const y = b.split('\n');
  let line = 0;
  while (line < x.length && x[line] === y[line]) line += 1;
  return line + 1;
}

// `--check <file>...`: no merging, just the invariants a merge depends on.
// There is one implementation of the key rule and this is how a test reaches it
// -- a Rust test that reimplemented KEY_FIELDS would be a second copy that can
// drift from the first, which is the failure this whole file is about.
if (process.argv[2] === '--check') {
  const files = process.argv.slice(3);
  if (!files.length) {
    console.error('usage: merge-json.js --check <file>...');
    process.exit(2);
  }
  let bad = 0;
  for (const f of files) {
    let j, text;
    try {
      text = fs.readFileSync(f, 'utf8');
      j = JSON.parse(text);
    } catch (e) {
      console.error(`${f}: not valid JSON (${e.message})`);
      bad += 1;
      continue;
    }
    const policy = policyFor(f);
    // A policy that names a member the file does not have is a policy about
    // some other file, and it would otherwise pass for ever.
    for (const which of Object.keys(policy)) {
      if (!Array.isArray(j[which])) {
        bad += 1;
        console.error(`${f}: FILE_POLICY in merge-json.js keys "${which}", which this file does not have as an array.`);
      }
    }
    // **Every member is classified, and one shape is refused.**
    //
    // An array of OBJECTS that is not keyable would be merged atomically —
    // whole-or-refused — when a reader would reasonably expect it to merge per
    // entry. That is the shape that silently loses records, so a new keyed file
    // cannot acquire it by default. Prose (an array of strings), scalars and
    // objects are all fine atomically or per key, and are reported rather than
    // refused so the classification is visible.
    for (const [which, arr] of Object.entries(j)) {
      const keyFn = keyFnFor(policy, which);
      if (Array.isArray(arr) && arr.length && arr.every((e) => e && typeof e === 'object')) {
        if (!arr.every((e) => keyFn(e) !== null)) {
          bad += 1;
          const named = policy[which] ? `its FILE_POLICY key (${policy[which].key.join('+')})` : 'an id (or addr, target, record+off, name)';
          console.error(
            `${f}: ${which} is an array of objects with no usable key, so a merge ` +
              `would treat it as one lump and lose one side's entries.\n` +
              `  Give every entry ${named}, or move them into an object keyed by name.`,
          );
          continue;
        }
      }
      if (!Array.isArray(arr) || !arr.length) continue;
      if (!arr.every((e) => keyFn(e) !== null)) continue;
      const seen = new Map();
      const dups = [];
      for (const e of arr) {
        const k = keyFn(e);
        if (seen.has(k)) dups.push(k);
        seen.set(k, e);
      }
      if (dups.length) {
        bad += 1;
        console.error(
          `${f}: ${which} has ${dups.length} duplicate key(s) -- ` +
            `merging it would DELETE the duplicates, one per collision:`,
        );
        for (const d of dups.slice(0, 8)) console.error(`    ${d}`);
        console.error(
          '  Either the entries are genuinely duplicated, or the key this driver ' +
            'picked is not the unique one for this array. KEY_FIELDS in ' +
            'tools/symbols/merge-json.js decides, and `id` outranks `addr` ' +
            'precisely because docs/arms.json has several arms per address.',
        );
      }
      // An array whose order is intent is exempt from key order, by policy and
      // only by policy -- the exemption is written down, never guessed.
      if (policy[which] && policy[which].order === 'file') continue;
      const keys = arr.map((e) => keyFn(e));
      const sorted = [...keys].sort();
      if (keys.some((k, i) => k !== sorted[i])) {
        bad += 1;
        console.error(
          `${f}: ${which} is not in key order.\n` +
            '  An unsorted array is what lets a text merge misalign in the first ' +
            'place, which is the failure this driver exists for -- so this is a ' +
            'failure and not a warning. Sort it by the key THIS FILE picks for ' +
            'it (KEY_FIELDS above), which is not always addr: docs/arms.json ' +
            'is keyed by id, because one address holds several arms.',
        );
      }
    }
    // **The layout the driver will write back must be the layout already on
    // disk**, for every registered file and not only the ones with a policy.
    // That is what makes a file with a layout the driver does not know -- the
    // way docs/stored-fields.json was, until C164's merge reformatted it -- fail
    // here, in the suite, instead of at the first merge.
    const want = serialize(j, policy);
    if (want !== text) {
      bad += 1;
      const rows = Object.values(policy).some((a) => a.rows);
      console.error(
        `${f}: not in the layout this driver writes back (first difference at line ${firstDifference(want, text)}).\n` +
          (rows
            ? '  Its FILE_POLICY says one record per line and two-space indent everywhere else, with a final newline. '
            : '  It has no FILE_POLICY, so the driver writes plain two-space JSON with a final newline. ') +
          'A merge of a file in any other layout is refused, so either restore that layout, or -- if this file has a ' +
          'layout contract of its own -- state it in FILE_POLICY in tools/symbols/merge-json.js.',
      );
    }
  }
  process.exit(bad ? 1 : 0);
}


const [, , basePath, oursPath, theirsPath, label] = process.argv;
if (!basePath || !oursPath || !theirsPath) {
  console.error('usage: merge-json.js <base> <ours> <theirs> [path]');
  console.error('       merge-json.js --check <file>...');
  process.exit(2);
}

const name = label || oursPath;
const policy = policyFor(label || oursPath);

function read(p) {
  try {
    const text = fs.readFileSync(p, 'utf8');
    return { text, json: JSON.parse(text) };
  } catch (e) {
    console.error(`l2json: ${name}: ${p} is not valid JSON (${e.message})`);
    return null;
  }
}

const baseFile = read(basePath);
const oursFile = read(oursPath);
const theirsFile = read(theirsPath);
if (!baseFile || !oursFile || !theirsFile) {
  // Refuse rather than guess: git then leaves the ordinary conflict.
  process.exit(1);
}
const base = baseFile.json;
const ours = oursFile.json;
const theirs = theirsFile.json;

// **Refuse to reformat.** If our side is not in the layout this driver writes,
// a "clean" merge would rewrite every line of it. Exiting non-zero without
// touching %A leaves the path conflicted with our content, and git says so --
// which is loud, where a reformatted file with a clean report was not.
{
  const want = serialize(ours, policy);
  if (want !== oursFile.text) {
    console.error(
      `l2json: ${name}: REFUSED. Our side is not in the layout this driver writes back ` +
        `(first difference at line ${firstDifference(want, oursFile.text)}), so a merge would reformat the ` +
        `whole file and report it clean. That is what turned docs/stored-fields.json from 274 lines into ` +
        `1,617 on the C164 merge.\n` +
        `  If this file has a layout contract, state it in FILE_POLICY in tools/symbols/merge-json.js; ` +
        `if it has drifted, restore the layout on our side. Then merge again. ` +
        `\`node tools/symbols/merge-json.js --check ${name}\` says which.`,
    );
    process.exit(1);
  }
}

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

const conflicts = [];

function keyable(arr, which) {
  const kf = keyFnFor(policy, which);
  return Array.isArray(arr) && arr.length > 0 && arr.every((e) => kf(e) !== null);
}

function mergeArray(which) {
  const kf = keyFnFor(policy, which);
  const b = new Map((base[which] || []).map((e) => [kf(e), e]));
  const o = new Map((ours[which] || []).map((e) => [kf(e), e]));
  const t = new Map((theirs[which] || []).map((e) => [kf(e), e]));
  const keep = new Map();

  for (const k of new Set([...b.keys(), ...o.keys(), ...t.keys()])) {
    const eb = b.get(k), eo = o.get(k), et = t.get(k);
    if (eo && et) {
      if (same(eo, et)) keep.set(k, eo);
      else if (eb && same(eb, eo)) keep.set(k, et);
      else if (eb && same(eb, et)) keep.set(k, eo);
      else {
        conflicts.push({ which, k, o: eo, t: et });
        keep.set(k, eo);
      }
    } else if (eo) {
      if (!(eb && same(eb, eo))) keep.set(k, eo); // else: theirs deleted it
    } else if (et) {
      if (!(eb && same(eb, et))) keep.set(k, et); // else: ours deleted it
    }
  }

  if (policy[which] && policy[which].order === 'file') {
    // Our order, with each row only theirs added placed after the row it
    // followed on their side (or first, if it led their list).
    const seq = [...o.keys()].filter((k) => keep.has(k));
    let anchor = -1;
    for (const k of t.keys()) {
      const at = seq.indexOf(k);
      if (at >= 0) {
        anchor = at;
        continue;
      }
      if (!keep.has(k)) continue;
      seq.splice(anchor + 1, 0, k);
      anchor += 1;
    }
    return seq.map((k) => keep.get(k));
  }
  const out = [...keep.values()];
  out.sort((x, y) => (kf(x) < kf(y) ? -1 : kf(x) > kf(y) ? 1 : 0));
  return out;
}

// Duplicate keys are a defect in their own right — two entries claiming one
// address is the same class of mistake as two corrections claiming one number —
// so they are reported rather than silently collapsed by the Map above.
function duplicates(j, which) {
  const kf = keyFnFor(policy, which);
  const seen = new Set();
  const dup = [];
  for (const e of j[which] || []) {
    const k = kf(e);
    if (seen.has(k)) dup.push(`${k} ${e.name || ''}`);
    seen.add(k);
  }
  return dup;
}

// **The members that are NOT keyed arrays, which this driver used to discard.**
//
// `const merged = { ...ours }` took OUR copy of every sibling — `_note`,
// `groups`, `version`, `marker` — and then overwrote only the keyed arrays. So
// a change made on the other side to any of them was thrown away, and the
// driver reported "merged by key, no entry changed on both sides", which was
// true and reassuring and beside the point. On one `arms.json` rebase that
// silently dropped five group declarations and twenty-one lines of prose, and
// every test stayed green because every test reads the `arms` array.
//
// It is the `addr`-versus-`id` defect one level up: **the entries were keyed
// and the object holding them was not.**
//
// The policy now, and it is chosen rather than inherited:
//
//   * a **keyed array** merges by key, as before;
//   * an **object** merges per key, recursively — `groups` is a map and three
//     agents adding three different groups is the ordinary case, not a
//     conflict;
//   * **everything else is atomic**: a scalar, a string, or an array of
//     strings. If only one side changed it, take that side. If both changed it
//     the same way, take it. **If both changed it differently, REFUSE.**
//
// Refusing on prose is deliberate. `_note` is exactly the thing a machine
// cannot merge sensibly and a person can, and a driver that guesses at it would
// be making the same mistake in a quieter place. Handing it back costs one
// person one minute; getting it wrong costs a paragraph nobody notices is gone.
function mergeMember(which, b, o, t) {
  if (o === undefined && t === undefined) return undefined;
  if (same(o, t)) return o;
  if (same(b, o)) return t;           // only theirs changed
  if (same(b, t)) return o;           // only ours changed
  const obj = (v) => v && typeof v === 'object' && !Array.isArray(v);
  if (obj(o) && obj(t)) {
    // Both changed the object. Merge it per key and only refuse on the keys
    // that genuinely collide.
    const out = {};
    for (const k of new Set([...Object.keys(o), ...Object.keys(t)])) {
      const sub = mergeMember(`${which}.${k}`, (b || {})[k], o[k], t[k]);
      if (sub !== undefined) out[k] = sub;
    }
    return out;
  }
  conflicts.push({ which, k: '(whole member)', o: { name: which }, t: { name: which } });
  return o;
}

const merged = {};
for (const which of new Set([...Object.keys(ours), ...Object.keys(theirs)])) {
  // A keyed array is NOT a member for this purpose: `mergeArray` merges it
  // per entry below, and running it through here as well would report a
  // conflict on every array both sides touched -- which is exactly the noise
  // this driver exists to remove. Caught by the driver own regression test.
  if (keyable(ours[which] || [], which) || keyable(theirs[which] || [], which)) continue;
  const v = mergeMember(which, base[which], ours[which], theirs[which]);
  if (v !== undefined) merged[which] = v;
}
let dupWarnings = [];
const arrays = [...new Set([...Object.keys(ours), ...Object.keys(theirs)])].filter(
  (w) => keyable(ours[w], w) || keyable(theirs[w], w),
);
for (const which of arrays) {
  if (!keyable(ours[which] || [], which) && !keyable(theirs[which] || [], which)) continue;
  // A scalar or an unkeyable array: both sides changing it is a real conflict.
  if (!keyable(ours[which] || [], which) || !keyable(theirs[which] || [], which)) {
    if (!same(ours[which], theirs[which])) {
      conflicts.push({ which, k: '(whole array)', o: { name: which }, t: { name: which } });
    }
    continue;
  }
  for (const [side, j] of [['ours', ours], ['theirs', theirs]]) {
    for (const d of duplicates(j, which)) {
      dupWarnings.push(`  ${side} has ${which} ${d} twice`);
    }
  }
  merged[which] = mergeArray(which);
}

if (dupWarnings.length) {
  console.error(`l2json: ${name}: duplicate keys before merging:\n${dupWarnings.join('\n')}`);
}

if (conflicts.length) {
  console.error(
    `l2json: ${name}: ${conflicts.length} entr${conflicts.length === 1 ? 'y was' : 'ies were'} ` +
      `changed on BOTH sides. Merge these by hand — and note that every OTHER entry merged ` +
      `cleanly, so a text conflict here would have been mostly noise:\n`,
  );
  for (const c of conflicts) {
    console.error(`  ${c.which} ${c.k}`);
    console.error(`    ours  : ${c.o.name || c.o.id || ''} — ${String(c.o.comment || c.o.title || '').slice(0, 120)}`);
    console.error(`    theirs: ${c.t.name || c.t.id || ''} — ${String(c.t.comment || c.t.title || '').slice(0, 120)}`);
  }
  process.exit(1);
}

// A file with a policy keeps its own member order (ours) as well as its rows'
// order; every other file is written exactly as it always has been.
let out = merged;
if (Object.keys(policy).length) {
  out = {};
  for (const k of [...Object.keys(ours), ...Object.keys(merged)]) {
    if (k in merged && !(k in out)) out[k] = merged[k];
  }
}
fs.writeFileSync(oursPath, serialize(out, policy));
const n = arrays
  .filter((w) => Array.isArray(merged[w]))
  .map((w) => `${merged[w].length} ${w}`)
  .join(', ');
console.error(`l2json: ${name}: merged by key — ${n}, no entry changed on both sides.`);
process.exit(0);
