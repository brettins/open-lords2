// comment-llm.js - restate one Rust file's comment blocks in the comment shape.
//
//   node tools/review/comment-llm.js crates/x/src/y.rs [--dry] [--think low|medium|high]
//   node tools/review/comment-llm.js crates/x/src/y.rs --flagged [--limit N]
//
// Each contiguous block of `//`, `///` or `//!` lines goes to Gemini Flash with the five
// exemplars from docs/agents.md, The comment shape. The reply replaces the block's lines
// and nothing else: code lines are compared byte for byte before the file is written.
// A block is kept as it was when the reply drops evidence the block carried (an address,
// a number, a path, a backticked name, a [V]/[I]/[D] mark, a C-number) or pairs an
// address with a name it did not sit beside before.
// --flagged: only blocks with a line the comment lint hands back (bold, shouting,
// contrast, a prose.js phrase), one call per block, the flagged lines named to the model.
const fs = require("fs"), path = require("path");
const MODEL = "gemini-3.6-flash";
const KEY = process.env.GEMINI_API_KEY; if (!KEY) { console.error("comment-llm: GEMINI_API_KEY unset"); process.exit(2); }
const args = process.argv.slice(2); const dry = args.includes("--dry"); const flagged = args.includes("--flagged");
const opt = k => (args.includes(k) ? args[args.indexOf(k) + 1] : undefined);
const think = opt("--think") || "minimal"; const limit = opt("--limit") ? Number(opt("--limit")) : Infinity;
const files = args.filter((a, i) => !a.startsWith("--") && !["--think", "--limit"].includes(args[i - 1]));
const root = path.resolve(__dirname, "..", "..");
const agents = fs.readFileSync(path.join(root, "docs/agents.md"), "utf8");
const shape = (agents.match(/### The comment shape[\s\S]*?```rust\n([\s\S]*?)```/) || [])[1] || "";
let phrases = [];
try { const src = fs.readFileSync(path.join(__dirname, "prose.js"), "utf8"); const m = src.match(/const PHRASES = \[([\s\S]*?)\];/); if (m) phrases = JSON.parse("[" + m[1].replace(/\n/g, "") + "]"); } catch {}
const prx = phrases.length ? new RegExp("\\b(" + phrases.map(p => p.replace(/ /g, "\\s+")).join("|") + ")\\b", "i") : /$^/;
const body = l => l.replace(/^\s*\/\/[/!]?\s?/, "");
const flaggedLine = l => { const b = body(l); return /\*\*[^*]+\*\*/.test(b) || /\b[A-Z]{4,}\b/.test(b.replace(/`[^`]*`/g, "")) || /,\s*not\b|\bnot\b.*—|—.*\bnot\b/.test(b) || prx.test(b); };
const RULES = `You restate Rust comment blocks in the shape of these lines, which are the standard:

${shape}
A block is: the function, its address and the decompilation line; what it does, with the numbers; then one line per claim that is inferred or a departure, [I] or [D] with the reason. State facts. Keep every backticked name, address, number, path, [V]/[I]/[D] mark and C-number that the block carries, each beside the same name it stood beside; never attach an address to a different name. Keep the comment marker (//, /// or //!) and indentation on every line. Drop framing, contrast, emphasis, restatement and anything the code beside the comment says itself; keep every fact. A block already in the shape comes back unchanged. Never more lines than the block has. Wrap at 90 columns.
The input is a JSON array of blocks, each an array of lines. Return a JSON array of the same length: each element the block's new lines as an array of strings. Nothing else.`;
const TOKEN = /`[^`]+`|0x[0-9A-Fa-f]{3,}|\+0x[0-9A-Fa-f]+|\b\d+\b|\[[VID]\]|\bC\d{1,3}\b|[\w./-]+\.(rs|md|c|json|pl8|wav|smk)\b/g;
const tokens = ls => new Set(ls.join("\n").match(TOKEN) || []);
// Every (name, address) pair on one line: the reply must keep each pair on one line too.
const pairs = ls => { const out = new Set(); for (const l of ls) { const names = l.match(/`[A-Za-z_][\w]*`/g) || [], addrs = l.match(/0x00[0-9A-Fa-f]{6}/g) || []; for (const n of names) for (const a of addrs) out.add(n + "@" + a); } return out; };
const words = ls => ls.map(body).join(" ").split(/\s+/).filter(Boolean).length;
async function ask(blocks, note) {
  const cfg = { responseMimeType: "application/json", temperature: 0 };
  if (think !== "none") cfg.thinkingConfig = { thinkingLevel: think };
  const r = await fetch(`https://generativelanguage.googleapis.com/v1beta/models/${MODEL}:generateContent?key=${KEY}`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ contents: [{ parts: [{ text: RULES + (note ? "\n" + note : "") + "\n\n" + JSON.stringify(blocks) }] }], generationConfig: cfg }) });
  if (!r.ok) throw new Error(`${r.status} ${(await r.text()).slice(0, 300)}`);
  const j = await r.json(); const t = j.candidates?.[0]?.content?.parts?.[0]?.text || "[]";
  return { out: JSON.parse(t), tokens: j.usageMetadata?.totalTokenCount || 0 };
}
(async () => {
  for (const f of files) {
    const p = path.resolve(root, f); const lines = fs.readFileSync(p, "utf8").split("\n");
    const isC = l => /^\s*\/\/[/!]?(\s|$)/.test(l) && !/^\s*\/\/\s*(arm|sfx):/.test(l);
    let blocks = []; let i = 0;
    while (i < lines.length) { if (!isC(lines[i])) { i++; continue; } let j = i; while (j < lines.length && isC(lines[j])) j++; blocks.push([i, j]); i = j; }
    if (flagged) blocks = blocks.filter(([a, b]) => lines.slice(a, b).some(flaggedLine)).slice(0, limit);
    if (!blocks.length) { console.log(`${f}: no blocks`); continue; }
    let out = [], used = 0;
    if (flagged) {
      for (const [a, b] of blocks) {
        const blk = lines.slice(a, b); const bad = blk.filter(flaggedLine).map(body);
        const r = await ask([blk], "The lines to restate first: " + JSON.stringify(bad));
        out.push(r.out[0]); used += r.tokens;
      }
    } else { const r = await ask(blocks.map(([a, b]) => lines.slice(a, b))); out = r.out; used = r.tokens; }
    let changed = 0, kept = 0, fewer = 0, same = 0, wOld = 0, wNew = 0; const why = [];
    const next = []; let cur = 0;
    for (let k = 0; k < blocks.length; k++) {
      const [a, b] = blocks[k]; next.push(...lines.slice(cur, a)); cur = b;
      const old = lines.slice(a, b), neu = Array.isArray(out[k]) ? out[k].map(String) : null;
      let reason = null;
      if (!neu || !neu.length) reason = "no reply";
      else if (neu.length > old.length) reason = "longer";
      else if (!neu.every(isC)) reason = "a line is not a comment";
      else { const miss = [...tokens(old)].filter(t => !tokens(neu).has(t)); if (miss.length) reason = "dropped " + miss.slice(0, 3).join(" "); else { const np = pairs(neu), op = pairs(old); const bad = [...np].filter(x => !op.has(x)); if (bad.length) reason = "new pairing " + bad[0]; } }
      wOld += words(old);
      if (!reason && neu.join("\n") !== old.join("\n")) { next.push(...neu); changed++; fewer += old.length - neu.length; wNew += words(neu); }
      else { next.push(...old); wNew += words(old); if (reason) { kept++; why.push(`  block ${a + 1}: ${reason}`); } else if (words(old) > 20) { same++; why.push(`  block ${a + 1}: unchanged (${words(old)} words)`); } }
    }
    next.push(...lines.slice(cur));
    const codeOld = lines.filter(l => !isC(l)).join("\n"), codeNew = next.filter(l => !isC(l)).join("\n");
    if (codeOld !== codeNew) { console.error(`${f}: a code line moved; nothing written`); continue; }
    console.log(`${f}: ${blocks.length} blocks, ${changed} restated, ${kept} kept, ${same} unchanged, ${fewer} lines fewer, comment words ${wOld} -> ${wNew}, ${used} tokens, think ${think}`);
    if (why.length) console.log(why.join("\n"));
    if (!dry) fs.writeFileSync(p, next.join("\n"));
  }
})();
