'use strict';
// (family, zoom) -> which per-frame size model fits shape-0 frames.
const { parse, spans, rleConsume, loadAll } = require('./pl8lib');
const DIR = 'F:/games/Lords of the Realm II';
const m = new Map();
for (const { name, buf } of loadAll(DIR)) {
  const p = parse(buf);
  const sp = spans(p);
  let rle = 0, raw = 0, grid = 0, rawplus = 0, other = 0, iso = 0;
  for (let i = 0; i < p.count; i++) {
    const f = p.frames[i];
    if (f.shape !== 0) { iso++; continue; }
    const r = rleConsume(buf, f);
    const g = (f.w >> 3) * (f.h >> 3);
    if (f.w * f.h === sp[i]) raw++;
    else if (!r.err && r.used === sp[i]) rle++;
    else if (g === sp[i]) grid++;
    else if (sp[i] > f.w * f.h) rawplus++;
    else other++;
  }
  const k = `fam=${p.family} zoom=${p.zoom}`;
  const e = m.get(k) || { files: 0, names: [], rle: 0, raw: 0, grid: 0, rawplus: 0, other: 0, iso: 0 };
  e.files++; e.names.push(name.replace('.pl8', ''));
  e.rle += rle; e.raw += raw; e.grid += grid; e.rawplus += rawplus; e.other += other; e.iso += iso;
  m.set(k, e);
}
for (const [k, e] of [...m].sort()) {
  console.log(`${k.padEnd(16)} files=${String(e.files).padStart(3)}  shape0: rle=${e.rle} raw=${e.raw} raw+extra=${e.rawplus} grid=${e.grid} other=${e.other}   shape1-4=${e.iso}`);
  if (e.files <= 12) console.log('     ' + e.names.join(' '));
}
