#!/usr/bin/env node
// Compare *where* a screen draws, not only how many times.
//
//   node tools/draws/drawcoords.js                # the table and the distribution
//   node tools/draws/drawcoords.js --screen 0x15  # every point, paired, for one screen
//   node tools/draws/drawcoords.js --module county.rs
//   node tools/draws/drawcoords.js --check        # fail on a near miss
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
const sd = require('./screendraws.js');

const ROOT = path.join(__dirname, '..', '..');
const SCREENS_DIR = path.join(ROOT, 'crates', 'l2-game', 'src', 'screens');

/** Manhattan pixels. Beyond this, two points are two different things. */
const NEAR = 24;

// ------------------------------------------------- argument positions, derived
//
// A signature's parameter list, mapped to the index of `x`, `y` and the width.
// `w` and `width` are the same role; `cols`/`rows` are a box in cells and are
// deliberately *not* treated as a width, because they are not pixels.
function positions(params) {
  const at = name => params.findIndex(p => p === name);
  const x = at('x'), y = at('y');
  if (x < 0 || y < 0) return null;
  let w = at('width');
  if (w < 0) w = at('w');
  return { x, y, w: w < 0 ? null : w };
}

/** `void Ui_DrawText(byte * str, int x, int y, …)` -> the parameter names. */
function cParams(sig) {
  const open = sig.indexOf('(');
  const args = sd.splitArgs(sd.balanced(sig, open));
  if (!args) return null;
  return args.map(a => {
    const m = /([A-Za-z_][A-Za-z0-9_]*)\s*(\[\s*\])?$/.exec(a.trim());
    return m ? m[1] : '';
  });
}

const ORIG_POS = new Map();   // primitive name -> {x, y, w}
const ORIG_NOSIG = new Set(); // a leaf whose argument positions are unknown
(function loadOriginalSignatures() {
  const db = JSON.parse(fs.readFileSync(path.join(ROOT, 'docs', 'symbols.json'), 'utf8'));
  const rows = Array.isArray(db) ? db : (db.functions || db.symbols || []);
  const sigs = new Map();
  for (const r of rows) if (r.name && r.signature) sigs.set(r.name, r.signature);
  for (const leaf of sd.LEAF) {
    const sig = sigs.get(leaf);
    const p = sig ? positions(cParams(sig) || []) : null;
    if (p) ORIG_POS.set(leaf, p); else ORIG_NOSIG.add(leaf);
  }
})();

// Our side. `impl Pen` in `crates/l2-game/src/shell/mod.rs`, plus the free
// functions there and the chrome blitters in `crates/l2-view/src`, all read the
// same way: a `fn` whose parameters include `x` and `y` draws at `(x, y)`.
//
// `&self` and `canvas` are stripped so the index matches a call site's argument
// list — `pen.body(canvas, x, y, …)` is written with the canvas first and the
// receiver before the dot.
const OURS_POS = new Map();
(function loadOurSignatures() {
  const files = [
    path.join(ROOT, 'crates', 'l2-game', 'src', 'shell', 'mod.rs'),
    path.join(ROOT, 'crates', 'l2-view', 'src', 'chrome.rs'),
    path.join(ROOT, 'crates', 'l2-view', 'src', 'text.rs'),
    path.join(ROOT, 'crates', 'l2-view', 'src', 'widget.rs'),
  ];
  for (const f of files) {
    if (!fs.existsSync(f)) continue;
    const src = fs.readFileSync(f, 'utf8');
    const RE = /\bfn\s+([a-z_][a-z_0-9]*)\s*(?=\()/g;
    let m;
    while ((m = RE.exec(src))) {
      const args = sd.splitArgs(sd.balanced(src, RE.lastIndex));
      if (!args) continue;
      const names = args
        .map(a => a.trim())
        .filter(a => a && !/^&?(mut\s+)?self$/.test(a))
        .map(a => a.split(':')[0].trim().replace(/^mut\s+/, ''))
        .filter(a => a !== 'canvas');
      const p = positions(names);
      // A method can appear in two of these files; keep the first, and never
      // let a later one silently disagree.
      if (p && !OURS_POS.has(m[1])) OURS_POS.set(m[1], p);
    }
  }
})();

// ------------------------------------------------------------------ literals
//
// `const LABEL_X: i32 = 0x60;` anywhere in the workspace. A name defined twice
// with different values resolves to neither — a coordinate we cannot pin is
// reported as uncomparable, not guessed.
const CONSTS = new Map();
(function loadConsts() {
  const walk = dir => {
    for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, e.name);
      if (e.isDirectory()) { if (e.name !== 'target') walk(p); continue; }
      if (!e.name.endsWith('.rs')) continue;
      const src = fs.readFileSync(p, 'utf8');
      const RE = /\bconst\s+([A-Z][A-Z0-9_]*)\s*:\s*[iu](?:8|16|32|64|size)\s*=\s*(-?(?:0x[0-9a-fA-F]+|\d+))\s*;/g;
      let m;
      while ((m = RE.exec(src))) {
        const v = num(m[2]);
        if (CONSTS.has(m[1]) && CONSTS.get(m[1]) !== v) CONSTS.set(m[1], null);
        else if (!CONSTS.has(m[1])) CONSTS.set(m[1], v);
      }
    }
  };
  walk(path.join(ROOT, 'crates'));
})();

function num(s) {
  const t = String(s).trim();
  if (/^-?0x[0-9a-fA-F]+$/.test(t)) return parseInt(t, 16);
  if (/^-?\d+$/.test(t)) return parseInt(t, 10);
  return null;
}

/** A literal, a named constant, or `null` for anything computed. */
function value(expr, side) {
  if (expr == null) return null;
  let t = String(expr).trim();
  // The decompiler casts freely: `(int)0x20`, `(uint)0x20`.
  t = t.replace(/^\((?:u?int|short|byte|char|undefined4?)\)\s*/, '').trim();
  const n = num(t);
  if (n !== null) return n;
  if (side === 'ours') {
    const path = /^(?:[A-Za-z_][A-Za-z0-9_]*::)*([A-Z][A-Z0-9_]*)$/.exec(t);
    if (path) { const v = CONSTS.get(path[1]); return v === undefined ? null : v; }
  }
  return null;
}

// -------------------------------------------------------------- the two sides
function originalPoints(roots) {
  const pts = [], blind = [];
  for (const s of sd.originalSites(roots)) {
    if (s.missing) continue;
    const p = ORIG_POS.get(s.name);
    if (!p) { blind.push({ why: 'no coordinate in the signature', ...s }); continue; }
    if (!s.args) { blind.push({ why: 'unparsed argument list', ...s }); continue; }
    const x = value(s.args[p.x]), y = value(s.args[p.y]);
    if (x === null || y === null) { blind.push({ why: 'computed', ...s }); continue; }
    const w = p.w === null ? null : value(s.args[p.w]);
    pts.push({ x, y, w, name: s.name, from: s.from, text: s.text });
  }
  return { pts, blind };
}

/**
 * One level of inlining, and only where it cannot be wrong.
 *
 * `delta_row(&pen, canvas, y, label, value)` draws at a fixed x inside itself.
 * Without this, every row of four county panels is uncomparable and the
 * denominator loses its most-populated screens. With more than one level it
 * would become a Rust interpreter, so: a local `fn` in the same module whose
 * body makes `Pen` calls is expanded once, substituting an argument for a
 * parameter where the call site passes a literal and leaving the coordinate
 * uncomparable where it does not.
 */
function inlineHelpers(src) {
  const helpers = new Map();
  const RE = /\bfn\s+([a-z_][a-z_0-9]*)\s*(?=\()/g;
  let m;
  while ((m = RE.exec(src))) {
    const argText = sd.balanced(src, RE.lastIndex);
    const args = sd.splitArgs(argText);
    if (!args) continue;
    const names = args.map(a => a.trim()).filter(Boolean)
      .map(a => a.split(':')[0].trim().replace(/^mut\s+/, ''));
    // The body: from the `{` after the signature to its match.
    const open = src.indexOf('{', RE.lastIndex + (argText ? argText.length : 0));
    if (open < 0) continue;
    let depth = 0, end = -1;
    for (let i = open; i < src.length; i++) {
      const c = src[i];
      if (c === '"') { for (i++; i < src.length && src[i] !== '"'; i++) if (src[i] === '\\') i++; continue; }
      if (c === '{') depth++;
      else if (c === '}') { if (--depth === 0) { end = i; break; } }
    }
    if (end < 0) continue;
    const inner = callsIn(src.slice(open, end), 'ours');
    if (inner.length) helpers.set(m[1], { params: names, calls: inner });
  }
  return helpers;
}

/** Every `recv.method(...)` / `mod::fn(...)` call in a slab of Rust. */
function callsIn(src, side) {
  // Strip comments; keep offsets stable by blanking rather than deleting, so a
  // reported line number is the line in the file.
  let s = src.replace(/\/\/[^\n]*/g, t => ' '.repeat(t.length));
  s = s.replace(/\/\*[\s\S]*?\*\//g, t => t.replace(/[^\n]/g, ' '));
  const out = [];
  const RE = /(?:([A-Za-z_][A-Za-z0-9_]*)\s*\.\s*|(?:[a-z_][a-z0-9_]*::)+)?([a-z_][a-z_0-9]*)\s*\(/g;
  let m;
  while ((m = RE.exec(s))) {
    const args = sd.splitArgs(sd.balanced(s, RE.lastIndex - 1));
    if (!args) continue;
    out.push({ recv: m[1] || null, name: m[2], args, at: m.index });
  }
  return out;
}

function ourPoints(moduleFile) {
  const p = path.join(SCREENS_DIR, moduleFile);
  if (!fs.existsSync(p)) return null;
  const src = fs.readFileSync(p, 'utf8');
  const lineAt = off => src.slice(0, off).split('\n').length;
  const helpers = inlineHelpers(src);
  const pts = [], blind = [];

  const record = (name, args, off, via) => {
    const pos = OURS_POS.get(name);
    if (!pos) return false;
    // `canvas` is the first argument at every call site and is not a parameter
    // in the position table, so drop it before indexing.
    const a = args[0] && /canvas/.test(args[0]) ? args.slice(1) : args;
    const x = value(a[pos.x], 'ours'), y = value(a[pos.y], 'ours');
    const site = { name, line: lineAt(off), via, text: a.join(', ').replace(/\s+/g, ' ').slice(0, 90) };
    if (x === null || y === null) { blind.push({ why: 'computed', ...site }); return true; }
    const w = pos.w === null ? null : value(a[pos.w], 'ours');
    pts.push({ x, y, w, ...site });
    return true;
  };

  for (const c of callsIn(src, 'ours')) {
    if (record(c.name, c.args, c.at)) continue;
    const h = helpers.get(c.name);
    if (!h) continue;
    // One level: bind the helper's parameters to this call site's arguments,
    // then re-read the helper's own draws through that binding.
    const bind = new Map();
    h.params.forEach((pn, i) => bind.set(pn, c.args[i]));
    for (const inner of h.calls) {
      const sub = inner.args.map(a => {
        const t = a.trim();
        return bind.has(t) ? bind.get(t) : a;
      });
      record(inner.name, sub, c.at, c.name);
    }
  }
  return { pts, blind };
}

// ---------------------------------------------------------------- the matching
function match(orig, ours) {
  const a = orig.map((p, i) => ({ ...p, i })), b = ours.map((p, i) => ({ ...p, i }));
  const usedA = new Set(), usedB = new Set(), pairs = [];
  // Exact first, so a real coincidence cannot steal an exact partner.
  for (const p of a) {
    const q = b.find(q => !usedB.has(q.i) && q.x === p.x && q.y === p.y);
    if (q) { usedA.add(p.i); usedB.add(q.i); pairs.push({ o: p, u: q, d: 0 }); }
  }
  const cand = [];
  for (const p of a) {
    if (usedA.has(p.i)) continue;
    for (const q of b) {
      if (usedB.has(q.i)) continue;
      const d = Math.abs(p.x - q.x) + Math.abs(p.y - q.y);
      if (d <= NEAR) cand.push({ o: p, u: q, d });
    }
  }
  cand.sort((m, n) => m.d - n.d || m.o.i - n.o.i);
  for (const c of cand) {
    if (usedA.has(c.o.i) || usedB.has(c.u.i)) continue;
    usedA.add(c.o.i); usedB.add(c.u.i); pairs.push(c);
  }
  return {
    pairs,
    origOnly: a.filter(p => !usedA.has(p.i)),
    oursOnly: b.filter(q => !usedB.has(q.i)),
  };
}

// ------------------------------------------------------------------ the report
function measure() {
  const inv = sd.inventory();
  const byModule = new Map();
  for (const rec of inv) {
    if (!rec.module) continue;
    if (!byModule.has(rec.module)) byModule.set(rec.module, { module: rec.module, screens: [], roots: [] });
    const g = byModule.get(rec.module);
    g.screens.push(rec.screen);
    for (const r of rec.roots || []) if (!g.roots.includes(r)) g.roots.push(r);
  }
  const out = [];
  for (const g of byModule.values()) {
    const o = originalPoints(g.roots);
    const u = ourPoints(g.module);
    if (!u) continue;
    const m = match(o.pts, u.pts);
    out.push({ ...g, o, u, m });
  }
  return out;
}

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
        `ours (${p.u.x},${p.u.y}) ${p.u.name}${p.u.via ? ` via ${p.u.via}` : ''} ${r.module}:${p.u.line}${w}`);
    }
    console.log(`  --- the original draws at ${r.m.origOnly.length} points with nothing of ours near:`);
    for (const p of r.m.origOnly) console.log(`      (${p.x},${p.y}) ${p.name} <- ${p.from}  ${p.text.slice(0, 80)}`);
    console.log(`  --- we draw at ${r.m.oursOnly.length} points with nothing of the original's near:`);
    for (const p of r.m.oursOnly) console.log(`      (${p.x},${p.y}) ${p.name} ${r.module}:${p.line}`);
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

  if (process.argv.includes('--check')) {
    const bad = near.filter(n => n.kind === 'point' && !ALLOWED.has(key(n)));
    const badw = near.filter(n => n.kind === 'width' && !ALLOWED.has(key(n)));
    if (bad.length || badw.length) {
      console.error('');
      for (const n of [...bad, ...badw]) {
        console.error(`drawcoords: ${n.module}:${n.p.u.line} draws at (${n.p.u.x},${n.p.u.y}) ` +
          `where ${n.p.o.from} draws ${n.p.o.name} at (${n.p.o.x},${n.p.o.y})` +
          (n.kind === 'width' ? ` with width ${n.p.o.w}, not ${n.p.u.w}` : ` — ${n.d} px out`));
      }
      process.exit(1);
    }
    console.log(`\ndrawcoords: ${pairs} compared, ${exact} exact, no unexplained near miss.`);
  }
}

// A near miss that is **deliberate** goes here with its reason, and nowhere
// else. The list is asserted non-growing by
// `crates/l2-game/tests/draws.rs::coordinates_are_the_originals`: a pair that
// drifts and is quietly added here is the failure this whole tool exists to
// stop, so each line has to say why the original's own number is wrong for us.
const ALLOWED = new Map([
  // key -> reason
]);
function key(n) { return `${n.module}:${n.p.o.x},${n.p.o.y}->${n.p.u.x},${n.p.u.y}`; }

module.exports = { measure, nearMisses, NEAR, ORIG_POS, OURS_POS, ALLOWED, key };
if (require.main === module) main();
