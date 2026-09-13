// prose-llm.js - one model call per file: the listing in, the rewritten lines out, applied.
//
//   node tools/review/prose-llm.js docs/netcode.md [more files]     # GEMINI_API_KEY, gemini-2.5-flash
//   node tools/review/prose-llm.js --top 20                          # the 20 worst files by prose.js
//
// Each file: `prose.js <file>` lists the lines; the model returns {"<line>": "<rewrite>"};
// `prose.js --apply` swaps them in. Nothing else touches the file. Commit is the caller's.
const fs = require("fs"), path = require("path"), cp = require("child_process"), os = require("os");
const root = path.resolve(__dirname, "..", "..");
const MODEL = process.env.PROSE_MODEL || "gemini-3.6-flash";
const KEY = process.env.GEMINI_API_KEY; if (!KEY) { console.error("prose-llm: GEMINI_API_KEY unset"); process.exit(2); }
const RULES = `Each line below, "N: text", is one line of a document or a source-code comment that argues with a claim nobody made.
Rewrite each so it states the fact and drops the argument. "X rather than Y" becomes "X". Fillers (actually, simply, in fact, of course, note that, to be clear, precisely, merely, genuinely, importantly, crucially) are deleted. "There is no Y, only X" becomes "X". "Which is why Z" becomes "so Z".
Keep every number, hex address, function name, file path, C-number, B-number, [V]/[I]/[D] mark, <!--fig:...--> marker, code span, link, quotation, markdown, indentation and comment prefix (//, //!, ///). A table row keeps its cell count. If the fact lives in the argued clause, keep it in plain words. Never add a fact. Omit a line you cannot rewrite safely.
Return a JSON object mapping the line number (as a string) to the full rewritten line. Nothing else.`;
async function ask(listing) {
  const r = await fetch(`https://generativelanguage.googleapis.com/v1beta/models/${MODEL}:generateContent?key=${KEY}`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ contents: [{ parts: [{ text: RULES + "\n\n" + listing }] }],
      generationConfig: { responseMimeType: "application/json", temperature: 0.2 } }) });
  if (!r.ok) throw new Error(`${r.status} ${(await r.text()).slice(0, 300)}`);
  const j = await r.json(); const t = j.candidates?.[0]?.content?.parts?.[0]?.text || "{}";
  return { map: JSON.parse(t), tokens: j.usageMetadata?.totalTokenCount || 0 };
}
(async () => {
  let args = process.argv.slice(2);
  if (args[0] === "--top") args = cp.execSync("node tools/review/prose.js", { cwd: root }).toString().split(/\r?\n/)
    .filter(l => /^\s*\d+ /.test(l) && !/symbols\.md/.test(l)).slice(0, Number(args[1] || 10)).map(l => l.trim().split(/\s+/)[1]);
  for (const f of args) {
    const listing = cp.execSync(`node tools/review/prose.js ${f}`, { cwd: root }).toString().split(/\r?\n/).filter(l => /^\d+: /.test(l)).join("\n");
    if (!listing) { console.log(`    0 ${f}`); continue; }
    let res; try { res = await ask(listing); } catch (e) { console.error(`prose-llm: ${f}: ${e.message}`); continue; }
    const tmp = path.join(os.tmpdir(), "prose-llm.json"); fs.writeFileSync(tmp, JSON.stringify(res.map));
    let out = cp.spawnSync("node", ["tools/review/prose.js", "--apply", f, tmp], { cwd: root, encoding: "utf8" });
    if (out.status) { // drop the lines it refused and retry once
      const bad = (out.stderr.match(/listing: (.*)/) || [, ""])[1].split(/,\s*/).filter(Boolean);
      for (const n of bad) delete res.map[n]; fs.writeFileSync(tmp, JSON.stringify(res.map));
      out = cp.spawnSync("node", ["tools/review/prose.js", "--apply", f, tmp], { cwd: root, encoding: "utf8" });
    }
    const left = cp.execSync(`node tools/review/prose.js ${f}`, { cwd: root }).toString().match(/(\d+) hit/)?.[1] || "?";
    console.log(`${String(listing.split("\n").length).padStart(5)} -> ${String(left).padStart(3)} ${f}  (${res.tokens} tokens${out.status ? ", apply failed: " + out.stderr.trim().slice(0, 120) : ""})`);
  }
})();
