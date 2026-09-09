// petable.js - read a table out of Lords2.exe by virtual address.
//
//   node tools/battle/petable.js <exe> <VA hex> <count> [width=4]
//
// Why this exists: the dispatch tables are the only anchor in the battle
// subsystem that cannot lie. g_troopTickTable's slot 2 IS the maceman handler,
// whatever the function looks like, because the game indexes it by troop type.
// docs/decisions.md C3 is what happens when you reason the other way round.
//
// Lords2.exe has no ASLR and a fixed image base of 0x400000, so a virtual
// address is a constant. This maps it through the PE section headers and reads
// the bytes off disk: no process, no window, no focus stolen. docs/battle.md
// 8.2a establishes that disk and live memory agree for the battle tables.
//
// When a u32 entry is the address of a function the corpus knows about, the
// name is printed beside it - so a table of function pointers resolves itself.
//
//   node tools/battle/petable.js "F:/games/Lords of the Realm II/Lords2.exe" 4d9140 12
//   node tools/battle/petable.js "F:/games/Lords of the Realm II/Lords2.exe" 4d9170 18
//   node tools/battle/petable.js "F:/games/Lords of the Realm II/Lords2.exe" 4d98f8 44 2
//
// Node needs forward slashes on Windows; see docs/environment.md.
const fs = require('fs');
const path = require('path');

const [exe, vaArg, countArg, widthArg] = process.argv.slice(2);
if (!exe || !vaArg || !countArg) {
  console.log(fs.readFileSync(__filename, 'utf8').split('\n').slice(0, 21).join('\n'));
  process.exit(1);
}
const va = parseInt(vaArg, 16);
const count = parseInt(countArg, 10);
const width = parseInt(widthArg || '4', 10);

const b = fs.readFileSync(exe);
const pe = b.readUInt32LE(0x3c), opt = pe + 24;
const imageBase = b.readUInt32LE(opt + 28);
const nsec = b.readUInt16LE(pe + 6), secOff = opt + b.readUInt16LE(pe + 20);

function fileOffset(virtual) {
  const rva = virtual - imageBase;
  for (let i = 0; i < nsec; i++) {
    const o = secOff + i * 40;
    const sva = b.readUInt32LE(o + 12), vsz = b.readUInt32LE(o + 8);
    const raw = b.readUInt32LE(o + 20), rsz = b.readUInt32LE(o + 16);
    if (rva >= sva && rva < sva + Math.max(vsz, rsz)) return raw + (rva - sva);
  }
  return -1;
}

// Resolve function pointers against the decompiled corpus index when it exists.
// tools/oracle/decomp/ is gitignored; without it the addresses still print.
const names = {};
const index = path.join(__dirname, '..', 'oracle', 'decomp', 'index.txt');
if (fs.existsSync(index)) {
  for (const line of fs.readFileSync(index, 'utf8').split('\n')) {
    if (!line.trim() || line.startsWith('#')) continue;
    const p = line.trim().split(/\s+/);
    names[parseInt(p[0], 16)] = p[1];
  }
}

const base = fileOffset(va);
if (base < 0) { console.error('VA 0x' + va.toString(16) + ' is not in any section'); process.exit(1); }

for (let i = 0; i < count; i++) {
  const o = base + i * width;
  const v = width === 1 ? b.readUInt8(o) : width === 2 ? b.readUInt16LE(o) : b.readUInt32LE(o);
  console.log(
    String(i).padStart(3),
    '0x' + (va + i * width).toString(16),
    '=', String(v).padStart(10),
    '0x' + v.toString(16).padStart(width * 2, '0'),
    names[v] || '');
}
