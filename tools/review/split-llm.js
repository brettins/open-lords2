// split-llm.js - split a large Rust file into a directory module: Flash plans the cut as
// line ranges (a few hundred tokens out), the script moves the text losslessly.
//
//   node tools/review/split-llm.js crates/l2-sim/src/runner.rs
//
// The model sees the file's outline (item starts with line numbers) and returns
// {"<name>": [[start, end], ...]} covering every line once; "mod" is the file that keeps
// the module doc, the types and the `mod x;` declarations. Each submodule gets
// `use super::*;` prepended; nothing else is rewritten. cargo check is the caller's:
// visibility errors (`pub(super)`) are the one thing a move cannot decide.
const fs = require("fs"), path = require("path");
const root = path.resolve(__dirname, "..", "..");
const MODEL = process.env.PROSE_MODEL || "gemini-3.6-flash";
const KEY = process.env.GEMINI_API_KEY; if (!KEY) { console.error("split-llm: GEMINI_API_KEY unset"); process.exit(2); }
const file = process.argv[2]; if (!file) { console.error("split-llm: <file>"); process.exit(2); }
const src = fs.readFileSync(path.join(root, file), "utf8").replace(/\r/g, "");
const lines = src.split("\n"); const N = lines.length;
const isModRs = /\/mod\.rs$/.test(file);
const dir = isModRs ? path.dirname(file) : file.replace(/\.rs$/, "");
const outline = lines.map((l, i) => [i + 1, l]).filter(([, l]) => /^(pub(\([a-z]+\))? |)(fn|impl|struct|enum|const|static|type|mod|trait|macro_rules!|use )|^#\[|^\/\/! |^}/.test(l)).map(([n, l]) => `${n}: ${l.slice(0, 110)}`).join("\n");
const RULES = `Below is the outline of ${file} (${N} lines): the line number and text of every top-level item start, attribute, closing brace and module doc line. Plan a split of this file into a directory module ${dir}/ with a mod file and submodule files grouped by concern, each under 900 lines. Top-level items must not be cut in the middle: a range starts at an item's first line (its doc comment or attribute, if any) and ends at its closing brace. \`impl\` blocks may be split only at method boundaries if you also assign the impl header line to each part (the script re-opens the block).
Return JSON only: {"mod": [[start, end], ...], "<name>": [[start, end], ...], ...} where every line 1..${N} belongs to exactly one range and names are lowercase identifiers. "mod" holds the module doc, the \`use\` lines, the types, constants and any \`#[cfg(test)] mod\` declarations.`;
(async () => {
  const r = await fetch(`https://generativelanguage.googleapis.com/v1beta/models/${MODEL}:generateContent?key=${KEY}`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ contents: [{ parts: [{ text: RULES + "\n\n" + outline }] }],
      generationConfig: { responseMimeType: "application/json", temperature: 0, thinkingConfig: { thinkingLevel: "minimal" } } }) });
  if (!r.ok) { console.error(`split-llm: ${r.status} ${(await r.text()).slice(0, 300)}`); process.exit(1); }
  const j = await r.json(); const t = j.candidates?.[0]?.content?.parts?.[0]?.text || "{}";
  let plan; try { plan = JSON.parse(t); } catch { console.error("split-llm: plan is not JSON: " + t.slice(0, 200)); process.exit(1); }
  // Coverage: every line once.
  const owner = new Array(N + 1).fill(null);
  for (const [name, ranges] of Object.entries(plan)) for (const [a, b] of ranges) for (let i = a; i <= b; i++) if (!owner[i]) owner[i] = name; // an overlap keeps the first owner
  // A line the plan skipped (a comment between items, a blank) goes with the next assigned line.
  let gaps = 0; for (let i = N; i >= 1; i--) if (!owner[i]) { owner[i] = owner[i + 1] || "mod"; if (lines[i - 1].trim() !== "") gaps++; }
  if (gaps) console.log(`split-llm: ${gaps} non-blank lines the plan skipped go with the item below them`);
  // No boundary inside an item: every line takes the owner of the item start above it,
  // where an item starts at a depth-0 line or a depth-1 method/const line in an impl.
  // A `mod x { ... }` block is one unit: its items must stay together, in one file.
  { let d = 0, cur = 1, inMod = false; for (let i = 1; i <= N; i++) { const l = lines[i - 1]; const code = l.replace(/\/\/.*$/, "").replace(/'(\\.|[^'\\])'/g, "''").replace(/"([^"\\]|\\.)*"/g, '""'); const top = d === 0 && /^\S/.test(l) && !/^}/.test(l); if (top) inMod = /^(pub(\([a-z]+\))? )?mod\b/.test(l); const starts = top || (d === 1 && !inMod && /^    (pub(\([a-z]+\))? )?(fn|const|type|static)\b/.test(l)); if (starts) cur = i; if (!(d === 0 && /^}/.test(l))) owner[i] = owner[cur]; d += (code.match(/{/g) || []).length - (code.match(/}/g) || []).length; } }
  // A doc comment or attribute belongs to the item under it: a boundary that falls
  // between them moves up so they travel together.
  for (let i = N - 1; i >= 1; i--) if (owner[i] !== owner[i + 1] && /^\s*(\/\/\/|\/\/!|#\[)/.test(lines[i - 1])) owner[i] = owner[i + 1];
  // Brace depth per line, so an impl block split across files is re-opened and closed
  // in each file that holds a piece of it. Braces inside line comments are ignored.
  const header = new Array(N + 2).fill(0), closes = new Array(N + 2).fill(0); let depth = 0, open = 0;
  for (let i = 1; i <= N; i++) {
    const code = lines[i - 1].replace(/\/\/.*$/, "").replace(/'(\\.|[^'\\])'/g, "''").replace(/"([^"\\]|\\.)*"/g, '""');
    const o = (code.match(/{/g) || []).length, c = (code.match(/}/g) || []).length;
    if (depth === 0 && o > c && /^(pub(\([a-z]+\))? )?(impl|trait|mod)\b/.test(lines[i - 1])) { open = i; header[i] = -i; }
    else if (depth > 0 && open) header[i] = open;
    depth += o - c;
    if (depth === 0 && open && header[i] === open && c > o) { closes[i] = open; open = 0; }
    if (depth === 0 && !header[i]) open = 0;
  }
  const files = {}; const opened = {};
  for (let i = 1; i <= N; i++) {
    const o = owner[i] || "mod"; const f = (files[o] = files[o] || []);
    const h = header[i] < 0 ? -header[i] : header[i];
    if (closes[i]) { if (opened[o] === closes[i]) { f.push(lines[i - 1]); opened[o] = 0; } continue; }
    if (h && opened[o] !== h) { if (opened[o]) f.push("}"); if (header[i] > 0) f.push(lines[h - 1]); opened[o] = h; }
    if (!h && opened[o]) { f.push("}"); opened[o] = 0; }
    f.push(lines[i - 1]);
  }
  for (const o of Object.keys(files)) if (opened[o]) files[o].push("}");
  const names = Object.keys(files).filter(n => n !== "mod");
  // The parent's own `use` lines (multi-line ones too): a glob of `super` does not carry
  // imports, so each submodule repeats them.
  const uses = []; { let d = 0, inUse = false; for (const l of lines) { if (d === 0 && /^(pub )?use /.test(l)) inUse = true; if (inUse) uses.push(l.replace(/^pub /, "")); if (inUse && /;\s*$/.test(l)) inUse = false; const code = l.replace(/\/\/.*$/, ""); d += (code.match(/{/g) || []).length - (code.match(/}/g) || []).length; } }
  const prelude = "#![allow(unused_imports)]\nuse super::*;\n" + uses.join("\n") + "\n\n";
  fs.mkdirSync(path.join(root, dir), { recursive: true });
  const modBody = files.mod.join("\n");
  const decl = names.map(n => `mod ${n};\npub use ${n}::*;`).join("\n") + "\n";
  // The mod declarations go after the module doc (`//!` lines) and before the first item.
  const docEnd = files.mod.findIndex(l => !/^\/\/!/.test(l) && l.trim() !== "");
  const modLines = files.mod.slice(); modLines.splice(docEnd < 0 ? 0 : docEnd, 0, "", decl);
  fs.writeFileSync(path.join(root, dir, "mod.rs"), modLines.join("\n").replace(/\n{3,}/g, "\n\n") + "\n");
  for (const n of names) fs.writeFileSync(path.join(root, dir, n + ".rs"), prelude + files[n].join("\n") + "\n");
  if (!isModRs) fs.unlinkSync(path.join(root, file));
  console.log(`${String(files.mod.length + 2).padStart(6)} ${dir}/mod.rs`); for (const n of names) console.log(`${String(files[n].length + 2).padStart(6)} ${dir}/${n}.rs`);
  console.log(`split-llm: ${names.length + 1} files from ${N} lines, every line kept (${j.usageMetadata?.totalTokenCount} tokens)`);
})();
