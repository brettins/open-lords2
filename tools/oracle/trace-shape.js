// Reshapes a raw event stream from trace-run.ps1 (<name>.raw.jsonl) into the shared record
// format of tools/oracle/scenarios/FORMAT.md (<name>.orig.jsonl). Usage: node trace-shape.js raw out
'use strict';
const fs = require('fs');
const [raw, out] = process.argv.slice(2);
const hex = (v) => '0x' + (v >>> 0).toString(16).toUpperCase().padStart(8, '0');
const rec = (frame, kind, addr, name, caller, args) =>
  JSON.stringify({ frame, kind, addr: addr === null ? null : hex(addr), name, caller: caller || null, args });
const lines = [];
let screen = null, phase = null;
for (const l of fs.readFileSync(raw, 'utf8').split('\n')) {
  if (!l) continue;
  const e = JSON.parse(l), a = e.args || [], from = e.from || null;
  if (e.ev === 'frame') {
    if (e.screen !== screen) lines.push(rec(e.f, 'screen', 0x004eac50, 'g_screenId', null, { id: e.screen }));
    if (e.phase !== phase) lines.push(rec(e.f, 'phase', 0x00569584, 'g_turnPhase', null, { phase: e.phase }));
    screen = e.screen; phase = e.phase;
    // Rand_Advance 0x00404A46 and Rng_LfsrStep 0x00404B2C calls in the frame.
    if (e.rand + e.lfsr) lines.push(rec(e.f, 'rand', null, 'Pcg32', null, { count: e.rand + e.lfsr }));
  } else if (e.ev === 'anchor') lines.push(rec(e.f, 'anchor', null, null, null, { label: e.label }));
  else if (e.ev === 'snap') lines.push(rec(e.f, 'snap', null, null, null, { hash: e.hash }));
  else if (e.ev === 'failed') lines.push(rec(e.f, 'failed', null, null, null, { step: e.step }));
  else if (e.ev === 'call') {
    switch (e.fn) {
      case 'Sound_PlayFile': lines.push(rec(e.f, 'sound', 0x00427990, e.fn, from, { file: e.text, speech: a[1] ? 1 : 0 })); break;
      case 'Music_Play': lines.push(rec(e.f, 'sound', 0x004263ad, e.fn, from, { file: e.text, looped: a[2] ? 1 : 0 })); break;
      // The bank slot, not its file: Sound_LoadBank 0x00425E4B maps slots to names.
      case 'Sound_RestartSlot': lines.push(rec(e.f, 'sound', 0x00426216, e.fn, from, { file: null, slot: a[0] })); break;
      case 'Sound_PlaySlot': lines.push(rec(e.f, 'sound', 0x00426120, e.fn, from, { file: null, slot: a[0] })); break;
      case 'Msg_PlayVoice': lines.push(rec(e.f, 'sound', 0x004b35c1, e.fn, from, { group: a[0], variant: a[1] })); break;
      // Msg_Enqueue(from, to, group, variant, category, ...) per docs/symbols.json; county as arg 6 is [I].
      case 'Msg_Enqueue': lines.push(rec(e.f, 'message', 0x00472bc5, e.fn, from, { group: a[2], category: a[4], to: a[1], from: a[0], county: a[5] })); break;
      // DAT_0057C944, the realm cursor, read at entry; the step byte is not captured (null).
      case 'AI_RunTurnStep': lines.push(rec(e.f, 'ai', 0x0049a581, e.fn, from, { realm: a[0], step: null, done: null })); break;
      case 'Pl8_DrawFrame': lines.push(rec(e.f, 'draw', 0x0040a21a, e.fn, from, { sheet: e.text, frame: a[1], x: a[2], y: a[3] })); break;
      case 'Eng_DrawString': lines.push(rec(e.f, 'text', 0x00402d37, e.fn, from, { group: a[0], index: a[1], x: a[2], y: a[3] })); break;
      case 'Smk_Play': lines.push(rec(e.f, 'film', 0x0042d91b, e.fn, from, { file: e.text })); break;
    }
  }
}
fs.writeFileSync(out, lines.join('\n') + '\n');
