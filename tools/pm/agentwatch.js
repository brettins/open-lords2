// What each agent in a task directory actually spent its turns on.
//
//   node tools/pm/agentwatch.js <dir>
//   node tools/pm/agentwatch.js <dir> --sort tokens
//
// <dir> holds the harness's *.output files (JSONL, one record per line).
// Columns: cargo test invocations, distinct decomp/*.c files opened,
// unbounded Read calls (no limit, or limit > 500) out of all Reads, and the
// final <subagent_tokens>.

const fs = require('fs');
const path = require('path');

const dir = process.argv[2];
if (!dir) { console.error('usage: node tools/pm/agentwatch.js <dir> [--sort tokens|cargo|decomp|reads|name]'); process.exit(2); }

const sortKey = (() => { const i = process.argv.indexOf('--sort'); return i > 0 ? process.argv[i + 1] : 'name'; })();

const CARGO = /\bcargo\s+(\+\S+\s+)?test\b/g;
const DECOMP = /decomp[\\/]+([0-9A-Fa-f]{8}\.c)/g;
const TOKENS = /<subagent_tokens>(\d+)<\/subagent_tokens>/g;

const rows = [];
for (const f of fs.readdirSync(dir).filter(n => n.endsWith('.output'))) {
  const text = fs.readFileSync(path.join(dir, f), 'utf8');
  let cargo = 0, reads = 0, bigReads = 0;
  const decomp = new Set();

  for (const line of text.split('\n')) {
    if (!line) continue;
    let o; try { o = JSON.parse(line); } catch (e) { continue; }
    const c = o.message && o.message.content;
    if (!Array.isArray(c)) continue;
    for (const p of c) {
      if (p.type !== 'tool_use') continue;
      const s = JSON.stringify(p.input || {});
      const cmd = (p.input && p.input.command) || '';
      CARGO.lastIndex = 0;
      while (CARGO.exec(cmd)) cargo++;
      DECOMP.lastIndex = 0;
      let m; while ((m = DECOMP.exec(s))) decomp.add(m[1].toLowerCase());
      if (p.name === 'Read') {
        reads++;
        const lim = p.input && p.input.limit;
        if (lim == null || lim > 500) bigReads++;
      }
    }
  }

  TOKENS.lastIndex = 0;
  let tok = null, m;
  while ((m = TOKENS.exec(text))) tok = Number(m[1]);

  rows.push({ name: f.replace(/\.output$/, ''), cargo, decomp: decomp.size, bigReads, reads, tok });
}

const key = { tokens: r => -(r.tok || 0), cargo: r => -r.cargo, decomp: r => -r.decomp, reads: r => -r.bigReads }[sortKey];
rows.sort(key ? (a, b) => key(a) - key(b) : (a, b) => a.name.localeCompare(b.name));

for (const r of rows) {
  console.log(
    `${r.name}  cargo-test ${String(r.cargo).padStart(3)}  decomp ${String(r.decomp).padStart(3)}` +
    `  unbounded-read ${String(r.bigReads).padStart(3)}/${String(r.reads).padEnd(3)}` +
    `  tokens ${r.tok == null ? '-' : r.tok}`
  );
}
const sum = k => rows.reduce((a, r) => a + (r[k] || 0), 0);
console.log(`\n${rows.length} agents  cargo-test ${sum('cargo')}  decomp ${sum('decomp')}` +
  `  unbounded-read ${sum('bigReads')}/${sum('reads')}  tokens ${sum('tok')}`);
