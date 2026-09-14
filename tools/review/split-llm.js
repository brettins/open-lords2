// split-llm.js - one Gemini Flash call splits a large Rust file into a directory module.
//
//   node tools/review/split-llm.js crates/l2-sim/src/runner.rs
//
// Sends the whole file, asks for {"files": {"<path>": "<content>"}} with mod.rs
// plus submodules, pure moves only, and writes them. cargo check is the caller's.
const fs = require("fs"), path = require("path");
const root = path.resolve(__dirname, "..", "..");
const MODEL = process.env.PROSE_MODEL || "gemini-3.6-flash";
const KEY = process.env.GEMINI_API_KEY; if (!KEY) { console.error("split-llm: GEMINI_API_KEY unset"); process.exit(2); }
const file = process.argv[2]; if (!file) { console.error("split-llm: <file>"); process.exit(2); }
const src = fs.readFileSync(path.join(root, file), "utf8");
const dir = file.replace(/\.rs$/, "");
const RULES = `You are splitting one Rust source file into a directory module. The file is ${file}, ${src.split("\n").length} lines.
Produce ${dir}/mod.rs plus submodule files ${dir}/<name>.rs grouped by concern, each under 900 lines. Pure moves: every item keeps its name, signature, body, attributes and doc comments byte for byte. mod.rs declares each submodule (\`mod name;\` or \`pub mod name;\`), keeps the module doc comment, the struct and enum definitions, and re-exports with \`pub use\` whatever the old file exported. Add \`use super::*;\` at the top of each submodule and widen private items to \`pub(super)\` only where a moved item is used from another file of the split. Nothing else changes.
Output every file as plain text, one after another, each introduced by a line that is exactly "==== <path> ====" (four equals signs, a space, the path, a space, four equals signs), followed by the file's content verbatim. No JSON, no code fences, nothing else.`;
(async () => {
  const r = await fetch(`https://generativelanguage.googleapis.com/v1beta/models/${MODEL}:generateContent?key=${KEY}`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ contents: [{ parts: [{ text: RULES + "\n\n```rust\n" + src + "\n```" }] }],
      generationConfig: { temperature: 0, maxOutputTokens: 120000, thinkingConfig: { thinkingLevel: "minimal" } } }) });
  if (!r.ok) { console.error(`split-llm: ${r.status} ${(await r.text()).slice(0, 300)}`); process.exit(1); }
  const j = await r.json(); const t = (j.candidates?.[0]?.content?.parts?.[0]?.text || "").replace(/\r/g, "");
  fs.writeFileSync(path.join(root, "split-llm.raw.txt"), t);
  const out = {}; const parts = t.split(/^==== (\S+) ====\n/m);
  for (let i = 1; i < parts.length; i += 2) out[parts[i]] = parts[i + 1].replace(/^```\w*\n/, "").replace(/\n```\s*$/, "\n");
  if (!Object.keys(out).length) { console.error("split-llm: no ==== path ==== sections in the reply (" + t.length + " chars); finish " + (j.candidates?.[0]?.finishReason)); process.exit(1); }
  let total = 0;
  for (const [p, c] of Object.entries(out)) { fs.mkdirSync(path.dirname(path.join(root, p)), { recursive: true }); fs.writeFileSync(path.join(root, p), c.endsWith("\n") ? c : c + "\n"); const n = c.split("\n").length; total += n; console.log(String(n).padStart(6), p); }
  fs.unlinkSync(path.join(root, file));
  console.log(`split-llm: ${Object.keys(out).length} files, ${total} lines from ${src.split("\n").length} (${j.usageMetadata?.totalTokenCount} tokens, finish ${j.candidates?.[0]?.finishReason})`);
})();
