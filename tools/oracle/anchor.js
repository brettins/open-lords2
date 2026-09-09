// Anchor unnamed functions to a subject, mechanically.
//
//   node tools/oracle/anchor.js strings              # what L2.eng text does each function draw
//   node tools/oracle/anchor.js strings --group 80   # ... only functions drawing this group
//   node tools/oracle/anchor.js stride               # which record type does it index
//   node tools/oracle/anchor.js stride --kind county
//   node tools/oracle/anchor.js screens              # screen id -> painter / widgets / input
//   node tools/oracle/anchor.js imports              # DirectDraw, file I/O, winmm, ...
//   node tools/oracle/anchor.js assets               # .pl8 / .wav / .256 filenames referenced
//   node tools/oracle/anchor.js dark                 # how much of the binary is really unanalysable
//   node tools/oracle/anchor.js fields               # which DAT_ names are really record fields
//   node tools/oracle/anchor.js leverage             # which unnamed global would anchor the most
//   node tools/oracle/anchor.js candidates           # everything above, joined, ranked
//
// L2.eng is found via $LORDS2_DIR, or --eng <path>.
//
// # What this is
//
// Two filters, both mechanical, both shown to work. Neither of them names a
// function; both of them say what a function is *about*, which is the step that
// was missing. Read docs/method.md section 8 before trusting either.
//
// **Filter 1 - record-stride arithmetic.** The binary indexes every record array
// by a fixed stride. A function computing `[base + i * 0x300]` is touching
// counties whether or not anybody has named `base`. That survives the base
// pointer being an unnamed DAT_, which is the usual reason a function looks dark.
//
// **Filter 2 - the string ids are a confession.** Eng_DrawString(group, index)
// and its four siblings take literal arguments at almost every call site, and
// L2.eng ships a descriptive label as index 0 of every group. A function drawing
// group 80 is drawing "A Battle is to be fought."
//
// # What it does not claim
//
// The same disclaimer xref.js carries, for the same reason (correction C3): this
// is a hypothesis generator. "Draws group 71, whose label is 'Select a castle to
// build'" is a fact. "Is the castle-building screen" is a guess, and this batch
// found the guess wrong often enough to matter - group 11's label is "Lords of
// the Realm 2" and its second string is the game's tagline, so a function
// drawing group 11 is the main menu and not, as the label's neighbours suggest,
// anything to do with sieges.
//
// Every name that came out of this still needed a check that could have failed.
// The measured error rate is in docs/method.md section 8.

const fs = require('fs');
const path = require('path');

// **Finding the corpus from a worktree.**
//
// `tools/oracle/decomp/` is gitignored and lives only in the checkout that
// built it, so `__dirname/decomp` is empty for every agent working in a
// `git worktree` — which is now all of them. That made the two tools most
// likely to PREVENT a wrong name the two tools nobody could run.
//
// Resolution order, the same one `logstrings.js` uses:
//   --decomp <dir>      an explicit path
//   $LORDS2_DECOMP      the environment, for a session-wide setting
//   __dirname/decomp    this checkout, which is right in the main one
//   ../../../<main>/... the common git dir, worked out below
//
// The last is the useful one: `git rev-parse --git-common-dir` names the main
// checkout even from inside a worktree, so an agent gets the corpus with no
// flag and no setup at all. It is a fallback rather than the default because
// an explicit path must always win.
function findCorpus(dirname) {
  const path = require("path");
  const fs = require("fs");
  const i = process.argv.indexOf("--decomp");
  const explicit = i >= 0 ? process.argv[i + 1] : process.env.LORDS2_DECOMP;
  if (explicit) return explicit;
  const here = path.join(dirname, "decomp");
  if (fs.existsSync(here)) return here;
  try {
    const { execFileSync } = require("child_process");
    const common = execFileSync("git", ["rev-parse", "--git-common-dir"], {
      cwd: dirname,
      encoding: "utf8",
    }).trim();
    // `<main>/.git` -> `<main>`; a worktree reports the main checkout's.
    const main = path.resolve(dirname, common, "..");
    const there = path.join(main, "tools", "oracle", "decomp");
    if (fs.existsSync(there)) return there;
  } catch {
    // no git, or not a checkout: fall through to the local path so the
    // error message below names something a reader recognises.
  }
  return here;
}
const DECOMP = findCorpus(__dirname);
const XREF = path.join(__dirname, 'out', 'xref.json');

// ---------------------------------------------------------------- L2.eng

// The five primitives that take an (L2.eng group, index) pair, and where in the
// argument list the pair sits. Everything else reaches text through these.
const ENG_CALLS = {
  Eng_DrawString: [0, 1],   // 0x00402D37  draw one string
  Eng_Seek:       [0, 1],   // 0x004018D7  leave a pointer in g_engCursor
  Ui_DrawCentred: [0, 1],   // centred in a box
  FUN_0040328e:   [0, 1],   // word-wrapped paragraph, takes a width
  // Copy into a buffer: (dest, group, index, len). BOTH spellings are listed on
  // purpose. symbols.json named 0x004017BF Eng_CopyString, so decompile-all.ps1
  // emits the name and the old FUN_ key stopped matching anything - silently,
  // because a key that matches nothing looks exactly like a primitive nobody
  // calls. That is what put this tool's count at 65 when the real direct-literal
  // count is 67 (docs/formats/eng.md 5.1). Rename a primitive here and in
  // symbols.json at the same time, and keep the old key until the corpus is
  // rebuilt.
  Eng_CopyString: [1, 2],
  FUN_004017bf:   [1, 2],
};

function loadEng(engPath) {
  if (!engPath) {
    const dir = process.env.LORDS2_DIR;
    if (dir) {
      for (const n of ['L2.eng', 'l2.eng', 'L2.ENG']) {
        const p = path.join(dir, n);
        if (fs.existsSync(p)) { engPath = p; break; }
      }
    }
  }
  if (!engPath || !fs.existsSync(engPath)) return null;
  const buf = fs.readFileSync(engPath);
  const off = g => buf[8 + g * 4] | (buf[9 + g * 4] << 8) | (buf[10 + g * 4] << 16);
  const n = (off(1) - 8) / 4;
  const groups = {};
  for (let g = 1; g < n; g++) {
    const start = off(g), end = (g + 1 < n) ? off(g + 1) : buf.length;
    const strs = [];
    let p = start;
    while (p < end) {
      let q = p; while (q < end && buf[q] !== 0) q++;
      strs.push(buf.slice(p, q).toString('latin1'));
      p = q + 1;
      while (p < end && buf[p] < 0x20) p++;   // Eng_DrawString skips sub-0x20 filler
    }
    groups[g] = strs;
  }
  return groups;
}

// ------------------------------------------------------- the corpus, parsed

const HEADER = /^\/\/ ==== ([0-9a-f]{8})\s+(\S+)\s+params=(-?\d+)\s+bytes=(\d+)/;

function loadCorpus() {
  if (!fs.existsSync(DECOMP)) {
    console.error(`no decompilation at ${DECOMP}`);
    console.error('run tools/oracle/decompile-all.ps1 first (about 22 seconds)');
    process.exit(1);
  }
  const fns = [];
  for (const file of fs.readdirSync(DECOMP).filter(f => f.endsWith('.c'))) {
    const lines = fs.readFileSync(path.join(DECOMP, file), 'utf8').split('\n');
    let cur = null;
    for (const line of lines) {
      const h = HEADER.exec(line);
      if (h) {
        cur = { addr: h[1], name: h[2], params: +h[3], bytes: +h[4], file, body: [] };
        cur.named = !cur.name.startsWith('FUN_');
        fns.push(cur);
        continue;
      }
      if (cur) cur.body.push(line);
    }
  }
  for (const f of fns) f.text = f.body.join('\n');
  return fns;
}

// ------------------------------------------------------------- filter 2

// Match `Prim(a,b,...)` and pull the literal group/index out. Ghidra spells small
// integers as decimal or 0xNN, and a char-typed argument as '\x12' or 'B'.
function litNum(tok) {
  tok = tok.trim();
  if (/^0x[0-9a-f]+$/i.test(tok)) return parseInt(tok, 16);
  if (/^-?\d+$/.test(tok)) return +tok;
  // C character escapes. The previous regex read '\b' as the *letter* b, because
  // \\? swallowed the backslash and let (.) match what followed, and '\0' as the
  // digit '0'. That put six screen ids in the wrong place - 0x08 reported as
  // 0x62, 0x09 as 0x74, 0x0a as 0x6e, 0x0b as 0x76, 0x0c as 0x66, 0x00 as 0x30 -
  // which is every management screen the game has. Verified against the arms of
  // Screen_Draw read by hand.
  const ESC = { a: 7, b: 8, t: 9, n: 10, v: 11, f: 12, r: 13, e: 27, '\\': 92, "'": 39, '"': 34, '?': 63 };
  const c = /^'(?:\\x([0-9a-f]{1,2})|\\([0-7]{1,3})|\\(.)|(.))'$/i.exec(tok);
  if (!c) return null;
  if (c[1] !== undefined) return parseInt(c[1], 16);
  if (c[2] !== undefined) return parseInt(c[2], 8);
  if (c[3] !== undefined) return ESC[c[3]] !== undefined ? ESC[c[3]] : c[3].charCodeAt(0);
  return c[4].charCodeAt(0);
}

// Split a call's argument list at top level - arguments contain commas inside
// nested calls and casts, so a plain split(',') is wrong often enough to matter.
function splitArgs(s) {
  const out = []; let depth = 0, cur = '';
  for (const ch of s) {
    if (ch === '(' || ch === '[') depth++;
    else if (ch === ')' || ch === ']') depth--;
    if (ch === ',' && depth === 0) { out.push(cur); cur = ''; continue; }
    cur += ch;
  }
  out.push(cur);
  return out;
}

function engHits(fn) {
  const hits = [];       // { prim, group, index|null }
  for (const [prim, [gi, ii]] of Object.entries(ENG_CALLS)) {
    const re = new RegExp(`\\b${prim}\\s*\\(`, 'g');
    let m;
    while ((m = re.exec(fn.text)) !== null) {
      // Walk to the matching close paren.
      let i = m.index + m[0].length, depth = 1, start = i;
      while (i < fn.text.length && depth > 0) {
        if (fn.text[i] === '(') depth++;
        else if (fn.text[i] === ')') depth--;
        i++;
      }
      const args = splitArgs(fn.text.slice(start, i - 1));
      const g = litNum(args[gi] ?? '');
      if (g === null || g < 1 || g > 400) continue;   // a computed group tells us nothing
      hits.push({ prim, group: g, index: litNum(args[ii] ?? '') });
    }
  }
  return hits;
}

// ------------------------------------------------------------- filter 1

// The record strides. Every one of these is established elsewhere in the docs,
// which is what makes the arithmetic evidence rather than numerology:
//   0x300 county     docs/kingdom.md      0x1a4 unit    docs/armies.md
//   0x160 realm      docs/kingdom.md      0x34  battle unit / 0x1b0 battle man  docs/battle.md
const STRIDES = [
  { kind: 'county',     stride: 0x300, doc: 'docs/kingdom.md' },
  { kind: 'unit',       stride: 0x1a4, doc: 'docs/armies.md' },
  { kind: 'realm',      stride: 0x160, doc: 'docs/kingdom.md' },
  { kind: 'battleMan',  stride: 0x1b0, doc: 'docs/battle.md' },
  { kind: 'battleUnit', stride: 0x34,  doc: 'docs/battle.md' },
];

function strideHits(fn) {
  const out = {};
  for (const { kind, stride } of STRIDES) {
    const hex = '0x' + stride.toString(16);
    // `i * 0x300`, `0x300 * i`, and Ghidra's `[i * 0x300]` all reduce to this.
    const re = new RegExp(`(\\*\\s*${hex}\\b)|(\\b${hex}\\s*\\*)`, 'gi');
    const n = (fn.text.match(re) || []).length;
    if (n) out[kind] = n;
  }
  return out;
}

// ------------------------------------------------------------- other signals

// Ghidra names a string constant after its contents and its address, so
// "icon.tmp.pl8" becomes the identifier s_icon_tmp_pl8_004d4428. The trailing
// address is what makes this unambiguous - and it is also why a naive \b after
// the extension never matches.
const ASSET = /\bs_([A-Za-z0-9_]+)_(pl8|wav|256|smk|dat|eng|sav|skr|bmp)_[0-9a-f]{8}\b/g;

const IMPORT_FAMILIES = {
  render:  /\b(DirectDraw\w*|IDirectDraw\w*|Blt\w*|Flip|Lock|Unlock|GetDC)\b/,
  fileio:  /\b(File_ReadChunk|_open|_read|_write|_lseek|_close|fopen|fread|fwrite)\b/,
  sound:   /\b(timeGetTime|midi\w+|wave\w+|mci\w+)\b/,
  input:   /\b(GetAsyncKeyState|GetCursorPos|SetCursorPos|PeekMessage\w*)\b/,
};

// ------------------------------------------------------------- commands

function fmtStr(s, n = 58) {
  s = s.replace(/\s+/g, ' ').trim();
  return JSON.stringify(s.length > n ? s.slice(0, n - 1) + '…' : s);
}

function cmdStrings(fns, eng, opt) {
  if (!eng) { console.error('need L2.eng: set LORDS2_DIR or pass --eng <path>'); process.exit(1); }
  const rows = [];
  for (const f of fns) {
    const hits = engHits(f);
    if (!hits.length) continue;
    const groups = [...new Set(hits.map(h => h.group))].sort((a, b) => a - b);
    if (opt.group && !groups.includes(+opt.group)) continue;
    if (opt.unnamedOnly && f.named) continue;
    rows.push({ f, hits, groups });
  }
  rows.sort((a, b) => b.f.bytes - a.f.bytes);
  console.log(`${rows.length} functions draw at least one literal L2.eng group`);
  console.log(`(${rows.filter(r => !r.f.named).length} of them unnamed)\n`);
  for (const { f, hits, groups } of rows.slice(0, +(opt.limit || 40))) {
    console.log(`${f.named ? f.name : '0x' + f.addr}  ${f.bytes}b  ${f.named ? '' : '[unnamed]'}`);
    for (const g of groups) {
      const label = eng[g] ? fmtStr(eng[g][0]) : '(no such group)';
      console.log(`    group ${String(g).padStart(3)}  label ${label}`);
      const idx = [...new Set(hits.filter(h => h.group === g && h.index !== null).map(h => h.index))]
        .sort((a, b) => a - b);
      for (const i of idx.slice(0, 8)) {
        const s = eng[g] && eng[g][i] !== undefined ? fmtStr(eng[g][i], 48) : '(out of range)';
        console.log(`        [${String(i).padStart(3)}] ${s}`);
      }
      if (idx.length > 8) console.log(`        ... ${idx.length - 8} more indices`);
    }
    console.log('');
  }
}

function cmdStride(fns, opt) {
  const rows = [];
  for (const f of fns) {
    const h = strideHits(f);
    if (!Object.keys(h).length) continue;
    if (opt.kind && !(opt.kind in h)) continue;
    if (opt.unnamedOnly && f.named) continue;
    rows.push({ f, h });
  }
  const byKind = {};
  for (const { f, h } of rows) for (const k of Object.keys(h)) {
    (byKind[k] ||= { all: 0, unnamed: 0 });
    byKind[k].all++; if (!f.named) byKind[k].unnamed++;
  }
  console.log(`${rows.length} functions index a known record stride`);
  console.log(`(${rows.filter(r => !r.f.named).length} of them unnamed)\n`);
  console.log('  record        stride   functions   unnamed');
  for (const { kind, stride } of STRIDES) {
    const s = byKind[kind] || { all: 0, unnamed: 0 };
    console.log(`  ${kind.padEnd(12)}  0x${stride.toString(16).padStart(4, '0')}   ${String(s.all).padStart(9)}   ${String(s.unnamed).padStart(7)}`);
  }
  console.log('');
  rows.sort((a, b) => b.f.bytes - a.f.bytes);
  for (const { f, h } of rows.slice(0, +(opt.limit || 40))) {
    const tag = Object.entries(h).map(([k, n]) => `${k}×${n}`).join(' ');
    console.log(`  ${(f.named ? f.name : '0x' + f.addr).padEnd(28)} ${String(f.bytes).padStart(5)}b  ${tag}`);
  }
}

// The screen-id dispatch is a third mechanical anchor and it fell out of the
// first two: every screen painter is reached from exactly one `g_screenId == N`
// arm of Screen_Draw, and the same id appears in Screen_DrawWidgets and
// Screen_HandleInput. Three independent tables agreeing on an id is a check the
// hypothesis could fail, which is why this is worth printing.
const DISPATCHERS = ['Screen_Draw', 'Screen_DrawWidgets', 'Screen_HandleInput'];

function screenTable(fns) {
  const table = new Map();      // id -> { draw, widgets, input }
  for (const d of DISPATCHERS) {
    const f = fns.find(x => x.name === d);
    if (!f) continue;
    // The arm's first call is the screen's handler. Skip C keywords - an arm may
    // open with `if (firstFrame == 1)` before getting to the call that matters.
    const KEYWORD = /^(if|else|while|for|switch|return|do|sizeof)$/;
    const re = /g_screenId == ('(?:\\x[0-9a-f]{2}|\\.|.)'|0x[0-9a-f]+|\d+)\)([\s\S]{0,400}?)(?=\n\s*\}\s*\n?\s*else|$)/g;
    let m;
    while ((m = re.exec(f.text)) !== null) {
      const id = litNum(m[1]);
      if (id === null) continue;
      const call = [...m[2].matchAll(/(\w+)\s*\(/g)].map(x => x[1]).find(n => !KEYWORD.test(n));
      if (!call) continue;
      if (!table.has(id)) table.set(id, {});
      const slot = d === 'Screen_Draw' ? 'draw' : d === 'Screen_DrawWidgets' ? 'widgets' : 'input';
      if (!table.get(id)[slot]) table.get(id)[slot] = call;
    }
  }
  return table;
}

function cmdScreens(fns, eng) {
  const table = screenTable(fns);
  const byName = new Map(fns.map(f => [f.name, f]));
  console.log(`${table.size} screen ids dispatched. A painter named in two or three of the`);
  console.log('three tables is corroborated; one appearing in only one is not.\n');
  console.log('   id  painter                       widgets              input                groups drawn');
  for (const id of [...table.keys()].sort((a, b) => a - b)) {
    const t = table.get(id);
    const p = t.draw && byName.get(t.draw);
    const groups = p ? [...new Set(engHits(p).map(h => h.group))].sort((a, b) => a - b) : [];
    const gs = groups.length
      ? groups.map(g => `${g}${eng && eng[g] ? '(' + fmtStr(eng[g][0], 26) + ')' : ''}`).join(' ')
      : '';
    const show = n => (n ? (n.startsWith('FUN_') ? '@' + n.slice(4) : n) : '-');
    console.log(`  0x${id.toString(16).padStart(2, '0')}  ${show(t.draw).padEnd(28)}  ${show(t.widgets).padEnd(19)}  ${show(t.input).padEnd(19)}  ${gs}`);
  }
}

function cmdAssets(fns, opt) {
  const rows = [];
  for (const f of fns) {
    if (opt.unnamedOnly && f.named) continue;
    const names = new Set();
    let m; ASSET.lastIndex = 0;
    while ((m = ASSET.exec(f.text)) !== null) names.add(`${m[1].replace(/_/g, ".")}.${m[2]}`);
    if (names.size) rows.push({ f, names: [...names] });
  }
  rows.sort((a, b) => b.f.bytes - a.f.bytes);
  console.log(`${rows.length} functions reference an asset filename (${rows.filter(r => !r.f.named).length} unnamed)\n`);
  for (const { f, names } of rows.slice(0, +(opt.limit || 40))) {
    console.log(`  ${(f.named ? f.name : '0x' + f.addr).padEnd(28)} ${String(f.bytes).padStart(5)}b  ${names.slice(0, 6).join(' ')}`);
  }
}

function cmdImports(fns, opt) {
  const tally = {};
  const rows = [];
  for (const f of fns) {
    if (opt.unnamedOnly && f.named) continue;
    const fam = Object.entries(IMPORT_FAMILIES).filter(([, re]) => re.test(f.text)).map(([k]) => k);
    if (!fam.length) continue;
    for (const k of fam) tally[k] = (tally[k] || 0) + 1;
    rows.push({ f, fam });
  }
  console.log('functions touching each API family (unnamed only: ' + !!opt.unnamedOnly + ')\n');
  for (const [k, n] of Object.entries(tally).sort((a, b) => b[1] - a[1])) {
    console.log(`  ${k.padEnd(8)} ${String(n).padStart(5)}`);
  }
  console.log('');
  rows.sort((a, b) => b.f.bytes - a.f.bytes);
  for (const { f, fam } of rows.slice(0, +(opt.limit || 25))) {
    console.log(`  ${(f.named ? f.name : '0x' + f.addr).padEnd(28)} ${String(f.bytes).padStart(5)}b  ${fam.join(' ')}`);
  }
}

// The correction this command exists to make. "844 dark functions" was repeated
// as though that many functions were unanalysable. They are not: a function with
// no *named* global still has globals, and naming one anchors all of its readers
// at once. The functions that genuinely touch nothing are few and tiny.
function cmdDark(fns) {
  const xr = fs.existsSync(XREF) ? JSON.parse(fs.readFileSync(XREF, 'utf8')).functions : null;
  if (!xr) { console.error('run: node tools/oracle/xref.js build'); process.exit(1); }
  const all = Object.entries(xr);
  const unnamed = all.filter(([, f]) => !f.named);
  const noNamedGlobal = unnamed.filter(([, f]) => !f.globals.some(g => !/^_?DAT_/.test(g)));
  const noGlobalAtAll = unnamed.filter(([, f]) => f.globals.length === 0);
  const med = list => {
    const s = list.map(([, f]) => f.bytes).sort((a, b) => a - b);
    return s.length ? s[s.length >> 1] : 0;
  };
  console.log(`${all.length} functions, ${all.length - unnamed.length} named, ${unnamed.length} unnamed\n`);
  console.log('  the honest breakdown of the unnamed:\n');
  console.log(`  ${String(noGlobalAtAll.length).padStart(5)}  touch NO global at all      median ${med(noGlobalAtAll)}b   <- the only genuinely dark ones`);
  console.log(`  ${String(noNamedGlobal.length - noGlobalAtAll.length).padStart(5)}  touch only unnamed globals  median ${med(noNamedGlobal.filter(x => x[1].globals.length))}b   <- anchored the moment a global is named`);
  console.log(`  ${String(unnamed.length - noNamedGlobal.length).padStart(5)}  touch a named global        median ${med(unnamed.filter(x => x[1].globals.some(g => !/^_?DAT_/.test(g))))}b   <- already in a cluster`);
  const tiny = noGlobalAtAll.filter(([, f]) => f.bytes <= 40).length;
  console.log(`\n  ${tiny} of the ${noGlobalAtAll.length} genuinely dark functions are 40 bytes or less.`);
  console.log('  At that size they are accessors and thunks, not mysteries.');
  const globals = new Set(all.flatMap(([, f]) => f.globals));
  const named = [...globals].filter(g => !/^_?DAT_/.test(g)).length;
  console.log(`\n  ${globals.size} distinct globals, ${named} named. That ratio, not the function`);
  console.log('  count, is what bounds how much of the binary reads as prose.');
}

// The bases of the five record arrays, from docs/symbols.json. Ghidra applies a
// name to one address, so `g_counties` labels 0x0053F9B0 and every *field* of
// every county gets its own invented DAT_ - DAT_0053f9b5 is county[i].owner and
// nothing else. Those are not unknown globals; they are known offsets wearing a
// disguise, and counting them as unknowns overstates how dark the binary is.
const RECORD_BASES = [
  { kind: 'county',     base: 0x0053F9B0, stride: 0x300, name: 'g_counties' },
  { kind: 'unit',       base: 0x0052F0B0, stride: 0x1a4, name: 'g_units' },
  { kind: 'realm',      base: 0x0057BF00, stride: 0x160, name: 'g_realms' },
];

function cmdFields(fns) {
  // A DAT_ that appears next to `* <stride>` is a field of that record type.
  const found = new Map();     // DAT name -> { kind, offset, sites }
  const unresolved = new Set();
  for (const f of fns) {
    for (const { kind, base, stride, name } of RECORD_BASES) {
      const re = new RegExp(`(?:DAT_([0-9a-f]{8})|\\b(${name})\\b)[^;\\n]{0,16}\\*\\s*0x${stride.toString(16)}\\b`, 'gi');
      let m;
      while ((m = re.exec(f.text)) !== null) {
        if (m[2]) continue;                       // the base itself: offset 0
        const addr = parseInt(m[1], 16);
        const off = addr - base;
        if (off < 0 || off >= stride) { unresolved.add(`DAT_${m[1]}`); continue; }
        const key = `DAT_${m[1]}`;
        if (!found.has(key)) found.set(key, { kind, offset: off, sites: 0 });
        found.get(key).sites++;
      }
    }
  }
  const byKind = {};
  for (const v of found.values()) byKind[v.kind] = (byKind[v.kind] || 0) + 1;
  console.log('Synthetic DAT_ names that are really a field of a known record array.\n');
  console.log('  record    base        distinct DAT_ names resolved');
  for (const { kind, base, name } of RECORD_BASES) {
    console.log(`  ${kind.padEnd(8)}  0x${base.toString(16).toUpperCase()}  ${String(byKind[kind] || 0).padStart(4)}   (${name})`);
  }
  const total = found.size;
  console.log(`\n  ${total} of the corpus's unnamed globals are field offsets of three arrays`);
  console.log(`  that are already named. ${unresolved.size} stride-adjacent DAT_ names did not`);
  console.log('  resolve into a record and are the ones actually worth chasing.\n');
  const rows = [...found].sort((a, b) => b[1].sites - a[1].sites).slice(0, 20);
  console.log('  sites  global              resolves to');
  for (const [g, v] of rows) {
    console.log(`  ${String(v.sites).padStart(5)}  ${g.padEnd(18)}  ${v.kind}[i] + 0x${v.offset.toString(16).toUpperCase().padStart(3, '0')}`);
  }
}

function cmdLeverage(fns) {
  const xr = fs.existsSync(XREF) ? JSON.parse(fs.readFileSync(XREF, 'utf8')).functions : null;
  if (!xr) { console.error('run: node tools/oracle/xref.js build'); process.exit(1); }
  const byName = new Map(fns.map(f => [f.name, f]));
  const score = new Map();
  for (const [n, f] of Object.entries(xr)) {
    if (f.named) continue;
    const anchored = f.globals.some(g => !/^_?DAT_/.test(g));
    for (const g of f.globals) {
      if (!/^_?DAT_/.test(g)) continue;
      if (!score.has(g)) score.set(g, { readers: 0, unanchored: 0, bytes: 0, strideKinds: new Set() });
      const s = score.get(g);
      s.readers++; s.bytes += f.bytes;
      if (!anchored) s.unanchored++;
      const src = byName.get(n);
      if (src) for (const k of Object.keys(strideHits(src))) s.strideKinds.add(k);
    }
  }
  const ranked = [...score].sort((a, b) => b[1].readers - a[1].readers);
  console.log('Globals ranked by how many unnamed functions they would anchor.');
  console.log('"unanchored" counts readers that currently touch no named global at all -');
  console.log('naming the global is the whole of what those functions get.\n');
  console.log('  readers  unanch  global              avg size  stride evidence');
  for (const [g, s] of ranked.slice(0, 30)) {
    console.log(`  ${String(s.readers).padStart(7)}  ${String(s.unanchored).padStart(6)}  ${g.padEnd(18)}  ${String(Math.round(s.bytes / s.readers)).padStart(7)}b  ${[...s.strideKinds].join(',') || '-'}`);
  }
}

function cmdCandidates(fns, eng, opt) {
  const min = +(opt.min || 100);
  const rows = [];
  for (const f of fns) {
    if (f.named || f.bytes < min) continue;
    const hits = engHits(f), str = strideHits(f);
    if (!hits.length && !Object.keys(str).length) continue;
    const groups = [...new Set(hits.map(h => h.group))].sort((a, b) => a - b);
    rows.push({ f, groups, str, score: groups.length * 2 + Object.keys(str).length });
  }
  rows.sort((a, b) => b.score - a.score || b.f.bytes - a.f.bytes);
  console.log(`${rows.length} unnamed functions >= ${min} bytes carry at least one mechanical anchor.`);
  console.log('This is a work list, not a set of findings - see the header.\n');
  for (const { f, groups, str } of rows.slice(0, +(opt.limit || 60))) {
    const s = Object.entries(str).map(([k, n]) => `${k}×${n}`).join(' ');
    const gs = groups.map(g => `${g}${eng && eng[g] ? '=' + fmtStr(eng[g][0], 30) : ''}`).join('  ');
    console.log(`  0x${f.addr}  ${String(f.bytes).padStart(5)}b  ${s.padEnd(22)}  ${gs}`);
  }
}

// ------------------------------------------------------------------ main

const argv = process.argv.slice(2);
const cmd = argv[0];
const opt = {};
for (let i = 1; i < argv.length; i++) {
  if (argv[i].startsWith('--')) {
    const k = argv[i].slice(2).replace(/-(\w)/g, (_, c) => c.toUpperCase());
    opt[k] = (argv[i + 1] && !argv[i + 1].startsWith('--')) ? argv[++i] : true;
  }
}
if (opt.unnamed) opt.unnamedOnly = true;

if (!cmd || cmd === 'help') {
  console.log(fs.readFileSync(__filename, 'utf8').split('\n').slice(2, 46).join('\n').replace(/^\/\/ ?/gm, ''));
  process.exit(0);
}

const fns = loadCorpus();
const eng = loadEng(opt.eng === true ? null : opt.eng);

switch (cmd) {
  case 'strings':    cmdStrings(fns, eng, opt); break;
  case 'stride':     cmdStride(fns, opt); break;
  case 'screens':    cmdScreens(fns, eng); break;
  case 'assets':     cmdAssets(fns, opt); break;
  case 'imports':    cmdImports(fns, opt); break;
  case 'dark':       cmdDark(fns); break;
  case 'fields':     cmdFields(fns); break;
  case 'leverage':   cmdLeverage(fns); break;
  case 'candidates': cmdCandidates(fns, eng, opt); break;
  default: console.error(`unknown command: ${cmd}`); process.exit(1);
}
