'use strict';
// Throwaway analysis helper for the 23 KNOWN_FAILING PL8 files.
// Implements the documented model from docs/formats/pl8.md + pl8-mode2.md,
// then reports per-frame byte consumption vs the declared offsets.
const fs = require('fs');
const path = require('path');

function parse(buf) {
  const family = buf[0];
  const zoom = buf[1];
  const count = buf.readUInt16LE(2);
  const h4 = buf.readUInt16LE(4);
  const h6 = buf[6];
  const h7 = buf[7];
  const frames = [];
  for (let i = 0; i < count; i++) {
    const o = 8 + i * 16;
    frames.push({
      i,
      w: buf.readUInt16LE(o),
      h: buf.readUInt16LE(o + 2),
      off: buf.readUInt32LE(o + 4),
      x: buf.readInt16LE(o + 8),
      y: buf.readInt16LE(o + 10),
      shape: buf[o + 12],
      rows: buf[o + 13],
      p14: buf[o + 14],
      p15: buf[o + 15],
    });
  }
  return { family, zoom, count, h4, h6, h7, frames, size: buf.length };
}

// declared span of frame i = next dataOffset - own dataOffset (EOF for last)
function spans(p) {
  return p.frames.map((f, i) =>
    (i + 1 < p.frames.length ? p.frames[i + 1].off : p.size) - f.off);
}

// bytes consumed under the documented model. returns {used, err}
function consume(buf, p, f) {
  const { w, h, shape, rows } = f;
  if (p.family === 1) return rleConsume(buf, f);
  if (p.family === 2) {
    if (shape === 0) return { used: w * h };
    const hh = h >> 1;
    let n = h * h;
    if (shape === 2) n += rows * w;
    else if (shape === 3 || shape === 4) n += rows * h;
    return { used: n };
  }
  return { used: w * h };
}

function rleConsume(buf, f) {
  let p = f.off;
  for (let r = 0; r < f.h; r++) {
    let x = 0;
    while (x < f.w) {
      if (p >= buf.length) return { used: p - f.off, err: 'eof' };
      const n = buf[p++];
      if (n === 0) {
        if (p >= buf.length) return { used: p - f.off, err: 'eof' };
        const m = buf[p++];
        if (m === 0) return { used: p - f.off, err: 'zero-skip' };
        x += m;
      } else { p += n; x += n; }
    }
    if (x !== f.w) return { used: p - f.off, err: `row ${r} overrun ${x}>${f.w}` };
  }
  return { used: p - f.off };
}

function loadAll(dir) {
  return fs.readdirSync(dir).filter(n => /\.pl8$/i.test(n)).sort()
    .map(n => ({ name: n, buf: fs.readFileSync(path.join(dir, n)) }));
}

module.exports = { parse, spans, consume, rleConsume, loadAll };
