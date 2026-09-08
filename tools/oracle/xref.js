// Build a reference index over the whole decompiled corpus.
//
//   node tools/oracle/xref.js build                  # index decomp/ -> out/xref.json
//   node tools/oracle/xref.js touches g_counties     # who reads/writes this global
//   node tools/oracle/xref.js callers Map_RenderIso  # who calls this
//   node tools/oracle/xref.js calls   Map_RenderIso  # what it calls, unnamed first
//   node tools/oracle/xref.js reach   Map_RenderIso  # everything reachable, by depth
//   node tools/oracle/xref.js unnamed --near g_screenLattice
//   node tools/oracle/xref.js clusters               # unnamed functions grouped by
//                                                    # which named globals they touch
//
// # Why this exists
//
// The project has 2,452 decompiled functions and, until now, no index over them.
// Targets were chosen by intuition, and correction C21 is what that costs: an
// entire layer - the user interface - was never analysed, because no phase was
// named after it and nothing made its absence visible. A reference graph makes
// that kind of hole a query rather than a hope.
//
// # What this does and does not claim
//
// It reports **what a function touches**: the globals it names, the functions it
// calls, the functions that call it. All of that is mechanical - it is grep with
// structure, and it cannot be wrong about anything except by parsing badly.
//
// It does **not** say what a function is for. Grouping by shared globals suggests
// where to look; it is a hypothesis generator, and correction C3 is what happens
// when one of those is mistaken for a finding (three blitters were matched to
// three storage modes and the story assembled itself). Every actual naming still
// needs its own check that could have failed - see docs/method.md.
//
// So: read the clusters as "these forty functions all touch the county array,
// and none of them is named" - a place to look. Never as "these are the county
// functions".

const fs = require('fs');
const path = require('path');

const DECOMP = path.join(__dirname, 'decomp');
const OUT_DIR = path.join(__dirname, 'out');
const INDEX = path.join(OUT_DIR, 'xref.json');
const SYMBOLS = path.join(__dirname, '..', '..', 'docs', 'symbols.json');

// A function header emitted by DecompileAll.java:
//   // ==== 0040526e  Map_RenderIso  params=0  bytes=537
const HEADER = /^\/\/ ==== ([0-9a-f]{8})\s+(\S+)\s+params=(-?\d+)\s+bytes=(\d+)/;

// Anything that looks like a global or a function reference. Ghidra spells
// unknown data DAT_xxxxxxxx / _DAT_xxxxxxxx and unknown code FUN_xxxxxxxx;
// applied symbols keep their own names, which is why naming one global makes
// dozens of functions legible at once.
const TOKEN = /\b(_?DAT_[0-9a-f]{8}|FUN_[0-9a-f]{8}|[A-Za-z_][A-Za-z0-9_]{2,})\b/g;

// Tokens that are C, not the program.
const NOISE = new Set(`
if else for while do switch case break continue return goto sizeof typedef
void char short int long float double signed unsigned const static extern
struct union enum register volatile inline restrict
undefined undefined1 undefined2 undefined3 undefined4 undefined5 undefined6
undefined7 undefined8 code byte uint ushort ulong bool true false NULL
uint3 uint5 uint6 uint7 int3 int5 int6 int7 char2 unkbyte9 unkuint9
param_1 param_2 param_3 param_4 param_5 param_6 param_7 param_8 param_9
extraout_ECX extraout_EDX extraout_EAX extraout_var in_EAX in_ECX in_EDX
unaff_EBX unaff_ESI unaff_EDI unaff_EBP CONCAT31 CONCAT44 CONCAT22 CONCAT13
SUB84 SUB41 SUB42 SUB21 ZEXT14 ZEXT12 ZEXT24 ZEXT48 ZEXT816 SEXT14 SEXT24
halt_baddata func_0x
`.trim().split(/\s+/));

function loadSymbolNames() {
  if (!fs.existsSync(SYMBOLS)) return new Set();
  const raw = JSON.parse(fs.readFileSync(SYMBOLS, 'utf8'));
  const names = new Set();
  const walk = (v) => {
    if (Array.isArray(v)) return v.forEach(walk);
    if (v && typeof v === 'object') {
      if (typeof v.name === 'string') names.add(v.name);
      Object.values(v).forEach(walk);
    }
  };
  walk(raw);
  return names;
}

function build() {
  if (!fs.existsSync(DECOMP)) {
    console.error(`no decompilation at ${DECOMP}`);
    console.error('run tools/oracle/decompile-all.ps1 first (about 22 seconds)');
    process.exit(1);
  }
  const known = loadSymbolNames();
  const fns = new Map();          // name -> record
  const byAddr = new Map();

  for (const file of fs.readdirSync(DECOMP).filter(f => f.endsWith('.c'))) {
    const lines = fs.readFileSync(path.join(DECOMP, file), 'utf8').split('\n');
    let cur = null;
    for (const line of lines) {
      const h = HEADER.exec(line);
      if (h) {
        cur = {
          addr: h[1], name: h[2], params: +h[3], bytes: +h[4], file,
          globals: new Set(), calls: new Set(), lines: 0,
        };
        fns.set(cur.name, cur);
        byAddr.set(cur.addr, cur);
        continue;
      }
      if (!cur) continue;
      cur.lines++;
      // Comments carry our own prose, including other symbols' names, and
      // would otherwise manufacture references that the code does not make.
      const code = line.replace(/\/\*[\s\S]*?\*\//g, ' ').replace(/\/\/.*$/, ' ');
      let m;
      TOKEN.lastIndex = 0;
      while ((m = TOKEN.exec(code)) !== null) {
        const t = m[1];
        if (t === cur.name || NOISE.has(t)) continue;
        if (/^_?DAT_[0-9a-f]{8}$/.test(t)) cur.globals.add(t);
        else if (/^FUN_[0-9a-f]{8}$/.test(t)) cur.calls.add(t);
        else if (known.has(t)) {
          // A named symbol: decide by whether it is a function we indexed.
          (fns.has(t) ? cur.calls : cur.globals).add(t);
        }
      }
    }
  }

  // Second pass: a named symbol may have been seen before its own definition.
  for (const f of fns.values()) {
    for (const g of [...f.globals]) {
      if (fns.has(g)) { f.globals.delete(g); f.calls.add(g); }
    }
  }

  const callers = new Map();
  for (const f of fns.values()) {
    for (const c of f.calls) {
      if (!callers.has(c)) callers.set(c, new Set());
      callers.get(c).add(f.name);
    }
  }

  const out = { built: new Date().toISOString(), functions: {} };
  for (const f of fns.values()) {
    out.functions[f.name] = {
      addr: f.addr, params: f.params, bytes: f.bytes, file: f.file, lines: f.lines,
      named: !f.name.startsWith('FUN_'),
      globals: [...f.globals].sort(),
      calls: [...f.calls].sort(),
      callers: [...(callers.get(f.name) || [])].sort(),
    };
  }
  fs.mkdirSync(OUT_DIR, { recursive: true });
  fs.writeFileSync(INDEX, JSON.stringify(out));

  const all = Object.values(out.functions);
  const named = all.filter(f => f.named).length;
  const globals = new Set(all.flatMap(f => f.globals));
  const namedGlobals = [...globals].filter(g => !/^_?DAT_/.test(g)).length;
  console.log(`${all.length} functions, ${named} named (${(100 * named / all.length).toFixed(1)}%)`);
  console.log(`${globals.size} distinct globals, ${namedGlobals} named`);
  console.log(`index -> ${INDEX}`);
}

function load() {
  if (!fs.existsSync(INDEX)) { console.error('run: node tools/oracle/xref.js build'); process.exit(1); }
  return JSON.parse(fs.readFileSync(INDEX, 'utf8')).functions;
}

const fmt = (n, f) => `${n}${f.named ? '' : ` @${f.addr}`}  ${f.bytes}b`;

function touches(fns, needle) {
  const hits = Object.entries(fns)
    .filter(([, f]) => f.globals.some(g => g.includes(needle)))
    .sort((a, b) => (a[1].named === b[1].named ? b[1].bytes - a[1].bytes : a[1].named ? 1 : -1));
  console.log(`${hits.length} functions touch a global matching "${needle}"`);
  console.log('(unnamed first, largest first - the ones worth reading)\n');
  for (const [n, f] of hits.slice(0, 40)) console.log('  ' + fmt(n, f));
}

function callers(fns, name) {
  const f = fns[name];
  if (!f) return console.error(`no such function: ${name}`);
  console.log(`${name} is called by ${f.callers.length}:`);
  for (const c of f.callers) console.log('  ' + fmt(c, fns[c] || { named: true, bytes: 0 }));
}

function calls(fns, name) {
  const f = fns[name];
  if (!f) return console.error(`no such function: ${name}`);
  const list = f.calls.filter(c => fns[c]).sort((a, b) => (fns[a].named === fns[b].named ? 0 : fns[a].named ? 1 : -1));
  console.log(`${name} calls ${list.length}:`);
  for (const c of list) console.log('  ' + fmt(c, fns[c]));
}

function reach(fns, name, maxDepth = 3) {
  const seen = new Map([[name, 0]]);
  let frontier = [name];
  for (let d = 1; d <= maxDepth && frontier.length; d++) {
    const next = [];
    for (const n of frontier) {
      for (const c of (fns[n]?.calls || [])) {
        if (fns[c] && !seen.has(c)) { seen.set(c, d); next.push(c); }
      }
    }
    frontier = next;
  }
  const un = [...seen].filter(([n]) => fns[n] && !fns[n].named);
  console.log(`${seen.size} functions reachable from ${name} within ${maxDepth} calls`);
  console.log(`${un.length} of them are unnamed:\n`);
  for (const [n, d] of un.sort((a, b) => a[1] - b[1] || fns[b[0]].bytes - fns[a[0]].bytes).slice(0, 40)) {
    console.log(`  depth ${d}  ${fmt(n, fns[n])}`);
  }
}

function clusters(fns) {
  // Group unnamed functions by which *named* globals they touch. A named global
  // is one somebody has already established, so this says "these unnamed
  // functions work on something we understand" - which is where reading pays.
  const groups = new Map();
  for (const [n, f] of Object.entries(fns)) {
    if (f.named) continue;
    const anchors = f.globals.filter(g => !/^_?DAT_/.test(g)).sort();
    if (!anchors.length) continue;
    const key = anchors.slice(0, 3).join(' + ');
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push([n, f]);
  }
  const ranked = [...groups].sort((a, b) => b[1].length - a[1].length);
  console.log(`${ranked.length} clusters of unnamed functions, by the named globals they touch\n`);
  for (const [key, list] of ranked.slice(0, 25)) {
    console.log(`  ${String(list.length).padStart(3)}  ${key}`);
  }
  const orphans = Object.values(fns).filter(f => !f.named && !f.globals.some(g => !/^_?DAT_/.test(g)));
  console.log(`\n${orphans.length} unnamed functions touch no named global at all.`);
  console.log('Those are the dark part of the binary - nothing anchors them yet.');
}

function unnamed(fns, near) {
  const hits = Object.entries(fns)
    .filter(([, f]) => !f.named && (!near || f.globals.some(g => g.includes(near)) || f.calls.some(c => c.includes(near))))
    .sort((a, b) => b[1].bytes - a[1].bytes);
  console.log(`${hits.length} unnamed functions${near ? ` near "${near}"` : ''}, largest first\n`);
  for (const [n, f] of hits.slice(0, 40)) {
    const anchors = f.globals.filter(g => !/^_?DAT_/.test(g)).slice(0, 4);
    console.log(`  ${fmt(n, f)}  ${anchors.join(' ') || '(no named global)'}`);
  }
}

function leverage(fns) {
  // Which *unnamed* global, if someone worked out what it is, would light up the
  // most currently-dark functions?
  //
  // "Dark" means the function touches no named global at all, so nothing anchors
  // it and no cluster lists it. Naming a global those functions share turns all
  // of them from unreachable into a group with a subject - which is why naming
  // compounds rather than adding up. This ranks that leverage.
  const dark = Object.entries(fns).filter(([, f]) => !f.named && !f.globals.some(g => !/^_?DAT_/.test(g)));
  const score = new Map();
  for (const [, f] of dark) {
    for (const g of f.globals) {
      if (!/^_?DAT_/.test(g)) continue;
      if (!score.has(g)) score.set(g, { dark: 0, total: 0, bytes: 0 });
      const s = score.get(g);
      s.dark++;
      s.bytes += f.bytes;
    }
  }
  for (const [, f] of Object.entries(fns)) {
    for (const g of f.globals) if (score.has(g)) score.get(g).total++;
  }
  const ranked = [...score].sort((a, b) => b[1].dark - a[1].dark);
  console.log(`${dark.length} functions are dark: they touch no named global, so nothing anchors them.`);
  console.log('Naming one of these globals would give each of them a subject:\n');
  console.log('  dark  total  global            avg size');
  for (const [g, s] of ranked.slice(0, 30)) {
    console.log(`  ${String(s.dark).padStart(4)}  ${String(s.total).padStart(5)}  ${g.padEnd(18)} ${Math.round(s.bytes / s.dark)}b`);
  }
  const top20 = ranked.slice(0, 20);
  const lit = new Set();
  for (const [name, f] of dark) {
    if (f.globals.some(g => top20.some(([t]) => t === g))) lit.add(name);
  }
  console.log(`\nNaming just the top 20 would anchor ${lit.count || lit.size} of the ${dark.length} dark functions.`);
}

const [cmd, arg, flag, flagVal] = process.argv.slice(2);
switch (cmd) {
  case 'build': build(); break;
  case 'touches': touches(load(), arg); break;
  case 'callers': callers(load(), arg); break;
  case 'calls': calls(load(), arg); break;
  case 'reach': reach(load(), arg, +(flagVal || 3)); break;
  case 'clusters': clusters(load()); break;
  case 'leverage': leverage(load()); break;
  case 'unnamed': unnamed(load(), flag === '--near' ? flagVal : arg); break;
  default:
    console.log(fs.readFileSync(__filename, 'utf8').split('\n').slice(2, 26).join('\n').replace(/^\/\/ ?/gm, ''));
}
