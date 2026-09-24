// Generates hooktable.h: every detour target, its expected prologue and how many bytes the
// detour steals. Addresses and signatures come from docs/symbols.json; the prologue from
// Lords2.exe. A prologue this length decoder cannot read, or that holds a relative branch,
// is left out and listed. Usage: node native/l2-trace/gen-hooks.js [Lords2.exe]
'use strict';
const fs = require('fs');
const path = require('path');
const root = path.join(__dirname, '..', '..');
const exe = fs.readFileSync(process.argv[2] || 'E:/games/l2-trace/Lords2.exe');
const syms = JSON.parse(fs.readFileSync(path.join(root, 'docs', 'symbols.json'), 'utf8')).functions;
const byName = new Map(syms.map((f) => [f.name.toLowerCase(), f]));

// [category, capture return, names]; tools/oracle/sounds.js PRIMITIVES, tools/draws/screendraws.js LEAF.
// Map_DrawTile 0x004063C1 picks the bank (arg 3) and frame DAT_005C9288, blits at g_drawX/Y.
const TILE = ['Plain', 'M3', 'M4'].flatMap((m) => [0, 1, 2].map((z) => `TileBlit_${m}_Z${z}`));
const GROUPS = [
  ['sound', 1, ['Sound_PlaySlot', 'Sound_PlaySlotWrap', 'Sound_RestartSlot', 'Sound_PlayFile', 'Sound_PlayTroopCry',
    'Sound_PlayS010', 'Sound_PlayS016', 'Sound_PlayS201Seq', 'Sound_PlayS246', 'Sound_LoadBank']],
  ['music', 0, ['Music_Play', 'Music_StartCampaign', 'Music_StartBattle', 'Music_Stop']],
  ['voice', 0, ['Msg_PlayVoice']],
  ['msg', 0, ['Msg_Enqueue', 'Msg_EnqueueCurrent']],
  ['turn', 1, ['Turn_Tick']],
  ['ai', 1, ['AI_RunTurnStep', 'Diplo_AnswerInbox', 'AI_Diplomacy', 'AI_SetTaxRates', 'FUN_0049e1bf',
    'Ai_ManageCountyFarms', 'AI_BuildCastles', 'FUN_0049f93d', 'FUN_0049f96c', 'FUN_0049f977', 'FUN_004a0015',
    'Ai_AdvanceArmies', 'FUN_0049e77d', 'AI_Taunt', 'Ai_ManageFarmsAll', 'AI_ManageFields', 'Ai_TradeForCounty',
    'AI_EmergencyWeapons', 'Ai_RunMission', 'Ai_MissionRaid', 'Ai_MissionAssistAlly', 'Ai_MissionSeekEnemy',
    'Ai_MissionHoldHome', 'Ai_MissionGarrisoned', 'Ai_MissionJoinGarrison', 'Ai_AimAtCounty',
    'Ai_FindCastleBuildingTile', 'Ai_FindTownTile', 'Ai_FindStandingCropTile', 'Ai_SetRations', 'Ai_SellGood',
    'Ai_BuyGood', 'Ai_BuyGoodDownTo', 'Ai_FarmStyleNeutralArable', 'Ai_FarmStyleNeutralGrazing',
    'Ai_FarmStyleArable', 'Ai_FarmStyleGrazing', 'Ai_FarmStyleMixed', 'Ai_FitCattleFields']],
  ['army', 1, ['Army_RaiseConfirm', 'Army_DisbandConfirm', 'Army_LeaveCastle', 'Army_SplitConfirm', 'Army_Split',
    'Army_Disband', 'Army_ContestCounty', 'Army_Garrison', 'Army_GarrisonConfirm', 'Army_GarrisonApply',
    'Army_BeginSiege', 'Army_BeginSiegeConfirm', 'Army_Create', 'Army_Destroy', 'Army_Combine',
    'Army_PrepareForBattle', 'Army_Desert', 'Army_WithdrawCasualties', 'MoveOrder_StripClicked',
    'MoveOrder_ConfirmGarrison', 'MoveOrder_ConfirmSiege', 'MoveOrder_ConfirmCombine', 'MoveOrder_Confirm']],
  ['order', 1, syms.filter((f) => /^Order_/.test(f.name)).map((f) => f.name)],
  ['rng', 1, ['Rand_Advance', 'Rng_LfsrStep']],
  ['draw', 0, ['Pl8_DrawFrame', 'Pl8_DrawFrameHere', 'Pl8_DrawFrameClipped', 'Eng_DrawString', 'Ui_DrawText',
    'Ui_DrawCentred', 'Ui_DrawTextCentredIn', 'Ui_DrawNumber', 'Ui_DrawNumberRight', 'Ui_DrawCount', 'Ui_DrawDelta',
    'Ui_DrawHappinessDelta', 'Ui_DrawUnitNoun', 'Ui_DrawYear', 'Ui_DrawBox', 'Ui_DrawBoxBorder', 'Ui_DrawBoxInterior',
    'Ui_DrawInsetRect', 'Ui_DrawTileStrip', 'Ui_DrawBevelRect', 'Ui_DrawRectOutline', 'Ui_DrawMenuTitles',
    'Ui_HistoryGraph', 'Ui_OkButton', 'Ui_DrawBoxStyle1', 'Ui_DrawBoxStyle2', 'Blit_Raster', 'Sprite_WGenSprite',
    'Sprite_WGenHSprite', 'Sprite_WGenFSprite', 'FUN_0040328e', 'FUN_0040352f', 'FUN_00403a8f', 'FUN_00403ee4',
    'FUN_0040437d', 'FUN_00409346', 'FUN_00408fcb', 'FUN_00410c71', ...TILE]],
  ['load', 1, ['File_ReadChunk']],
  ['film', 1, ['Smk_Play']],
  ['save', 1, ['Save_Read']],
  ['newgame', 0, ['Game_NewGame']],
];
// Arguments the signatures leave as int that are pointers: s = string, k = sheet buffer.
const ARGS = {
  Smk_Play: 'siiii', Pl8_DrawFrame: 'kiii', Pl8_DrawFrameHere: 'kiii', Pl8_DrawFrameClipped: 'kiii',
  Blit_Raster: 'kiiii', FUN_0040328e: 'iiii', FUN_0040352f: 'siiii', FUN_00408fcb: 'si', FUN_00403ee4: 'iiii',
  FUN_0040437d: 'iiiii', FUN_00403a8f: 'iiii', FUN_00409346: 'iiii', Ui_DrawBoxStyle1: 'iiii', Ui_DrawBoxStyle2: 'iiii',
  Diplo_AnswerInbox: 'i', FUN_0049e1bf: 'i', FUN_0049f93d: 'i', FUN_0049f977: 'i', FUN_004a0015: 'i', FUN_0049e77d: 'i',
  Sound_PlaySlotWrap: 'i', Ui_DrawTextCentredIn: 'siiipi', Sound_PlayTroopCry: 'i', Msg_EnqueueCurrent: '',
};
// Globals read beside the arguments (and again at return): AI_RunTurnStep's realm cursor
// DAT_0057C944 (0x0049A581 body), Turn_Tick's g_turnPhase 0x00569584.
const GLOBALS = { AI_RunTurnStep: [0x0057c944], Turn_Tick: [0x00569584] };
for (const t of TILE) { ARGS[t] = 'kiii'; GLOBALS[t] = [0x005c9288, 0x00591524, 0x00591528]; }

function argSpec(f) {
  if (ARGS[f.name] !== undefined) return ARGS[f.name];
  const m = /\(([^)]*)\)/.exec(f.signature || '');
  if (!m || m[1].trim() === 'void' || !m[1].trim()) return '';
  return m[1].split(',').map((a) => {
    if (/char\s*\*|byte\s*\*/.test(a) && !/font/.test(a)) return 's';
    if (/\*/.test(a)) return 'p';
    return 'i';
  }).join('').slice(0, 8);
}

const secs = (() => {
  const pe = exe.readUInt32LE(0x3c), n = exe.readUInt16LE(pe + 6), opt = exe.readUInt16LE(pe + 20);
  const out = [];
  for (let i = 0, o = pe + 24 + opt; i < n; i++, o += 40)
    out.push({ va: 0x400000 + exe.readUInt32LE(o + 12), size: exe.readUInt32LE(o + 8), raw: exe.readUInt32LE(o + 20) });
  return out;
})();
const at = (va) => { const s = secs.find((s) => va >= s.va && va < s.va + s.size); return s.raw + va - s.va; };

// Instruction length for the prologue shapes this binary uses; null = unknown or relative.
function insnLen(b, i) {
  let p = i, op = b[p];
  if (op === 0x64 || op === 0x66) op = b[++p];
  const modrm = (q) => {
    const m = b[q], mod = m >> 6, rm = m & 7;
    let n = 1;
    if (mod !== 3 && rm === 4) { n++; if (mod === 0 && (b[q + 1] & 7) === 5) n += 4; }
    if (mod === 0 && rm === 5) n += 4; else if (mod === 1) n += 1; else if (mod === 2) n += 4;
    return n;
  };
  if (op >= 0x50 && op <= 0x5f) return p - i + 1;
  if (op === 0x90) return p - i + 1;
  if (op === 0x6a) return p - i + 2;
  if (op === 0x68 || (op >= 0xb8 && op <= 0xbf) || op === 0xa1 || op === 0xa3) return p - i + 5;
  if ([0x8b, 0x89, 0x8d, 0x33, 0x2b, 0x03, 0x3b, 0x85, 0x31, 0x29, 0x01, 0x39, 0x88, 0x8a, 0x0b, 0x23, 0x09, 0x21].includes(op))
    return p - i + 1 + modrm(p + 1);
  if (op === 0x83 || op === 0x80 || op === 0xc6) return p - i + 1 + modrm(p + 1) + 1;
  if (op === 0x81 || op === 0xc7) return p - i + 1 + modrm(p + 1) + 4;
  return null;
}

const rows = [], left = [];
for (const [cat, ret, names] of GROUPS) {
  for (const name of names) {
    const raw = /^FUN_([0-9a-f]{8})$/i.exec(name);
    const f = byName.get(name.toLowerCase()) || (raw && { addr: '0x' + raw[1], name, signature: '' });
    if (!f) { left.push(`${name}: not in symbols.json`); continue; }
    const va = parseInt(f.addr, 16), off = at(va);
    let n = 0;
    while (n < 5) {
      const l = insnLen(exe, off + n);
      if (l === null) break;
      n += l;
    }
    if (n < 5) { left.push(`${f.name} ${f.addr}: prologue ${exe.slice(off, off + 8).toString('hex')} not decoded`); continue; }
    rows.push({ va, name: f.name, cat, ret, args: argSpec(f), n, bytes: [...exe.slice(off, off + n)], globals: GLOBALS[f.name] || [] });
  }
}
const cats = [...new Set(GROUPS.map((g) => g[0]))];
const hex = (v) => '0x' + v.toString(16).toUpperCase().padStart(8, '0');
let h = '// Generated by gen-hooks.js from docs/symbols.json and Lords2.exe; do not edit.\n#pragma once\n#include <cstdint>\n\n';
h += `enum HookCat : uint8_t { ${cats.map((c, i) => `Cat_${c} = ${i}`).join(', ')}, Cat_count };\n`;
h += `static const char* const kCatNames[] = { ${cats.map((c) => `"${c}"`).join(', ')} };\n\n`;
h += 'struct HookDef { uint32_t addr; const char* name; uint8_t cat, ret, stolen; const char* args; uint8_t expect[16]; uint32_t globals[3]; };\n';
h += 'static const HookDef kHooks[] = {\n';
for (const r of rows) {
  h += `  {${hex(r.va)}, "${r.name}", Cat_${r.cat}, ${r.ret}, ${r.n}, "${r.args}", {${r.bytes.map((b) => '0x' + b.toString(16).padStart(2, '0')).join(',')}}, {${r.globals.map(hex).join(', ') || '0'}}},\n`;
}
h += '};\n';
h += left.map((l) => `// not hooked: ${l}\n`).join('');
fs.writeFileSync(path.join(__dirname, 'hooktable.h'), h);
fs.writeFileSync(path.join(__dirname, 'hooktable.json'), JSON.stringify(rows.map((r) => ({ addr: hex(r.va), name: r.name, cat: r.cat })), null, 0));
console.log(`${rows.length} hooks, ${left.length} left out`);
for (const l of left) console.log('  ' + l);
