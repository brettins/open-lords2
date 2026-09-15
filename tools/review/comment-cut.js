// comment-cut.js - a Rust comment keeps its evidence and loses its prose.
//
//   node tools/review/comment-cut.js crates/l2-kingdom [more dirs or files] [--dry]
//
// A comment block is split into sentences (a sentence ends at a line ending in . : ? ! or
// at a code fence, which is one sentence whole). A sentence stays when it carries an
// evidence token: an address, a +0x offset, a decompilation line (file.c:NNN), an arm or
// sfx marker, a [V]/[I]/[D] mark, a C-number, a group/index of L2.eng, or a test's
// "red"/"ablat" line. Everything else goes; git history keeps it. Code lines are compared
// byte for byte before a file is written. Blank comment lines collapse to one between
// kept sentences, none at the edges.
const fs = require("fs"), path = require("path");
const args = process.argv.slice(2); const dry = args.includes("--dry");
const root = path.resolve(__dirname, "..", "..");
const targets = args.filter(a => !a.startsWith("--"));
const EVIDENCE = /0x[0-9A-Fa-f]{4,}|\+0x[0-9A-Fa-f]+|\b[0-9a-f]{8}\.c:\d+|\bFUN_[0-9A-Fa-f]{8}\b|\bDAT_[0-9A-Fa-f]{8}\b|\/\/\s*(arm|sfx):|\[[VID]\]|\bC\d{1,3}\b|\bgroup \d+\b|\bL2\.eng\b|\b(turns|goes|is) red\b|\bablat/;
const body = l => l.replace(/^\s*\/\/[/!]?\s?/, "");
const marker = l => (l.match(/^\s*\/\/[/!]?/) || [""])[0];
const isC = l => /^\s*\/\/[/!]?(\s|$)/.test(l);
const ends = l => /[.:?!]\s*$|\*\*\s*$/.test(body(l).replace(/[)`*_\]]+\s*$/, "")) || body(l).trim() === "";
function cutBlock(block) {
  // sentences: arrays of line indexes
  const sents = []; let cur = []; let fence = false;
  for (let i = 0; i < block.length; i++) {
    const b = body(block[i]);
    if (/^```/.test(b)) { if (!fence) { if (cur.length) { sents.push(cur); cur = []; } cur.push(i); fence = true; } else { cur.push(i); sents.push(cur); cur = []; fence = false; } continue; }
    if (fence) { cur.push(i); continue; }
    if (b.trim() === "") { if (cur.length) { sents.push(cur); cur = []; } continue; }
    cur.push(i); if (ends(block[i])) { sents.push(cur); cur = []; }
  }
  if (cur.length) sents.push(cur);
  const keep = sents.filter(s => s.some(i => EVIDENCE.test(block[i])));
  if (!keep.length) return [];
  const out = []; const m = marker(block[0]);
  keep.forEach((s, k) => { if (k) out.push(m); s.forEach(i => out.push(block[i])); });
  return out;
}
function cutFile(p) {
  const lines = fs.readFileSync(p, "utf8").split("\n"); const next = []; let i = 0, before = 0, after = 0;
  while (i < lines.length) {
    if (!isC(lines[i])) { next.push(lines[i]); i++; continue; }
    let j = i; while (j < lines.length && isC(lines[j])) j++;
    const block = lines.slice(i, j); const cut = cutBlock(block); before += block.length; after += cut.length;
    // an attribute or item follows a doc block; a dropped doc block must not leave a blank
    next.push(...cut); i = j;
  }
  const codeOld = lines.filter(l => !isC(l)).join("\n"), codeNew = next.filter(l => !isC(l)).join("\n");
  if (codeOld !== codeNew) throw new Error(`${p}: a code line moved`);
  if (!dry && after !== before) fs.writeFileSync(p, next.join("\n"));
  return [before, after];
}
const files = [];
for (const t of targets) { const p = path.resolve(root, t); if (fs.statSync(p).isDirectory()) { const walk = d => { for (const e of fs.readdirSync(d, { withFileTypes: true })) { const q = path.join(d, e.name); if (e.isDirectory()) { if (e.name !== "target") walk(q); } else if (e.name.endsWith(".rs")) files.push(q); } }; walk(p); } else files.push(p); }
let B = 0, A = 0;
for (const f of files) { const [b, a] = cutFile(f); B += b; A += a; }
console.log(`${files.length} files: comment lines ${B} -> ${A}${dry ? " (dry)" : ""}`);
