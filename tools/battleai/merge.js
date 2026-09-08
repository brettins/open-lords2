#!/usr/bin/env node
// Merge tools/battleai/newsymbols.json into docs/symbols.json.
//
//   node tools/battleai/merge.js            merge (idempotent - updates entries in place)
//   node tools/battleai/merge.js --dry-run  report what would change
//
// Existing entries with the same address are replaced; the section is inserted after
// the one named in "after". Nothing else in symbols.json is touched.
'use strict';
const fs = require('fs');
const path = require('path');

const repo = path.resolve(__dirname, '..', '..');
const target = path.join(repo, 'docs', 'symbols.json');
const src = JSON.parse(fs.readFileSync(path.join(__dirname, 'newsymbols.json'), 'utf8'));
const data = JSON.parse(fs.readFileSync(target, 'utf8'));
const dry = process.argv.includes('--dry-run');

let added = 0, replaced = 0;

if (!data.sections.some(s => s.id === src.section.id)) {
  const at = data.sections.findIndex(s => s.id === src.section.after);
  data.sections.splice(at < 0 ? data.sections.length : at + 1, 0,
    { id: src.section.id, title: src.section.title });
  console.log(`+ section ${src.section.id}`);
}

for (const kind of ['functions', 'globals']) {
  for (const e of src[kind]) {
    const row = Object.assign({ section: src.section.id }, e);
    // keep the field order symbols.json uses: addr, name, section, confidence, ...
    const ordered = { addr: row.addr, name: row.name, section: row.section, confidence: row.confidence };
    if (row.signature) ordered.signature = row.signature;
    ordered.comment = row.comment;
    const i = data[kind].findIndex(x => x.addr === row.addr);
    if (i < 0) { data[kind].push(ordered); added++; }
    else { data[kind][i] = ordered; replaced++; }
  }
}

console.log(`${added} added, ${replaced} replaced; ` +
  `${data.functions.length} functions, ${data.globals.length} globals`);
if (!dry) fs.writeFileSync(target, JSON.stringify(data, null, 2) + '\n');
