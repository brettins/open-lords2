'use strict';
// Where is byte 0x0D honoured on shape-0 frames, and where is it ignored?
const { parse, spans, rleConsume, loadAll } = require('./pl8lib');
const DIR = process.argv[2] || 'F:/games/Lords of the Realm II';

function rleRows(buf, p, w, n) {
  for (let r = 0; r < n; r++) {
    let x = 0;
    while (x < w) {
      if (p >= buf.length) return -1;
      const c = buf[p++];
      if (c === 0) { if (p >= buf.length) return -1; const m = buf[p++]; if (m === 0) return -1; x += m; }
      else { p += c; x += c; }
    }
    if (x !== w) return -1;
  }
  return p;
}

const perFile = [];
let honoured = 0, ignored = 0, neither = 0, rowsZero = 0;
const honFiles = new Set(), ignFiles = new Set();
for (const { name, buf } of loadAll(DIR)) {
  const p = parse(buf);
  if (p.family > 2) continue;
  const sp = spans(p);
  let h = 0, ig = 0, n0 = 0;
  for (let i = 0; i < p.frames.length; i++) {
    const f = p.frames[i];
    if (f.shape !== 0) continue;
    const raw = f.w * f.h;
    if (f.rows === 0) { rowsZero++; n0++; continue; }
    const extra = sp[i] - raw;
    if (extra === 0) { ignored++; ig++; ignFiles.add(name); }
    else {
      const e = rleRows(buf, f.off + raw, f.w, f.rows);
      if (e >= 0 && e - f.off === sp[i]) { honoured++; h++; honFiles.add(name); }
      else { neither++; console.log(`NEITHER ${name} f${i} ${f.w}x${f.h} rows=${f.rows} extra=${extra}`); }
    }
  }
  if (h || ig) perFile.push(`${name} fam=${p.family} zoom=${p.zoom} h4=${p.h4} h6=${p.h6} h7=${p.h7} honoured=${h} ignored=${ig} rows0=${n0}`);
}
console.log(`shape-0 frames: rows==0 ${rowsZero}, rows>0 honoured ${honoured}, rows>0 ignored ${ignored}, unexplained ${neither}`);
console.log('--- files with any rows>0 shape-0 frame ---');
for (const l of perFile) console.log('  ' + l);
