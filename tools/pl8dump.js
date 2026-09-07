// pl8dump.js - decode a Lords of the Realm II .pl8 sprite frame to PNG.
// Usage: node pl8dump.js <file.pl8> <palette.256> <frameIndex> <out.png>
const fs = require('fs'), zlib = require('zlib');

function crc32(buf) {
  let c, t = [];
  for (let n = 0; n < 256; n++) { c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xEDB88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; }
  let crc = 0xFFFFFFFF;
  for (const b of buf) crc = t[(crc ^ b) & 0xFF] ^ (crc >>> 8);
  return (crc ^ 0xFFFFFFFF) >>> 0;
}
function chunk(type, data) {
  const len = Buffer.alloc(4); len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const c = Buffer.alloc(4); c.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, c]);
}
function png(w, h, rgba) {
  const raw = Buffer.alloc((w * 4 + 1) * h);
  for (let y = 0; y < h; y++) { raw[y * (w * 4 + 1)] = 0; rgba.copy(raw, y * (w * 4 + 1) + 1, y * w * 4, (y + 1) * w * 4); }
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; ihdr[9] = 6; ihdr[10] = 0; ihdr[11] = 0; ihdr[12] = 0;
  return Buffer.concat([Buffer.from([0x89,0x50,0x4E,0x47,0x0D,0x0A,0x1A,0x0A]),
    chunk('IHDR', ihdr), chunk('IDAT', zlib.deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}

const [file, palFile, frameArg, out] = process.argv.slice(2);
const b = fs.readFileSync(file);
const pal = fs.readFileSync(palFile);
const frameIdx = parseInt(frameArg || '0', 10);

const storage = b[0], subMode = b[1], frames = b.readUInt16LE(2);
console.log(`${file}: storage=${storage}:${subMode} frames=${frames} u4=${b.readUInt16LE(4)} b6=${b[6]} b7=${b[7]}`);
if (frameIdx >= frames) { console.error(`frame ${frameIdx} out of range`); process.exit(1); }

const rec = 8 + frameIdx * 16;
const w = b.readUInt16LE(rec), h = b.readUInt16LE(rec + 2), off = b.readUInt32LE(rec + 4);
const extra = b.slice(rec + 8, rec + 16).toString('hex');
console.log(`frame ${frameIdx}: ${w}x${h} dataOffset=0x${off.toString(16)} trailing=${extra}`);
console.log(`expected first offset = 8 + ${frames}*16 = ${8 + frames * 16} (0x${(8 + frames * 16).toString(16)})`);

// Decode to palette indices. Palette index 0 is transparent: the game blits
// only non-zero bytes (verified in the original at 0x004B43B1).
const idx = new Uint8Array(w * h).fill(0);
const alpha = new Uint8Array(w * h).fill(0);
let p = off;
if (storage === 0) {
  for (let i = 0; i < w * h; i++) {
    const v = b[off + i];
    idx[i] = v;
    alpha[i] = v !== 0 ? 255 : 0;
  }
  p = off + w * h;
} else if (storage === 1) {
  for (let y = 0; y < h; y++) {
    let x = 0;
    while (x < w) {
      if (p >= b.length) { console.error(`ran off end at row ${y}`); y = h; break; }
      const n = b[p++];
      if (n === 0) {
        const skip = b[p++];
        if (skip === 0) { console.error(`row ${y}: zero-length skip run`); break; }
        x += skip;
      } else {
        for (let i = 0; i < n; i++) {
          const v = b[p++];
          if (x < w) { idx[y * w + x] = v; alpha[y * w + x] = v !== 0 ? 255 : 0; }
          x++;
        }
      }
    }
    if (x !== w) console.error(`row ${y} width mismatch: got ${x} want ${w}`);
  }
} else {
  console.error(`storage mode ${storage} is not decodable yet`);
  process.exit(1);
}
console.log(`decoded ${p - off} bytes; next frame starts at 0x${(frames > frameIdx + 1 ? b.readUInt32LE(rec + 16 + 4) : p).toString(16)}`);

const rgba = Buffer.alloc(w * h * 4);
for (let i = 0; i < w * h; i++) {
  const e = idx[i] * 3;
  // 6-bit VGA -> 8-bit
  rgba[i*4]   = Math.round(pal[e]     * 255 / 63);
  rgba[i*4+1] = Math.round(pal[e + 1] * 255 / 63);
  rgba[i*4+2] = Math.round(pal[e + 2] * 255 / 63);
  rgba[i*4+3] = alpha[i];
}
fs.writeFileSync(out, png(w, h, rgba));
console.log(`wrote ${out}`);
