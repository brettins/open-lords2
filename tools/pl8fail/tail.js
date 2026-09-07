'use strict';
// Dump the head and tail of the span of specified frames.
const { parse, spans, loadAll } = require('./pl8lib');
const DIR = 'F:/games/Lords of the Realm II';
const file = process.argv[2];
const idxs = process.argv.slice(3).map(Number);
const mode = process.env.MODE || 'tail';

const e = loadAll(DIR).find(x => x.name.toLowerCase() === (file + '.pl8').toLowerCase());
const buf = e.buf, p = parse(buf), sp = spans(p);
function hex(b) { return [...b].map(v => v.toString(16).padStart(2, '0')).join(' '); }
function dec(b) { return [...b].map(v => String(v).padStart(3)).join(' '); }
for (const i of idxs) {
  const f = p.frames[i];
  const raw = f.w * f.h;
  console.log(`f${i} ${f.w}x${f.h} off=${f.off} span=${sp[i]} raw=${raw} extra=${sp[i]-raw} shape=${f.shape} rows=${f.rows} x=${f.x} y=${f.y}`);
  const s = buf.subarray(f.off, f.off + sp[i]);
  if (mode === 'all') {
    for (let r = 0; r < s.length; r += 16) console.log('   +' + String(r).padStart(4) + ' ' + hex(s.subarray(r, r + 16)));
  } else {
    console.log('   head: ' + hex(s.subarray(0, 32)));
    console.log('   tail: ' + hex(s.subarray(Math.max(0, s.length - 40))));
    console.log('   tailD:' + dec(s.subarray(Math.max(0, s.length - 40))));
  }
}
