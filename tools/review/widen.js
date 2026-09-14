// widen.js - after a module split, make the items rustc calls private `pub(super)`.
//
//   node tools/review/widen.js <crate> <dir>      e.g. l2-sim crates/l2-sim/src/runner
//
// Runs `cargo check -p <crate> --tests`, reads E0624 (private method), E0603 (private
// item) and E0616 (private field), widens each definition inside <dir>, repeats until
// the check is clean or nothing changes. The one thing a mechanical move cannot decide.
const fs = require("fs"), path = require("path"), cp = require("child_process");
const [crate, dir] = process.argv.slice(2); if (!crate || !dir) { console.error("widen: <crate> <dir>"); process.exit(2); }
const root = path.resolve(__dirname, "..", "..");
const files = () => fs.readdirSync(path.join(root, dir)).filter(f => f.endsWith(".rs")).map(f => path.join(root, dir, f));
for (let round = 0; round < 8; round++) {
  const out = cp.spawnSync("cargo", ["check", "-p", crate, "--tests"], { cwd: root, encoding: "utf8" });
  const text = (out.stdout || "") + (out.stderr || "");
  const names = new Set();
  for (const m of text.matchAll(/error\[E0624\]: (?:method|associated function) `(\w+)` is private/g)) names.add(["fn", m[1]]);
  for (const m of text.matchAll(/error\[E0603\]: (?:function|struct|enum|constant|type alias|module|static) `(\w+)` is private/g)) names.add(["item", m[1]]);
  for (const m of text.matchAll(/error\[E0616\]: field `(\w+)` of struct `(\w+)` is private/g)) names.add(["field", m[1]]);
  const errors = (text.match(/^error/gm) || []).length;
  if (!names.size) { console.log(`widen: round ${round}, ${errors} error(s) left, none about visibility`); process.exit(errors ? 1 : 0); }
  let changed = 0;
  for (const [kind, name] of names) for (const f of files()) {
    let s = fs.readFileSync(f, "utf8"), t = s;
    if (kind === "fn") t = s.replace(new RegExp(`^(\\s*)fn ${name}\\b`, "m"), `$1pub(super) fn ${name}`);
    else if (kind === "item") t = s.replace(new RegExp(`^(\\s*)(fn|struct|enum|const|type|static|mod) ${name}\\b`, "m"), `$1pub(super) $2 ${name}`);
    else t = s.replace(new RegExp(`^(\\s{4})${name}:`, "m"), `$1pub(super) ${name}:`);
    if (t !== s) { fs.writeFileSync(f, t); changed++; }
  }
  console.log(`widen: round ${round}, ${names.size} private name(s), ${changed} widened`);
  if (!changed) process.exit(1);
}
