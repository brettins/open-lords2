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
const RULES = `Each line below, "N: text", contains a hedge or an argument with a claim nobody made (rather than, actually, there is no, which is why, not just, in fact, exactly as, was never, and the like).
Delete the phrase and the clause it introduces, up to the next comma, semicolon or full stop. Do not rewrite, soften or replace it with another word. Keep the rest of the line byte for byte. If the whole line is the argument, return "" to drop it.
Return a JSON object mapping the line number (as a string) to the shortened line. Nothing else.`;
async function ask(listing) {
  const r = await fetch(`https://generativelanguage.googleapis.com/v1beta/models/${MODEL}:generateContent?key=${KEY}`, {
    method: "POST", headers: { "content-type": "application/json" },
    body: JSON.stringify({ contents: [{ parts: [{ text: RULES + "\n\n" + listing }] }],
      generationConfig: { responseMimeType: "application/json", temperature: 0, thinkingConfig: { thinkingLevel: "minimal" } } }) });
  if (!r.ok) throw new Error(`${r.status} ${(await r.text()).slice(0, 300)}`);
  const j = await r.json(); const t = j.candidates?.[0]?.content?.parts?.[0]?.text || "{}";
  return { map: JSON.parse(t), tokens: j.usageMetadata?.totalTokenCount || 0 };
}
// --symbols: the comments in docs/symbols.json (functions and globals) that carry a
// phrase, cut the same way in chunks of 50, then docs/symbols.md regenerated.
async function symbols() {
  const file = path.join(root, "docs/symbols.json"), s = JSON.parse(fs.readFileSync(file, "utf8"));
  const phrases = cp.execSync("node tools/review/prose.js --phrases", { cwd: root }).toString().split(/\r?\n/).filter(Boolean);
  const RX = new RegExp("\\b(" + phrases.map(p => p.replace(/ /g, "\\s+")).join("|") + ")\\b", "i");
  const hits = [...s.functions, ...s.globals].filter(e => typeof e.comment === "string" && RX.test(e.comment));
  let done = 0, tokens = 0;
  for (let i = 0; i < hits.length; i += 50) {
    const chunk = hits.slice(i, i + 50);
    const listing = chunk.map((e, k) => `${k + 1}: ${e.comment.replace(/\s+/g, " ")}`).join("\n");
    let res; try { res = await ask(listing); } catch (e) { console.error(`prose-llm: symbols chunk ${i}: ${e.message}`); continue; }
    for (const [n, v] of Object.entries(res.map)) { const e = chunk[n - 1]; if (e && typeof v === "string" && v !== "") { e.comment = v.replace(/ +([.,;:)])/g, "$1").replace(/  +/g, " ").trim(); done++; } }
    tokens += res.tokens;
  }
  fs.writeFileSync(file, JSON.stringify(s, null, 2) + "\n");
  cp.execSync("node tools/symbols/symbols_md.js", { cwd: root, stdio: "inherit" });
  console.log(`symbols: ${hits.length} comments listed, ${done} cut (${tokens} tokens); docs/symbols.md regenerated`);
}

(async () => {
  let args = process.argv.slice(2);
  if (args[0] === "--symbols") return symbols();
  if (args[0] === "--top") args = cp.execSync("node tools/review/prose.js", { cwd: root }).toString().split(/\r?\n/)
    .filter(l => /^\s*\d+ /.test(l) && !/symbols\.md/.test(l)).slice(0, Number(args[1] || 10)).map(l => l.trim().split(/\s+/)[1]);
  for (const f of args) {
    const listing = cp.execSync(`node tools/review/prose.js ${f}`, { cwd: root }).toString().split(/\r?\n/).filter(l => /^\d+: /.test(l)).join("\n");
    if (!listing) { console.log(`    0 ${f}`); continue; }
    let res; try { res = await ask(listing); } catch (e) { console.error(`prose-llm: ${f}: ${e.message}`); continue; }
    // Tidy the cut, mechanically: the old line's indent, no space before punctuation, no double space.
    const src = fs.readFileSync(path.join(root, f), "utf8").split(/\r?\n/);
    for (let [n, v] of Object.entries(res.map)) {
      if (v === "" || !src[n - 1]) continue;
      const indent = src[n - 1].match(/^\s*/)[0];
      const pre = src[n - 1].match(/^\s*(\/\/[!/]?)/); // a comment line keeps its prefix
      if (pre && !/^\s*\/\//.test(v)) v = pre[1] + " " + v.trim();
      res.map[n] = indent + v.trimStart().replace(/ +([.,;:)])/g, "$1").replace(/  +/g, " ").replace(/\*\*\s*\*\*/g, "").trimEnd();
    }
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
