// tablediff.js - diff every constant table in the crates against Lords2.exe.
//
// tools/oracle/kingdom.ps1 asserts 19 tables, each address and each expected
// value typed in by hand; tools/oracle/tables.ps1 asserts nothing at all - it
// prints three tables for a human to eyeball. Meanwhile crates/l2-kingdom
// alone declares 42 `pub const` tables, most of which carry their source
// address in the doc comment right above them.
//
// So the address is already in the source. This reads it from there, reads the
// same address out of the executable, and diffs. Nothing is typed in twice,
// and a table added tomorrow is covered the moment its doc comment names an
// address.
//
// Usage: node tools/audit/tablediff.js
const fs = require('fs'), path = require('path'), cp = require('child_process');
const ROOT = 'E:/dev/lords2';
const files = [];
(function walk(d) {
  for (const f of fs.readdirSync(d)) {
    const p = path.join(d, f);
    if (fs.statSync(p).isDirectory()) { if (f !== 'target') walk(p); }
    else if (f.endsWith('.rs')) files.push(p);
  }
})(path.join(ROOT, 'crates'));

// PE mapping, same logic as rva.js.
const EXE = process.env.LORDS2_EXE || 'F:/games/Lords of the Realm II/Lords2.exe';
const b = fs.readFileSync(EXE);
const pe = b.readUInt32LE(0x3c), optOff = pe + 24, optSize = b.readUInt16LE(pe + 20);
const BASE = b.readUInt32LE(optOff + 28), nsec = b.readUInt16LE(pe + 6);
const secs = [];
for (let i = 0; i < nsec; i++) {
  const o = optOff + optSize + i * 40;
  secs.push({ vsize: b.readUInt32LE(o+8), va: b.readUInt32LE(o+12), rsize: b.readUInt32LE(o+16), roff: b.readUInt32LE(o+20) });
}
function loc(va) {
  const r = va - BASE;
  for (const s of secs) if (r >= s.va && r < s.va + Math.max(s.vsize, s.rsize))
    return { off: s.roff + (r - s.va), stored: (r - s.va) < s.rsize };
  return null;
}

const noaddr = [], nonnum = [];
let checked = 0, ok = 0, mismatch = 0, skipped = 0;
const report = [];
for (const p of files) {
  const src = fs.readFileSync(p, 'utf8');
  const rel = path.relative(ROOT, p);
  // A doc-comment block ending in `pub const NAME: [T; N] = [ ... ];`
  const re = /((?:^\s*\/\/\/.*\n)+)\s*pub const (\w+):\s*\[[\[(]?(i32|u16|u8|i8|u32)[^=]*=\s*\[([\s\S]*?)\];/gm;
  for (const m of src.matchAll(re)) {
    const doc = m[1], name = m[2], ty = m[3];
    const addrM = doc.match(/\(`?0x(00[0-9A-Fa-f]{6})`?\)/);
    if (!addrM) { skipped++; noaddr.push(name); continue; }
    const va = parseInt(addrM[1], 16);
    // flatten the literal; only pure-numeric tables can be compared
    const nums = m[4].replace(/\/\/.*$/gm, '').match(/-?\d+/g);
    if (!nums) { skipped++; nonnum.push(name); continue; }
    if (/[A-Za-z_]\w*\s*(,|\]|$)/.test(m[4].replace(/\/\/.*$/gm, ''))) { skipped++; continue; }
    const want = nums.map(Number);
    const l = loc(va);
    if (!l || !l.stored) { report.push(`  SKIP  ${name.padEnd(26)} 0x${va.toString(16)} not stored in the image`); skipped++; continue; }
    const sz = { i8:1,u8:1,u16:2,i32:4,u32:4 }[ty];
    const rd = { i8:'readInt8',u8:'readUInt8',u16:'readUInt16LE',i32:'readInt32LE',u32:'readUInt32LE' }[ty];
    const got = [];
    for (let i = 0; i < want.length; i++) got.push(b[rd](l.off + i * sz));
    checked++;
    const bad = [];
    for (let i = 0; i < want.length; i++) if (got[i] !== want[i]) bad.push(i);
    if (bad.length === 0) { ok++; report.push(`  PASS  ${name.padEnd(26)} 0x${va.toString(16)}  ${want.length} x ${ty}  (${rel})`); }
    else { mismatch++; report.push(`  FAIL  ${name.padEnd(26)} 0x${va.toString(16)}  ${bad.length}/${want.length} differ, first at [${bad[0]}]: crate ${want[bad[0]]} binary ${got[bad[0]]}  (${rel})`); }
  }
}
console.log(`crate files scanned: ${files.length}`);
console.log(report.sort().join('\n'));
console.log(`\n${ok} pass, ${mismatch} fail, ${checked} compared; ${skipped} tables had no usable address or were not plain numbers`);
console.log(`  no address in doc comment: ${noaddr.join(", ")}`);
console.log(`  not plain numeric literals: ${nonnum.join(", ")}`);
