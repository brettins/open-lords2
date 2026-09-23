// node tools/review/prune-imports.js <crate>: the step after split-llm.js + widen.js. Removes
// the imports rustc reports unused, the copies of the parent's `use` lines a split leaves
// behind. `cargo fix` breaks the test build here (it drops imports only #[cfg(test)] code
// uses), so: an import unused in the lib test build (or in an integration test) is removed;
// one unused only in the plain lib build gets `#[cfg(test)]`; `pub use x::*` that rustc says
// reexports nothing pub becomes `use x::*`. Then `cargo build -p <crate>` and
// `cargo test -p <crate> --no-run` are the caller's.
const { execSync } = require("child_process");
const fs = require("fs");
const crate = process.argv[2];
if (!crate) { console.error("usage: prune-imports.js <crate>"); process.exit(1); }
const run = args => { try { return execSync(`cargo ${args} -p ${crate} --message-format=json`, { maxBuffer: 1 << 30, stdio: ["ignore", "pipe", "ignore"] }).toString(); } catch (e) { return e.stdout.toString(); } };
const parse = out => out.split("\n").filter(l => l.startsWith("{")).map(l => JSON.parse(l));
const mine = m => m.reason === "compiler-message" && m.message.level === "warning" && m.package_id.includes(`/${crate}#`);
const unused = m => /^unused imports?:/.test(m.message.message);
const key = m => m.message.spans.map(s => `${s.file_name}|${s.byte_start}-${s.byte_end}`).join(",");
const suggestions = m => m.message.children.flatMap(c => c.spans.filter(s => s.suggested_replacement !== null));

function pass() {
  const libOut = parse(run("check --lib")), testOut = parse(run("test --lib --no-run"));
  const A = libOut.filter(m => mine(m) && unused(m));
  // A dev-dependency cycle builds the plain lib inside the test run too: subtract its messages.
  let B = testOut.filter(m => mine(m) && unused(m));
  if (testOut.some(m => m.reason === "compiler-artifact" && m.package_id.includes(`/${crate}#`) && m.target.kind.includes("lib") && !m.profile.test)) {
    const n = {}; for (const m of A) n[key(m)] = (n[key(m)] || 0) + 1;
    B = B.filter(m => n[key(m)] > 0 ? (n[key(m)]--, false) : true);
  }
  const cOut = parse(run("check --tests")).filter(m => m.reason !== "compiler-message" || m.target.kind.includes("test"));
  const C = cOut.filter(m => mine(m) && unused(m));
  const inB = new Set(B.map(key)), libOnly = A.filter(m => !inB.has(key(m)));
  const ops = {}, taken = {};
  const add = (f, a, b, r) => (ops[f] ||= []).push([a, b, r]);
  // Two warnings on one `use` (a file compiled twice) disagree on braces: one per pass.
  const claim = (f, lo, hi) => { const t = (taken[f] ||= []); if (t.some(([a, b]) => lo < b && a < hi)) return false; t.push([lo, hi]); return true; };
  for (const m of [...B, ...C]) {
    const ss = suggestions(m).filter(s => s.suggestion_applicability === "MachineApplicable");
    if (!ss.length || !claim(ss[0].file_name, Math.min(...ss.map(s => s.byte_start)), Math.max(...ss.map(s => s.byte_end)))) continue;
    for (const s of ss) add(s.file_name, s.byte_start, s.byte_end, s.suggested_replacement);
  }
  for (const m of libOnly) {
    const ps = m.message.spans, f = ps[0].file_name, text = fs.readFileSync(f).toString("latin1");
    const s0 = Math.min(...ps.map(s => s.byte_start)), u = text.lastIndexOf("use ", s0);
    const st = text.lastIndexOf("\n", u) + 1, en = text.indexOf(";", s0) + 1, stmt = text.slice(st, en), indent = stmt.match(/^\s*/)[0];
    if (!claim(f, st, en)) continue;
    const ss = suggestions(m);
    if (ss.length === 1 && ss[0].byte_start <= st + indent.length && ss[0].byte_end >= en) { add(f, st, st, `${indent}#[cfg(test)]\n`); continue; }
    const pm = stmt.match(/^\s*(?:pub(?:\([^)]*\))? )?use ([\w:]+)::\{([^{}]*)\};$/s);
    if (!pm) { console.log(`prune-imports: by hand, ${f}: ${stmt.replace(/\s+/g, " ")}`); continue; }
    for (const s of ss) add(f, s.byte_start, s.byte_end, s.suggested_replacement);
    add(f, en, en, `\n${indent}#[cfg(test)]\n${indent}use ${pm[1]}::{${ps.map(s => text.slice(s.byte_start, s.byte_end)).join(", ")}};`);
  }
  for (const m of [...libOut, ...testOut, ...cOut].filter(m => mine(m) && /^glob import doesn't reexport/.test(m.message.message))) {
    const s = m.message.spans[0], text = fs.readFileSync(s.file_name).toString("latin1");
    if (text.slice(s.byte_start - 8, s.byte_start) === "pub use " && claim(s.file_name, s.byte_start - 8, s.byte_start)) add(s.file_name, s.byte_start - 8, s.byte_start, "use ");
  }
  let n = 0;
  for (const [f, list] of Object.entries(ops)) {
    let buf = fs.readFileSync(f);
    list.sort((a, b) => b[0] - a[0] || b[1] - a[1]);
    for (const [a, b, r] of list) { buf = Buffer.concat([buf.subarray(0, a), Buffer.from(r, "latin1"), buf.subarray(b)]); n++; }
    fs.writeFileSync(f, buf);
  }
  return n;
}

let total = 0;
for (let i = 0; i < 4; i++) { const n = pass(); total += n; if (!n) break; }
console.log(`prune-imports: ${total} edits in ${crate}`);
