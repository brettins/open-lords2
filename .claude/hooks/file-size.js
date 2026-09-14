// PostToolUse on Edit|Write: a Rust file over the cap gets a line back to the agent
// naming the split command. Caps: 300 lines for a test file, 500 for a source; the
// player's target for any new file is 300. Silent under the cap.
const fs = require("fs"), path = require("path");
let input = ""; process.stdin.on("data", d => input += d).on("end", () => {
  let file; try { file = JSON.parse(input).tool_input?.file_path; } catch { return; }
  if (!file || !/[.]rs$/.test(file) || !fs.existsSync(file)) return;
  const root = process.env.CLAUDE_PROJECT_DIR || process.cwd();
  const rel = path.relative(root, file).split(path.sep).join("/");
  if (rel.startsWith("..")) return;
  const lines = fs.readFileSync(file, "utf8").split("\n").length;
  const cap = /\/tests\//.test(rel) ? 300 : 500;
  if (lines <= cap) return;
  process.stdout.write(`file-size: ${rel} is ${lines} lines (cap ${cap}). Split it before reporting: node tools/review/split-llm.js ${rel}, then node tools/review/widen.js <crate> crates/<crate> until cargo check is clean. New files stay under 300.\n`);
});
