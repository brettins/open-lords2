// docs/symbols.json is the single source of truth for our names in Lords2.exe.
// This script writes the tables in docs/symbols.md from it, or checks they match.
//
//   node tools/symbols/symbols_md.js            rewrite the generated blocks in docs/symbols.md
//   node tools/symbols/symbols_md.js --check    exit 1 if docs/symbols.md is out of date
//
// Prose stays in symbols.md; only the regions between
//   <!-- BEGIN symbols.json: <section id> -->  ...  <!-- END symbols.json: <section id> -->
// are generated. Every section declared in symbols.json must have such a region.

const fs = require('path') && require('fs');
const path = require('path');

const repo = path.resolve(__dirname, '..', '..');
const jsonPath = path.join(repo, 'docs', 'symbols.json');
const mdPath = path.join(repo, 'docs', 'symbols.md');
const check = process.argv.includes('--check');

const data = JSON.parse(fs.readFileSync(jsonPath, 'utf8'));

const fail = m => { console.error('symbols_md: ' + m); process.exit(1); };

// ---- sanity checks on the data file itself -------------------------------
{
  const seenAddr = new Map(), seenName = new Map();
  for (const kind of ['functions', 'globals'])
    for (const e of data[kind] || []) {
      for (const k of ['addr', 'name', 'section', 'confidence'])
        if (!e[k]) fail(`${kind} entry ${e.addr || e.name || '?'} is missing "${k}"`);
      if (!/^0x[0-9A-F]{8}$/.test(e.addr))
        fail(`${e.name}: addr "${e.addr}" must be 0x + 8 upper-case hex digits`);
      if (!['verified', 'inferred'].includes(e.confidence))
        fail(`${e.name}: confidence must be "verified" or "inferred"`);
      if (!data.sections.some(s => s.id === e.section))
        fail(`${e.name}: unknown section "${e.section}"`);
      if (seenAddr.has(e.addr)) fail(`address ${e.addr} used twice (${seenAddr.get(e.addr)}, ${e.name})`);
      if (seenName.has(e.name)) fail(`name ${e.name} used twice (${seenName.get(e.name)}, ${e.addr})`);
      seenAddr.set(e.addr, e.name); seenName.set(e.name, e.addr);

      // ---- the `signature` field must agree with the array it sits in -------
      //
      // `ghidra_scripts/ApplySymbols.java` feeds this string to Ghidra's C
      // parser and applies the result at the entry's address. When the string
      // does not parse, or parses as something that is not a function, the
      // entry is **dropped on every rebuild** — the name never reaches the
      // corpus, `ApplySymbols` prints one line, and the pipeline as a whole
      // still reports success. Two entries did that on two consecutive days:
      //
      //   void __cdecl Setup_SetOption(int, int)   - a calling convention,
      //                                              which the parser rejects
      //   int g_goodsStall[14][5]                  - a data table filed under
      //                                              `functions`
      //
      // Both were found by an integrator reading output nobody is required to
      // read. These four rules catch that whole class here instead, before
      // Ghidra is involved, and they run anywhere.
      if (kind === 'globals' && e.signature)
        fail(`${e.name} (${e.addr}) is a global and carries a "signature".\n`
          + `  A signature is a function's field. If this is really a function, move the entry\n`
          + `  to "functions"; if it is data, put its shape in the comment instead.`);
      if (kind === 'functions' && e.signature) {
        const sig = e.signature;
        if (!/\(.*\)/.test(sig))
          fail(`${e.name} (${e.addr}): signature has no parameter list.\n`
            + `      ${sig}\n`
            + `  Ghidra applies this as a function prototype, so it needs "(...)" - use "(void)"\n`
            + `  for one that takes nothing. If this entry is data, move it to "globals".`);
        if (/\[\s*\d*\s*\]/.test(sig))
          fail(`${e.name} (${e.addr}): signature declares an array, so it is data, not a function.\n`
            + `      ${sig}\n`
            + `  Move the entry to "globals", drop the "signature" field, and put the shape at the\n`
            + `  front of the comment. ApplySymbols would otherwise look for a function at this\n`
            + `  address, find data, and silently drop the name on every rebuild.`);
        const cc = sig.match(/__(cdecl|stdcall|fastcall|thiscall)/);
        if (cc)
          fail(`${e.name} (${e.addr}): signature carries the calling convention "__${cc[1]}".\n`
            + `      ${sig}\n`
            + `  Ghidra's C parser refuses it there ("Can't resolve return type") and drops the\n`
            + `  whole signature. Delete "__${cc[1]}" - the convention is Ghidra's to infer, and it\n`
            + `  re-emits it in the decompiled corpus by itself.`);
        if (!sig.includes(e.name))
          fail(`${e.name} (${e.addr}): signature does not name its own symbol.\n`
            + `      ${sig}\n`
            + `  ApplySymbols matches the declared name against the entry; a signature naming\n`
            + `  something else is applied to the wrong thing or not at all.`);
      }
    }

  // ---- and on docs/hypotheses.json, which is the other half of the same rule.
  //
  // Two things kept going wrong by hand and are now checked here, because this
  // script is what CI already runs on every push:
  //
  //  * `confidence` in the hypothesis file is an ENUM. Three agents created that
  //    file on one day and 29 of 65 entries carried a paragraph in the key
  //    instead, which makes it unreadable by anything mechanical. See
  //    docs/method.md 7.6.
  //  * PROMOTION IS A MOVE, NOT A COPY. An address verified in symbols.json must
  //    not also sit in hypotheses.json — a name in both is a name whose tier
  //    nobody can read off.
  const hypPath = path.join(repo, 'docs', 'hypotheses.json');
  if (fs.existsSync(hypPath)) {
    const hyp = JSON.parse(fs.readFileSync(hypPath, 'utf8'));
    const TIER = ['subject', 'role', 'both'];
    for (const kind of ['functions', 'fields', 'globals'])
      for (const e of hyp[kind] || []) {
        const who = e.addr || `${e.record}+${e.off}` || e.name || '?';
        if (!TIER.includes(e.confidence))
          fail(`hypotheses ${kind} ${who} (${e.name}): confidence must be one of `
             + `${TIER.join(' | ')} — see docs/method.md 7.6`);
        if (!e.basis) fail(`hypotheses ${kind} ${who} (${e.name}) is missing "basis"`);
        if (e.addr && seenAddr.has(e.addr))
          fail(`${e.addr} is verified in symbols.json as ${seenAddr.get(e.addr)} and `
             + `also a hypothesis (${e.name}). Promotion is a move, not a copy.`);
      }
  }
}

// ---- rendering ------------------------------------------------------------
const paramNames = sig => {
  if (!sig) return null;
  const open = sig.indexOf('('), close = sig.lastIndexOf(')');
  if (open < 0 || close < open) return null;
  const inner = sig.slice(open + 1, close).trim();
  if (inner === '' || inner === 'void') return '';
  return inner.split(',').map(p => (p.trim().match(/[A-Za-z_][A-Za-z0-9_]*$/) || [''])[0]).join(', ');
};

const cell = s => String(s).replace(/\|/g, '\\|');

function table(entries, kind) {
  if (!entries.length) return '';
  const head = kind === 'functions'
    ? '| Address | Name | Confidence | What it does |\n|---|---|---|---|'
    : '| Address | Name | Confidence | Meaning |\n|---|---|---|---|';
  const rows = entries.map(e => {
    const p = kind === 'functions' ? paramNames(e.signature) : null;
    const name = '`' + e.name + (p === null ? '' : '(' + p + ')') + '`';
    return `| \`${e.addr}\` | ${name} | ${e.confidence} | ${cell(e.comment || '')} |`;
  });
  return [head, ...rows].join('\n');
}

function block(sectionId) {
  const fns = (data.functions || []).filter(e => e.section === sectionId);
  const vars = (data.globals || []).filter(e => e.section === sectionId);
  const parts = [];
  if (fns.length) parts.push(table(fns, 'functions'));
  if (vars.length) parts.push((fns.length ? '**Globals**\n\n' : '') + table(vars, 'globals'));
  return parts.join('\n\n');
}

// ---- splice into the markdown --------------------------------------------
let md = fs.readFileSync(mdPath, 'utf8');
const before = md;

for (const s of data.sections) {
  const begin = `<!-- BEGIN symbols.json: ${s.id} -->`;
  const end = `<!-- END symbols.json: ${s.id} -->`;
  const i = md.indexOf(begin), j = md.indexOf(end);
  if (i < 0 || j < 0) fail(`docs/symbols.md has no generated region for section "${s.id}"`);
  if (j < i) fail(`section "${s.id}": END marker before BEGIN marker`);
  md = md.slice(0, i + begin.length) + '\n\n' + block(s.id) + '\n\n' + md.slice(j);
}

for (const m of before.matchAll(/<!-- BEGIN symbols\.json: ([^ ]+) -->/g))
  if (!data.sections.some(s => s.id === m[1]))
    fail(`docs/symbols.md has a region for unknown section "${m[1]}"`);

const nf = (data.functions || []).length, ng = (data.globals || []).length;
if (check) {
  if (md !== before) fail('docs/symbols.md is out of date - run: node tools/symbols/symbols_md.js');
  console.log(`symbols.md is up to date (${nf} functions, ${ng} globals)`);
} else if (md === before) {
  console.log(`symbols.md already current (${nf} functions, ${ng} globals)`);
} else {
  fs.writeFileSync(mdPath, md);
  console.log(`symbols.md rewritten (${nf} functions, ${ng} globals)`);
}
