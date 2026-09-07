// Find the entry point of the function that contains a given address, without
// Ghidra.
//
//   node tools/net/enclosing.js "F:/games/Lords of the Realm II/Lords2.exe" 4b7fce 4b826e
//
// Method: every `call rel32` in .text names a function entry, so the set of
// call targets is a (partial) list of function starts. The enclosing function
// of X is then the greatest call target <= X. Cross-checked two ways:
//   * the candidate should begin with a recognisable prologue;
//   * the byte before it should be a terminator (ret / jmp / int3 padding),
//     because MSVC lays functions out back to back with 0xCC filler.
// Both checks are printed so a wrong answer is visible rather than silent.
//
// This is a heuristic. It cannot see functions that are only ever reached
// indirectly, and it will merge a function with a preceding one that is never
// called directly. Treat the result as a lead, not a fact.
const fs = require('fs');
const file = process.argv[2];
const targets = process.argv.slice(3).map(s => parseInt(s, 16));
if (!file || !targets.length) {
  console.log('usage: node enclosing.js <exe> <hex addr> [<hex addr> ...]');
  process.exit(1);
}
const b = fs.readFileSync(file);
const pe = b.readUInt32LE(0x3c);
const nsec = b.readUInt16LE(pe + 6), optSize = b.readUInt16LE(pe + 20), opt = pe + 24;
const imageBase = b.readUInt32LE(opt + 28);
const secOff = opt + optSize;
let text = null;
for (let i = 0; i < nsec; i++) {
  const o = secOff + i * 40;
  const name = b.toString('ascii', o, o + 8).replace(/\0+$/, '');
  if (name === '.text') text = { va: b.readUInt32LE(o + 12), raw: b.readUInt32LE(o + 20), rs: b.readUInt32LE(o + 16) };
}
const v2o = v => text.raw + (v - imageBase - text.va);
const o2v = o => imageBase + text.va + (o - text.raw);
const hex = n => '0x' + (n >>> 0).toString(16).padStart(8, '0');

const callTargets = new Set();
for (let o = text.raw; o < text.raw + text.rs - 5; o++) {
  if (b[o] !== 0xe8) continue;
  const t = o2v(o) + 5 + b.readInt32LE(o + 1);
  if (t >= imageBase + text.va && t < imageBase + text.va + text.rs) callTargets.add(t);
}
const sorted = [...callTargets].sort((a, b2) => a - b2);
console.log(`${sorted.length} distinct direct-call targets in .text`);

// Prologues MSVC actually emits at this vintage.
function prologue(o) {
  if (b[o] === 0x55 && b[o + 1] === 0x8b && b[o + 2] === 0xec) return 'push ebp; mov ebp,esp';
  if (b[o] === 0x55 && b[o + 1] === 0x89 && b[o + 2] === 0xe5) return 'push ebp; mov ebp,esp (gas)';
  if (b[o] === 0x83 && b[o + 1] === 0xec) return 'sub esp,' + b[o + 2];
  if (b[o] === 0x81 && b[o + 1] === 0xec) return 'sub esp,' + b.readUInt32LE(o + 2);
  if (b[o] === 0x53 || b[o] === 0x56 || b[o] === 0x57) return 'push (callee-saved)';
  if (b[o] === 0xa1) return 'mov eax,[abs]';
  return null;
}
function terminatorBefore(o) {
  const p = b[o - 1];
  if (p === 0xcc) return 'int3 padding';
  if (p === 0xc3) return 'ret';
  if (p === 0x90) return 'nop';
  if (b[o - 3] === 0xc2) return 'ret imm16';
  return 'none (0x' + p.toString(16) + ')';
}

for (const t of targets) {
  const va = t < imageBase ? t + imageBase : t;
  let best = -1;
  for (const s of sorted) { if (s <= va) best = s; else break; }
  const idx = sorted.indexOf(best);
  const next = idx + 1 < sorted.length ? sorted[idx + 1] : 0;
  const o = v2o(best);
  console.log(`\naddress ${hex(va)}`);
  console.log(`  enclosing function entry : ${hex(best)}   (spans to at least ${hex(next)})`);
  console.log(`  prologue                 : ${prologue(o) || 'UNRECOGNISED'}`);
  console.log(`  byte before entry        : ${terminatorBefore(o)}`);
  console.log(`  first bytes              : ${[...b.slice(o, o + 16)].map(x => x.toString(16).padStart(2, '0')).join(' ')}`);
  // Who calls it, and how many times: a function called from exactly one place
  // is easier to reason about than one called from twenty.
  const callers = [];
  for (let p = text.raw; p < text.raw + text.rs - 5; p++) {
    if (b[p] !== 0xe8) continue;
    if (o2v(p) + 5 + b.readInt32LE(p + 1) === best) callers.push(o2v(p));
  }
  console.log(`  direct callers           : ${callers.length ? callers.map(hex).join(' ') : '(none - reached indirectly?)'}`);
}
