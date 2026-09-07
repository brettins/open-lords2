'use strict';
// Compare the four 108-glyph font files frame-by-frame.
const { parse, spans, loadAll } = require('./pl8lib');
const DIR = 'F:/games/Lords of the Realm II';
const files = process.argv.slice(2);
const all = loadAll(DIR);
const ps = files.map(n => {
  const e = all.find(x => x.name.toLowerCase() === n.toLowerCase() + '.pl8');
  const p = parse(e.buf); p.name = n; p.buf = e.buf; p.sp = spans(p); return p;
});
const n = Math.min(...ps.map(p => p.count));
console.log(files.map(f => f.padEnd(26)).join(''));
console.log(ps.map(p => `fam=${p.family} z=${p.zoom} h4=${p.h4} h7=${p.h7}`.padEnd(26)).join(''));
let diff = 0;
for (let i = 0; i < n; i++) {
  const cells = ps.map(p => {
    const f = p.frames[i];
    return `${String(i).padStart(3)} ${f.w}x${f.h} r=${f.rows} sp=${p.sp[i]}`.padEnd(26);
  });
  const dims = ps.map(p => `${p.frames[i].w}x${p.frames[i].h}`);
  const rows = ps.map(p => p.frames[i].rows);
  const same = dims.every(d => d === dims[0]) && rows.every(r => r === rows[0]);
  if (!same) diff++;
  if (process.env.ALL || !same) console.log(cells.join('') + (same ? '' : '  <-- differs'));
}
console.log(`frames with differing dims/rows: ${diff}/${n}`);
