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
// Every .rs under <dir>, recursively: <dir> may be one split module or a whole crate's src.
const files = () => { const out = []; const walk = d => { for (const e of fs.readdirSync(d, { withFileTypes: true })) { const p = path.join(d, e.name); if (e.isDirectory()) walk(p); else if (e.name.endsWith(".rs")) out.push(p); } }; walk(path.join(root, dir)); return out; };
for (let round = 0; round < 8; round++) {
  const out = cp.spawnSync("cargo", ["check", "-p", crate, "--tests", "--message-format=short"], { cwd: root, encoding: "utf8", maxBuffer: 64 << 20 });
  const text = (out.stdout || "") + (out.stderr || "");
  const names = new Set();
  for (const m of text.matchAll(/error\[E0624\]: (?:method|associated function) `(\w+)` is private/g)) names.add(["fn", m[1]]);
  for (const m of text.matchAll(/error\[E0624\]: associated (?:constant|type) `(\w+)` is private/g)) names.add(["item", m[1]]);
  // A widened fn whose signature names a narrower type (E0446): the type widens too.
  for (const m of text.matchAll(/error(?:\[E0446\])?: type `(\w+)` is private/g)) names.add(["item", m[1]]);
  for (const m of text.matchAll(/error\[E0603\]: (?:function|struct|enum|constant|type alias|module|static) `(\w+)` is private/g)) names.add(["item", m[1]]);
  for (const m of text.matchAll(/error\[E0616\]: field `(\w+)` of struct `([\w:]+)` is private/g)) names.add(["field", m[2].split("::").pop() + "." + m[1]]);
  // A struct literal naming private fields: every backticked name before "of struct".
  for (const m of text.matchAll(/error\[E0451\]: fields? (.*?) of struct `([\w:]+)` (?:is|are) private/g)) for (const f of m[1].matchAll(/`(\w+)`/g)) names.add(["field", m[2].split("::").pop() + "." + f[1]]);
  // A submodule that kept the parent's `use` lines and got them again from the prelude:
  // the second copy goes (E0252 names the line).
  for (const m of text.matchAll(/^(crates[^:\n]+\.rs):(\d+):(\d+): error\[E0252\]: the name `(\w+)` is defined multiple times/gm)) names.add(["dupuse", m[1].replace(/\\/g, "/") + ":" + m[2] + ":" + m[3] + ":" + m[4]]);
  // A private item of a sibling is "not found" rather than "private" through a glob.
  for (const m of text.matchAll(/error\[E04(?:22|25)\]: cannot find (?:value|function|type|struct, variant or union type) `(\w+)` in this scope/g)) names.add(["item", m[1]]);
  for (const m of text.matchAll(/error\[E04(?:12|33)\]: cannot find type `(\w+)` in this scope/g)) names.add(["item", m[1]]);
  for (const m of text.matchAll(/error\[E0433\]: cannot find module or crate `(\w+)` in this scope/g)) names.add(["mod", m[1]]);
  // A private sibling fn hidden behind a builtin macro of the same name (`file`).
  for (const m of text.matchAll(/error\[E0423\]: expected function, found macro `(\w+)`/g)) names.add(["fn", m[1]]);
  // A qualifier that landed on a trait method (E0449) comes off again.
  for (const m of text.matchAll(/^(crates[^:\n]+\.rs):(\d+):\d+: error\[E0449\]/gm)) names.add(["unqualify", m[1].replace(/\\/g, "/") + ":" + m[2]]);
  // `super::x` from a file that moved one level deeper: the path gains a `super::`.
  for (const m of text.matchAll(/^(crates[^:\n]+\.rs):(\d+):(\d+): error\[E0433\]: (?:failed to resolve: )?(?:could not find|cannot find) `(\w+)` in `super`/gm)) names.add(["deeper", m[1].replace(/\\/g, "/") + ":" + m[2] + ":" + m[3] + ":" + m[4]]);
  const errors = (text.match(/^(crates[^:\n]+:\d+:\d+: )?error/gm) || []).length;
  if (!names.size) { console.log(`widen: round ${round}, ${errors} error(s) left, none about visibility`); process.exit(errors ? 1 : 0); }
  let changed = 0;
  // Only the token at the reported column leaves the `use` line: one element of a braced
  // list, or the whole line for a bare path.
  const dups = {}; for (const [kind, name] of names) if (kind === "dupuse") { const [f, l, c] = name.split(":"); (dups[f] = dups[f] || {})[l] = (dups[f][l] || []).concat(Number(c)); }
  for (const f of Object.keys(dups)) {
    const p = path.resolve(root, f); if (!fs.existsSync(p)) continue; const ls = fs.readFileSync(p, "utf8").split("\n"); let n = 0;
    for (const l of Object.keys(dups[f])) { const s = ls[l - 1]; if (!/^\s*(pub(\([a-z]+\))? )?use .*;\s*$/.test(s || "")) continue;
      const br = s.match(/\{([^}]*)\}/);
      if (br) { let at = s.indexOf("{") + 1; const parts = []; for (const raw of br[1].split(",")) { const tok = raw.trim(); const col = at + raw.indexOf(tok) + 1; at += raw.length + 1; if (tok && !dups[f][l].includes(col)) parts.push(tok); }
        ls[l - 1] = parts.length ? s.replace(/\{[^}]*\}/, parts.length === 1 ? parts[0] : `{${parts.join(", ")}}`) : null; }
      else ls[l - 1] = null;
      n++; }
    if (n) { fs.writeFileSync(p, ls.filter(x => x !== null).join("\n")); changed++; } }
  for (const [kind, name] of names) if (kind === "unqualify") {
    const [f, l] = name.split(":"); const p = path.resolve(root, f); if (!fs.existsSync(p)) continue;
    const ls = fs.readFileSync(p, "utf8").split("\n"); const s = ls[l - 1] || ""; const t = s.replace(/^(\s*)pub\((?:super|crate)\) /, "$1");
    if (t !== s) { ls[l - 1] = t; fs.writeFileSync(p, ls.join("\n")); changed++; }
  }
  for (const [kind, name] of names) if (kind === "deeper") {
    const [f, l, c, id] = name.split(":"); const p = path.resolve(root, f); if (!fs.existsSync(p)) continue;
    const ls = fs.readFileSync(p, "utf8").split("\n"); const s = ls[l - 1] || ""; const at = Number(c) - 1;
    if (s.slice(at, at + id.length) === id && s.slice(at - 7, at) === "super::") { ls[l - 1] = s.slice(0, at) + "super::" + s.slice(at); fs.writeFileSync(p, ls.join("\n")); changed++; }
  }
  for (const [kind, name] of names) for (const f of files()) {
    if (kind === "dupuse" || kind === "deeper" || kind === "unqualify") continue;
    let s = fs.readFileSync(f, "utf8"), t = s;
    // A name already pub(super) that rustc still calls private is used by a cousin: pub(crate).
    // A crate root (main.rs, lib.rs) has no super: pub(crate) from the start.
    const rootFile = /^(main|lib)\.rs$/.test(path.basename(f)) || /[\\/]tests[\\/][^\\/]+\.rs$/.test(f);
    const vis = ps => (ps || rootFile) ? "pub(crate)" : "pub(super)";
    if (kind === "mod") t = s.replace(new RegExp(`^(\\s*)(pub\\(super\\) )?mod ${name}\\b`, "m"), (m, ind, ps) => `${ind}${vis(ps)} mod ${name}`);
    else if (kind === "fn") t = s.replace(new RegExp(`^(\\s*)(pub\\(super\\) )?fn ${name}\\b`, "m"), (m, ind, ps) => `${ind}${vis(ps)} fn ${name}`);
    else if (kind === "item") t = s.replace(new RegExp(`^(\\s*)(pub\\(super\\) )?(fn|struct|enum|const|type|static|mod|trait) ${name}\\b`, "m"), (m, ind, ps, kw) => `${ind}${vis(ps)} ${kw} ${name}`);
    else { const [st, fld] = name.split("."); const i = s.search(new RegExp(`^(pub(\\([a-z]+\\))? )?struct ${st}\\b[^;{]*\\{`, "m")); if (i >= 0) { const end = s.indexOf("\n}", i); const body = s.slice(i, end).replace(new RegExp(`^(\\s{4})(pub\\(super\\) )?${fld}:`, "m"), (m, ind, ps) => `${ind}${vis(ps)} ${fld}:`); t = s.slice(0, i) + body + s.slice(end); } }
    if (t !== s) { fs.writeFileSync(f, t); changed++; }
  }
  console.log(`widen: round ${round}, ${names.size} private name(s), ${changed} widened`);
  if (!changed) process.exit(1);
}
