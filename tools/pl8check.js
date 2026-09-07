// pl8check.js - validate PL8 decoding across a directory.
// Storage mode is the u16 at offset 0:  0 = raw (w*h bytes), 1 = RLE.
const fs = require('fs'), path = require('path');
const dir = process.argv[2];
const files = fs.readdirSync(dir).filter(f => /\.pl8$/i.test(f)).sort();
let ok = 0, bad = 0, frames = 0, modes = {};
const failures = [];

for (const f of files) {
  const b = fs.readFileSync(path.join(dir, f));
  let fileOk = true, why = [];
  try {
    const mode = b[0], sub = b[1], n = b.readUInt16LE(2);
    modes[mode+':'+sub] = (modes[mode+':'+sub] || 0) + 1;
    if (b.readUInt32LE(12) !== 8 + n * 16) { fileOk = false; why.push('first offset mismatch'); }
    for (let i = 0; i < n && fileOk; i++) {
      const rec = 8 + i * 16;
      const w = b.readUInt16LE(rec), h = b.readUInt16LE(rec + 2), off = b.readUInt32LE(rec + 4);
      const next = (i + 1 < n) ? b.readUInt32LE(rec + 16 + 4) : b.length;
      let p = off;
      if (mode === 0) {
        p = off + w * h;
      } else {
        for (let y = 0; y < h; y++) {
          let x = 0;
          while (x < w) {
            if (p >= b.length) { fileOk = false; why.push(`f${i} EOF overrun`); x = w; y = h; break; }
            const c = b[p++];
            if (c === 0) x += b[p++]; else { p += c; x += c; }
          }
          if (x !== w) { fileOk = false; why.push(`f${i} row${y} ${x}!=${w}`); break; }
        }
      }
      if (fileOk && p !== next) { fileOk = false; why.push(`f${i} end 0x${p.toString(16)} want 0x${next.toString(16)}`); }
      frames++;
    }
  } catch (e) { fileOk = false; why.push('exception: ' + e.message); }
  if (fileOk) ok++; else { bad++; failures.push(`${f} [mode ${b[0]}:${b[1]}]: ${why[0]}`); }
}
console.log(`files: ${ok} OK, ${bad} FAILED   frames: ${frames}`);
console.log('storage modes seen:', modes);
if (failures.length) { console.log('\nfailures:'); failures.slice(0, 12).forEach(x => console.log('  ' + x)); }
