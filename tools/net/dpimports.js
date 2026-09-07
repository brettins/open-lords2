// Dump the DPLAYX.dll import thunks of a PE32 image: ordinal or name, the IAT
// slot address the loader patches, and the call sites that use it.
//
//   node tools/net/dpimports.js "F:/games/Lords of the Realm II/Lords2.exe" [DLLNAME]
//
// Verifies the claim in docs/symbols.md that the game's entire network entry
// surface is two ordinal imports.
const fs = require('fs');
const path = process.argv[2];
const want = (process.argv[3] || 'DPLAYX.dll').toLowerCase();
const b = fs.readFileSync(path);
const pe = b.readUInt32LE(0x3c);
if (b.toString('ascii', pe, pe + 4) !== 'PE\0\0') { console.log('not PE'); process.exit(1); }
const nsec = b.readUInt16LE(pe + 6), optSize = b.readUInt16LE(pe + 20);
const opt = pe + 24, magic = b.readUInt16LE(opt);
if (magic !== 0x10b) { console.log('not PE32'); process.exit(1); }
const imageBase = b.readUInt32LE(opt + 28);
const ddOff = opt + 96;
const secOff = opt + optSize;
const secs = [];
for (let i = 0; i < nsec; i++) {
  const o = secOff + i * 40;
  secs.push({ name: b.toString('ascii', o, o + 8).replace(/\0+$/, ''), va: b.readUInt32LE(o + 12), vs: b.readUInt32LE(o + 8), raw: b.readUInt32LE(o + 20), rs: b.readUInt32LE(o + 16) });
}
const r2o = r => { for (const s of secs) if (r >= s.va && r < s.va + Math.max(s.vs, s.rs)) return s.raw + (r - s.va); return -1; };
const cstr = o => { let e = o; while (e < b.length && b[e]) e++; return b.toString('ascii', o, e); };
const hex = n => '0x' + n.toString(16).padStart(8, '0');

const impRva = b.readUInt32LE(ddOff + 8);
let descOff = r2o(impRva);
const iatSlots = [];
for (let i = 0; ; i++) {
  const rec = descOff + i * 20;
  const origThunk = b.readUInt32LE(rec + 0);
  const nameRva = b.readUInt32LE(rec + 12);
  const firstThunk = b.readUInt32LE(rec + 16);
  if (nameRva === 0) break;
  const dll = cstr(r2o(nameRva));
  if (dll.toLowerCase() !== want) continue;
  console.log(`${dll}  ILT=${hex(origThunk)} IAT=${hex(firstThunk)}`);
  const iltOff = r2o(origThunk || firstThunk);
  for (let k = 0; ; k++) {
    const t = b.readUInt32LE(iltOff + k * 4);
    if (t === 0) break;
    const slotVa = imageBase + firstThunk + k * 4;
    let what;
    if (t & 0x80000000) what = `ordinal ${t & 0xffff}`;
    else { const h = r2o(t); what = `name "${cstr(h + 2)}" hint ${b.readUInt16LE(h)}`; }
    console.log(`  IAT slot ${hex(slotVa)}  ${what}`);
    iatSlots.push({ slotVa, what });
  }
}
if (!iatSlots.length) { console.log(`no imports from ${want}`); process.exit(0); }

// Find every "call dword ptr [slot]" -> FF 15 <le32 slot>, and "jmp" FF 25.
console.log('\ncall sites (FF 15 / FF 25 absolute indirect):');
const text = secs.find(s => s.name === '.text');
for (let o = text.raw; o < text.raw + text.rs - 6; o++) {
  if (b[o] !== 0xff) continue;
  const mod = b[o + 1];
  if (mod !== 0x15 && mod !== 0x25) continue;
  const target = b.readUInt32LE(o + 2);
  const hit = iatSlots.find(s => s.slotVa === target);
  if (!hit) continue;
  const va = imageBase + text.va + (o - text.raw);
  console.log(`  ${hex(va)}  ${mod === 0x15 ? 'call' : 'jmp '} [${hex(target)}]  -> ${hit.what}`);
}
