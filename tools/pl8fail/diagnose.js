'use strict';
const { parse, spans, consume, loadAll } = require('./pl8lib');
const DIR = process.argv[2] || 'F:/games/Lords of the Realm II';
const only = process.argv.slice(3);

const FAIL = ['Base2a','Castle1a','Castle1b','Castle1c','Castle1d','Castle2a','Castle2b',
  'Castle2c','Castle2d','Fntl2_14','Font_10','Font_c2','Roads2a','T16_bat1','T32_bat',
  'Town1a','Town1b','Town1c','Town1d','Town2a','Town2b','Town2c','Town2d'];

for (const { name, buf } of loadAll(DIR)) {
  const base = name.replace(/\.pl8$/i, '');
  if (only.length ? !only.includes(base) : !FAIL.includes(base)) continue;
  const p = parse(buf);
  const sp = spans(p);
  const bad = [];
  for (let i = 0; i < p.frames.length; i++) {
    const f = p.frames[i];
    const c = consume(buf, p, f);
    if (c.used !== sp[i] || c.err) bad.push({ i, f, used: c.used, span: sp[i], d: c.used - sp[i], err: c.err });
  }
  const shapes = {};
  for (const f of p.frames) shapes[f.shape] = (shapes[f.shape] || 0) + 1;
  console.log(`\n=== ${name}  size=${buf.length} fam=${p.family} zoom=${p.zoom} n=${p.count} h4=${p.h4} h6=${p.h6} h7=${p.h7} shapes=${JSON.stringify(shapes)}`);
  console.log(`  first dataOffset=${p.frames[0].off} expected=${8 + p.count * 16}`);
  console.log(`  bad frames: ${bad.length}/${p.count}`);
  for (const b of bad.slice(0, 12)) {
    console.log(`   f${b.i} ${b.f.w}x${b.f.h} off=${b.f.off} shape=${b.f.shape} rows=${b.f.rows} p14=${b.f.p14} p15=${b.f.p15} used=${b.used} span=${b.span} delta=${b.d}${b.err ? ' err=' + b.err : ''}`);
  }
  if (bad.length > 12) console.log(`   ... ${bad.length - 12} more`);
  // delta histogram
  const hist = {};
  for (const b of bad) hist[b.d] = (hist[b.d] || 0) + 1;
  console.log(`  delta histogram: ${JSON.stringify(hist)}`);
}
