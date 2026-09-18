// Named functions that no line in crates/ cites, by symbols.md section.
//
//   node tools/oracle/uncited.js
//   node tools/oracle/uncited.js --json

const fs = require('fs');
const path = require('path');
const { crateFiles, ROOT } = require('./dossier.js');

const syms = JSON.parse(fs.readFileSync(path.join(ROOT, 'docs', 'symbols.json'), 'utf8'));

// section key -> the heading it sits under in symbols.md
const md = fs.readFileSync(path.join(ROOT, 'docs', 'symbols.md'), 'utf8').split(/\r?\n/);
const heading = {};
let cur = '(no heading)';
for (const l of md) {
  const h = /^#{1,3}\s+(.*)/.exec(l);
  if (h) cur = h[1].trim();
  const b = /<!--\s*BEGIN symbols\.json:\s*(\S+)\s*-->/.exec(l);
  if (b) heading[b[1]] = cur;
}

// one pass over crates/: collect every 0x004xxxxx / 0x005xxxxx and every identifier
const addrs = new Set();
const words = new Set();
const addrRe = /0x00[45][0-9a-fA-F]{5}/g;
const wordRe = /[A-Za-z_][A-Za-z0-9_]*/g;
for (const f of crateFiles()) {
  const t = fs.readFileSync(f, 'utf8');
  let m;
  while ((m = addrRe.exec(t))) addrs.add(m[0].toLowerCase());
  while ((m = wordRe.exec(t))) words.add(m[0]);
}

const fns = syms.functions.filter(f => f.name && !/^FUN_/i.test(f.name));
const uncited = [];
for (const f of fns) {
  const a = f.addr.toLowerCase();
  if (addrs.has(a)) continue;
  if (words.has(f.name)) continue;
  uncited.push(f);
}

if (process.argv.includes('--json')) {
  console.log(JSON.stringify({
    total: fns.length,
    uncited: uncited.length,
    functions: uncited.map(f => ({ addr: f.addr, name: f.name, section: f.section, heading: heading[f.section] || f.section }))
  }, null, 1));
  return;
}

const groups = new Map();
for (const f of uncited) {
  const h = heading[f.section] || f.section;
  if (!groups.has(h)) groups.set(h, []);
  groups.get(h).push(f);
}
for (const [h, list] of [...groups].sort((a, b) => b[1].length - a[1].length)) {
  console.log(`\n${h}  (${list.length})`);
  for (const f of list.sort((a, b) => a.addr.localeCompare(b.addr))) {
    console.log(`  ${f.addr}  ${f.name}${f.confidence ? '  ' + f.confidence : ''}`);
  }
}
console.log(`\nuncited ${uncited.length} of ${fns.length}`);
