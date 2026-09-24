// Diffs two FORMAT.md traces: node trace-diff.js <a.orig.jsonl> <b.ours.jsonl> [--md] [--context N]
// Steps are the spans between anchors (screen changes when neither side has one); within a step,
// each category's ordered subsequence is compared and its first divergence printed. C246.
'use strict';
const fs = require('fs');
const path = require('path');

const args = process.argv.slice(2);
const flag = (n) => args.includes(n);
const opt = (n, d) => { const i = args.indexOf(n); return i < 0 ? d : args[i + 1]; };
const [origFile, oursFile] = args.filter((a, i) => !a.startsWith('--') && !(i > 0 && args[i - 1] === '--context'));
const CONTEXT = Number(opt('--context', 3));

const load = (f) => fs.readFileSync(f, 'utf8').split('\n').filter(Boolean).map((l, i) => ({ ...JSON.parse(l), i }));
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
const RANK = ['screen', 'sound', 'music', 'voice', 'film', 'message', 'call', 'phase', 'ai', 'order', 'slot', 'text', 'draw', 'rand'];

// Steps: [label, records]; a repeated label gets #n.
function steps(recs, byAnchor) {
  const out = [];
  const seen = {};
  let cur = { label: '(start)', recs: [] };
  let n = 0;
  for (const r of recs) {
    const cut = byAnchor ? r.kind === 'anchor' : r.kind === 'screen';
    if (cut) {
      out.push(cur);
      let label = byAnchor ? r.args.label : `screen-change-${++n}`;
      seen[label] = (seen[label] || 0) + 1;
      if (seen[label] > 1) label += `#${seen[label]}`;
      cur = { label, recs: [] };
    }
    cur.recs.push(r);
  }
  out.push(cur);
  return out;
}

// The draw/text records of each snap: the frame before it on the original side (a record
// carries the frame count before the present), its own frame on ours (the draw precedes the hash).
function snapDraws(recs, lag) {
  const out = [];
  recs.forEach((r, k) => {
    if (r.kind !== 'snap') return;
    const got = { draw: [], text: [], hash: r.args.hash };
    for (let j = k - 1; j >= 0; j--) {
      const q = recs[j];
      if (q.frame !== r.frame - lag || !['draw', 'text', 'rand'].includes(q.kind)) break;
      if (q.kind === 'draw') got.draw.unshift(drawKey(q));
      if (q.kind === 'text') got.text.unshift(textKey(q));
    }
    out.push(got);
  });
  return out;
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

function sequences(recs, side) {
  const seq = {};
  for (const r of recs) {
    if (harness(r)) continue;
    const k = keyOf(r);
    if (!k) continue;
    const [cat, key] = k;
    const s = (seq[cat] = seq[cat] || []);
    if (COLLAPSE.has(cat) && s.length && s[s.length - 1].key === key) continue;
    if (DISTINCT.has(cat) && s.some((x) => x.key === key)) continue;
    s.push({ key, r });
  }
  return seq;
}

const show = (e, taint) => {
  if (!e) return '(end)';
  const a = { ...e.r.args };
  const where = e.r.caller ? ` from ${e.r.caller}` : '';
  const t = taint && taint(e.r) ? ' [rng-tainted]' : '';
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

function diff(orig, ours, oursLag = 0) {
  const byAnchor = orig.some((r) => r.kind === 'anchor') || ours.some((r) => r.kind === 'anchor');
  const so = steps(orig, byAnchor), su = steps(ours, byAnchor);
  // Soft: one of our Pcg32 draws precedes the record within its step.
  const stepRand = new Map(su.map((s) => [s, s.recs.find((r) => r.kind === 'rand')]));
  const taint = (r) => [...stepRand].some(([s, q]) => q && q.i < r.i && s.recs.includes(r));
  const findings = [];
  const totals = {};
  const labels = [...new Set([...so.map((s) => s.label), ...su.map((s) => s.label)])];
  for (const label of labels) {
    const o = so.find((s) => s.label === label), u = su.find((s) => s.label === label);
    if (!o || !u) { findings.push({ label, cat: 'step', note: `step only in ${o ? 'original' : 'ours'}` }); continue; }
    const qo = sequences(o.recs, 'orig'), qu = sequences(u.recs, 'ours');
    for (const cat of new Set([...Object.keys(qo), ...Object.keys(qu)])) {
      const a = qo[cat] || [], b = qu[cat] || [];
      totals[cat] = totals[cat] || [0, 0];
      totals[cat][0] += a.length; totals[cat][1] += b.length;
      const i = cat === 'slot' || cat === 'order' ? (a.length === b.length ? -1 : 0) : firstDiff(a, b);
      if (i < 0) continue;
      const lo = Math.max(0, i - CONTEXT);
      findings.push({
        label, cat, index: i, counts: [a.length, b.length],
        tainted: b[i] ? taint(b[i].r) : false,
        orig: a.slice(lo, i + CONTEXT + 1).map((e, k) => (lo + k === i ? '> ' : '  ') + show(e)),
        ours: b.slice(lo, i + CONTEXT + 1).map((e, k) => (lo + k === i ? '> ' : '  ') + show(e, taint)),
      });
    }
    const ro = o.recs.filter((r) => r.kind === 'rand').reduce((s, r) => s + r.args.count, 0);
    const ru = u.recs.filter((r) => r.kind === 'rand').reduce((s, r) => s + r.args.count, 0);
    totals.rand = totals.rand || [0, 0]; totals.rand[0] += ro; totals.rand[1] += ru;
    if (ro !== ru) findings.push({ label, cat: 'rand', counts: [ro, ru], note: 'draw counts (Rand_Advance+Rng_LfsrStep vs Pcg32)' });
    const po = snapDraws(o.recs, 1), pu = snapDraws(u.recs, oursLag);
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
  }
  const sites = builtSites();
  const notBuilt = {};
  for (const r of orig) if (r.addr && !sites.has(r.addr.toUpperCase())) notBuilt[`${r.addr} ${r.name}`] = (notBuilt[`${r.addr} ${r.name}`] || 0) + 1;
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
if (require.main === module && flag('--dir')) {
  const dir = opt('--dir'), a = opt('--a', 'orig'), b = opt('--b', 'ours');
  const names = args.filter((x, i) => !x.startsWith('--') && !['--dir', '--a', '--b', '--context'].includes(args[i - 1]));
  let any = 0;
  for (const s of names) {
    const d = diff(load(path.join(dir, `${s}.${a}.jsonl`)), load(path.join(dir, `${s}.${b}.jsonl`)), b.includes("ours") ? 0 : 1);
    any += d.findings.length;
    console.log(render(s, d) + '\n');
  }
  process.exit(any ? 1 : 0);
} else if (require.main === module) {
  if (!origFile || !oursFile) { console.error('usage: node trace-diff.js <orig.jsonl> <ours.jsonl> [--context N]'); process.exit(2); }
  const d = diff(load(origFile), load(oursFile), oursFile.includes(".ours.") ? 0 : 1);
  console.log(render(path.basename(origFile).replace(/\..*$/, ''), d));
  process.exit(d.findings.length ? 1 : 0);
}
module.exports = { diff, render, load };
