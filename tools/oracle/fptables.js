// Find every function-pointer table in Lords2.exe, mechanically.
//
//   node tools/oracle/fptables.js            # all runs of >= 3 consecutive function pointers
//   node tools/oracle/fptables.js --min 4
//   node tools/oracle/fptables.js --unnamed  # only tables that contain unnamed entries
//
// # Why this exists
//
// A dispatch table is the strongest mechanical grouping the binary offers. It
// says *these functions are alternatives for one another*, which is exactly the
// constraint that makes a neighbour's name evidence for its siblings. The battle
// order handlers were found through one of these by hand; `g_netCmdWriters` was
// found, mis-sized, and then found to have a SECOND table butted against it that
// nobody had noticed (docs/agents.md). Both of those are the same fact: nobody
// had enumerated the tables.
//
// # What it claims and does not claim
//
// A run of consecutive dwords, all of which are the entry address of a function
// Ghidra found, is a fact about the bytes. It is NOT proof of a dispatch table:
// three unrelated pointers can sit side by side, and the boundaries of a real
// table are decided by its READER, not by where the run of valid pointers stops
// - which is precisely how g_netCmdWriters came to be documented as 112 entries
// when it is 100. So the run length printed here is an UPPER BOUND on a table
// and a LOWER BOUND on nothing. Read the reader before believing an extent.
//
// The corpus is only used for the function-start set and the current names.

const fs = require("fs");
const path = require("path");

function findCorpus(dirname) {
  const i = process.argv.indexOf("--decomp");
  const explicit = i >= 0 ? process.argv[i + 1] : process.env.LORDS2_DECOMP;
  if (explicit) return explicit;
  const here = path.join(dirname, "decomp");
  if (fs.existsSync(here) && fs.readdirSync(here).length) return here;
  const { execSync } = require("child_process");
  try {
    const common = execSync("git rev-parse --git-common-dir", { encoding: "utf8" }).trim();
    const main = path.resolve(path.dirname(path.resolve(common)));
    const p = path.join(main, "tools", "oracle", "decomp");
    if (fs.existsSync(p) && fs.readdirSync(p).length) return p;
  } catch (e) {}
  throw new Error("no decompiled corpus; pass --decomp <dir>");
}

const corpus = findCorpus(__dirname);

// address -> name, from the corpus headers ("// ==== 00401000  FUN_00401000  params=3  bytes=133")
const funcs = new Map();
for (const f of fs.readdirSync(corpus).filter((f) => f.endsWith(".c"))) {
  const text = fs.readFileSync(path.join(corpus, f), "utf8");
  const re = /^\/\/ ==== ([0-9a-f]{8})\s+(\S+)\s+params=(\d+)\s+bytes=(\d+)/gm;
  let m;
  while ((m = re.exec(text))) {
    funcs.set(parseInt(m[1], 16), { name: m[2], bytes: +m[4], params: +m[3] });
  }
}

const exe = process.env.LORDS2_DIR
  ? path.join(process.env.LORDS2_DIR, "Lords2.exe")
  : "F:/games/Lords of the Realm II/Lords2.exe";
const b = fs.readFileSync(exe);
const pe = b.readUInt32LE(0x3c);
const nsec = b.readUInt16LE(pe + 6),
  optSize = b.readUInt16LE(pe + 20);
const opt = pe + 24;
const imageBase = b.readUInt32LE(opt + 28);
const secOff = opt + optSize;
const secs = [];
for (let i = 0; i < nsec; i++) {
  const o = secOff + i * 40;
  secs.push({
    name: b.toString("ascii", o, o + 8).replace(/\0+$/, ""),
    va: imageBase + b.readUInt32LE(o + 12),
    vs: b.readUInt32LE(o + 8),
    raw: b.readUInt32LE(o + 20),
    rs: b.readUInt32LE(o + 16),
  });
}
const secOf = (va) => secs.find((s) => va >= s.va && va < s.va + Math.max(s.vs, s.rs));

const MIN = (() => {
  const i = process.argv.indexOf("--min");
  return i >= 0 ? +process.argv[i + 1] : 3;
})();
const ONLY_UNNAMED = process.argv.includes("--unnamed");

const runs = [];
for (const s of secs) {
  if (!s.rs) continue;
  const end = Math.min(s.raw + s.rs, b.length);
  let run = null;
  for (let off = s.raw; off + 4 <= end; off += 4) {
    const va = b.readUInt32LE(off);
    const fn = funcs.get(va);
    if (fn) {
      const at = s.va + (off - s.raw);
      if (run && run.at + run.entries.length * 4 === at) run.entries.push(va);
      else {
        if (run && run.entries.length >= MIN) runs.push(run);
        run = { sec: s.name, at, entries: [va] };
      }
    } else {
      if (run && run.entries.length >= MIN) runs.push(run);
      run = null;
    }
  }
  if (run && run.entries.length >= MIN) runs.push(run);
}

let totalEntries = 0,
  totalUnnamed = 0;
const shown = [];
for (const r of runs) {
  const uniq = new Set(r.entries);
  const unnamed = [...uniq].filter((a) => funcs.get(a).name.startsWith("FUN_"));
  totalEntries += uniq.size;
  totalUnnamed += unnamed.length;
  if (ONLY_UNNAMED && !unnamed.length) continue;
  shown.push({ r, uniq, unnamed });
}

shown.sort((a, b) => b.unnamed.length - a.unnamed.length || b.r.entries.length - a.r.entries.length);
for (const { r, uniq, unnamed } of shown) {
  console.log(
    `\n0x${r.at.toString(16).padStart(8, "0")}  .${r.sec}  ${r.entries.length} slots, ` +
      `${uniq.size} distinct, ${unnamed.length} unnamed`
  );
  r.entries.forEach((a, i) => {
    const f = funcs.get(a);
    console.log(`   [${String(i).padStart(3)}] 0x${a.toString(16).padStart(8, "0")}  ${f.name.padEnd(34)} ${f.bytes}b`);
  });
}
console.log(
  `\n${runs.length} runs of >= ${MIN} consecutive function pointers; ` +
    `${totalEntries} distinct functions in them, ${totalUnnamed} of them unnamed.`
);
