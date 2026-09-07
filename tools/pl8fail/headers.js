'use strict';
// Header bytes + content classification for every file matching a name pattern.
const { parse, spans, loadAll } = require('./pl8lib');
const DIR = process.argv[2] || 'F:/games/Lords of the Realm II';
const re = new RegExp(process.argv[3] || '.', 'i');
for (const { name, buf } of loadAll(DIR)) {
  if (!re.test(name)) continue;
  const p = parse(buf);
  const shapes = {};
  for (const f of p.frames) shapes[f.shape] = (shapes[f.shape] || 0) + 1;
  const iso = p.frames.filter(f => f.shape !== 0).length;
  console.log(`${name.padEnd(15)} fam=${p.family} zoom=${p.zoom} n=${String(p.count).padStart(4)} h4=${String(p.h4).padStart(3)} h6=${p.h6} h7=${String(p.h7).padStart(2)} iso=${String(iso).padStart(4)} raw=${p.count - iso} size=${buf.length}`);
}
