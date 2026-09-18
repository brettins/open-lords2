#!/usr/bin/env node
// Compare *where* a screen draws, not only how many times.
//
//   node tools/draws/drawcoords.js                # the table and the distribution
//   node tools/draws/drawcoords.js --screen 0x15  # every point, paired, for one screen
//   node tools/draws/drawcoords.js --module county
//   node tools/draws/drawcoords.js --check        # against docs/draw-coords.json
//   node tools/draws/drawcoords.js --write        # rewrite that store
//   node tools/draws/drawcoords.js --json         # the whole measurement
//
// # Why this exists
//
// `tools/draws/screendraws.js` pairs *our* call sites with the original's and
// calls the pair reproduced. **It has never compared a coordinate.** So a
// screen can score well and be laid out wrongly, and every instance of that
// found so far was found by a player looking at it:
//
// * five ration columns two pixels left, because an invented trailing space is
//   inside what `Ui_DrawNumberRight` measures (`docs/decisions.md` C140);
// * a group-87 caption four pixels right, from the same cause the other way;
// * `BODY_WRAP` turning out to be two numbers, `0x120` and `0x130`, because one
//   constant was read off one call site and used for both halves of a panel.
//
// Four placement defects, four causes, none visible to any check we had.
//
// # The unit, and it is a point rather than a call
//
// > **One record is one literal `(x, y)` a draw primitive is called with**,
// > plus the `width` where the primitive takes one, because a width changes the
// > anchor: `Ui_DrawCentred` and `Ui_DrawNumberRight` both centre inside it.
//
// The point is deliberately **not** paired with a call site by identity. There
// is no `// draw:` marker on a draw call and `docs/draws.md` §11 argues at
// length against adding ~1,000 of them. What is available without markers is
// the *set* of coordinates each side draws at, and that turns out to be enough,
// because the defect class we are hunting is a point that has **drifted**: our
// (0x11E, 0xA1) against the original's (0x120, 0xA1) is a nearest-neighbour
// match at distance 2, and a distance of 2 is not a thing that happens by
// coincidence on a 640x480 screen.
//
// So the matching is: exact points first, then nearest neighbour among what is
// left, greedily by ascending Manhattan distance, up to [`NEAR`]. Anything
// further away is reported as unmatched rather than guessed at.
//
// # Both sides read their argument positions out of a signature
//
// Which argument of `Eng_DrawString` is the x? The tool does not know and is
// not told. `docs/symbols.json` carries
// `void Eng_DrawString(int group, int index, int x, int y, undefined * font,
// undefined4 colour)`, so the positions are **derived from the parameter
// names** in the project's own symbol database — and on our side from the
// `pub fn` signatures in `crates/l2-game/src/shell/mod.rs`, whose parameters
// are named `x`, `y` and `width` for the same reason.
//
// `docs/agents.md`: *prefer a shape that cannot be wrong to a check that
// notices when it is.* A hand-written table of argument indices would be a
// second list maintained by whoever renames a parameter, which is the pattern
// that lies. This one goes stale only if somebody renames a parameter to
// something that is not a coordinate, and then the primitive drops out of the
// denominator loudly rather than reporting a wrong number.
//
// # What it cannot see, stated up front because the denominator has a hole
//
// 1. **A computed coordinate.** `Ui_DrawCount(taxShown, 0, g_penAdvance + 0x60,
//    200, …)` has a literal y and an x that depends on how wide the previous
//    string came out. Ours is `pen.count(canvas, x, 200, …)` with `x` from the
//    previous draw's return. Both sides are right and neither is a number.
//    These are counted and reported, never silently dropped.
// 2. **A primitive whose signature does not name a coordinate.**
//    `Sprite_WGenSprite(int a, int b, int c)` is three unnamed integers.
// 3. **The campaign map**, which is `tools/draws/mapdraws.js`'s audit and whose
//    coordinates are tile arithmetic rather than literals. The mercenary marker
//    that was drawn on the wrong tile of the town is **not** findable here, and
//    saying so is the point of this list.
// 4. **A draw behind a helper of ours that takes only some of its
//    coordinates.** `delta_row(&pen, canvas, y, label, value)` fixes x inside
//    the helper. One level of inlining is done — see [`inlineHelpers`] — and
//    anything deeper is counted as uncomparable.
'use strict';
const fs = require('fs');
const path = require('path');
const m = require('./drawcoords-measure.js');
const sd = require('./screendraws.js');
const { NEAR, measure } = m;
const ROOT = path.join(__dirname, '..', '..');

function distribution(rows) {
  const d = new Map();
  for (const r of rows) for (const p of r.m.pairs) d.set(p.d, (d.get(p.d) || 0) + 1);
  return [...d.entries()].sort((a, b) => a[0] - b[0]);
}

function table(rows) {
  const head = ['module', 'screens', 'orig pts', 'blind', 'our pts', 'blind', 'exact', 'near', 'orig only', 'ours only'];
  const body = rows.map(r => [
    r.module, r.screens.length, r.o.pts.length, r.o.blind.length, r.u.pts.length, r.u.blind.length,
    r.m.pairs.filter(p => p.d === 0).length, r.m.pairs.filter(p => p.d > 0).length,
    r.m.origOnly.length, r.m.oursOnly.length,
  ]);
  const tot = ['', rows.reduce((n, r) => n + r.screens.length, 0)];
  for (let i = 2; i < head.length; i++) tot.push(body.reduce((n, b) => n + b[i], 0));
  const all = [head, ...body, tot];
  const wid = head.map((_, i) => Math.max(...all.map(r => String(r[i]).length)));
  const line = r => r.map((c, i) => (i === 0 ? String(c).padEnd(wid[i]) : String(c).padStart(wid[i]))).join('  ');
  console.log(line(head));
  console.log(wid.map(w => '-'.repeat(w)).join('  '));
  for (const b of body) console.log(line(b));
  console.log(wid.map(w => '-'.repeat(w)).join('  '));
  console.log(line(tot));
  return { body, tot };
}

function nearMisses(rows) {
  const out = [];
  for (const r of rows) {
    for (const p of r.m.pairs) {
      if (p.d === 0) {
        // The point agrees and the *width* may not — and a width moves the
        // glyphs, because `Ui_DrawCentred` and `Ui_DrawNumberRight` centre
        // inside it. `BODY_WRAP` is exactly this defect.
        if (p.o.w !== null && p.u.w !== null && p.o.w !== p.u.w) {
          out.push({ kind: 'width', module: r.module, d: Math.abs(p.o.w - p.u.w), p });
        }
        continue;
      }
      out.push({ kind: 'point', module: r.module, d: p.d, p });
    }
  }
  return out.sort((a, b) => b.d - a.d);
}

function main() {
  const arg = f => { const i = process.argv.indexOf(f); return i < 0 ? null : process.argv[i + 1]; };
  if (!sd.corpusLoaded()) {
    // `docs/environment.md`: **absent is not wrong**, and a skip that reads as
    // a pass is what hid a broken fixture for weeks. Say which it is.
    console.log('SKIP drawcoords: no decompiled corpus, so no coordinate was compared.');
    process.exit(process.argv.includes('--check') ? 0 : 2);
  }
  const rows = measure();

  const one = arg('--screen'), mod = arg('--module');
  if (one || mod) {
    const inv = sd.inventory();
    const want = mod || (inv.find(r => r.screen === one) || {}).module;
    const r = rows.find(r => r.module === want);
    if (!r) { console.error(`no enumerated module for ${one || mod}`); process.exit(2); }
    console.log(`=== ${r.module} — screens ${r.screens.join(', ')}`);
    for (const p of r.m.pairs.slice().sort((a, b) => a.o.y - b.o.y || a.o.x - b.o.x)) {
      const w = p.o.w !== null || p.u.w !== null ? `  w ${p.o.w}/${p.u.w}` : '';
      console.log(`  ${p.d === 0 ? '  ok' : `d=${String(p.d).padStart(3)}`}  ` +
        `orig (${p.o.x},${p.o.y}) ${p.o.name} <- ${p.o.from}   ` +
        `ours (${p.u.x},${p.u.y}) ${p.u.name}${p.u.via ? ` via ${p.u.via}` : ''} ${p.u.file}:${p.u.line}${w}`);
    }
    console.log(`  --- the original draws at ${r.m.origOnly.length} points with nothing of ours near:`);
    for (const p of r.m.origOnly) console.log(`      (${p.x},${p.y}) ${p.name} <- ${p.from}  ${p.text.slice(0, 80)}`);
    console.log(`  --- we draw at ${r.m.oursOnly.length} points with nothing of the original's near:`);
    for (const p of r.m.oursOnly) console.log(`      (${p.x},${p.y}) ${p.name} ${p.file}:${p.line}`);
    console.log(`  --- uncomparable: ${r.o.blind.length} of the original's sites, ${r.u.blind.length} of ours`);
    return;
  }

  if (process.argv.includes('--json')) {
    console.log(JSON.stringify(rows.map(r => ({
      module: r.module, screens: r.screens,
      original: { points: r.o.pts.length, blind: r.o.blind.length },
      ours: { points: r.u.pts.length, blind: r.u.blind.length },
      exact: r.m.pairs.filter(p => p.d === 0).length,
      near: r.m.pairs.filter(p => p.d > 0).map(p => ({ d: p.d, orig: [p.o.x, p.o.y], ours: [p.u.x, p.u.y], line: p.u.line })),
    })), null, 2));
    return;
  }

  const { tot } = table(rows);
  const pairs = rows.reduce((n, r) => n + r.m.pairs.length, 0);
  const exact = rows.reduce((n, r) => n + r.m.pairs.filter(p => p.d === 0).length, 0);
  const near = nearMisses(rows);
  console.log();
  console.log(`  ${pairs} draws could be compared by coordinate. ${exact} agree exactly ` +
    `(${pairs ? Math.round((100 * exact) / pairs) : 0}%).`);
  console.log();
  console.log('  The disagreements, by Manhattan distance:');
  for (const [d, n] of distribution(rows)) if (d) console.log(`    ${String(d).padStart(3)} px   ${n}`);
  const widths = near.filter(n => n.kind === 'width');
  if (widths.length) console.log(`    and ${widths.length} pairs on the same point with a different width.`);
  console.log();
  console.log('  And the hole in the denominator, which is the part to read first:');
  console.log(`    ${tot[3]} of the original's ${tot[2] + tot[3]} leaf call sites have no literal (x, y)`);
  console.log(`    ${tot[5]} of our ${tot[4] + tot[5]} have none either.`);
  console.log(`    ${tot[8]} points the original draws at have nothing of ours within ${NEAR} px —`);
  console.log('    mostly draws we simply do not make, which is screendraws.js\'s question.');
  console.log(`    ${tot[9]} points of ours have nothing of the original's near them.`);
  console.log();
  console.log('  Not measured here at all: the campaign map (tools/draws/mapdraws.js),');
  console.log('  whose coordinates are tile arithmetic and not literals.');

  if (process.argv.includes('--write')) { writeStore(snapshot(rows)); return; }
  if (process.argv.includes('--check')) process.exit(check(snapshot(rows)));
}

// -------------------------------------------------------------- the stored side
//
// `docs/arms.json` and `docs/audio.json` are the shape: a file the tool writes
// and a `--check` that fails when today's tree stops agreeing with it. Without
// one, a measurement is a number somebody once printed.
const STORE = path.join(ROOT, 'docs', 'draw-coords.json');

function snapshot(rows) {
  const tally = f => rows.reduce((n, r) => n + f(r), 0);
  return {
    totals: {
      compared: tally(r => r.m.pairs.length),
      exact: tally(r => r.m.pairs.filter(p => p.d === 0).length),
      original_points: tally(r => r.o.pts.length),
      original_blind: tally(r => r.o.blind.length),
      our_points: tally(r => r.u.pts.length),
      our_blind: tally(r => r.u.blind.length),
      original_only: tally(r => r.m.origOnly.length),
      ours_only: tally(r => r.m.oursOnly.length),
    },
    near_misses: nearMisses(rows).map(n => ({
      module: n.module, kind: n.kind, d: n.d,
      original: { name: n.p.o.name, from: n.p.o.from, x: n.p.o.x, y: n.p.o.y, w: n.p.o.w },
      ours: { name: n.p.u.name, file: n.p.u.file, line: n.p.u.line, x: n.p.u.x, y: n.p.u.y, w: n.p.u.w },
      reason: null,
    })),
  };
}

function writeStore(snap) {
  const doc = {
    // Written, never typed. The reasons below are the one hand-kept field.
    generated_by: 'node tools/draws/drawcoords.js --write',
    ...snap,
  };
  const old = fs.existsSync(STORE) ? JSON.parse(fs.readFileSync(STORE, 'utf8')) : { near_misses: [] };
  const was = new Map((old.near_misses || []).map(n => [nearKey(n), n.reason]));
  for (const n of doc.near_misses) if (was.get(nearKey(n))) n.reason = was.get(nearKey(n));
  fs.writeFileSync(STORE, JSON.stringify(doc, null, 2) + '\n');
  console.log(`\ndrawcoords: wrote ${STORE}`);
}

const nearKey = n => `${n.module} ${n.kind} ${n.original.name}@${n.original.x},${n.original.y}` +
  ` <- ${n.original.from} -> ${n.ours.name}@${n.ours.x},${n.ours.y}`;

/**
 * Two-sided: the counts must be the stored ones, and the near misses must be
 * the stored set. A count that moves is not a defect on its own — it moves
 * whenever a screen gains a draw — but it is the only thing that makes the
 * stored figure a measurement rather than a memory, so it fails and
 * `--write` is the answer. A *new* near miss is the defect this exists for.
 */
function check(snap) {
  if (!fs.existsSync(STORE)) {
    console.error(`\ndrawcoords: no ${STORE}; run node tools/draws/drawcoords.js --write`);
    return 1;
  }
  const stored = JSON.parse(fs.readFileSync(STORE, 'utf8'));
  let bad = 0;
  for (const [k, v] of Object.entries(snap.totals)) {
    if (stored.totals[k] !== v) {
      bad++;
      console.error(`drawcoords: ${k} is ${v}, the stored figure is ${stored.totals[k]}` +
        (k === 'exact' && v < stored.totals[k] ? ' — a fall here is a placement regression' : ''));
    }
  }
  const was = new Set((stored.near_misses || []).map(nearKey));
  for (const n of snap.near_misses) {
    if (was.has(nearKey(n))) { was.delete(nearKey(n)); continue; }
    bad++;
    console.error(`drawcoords: ${n.ours.file}:${n.ours.line} draws at (${n.ours.x},${n.ours.y}) ` +
      `where ${n.original.from} draws ${n.original.name} at (${n.original.x},${n.original.y})` +
      (n.kind === 'width' ? ` with width ${n.original.w}, not ${n.ours.w}` : ` — ${n.d} px out`));
  }
  for (const k of was) { bad++; console.error(`drawcoords: the stored near miss ${k} is gone — --write`); }
  if (!bad) {
    console.log(`\ndrawcoords: ${snap.totals.compared} compared, ${snap.totals.exact} exact, ` +
      `${snap.near_misses.length} near misses, all as stored.`);
  } else {
    console.error('  run: node tools/draws/drawcoords.js --write');
  }
  return bad ? 1 : 0;
}

module.exports = { measure, nearMisses, snapshot, check, NEAR, STORE };
if (require.main === module) main();
