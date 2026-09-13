// Everything the existing tools know about one function, in one block.
//
//   node tools/oracle/dossier.js Map_DrawArmies
//   node tools/oracle/dossier.js 0x00408438
//
// Needs tools/oracle/out/xref.json (xref.js build) and $LORDS2_DIR.

const fs = require('fs');
const path = require('path');
const cp = require('child_process');

const ROOT = path.resolve(__dirname, '..', '..');
const XREF = path.join(ROOT, 'tools', 'oracle', 'out', 'xref.json');

// ---- crates/ scan, shared with uncited.js
function crateFiles() {
  const out = [];
  (function walk(d) {
    let ents;
    try { ents = fs.readdirSync(d, { withFileTypes: true }); } catch (e) { return; }
    for (const e of ents) {
      if (e.name === 'target' || e.name === '.git') continue;
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (/\.(rs|toml)$/.test(e.name)) out.push(p);
    }
  })(path.join(ROOT, 'crates'));
  return out;
}

function scanCrates(re) {
  const hits = [];
  for (const f of crateFiles()) {
    const lines = fs.readFileSync(f, 'utf8').split(/\r?\n/);
    for (let i = 0; i < lines.length; i++) {
      re.lastIndex = 0;
      if (re.test(lines[i])) hits.push({ file: path.relative(ROOT, f).replace(/\\/g, '/'), line: i + 1, text: lines[i].trim() });
    }
  }
  return hits;
}

module.exports = { crateFiles, scanCrates, ROOT };
if (require.main !== module) return;

// ---- resolve the subject
const arg = process.argv[2];
if (!arg) { console.error('usage: node tools/oracle/dossier.js <addr|name>'); process.exit(2); }

const xref = JSON.parse(fs.readFileSync(XREF, 'utf8')).functions;
const syms = JSON.parse(fs.readFileSync(path.join(ROOT, 'docs', 'symbols.json'), 'utf8'));

const hex = /^(0x)?[0-9a-fA-F]{6,8}$/.test(arg) ? arg.replace(/^0x/, '').toLowerCase().padStart(8, '0') : null;

let name = null;
if (hex) {
  for (const [k, v] of Object.entries(xref)) if (v.addr === hex) { name = k; break; }
  if (!name) { const s = syms.functions.find(f => f.addr.toLowerCase().replace('0x', '') === hex); if (s) name = s.name; }
} else {
  name = xref[arg] ? arg : null;
  if (!name) { const s = syms.functions.find(f => f.name === arg); if (s) name = arg; }
}
if (!name) { console.error('not found: ' + arg); process.exit(1); }

const fn = xref[name] || {};
const addr = (fn.addr || (syms.functions.find(f => f.name === name) || {}).addr || '').replace(/^0x/, '').toLowerCase();
const sym = syms.functions.find(f => f.addr.toLowerCase().replace('0x', '') === addr);

const sh = (cmd) => { try { return cp.execSync(cmd, { cwd: ROOT, encoding: 'utf8', maxBuffer: 64 << 20, stdio: ['ignore', 'pipe', 'ignore'] }); } catch (e) { return (e.stdout || '') + ''; } };

// ---- header
console.log(`${name}  0x${addr}  ${fn.file || '?'}  ${fn.bytes || '?'}b  ${fn.lines || '?'} lines  params ${fn.params != null ? fn.params : '?'}`);
if (sym) console.log(`  symbols.json  section ${sym.section}  ${sym.confidence}  ${sym.signature || ''}`);

const list = (title, arr) => {
  console.log(`\n${title} (${arr.length})`);
  for (const a of arr) console.log('  ' + a);
};

list('callers', fn.callers || []);
list('calls', fn.calls || []);
list('globals', fn.globals || []);

// ---- anchor.js: eng groups and record stride
const strings = sh('node tools/oracle/anchor.js strings');
const blocks = strings.split(/\n(?=\S)/);
const engBlock = blocks.find(b => b.split(/\s+/)[0] === name);
console.log('\neng groups');
if (engBlock) console.log(engBlock.replace(/^\S+.*\n/, '').replace(/\n+$/, ''));
else console.log('  none');

const stride = sh('node tools/oracle/anchor.js stride');
const strideLines = stride.split(/\r?\n/).filter(l => l.trim().split(/\s+/)[0] === name);
console.log('\nrecord stride');
console.log(strideLines.length ? strideLines.map(l => '  ' + l.trim()).join('\n') : '  none');

// ---- logstrings.js
const logs = sh('node tools/oracle/logstrings.js');
const logBlocks = logs.split(/\n(?=[* ] 0x)/);
const logBlock = logBlocks.find(b => b.includes(' ' + name + ' '));
console.log('\nlog strings');
console.log(logBlock ? logBlock.replace(/\n+$/, '') : '  none');

// ---- widgets.js: tables pointing here
const w = sh(`node tools/oracle/widgets.js ref ${addr.replace(/^0+/, '')}`).trim();
console.log('\nwidget table refs');
console.log(w ? w.split(/\r?\n/).map(l => '  ' + l.trim()).join('\n') : '  none');

// ---- crates/
const short = addr.replace(/^0+/, '');
const re = new RegExp(`0x0*${short}\\b|\\b${name}\\b`, 'i');
const hits = scanCrates(re);
console.log(`\ncrates cites (${hits.length})`);
for (const h of hits) console.log(`  ${h.file}:${h.line}  ${h.text.slice(0, 140)}`);
