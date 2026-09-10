#!/usr/bin/env node
// Count the campaign map's draw calls in the original, mechanically.
//
//   node tools/draws/mapdraws.js            # the table
//   node tools/draws/mapdraws.js --sites    # every call site, one per line
//
// # What a "draw call" is here
//
// docs/draws.md section 3: the unit that pays is the **sheet-and-frame
// reference**, not the painter. So this counts *leaf* draw primitives - the
// calls that actually put a picture, a glyph run or a rectangle on the frame
// buffer - inside the set of functions that paint the campaign map. A call to
// another painter (Map_DrawTile, CountyStrip_Draw) is a recursion, not a draw,
// and is followed rather than counted.
//
// It deliberately does NOT count the five unrolled tile blitters or the three
// clip-state variants of one blit. FUN_00452820 / FUN_0045337B / FUN_004538F6
// are one draw seen through five clip modes, and g_clipState picking between
// FUN_004B4537 / FUN_004B4814 / FUN_004B48B8 is the same blit clipped. Counting
// them would inflate the map's total by a factor nothing else in docs/draws.md
// shares.
'use strict';
const fs = require('fs');
const path = require('path');

// Find the corpus from a worktree the way tools/oracle/xref.js does.
function corpus() {
  let d = __dirname;
  for (let i = 0; i < 8; i++) {
    const c = path.join(d, 'tools', 'oracle', 'decomp');
    if (fs.existsSync(c)) return c;
    d = path.dirname(d);
  }
  // A worktree under .claude/worktrees/<name> - the corpus is gitignored and
  // lives only in the main checkout.
  const main = 'E:/dev/lords2/tools/oracle/decomp';
  if (fs.existsSync(main)) return main;
  throw new Error('no decomp corpus found');
}

const DIR = corpus();
const bodies = new Map(); // name -> {addr, name, text}
for (const f of fs.readdirSync(DIR).filter(f => f.endsWith('.c'))) {
  const text = fs.readFileSync(path.join(DIR, f), 'utf8');
  const re = /^\/\/ ==== ([0-9a-f]{8})  (\S+)  params=\d+  bytes=(\d+)$/gm;
  let m, prev = null;
  while ((m = re.exec(text)) !== null) {
    if (prev) prev.text = text.slice(prev.end, m.index);
    prev = { addr: m[1], name: m[2], bytes: +m[3], end: re.lastIndex };
    bodies.set(m[2], prev);
  }
  if (prev) prev.text = text.slice(prev.end);
}

// Leaf draw primitives: everything that writes pixels and takes its subject as
// an argument. Names from docs/symbols.json; the FUN_ ones are unnamed leaves
// whose bodies were read for this audit.
const LEAF = [
  'Pl8_DrawFrame', 'Pl8_DrawFrameHere', 'Pl8_DrawFrameClipped',
  'Eng_DrawString', 'Ui_DrawText', 'Ui_DrawCentred', 'Ui_DrawNumber',
  'Ui_DrawNumberRight', 'Ui_DrawCount', 'Ui_DrawDelta', 'Ui_DrawUnitNoun',
  'Ui_DrawYear', 'Ui_DrawBox', 'Ui_DrawBoxBorder', 'Ui_DrawBoxInterior',
  'Ui_DrawTileStrip', 'Ui_DrawBevelRect', 'Ui_DrawMenuTitles',
  'Ui_HistoryGraph',
  'FUN_004025d7',   // a string, left-aligned, from a char*
  'FUN_0040328e',   // a wrapped L2.eng paragraph
  'FUN_00403cf4',   // a rectangle outline
  'FUN_0040437d',   // a filled rectangle
  'FUN_00409346',   // a framed box with a picture
  'FUN_00409429',   // a framed box, no interior
  'FUN_004b414a',   // a saved-backdrop restore / shaded plate
  'FUN_004b4056',   // one pixel
  'FUN_004b3f7a',   // one pixel
  'Blit_Raster',    // the minimap's 128x128 raster
];
// Blit tails: one logical blit, three clip states. Counted once per *caller
// block*, not three times - see the header.
const BLIT_TAIL = ['FUN_004b4537', 'FUN_004b4814', 'FUN_004b48b8',
  'Blit_Unclipped', 'Blit_ClippedLeft', 'Blit_ClippedRight'];

// The map's blitters take their subject in two globals rather than in
// arguments, so the *frame selection* is the sheet-and-frame reference and the
// blit is shared. DAT_005C9288 is the frame register every map blitter reads;
// an assignment to it is one picture. `tiles[t].frame = ...` inside Sprite_TopIt
// is the same thing one level down - it repaints the terrain tile itself.
// `DAT_005C9288 = 999` in Map_DrawArmies is a *no-draw* sentinel - the tail
// tests it and skips the blit - so it is excluded rather than counted.
const NOT_A_DRAW = /= 999;/;
const FRAME_SET = [
  /\bDAT_005c9288 = /,
  /\(&g_tiles\[0\]\.frame\)\[g_tileCursor\] = /,
];
// A ladder over five banks, five clip modes and two zooms is one draw: the tile.
const TILE_DRAW = new Map([
  ['Map_DrawTile', 1], ['Map_DrawTileApex', 1], ['Map_DrawSurroundTile', 1],
  ['FUN_0042a8d1', 1], ['FUN_0042a9ab', 1], ['FUN_00406bba', 1],
  // The per-pixel county tint is one draw over the raster.
  ['Minimap_DrawOverlay', 1],
]);

// The campaign map's painting tree, hand-walked from Screen_DrawCampaign and
// from Battle_Frame's per-frame tail. Each entry is [function, what it paints,
// reachable-on-the-campaign-map?].
const TREE = [
  ['Screen_DrawCampaign', 'the full repaint', 'live'],
  ['Map_DrawFrame', 'one map frame', 'live'],
  ['Map_RenderIso', 'terrain: first and last rows', 'live'],
  ['Map_RenderAlignedRow', 'terrain: an aligned row', 'live'],
  ['Map_RenderOffsetRow', 'terrain: a half-offset row', 'live'],
  ['Map_DrawTile', 'the terrain diamond', 'live'],
  ['Map_DrawTileApex', "the diamond's overhang", 'live'],
  ['Map_DrawSurroundTile', 'an off-map surround tile', 'live'],
  ['FUN_0042a8d1', 'surround, left half', 'live'],
  ['FUN_0042a9ab', 'surround, right half', 'live'],
  ['FUN_00405602', 'overlays: first and last rows', 'live'],
  ['FUN_00405eb5', 'overlays: an aligned row', 'live'],
  ['FUN_00405fac', 'overlays: a half-offset row', 'live'],
  ['Sprite_TopIt', 'the six tile overlays', 'live'],
  ['FUN_00407f82', "the besieger's banner", 'live'],
  ['Map_DrawPathMarker', 'a path-preview ball', 'live'],
  ['FUN_00408c50', 'two debug numbers per tile', 'debug'],
  ['FUN_00405487', 'units: first and last rows', 'live'],
  ['FUN_00405862', 'units: an aligned row', 'live'],
  ['FUN_004059af', 'units: a half-offset row', 'live'],
  ['Map_DrawArmies', 'a unit and its banner', 'live'],
  ['Screen_DrawMenuBar', 'the 640x24 menu bar', 'live'],
  ['CountyStrip_Draw', 'the county strip', 'live'],
  ['FUN_004100af', 'strip row: cattle', 'live'],
  ['FUN_0041023a', 'strip row: grain', 'live'],
  ['FUN_004103c5', 'strip row: reclamation', 'live'],
  ['FUN_00410502', 'strip row: stone', 'live'],
  ['FUN_00410598', 'strip row: wood', 'live'],
  ['FUN_0041062e', 'strip row: iron', 'live'],
  ['FUN_004106c4', 'strip row: weapons', 'live'],
  ['CountyStrip_DrawCastleIcon', 'strip row: castle', 'live'],
  ['Minimap_Draw', 'the minimap plate', 'live'],
  ['Minimap_DrawOverlay', 'the minimap tint', 'live'],
  ['Screen_DrawEndTurn', 'the End Turn strip', 'live'],
  ['FUN_0040c725', 'an open drop-down menu', 'live'],
  ['FUN_00420316', 'the multiplayer chat banner', 'live'],
  ['FUN_0042476b', 'the network-wait glyph', 'live'],
  ['FUN_0041a639', 'the turn timer', 'live'],
  ['FUN_0041a844', 'the multiplayer heartbeat', 'live'],
  ['FUN_00476e95', 'the tooltip', 'live'],
  ['FUN_004248a3', 'a four-number debug box', 'debug'],
  ['FUN_004247f5', 'DUMB VIEW ON', 'debug'],
  ['FUN_00406bba', 'a second tall-tile pass', 'dead'],
  ['FUN_0040619d', 'that pass, offset rows', 'dead'],
  ['FUN_004062fb', 'that pass, aligned rows', 'dead'],
  ['FUN_0041424a', 'zoom-1 left edge', 'dead'],
  ['FUN_0041432b', 'zoom-2 left edge', 'dead'],
];

const sites = process.argv.includes('--sites');
let total = 0, live = 0, debug = 0, dead = 0;
const rows = [];
for (const [name, what, status] of TREE) {
  const b = bodies.get(name);
  if (!b) { console.error(`MISSING ${name}`); continue; }
  let n = 0;
  const found = [];
  for (const line of b.text.split('\n')) {
    for (const p of LEAF) {
      const re = new RegExp('\\b' + p + '\\s*\\(');
      if (re.test(line)) { n++; found.push(p + '  ' + line.trim().slice(0, 90)); break; }
    }
  }
  // Blit tails: one per if/else-if/else group. Every group in this tree has
  // exactly the three-arm shape, so count groups = count of the unclipped arm.
  // Frame selections: one picture each, blitted through the shared tail below.
  // Only in the four sprite painters. Everywhere else `DAT_005C9288 = ` is the
  // *tile's own* stored frame being reloaded, which is the tile draw already
  // counted by TILE_DRAW, and counting it again would double every diamond.
  const FRAME_FNS = new Set(['Sprite_TopIt', 'Map_DrawPathMarker', 'Map_DrawArmies', 'FUN_00407f82']);
  for (const line of (FRAME_FNS.has(name) ? b.text.split('\n') : [])) {
    for (const re of FRAME_SET) {
      if (re.test(line)) { if (!NOT_A_DRAW.test(line)) { n++; found.push('frame ' + line.trim().slice(0, 90)); } break; }
    }
  }
  // A whole bank/clip/zoom ladder is one draw: the tile.
  if (TILE_DRAW.has(name)) { n += TILE_DRAW.get(name); found.push('tile  (bank x clip x zoom ladder, one diamond)'); }
  rows.push([name, b.addr, what, status, n]);
  total += n;
  if (status === 'live') live += n; else if (status === 'debug') debug += n; else dead += n;
  if (sites) { console.log(`== ${name} (0x00${b.addr}) ${what} [${status}]`); for (const f of found) console.log('   ' + f); }
}
if (!sites) {
  console.log('| function | addr | paints | status | draws |');
  console.log('|---|---|---|---|---:|');
  for (const r of rows) console.log(`| \`${r[0]}\` | 0x00${r[1]} | ${r[2]} | ${r[3]} | ${r[4]} |`);
}
console.log(`\ntotal ${total}   live ${live}   debug-gated ${debug}   dead ${dead}`);
