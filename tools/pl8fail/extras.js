'use strict';
// Render the trailing block of every shape-0 frame that has one, as rows RLE rows.
const { parse, spans, loadAll } = require('./pl8lib');
const DIR = 'F:/games/Lords of the Realm II';
const want = process.argv[2];
function decodeRows(buf, p, w, n) {
  const out = [];
  for (let r = 0; r < n; r++) {
    const row = new Array(w).fill(0);
    let x = 0;
    while (x < w) {
      const c = buf[p++];
      if (c === 0) { const m = buf[p++]; x += m; }
      else { for (let k = 0; k < c; k++) row[x + k] = buf[p + k]; p += c; x += c; }
    }
    out.push(row);
  }
  return { rows: out, end: p };
}
for (const { name, buf } of loadAll(DIR)) {
  const base = name.replace(/\.pl8$/i, '');
  if (want && base.toLowerCase() !== want.toLowerCase()) continue;
  const p = parse(buf); const sp = spans(p);
  let blank = 0, painted = 0, shown = 0;
  for (let i = 0; i < p.count; i++) {
    const f = p.frames[i];
    if (f.shape !== 0 || f.rows === 0) continue;
    const raw = f.w * f.h;
    if (sp[i] === raw) continue;
    const d = decodeRows(buf, f.off + raw, f.w, f.rows);
    if (d.end - f.off !== sp[i]) { console.log(`MISFIT ${name} f${i}`); continue; }
    const any = d.rows.some(r => r.some(v => v));
    if (any) painted++; else blank++;
    if (any && shown < 12 && want) {
      shown++;
      console.log(`${name} f${i} ${f.w}x${f.h} rows=${f.rows} x=${f.x} y=${f.y}`);
      for (const r of d.rows) console.log('    |' + r.map(v => v ? '#' : '.').join('') + '|');
      // and the raw part
      for (let y = 0; y < f.h; y++) {
        const row = [...buf.subarray(f.off + y * f.w, f.off + (y + 1) * f.w)];
        console.log('    :' + row.map(v => v ? '#' : '.').join('') + ':');
      }
    }
  }
  if (blank + painted) console.log(`${name}: frames with trailing block: ${blank + painted} (all-blank ${blank}, painted ${painted})`);
}
