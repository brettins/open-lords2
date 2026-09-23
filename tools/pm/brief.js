// Row brief-from-ledger-rows: an agent brief is the standard preamble plus the
// rows, taken from the ledger so a stale `next` is caught when the brief is read.
//
//   node tools/pm/work.js --brief id,id [--branch agent/x] [--budget 80] [--file <ledger>]

const fs = require('fs');
const path = require('path');

const FIELDS = ['source', 'next', 'note'];

function brief(rows, ids, { branch, budget } = {}) {
  const byId = new Map(rows.map((r) => [r.id, r]));
  const missing = ids.filter((id) => !byId.has(id));
  if (missing.length) throw new Error(`not in the ledger: ${missing.join(', ')}`);
  const br = branch || `agent/${ids[0]}`;
  const n = Number(budget) || 80;
  const preamble = fs
    .readFileSync(path.join(__dirname, 'preamble.md'), 'utf8')
    .replace(/\{branch\}/g, br)
    .replace(/\{slug\}/g, br.replace(/^agent\//, '').replace(/[^\w-]/g, '-'))
    .replace(/\{budget\}/g, String(n));
  const out = [preamble.trimEnd(), '', `Branch: ${br}. Budget ${n} tool calls, hard stop.`, '', 'Rows, in priority order:'];
  ids.forEach((id, i) => {
    const r = byId.get(id);
    out.push(`${i + 1}. ${r.id} (${r.state}): ${r.title}`);
    for (const f of FIELDS) if (r[f]) out.push(`   ${f}: ${r[f]}`);
    if ((r.depends_on || []).length) out.push(`   depends on: ${r.depends_on.join(', ')}`);
  });
  out.push('', 'A row\'s `next` is the plan. If it is stale or wrong, say so in the report rather than follow it.');
  return out.join('\n') + '\n';
}

module.exports = { brief };
