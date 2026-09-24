// Reshapes a raw event stream from trace-run.ps1 (<name>.raw.jsonl) into the shared record
// format of tools/oracle/scenarios/FORMAT.md (<name>.orig.jsonl). Usage: node trace-shape.js raw out
'use strict';
const fs = require('fs');
const [raw, out] = process.argv.slice(2);
const hex = (v) => '0x' + (v >>> 0).toString(16).toUpperCase().padStart(8, '0');
const rec = (frame, kind, addr, name, caller, args) =>
  JSON.stringify({ frame, kind, addr: addr === null ? null : hex(addr), name, caller: caller || null, args });
const path = require('path');
const hooks = JSON.parse(fs.readFileSync(path.join(__dirname, '../../native/l2-trace/hooktable.json'), 'utf8'));
const tileAddr = Object.fromEntries(hooks.map((h) => [h.name, parseInt(h.addr, 16)]));
// A draw leaf called from inside another hooked draw primitive (hooktable.json size) is that primitive's.
const inner = hooks.filter((h) => h.cat === 'draw' && h.size).map((h) => [parseInt(h.addr, 16), parseInt(h.addr, 16) + h.size]);
const nested = (from) => from !== null && inner.some(([a, b]) => from >= a && from < b);
// Parameter names from docs/symbols.json signatures.
const params = Object.fromEntries(JSON.parse(fs.readFileSync(path.join(__dirname, '../../docs/symbols.json'), 'utf8')).functions
  .map((f) => [f.name, ((/\(([^)]*)\)/.exec(f.signature || '') || [])[1] || '').split(',').map((x) => x.trim().split(/[\s*]+/).pop() || '')]));
const BLIT = /^(Blit_(Unclipped|ClippedLeft|ClippedRight)|FUN_004b4537|FUN_004b4814|FUN_004b48b8|FUN_004b49f7|FUN_004b4cb8|FUN_004b5333)$/;
const TEXT = /^(Ui_Draw(Text|Centred|TextCentredIn|Number|NumberRight|Count|Delta|HappinessDelta|UnitNoun|Year)|FUN_0040328E|FUN_0040352f)$/;
const drawCat = new Set(hooks.filter((h) => h.cat === 'draw').map((h) => h.name));
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
  // [realm stepped, g_realms[realm].aiStep before, after] (detour.cpp); 999 is done.
  else if (e.ev === 'ret' && e.fn === 'AI_RunTurnStep') lines.push(rec(e.f, 'ai', 0x0049a581, e.fn, from, { realm: a[0], step: a[1], done: a[2] >= 999 ? 1 : 0 }));
  // The map's terrain leaves (Map_DrawTile 0x004063C1): (bank, ...) + frame DAT_005C9288, g_drawX/Y.
  // Army_*, MoveOrder_* and the battle's Order_* (hooktable.json cats army, order): what a scenario did.
  else if (e.ev === 'call' && (e.cat === 'army' || e.cat === 'order' || e.cat === 'panel')) lines.push(rec(e.f, 'call', tileAddr[e.fn], e.fn, from, { args: a }));
  else if (e.ev === 'call' && /^TileBlit_/.test(e.fn)) lines.push(rec(e.f, 'draw', tileAddr[e.fn], e.fn, from, { sheet: e.text, frame: a[4], x: a[5], y: a[6] }));
  else if (e.ev === 'call') {
    switch (e.fn) {
      case 'Sound_PlayFile': lines.push(rec(e.f, 'sound', 0x00427990, e.fn, from, { file: e.text, speech: a[1] ? 1 : 0 })); break;
      case 'Music_Play': lines.push(rec(e.f, 'sound', 0x004263ad, e.fn, from, { file: e.text, looped: a[2] ? 1 : 0 })); break;
      // Slot n names the last Sound_LoadBank 0x00425E4B table's entry n - 1 (detour.cpp).
      case 'Sound_RestartSlot': lines.push(rec(e.f, 'sound', 0x00426216, e.fn, from, { file: e.text || null, slot: a[0] })); break;
      case 'Sound_PlaySlot': lines.push(rec(e.f, 'sound', 0x00426120, e.fn, from, { file: e.text || null, slot: a[0] })); break;
      case 'Msg_PlayVoice': lines.push(rec(e.f, 'sound', 0x004b35c1, e.fn, from, { group: a[0], variant: a[1] })); break;
      // Msg_Enqueue(from, to, group, variant, category, ...) per docs/symbols.json; county as arg 6 is [I].
      case 'Msg_Enqueue': lines.push(rec(e.f, 'message', 0x00472bc5, e.fn, from, { group: a[2], category: a[4], to: a[1], from: a[0], county: a[5] })); break;
      case 'Pl8_DrawFrame': lines.push(rec(e.f, 'draw', 0x0040a21a, e.fn, from, { sheet: e.text, frame: a[1], x: a[2], y: a[3] })); break;
      case 'Eng_DrawString': lines.push(rec(e.f, 'text', 0x00402d37, e.fn, from, { group: a[0], index: a[1], x: a[2], y: a[3] })); break;
      case 'Smk_Play': lines.push(rec(e.f, 'film', 0x0042d91b, e.fn, from, { file: e.text })); break;
      case 'Pl8_DrawFrameHere': case 'Pl8_DrawFrameClipped':
        if (!nested(from)) lines.push(rec(e.f, 'draw', tileAddr[e.fn], e.fn, from, { sheet: e.text, frame: a[1], x: a[2], y: a[3] })); break;
      // Blit_Raster 0x0040A01D (src, x, y, w, h): the minimap picture, Minimap_Draw 0x00410AA9.
      case 'Blit_Raster': if (!nested(from)) lines.push(rec(e.f, 'draw', tileAddr[e.fn], e.fn, from, { sheet: e.text, x: a[1], y: a[2], w: a[3], h: a[4] })); break;
      default:
        if (!drawCat.has(e.fn) || nested(from)) break;
        // Map_DrawArmies 0x00408438 and the battle painters: sheet arg, frame DAT_005C9288, g_drawX/Y.
        if (BLIT.test(e.fn)) { lines.push(rec(e.f, 'draw', tileAddr[e.fn], e.fn, from, { sheet: e.text, frame: a[1], x: a[2], y: a[3] })); break; }
        {
          const names = params[e.fn] || [], o = {};
          a.forEach((v, i) => { const n = names[i] || 'a' + i; if (!/font/i.test(n)) o[n] = v; });
          if (e.text) o.str = e.text;
          lines.push(rec(e.f, TEXT.test(e.fn) ? 'text' : 'draw', tileAddr[e.fn], e.fn, from, o));
        }
    }
  }
}
fs.writeFileSync(out, lines.join('\n') + '\n');
