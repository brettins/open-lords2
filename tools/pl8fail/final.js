'use strict';
// Deterministic decode model (no "try both" fallbacks) + canvas-bounds check.
//   node final.js "F:/games/Lords of the Realm II"
const { parse, spans, loadAll } = require('./pl8lib');
const DIR = process.argv[2] || 'F:/games/Lords of the Realm II';

// --- RLE rows: consume exactly n rows of width w, painting into `paint(y,x,v)`
function rleRows(buf, p, w, n, paint) {
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
      } else {
        if (p + c > buf.length) return -1;
        for (let k = 0; k < c; k++) { if (paint) paint(r, x + k, buf[p + k]); }
        p += c; x += c;
      }
    }
    if (x !== w) return -1;
  }
  return p;
}

// --- one frame: returns { end, rule, oob }
function decode(buf, hdr, f) {
  const { w, h, shape, rows, off } = f;
  if (shape !== 0) {                                   // isometric diamond family
    const hh = h >> 1;
    let n = h * h;
    let oob = 0;
    if (w !== 2 * h - 2 || h % 2) oob = -1;            // geometry precondition
    const H = h + (shape >= 2 ? rows : 0);
    if (shape === 2) n += rows * w;
    else if (shape === 3 || shape === 4) n += rows * h;
    // canvas-bounds check for the chevron records
    if (shape >= 2) {
      for (let i = 0; i < rows; i++) {
        const mLo = shape === 2 ? 0 : shape === 3 ? 0 : hh - 1;
        const mHi = shape === 2 ? (w >> 1) - 1 : shape === 3 ? hh - 1 : 2 * hh - 2;
        for (let m = mLo; m <= mHi; m++) {
          const cy = rows + Math.abs(hh - 1 - m) - (i + 1);
          if (cy < 0 || cy >= H || 2 * m + 1 >= w) oob++;
        }
      }
    }
    return { end: off + n, rule: 'iso' + shape, oob };
  }
  if (hdr.family === 1 && hdr.zoom === 0) {            // RLE sprite stream
    const e = rleRows(buf, off, w, h, null);
    return { end: e, rule: 'rle' };
  }
  let end = off + w * h;
  let rule = 'raw';
  if (hdr.family === 0 && rows > 0) {                  // overhang rows above the rect
    const e = rleRows(buf, end, w, rows, null);
    end = e; rule = 'raw+over';
  }
  return { end, rule };
}

const tally = {};
let filesOk = 0, filesBad = 0, framesOk = 0, framesBad = 0, oobTotal = 0;
const bad = [];
for (const { name, buf } of loadAll(DIR)) {
  const p = parse(buf);
  const sp = spans(p);
  let nb = 0;
  for (let i = 0; i < p.count; i++) {
    const f = p.frames[i];
    let r = decode(buf, p, f);
    // structural exception: the 1/8-resolution region-id tables (4 files)
    if (r.end - f.off !== sp[i] && f.shape === 0 &&
        sp[i] === (f.w >> 3) * (f.h >> 3)) { r = { end: f.off + sp[i], rule: 'grid' }; }
    if (r.end === f.off + sp[i]) { framesOk++; tally[r.rule] = (tally[r.rule] || 0) + 1; }
    else { framesBad++; nb++; if (bad.length < 20) bad.push(`${name} f${i} rule=${r.rule} used=${r.end - f.off} span=${sp[i]}`); }
    if (r.oob) oobTotal += (r.oob === -1 ? 1 : r.oob);
  }
  if (nb) { filesBad++; } else filesOk++;
}
console.log(`${DIR}`);
console.log(`  files ok=${filesOk} bad=${filesBad}   frames ok=${framesOk} bad=${framesBad}`);
console.log(`  by rule: ${JSON.stringify(tally)}`);
console.log(`  isometric canvas/geometry violations: ${oobTotal}`);
for (const b of bad) console.log('   ' + b);
