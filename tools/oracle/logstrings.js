// What the engine says about itself.
//
//   node tools/oracle/logstrings.js                  every Log_Write message, by caller
//   node tools/oracle/logstrings.js --unnamed        only functions we have not named
//   node tools/oracle/logstrings.js --grep DirectDraw
//   node tools/oracle/logstrings.js --fn Log_Write --arg 0
//
// The exe is found via $LORDS2_DIR, or --exe <path>. Needs the decompiled
// corpus in tools/oracle/decomp (see decompile-all.ps1); never commit that.
//
// # Why this exists
//
// `L2.eng` is the strongest naming lever on this project and it has a hard
// limit: it only reaches code that DRAWS TEXT. The engine layer - DirectDraw,
// the window, the network transport, the video player - draws no strings, and
// was the darkest part of the binary for exactly that reason.
//
// `Lords2.exe` writes `status.txt` beside itself. The writer is
// `Log_Write` (0x004AFAB9); 85 functions call it with a LITERAL message
// address, and the message is the game describing what that function is doing.
// This script joins those literals to the strings in `.data` and prints them
// against the caller, which is the same move `anchor.js strings` makes with
// L2.eng groups.
//
// # What it claims, and what it does not
//
// It claims the string. That is mechanical and cannot be wrong except by
// parsing badly: an immediate in a call, resolved through the PE section table.
//
// It does NOT claim a name. Two cautions, both paid for:
//
//  * **The number argument is passed PLUS ONE.** Log_Write(msg, extra, n)
//    prints n - 1, so every call site writes `value + 1` and 0 means "no
//    number". Reading the immediate at face value is off by one at 218 sites.
//
//  * **A message says what the CALLER was doing, not what the function IS.**
//    Seven different art loaders share "ERR:Data load, couldn't find  "
//    verbatim. The lever gives a subject; the role still needs its own check
//    that could have failed. docs/decisions.md CNEW-selfname.
//
// The eight messages that DO name their own routine - "ERR:top_it bad data ",
// "ERR:gen_frame bad data " and so on, the C convention of putting the function
// name in its own error - are the exception and are written up in that
// correction, along with the five existing names they graded.

const fs = require('fs');
const path = require('path');

const argOf = k => { const i = process.argv.indexOf(k); return i > 0 ? process.argv[i + 1] : null; };

// The corpus is gitignored, so a git WORKTREE does not have one - and every
// agent now works in a worktree. `--decomp` / $LORDS2_DECOMP points at the
// main checkout's copy. anchor.js and xref.js assume `__dirname/decomp` and
// are unusable from a worktree for this reason; they should gain the same.
const DECOMP = argOf('--decomp') || process.env.LORDS2_DECOMP || path.join(__dirname, 'decomp');

const exePath = argOf('--exe')
  || path.join(process.env.LORDS2_DIR || 'F:/games/Lords of the Realm II', 'Lords2.exe');
if (!fs.existsSync(exePath)) {
  console.error(`logstrings: no Lords2.exe at ${exePath}\n  set LORDS2_DIR or pass --exe <path>`);
  process.exit(1);
}
if (!fs.existsSync(DECOMP)) {
  console.error(`logstrings: no corpus at ${DECOMP}\n  run tools/oracle/decompile-all.ps1 first`);
  process.exit(1);
}

// ---- PE section table, read rather than hardcoded -------------------------
const exe = fs.readFileSync(exePath);
const peOff = exe.readUInt32LE(0x3c);
const nSecs = exe.readUInt16LE(peOff + 6);
const optSize = exe.readUInt16LE(peOff + 20);
const imageBase = exe.readUInt32LE(peOff + 24 + 28);
const secs = [];
for (let i = 0; i < nSecs; i++) {
  const o = peOff + 24 + optSize + i * 40;
  secs.push({
    va: imageBase + exe.readUInt32LE(o + 12),
    vsize: exe.readUInt32LE(o + 8),
    raw: exe.readUInt32LE(o + 20),
    rsize: exe.readUInt32LE(o + 16),
  });
}
function fileOff(va) {
  for (const s of secs)
    if (va >= s.va && va < s.va + s.vsize) {
      const o = s.raw + (va - s.va);
      return o < s.raw + s.rsize ? o : -1;      // in the virtual tail, not on disk
    }
  return -1;
}
function cstr(va) {
  const o = fileOff(va);
  if (o < 0) return null;
  let e = o;
  while (e < exe.length && exe[e] !== 0 && e - o < 300) e++;
  const s = exe.toString('latin1', o, e);
  return /^[\x20-\x7e]*$/.test(s) && s.length ? s : null;
}

// ---- the corpus ------------------------------------------------------------
const fnName = argOf('--fn') || 'Log_Write';
const argIdx = Number(argOf('--arg') || 0);
const onlyUnnamed = process.argv.includes('--unnamed');
const grep = argOf('--grep');

// Log_Write is the name in docs/symbols.json; a corpus rebuilt before that
// still spells it FUN_004afab9. Accept both, because a key that matches
// nothing looks exactly like a primitive nobody calls - which is precisely how
// anchor.js silently lost two L2.eng groups (docs/formats/eng.md 5.1).
const CALLEE = new RegExp(
  `\\b(?:${fnName}|FUN_004afab9|FUN_004AFAB9)\\s*\\(([^;()]*(?:\\([^()]*\\)[^;()]*)*)\\)\\s*;`, 'g');

let functions = 0, sites = 0, resolved = 0;
for (const file of fs.readdirSync(DECOMP).filter(f => f.endsWith('.c')).sort()) {
  const text = fs.readFileSync(path.join(DECOMP, file), 'utf8');
  const marks = [];
  const head = /^\/\/ ==== ([0-9a-f]{8})\s+(\S+)\s+params=(-?\d+)\s+bytes=(\d+)\s*$/gm;
  let m;
  while ((m = head.exec(text)) !== null)
    marks.push({ addr: m[1], name: m[2], bytes: +m[4], start: m.index, end: head.lastIndex });

  for (let i = 0; i < marks.length; i++) {
    const body = text.slice(marks[i].end, i + 1 < marks.length ? marks[i + 1].start : text.length);
    const out = [];
    CALLEE.lastIndex = 0;
    while ((m = CALLEE.exec(body)) !== null) {
      sites++;
      const a = (m[1].split(',')[argIdx] || '').trim();
      if (!/^(0x[0-9a-fA-F]+|\d+)$/.test(a)) { out.push(`(computed: ${a})`); continue; }
      const s = cstr(parseInt(a));
      if (s === null) { out.push(`(unresolved: ${a})`); continue; }
      resolved++;
      out.push(s);
    }
    if (!out.length) continue;
    const unnamed = /^FUN_/.test(marks[i].name);
    if (onlyUnnamed && !unnamed) continue;
    const keep = grep ? out.filter(s => s.toLowerCase().includes(grep.toLowerCase())) : out;
    if (!keep.length) continue;
    functions++;
    console.log(`${unnamed ? '* ' : '  '}0x${marks[i].addr.toUpperCase()}  ${marks[i].name}  ${marks[i].bytes}b`);
    for (const s of keep) console.log(`      ${JSON.stringify(s)}`);
  }
}
console.log(`\n${functions} functions printed; ${resolved} of ${sites} call sites passed a literal.`);
if (onlyUnnamed) console.log('(* marks a function still unnamed IN THE CORPUS - rebuild it after adding names)');
