// coverage.js - every function address the docs name, checked against the
// whole-binary decompilation that already exists on disk.
//
// docs/battle-ai.md §9 says "Battlefield_BuildCastle was not decompiled, so
// every siege claim in section 6 rests on that". docs/audit.md section 8 says
// Ghidra-derived claims could not be re-checked because "running Ghidra would
// have contended for the project lock". Both are statements about a tooling
// era that ended when tools/oracle/decompile-all.ps1 landed. This measures how
// much of the docs' "unread code" is in fact sitting in tools/oracle/decomp/.
const fs = require('fs'), path = require('path');
const ROOT = 'E:/dev/lords2';
const idx = fs.readFileSync(path.join(ROOT, 'tools/oracle/decomp/index.txt'), 'utf8')
  .split('\n').filter(l => l && !l.startsWith('#'));
const byAddr = new Map(), names = new Map();
for (const l of idx) {
  const m = l.match(/^([0-9a-f]{8})\s+(\S+)\s+(\d+)\s+(\d+)\s+([0-9a-f]{8})/);
  if (m) { byAddr.set(m[1], { name: m[2], bytes: +m[3], bucket: m[5] }); names.set(m[2], m[1]); }
}
// text of every doc
const docs = [];
(function walk(d) {
  for (const f of fs.readdirSync(d)) {
    const p = path.join(d, f);
    if (fs.statSync(p).isDirectory()) walk(p);
    else if (/\.(md|html)$/.test(f)) docs.push(p);
  }
})(path.join(ROOT, 'docs'));

const hits = new Map();   // addr -> Set of "file:line"
for (const p of docs) {
  const lines = fs.readFileSync(p, 'utf8').split('\n');
  lines.forEach((l, i) => {
    for (const m of l.matchAll(/0x00([0-9A-Fa-f]{6})\b/g)) {
      const a = ('00' + m[1]).toLowerCase();
      if (byAddr.has(a)) {
        if (!hits.has(a)) hits.set(a, []);
        hits.get(a).push(`${path.relative(ROOT, p)}:${i + 1}`);
      }
    }
  });
}
console.log(`decomp index: ${byAddr.size} functions`);
console.log(`docs scanned: ${docs.length} files`);
console.log(`distinct function entry points named in docs and present in decomp: ${hits.size}`);
const named = [...byAddr.values()].filter(v => !v.name.startsWith('FUN_')).length;
console.log(`decomp functions carrying a real (symbols.json) name: ${named} / ${byAddr.size}`);

// mode 2: check specific names given on the command line
if (process.argv[2] === 'check') {
  console.log('\nfunctions the docs call untraced / undecompiled:');
  for (const n of process.argv.slice(3)) {
    const a = names.get(n) || (byAddr.has(n.toLowerCase()) ? n.toLowerCase() : null);
    if (a) {
      const e = byAddr.get(a);
      console.log(`  PRESENT  ${e.name.padEnd(30)} 0x${a}  ${String(e.bytes).padStart(5)} bytes  -> tools/oracle/decomp/${e.bucket}.c`);
    } else console.log(`  ABSENT   ${n}`);
  }
}
