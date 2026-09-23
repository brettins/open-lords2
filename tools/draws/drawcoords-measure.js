#!/usr/bin/env node
// The measurement behind `drawcoords.js`: both sides of a screen reduced to
// the literal points they draw at, and the pairing between them. Split out at
// the file-size cap; the argument is in `drawcoords.js`'s header.
'use strict';
const fs = require('fs');
const path = require('path');
const sd = require('./screendraws.js');

const ROOT = path.join(__dirname, '..', '..');
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
  // Case-folded too: the decompiler writes `FUN_0040328e` and every document
  // writes `FUN_0040328E`, so an exact lookup finds neither from the other.
  for (const r of rows) if (r.name && r.signature) sigs.set(r.name.toLowerCase(), r.signature);
  for (const leaf of sd.LEAF) {
    const sig = sigs.get(leaf.toLowerCase());
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
//
// **Only a name `screendraws.js` already calls a draw of ours.** Harvesting
// every `fn` with `x`/`y` swept in `merchant_grid(x, y)` and the hit tests, so
// our blind count was an over-count of unknown size; `sd.OURS` is the curated
// list that names each of our methods against the original it stands for, and
// a method absent from it is not a draw. The roots are walked rather than
// listed because `shell/mod.rs` became `shell/drawing/pen.rs` and a fixed list
// silently lost every `Pen` signature.
const OURS_POS = new Map();
(function loadOurSignatures() {
  const files = [];
  const walk = d => {
    if (!fs.existsSync(d)) return;
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.endsWith('.rs')) files.push(p);
    }
  };
  walk(path.join(ROOT, 'crates', 'l2-game', 'src', 'shell'));
  walk(path.join(ROOT, 'crates', 'l2-view', 'src'));
  for (const f of files) {
    const src = fs.readFileSync(f, 'utf8');
    const RE = /\bfn\s+([a-z_][a-z_0-9]*)\s*(?=\()/g;
    let m;
    while ((m = RE.exec(src))) {
      if (!sd.OURS.has(m[1])) continue;
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
      // `ASK_AT.0` is `const ASK_AT: (i32, i32) = (0x60, 0x100);` and
      // `ADVICE_WELL.x` is `const ADVICE_WELL: Rect = Rect::new(0x50, 0xD0, …)`.
      // Both were counted as computed, and the trade panel's one comparable
      // point — the original's `Eng_DrawString(0x44,7,0x60,0x100)` — was among
      // them, so the whole screen scored zero pairs against an exact agreement.
      const TUP = /\bconst\s+([A-Z][A-Z0-9_]*)\s*:\s*(?:\((?:[iu](?:8|16|32|64|size)\s*,\s*)+[iu](?:8|16|32|64|size)\)|Rect)\s*=\s*(?:Rect::new)?\s*\(([^;]*?)\)\s*;/g;
      while ((m = TUP.exec(src))) {
        const vs = m[2].split(',').map(t => num(t));
        if (vs.some(v => v === null)) continue;
        for (const [i, field] of [[0, '0'], [1, '1'], [0, 'x'], [1, 'y']]) {
          const k = `${m[1]}.${field}`;
          if (CONSTS.has(k) && CONSTS.get(k) !== vs[i]) CONSTS.set(k, null);
          else if (!CONSTS.has(k)) CONSTS.set(k, vs[i]);
        }
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
    const path = /^(?:[A-Za-z_][A-Za-z0-9_]*::)*([A-Z][A-Z0-9_]*(?:\.(?:0|1|x|y))?)$/.exec(t);
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
function inlineHelpers(src, file) {
  const helpers = new Map();
  const RE = /\bfn\s+([a-z_][a-z_0-9]*)\s*(?=\()/g;
  let m;
  while ((m = RE.exec(src))) {
    const argText = sd.balanced(src, RE.lastIndex);
    const args = sd.splitArgs(argText);
    if (!args) continue;
    // `&self` is a parameter and never an argument: leaving it in bound the
    // receiver's slot to the call site's first real argument and shifted every
    // coordinate one place, which is how `castle/screen.rs:293` was read as
    // drawing at (15,224) when it draws at `START_AT` (224,456).
    const names = args.map(a => a.trim()).filter(Boolean)
      .filter(a => !/^&?(mut\s+)?self$/.test(a))
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
    const lineAt = off => src.slice(0, off).split('\n').length;
    const inner = callsIn(src.slice(open, end), 'ours')
      .map(c => ({ ...c, file, line: lineAt(open + c.at) }));
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
  const RE = /(?:([A-Za-z_][A-Za-z0-9_]*)\s*\.\s*|((?:[a-z_][a-z0-9_]*::)+))?([a-z_][a-z_0-9]*)\s*\(/g;
  let m;
  while ((m = RE.exec(s))) {
    const args = sd.splitArgs(sd.balanced(s, RE.lastIndex - 1));
    if (!args) continue;
    out.push({ recv: m[1] || null, qualified: !!m[2], name: m[3], args, at: m.index });
  }
  return out;
}

// A module is one screen's source and, since the screens were split into
// directories, routinely several files: `county/layout.rs` defines the helper
// `county/draw/panels.rs` calls. The helper table is therefore built over the
// whole module before any file is scanned, or the one level of inlining would
// fire only where caller and callee happened to land in the same file.
function ourPoints(moduleFile) {
  const files = sd.moduleFiles(moduleFile);
  if (!files) return null;
  const srcs = files.map(f => ({ file: f.file, src: fs.readFileSync(f.path, 'utf8') }));
  const helpers = new Map();
  for (const s of srcs) for (const [k, v] of inlineHelpers(s.src, s.file)) if (!helpers.has(k)) helpers.set(k, v);
  // **One source expression is one record.** `pen.eng(…, FROM_AT.0, …)` inside
  // `fn draw` was counted twice — once scanning the file, once inlined through
  // the `self.draw(…)` call site — and the free copy then paired with the
  // *next* original draw 18 px away, which is how `supplies` reported two near
  // misses against coordinates it already matched exactly. Keyed by the line
  // the coordinate is written on: the resolved readings win over the blind one,
  // and a helper called twice with different literals keeps both points.
  const seen = new Map();
  const at = s => `${s.file}:${s.line}:${s.name}`;

  for (const { file, src } of srcs) {
    const lineAt = off => src.slice(0, off).split('\n').length;
    // `where` is the line the coordinate is *written on*, which for an inlined
    // helper is inside the helper and not at the call site. A message naming
    // the call site sends the reader to a line with no number on it.
    const record = (name, args, off, via, where) => {
      const pos = OURS_POS.get(name);
      if (!pos) return false;
      // `canvas` is not in the position table; `number_in(face, canvas, x, y)` puts it second.
      const a = args.filter((s, i) => !(i === 0 ? /canvas/ : /^\s*canvas\s*$/).test(s));
      const x = value(a[pos.x], 'ours'), y = value(a[pos.y], 'ours');
      const site = { name, file: where ? where.file : file, line: where ? where.line : lineAt(off),
        via, text: a.join(', ').replace(/\s+/g, ' ').slice(0, 90) };
      const k = at(site);
      if (!seen.has(k)) seen.set(k, { direct: new Map(), inlined: new Map(), blind: null });
      const e = seen.get(k);
      if (x === null || y === null) { e.blind = e.blind || { why: 'computed', ...site }; return true; }
      const w = pos.w === null ? null : value(a[pos.w], 'ours');
      (via ? e.inlined : e.direct).set(`${x},${y},${w}`, { x, y, w, ...site });
      return true;
    };

    for (const c of callsIn(src, 'ours')) {
      if (record(c.name, c.args, c.at)) continue;
      const h = helpers.get(c.name);
      // `text::draw(canvas, 16, 452, …)` is not this module's `fn draw`, and
      // binding one to the other inlined a whole screen painter under a debug
      // overlay's arguments — `divide/screen.rs:291` then read as (16,392)
      // against `DAUGHTER_NUMBER_X` 0x188. A qualified path is a different
      // function, and a binding that does not cover the parameters is not one.
      if (!h || c.qualified || h.params.length !== c.args.length) continue;
      // One level: bind the helper's parameters to this call site's arguments,
      // then re-read the helper's own draws through that binding.
      const bind = new Map();
      h.params.forEach((pn, i) => bind.set(pn, c.args[i]));
      for (const inner of h.calls) {
        const sub = inner.args.map(a => {
          const t = a.trim();
          return bind.has(t) ? bind.get(t) : a;
        });
        record(inner.name, sub, c.at, `${c.name} at ${file}:${lineAt(c.at)}`, inner);
      }
    }
  }
  const pts = [], blind = [];
  for (const e of seen.values()) {
    // The direct reading wins: a coordinate the line states outright is not a
    // guess, and inlining is only there to rescue the ones it does not.
    const use = e.direct.size ? e.direct : e.inlined;
    if (use.size) pts.push(...use.values());
    else if (e.blind) blind.push(e.blind);
  }
  return { pts, blind };
}

// ---------------------------------------------------------------- the matching
//
// **A pair must be the same kind of draw, not merely two points near each
// other.** Proximity alone paired `Ui_DrawBox(16,48)` — a window frame — with
// our `heading(20,56)` — a caption — and called the 12 px between them a
// defect; six of eleven reported near misses were that mistake. The class is
// derived, not typed: `sd.OURS` already names each of our methods against the
// original it stands for (`heading: 'Ui_DrawText, heading font'`), so the
// original's names in that string *are* the class. A method that names none
// cannot pair, and its points are reported as ours alone.
// Not the primitive but its **family**, because our side reaches one original
// through several methods and the reverse: the original's
// `Eng_DrawString(0x21,1,0xf0,0x9e)` is our `pen.eng`, and its
// `Eng_DrawString(100,…,0xf0,0xb0)` is our `pen.body` over a name we looked up
// ourselves — 50 exact agreements read as unpaired when the class was the
// primitive's own name. Three families, read off the names both sides already
// carry: a rectangle, a sheet frame, and glyphs.
const family = n =>
  /Box|Rect|outline|window/i.test(n) ? 'box'
  : /^(Pl8_|Widget_)|Button|frame/i.test(n) ? 'sheet'
  : 'text';
const compatible = (o, u) => family(o.name) === family(u.name);

function match(orig, ours) {
  const a = orig.map((p, i) => ({ ...p, i })), b = ours.map((p, i) => ({ ...p, i }));
  const usedA = new Set(), usedB = new Set(), pairs = [];
  // Exact first, so a real coincidence cannot steal an exact partner.
  for (const p of a) {
    const q = b.find(q => !usedB.has(q.i) && q.x === p.x && q.y === p.y && compatible(p, q));
    if (q) { usedA.add(p.i); usedB.add(q.i); pairs.push({ o: p, u: q, d: 0 }); }
  }
  const cand = [];
  for (const p of a) {
    if (usedA.has(p.i)) continue;
    for (const q of b) {
      if (usedB.has(q.i) || !compatible(p, q)) continue;
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

module.exports = { NEAR, ORIG_POS, OURS_POS, family, measure, value };
