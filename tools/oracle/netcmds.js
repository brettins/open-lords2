// The network command table, joined: opcode -> writer, handler, payload length, senders.
//
//   node tools/oracle/netcmds.js              # the whole table
//   node tools/oracle/netcmds.js 0x2e         # one opcode, with both bodies
//   node tools/oracle/netcmds.js --unnamed    # only opcodes whose halves are still FUN_
//
// # Why this exists
//
// `Net_SendCommand(op, 0)` is called from 107 functions with a LITERAL opcode at
// almost every site, and every campaign action in the binary is written twice -
// once as a direct call and once as a `Net_SendCommand` under `g_multiplayer`.
// So a call site NAMES its opcode, exactly the way `Eng_DrawString(group, index)`
// names its subject and `Log_Write` names its function.
//
// Opcode `i` then has two functions of its own: `g_netCmdWriters[i]` serialises
// the payload and `g_netCmdHandlers[i]` applies it on the receiving peer. Those
// 194 functions were the single largest unnamed block in the binary.
//
// The join is what makes it evidence rather than a guess, because there are two
// independent constraints on every pair and they have to agree:
//
//   * the SENDER - which named function calls `Net_SendCommand` with this opcode;
//   * the HANDLER BODY - which already-named game function it calls to do the work.
//
// A pair where those two disagree is a pair to leave unnamed. Several do.
//
// # What it does not claim
//
// The table extent is read as 100 entries because `g_netCmdLength` is 100 bytes
// and `Net_SendCommand` indexes all three by the same `op`. A previous reading
// took 112 and the extra twelve were the head of the handler table butted against
// the writer table - see docs/agents.md. This tool prints where each table starts
// and stops so that mistake is visible rather than assumed.

const fs = require("fs");
const path = require("path");

function findCorpus(dirname) {
  const i = process.argv.indexOf("--decomp");
  const explicit = i >= 0 ? process.argv[i + 1] : process.env.LORDS2_DECOMP;
  if (explicit) return explicit;
  const here = path.join(dirname, "decomp");
  if (fs.existsSync(here) && fs.readdirSync(here).length) return here;
  const { execSync } = require("child_process");
  const common = execSync("git rev-parse --git-common-dir", { encoding: "utf8" }).trim();
  const main = path.resolve(path.dirname(path.resolve(common)));
  const p = path.join(main, "tools", "oracle", "decomp");
  if (fs.existsSync(p) && fs.readdirSync(p).length) return p;
  throw new Error("no decompiled corpus; pass --decomp <dir>");
}

const corpus = findCorpus(__dirname);
const files = fs.readdirSync(corpus).filter((f) => f.endsWith(".c"));

// Split every corpus file into functions: {addr, name, bytes, body}
const byAddr = new Map();
const all = [];
for (const f of files) {
  const text = fs.readFileSync(path.join(corpus, f), "utf8");
  const re = /^\/\/ ==== ([0-9a-f]{8})\s+(\S+)\s+params=(\d+)\s+bytes=(\d+)/gm;
  const marks = [];
  let m;
  while ((m = re.exec(text))) marks.push({ i: m.index, addr: parseInt(m[1], 16), name: m[2], bytes: +m[4] });
  marks.forEach((mk, k) => {
    mk.body = text.slice(mk.i, k + 1 < marks.length ? marks[k + 1].i : text.length);
    byAddr.set(mk.addr, mk);
    all.push(mk);
  });
}

// Read the three tables straight out of the executable.
const exe = process.env.LORDS2_DIR
  ? path.join(process.env.LORDS2_DIR, "Lords2.exe")
  : "F:/games/Lords of the Realm II/Lords2.exe";
const b = fs.readFileSync(exe);
const pe = b.readUInt32LE(0x3c);
const nsec = b.readUInt16LE(pe + 6), optSize = b.readUInt16LE(pe + 20);
const opt = pe + 24, imageBase = b.readUInt32LE(opt + 28), secOff = opt + optSize;
const secs = [];
for (let i = 0; i < nsec; i++) {
  const o = secOff + i * 40;
  secs.push({ va: imageBase + b.readUInt32LE(o + 12), vs: b.readUInt32LE(o + 8), raw: b.readUInt32LE(o + 20), rs: b.readUInt32LE(o + 16) });
}
const off = (va) => { const s = secs.find((s) => va >= s.va && va < s.va + Math.max(s.vs, s.rs)); return s.raw + (va - s.va); };

const N = 100;
const WRITERS = 0x004d57f0, HANDLERS = 0x004d5980, LENGTHS = 0x004d5b90;
const writers = [], handlers = [], lengths = [];
for (let i = 0; i < N; i++) {
  writers.push(b.readUInt32LE(off(WRITERS) + i * 4));
  handlers.push(b.readUInt32LE(off(HANDLERS) + i * 4));
  lengths.push(b.readUInt8(off(LENGTHS) + i));
}

// Every literal Net_SendCommand(op, ...) call site, attributed to its function.
const senders = new Map(); // op -> Map(funcName -> count)
for (const fn of all) {
  const re = /Net_SendCommand\(\s*(0x[0-9a-fA-F]+|\d+)\s*,/g;
  let m;
  while ((m = re.exec(fn.body))) {
    // skip citations inside the plate comment
    const upto = fn.body.slice(0, m.index);
    const lastOpen = upto.lastIndexOf("/*"), lastClose = upto.lastIndexOf("*/");
    if (lastOpen > lastClose) continue;
    const op = Number(m[1]);
    if (!senders.has(op)) senders.set(op, new Map());
    const s = senders.get(op);
    s.set(fn.name, (s.get(fn.name) || 0) + 1);
  }
}

const nm = (a) => (byAddr.get(a) ? byAddr.get(a).name : "<no function>");
const sz = (a) => (byAddr.get(a) ? byAddr.get(a).bytes : 0);

const arg = process.argv[2];
if (arg && /^(0x)?[0-9a-fA-F]+$/.test(arg)) {
  const op = Number(arg.startsWith("0x") ? arg : "0x" + arg);
  console.log(`opcode 0x${op.toString(16)}  payload ${lengths[op] === 0xff ? "INVALID" : lengths[op] + " bytes"}`);
  console.log(`senders: ${[...(senders.get(op) || new Map()).keys()].join(", ") || "(none)"}`);
  console.log(`\n---- writer 0x${writers[op].toString(16)} ----\n${byAddr.get(writers[op])?.body || ""}`);
  console.log(`\n---- handler 0x${handlers[op].toString(16)} ----\n${byAddr.get(handlers[op])?.body || ""}`);
  process.exit(0);
}

const onlyUnnamed = process.argv.includes("--unnamed");
let unnamedHalves = 0, live = 0;
console.log(`op   len  writer                                   handler                                  senders`);
for (let i = 0; i < N; i++) {
  const dead = lengths[i] === 0xff;
  if (!dead) live++;
  const w = nm(writers[i]), h = nm(handlers[i]);
  if (w.startsWith("FUN_")) unnamedHalves++;
  if (h.startsWith("FUN_")) unnamedHalves++;
  if (onlyUnnamed && !w.startsWith("FUN_") && !h.startsWith("FUN_")) continue;
  const send = [...(senders.get(i) || new Map()).keys()].join(" ");
  console.log(
    `0x${i.toString(16).padStart(2, "0")} ${String(dead ? "--" : lengths[i]).padStart(4)}  ` +
      `${(w + " " + sz(writers[i]) + "b").padEnd(40)} ${(h + " " + sz(handlers[i]) + "b").padEnd(40)} ${send}`
  );
}
console.log(`\n${live} live opcodes of ${N}; ${unnamedHalves} of ${2 * N} table halves still unnamed.`);
console.log(`${[...senders.keys()].length} distinct opcodes appear at a literal call site.`);
