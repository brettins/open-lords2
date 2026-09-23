'use strict';
// Constant propagation over one decompiled body, for draw-coords.json's blind
// sites: a local assigned one literal once and never written again, and an
// argument that is literal arithmetic once those are substituted
// (`iVar3 = 0x20; ... Ui_DrawText(s, iVar3 + 0x10, 0x60, ...)`).
// Nothing else: a parameter, a global or a loop counter stays computed.
const LIT = /^-?(?:0x[0-9a-fA-F]+|\d+)$/;

function strip(body) {
  let inBlock = false;
  return body.map(l => {
    let t = l;
    if (inBlock) { const e = t.indexOf('*/'); if (e < 0) return ''; t = t.slice(e + 2); inBlock = false; }
    t = t.replace(/\/\*[\s\S]*?\*\//g, ' ');
    const o = t.indexOf('/*');
    if (o >= 0) { inBlock = true; t = t.slice(0, o); }
    const c = t.indexOf('//');
    return c >= 0 ? t.slice(0, c) : t;
  }).join('\n');
}

const memo = new Map();
/** name -> literal, for the single-assignment locals of `fn` ({name, body}). */
function locals(fn) {
  if (memo.has(fn.name)) return memo.get(fn.name);
  const src = strip(fn.body);
  const env = new Map();
  const ASSIGN = /^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(-?(?:0x[0-9a-fA-F]+|\d+))\s*;\s*$/gm;
  let m;
  const seen = new Map();
  while ((m = ASSIGN.exec(src))) seen.set(m[1], (seen.get(m[1]) || []).concat(m[2]));
  for (const [v, lits] of seen) {
    if (lits.length !== 1 || !/^[a-z]Var\d+$|^local_[0-9a-f]+$/.test(v)) continue;
    // Every write of v: an assignment, a compound one, ++/--, or its address taken.
    const writes = src.match(new RegExp(`(?:\\b${v}\\s*(?:[-+*/|&^]|<<|>>)?=(?!=)|\\b${v}\\s*(?:\\+\\+|--)|(?:\\+\\+|--)\\s*${v}\\b|&\\s*${v}\\b)`, 'g')) || [];
    if (writes.length === 1) env.set(v, parseInt(lits[0], lits[0].replace('-', '').startsWith('0x') ? 16 : 10));
  }
  memo.set(fn.name, env);
  return env;
}

/** The value of `expr` in `env`, or null. Literal + - * only, and parentheses. */
function evaluate(expr, env) {
  let t = String(expr).trim().replace(/\((?:u?int|short|byte|char|undefined4?)\)\s*/g, '');
  let bad = false;
  t = t.replace(/\b[A-Za-z_][A-Za-z0-9_]*\b/g, id => {
    if (/^0x[0-9a-fA-F]+$/.test(id)) return id;
    if (env.has(id)) return String(env.get(id));
    bad = true;
    return id;
  });
  if (bad || !/^[-+*()\s0-9a-fA-Fx]+$/.test(t) || !/\d/.test(t)) return null;
  if (LIT.test(t)) return parseInt(t, /0x/.test(t) ? 16 : 10);
  try {
    const v = Function(`"use strict"; return (${t});`)();
    return Number.isInteger(v) ? v : null;
  } catch { return null; }
}

module.exports = { locals, evaluate };
