// PostToolUse on Edit|Write: the comment lines an agent just wrote in a Rust file come
// back when they carry no evidence. The standard since 2026-09-14 (CLAUDE.md rule 7): a
// comment sentence carries an address, a +0x offset, a decompilation line, an arm or sfx
// marker, a [V]/[I]/[D] mark, a C-number, an L2.eng group, or the line a test's ablation
// names; a sentence without one is what the code already says, and goes. The hook edits
// nothing and quotes the shape (docs/agents.md, The comment shape).
const fs = require("fs"), path = require("path");
const EVIDENCE = /0x[0-9A-Fa-f]{4,}|\+0x[0-9A-Fa-f]+|\b[0-9a-f]{8}\.c:\d+|\bFUN_[0-9A-Fa-f]{8}\b|\bDAT_[0-9A-Fa-f]{8}\b|\/\/\s*(arm|sfx):|\[[VID]\]|\bC\d{1,3}\b|\bgroup \d+\b|\bL2\.eng\b|\b(turns|goes|is) red\b|\bablat/;
const body = l => l.replace(/^\s*\/\/[/!]?\s?/, "");
const ends = l => /[.:?!]\s*$/.test(body(l).replace(/[)`*_\]]+\s*$/, "")) || body(l).trim() === "";
let input = ""; process.stdin.on("data", d => input += d).on("end", () => {
  let tool; try { tool = JSON.parse(input).tool_input; } catch { return; }
  const file = tool?.file_path; if (!file || !/[.]rs$/.test(file) || !fs.existsSync(file)) return;
  const root = process.env.CLAUDE_PROJECT_DIR || process.cwd();
  const rel = path.relative(root, file).split(path.sep).join("/"); if (rel.startsWith("..")) return;
  const written = tool.new_string ?? tool.content ?? (tool.edits || []).map(e => e.new_string).join("\n");
  if (!written) return;
  const lines = written.split("\n").filter(l => /^\s*\/\/[/!]?(\s|$)/.test(l));
  // sentences: a run of comment lines up to one ending in . : ? ! or a blank
  const sents = []; let cur = []; let fence = false;
  for (const l of lines) { const b = body(l); if (/^```/.test(b)) { fence = !fence; cur.push(l); if (!fence) { sents.push(cur); cur = []; } continue; } if (fence) { cur.push(l); continue; } if (b.trim() === "") { if (cur.length) { sents.push(cur); cur = []; } continue; } cur.push(l); if (ends(l)) { sents.push(cur); cur = []; } }
  if (cur.length) sents.push(cur);
  const bare = sents.filter(s => !s.some(l => EVIDENCE.test(l)));
  if (!bare.length) return;
  const shown = bare.slice(0, 4).map(s => "  " + body(s[0]).slice(0, 110)).join("\n");
  process.stdout.write(`comment-lint: ${rel}: ${bare.length} comment sentence(s) carry no evidence (rule 7: the function and address, an offset, a decompilation line, a [V]/[I]/[D] with its reason, a C-number, the ablation a test names). The code says the rest; delete them, like\n  /// \`Turn_End\` (\`0x0043AC23\`) writes 999 when the person presses the button.\n${shown}\n`);
});
