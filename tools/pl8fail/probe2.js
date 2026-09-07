'use strict';
// Test hypothesis: the undershooting raw frames are actually RLE.
const { parse, spans, rleConsume, loadAll } = require('./pl8lib');
const DIR = process.argv[2] || 'F:/games/Lords of the Realm II';
const only = process.argv.slice(3);

for (const { name, buf } of loadAll(DIR)) {
  const base = name.replace(/\.pl8$/i, '');
  if (!only.includes(base)) continue;
  const p = parse(buf);
  const sp = spans(p);
  console.log(`\n=== ${name} fam=${p.family} zoom=${p.zoom} n=${p.count} h4=${p.h4} h6=${p.h6} h7=${p.h7}`);
  let rawOk = 0, rleOk = 0, neither = 0;
  const detail = [];
  for (let i = 0; i < p.frames.length; i++) {
    const f = p.frames[i];
    const raw = f.w * f.h;
    const r = rleConsume(buf, f);
    const isRaw = raw === sp[i];
    const isRle = !r.err && r.used === sp[i];
    if (isRaw && !isRle) rawOk++;
    else if (isRle && !isRaw) { rleOk++; detail.push(`RLE f${i} ${f.w}x${f.h} rows=${f.rows} span=${sp[i]} raw=${raw}`); }
    else if (isRaw && isRle) rawOk++;
    else { neither++; detail.push(`?? f${i} ${f.w}x${f.h} rows=${f.rows} span=${sp[i]} raw=${raw} rle=${r.used} err=${r.err || ''}`); }
  }
  console.log(`  raw-fits=${rawOk} rle-fits(only)=${rleOk} neither=${neither}`);
  for (const d of detail.slice(0, 40)) console.log('   ' + d);
}
