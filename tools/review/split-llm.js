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
// A mod.rs, lib.rs or main.rs is a module root already: it splits in place, siblings beside it.
const isModRs = /\/(mod|lib|main)\.rs$/.test(file);
const dir = isModRs ? path.dirname(file) : file.replace(/\.rs$/, "");
// An integration test `tests/x.rs` becomes the target `tests/x/main.rs`.
const modPath = isModRs ? file : dir + (/\/tests\/[^/]+\.rs$/.test(file) ? "/main.rs" : "/mod.rs");
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
  // A submodule named like an item or an import of the file would shadow it (`mod trade`
  // beside `use l2_kingdom::trade::{self}`, `mod blacksmith` beside `fn blacksmith`): _part.
  { const taken = new Set(); for (const l of lines) { let m;
      if ((m = l.match(/^(?:pub(?:\([a-z]+\))? )?(?:fn|struct|enum|const|static|type|trait|mod|macro_rules!)\s+(\w+)/))) taken.add(m[1]);
      if ((m = l.match(/^(?:pub )?use\s+([\w:]+)(?:::\{([^}]*)\})?\s*;/))) { const segs = m[1].split("::"); if (m[2]) for (const p of m[2].split(",")) { const t = p.trim().replace(/^.* as /, ""); taken.add(t === "self" ? segs[segs.length - 1] : t); } else taken.add(segs[segs.length - 1]); } }
    for (const k of Object.keys(plan)) if (k !== "mod" && taken.has(k)) { plan[k + "_part"] = plan[k]; delete plan[k]; console.log(`split-llm: "${k}" is taken in the file; the submodule is ${k}_part`); } }
  // Coverage: every line once.
  const owner = new Array(N + 1).fill(null);
  for (const [name, ranges] of Object.entries(plan)) for (const [a, b] of ranges) for (let i = a; i <= b; i++) if (!owner[i]) owner[i] = name; // an overlap keeps the first owner
  // A line the plan skipped (a comment between items, a blank) goes with the next assigned line.
  let gaps = 0; for (let i = N; i >= 1; i--) if (!owner[i]) { owner[i] = owner[i + 1] || "mod"; if (lines[i - 1].trim() !== "") gaps++; }
  if (gaps) console.log(`split-llm: ${gaps} non-blank lines the plan skipped go with the item below them`);
  // No boundary inside an item: every line takes the owner of the item start above it,
  // where an item starts at a depth-0 line or a depth-1 method/const line in an impl.
  // A `mod x { ... }` block is one unit: its items must stay together, in one file.
  { let d = 0, cur = 1, inMod = false; for (let i = 1; i <= N; i++) { const l = lines[i - 1]; const code = l.replace(/\/\/.*$/, "").replace(/'(\\.|[^'\\])'/g, "''").replace(/"([^"\\]|\\.)*"/g, '""'); const top = d === 0 && /^\S/.test(l) && !/^[}\])]/.test(l); if (top) inMod = /^(pub(\([a-z]+\))? )?mod\b/.test(l); const starts = top || (d === 1 && !inMod && /^    (pub(\([a-z]+\))? )?(fn|const|type|static)\b/.test(l)); if (starts) cur = i; if (!(d === 0 && /^}/.test(l))) owner[i] = owner[cur]; d += (code.match(/{/g) || []).length - (code.match(/}/g) || []).length; } }
  // A `mod x;` declaration (and its `pub use x::*;`) stays in the root file: the module
  // it names lives beside the root, not beside a part.
  for (let i = 1; i <= N; i++) if (/^(pub(\([a-z]+\))? )?mod \w+;$/.test(lines[i - 1]) || /^pub use \w+::\*;$/.test(lines[i - 1])) owner[i] = "mod";
  // `fn main` is the binary's entry and stays in the crate root with any attributes on it.
  for (let i = 1; i <= N; i++) if (/^fn main\b/.test(lines[i - 1])) { let d = 0; for (let j = i; j <= N; j++) { const l = lines[j - 1]; owner[j] = "mod"; d += (l.match(/{/g) || []).length - (l.match(/}/g) || []).length; if (d === 0 && j > i && /^}/.test(l)) break; } }
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
  // Closing a re-opened block that got no item drops the header instead: an empty
  // `impl Trait for T {}` beside the real one is a conflicting implementation.
  const close = (f, o, closer) => { const h = opened[o]; opened[o] = 0; if (f.length && f[f.length - 1] === lines[h - 1]) f.pop(); else f.push(closer); };
  for (let i = 1; i <= N; i++) {
    const o = owner[i] || "mod"; const f = (files[o] = files[o] || []);
    const h = header[i] < 0 ? -header[i] : header[i];
    if (closes[i]) { if (opened[o] === closes[i]) close(f, o, lines[i - 1]); continue; }
    if (h && opened[o] !== h) { if (opened[o]) close(f, o, "}"); if (header[i] > 0) f.push(lines[h - 1]); opened[o] = h; }
    if (!h && opened[o]) close(f, o, "}");
    f.push(lines[i - 1]);
  }
  for (const o of Object.keys(files)) if (opened[o]) close(files[o], o, "}");
  const names = Object.keys(files).filter(n => n !== "mod");
  const KEYWORDS = /^(as|break|const|continue|crate|else|enum|extern|false|fn|for|if|impl|in|let|loop|match|mod|move|mut|pub|ref|return|self|static|struct|super|trait|true|type|unsafe|use|where|while|async|await|dyn|abstract|become|box|do|final|macro|override|priv|typeof|unsized|virtual|yield|try|tests)$/;
  for (const n of names) if (KEYWORDS.test(n) || !/^[a-z][a-z0-9_]*$/.test(n)) { console.error(`split-llm: "${n}" is not a usable module name`); process.exit(1); }
  // The parent's own `use` lines (multi-line ones too): a glob of `super` does not carry
  // imports, so each submodule repeats them.
  const uses = []; { let d = 0, inUse = false; for (const l of lines) { if (d === 0 && /^(pub )?use /.test(l)) inUse = true; if (inUse) uses.push(l.replace(/^pub /, "")); if (inUse && /;\s*$/.test(l)) inUse = false; const code = l.replace(/\/\/.*$/, ""); d += (code.match(/{/g) || []).length - (code.match(/}/g) || []).length; } }
  // Each submodule sees the parent and every sibling: private items a sibling holds are
  // not re-exported by the parent's `pub use`, so the globs go sideways too.
  // The parent's own `super::` is the part's `super::super::`.
  // The parent's own submodules (`mod x;` here, `use x::*` here) are `super::x` in a part.
  const declared = new Set(lines.map(l => (l.match(/^(?:pub(?:\([a-z]+\))? )?mod (\w+);$/) || [])[1]).filter(Boolean));
  const prelude = n => "#![allow(unused_imports)]\nuse super::*;\n" + names.filter(s => s !== n).map(s => `use super::${s}::*;`).join("\n") + "\n" + uses.map(u => u.replace(/^use super::/, "use super::super::").replace(/^use (\w+)::/, (m, x) => declared.has(x) ? `use super::${x}::` : m)).join("\n") + "\n\n";
  fs.mkdirSync(path.join(root, dir), { recursive: true });
  const modBody = files.mod.join("\n");
  const decl = names.map(n => `mod ${n};\npub use ${n}::*;`).join("\n") + "\n";
  // The mod declarations go after the module doc (`//!` lines) and before the first item.
  if (!names.length) { console.error("split-llm: the plan put every line in mod; nothing to split"); process.exit(1); }
  // The declarations go after the module doc, and after any `macro_rules!` the parts use:
  // a macro by example is textually scoped, so it must precede the `mod x;` that needs it.
  let docEnd = files.mod.findIndex(l => !/^\/\/!/.test(l) && !/^#!\[/.test(l) && l.trim() !== "");
  { let d = 0, inMacro = false; for (let i = 0; i < files.mod.length; i++) { const l = files.mod[i]; if (d === 0 && /^macro_rules!/.test(l)) inMacro = true; const code = l.replace(/\/\/.*$/, ""); d += (code.match(/{/g) || []).length - (code.match(/}/g) || []).length; if (inMacro && d === 0) { inMacro = false; docEnd = Math.max(docEnd, i + 1); }
      // `#[macro_use] mod x;` likewise: its macros reach only what is declared after it.
      if (d === 0 && /^(pub(\([a-z]+\))? )?mod \w+;$/.test(l) && files.mod.slice(Math.max(0, i - 3), i).some(a => /^#\[macro_use\]/.test(a))) docEnd = Math.max(docEnd, i + 1); } }
  // `use` lines the plan sent to a submodule still serve the items that stayed.
  const lost = uses.filter(u => !files.mod.includes(u) && !files.mod.includes("pub " + u));
  const modLines = files.mod.slice(); modLines.splice(docEnd < 0 ? 0 : docEnd, 0, "", decl, ...(lost.length ? [...lost, ""] : []));
  // A `mod x;` the file already had names a sibling at the old level (a test's `mod common;`).
  if (!isModRs) for (let i = 0; i < modLines.length; i++) { const m = modLines[i].match(/^(pub(\([a-z]+\))? )?mod (\w+);$/); if (!m || names.includes(m[3]) || /#\[path/.test(modLines[i - 1] || "")) continue;
    const up = path.join(root, path.dirname(file)); const rel = fs.existsSync(path.join(up, m[3] + ".rs")) ? `../${m[3]}.rs` : fs.existsSync(path.join(up, m[3], "mod.rs")) ? `../${m[3]}/mod.rs` : null;
    if (rel) modLines.splice(i, 0, `#[path = "${rel}"]`), i++; }
  // `#[path = "x.rs"]` inside a file that moved one directory deeper points one level up.
  const deeper = s => isModRs ? s : s.replace(/^(\s*#\[path = ")(?!\/)/gm, "$1../").replace(/(include_(?:str|bytes)!\(")(?!\/)/g, "$1../");
  fs.writeFileSync(path.join(root, modPath), deeper(modLines.join("\n").replace(/\n{3,}/g, "\n\n")) + "\n");
  for (const n of names) fs.writeFileSync(path.join(root, dir, n + ".rs"), prelude(n) + deeper(files[n].join("\n")) + "\n");
  if (!isModRs) fs.unlinkSync(path.join(root, file));
  // Any `#[path]` elsewhere in the crate that named the old file now names the new root.
  if (!isModRs) { const crateDir = path.join(root, file.split("/").slice(0, 2).join("/")); const target = path.resolve(root, file);
    const walk = d => { for (const e of fs.readdirSync(d, { withFileTypes: true })) { const p = path.join(d, e.name); if (e.isDirectory()) { if (e.name !== "target") walk(p); } else if (e.name.endsWith(".rs")) {
      const s = fs.readFileSync(p, "utf8"); const t = s.replace(/#\[path = "([^"]+)"\]/g, (m, rel) => path.resolve(path.dirname(p), rel) === target ? `#[path = "${path.relative(path.dirname(p), path.join(root, modPath)).replace(/\\/g, "/")}"]` : m);
      if (t !== s) fs.writeFileSync(p, t); } } }; walk(crateDir); }
  console.log(`${String(files.mod.length + 2).padStart(6)} ${modPath}`); for (const n of names) console.log(`${String(files[n].length + 2).padStart(6)} ${dir}/${n}.rs`);
  console.log(`split-llm: ${names.length + 1} files from ${N} lines, every line kept (${j.usageMetadata?.totalTokenCount} tokens)`);
  process.exit(0); // node 24 on Windows can assert in libuv while closing the fetch handle at exit
})();
