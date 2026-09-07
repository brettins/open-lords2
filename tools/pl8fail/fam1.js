'use strict';
// Every family-1 file: header bytes, shape distribution, and whether each frame
// fits RLE, raw, or both.
const { parse, spans, rleConsume, loadAll } = require('./pl8lib');
const DIR = 'F:/games/Lords of the Realm II';
for (const { name, buf } of loadAll(DIR)) {
  const p = parse(buf);
  if (p.family !== (process.env.FAM ? +process.env.FAM : 1)) continue;
  const sp = spans(p);
  const shapes = {};
  let rleOnly = 0, rawOnly = 0, both = 0, neither = 0;
  for (let i = 0; i < p.count; i++) {
    const f = p.frames[i];
    shapes[f.shape] = (shapes[f.shape] || 0) + 1;
    const r = rleConsume(buf, f);
    const isRle = !r.err && r.used === sp[i];
    const isRaw = f.w * f.h === sp[i];
    if (isRle && isRaw) both++; else if (isRle) rleOnly++; else if (isRaw) rawOnly++; else neither++;
  }
  console.log(`${name.padEnd(15)} z=${p.zoom} n=${String(p.count).padStart(4)} h4=${String(p.h4).padStart(3)} h6=${p.h6} h7=${String(p.h7).padStart(2)} shapes=${JSON.stringify(shapes).padEnd(14)} rle=${rleOnly} raw=${rawOnly} both=${both} neither=${neither}`);
}
