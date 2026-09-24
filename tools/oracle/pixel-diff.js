// Compares two framebuffers per named region: node pixel-diff.js <orig.bmp> <ours.bmp> [--png out.png]
//   [--mask x,y,w,h ...] [--tol N]. The draw-call diff cannot see what the original does not repaint
// (menu bar, side panel, text it caches); its framebuffer holds them all the same.
// Shots: trace-run.ps1 SaveShot (8-bit, the fake ddraw's primary + palette) and our trace runner's
// shot.rs (24-bit, Machine::present). Palettes are 6-bit scaled on both sides by different formulas,
// so a channel within --tol (default 8) is the same colour.
'use strict';
const fs = require('fs');
const path = require('path');
const zlib = require('zlib');

const W = 640, H = 480;
// docs/screens.md §0 and §4.3 [V] for the campaign map; the battle overview §6 [D].
const REGIONS = {
  'menu-bar': [0, 0, 640, 24],
  'map-view': [0, 24, 478, 456],
  'side-panel': [478, 24, 162, 456],
  'minimap': [478, 24, 162, 132],
  'county-panel': [478, 156, 162, 274],
  'end-turn': [478, 430, 162, 50],
  'battle-overview': [480, 48, 160, 160],
  'full': [0, 0, 640, 480],
};

// An 8-, 24- or 32-bit BMP as RGB, top row first.
function readBmp(file) {
  const b = fs.readFileSync(file);
  if (b.toString('latin1', 0, 2) !== 'BM') throw new Error(`${file}: not a BMP`);
  const off = b.readUInt32LE(10), hs = b.readUInt32LE(14), w = b.readInt32LE(18), hRaw = b.readInt32LE(22);
  const bpp = b.readUInt16LE(28), h = Math.abs(hRaw), up = hRaw > 0;
  const row = ((w * bpp + 31) >> 5) << 2, rgb = Buffer.alloc(w * h * 3);
  const pal = 14 + hs;
  for (let y = 0; y < h; y++) {
    const src = off + (up ? h - 1 - y : y) * row;
    for (let x = 0; x < w; x++) {
      const o = (y * w + x) * 3;
      if (bpp === 8) { const p = pal + b[src + x] * 4; rgb[o] = b[p + 2]; rgb[o + 1] = b[p + 1]; rgb[o + 2] = b[p]; }
      else { const p = src + x * (bpp >> 3); rgb[o] = b[p + 2]; rgb[o + 1] = b[p + 1]; rgb[o + 2] = b[p]; }
    }
  }
  return { w, h, rgb };
}

const inside = (x, y, [rx, ry, rw, rh]) => x >= rx && x < rx + rw && y >= ry && y < ry + rh;

// Per region: differing pixels (masked ones excluded) and their bounding box.
function compare(a, b, { masks = [], tol = 8 } = {}) {
  if (a.w !== W || a.h !== H || b.w !== W || b.h !== H) throw new Error(`frames are ${a.w}x${a.h} and ${b.w}x${b.h}, not ${W}x${H}`);
  const diff = new Uint8Array(W * H);
  for (let i = 0; i < W * H; i++) {
    const x = i % W, y = (i / W) | 0;
    if (masks.some((m) => inside(x, y, m))) { diff[i] = 2; continue; }
    const o = i * 3;
    if (Math.abs(a.rgb[o] - b.rgb[o]) > tol || Math.abs(a.rgb[o + 1] - b.rgb[o + 1]) > tol || Math.abs(a.rgb[o + 2] - b.rgb[o + 2]) > tol) diff[i] = 1;
  }
  const regions = {};
  for (const [name, r] of Object.entries(REGIONS)) {
    let n = 0, x0 = W, y0 = H, x1 = -1, y1 = -1;
    for (let y = r[1]; y < r[1] + r[3]; y++) for (let x = r[0]; x < r[0] + r[2]; x++) {
      if (diff[y * W + x] !== 1) continue;
      n++; if (x < x0) x0 = x; if (y < y0) y0 = y; if (x > x1) x1 = x; if (y > y1) y1 = y;
    }
    regions[name] = { pixels: n, of: r[2] * r[3], box: n ? [x0, y0, x1 - x0 + 1, y1 - y0 + 1] : null };
  }
  return { regions, diff };
}

const CRC = new Int32Array(256).map((_, n) => { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; return c; });
const crc32 = (buf) => { let c = -1; for (const v of buf) c = CRC[(c ^ v) & 255] ^ (c >>> 8); return (c ^ -1) >>> 0; };
function png(w, h, rgb) {
  const chunk = (type, data) => {
    const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
    const td = Buffer.concat([Buffer.from(type, 'latin1'), data]);
    const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(td));
    return Buffer.concat([len, td, crc]);
  };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) rgb.copy(raw, y * (w * 3 + 1) + 1, y * w * 3, (y + 1) * w * 3);
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}

// Original | ours | the original dimmed with differing pixels red and masked ones blue.
function sideBySide(a, b, diff, file) {
  const out = Buffer.alloc(W * 3 * H * 3);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const i = y * W + x, s = i * 3, row = y * W * 9;
    a.rgb.copy(out, row + x * 3, s, s + 3);
    b.rgb.copy(out, row + (W + x) * 3, s, s + 3);
    const d = row + (2 * W + x) * 3, g = (a.rgb[s] + a.rgb[s + 1] + a.rgb[s + 2]) / 9;
    const px = diff[i] === 1 ? [255, 0, 0] : diff[i] === 2 ? [0, 0, 160] : [g, g, g];
    out[d] = px[0]; out[d + 1] = px[1]; out[d + 2] = px[2];
  }
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, png(W * 3, H, out));
}

// tools/oracle/pixel-masks.json: { "<scenario>" | "*": [[x, y, w, h, "why"], ...] }.
function masksFor(scenario) {
  const f = path.join(__dirname, 'pixel-masks.json');
  if (!fs.existsSync(f)) return [];
  const all = JSON.parse(fs.readFileSync(f, 'utf8'));
  return [...(all['*'] || []), ...(all[scenario] || [])].map((m) => m.slice(0, 4));
}

// One pair of shot files: the region table, and the PNG when `png` names one.
function diffShots(origFile, oursFile, { scenario, png: pngFile, tol } = {}) {
  const a = readBmp(origFile), b = readBmp(oursFile);
  const { regions, diff } = compare(a, b, { masks: masksFor(scenario), tol });
  if (pngFile && regions.full.pixels) sideBySide(a, b, diff, pngFile);
  return regions;
}

const fmt = (regions) => Object.entries(regions).filter(([, r]) => r.pixels)
  .map(([n, r]) => `${n} ${r.pixels}/${r.of} at ${r.box.join(',')}`).join('; ') || 'identical';

if (require.main === module) {
  const args = process.argv.slice(2);
  const opt = (n) => { const i = args.indexOf(n); return i < 0 ? undefined : args[i + 1]; };
  const files = args.filter((x, i) => !x.startsWith('--') && !['--png', '--mask', '--tol'].includes(args[i - 1]));
  if (files.length !== 2) { console.error('usage: node pixel-diff.js <orig.bmp> <ours.bmp> [--png out] [--mask x,y,w,h] [--tol N]'); process.exit(2); }
  const masks = args.flatMap((x, i) => (args[i - 1] === '--mask' ? [x.split(',').map(Number)] : []));
  const a = readBmp(files[0]), b = readBmp(files[1]);
  const { regions, diff } = compare(a, b, { masks, tol: opt('--tol') == null ? 8 : Number(opt('--tol')) });
  if (opt('--png')) sideBySide(a, b, diff, opt('--png'));
  console.log(JSON.stringify(regions));
  process.exit(regions.full.pixels ? 1 : 0);
}
module.exports = { readBmp, compare, diffShots, fmt, png, REGIONS };
