// PostToolUse on Edit|Write: the comment lines an agent just wrote in a Rust file are
// checked for the rhetoric rule 7 forbids, and the agent gets the lines back. It edits
// nothing (the trim that did cut clauses out of cited facts; docs/agents.md, The
// oracle check). Only lines the edit touched are read: the whole file's rhetoric is a
// separate job.
//
// Flagged: **bold**, SHOUTED words, and the arguing phrases tools/review/prose.js lists
// ("not the X", "rather than", "exactly as" ...). A comment carries the function, the
// address and the fact; contrast with what the code is not is prose.
const fs = require("fs"), path = require("path");
let input = ""; process.stdin.on("data", d => input += d).on("end", () => {
  let tool; try { tool = JSON.parse(input).tool_input; } catch { return; }
  const file = tool?.file_path; if (!file || !/[.]rs$/.test(file) || !fs.existsSync(file)) return;
  const root = process.env.CLAUDE_PROJECT_DIR || process.cwd();
  const rel = path.relative(root, file).split(path.sep).join("/"); if (rel.startsWith("..")) return;
  const written = tool.new_string ?? tool.content ?? (tool.edits || []).map(e => e.new_string).join("\n");
  if (!written) return;
  let phrases = [];
  try { const src = fs.readFileSync(path.join(root, "tools/review/prose.js"), "utf8"); const m = src.match(/const PHRASES = \[([\s\S]*?)\];/); if (m) phrases = JSON.parse("[" + m[1].replace(/\n/g, "") + "]"); } catch {}
  const rx = phrases.length ? new RegExp("\\b(" + phrases.map(p => p.replace(/ /g, "\\s+")).join("|") + ")\\b", "i") : null;
  const hits = [];
  for (const raw of written.split("\n")) {
    const l = raw.trim(); if (!/^\/\/[/!]?/.test(l)) continue;
    const body = l.replace(/^\/\/[/!]?\s?/, "");
    if (/\*\*[^*]+\*\*/.test(body)) hits.push(["bold", body]);
    else if (/\b[A-Z]{4,}\b/.test(body.replace(/`[^`]*`/g, "")) && !/\b(TODO|FIXME|NOTE|SAFETY|DAT|FUN|LAB|WM|VK|RT|PL8|SMK|WAV|ENG|CD|AI|OK|PRNG|UI|SFX)\b/.test(body.match(/\b[A-Z]{4,}\b/)?.[0] || "")) hits.push(["shout", body]);
    else if (/\bnot (the|a|an|what|where|one)\b/.test(body) && /,\s*not\b|\bnot\b.*—|—.*\bnot\b/.test(body)) hits.push(["contrast", body]);
    else if (rx && rx.test(body)) hits.push(["phrase", body]);
  }
  if (!hits.length) return;
  const shown = hits.slice(0, 4).map(([k, b]) => `  ${k}: ${b.slice(0, 110)}`).join("\n");
  const shape = "/// `Turn_End` (`0x0043AC23`) writes 999 when the person presses the button.";
  process.stdout.write(`comment-lint: ${rel}: ${hits.length} comment line(s) to restate in the shape of docs/agents.md, The comment shape (the function, the address, the fact), like\n  ${shape}\n${shown}\n`);
});
