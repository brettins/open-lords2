// prose.js - find the phrases prose uses to argue with something that is not there.
//
//   node tools/review/prose.js                 # per-file counts, worst first
//   node tools/review/prose.js docs/kingdom.md # every offending line in one file
//   node tools/review/prose.js --phrases       # the list
//
// Scans docs/*.md, CLAUDE.md, README.md and the // comment lines of crates/**/*.rs.
// docs/decisions.md is skipped: it is history, read by C-number. A hit is a lead
// for a rewrite, not an error; keep every number, address, path and [V]/[I]/[D] mark.
const fs = require("fs"), path = require("path"), cp = require("child_process");
const PHRASES = ["rather than","is not a","there is no","actually","which is why","exactly as",
  "the real","simply","was never","merely","only ever","without a","genuinely","precisely",
  "it is worth","not just","note that","not because","that said","nothing to do with",
  "is in no","does not mean","this only","never was","in fact","in no way","importantly",
  "not a skin","not a target","not a manual","to be clear","of course","crucially","needless to say"];
const RX = new RegExp("\\b(" + PHRASES.map(p => p.replace(/ /g, "\\s+")).join("|") + ")\\b", "i");
const args = process.argv.slice(2);
if (args[0] === "--phrases") { console.log(PHRASES.join("\n")); process.exit(0); }
const root = path.resolve(__dirname, "..", "..");
const files = args.length ? args : cp.execSync("git ls-files docs/*.md CLAUDE.md README.md crates/**/*.rs", { cwd: root })
  .toString().split(/\r?\n/).filter(f => f && !f.endsWith("docs/decisions.md"));
const rows = [];
for (const f of files) {
  const rs = f.endsWith(".rs"), lines = fs.readFileSync(path.join(root, f), "utf8").split(/\r?\n/);
  const hits = [];
  lines.forEach((l, i) => { if ((!rs || /^\s*\/\//.test(l)) && RX.test(l)) hits.push([i + 1, l.trim()]); });
  if (hits.length) rows.push([f, hits]);
}
rows.sort((a, b) => b[1].length - a[1].length);
if (args.length) for (const [, hits] of rows) for (const [n, l] of hits) console.log(`${n}: ${l}`);
else { for (const [f, hits] of rows) console.log(String(hits.length).padStart(5), f); }
console.log(`prose: ${rows.reduce((s, r) => s + r[1].length, 0)} hit(s) in ${rows.length} file(s)`);
