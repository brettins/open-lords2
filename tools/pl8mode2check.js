#!/usr/bin/env node
// Validate the PL8 storage-mode-2 (isometric) model against a directory of .pl8 files.
// See docs/formats/pl8-mode2.md.
//
//   node tools/pl8mode2check.js "F:/games/Lords of the Realm II"
//
// Two independent checks, both of which must pass for every frame:
//   1. end-offset invariant  - the decode consumes exactly the bytes from this
//      frame's dataOffset up to the next frame's dataOffset (or EOF)
//   2. canvas bounds         - every pixel written lands inside the frame's
//      true bounding box, width x (height + extraRows)
//
// Dependency-free.

'use strict';
const fs = require('fs');
const path = require('path');

const SHAPE_RAW = 0;
const SHAPE_DIAMOND = 1;
const SHAPE_FULL = 2;
const SHAPE_LEFT = 3;
const SHAPE_RIGHT = 4;

function parse(buf) {
  const n = buf.readUInt16LE(2);
  const frames = [];
  for (let i = 0; i < n; i++) {
    const o = 8 + i * 16;
    frames.push({
      index: i,
      w: buf.readUInt16LE(o),
      h: buf.readUInt16LE(o + 2),
      off: buf.readUInt32LE(o + 4),
      x: buf.readInt16LE(o + 8),
      y: buf.readInt16LE(o + 10),
      shape: buf[o + 12],
      rows: buf[o + 13],
    });
  }
  for (let i = 0; i < n; i++) {
    frames[i].end = i + 1 < n ? frames[i + 1].off : buf.length;
  }
  return { mode: buf[0], zoom: buf[1], count: n, frames };
}

// Bytes a frame must occupy, or null if the shape byte is unknown.
function expectedSize(f) {
  const rows = f.shape >= SHAPE_FULL ? f.rows : 0; // shape 1 ignores byte 0x0D
  switch (f.shape) {
    case SHAPE_RAW: return f.w * f.h;
    case SHAPE_DIAMOND: return f.h * f.h;
    case SHAPE_FULL: return f.h * f.h + rows * f.w;
    case SHAPE_LEFT:
    case SHAPE_RIGHT: return f.h * f.h + rows * f.h;
    default: return null;
  }
}

// A "grid" frame is a 1/8-resolution region-ID table, not pixels.
function isRegionGrid(f) {
  return f.shape === SHAPE_RAW &&
         f.w % 8 === 0 && f.h % 8 === 0 &&
         f.end - f.off === (f.w / 8) * (f.h / 8);
}

// Walk the frame exactly as the decoder would, reporting bytes consumed and
// any write that falls outside the width x (height + rows) canvas.
function walk(f) {
  let p = f.off;
  let oob = 0;

  if (f.shape === SHAPE_RAW) return { consumed: f.w * f.h, oob: 0 };

  const hh = f.h >> 1;
  const H = f.h + (f.shape >= SHAPE_FULL ? f.rows : 0);

  for (let r = 0; r < f.h; r++) {
    const rw = r < hh ? 2 + 4 * r : 2 + 4 * (f.h - 1 - r);
    const x0 = (f.w - rw) >> 1;
    if (x0 < 0 || x0 + rw > f.w) oob += rw;
    p += rw;
  }

  if (f.shape >= SHAPE_FULL) {
    const recLen = f.shape === SHAPE_FULL ? f.w : f.h;
    const mFrom = f.shape === SHAPE_RIGHT ? hh - 1 : 0;
    const mTo = f.shape === SHAPE_FULL ? f.w / 2 - 1
              : f.shape === SHAPE_LEFT ? hh - 1
              : 2 * hh - 2;
    for (let i = 0; i < f.rows; i++) {
      for (let m = mFrom; m <= mTo; m++) {
        const cy = f.rows + Math.abs(hh - 1 - m) - (i + 1);
        const cx = 2 * m;
        if (cy < 0 || cy >= H || cx < 0 || cx + 1 >= f.w + 1 || cx + 1 > f.w - 1) oob++;
      }
      p += recLen;
    }
  }

  return { consumed: p - f.off, oob };
}

function main() {
  const dir = process.argv[2];
  if (!dir) {
    console.error('usage: node tools/pl8mode2check.js <game-directory>');
    process.exit(2);
  }

  let files = 0, ok = 0, bad = 0;
  let iso = 0, isoMismatch = 0, raw = 0, grids = 0, oob = 0, geomOk = 0, geomBad = 0;
  const failures = [];

  for (const name of fs.readdirSync(dir).filter(f => /\.pl8$/i.test(f)).sort()) {
    const buf = fs.readFileSync(path.join(dir, name));
    if (buf.length < 8) continue;
    const p = parse(buf);
    if (p.mode !== 2) continue;
    files++;

    if (p.frames.length && p.frames[0].off !== 8 + 16 * p.count) {
      failures.push(`${name}: first dataOffset ${p.frames[0].off} != ${8 + 16 * p.count}`);
    }

    for (const f of p.frames) {
      const actual = f.end - f.off;

      if (isRegionGrid(f)) { grids++; ok++; continue; }

      const exp = expectedSize(f);
      if (exp !== actual) {
        bad++;
        failures.push(`${name} f${f.index} ${f.w}x${f.h} shape=${f.shape} ` +
                      `rows=${f.rows}: got ${actual} bytes, expected ${exp}`);
        continue;
      }
      ok++;

      if (f.shape === SHAPE_RAW) { raw++; continue; }

      iso++;
      if (f.w !== 2 * f.h - 2 || f.h % 2 !== 0) {
        geomBad++;
        failures.push(`${name} f${f.index} ${f.w}x${f.h}: violates w == 2h-2 (h even)`);
      } else {
        geomOk++;
      }

      const wk = walk(f);
      if (wk.consumed !== actual) {
        isoMismatch++;
        failures.push(`${name} f${f.index}: walk consumed ${wk.consumed}, declared ${actual}`);
      }
      oob += wk.oob;
    }
  }

  console.log(`mode-2 files: ${files}   frames ok: ${ok}   frames bad: ${bad}`);
  console.log(`  isometric frames (shape 1/2/3/4): ${iso}, byte-consumption mismatches: ${isoMismatch}`);
  console.log(`  raw frames (shape 0):              ${raw}`);
  console.log(`  1/8-resolution region grids:       ${String(grids).padStart(3)}`);
  console.log(`  out-of-canvas writes:              ${String(oob).padStart(3)}`);
  console.log(`  isometric frames with w == 2h-2 and h even: ${geomOk} / ${geomOk + geomBad}`);

  if (failures.length) {
    console.log('\nfailures:');
    for (const f of failures.slice(0, 50)) console.log('  ' + f);
    if (failures.length > 50) console.log(`  ... and ${failures.length - 50} more`);
    process.exit(1);
  }
}

main();
