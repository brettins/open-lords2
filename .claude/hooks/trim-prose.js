// PostToolUse on Edit|Write: the file an agent just wrote gets its filler cut by
// tools/review/prose-llm.js (one Gemini Flash call, delete-only). The player's
// rule: the plainest listing of facts, and losing a little is the accepted price.
// Silent when there is no key, no hit, or the file is not a doc or a Rust source.
const path = require("path"), cp = require("child_process");
let input = ""; process.stdin.on("data", d => input += d).on("end", () => {
  let file; try { file = JSON.parse(input).tool_input?.file_path; } catch { return; }
  if (!file || !process.env.GEMINI_API_KEY) return;
  const root = process.env.CLAUDE_PROJECT_DIR || process.cwd();
  const rel = path.relative(root, file).split(path.sep).join("/");
  if (rel.startsWith("..") || !/[.](md|rs)$/.test(rel) || rel === "docs/decisions.md") return;
  const r = cp.spawnSync("node", ["tools/review/prose-llm.js", rel], { cwd: root, encoding: "utf8", timeout: 60000 });
  if (r.stdout && / -> /.test(r.stdout)) process.stdout.write("prose: " + r.stdout.trim() + "\n");
});
