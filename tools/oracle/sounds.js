// **Every place `Lords2.exe` asks for a sound.**
//
//   node tools/oracle/sounds.js            # the table, by primitive
//   node tools/oracle/sounds.js --count    # just the totals
//
// The audio equivalent of the input-arm audit, and it is far cheaper than that
// one was, for a reason worth stating: **the original funnels every sound
// through eight leaf functions**, so the denominator is a grep rather than a
// reading of every screen. `docs/audio-triggers.md` is what this produces and
// why it matters.
//
// Reads `tools/oracle/decomp/*.c`, which is gitignored — regenerate it with
// `tools/oracle/decompile-all.ps1` if it is missing. A site is attributed to
// the function whose `// ==== <addr> <name>` header it falls under, which is
// what `DecompileFunc.java` emits.

const fs = require('fs');
const path = require('path');

const DECOMP = path.join(__dirname, 'decomp');

/// The eight leaf functions every sound in the game goes through, and what
/// each one *is*. `docs/symbols.md` names them all.
const PRIMITIVES = [
  ['Sound_PlaySlot', '0x00426120', 'bank', 'drops the request if that buffer is still playing'],
  ['FUN_004262cf', '0x004262CF', 'bank', 'a 28-byte thunk that forwards to Sound_PlaySlot'],
  ['Sound_RestartSlot', '0x00426216', 'bank', 'rewinds and plays regardless'],
  ['Sound_PlayFile', '0x00427990', 'file', 'one-shot by name; arg 2 picks the speech or effects flag'],
  ['Msg_PlayVoice', '0x004B35C1', 'voice', 'an L2.eng group to a filename, then Sound_PlayFile'],
  ['Sound_PlayTroopCry', '0x00499CB1', 'cry', '11 x 4 x 4, then Sound_PlayFile'],
  ['Music_StartCampaign', '0x00499ACA', 'music', 'the progress-bar ladder'],
  ['Music_StartBattle', '0x00477B2F', 'music', 'the alternating pair'],
];

/// Sites that live *inside* one of the primitives are that primitive's
/// implementation, not a trigger. Counting them would double-count every voice
/// line and every troop cry.
const DISPATCHERS = new Set([
  'Msg_PlayVoice', 'Sound_PlayTroopCry', 'Sound_PlayFile',
  'Music_StartBattle', 'Music_StartCampaign', 'FUN_004262cf',
  'Sound_LoadKingdomBank', 'Sound_LoadBattleBank',
]);

function scan() {
  const rows = [];
  if (!fs.existsSync(DECOMP)) {
    console.error(`no corpus at ${DECOMP} - run tools/oracle/decompile-all.ps1`);
    process.exit(2);
  }
  for (const f of fs.readdirSync(DECOMP).filter((n) => n.endsWith('.c'))) {
    const lines = fs.readFileSync(path.join(DECOMP, f), 'utf8').split(/\r?\n/);
    let fn = null;
    let addr = null;
    lines.forEach((line, i) => {
      const h = line.match(/^\/\/ ==== ([0-9a-f]{8})\s+(\S+)/);
      if (h) {
        addr = '0x' + h[1].toUpperCase();
        fn = h[2];
        return;
      }
      if (/^\s*\/\//.test(line) || /^\s*\*/.test(line) || /^\s*\/\*/.test(line)) return;
      for (const [prim, , cls] of PRIMITIVES) {
        const re = new RegExp('(^|[^A-Za-z0-9_])' + prim + '\\s*\\(([^)]*)\\)');
        const m = line.match(re);
        if (!m) continue;
        // The definition line, not a call.
        if (/^\s*(void|int|uint|char|undefined\d?|BOOL)\b[^;]*\)\s*$/.test(line)) continue;
        if (DISPATCHERS.has(fn)) continue;
        rows.push({ prim, cls, fn, addr, arg: m[2].trim(), file: f, line: i + 1 });
      }
    });
  }
  return rows;
}

const rows = scan();
const byPrim = new Map();
for (const r of rows) {
  if (!byPrim.has(r.prim)) byPrim.set(r.prim, []);
  byPrim.get(r.prim).push(r);
}

if (process.argv.includes('--count')) {
  for (const [prim, , cls] of PRIMITIVES) {
    const n = (byPrim.get(prim) || []).length;
    console.log(`${prim.padEnd(22)} ${cls.padEnd(6)} ${String(n).padStart(3)}`);
  }
  console.log(`\n${rows.length} trigger sites across ` +
    `${new Set(rows.map((r) => r.fn)).size} functions`);
  process.exit(0);
}

for (const [prim, paddr, cls, what] of PRIMITIVES) {
  const list = (byPrim.get(prim) || []).sort((a, b) =>
    a.fn.localeCompare(b.fn) || a.line - b.line);
  console.log(`\n## ${prim} (${paddr}) - ${cls}`);
  console.log(`${what}. ${list.length} trigger site(s).\n`);
  console.log('| caller | addr | argument |');
  console.log('|---|---|---|');
  for (const r of list) {
    console.log(`| \`${r.fn}\` | \`${r.addr}\` | \`${r.arg}\` |`);
  }
}
console.log(`\n**${rows.length} trigger sites across ` +
  `${new Set(rows.map((r) => r.fn)).size} functions.**`);
