// reaches.js - can anything in Lords2.exe reach this function?
//
//   node tools/oracle/reaches.js 437107               one address
//   node tools/oracle/reaches.js 437002 438a91 4374c4 several, as a control
//
// The exe is found via $LORDS2_DIR, or --exe <path>.
//
// # Why this exists, and why the two tools we had were not enough
//
// `docs/arms.json`'s `dead` status says, in its own words, that the reason has
// to be "an exhaustive check rather than a failure to find". Two tools looked
// like they answered it and neither does on its own:
//
//   * xref.js reads the DECOMPILED CORPUS. It sees the calls the decompiler
//     recovered, in the functions the decompiler produced. That is a large
//     sample of the binary and it is not the binary.
//   * widgets.js `ref` scans the image for the address as a dword, which is how
//     a handler reaches a widget or hotspot table. It says nothing about calls.
//
// This does three scans over the WHOLE FILE and reports them separately:
//
//   E8 rel32   a direct call
//   E9 rel32   a direct jump (a tail call, and how a thunk reaches a body)
//   dword      the address stored anywhere - a table, a vtable, a callback
//
// Together those are the ways x86 reaches a function: a relative displacement
// in an instruction, or an absolute address in memory.
//
// # It over-approximates on purpose
//
// This is not a disassembler. It reports every offset whose bytes WOULD decode
// as a call or jump to the target if that offset happened to be an instruction
// boundary, so it will report coincidences inside data and inside the middle of
// other instructions.
//
// That is the useful direction. A false positive costs you a minute reading one
// address; a false negative would let you delete something the game calls. And
// it makes ZERO a proof: a real direct call MUST be those five bytes at some
// offset, so if no offset holds them, no direct call exists.
//
// # Run it on something you know is called, first
//
// A tool whose first run says "nothing found" is a tool nobody has watched
// work, and this one's whole output is an absence. Pass a few functions you
// already know are reached and read what comes back. When this was written the
// control was five, and one of them earned its place:
//
//   FUN_00437002   E8 at 0x430586      called from Screen_FrameInput's 0x04 arm
//   FUN_00438A91   E8 at 0x430574      the same arm
//   FUN_0043B412   E8 at 0x430D03      the 0x18 arm
//   FUN_0043B4CB   E8 at 0x4308E7      the 0x1A arm
//   FUN_004374C4   NO CALL AT ALL, and one dword at 0x4DC5B0
//
// The last is the point. It is the garrisoned unit panel's "leave the castle"
// button, it is perfectly live, and it is reached only by its address sitting in
// a hotspot table. A call scan alone would have called it dead.
//
// docs/decisions.md C94.

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
const nsec = b.readUInt16LE(pe + 6);
const optSize = b.readUInt16LE(pe + 20);
const optH = pe + 24;
const BASE = b.readUInt32LE(optH + 28);
const secOff = optH + optSize;
const secs = [];
for (let i = 0; i < nsec; i++) {
  const o = secOff + i * 40;
  secs.push({
    name: b.toString('ascii', o, o + 8).replace(/\0+$/, ''),
    va: b.readUInt32LE(o + 12),
    raw: b.readUInt32LE(o + 20),
    rsize: b.readUInt32LE(o + 16),
  });
}
const sectionOf = o => secs.find(s => o >= s.raw && o < s.raw + s.rsize);
const off2va = o => {
  const s = sectionOf(o);
  return s ? BASE + s.va + (o - s.raw) : 0;
};

// Function names, if the corpus has been built. `decompile-all.ps1` writes
// index.txt as `<addr>  <name>  <bytes>  <params>  <file>`.
const names = new Map();
const index = path.join(__dirname, 'decomp', 'index.txt');
if (fs.existsSync(index)) {
  for (const line of fs.readFileSync(index, 'utf8').split('\n')) {
    const m = line.match(/^([0-9a-f]{8})\s+(\S+)/);
    if (m) names.set(parseInt(m[1], 16) >>> 0, m[2]);
  }
}

const args = process.argv.slice(2).filter((a, i, all) => a !== '--exe' && all[i - 1] !== '--exe');
if (args.length === 0) {
  console.log(fs.readFileSync(__filename, 'utf8').split('\n')
    .filter(l => l.startsWith('//')).map(l => l.replace(/^\/\/ ?/, '')).join('\n'));
  process.exit(0);
}

for (const arg of args) {
  const va = parseInt(arg, 16) >>> 0;
  const call = [];
  const jmp = [];
  const abs = [];

  for (let o = 0; o + 5 <= b.length; o++) {
    const op = b[o];
    if (op !== 0xe8 && op !== 0xe9) continue;
    const next = off2va(o + 5);
    if (!next) continue;
    if (((next + b.readInt32LE(o + 1)) >>> 0) === va) (op === 0xe8 ? call : jmp).push(o);
  }
  const needle = Buffer.alloc(4);
  needle.writeUInt32LE(va);
  for (let i = 0; (i = b.indexOf(needle, i)) !== -1; i += 1) abs.push(i);

  const show = list =>
    list.length
      ? list.map(o => `${(sectionOf(o) || { name: '?' }).name}@0x${off2va(o).toString(16)}`).join('  ')
      : '(none)';
  const name = names.has(va) ? ` ${names.get(va)}` : '';
  const total = call.length + jmp.length + abs.length;
  console.log(`0x${va.toString(16)}${name}  ${total === 0 ? '*** UNREACHABLE ***' : ''}`);
  console.log(`  E8 call rel32 : ${String(call.length).padStart(3)}  ${show(call)}`);
  console.log(`  E9 jmp  rel32 : ${String(jmp.length).padStart(3)}  ${show(jmp)}`);
  console.log(`  absolute dword: ${String(abs.length).padStart(3)}  ${show(abs)}`);
}
