// Which Smacker API does Lords2.exe actually use?
//
//   node tools/media/smkapi.js <exe> <dll> [dllNameFilter]
//
// The exe imports smackw32.dll purely by ordinal, so the import table alone says
// nothing. This joins the exe's ordinal imports against the DLL's export table
// (which does carry names) and counts call sites for each, giving the exact API
// surface our engine has to reproduce.
//
//   node tools/media/smkapi.js "F:/games/Lords of the Realm II/Lords2.exe" \
//                              "F:/games/Lords of the Realm II/Smackw32.dll" smack

const fs = require('fs');

function pe(file) {
  const b = fs.readFileSync(file);
  const h = b.readUInt32LE(0x3c);
  if (b.toString('ascii', h, h + 4) !== 'PE\0\0') throw new Error(file + ': not a PE');
  const nsec = b.readUInt16LE(h + 6), optSize = b.readUInt16LE(h + 20), opt = h + 24;
  const magic = b.readUInt16LE(opt);
  const dd = opt + (magic === 0x10b ? 96 : 112);
  const secOff = opt + optSize;
  const secs = [];
  for (let i = 0; i < nsec; i++) {
    const o = secOff + i * 40;
    secs.push({
      name: b.toString('ascii', o, o + 8).replace(/\0+$/, ''),
      va: b.readUInt32LE(o + 12), vs: b.readUInt32LE(o + 8),
      raw: b.readUInt32LE(o + 20), rs: b.readUInt32LE(o + 16),
    });
  }
  const r2o = (r) => {
    for (const s of secs) if (r >= s.va && r < s.va + Math.max(s.vs, s.rs)) return s.raw + (r - s.va);
    return -1;
  };
  const cstr = (o) => { let e = o; while (e < b.length && b[e]) e++; return b.toString('ascii', o, e); };
  return { b, dd, secs, r2o, cstr, base: b.readUInt32LE(opt + 28) };
}

function exportsByOrdinal(file) {
  const { b, dd, r2o, cstr } = pe(file);
  const rva = b.readUInt32LE(dd);
  if (!rva) return {};
  const e = r2o(rva);
  const ordBase = b.readUInt32LE(e + 16), nName = b.readUInt32LE(e + 24);
  const aName = r2o(b.readUInt32LE(e + 32)), aOrd = r2o(b.readUInt32LE(e + 36));
  const map = {};
  for (let i = 0; i < nName; i++) {
    map[b.readUInt16LE(aOrd + i * 2) + ordBase] = cstr(r2o(b.readUInt32LE(aName + i * 4)));
  }
  return map;
}

// exe -> { 'dll!#ord' or 'dll!name': { dll, ordinal, name, slotVA } }
function imports(file) {
  const { b, dd, r2o, cstr, base } = pe(file);
  const out = [];
  let o = r2o(b.readUInt32LE(dd + 8));
  for (let i = 0; ; i++) {
    const rec = o + i * 20;
    const nr = b.readUInt32LE(rec + 12);
    if (!nr) break;
    const dll = cstr(r2o(nr));
    const oft = b.readUInt32LE(rec), ft = b.readUInt32LE(rec + 16);
    const thunk = r2o(oft || ft);
    for (let j = 0; ; j++) {
      const v = b.readUInt32LE(thunk + j * 4);
      if (!v) break;
      out.push({
        dll,
        ordinal: (v & 0x80000000) ? (v & 0xffff) : null,
        name: (v & 0x80000000) ? null : cstr(r2o(v) + 2),
        slotVA: (base + ft + j * 4) >>> 0,
      });
    }
  }
  return out;
}

function callSites(file, slots) {
  const { b, secs, base } = pe(file);
  const text = secs.find((s) => s.name === '.text');
  const H = (v) => '0x' + (v >>> 0).toString(16).padStart(8, '0');
  const calls = {}, thunks = {};
  for (let p = text.raw; p < text.raw + text.rs - 6; p++) {
    if (b[p] === 0xff && (b[p + 1] === 0x15 || b[p + 1] === 0x25)) {
      const tgt = b.readUInt32LE(p + 2);
      if (!slots[tgt]) continue;
      const va = base + text.va + (p - text.raw);
      if (b[p + 1] === 0x25) thunks[va] = slots[tgt];
      else (calls[slots[tgt]] = calls[slots[tgt]] || []).push(H(va));
    }
  }
  for (let p = text.raw; p < text.raw + text.rs - 5; p++) {
    if (b[p] !== 0xe8) continue;
    const va = base + text.va + (p - text.raw);
    const tgt = (va + 5 + b.readInt32LE(p + 1)) >>> 0;
    if (thunks[tgt]) (calls[thunks[tgt]] = calls[thunks[tgt]] || []).push(H(va));
  }
  return calls;
}

const [exe, dll, filter = ''] = process.argv.slice(2);
if (!exe || !dll) {
  console.error('usage: node smkapi.js <exe> <dll> [dllNameFilter]');
  process.exit(2);
}
const exp = exportsByOrdinal(dll);
const imp = imports(exe).filter((i) => !filter || new RegExp(filter, 'i').test(i.dll));
const slots = {};
for (const i of imp) slots[i.slotVA] = i.ordinal !== null ? '#' + i.ordinal : i.name;
const calls = callSites(exe, slots);

console.log(exe);
console.log('imports ' + imp.length + ' symbol(s)' + (filter ? ' from DLLs matching /' + filter + '/' : '') +
  ', ' + imp.filter((i) => i.ordinal !== null).length + ' by ordinal, ' +
  imp.filter((i) => i.name !== null).length + ' by name\n');
console.log('ord  export name                    sites  call sites');
for (const i of imp.sort((a, b) => (a.ordinal || 0) - (b.ordinal || 0))) {
  const key = i.ordinal !== null ? '#' + i.ordinal : i.name;
  const sites = calls[key] || [];
  console.log(
    String(i.ordinal === null ? '-' : i.ordinal).padStart(3) + '  ' +
    (i.ordinal !== null ? (exp[i.ordinal] || '(not exported by name)') : i.name).padEnd(30) +
    String(sites.length).padStart(5) + '  ' + sites.join(' '));
}
const unused = Object.entries(exp)
  .filter(([o]) => !imp.some((i) => String(i.ordinal) === o))
  .map(([o, n]) => o + ':' + n);
console.log('\nexported but not imported (' + unused.length + '): ' + unused.join(' '));
