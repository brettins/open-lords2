// rva.js - read bytes out of Lords2.exe at a virtual address, with no Ghidra.
//
// Written for docs/audit-method.md. The whole-binary decompilation in
// tools/oracle/decomp/ answers "what does this function do"; it does not answer
// "what are the exact bytes", which is what you need when the decompiler's C
// looks impossible (a duplicated comparison, a switch it flattened wrong) or
// when the value lives in .rdata/.data rather than in code.
//
// Usage:
//   node tools/audit/rva.js <hexVA> [count]        hex+ascii dump
//   node tools/audit/rva.js <hexVA> <count> i8|u8|i16|u16|i32|u32
//   node tools/audit/rva.js sections
//
// Env LORDS2_EXE overrides the path.
const fs = require('fs');
const EXE = process.env.LORDS2_EXE || 'F:/games/Lords of the Realm II/Lords2.exe';
const b = fs.readFileSync(EXE);
const pe = b.readUInt32LE(0x3c);
const nsec = b.readUInt16LE(pe + 6), optOff = pe + 24, optSize = b.readUInt16LE(pe + 20);
const BASE = b.readUInt32LE(optOff + 28);
const secs = [];
for (let i = 0; i < nsec; i++) {
  const o = optOff + optSize + i * 40;
  secs.push({
    name: b.subarray(o, o + 8).toString().replace(/\0/g, ''),
    vsize: b.readUInt32LE(o + 8), va: b.readUInt32LE(o + 12),
    rsize: b.readUInt32LE(o + 16), roff: b.readUInt32LE(o + 20),
  });
}
// A VA is *mapped* if it falls in the virtual extent; it is *stored* only if it
// also falls inside the raw data. Uninitialised .data is mapped but not stored,
// and that distinction is the whole of decisions.md C14/C16.
function locate(va) {
  const r = va - BASE;
  for (const s of secs) {
    if (r >= s.va && r < s.va + Math.max(s.vsize, s.rsize)) {
      const off = s.roff + (r - s.va);
      return { sec: s, off, stored: (r - s.va) < s.rsize };
    }
  }
  return null;
}
const arg = process.argv[2];
if (!arg || arg === 'sections') {
  console.log(`${EXE}  ${b.length} bytes  ImageBase 0x${BASE.toString(16)}`);
  for (const s of secs) {
    console.log(`  ${s.name.padEnd(8)} VA 0x${(BASE + s.va).toString(16)}..0x${(BASE + s.va + s.vsize).toString(16)}` +
      `  vsize ${s.vsize}  raw ${s.rsize}  fileoff 0x${s.roff.toString(16)}` +
      (s.vsize > s.rsize ? `  [${s.vsize - s.rsize} bytes mapped but NOT stored]` : ''));
  }
  process.exit(0);
}
const va = parseInt(arg, 16);
const n = parseInt(process.argv[3] || '64', 10);
const ty = process.argv[4];
const loc = locate(va);
if (!loc) { console.error(`0x${va.toString(16)} is in no section`); process.exit(1); }
console.error(`# 0x${va.toString(16)} -> ${loc.sec.name} file 0x${loc.off.toString(16)} ` +
  `(${loc.stored ? 'stored in file' : 'MAPPED BUT NOT STORED - reads as zero at runtime init'})`);
if (!loc.stored) { console.log('(no bytes in file)'); process.exit(0); }
if (ty) {
  const sz = { i8: 1, u8: 1, i16: 2, u16: 2, i32: 4, u32: 4 }[ty];
  const rd = { i8: 'readInt8', u8: 'readUInt8', i16: 'readInt16LE', u16: 'readUInt16LE', i32: 'readInt32LE', u32: 'readUInt32LE' }[ty];
  if (!sz) { console.error('type must be i8|u8|i16|u16|i32|u32'); process.exit(1); }
  const out = [];
  for (let i = 0; i < n; i++) out.push(b[rd](loc.off + i * sz));
  console.log(out.join(' '));
} else {
  for (let i = 0; i < n; i += 16) {
    const row = b.subarray(loc.off + i, loc.off + Math.min(i + 16, n));
    console.log(`${(va + i).toString(16).padStart(8, '0')}  ` +
      [...row].map(x => x.toString(16).padStart(2, '0')).join(' ').padEnd(48) + '  ' +
      [...row].map(x => (x >= 32 && x < 127) ? String.fromCharCode(x) : '.').join(''));
  }
}
