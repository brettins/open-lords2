// Diffs two FORMAT.md traces: node trace-diff.js <a.orig.jsonl> <b.ours.jsonl> [--md] [--context N]
// Steps are the spans between anchors (screen changes when neither side has one); within a step,
// each category's ordered subsequence is compared and its first divergence printed. C246.
// Both files stream: a 1.7 GB field-battle-manual.ours.jsonl overflows a string.
'use strict';
const fs = require('fs');
const path = require('path');
const { StringDecoder } = require('string_decoder');
const pixels = require('./pixel-diff');

const args = process.argv.slice(2);
const flag = (n) => args.includes(n);
const opt = (n, d) => { const i = args.indexOf(n); return i < 0 ? d : args[i + 1]; };
const [origFile, oursFile] = args.filter((a, i) => !a.startsWith('--') && !(i > 0 && ['--context', '--png-dir'].includes(args[i - 1])));
// Side-by-side PNGs of differing shots (pixel-diff.js); default <trace dir>/pixels.
const PNG_DIR = opt('--png-dir', null);
const CONTEXT = Number(opt('--context', 3));

function* lines(file) {
  const fd = fs.openSync(file, 'r'), buf = Buffer.alloc(1 << 22), dec = new StringDecoder('utf8');
  let rest = '';
  try {
    for (let n; (n = fs.readSync(fd, buf, 0, buf.length, null)) > 0;) {
      const parts = (rest + dec.write(buf.subarray(0, n))).split('\n');
      rest = parts.pop();
      for (const l of parts) if (l) yield l;
    }
    rest += dec.end();
    if (rest) yield rest;
  } finally { fs.closeSync(fd); }
}

function hasAnchor(file) {
  const fd = fs.openSync(file, 'r'), buf = Buffer.alloc(1 << 22), pat = Buffer.from('"kind":"anchor"');
  try {
    for (let n, keep = 0; (n = fs.readSync(fd, buf, keep, buf.length - keep, null)) > 0;) {
      if (buf.subarray(0, keep + n).indexOf(pat) >= 0) return true;
      const end = keep + n;
      keep = Math.min(pat.length - 1, end);
      buf.copy(buf, 0, end - keep, end);
    }
    return false;
  } finally { fs.closeSync(fd); }
}
const low = (s) => (s == null ? '?' : String(s).toLowerCase());

// Harness artifacts of the original side, not the game: message 298 "Non 256 colour display",
// posted from 0x004B9A23 because native/l2-trace's fake DirectDraw reports a non-8-bit mode.
const harness = (r) => (r.kind === 'message' || r.name === 'Msg_PlayVoice') && r.args.group === 298;

// Category and comparison key of one record; null drops it from the ordered comparison.
function keyOf(r) {
  const a = r.args || {};
  switch (r.kind) {
    case 'screen': return ['screen', String(a.id)];
    case 'phase': return ['phase', String(a.phase)];
    case 'film': return ['film', low(a.file)];
    case 'message': return ['message', `${a.group}/c${a.category}/to${a.to}`];
    case 'ai': return a.realm == null ? null : ['ai', `realm${a.realm}`];
    case 'order': return ['order', 'order'];
    // Army_*, Battle_*, UnitPanel_Draw: compared by name, only where ours has the emit site.
    case 'call': return BUILT().has(String(r.addr).toUpperCase()) ? ['call', r.name] : null;
    case 'sound':
      if (r.name === 'Music_Play') return ['music', low(a.file)];
      if (r.name === 'Msg_PlayVoice') return ['voice', String(a.group)];
      if (r.name === 'Sound_PlayFile') return ['sound', low(a.file)];
      // Sound_PlaySlot/RestartSlot: the original records the slot, not the file (trace-shape.js).
      return ['slot', r.name];
    default: return null;
  }
}
const COLLAPSE = new Set(['screen', 'phase']);
// The original steps one realm a call, round-robin (0x0049A581), many calls a frame; ours every
// realm a frame: only which realms step, in first-step order, compares.
const DISTINCT = new Set(['ai']);
let built = null;
const BUILT = () => (built = built || builtSites());
const RANK = ['pixels', 'screen', 'sound', 'music', 'voice', 'film', 'message', 'call', 'phase', 'ai', 'order', 'slot', 'text', 'draw', 'rand'];


// One pass over a trace: per step (label; a repeated label gets #n) its category sequences, its
// rand count and each snap's draw/text keys. A draw or text line stays raw, and only in the run of
// one frame's draw/text/rand lines before the next other record; a snap takes that run when its
// frame is the one before it on the original side (a record carries the frame count before the
// present), its own frame on ours (the draw precedes the hash).
const LEAF = /^\{"frame":(-?\d+),"kind":"(draw|text|rand)"/;
function scan(file, byAnchor, lag, orig) {
  const out = [], seen = {}, notBuilt = {};
  const sites = orig ? BUILT() : null;
  const step = (label) => ({ label, seq: {}, rand: 0, randAt: null, snaps: [], shots: [] });
  let cur = step('(start)'), n = 0, i = 0, run = { frame: null, lines: [] };
  const miss = (r) => { if (sites && r.addr && !sites.has(r.addr.toUpperCase())) notBuilt[`${r.addr} ${r.name}`] = (notBuilt[`${r.addr} ${r.name}`] || 0) + 1; };
  for (const l of lines(file)) {
    const at = i++, m = LEAF.exec(l);
    if (m) {
      const frame = Number(m[1]);
      if (run.frame !== frame) run = { frame, lines: [] };
      if (m[2] !== 'rand') { run.lines.push(l); if (orig) miss(JSON.parse(l)); continue; }
      const r = JSON.parse(l);
      miss(r);
      cur.rand += r.args.count;
      if (cur.randAt == null) cur.randAt = at;
      continue;
    }
    const r = JSON.parse(l);
    miss(r);
    if (byAnchor ? r.kind === 'anchor' : r.kind === 'screen') {
      out.push(cur);
      let label = byAnchor ? r.args.label : `screen-change-${++n}`;
      seen[label] = (seen[label] || 0) + 1;
      if (seen[label] > 1) label += `#${seen[label]}`;
      cur = step(label);
    }
    if ((r.kind === 'snap' || r.kind === 'shot') && r.args.shot) cur.shots.push(path.join(path.dirname(file), r.args.shot));
    if (r.kind === 'snap') {
      const got = { draw: [], text: [] };
      if (run.frame === r.frame - lag) {
        for (const q of run.lines.map((x) => JSON.parse(x))) got[q.kind].push(q.kind === 'draw' ? drawKey(q) : textKey(q));
      }
      cur.snaps.push(got);
    }
    run = { frame: null, lines: [] };
    if (harness(r)) continue;
    const k = keyOf(r);
    if (!k) continue;
    const [cat, key] = k;
    const s = (cur.seq[cat] = cur.seq[cat] || []);
    if (COLLAPSE.has(cat) && s.length && s[s.length - 1].key === key) continue;
    if (DISTINCT.has(cat) && s.some((x) => x.key === key)) continue;
    // Soft: one of our Pcg32 draws precedes the record within its step.
    s.push({ key, r, tainted: cur.randAt != null && cur.randAt < at });
  }
  out.push(cur);
  return { steps: out, notBuilt };
}

// A sprite by sheet, frame and place; a Ui_* leaf (no sheet) by name and arguments.
function drawKey(q) {
  const a = q.args;
  if (a.sheet != null) return `${low(a.sheet)}:${a.frame}@${a.x},${a.y}`;
  return `${q.name}(${Object.entries(a).map(([k, v]) => `${k}=${v}`).join(',')})`;
}
// Text by L2.eng group/index, else a number's value, else the literal (ours only: no group).
function textKey(q) {
  const a = q.args;
  if (a.group != null) return `${a.group}/${a.index}@${a.x},${a.y}`;
  if (a.value != null) return `#${a.value}@${a.x},${a.y}`;
  return `${JSON.stringify(a.s)}@${a.x},${a.y}`;
}


const show = (e, taint) => {
  if (!e) return '(end)';
  const a = { ...e.r.args };
  const where = e.r.caller ? ` from ${e.r.caller}` : '';
  const t = taint && e.tainted ? ' [rng-tainted]' : '';
  return `f${e.r.frame} ${e.r.name || e.r.kind} ${JSON.stringify(a)}${where}${t}`;
};

function firstDiff(a, b, keyed = (x) => x.key) {
  const n = Math.max(a.length, b.length);
  for (let i = 0; i < n; i++) if (!a[i] || !b[i] || keyed(a[i]) !== keyed(b[i])) return i;
  return -1;
}

function multisetDiff(a, b) {
  const count = (xs) => xs.reduce((m, x) => m.set(x, (m.get(x) || 0) + 1), new Map());
  const ca = count(a), cb = count(b);
  const only = (x, y) => [...x].flatMap(([k, v]) => (v > (y.get(k) || 0) ? [`${k} x${v - (y.get(k) || 0)}`] : []));
  return { origOnly: only(ca, cb), oursOnly: only(cb, ca) };
}

// Addresses some emit site in our crates cites: an original address none cites is "not built".
function builtSites() {
  const root = path.join(__dirname, '..', '..', 'crates');
  const addrs = new Set();
  const walk = (d) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.endsWith('.rs')) {
        const re = /trace(?:_changed)?!\([\s\S]*?"\w+",\s*(0x[0-9A-Fa-f]{8})/g;
        for (const m of fs.readFileSync(p, 'utf8').matchAll(re)) addrs.add(m[1].toUpperCase());
      }
    }
  };
  walk(root);
  return addrs;
}


function diff(origPath, oursPath, oursLag = 0) {
  const byAnchor = hasAnchor(origPath) || hasAnchor(oursPath);
  const O = scan(origPath, byAnchor, 1, true), U = scan(oursPath, byAnchor, oursLag, false);
  const so = O.steps, su = U.steps;
  const findings = [];
  const totals = {};
  const labels = [...new Set([...so.map((s) => s.label), ...su.map((s) => s.label)])];
  for (const label of labels) {
    const o = so.find((s) => s.label === label), u = su.find((s) => s.label === label);
    if (!o || !u) { findings.push({ label, cat: 'step', note: `step only in ${o ? 'original' : 'ours'}` }); continue; }
    const qo = o.seq, qu = u.seq;
    for (const cat of new Set([...Object.keys(qo), ...Object.keys(qu)])) {
      const a = qo[cat] || [], b = qu[cat] || [];
      totals[cat] = totals[cat] || [0, 0];
      totals[cat][0] += a.length; totals[cat][1] += b.length;
      const i = cat === 'slot' || cat === 'order' ? (a.length === b.length ? -1 : 0) : firstDiff(a, b);
      if (i < 0) continue;
      const lo = Math.max(0, i - CONTEXT);
      findings.push({
        label, cat, index: i, counts: [a.length, b.length],
        tainted: b[i] ? b[i].tainted : false,
        orig: a.slice(lo, i + CONTEXT + 1).map((e, k) => (lo + k === i ? '> ' : '  ') + show(e)),
        ours: b.slice(lo, i + CONTEXT + 1).map((e, k) => (lo + k === i ? '> ' : '  ') + show(e, true)),
      });
    }
    const ro = o.rand, ru = u.rand;
    totals.rand = totals.rand || [0, 0]; totals.rand[0] += ro; totals.rand[1] += ru;
    if (ro !== ru) findings.push({ label, cat: 'rand', counts: [ro, ru], note: 'draw counts (Rand_Advance+Rng_LfsrStep vs Pcg32)' });
    const po = o.snaps, pu = u.snaps;
    po.forEach((p, k) => {
      const q = pu[k];
      if (!q) { findings.push({ label, cat: 'draw', note: `snap ${k} only in original` }); return; }
      for (const cat of ['draw', 'text']) {
        // Distinct: the original repaints some leaves several times a frame, which no pixel shows.
        const m = multisetDiff([...new Set(p[cat])], [...new Set(q[cat])]);
        totals[cat] = totals[cat] || [0, 0]; totals[cat][0] += p[cat].length; totals[cat][1] += q[cat].length;
        if (!p[cat].length && q[cat].length) { findings.push({ label, cat, counts: [0, q[cat].length], note: `snap ${k}: the original side recorded no ${cat} records` }); continue; }
        if (m.origOnly.length || m.oursOnly.length) findings.push({ label, cat, counts: [p[cat].length, q[cat].length], note: `snap ${k} multiset`, orig: m.origOnly.slice(0, 8), ours: m.oursOnly.slice(0, 8) });
        else if (p[cat].join() !== q[cat].join()) findings.push({ label, cat, note: `snap ${k} same set, paint order differs at ${firstDiff(p[cat], q[cat], (x) => x)}` });
      }
    });
    // Totals: shots compared / shots that differ.
    const name = path.basename(origPath).replace(/\..*$/, '');
    o.shots.forEach((a, k) => {
      const b = u.shots[k];
      if (!b || !fs.existsSync(a) || !fs.existsSync(b)) { findings.push({ label, cat: 'pixels', note: `shot ${k}: ${b ? 'a file is missing' : 'only in original'}` }); return; }
      const png = path.join(PNG_DIR || path.join(path.dirname(origPath), 'pixels'), `${name}-${label.replace(/[^\w#-]+/g, '_')}-shot${k}.png`);
      const r = pixels.diffShots(a, b, { scenario: name, png });
      totals.pixels = totals.pixels || [0, 0]; totals.pixels[0]++; if (r.full.pixels) totals.pixels[1]++;
      if (r.full.pixels) findings.push({ label, cat: 'pixels', note: `shot ${k} (${path.basename(a)}): ${pixels.fmt(r)} -> ${png}` });
    });
  }
  const notBuilt = O.notBuilt;
  findings.sort((x, y) => RANK.indexOf(x.cat) - RANK.indexOf(y.cat));
  return { byAnchor, findings, totals, notBuilt };
}

function render(name, d) {
  const out = [`### ${name}`, '', `Steps by ${d.byAnchor ? 'anchor' : 'screen change'}. Totals (original / ours): ` +
    Object.entries(d.totals).map(([c, [a, b]]) => `${c} ${a}/${b}`).join(', ') + '.'];
  if (Object.keys(d.notBuilt).length) out.push('', 'Not built (original addresses no emit site of ours cites): ' + Object.entries(d.notBuilt).map(([k, v]) => `${k} x${v}`).join(', '));
  if (!d.findings.length) out.push('', 'No divergence.');
  for (const f of d.findings) {
    out.push('', `- **${f.cat}** in step \`${f.label}\`${f.index != null ? ` at #${f.index}` : ''}` +
      (f.counts ? ` (original ${f.counts[0]}, ours ${f.counts[1]})` : '') + (f.tainted ? ' [rng-tainted]' : '') + (f.note ? `: ${f.note}` : ''));
    if (f.orig) out.push('  ```', '  original:', ...f.orig.map((l) => '    ' + l), '  ours:', ...(f.ours || []).map((l) => '    ' + l), '  ```');
  }
  return out.join('\n');
}

// --dir D --a orig --b ours s1 s2 ...: diffs D/<s>.<a>.jsonl against D/<s>.<b>.jsonl for each s.

// --dir D --a orig --b ours s1 s2 ...: diffs D/<s>.<a>.jsonl against D/<s>.<b>.jsonl for each s.
if (require.main === module && flag('--dir')) {
  const dir = opt('--dir'), a = opt('--a', 'orig'), b = opt('--b', 'ours');
  const names = args.filter((x, i) => !x.startsWith('--') && !['--dir', '--a', '--b', '--context', '--png-dir'].includes(args[i - 1]));
  let any = 0;
  for (const s of names) {
    const d = diff(path.join(dir, `${s}.${a}.jsonl`), path.join(dir, `${s}.${b}.jsonl`), b.includes('ours') ? 0 : 1);
    any += d.findings.length;
    console.log(render(s, d) + '\n');
  }
  process.exit(any ? 1 : 0);
} else if (require.main === module) {
  if (!origFile || !oursFile) { console.error('usage: node trace-diff.js <orig.jsonl> <ours.jsonl> [--context N]'); process.exit(2); }
  const d = diff(origFile, oursFile, oursFile.includes('.ours.') ? 0 : 1);
  console.log(render(path.basename(origFile).replace(/\..*$/, ''), d));
  process.exit(d.findings.length ? 1 : 0);
}
module.exports = { diff, render, scan, lines };
