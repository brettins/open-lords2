'use strict';
// Full-corpus byte-consumption check under a chosen model.
//   node check.js [dir] [model]
// models:  doc   = the model currently in docs/formats/pl8.md (family dispatches)
//          shape = shape byte dispatches for every family
//          full  = shape + trailing RLE rows for shape 0
const { parse, spans, rleConsume, loadAll } = require('./pl8lib');
const DIR = process.argv[2] || 'F:/games/Lords of the Realm II';
const MODEL = process.argv[3] || 'full';
const VERBOSE = !!process.env.VERBOSE;

function isoSize(f) {
  const n = f.h * f.h;
  if (f.shape === 2) return n + f.rows * f.w;
  if (f.shape === 3 || f.shape === 4) return n + f.rows * f.h;
  return n; // shape 1 ignores rows
}

// consume `n` RLE rows of width w starting at p. returns end offset or -1
function rleRows(buf, p, w, n) {
  for (let r = 0; r < n; r++) {
    let x = 0;
    while (x < w) {
      if (p >= buf.length) return -1;
      const c = buf[p++];
      if (c === 0) {
        if (p >= buf.length) return -1;
        const m = buf[p++];
        if (m === 0) return -1;
        x += m;
      } else { p += n === 0 ? 0 : c; x += c; }
    }
    if (x !== w) return -1;
  }
  return p;
}

function used(buf, p, f, span) {
  const raw = f.w * f.h;
  const grid = (f.w >> 3) * (f.h >> 3);
  if (MODEL === 'doc') {
    if (p.family === 2) {
      if (f.shape === 0) return span === grid ? grid : raw;
      return isoSize(f);
    }
    if (p.family === 1) { const r = rleConsume(buf, f); return r.err ? -1 : r.used; }
    return raw;
  }
  // shape-dispatched models
  if (f.shape !== 0) return isoSize(f);
  if (p.family === 1 && MODEL !== 'shape') {
    // family 1 = RLE, except Font_c2-style files (see report)
    const r = rleConsume(buf, f);
    if (!r.err && r.used === span) return r.used;
    if (raw === span) return raw;              // stored raw despite family 1
    return r.err ? -1 : r.used;
  }
  if (p.family === 1) { const r = rleConsume(buf, f); return r.err ? -1 : r.used; }
  if (span === grid && grid !== raw) return grid; // region-id table
  if (MODEL === 'full' && f.rows > 0) {
    const e = rleRows(buf, f.off + raw, f.w, f.rows);
    if (e >= 0 && e - f.off === span) return e - f.off;
    return raw; // rows byte not honoured (matches when extra == 0)
  }
  return raw;
}

let filesOk = 0, filesBad = 0, framesOk = 0, framesBad = 0;
const badFiles = [];
for (const { name, buf } of loadAll(DIR)) {
  const p = parse(buf);
  if (p.family > 2) continue;
  if (p.frames[0] && p.frames[0].off !== 8 + p.count * 16) { /* still check */ }
  const sp = spans(p);
  let bad = 0;
  for (let i = 0; i < p.frames.length; i++) {
    const u = used(buf, p, p.frames[i], sp[i]);
    if (u === sp[i]) framesOk++; else { framesBad++; bad++;
      if (VERBOSE && bad <= 4) console.log(`  ${name} f${i} ${p.frames[i].w}x${p.frames[i].h} shape=${p.frames[i].shape} rows=${p.frames[i].rows} used=${u} span=${sp[i]} d=${u - sp[i]}`); }
  }
  if (bad) { filesBad++; badFiles.push(`${name}(${bad}/${p.count})`); } else filesOk++;
}
console.log(`model=${MODEL}  files ok=${filesOk} bad=${filesBad}   frames ok=${framesOk} bad=${framesBad}`);
if (badFiles.length) console.log('bad files: ' + badFiles.join(' '));
