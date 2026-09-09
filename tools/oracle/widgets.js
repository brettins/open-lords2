// widgets.js - read the interface's tables straight out of Lords2.exe.
//
// Why this exists. The interface is data: 24-byte widget records with a
// function pointer at +8, 12-byte menu records with another. Those pointers
// are never mentioned in code, so xref.js cannot see them and a reading that
// starts from a painter never reaches the handler. Decoding the table names
// the handler, gives its screen position, and says which of a pair it is -
// all from bytes nobody wrote a story about.
//
//   node tools/oracle/widgets.js widgets 4ddc10 4        one widget table
//   node tools/oracle/widgets.js menu 4dc428 3           the menu bar and its drop-downs
//   node tools/oracle/widgets.js dwords 4dc360 96 12     raw, naming any function pointer
//   node tools/oracle/widgets.js ref 434d33 43496c       who *points at* these functions
//
// The exe is found via $LORDS2_DIR, or --exe <path>. Function names come from
// tools/oracle/decomp/index.txt, so run decompile-all.ps1 first for them.
//
// Layouts, both read out of Widget_Draw / Widget_Test / Ui_DrawMenuTitles:
//
//   widget, 24 bytes   +0 x  +2 y  +4 frame  +6 size  +8 handler
//                      +0C state  +0D press timer  +0F kind  +10 hotspot id  +14 hotspot arg
//   menu bar, 16 bytes +0 x  +2 measuredX (written back at draw time)  +4 y
//                      +6 engGroup  +8 items  +0C count
//   menu item, 12 byte +0 y  +2 engIndex  +4 handler  +8 0
//
// Conventions worth knowing when you read the output: button-sheet frames
// 29/31 are tick and cross, 35/37 a scroll pair, **68 plus and 66 minus**,
// 21/23 up and down; and hotspot id 1 means confirm, 0 means cancel.
//
// That pair used to be written here the other way round, and the error left
// this file and got into docs/hypotheses.json as two names: 0x0043593A was
// filed as Armoury_BuyLess and 0x004359BC as Armoury_BuyMore, and both bodies
// say the opposite. It is settled by the only *other* table that uses the pair
// - the diplomacy gift row at 0x004DD9D0, whose frame-68 record carries hotspot
// id 1 and whose handler FUN_00436372 reads `if (id == 1) g_diploGold += 10;`.
// A convention note in a hypothesis generator is a hypothesis too; this one is
// now anchored to a body. docs/decisions.md C61, and C3 for the general case.

const fs = require('fs');
const path = require('path');

function opt(name, dflt) {
  const i = process.argv.indexOf('--' + name);
  return i > 0 && process.argv[i + 1] ? process.argv[i + 1] : dflt;
}
const EXE = opt('exe', path.join(process.env.LORDS2_DIR || 'F:/games/Lords of the Realm II', 'Lords2.exe'));
if (!fs.existsSync(EXE)) {
  console.error(`no Lords2.exe at ${EXE}\nset LORDS2_DIR, or pass --exe <path>`);
  process.exit(1);
}
const b = fs.readFileSync(EXE);

const pe = b.readUInt32LE(0x3c);
const nsec = b.readUInt16LE(pe + 6), optSize = b.readUInt16LE(pe + 20), optH = pe + 24;
const BASE = b.readUInt32LE(optH + 28), secOff = optH + optSize;
const secs = [];
for (let i = 0; i < nsec; i++) {
  const o = secOff + i * 40;
  secs.push({
    name: b.toString('ascii', o, o + 8).replace(/\0+$/, ''),
    va: b.readUInt32LE(o + 12), vs: b.readUInt32LE(o + 8),
    raw: b.readUInt32LE(o + 20), rs: b.readUInt32LE(o + 16),
  });
}
const va2off = va => {
  const r = va - BASE;
  for (const s of secs) if (r >= s.va && r < s.va + Math.max(s.vs, s.rs)) return s.raw + (r - s.va);
  return -1;
};
const off2va = o => { for (const s of secs) if (o >= s.raw && o < s.raw + s.rs) return BASE + s.va + (o - s.raw); return -1; };
const secOf = o => { for (const s of secs) if (o >= s.raw && o < s.raw + s.rs) return s.name; return '?'; };

const names = new Map();
const idx = path.join(__dirname, 'decomp', 'index.txt');
if (fs.existsSync(idx)) {
  for (const line of fs.readFileSync(idx, 'utf8').split(/\r?\n/)) {
    const m = /^([0-9a-f]{8})\s+(\S+)/.exec(line);
    if (m) names.set(parseInt(m[1], 16), m[2]);
  }
}
const nm = a => names.get(a) || ('0x' + a.toString(16).padStart(8, '0'));
const hex = a => (typeof a === 'string' ? parseInt(a.replace(/^0x/i, ''), 16) : a);

function needOffset(va) {
  const o = va2off(va);
  if (o < 0) { console.error(`0x${va.toString(16)} is not in any section`); process.exit(1); }
  return o;
}

function cmdWidgets(args) {
  for (let k = 0; k < args.length; k += 2) {
    const va = hex(args[k]), count = +args[k + 1];
    const o = needOffset(va);
    console.log(`--- widgets 0x${va.toString(16)}  ${count} records ---`);
    console.log('   #    x    y  frame size  handler                          kind  hotspot  arg');
    for (let i = 0; i < count; i++) {
      const r = o + i * 24;
      const p = (v, w) => String(v).padStart(w);
      console.log('  ' + p(i, 2) + p(b.readInt16LE(r), 5) + p(b.readInt16LE(r + 2), 5) +
        p(b.readInt16LE(r + 4), 7) + p(b.readInt16LE(r + 6), 5) + '  ' +
        nm(b.readUInt32LE(r + 8)).padEnd(32) + p(b[r + 15], 4) + p(b.readInt32LE(r + 16), 9) +
        p(b.readInt32LE(r + 20), 5));
    }
  }
}

function cmdMenu(args) {
  const va = hex(args[0]), count = +(args[1] || 3);
  const o = needOffset(va);
  for (let i = 0; i < count; i++) {
    const r = o + i * 16;
    const items = b.readUInt32LE(r + 8), n = b.readUInt32LE(r + 12);
    console.log(`--- menu 0x${(va + i * 16).toString(16)}  x=${b.readInt16LE(r)} ` +
      `measuredX=${b.readInt16LE(r + 2)} y=${b.readInt16LE(r + 4)} ` +
      `engGroup=${b.readInt16LE(r + 6)} items=0x${items.toString(16)} count=${n} ---`);
    const io = va2off(items);
    if (io < 0) { console.log('   (items pointer is not in any section)'); continue; }
    for (let j = 0; j < n; j++) {
      const q = io + j * 12;
      console.log(`   y=${String(b.readInt16LE(q)).padStart(4)} ` +
        `engIndex=${String(b.readInt16LE(q + 2)).padStart(3)}  ${nm(b.readUInt32LE(q + 4))}`);
    }
  }
}

function cmdDwords(args) {
  const va = hex(args[0]), len = +args[1], stride = +(args[2] || 16);
  const o = needOffset(va);
  for (let i = 0; i < len; i += stride) {
    const cells = [];
    for (let j = 0; j + 4 <= stride; j += 4) {
      const v = b.readUInt32LE(o + i + j);
      cells.push(names.has(v) ? names.get(v) : (v > BASE && v < BASE + 0x100000 ? '*' + v.toString(16) : String(v)));
    }
    console.log(`0x${(va + i).toString(16)}  ${cells.map(c => c.padStart(14)).join(' ')}`);
  }
}

function cmdRef(args) {
  for (const a of args) {
    const va = hex(a);
    const needle = Buffer.alloc(4); needle.writeUInt32LE(va);
    const hits = [];
    let i = 0;
    while ((i = b.indexOf(needle, i)) !== -1) { hits.push(i); i += 1; }
    console.log(`0x${va.toString(16)} ${names.has(va) ? '(' + names.get(va) + ')' : ''}: ` +
      `${hits.length} reference(s)  ` + hits.map(o => `${secOf(o)}@0x${off2va(o).toString(16)}`).join('  '));
  }
}

const [cmd, ...rest] = process.argv.slice(2).filter((a, i, all) =>
  a !== '--exe' && all[i - 1] !== '--exe');
switch (cmd) {
  case 'widgets': cmdWidgets(rest); break;
  case 'menu':    cmdMenu(rest); break;
  case 'dwords':  cmdDwords(rest); break;
  case 'ref':     cmdRef(rest); break;
  default:
    console.log(fs.readFileSync(__filename, 'utf8').split('\n')
      .filter(l => l.startsWith('//')).map(l => l.replace(/^\/\/ ?/, '')).join('\n'));
}
