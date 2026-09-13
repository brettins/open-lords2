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
const args = process.argv.slice(2);
const root = path.resolve(__dirname, "..", "..");
const PHRASES = ["rather than","is not a","there is no","actually","which is why","exactly as",
  "the real","simply","was never","merely","only ever","without a","genuinely","precisely",
  "it is worth","not just","note that","not because","that said","nothing to do with",
  "is in no","does not mean","this only","never was","in fact","in no way","importantly",
  "not a skin","not a target","not a manual","to be clear","of course","crucially","needless to say"];
const RX = new RegExp("\\b(" + PHRASES.map(p => p.replace(/ /g, "\\s+")).join("|") + ")\\b", "i");
if (args[0] === "--phrases") { console.log(PHRASES.join("\n")); process.exit(0); }
// --apply <file> <replacements.json> [--commit]: {"<line number>": "<new line text or empty to delete>"}
// written by an agent in one shot, applied here in one shot. Line numbers are from the
// listing run against the same file revision. Refuses if a target line no longer matches
// the listing regex (the file moved under it), and refuses any rewrite that is not a pure
// deletion: every word of the new line must appear in the old line, in order, except
// "not" / "instead of" (the "rather than" swap). A rewrite cannot add a fact.
// --commit then runs corrections.js --check (relocking if asked) and commits the file.
const KEEP = new Set(["not", "instead", "of"]);
const words = l => l.toLowerCase().match(/[a-z0-9_]+/g) || [];
const deletionOnly = (o, n) => { const a = words(o); let i = 0; for (const w of words(n)) { const j = a.indexOf(w, i); if (j < 0) { if (!KEEP.has(w)) return false; } else i = j + 1; } return true; };
if (args[0] === "--apply") {
  const [, file, repl] = args;
  const map = JSON.parse(fs.readFileSync(repl, "utf8"));
  const lines = fs.readFileSync(path.join(root, file), "utf8").split(/\r?\n/);
  const bad = Object.keys(map).filter(n => !RX.test(lines[n - 1] || ""));
  if (bad.length) { console.error("prose: lines no longer match the listing: " + bad.join(", ")); process.exit(1); }
  const added = Object.keys(map).filter(n => map[n] !== "" && !deletionOnly(lines[n - 1], map[n]));
  if (added.length) { console.error("prose: rewrites add words, refused: " + added.join(", ")); process.exit(1); }
  let dropped = 0;
  const out = lines.flatMap((l, i) => (String(i + 1) in map) ? (map[i + 1] === "" ? (dropped++, []) : [map[i + 1]]) : [l]);
  fs.writeFileSync(path.join(root, file), out.join("\n"));
  console.log(`prose: applied ${Object.keys(map).length} replacement(s), ${dropped} line(s) dropped, in ${file}`);
  if (args[3] === "--commit") {
    const run = (c, a) => cp.spawnSync(c, a, { cwd: root, encoding: "utf8", shell: true });
    let chk = run("node", ["tools/decisions/corrections.js", "--check"]);
    const paths = [file];
    if (chk.status && /--relock/.test(chk.stdout + chk.stderr)) { run("node", ["tools/decisions/corrections.js", "--relock"]); paths.push("tools/decisions/citations.lock"); chk = run("node", ["tools/decisions/corrections.js", "--check"]); }
    if (chk.status) { console.error("prose: corrections.js --check is red, not committed:\n" + (chk.stdout + chk.stderr).slice(-600)); process.exit(1); }
    const msg = path.join(require("os").tmpdir(), "prose-msg.txt");
    fs.writeFileSync(msg, `${path.basename(file)}: ${Object.keys(map).length} lines stop arguing\n`);
    const c = run("git", ["commit", "-q", "-F", msg, "--", ...paths]);
    if (c.status) { console.error("prose: commit failed:\n" + c.stderr); process.exit(1); }
    console.log("prose: committed " + run("git", ["rev-parse", "--short", "HEAD"]).stdout.trim());
  }
  process.exit(0);
}
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
