'use strict';
// Render shape-0 frames raw, side by side across files.
const { parse, spans, loadAll } = require('./pl8lib');
const DIR = process.argv[2] || 'F:/games/Lords of the Realm II';
const names = process.argv[3].split(',');
const idxs = process.argv.slice(4).map(Number);
const all = loadAll(DIR);
const ps = names.map(n => {
  const e = all.find(x => x.name.toLowerCase() === n.toLowerCase() + '.pl8');
  const p = parse(e.buf); p.buf = e.buf; p.name = n; return p;
});
for (const i of idxs) {
  const grids = ps.map(p => {
    const f = p.frames[i];
    const g = [];
    for (let y = 0; y < f.h; y++)
      g.push([...p.buf.subarray(f.off + y * f.w, f.off + (y + 1) * f.w)].map(v => v ? '#' : '.').join(''));
    return { g, label: `${p.name} f${i} ${f.w}x${f.h} r=${f.rows}` };
  });
  console.log(grids.map(x => x.label.padEnd(26)).join(''));
  const H = Math.max(...grids.map(x => x.g.length));
  for (let y = 0; y < H; y++)
    console.log(grids.map(x => ('|' + (x.g[y] || '') + '|').padEnd(26)).join(''));
  console.log('');
}
