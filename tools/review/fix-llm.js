// fix-llm.js - compile, hand each error with its file to Gemini Flash, apply the fix, repeat.
//
//   node tools/review/fix-llm.js [--rounds 6] [-p crate]
//
// Each round: `cargo check --workspace --tests` (or one crate), errors grouped by file; for
// each file the model sees the errors and the file's text and answers with find/replace
// blocks: "==== find ====", the exact old lines, "==== replace ====", the new lines,
// "==== end ====". A find that is not in the file exactly once is skipped: the model's
// line arithmetic is off by one too often to trust, exact text is not. Stops when the
// check is clean, or a round fixes nothing: what is left goes to a person or an agent.
const fs = require("fs"), path = require("path"), cp = require("child_process");
const root = path.resolve(__dirname, "..", "..");
const MODEL = process.env.PROSE_MODEL || "gemini-3.6-flash";
const KEY = process.env.GEMINI_API_KEY; if (!KEY) { console.error("fix-llm: GEMINI_API_KEY unset"); process.exit(2); }
const args = process.argv.slice(2); const rounds = Number(args[args.indexOf("--rounds") + 1]) || 6;
const crate = args.includes("-p") ? args[args.indexOf("-p") + 1] : null;
const RULES = `You are fixing Rust compile errors in one file of a project that was just mechanically split into directory modules (pure moves; nothing was rewritten). The errors are almost always one of: a name that needs \`pub(super)\` (or \`pub(crate)\`) on its definition; a \`super::x\` path that must become \`super::super::x\` because the code moved one module deeper; a name imported twice (delete the duplicate \`use\`); a missing \`mod x;\` line; an unclosed or extra brace at a file seam; a \`#[path = ...]\` that needs \`../\`. Never change behaviour, rename anything, or delete code that is not a duplicate import or a stray brace.
Answer only with find/replace blocks, nothing else:
==== find ====
<the exact existing lines to replace, copied verbatim, at least one line, enough to be unique in the file>
==== replace ====
<the new lines, may be empty>
==== end ====
Copy the find lines exactly as they are in the file (same indentation, no line-number prefix). Keep each block small. Give as few blocks as possible.`;
async function ask(text) {
  const r = await fetch(`https://generativelanguage.googleapis.com/v1beta/models/${MODEL}:generateContent?key=${KEY}`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ contents: [{ parts: [{ text }] }], generationConfig: { temperature: 0, thinkingConfig: { thinkingLevel: "minimal" } } }) });
  if (!r.ok) throw new Error(`${r.status} ${(await r.text()).slice(0, 200)}`);
  const j = await r.json(); return (j.candidates?.[0]?.content?.parts?.[0]?.text || "").replace(/\r/g, "");
}
function errors() {
  const out = cp.spawnSync("cargo", ["check", ...(crate ? ["-p", crate] : ["--workspace"]), "--tests", "--message-format=short"], { cwd: root, encoding: "utf8", maxBuffer: 64 << 20 });
  const text = (out.stdout || "") + (out.stderr || ""); const byFile = {};
  for (const m of text.matchAll(/^(crates[^:\n]+\.rs):(\d+):(\d+): error(?:\[E\d+\])?: (.*)$/gm)) (byFile[m[1].replace(/\\/g, "/")] = byFile[m[1].replace(/\\/g, "/")] || []).push(`${m[2]}:${m[3]}: ${m[4]}`);
  // "file not found for module" and "unclosed delimiter" point at the file that needs the line
  return byFile;
}
(async () => {
  for (let round = 0; round < rounds; round++) {
    const byFile = errors(); const files = Object.keys(byFile); const total = files.reduce((s, f) => s + byFile[f].length, 0);
    if (!files.length) { console.log(`fix-llm: round ${round}, clean`); process.exit(0); }
    console.log(`fix-llm: round ${round}, ${total} error(s) in ${files.length} file(s)`);
    let applied = 0;
    for (const f of files) {
      const p = path.join(root, f); if (!fs.existsSync(p)) continue;
      let text = fs.readFileSync(p, "utf8").replace(/\r/g, "");
      const numbered = text.split("\n").map((l, i) => `${i + 1}: ${l}`).join("\n");
      let reply; try { reply = await ask(`${RULES}\n\nFile: ${f}\nErrors (line:col: message):\n${[...new Set(byFile[f])].slice(0, 30).join("\n")}\n\n${numbered}`); } catch (e) { console.log(`   ${f}: ${e.message}`); continue; }
      const blocks = [...reply.matchAll(/==== find ====\n([\s\S]*?)\n==== replace ====\n([\s\S]*?)\n?==== end ====/g)].map(m => [m[1].replace(/^\d+: /gm, ""), m[2].replace(/^\d+: /gm, "")]);
      if (!blocks.length) { console.log(`   ${f}: no blocks in the reply`); continue; }
      let done = 0, skipped = 0;
      for (const [old, repl] of blocks) { if (/^==== /m.test(repl) || /^==== /m.test(old)) { skipped++; continue; } const i = text.indexOf(old); if (i < 0 || text.indexOf(old, i + 1) >= 0) { skipped++; continue; } text = text.slice(0, i) + repl + text.slice(i + old.length); done++; }
      if (done) { fs.writeFileSync(p, text); applied++; }
      console.log(`   ${f}: ${done} block(s) applied, ${skipped} not found once, for ${byFile[f].length} error(s)`);
    }
    if (!applied) { console.log("fix-llm: nothing applied; stopping"); process.exit(1); }
  }
  const left = errors(); console.log(`fix-llm: rounds exhausted, ${Object.values(left).flat().length} error(s) left in ${Object.keys(left).join(", ")}`); process.exit(1);
})();
