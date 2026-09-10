#!/usr/bin/env node
// Count what a screen draws — in the original, and in ours — with the same rule.
//
//   node tools/draws/screendraws.js              # the table
//   node tools/draws/screendraws.js --sites 0x09 # every call site for one screen
//   node tools/draws/screendraws.js --check      # the two cheap checks, plus the ratio
//
// # Why the number is computed and never typed
//
// `docs/draws.md` §5: *per screen, draw calls in the original against draw calls
// in our painter, both counted by the same script, printed by the census and
// never typed.* `tools/figures/figures.js` set the precedent — **a number that
// cannot drift beats a number that is checked** — and 34 figures had already
// gone stale in four documents by being typed.
//
// So this reads two artefacts that no one person maintains together: the
// decompiled corpus (which nobody in this project writes) and
// `crates/l2-game/src/screens/*.rs` (which everybody does. `docs/agents.md`:
// *two artefacts that must agree and are maintained by the same person, at the
// same time, for the same reason, is the pattern that lies* — these are not.
//
// # What a draw call is, and it is the same rule on both sides
//
// **One call site, in the source text, of a leaf draw primitive.** A leaf is a
// function that puts a picture, a glyph run or a rectangle on the frame buffer
// and takes its subject as an argument. A call to another *painter* is a
// recursion and is followed rather than counted.
//
// A call inside a loop counts **once**. A call inside a branch nothing can
// reach counts **once**, because the audit's job is to compare two pieces of
// source and a reachability claim is a separate finding — see `dead` in the
// inventory.
//
// The unit is deliberately the call site and not the pixel. `docs/draws.md` §3
// found that the honest unit is the *sheet-and-frame reference*; a call site is
// the closest thing to that which can be counted mechanically on both sides,
// and where the two come apart the inventory record says so in `notes`.
//
// # What is excluded, by construction rather than by a hand list
//
// * **The campaign map's own drawing.** Every county panel is an inset over the
//   map and its painter's first call repaints the map beneath it. Descending
//   into that counts the map once per panel — it turned `Panel_Tax`'s 9 real
//   calls into 34. So anything reachable from `Screen_DrawCampaign`,
//   `Screen_DrawMenuBar`, `Screen_DrawEndTurn` or `Widget_Draw` belongs to the
//   map's audit (`tools/draws/mapdraws.js`) and stops this walk.
// * **`Widget_Draw` itself**, which is shared. Each widget *record* is one
//   thing on the screen, so the inventory counts records in `widgets` and says
//   so separately — and `g_sendSuppliesWidgets` is why: eight records, every
//   caller passes six, and two buttons exist that are never drawn.
// * **The blitters** (`Blit_*`, and the three clip-state tails), which are the
//   implementation of every primitive above rather than a thing on a screen.
'use strict';
const fs = require('fs');
const path = require('path');
const cp = require('child_process');

// ---------------------------------------------------------------- the corpus
//
// Gitignored, and it lives only in the checkout that built it — so an agent in
// a worktree has to be handed it. `git rev-parse --git-common-dir` names the
// main checkout from inside a worktree, which is the resolution order
// `tools/oracle/xref.js` settled on.
function corpus() {
  const i = process.argv.indexOf('--decomp');
  const explicit = i >= 0 ? process.argv[i + 1] : process.env.LORDS2_DECOMP;
  if (explicit) return explicit;
  const here = path.join(__dirname, '..', 'oracle', 'decomp');
  if (fs.existsSync(here)) return here;
  try {
    const common = cp.execFileSync('git', ['rev-parse', '--git-common-dir'],
      { cwd: __dirname, encoding: 'utf8' }).trim();
    const main = path.resolve(__dirname, common, '..');
    const there = path.join(main, 'tools', 'oracle', 'decomp');
    if (fs.existsSync(there)) return there;
  } catch { /* not a checkout; fall through so the error names a real path */ }
  return here;
}

const HDR = /^\/\/ ==== ([0-9a-f]{8})\s+(\S+)\s+params=(-?\d+)\s+bytes=(\d+)/;
const byName = new Map(), byAddr = new Map();
(function load() {
  // **Absent is not the same as wrong**, and conflating them is what
  // `docs/environment.md` records as having hidden a broken fixture for weeks.
  // A missing corpus makes the *original* side unmeasurable and leaves our side
  // perfectly measurable, so it is a skip here and a hard error only at the
  // point something actually needs the corpus.
  const dir = corpus();
  if (!fs.existsSync(dir)) return;
  for (const f of fs.readdirSync(dir).filter(f => f.endsWith('.c'))) {
    const lines = fs.readFileSync(path.join(dir, f), 'utf8').split(/\r?\n/);
    let cur = null;
    const push = () => { if (cur) { byName.set(cur.name, cur); byAddr.set(cur.addr, cur); } };
    for (const l of lines) {
      const m = HDR.exec(l);
      if (m) { push(); cur = { addr: m[1], name: m[2], bytes: +m[4], body: [] }; }
      else if (cur) cur.body.push(l);
    }
    push();
  }
})();

// ------------------------------------------------------------ the primitives
//
// Pinned by name from `docs/symbols.json`, not matched by a pattern: a pattern
// over `/Draw/` catches `Screen_Draw`, `App_Draw` and `Army_WithdrawCasualties`,
// and the last of those is not a draw at all.
//
// The `FUN_` entries are unnamed leaves whose bodies were read for this audit.
// **`FUN_004025D7` centres**: it is
// `Ui_DrawText(s, x + max(0, (width - w) / 2), y, …)`, which is why
// `Ui_DrawNumberRight` and `Ui_DrawCentred` both end in it and why the former's
// name is wrong. `docs/draws.md` §7.
const LEAF = new Set([
  'Pl8_DrawFrame', 'Pl8_DrawFrameHere', 'Pl8_DrawFrameClipped',
  'Eng_DrawString', 'Ui_DrawText', 'Ui_DrawCentred', 'Ui_DrawNumber',
  'Ui_DrawNumberRight', 'Ui_DrawCount', 'Ui_DrawDelta', 'Ui_DrawHappinessDelta',
  'Ui_DrawUnitNoun', 'Ui_DrawYear', 'Ui_DrawBox', 'Ui_DrawBoxBorder',
  'Ui_DrawBoxInterior', 'Ui_DrawInsetRect', 'Ui_DrawTileStrip',
  'Ui_DrawBevelRect', 'Ui_DrawMenuTitles', 'Ui_HistoryGraph', 'Ui_OkButton',
  'Glyph_Draw', 'Blit_Raster',
  'FUN_004025d7', // a string centred in a width, from a char*
  'FUN_0040328e', // a wrapped L2.eng paragraph
  'FUN_00403cf4', // a rectangle outline
  'FUN_0040437d', // a filled rectangle
  'FUN_00409346', // a framed box with a picture
  'FUN_00409429', // a framed box, no interior
  'FUN_004093e0', // the border-set window: Ui_DrawBoxBorder(set, …) + interior
  // **The full-screen backdrop, and it was missing from the first draft of
  // this list.** `FUN_00408FCB(name, height)` reads `g_screenStride * height`
  // bytes from offset `0x18` of a `.pl8` **straight into `g_backBufferBits`**.
  // `Merchant.pl8` and `Cas_back.pl8` are each 307,224 bytes = 640 * 480 + 24:
  // they are not sprite sheets at all, they are the picture.
  //
  // Leaving it out reported `Screen_LordsOfMagicAd` as **zero draws** and
  // `Screen_Merchant` as one, when the thing that dominates both screens is
  // this call. Note what the mistake was: the `.256` beside it really *is* not
  // a draw — `File_ReadChunk("merchant.256", …, 0x300)` is 768 bytes of palette
  // — and the two were conflated. Eleven painters call it.
  'FUN_00408fcb',
  // **A picture that is not a `Pl8_DrawFrame`, and the counter could not see
  // it.** `Sprite_WGenSprite` and its two siblings blit a sheet frame whose
  // *only* leaves are the three `Blit_*` clip states — which `NOT_A_DRAW`
  // correctly collapses to one logical blit, with the side effect that the
  // wrapper reached no leaf at all, `reachesLeaf` was false, and **its call
  // sites were never counted**. `Screen_BattlePrompt` and `Screen_BattleResult`
  // make four each (the `Icon_tmp.pl8` medallion and two realm shields);
  // `Screen_SiegePrep` one; the map information panel several. Naming the
  // wrapper as the leaf is the fix, and it works precisely because the walk
  // does not descend into a leaf.
  'Sprite_WGenSprite', 'Sprite_WGenFSprite', 'Sprite_WGenHSprite', 'Sprite_GenFrame',
  // A four-pixel line, and the whole of the standings screen's bar chart:
  // `Screen_GreatestNoble` draws four of these per realm and nothing else, so
  // without it that screen reads as five draws when it is a chart.
  'FUN_00403a8f',
  // The front end's own primitives, read for the setup-page audit. Without
  // them page 3 — the save browser — counted **zero**, which is exactly the
  // failure mode this list exists to avoid: a leaf set assembled from the
  // screens you have already read reports the screens you have not as empty.
  'FUN_00403ee4', // a bevelled recess (distinct from FUN_00403CF4's flat outline)
  'FUN_0040352f', // a wrapped paragraph from a char*, the sibling of FUN_0040328E
  'FUN_0040acce', // the text caret
  'FUN_00410c71', // a county map thumbnail
  'FUN_0040c725', // the open menu-bar drop-down, called only from the frame loop
]);

// One logical blit seen through three clip states, plus the clip-state tails.
// Counting them multiplies every sheet draw by three or four.
const NOT_A_DRAW = new Set([
  'Blit_Unclipped', 'Blit_ClippedLeft', 'Blit_ClippedRight',
  'FUN_004b4537', 'FUN_004b4814', 'FUN_004b48b8',
]);

// The shared substrate, expanded transitively so the list cannot go stale when
// somebody names one of its members.
// `Net_PumpReceive` is here for a different reason from the other three and it
// is worth saying which: **pumping the network is not drawing, and the walk
// escaped through it.** `Screen_BattlePrompt` and `Screen_BattleResult` are the
// only two painters that call `Gfx_LoadCountyMode`, and from there the path runs
//
//   Gfx_LoadCountyMode -> Net_PumpReceive -> Net_OnSessionEvent -> Net_LeaveGame
//     -> Smk_Skip -> Smk_OnFinished -> Screen_DrawBattlefield -> the whole ladder
//
// so the two battle-seam screens counted **228** and **226** draw calls, most of
// them the battlefield's. With this root they count 35 and 33. Adding it changes
// no other screen's figure — checked across all eleven enumerated at the time.
const SUBSTRATE_ROOTS = ['Screen_DrawCampaign', 'Screen_DrawMenuBar',
  'Screen_DrawEndTurn', 'Widget_Draw', 'Net_PumpReceive'];
const SUBSTRATE = new Set();
const CALL = /\b([A-Za-z_][A-Za-z0-9_]*)\s*\(/g;
// **A comment is a region, not a line, and the first draft of this got it
// wrong.** Every decompiled function carries a `/* … */` block written by this
// project, and those blocks quote call sites back at you — `Armoury_LoadScreen`'s
// says *"Widget_Draw(0x60, 4, …)"*, and its doc comment alone contributed three
// phantom draw calls, reporting seven where the body has four.
//
// Skipping lines that *start* with comment punctuation is not enough, because a
// continuation line of a block comment starts with the quoted text itself. So
// the state is tracked across lines. Found by an agent re-reading a body against
// the tool's number, which is the only thing that ever catches a tool: something
// external contradicting it. `docs/agents.md`, *a tool that degrades silently*.
// **And a call is a region too, for the same reason.** Ghidra wraps a long
// argument list by putting the `(` on the *next* line:
//
//     Ui_DrawHappinessDelta
//               ((int)g_realms[b].taxHapEmpire + ...,0xf0
//                ,0xe8,&g_fontBody,0x3f,0xf9);
//
// A line-by-line scan for `name(` cannot see that, and the name is then the
// last thing on its own line with nothing after it. It costs **five** call
// sites in 1,334 across the whole corpus — two on the tax panel, two on the
// ration panel, one in `Panel_JobBlacksmith` — and every one of them
// **under-reports the denominator**, which is the direction `docs/agents.md`
// says is the dangerous one: a smaller original makes us look closer to 1:1
// than we are, and a count has no local evidence of being wrong.
//
// Both of this function's bugs were found the same way, by an agent re-reading
// a body against the tool's number. Nothing in the tool could have noticed
// either.
function callsIn(fn) {
  let inBlock = false;
  const stripped = fn.body.map(l => {
    let text = l;
    if (inBlock) {
      const end = text.indexOf('*/');
      if (end < 0) return '';
      text = text.slice(end + 2);
      inBlock = false;
    }
    text = text.replace(/\/\*[\s\S]*?\*\//g, ' ');
    const open = text.indexOf('/*');
    if (open >= 0) { inBlock = true; text = text.slice(0, open); }
    const line = text.indexOf('//');
    if (line >= 0) text = text.slice(0, line);
    return text;
  });
  // Pull a continuation that begins with `(` back onto the line before it, so
  // the name and its parenthesis meet. Comments are already gone, so this
  // cannot join a line into a `//` that would then swallow it.
  const joined = stripped.join('\n').replace(/\n\s*(?=\()/g, '').split('\n');
  const out = [];
  joined.forEach((text, i) => {
    if (!text.trim()) return;
    let m; CALL.lastIndex = 0;
    while ((m = CALL.exec(text))) out.push({ line: i, name: m[1], text: text.trim() });
  });
  return out;
}
function lookup(n) {
  return byName.get(n) || byAddr.get(String(n).replace(/^(FUN_|0x)/, '').toLowerCase().padStart(8, '0'));
}
(function seedSubstrate() {
  const q = [...SUBSTRATE_ROOTS];
  while (q.length) {
    const n = q.pop();
    if (SUBSTRATE.has(n) || LEAF.has(n)) continue;
    SUBSTRATE.add(n);
    const fn = lookup(n);
    if (!fn) continue;
    for (const c of callsIn(fn)) if (!SUBSTRATE.has(c.name)) q.push(c.name);
  }
  for (const r of SUBSTRATE_ROOTS) SUBSTRATE.add(r);
})();

// Does this function reach a leaf at all? Used to decide whether descending
// into a callee is worth it — a helper that draws nothing is not a painter.
const reachMemo = new Map();
function reachesLeaf(n, stack = new Set()) {
  if (LEAF.has(n)) return true;
  if (NOT_A_DRAW.has(n)) return false;
  if (reachMemo.has(n)) return reachMemo.get(n);
  if (stack.has(n) || stack.size > 8) return false;
  const fn = lookup(n);
  if (!fn) return false;
  stack.add(n);
  let r = false;
  for (const c of callsIn(fn)) if (reachesLeaf(c.name, stack)) { r = true; break; }
  stack.delete(n);
  reachMemo.set(n, r);
  return r;
}

/** Every leaf call site reachable from `roots`, following painters. */
function originalSites(roots) {
  const seen = new Set(), sites = [];
  const walk = (n, depth) => {
    const fn = lookup(n);
    if (!fn) { sites.push({ missing: n }); return; }
    if (seen.has(fn.name)) return;
    seen.add(fn.name);
    for (const c of callsIn(fn)) {
      if (NOT_A_DRAW.has(c.name)) continue;
      if (LEAF.has(c.name)) sites.push({ from: fn.name, depth, ...c });
      else if (!SUBSTRATE.has(c.name) && reachesLeaf(c.name)) walk(c.name, depth + 1);
    }
  };
  for (const r of roots) walk(r, 0);
  return sites;
}

// ------------------------------------------------------------------- our side
//
// The same rule, applied to `crates/l2-game/src/screens/*.rs`: one call site of
// a method that puts something on the `Canvas`. The list is our `Pen`'s API
// (`crates/l2-game/src/shell/mod.rs`), each entry against the original it
// stands for — so a reader can check the mapping rather than trust it.
const OURS = new Map(Object.entries({
  window: 'Ui_DrawBox / FUN_004093E0',
  window_from: 'Ui_DrawBox, origin from a rect',
  box_interior: 'Ui_DrawBoxInterior',
  inset: 'Ui_DrawInsetRect',
  // **The original's rectangle outline, which our side had no way to write.**
  // `FUN_00403CF4` is in LEAF above, so it counts on the original's side; the
  // only thing in this tree that drew one was `widget::frame`, which is counted
  // as a placeholder — correctly, because it takes an `Ink` colour rather than
  // the painter's literal. `Pen::outline` is the primitive with its palette
  // index, so a call site can now be the original's rather than ours. The
  // sibling leaves `FUN_0040437D` (a filled rectangle) and `FUN_00403A8F` (a
  // line) still have no `Pen` counterpart and are drawn with a bare
  // `canvas.fill_rect`, which this scanner cannot see at all — an asymmetry
  // that under-reports our side, recorded here rather than papered over.
  outline: 'FUN_00403CF4',
  body: 'Ui_DrawText, body font',
  heading: 'Ui_DrawText, heading font',
  body_centred: 'FUN_004025D7',
  heading_centred: 'FUN_004025D7',
  body_wrapped: 'FUN_0040328E',
  eng: 'Eng_DrawString',
  eng_centred: 'Ui_DrawCentred',
  eng_heading_centred: 'Ui_DrawCentred, heading font',
  number: 'Ui_DrawNumber',
  number_centred: 'Ui_DrawNumberRight  (which centres)',
  count: 'Ui_DrawCount',
  misc_frame: 'Pl8_DrawFrame, Misc_cty.pl8',
  system_frame: 'Pl8_DrawFrame, System.pl8',
  frame: 'Pl8_DrawFrame',
  ok_button: 'Ui_OkButton',
  increase_button: 'Widget_Draw, frame 68',
  decrease_button: 'Widget_Draw, frame 66',
  strip_hotspot: 'a county-strip row',
  background: 'the .256 plate',
  palette: 'the .256 palette',
}));
// `a.text(group, index)` is a *lookup* and not a draw; it is the argument to
// one. Counting it double-counts every `eng`. Named here so the omission is a
// decision rather than an oversight.
const NOT_OURS = new Set(['text', 'sheet', 'rect', 'index', 'has_grid', 'fallback', 'wrap', 'screen_id']);

// --------------------------------------------- the second axis: is it the game's?
//
// **A screen can reproduce every draw call and still be entirely placeholder,
// and nothing was counting that.** It is the half of the player's report the
// call count cannot see — *"I see placeholder shit everywhere"* was made about
// screens whose draw counts are fine.
//
// So each of our draws is one of two kinds:
//
// * **real** — it goes through the game's own assets. Every `Pen` method draws
//   with `Fntl2_14.pl8` / `Fntl2_22.pl8` through
//   `crates/l2-game/src/shell/font.rs`, and every `draw_*` on the chrome or the
//   village art blits a frame out of a `.pl8` the player owns.
// * **placeholder** — `l2_view::text` is our own hand-authored 5 x 7 bitmap
//   font and `widget::panel` / `frame` / `button` are our own rectangles. They
//   are the right thing for a debug overlay and the wrong thing for a screen
//   whose whole claim is that it shows what the game showed.
//
// Neither number is a verdict on its own: a placeholder mark is correct on
// `screens/index.rs`, which is ours on purpose and says so. The ratio is the
// lead, and the record's `notes` is where a deliberate one is defended.
const PLACEHOLDER = /\b(text::draw|text::draw_centred|text::draw_right|widget::panel|widget::frame|widget::button|widget::label)\s*\(/g;
// A sheet or chrome blit, **and a `Font` drawn directly rather than through
// `Pen`.** The second half was a false negative and it cost a wrong number in a
// brief: `menubar.rs` was reported as drawing nothing through the game's own
// artwork when it had been drawing every caption with the install's real
// `Fntl2_14.pl8` all along, through
// `ctx.assets.shell.body.as_ref().map(|f| f.draw(canvas, ..))`. A grep for
// `pen.` cannot see that. `\.draw` cannot match `text::draw`, which is reached
// through `::`, so the two stay distinguishable.
const REAL_EXTRA = /\b[a-z_0-9]+\.(draw|draw_centred|draw_right|draw_system|draw_misc|draw_panel_frame|draw_strip|draw_box|draw_banner|draw_scene|draw_tops|draw_resources|draw_animations|draw_menu_bar_background|draw_right_panel|draw_minimap_side|draw_minimap_badge)\s*\(\s*canvas/g;

// **An English caption written in our source where the original fetches an
// `L2.eng` string.** This is an invention in the strictest sense — words on a
// screen the original never puts there — and it is the single most mechanical
// signal in the audit. It is a *lead*, not a verdict: a genuine diagnostic of
// ours (an asset-missing warning, a status line) is a legitimate literal and
// belongs in the record's `literals_ours` with a reason.
const LITERAL = /\b(text::draw|text::draw_centred|text::draw_right|widget::button|widget::label)\s*\([^;]*?"([A-Za-z][^"]*)"/g;

function ourKinds(moduleFile) {
  const p = path.join(SCREENS_DIR, moduleFile);
  if (!fs.existsSync(p)) return null;
  let real = 0, placeholder = 0;
  const literals = [];
  fs.readFileSync(p, 'utf8').split(/\r?\n/).forEach((l, i) => {
    if (/^\s*(\/\/|\/\*|\*)/.test(l)) return;
    let m;
    OURS_CALL.lastIndex = 0;
    while ((m = OURS_CALL.exec(l))) if (!NOT_OURS.has(m[1]) && OURS.has(m[1])) real++;
    REAL_EXTRA.lastIndex = 0; while (REAL_EXTRA.exec(l)) real++;
    PLACEHOLDER.lastIndex = 0; while (PLACEHOLDER.exec(l)) placeholder++;
    LITERAL.lastIndex = 0;
    while ((m = LITERAL.exec(l))) literals.push({ line: i + 1, text: m[2] });
  });
  return { real, placeholder, literals };
}

const SCREENS_DIR = path.join(__dirname, '..', '..', 'crates', 'l2-game', 'src', 'screens');
const OURS_CALL = /\b[a-z_][a-z_0-9]*\.([a-z_][a-z_0-9]*)\s*\(/g;
function ourSites(moduleFile) {
  const p = path.join(SCREENS_DIR, moduleFile);
  if (!fs.existsSync(p)) return null;
  const sites = [];
  fs.readFileSync(p, 'utf8').split(/\r?\n/).forEach((l, i) => {
    if (/^\s*(\/\/|\/\*|\*)/.test(l)) return;   // a comment, including `//!` docs
    let m; OURS_CALL.lastIndex = 0;
    while ((m = OURS_CALL.exec(l))) {
      if (NOT_OURS.has(m[1])) continue;
      if (OURS.has(m[1])) sites.push({ line: i + 1, name: m[1], text: l.trim() });
    }
  });
  return sites;
}

// ---------------------------------------------------------------- the inventory
const INV = path.join(__dirname, 'screens.json');
function inventory() {
  if (!fs.existsSync(INV)) {
    console.error(`no inventory at ${INV}`);
    process.exit(2);
  }
  return JSON.parse(fs.readFileSync(INV, 'utf8'));
}

// **Why `original` is stored in the inventory rather than always recomputed.**
//
// The decompiled corpus is gitignored and lives only in a checkout that built
// it, so CI has no way to count the original's draw calls — and the number that
// matters most is the one CI must be able to check. So the count is written
// into `screens.json` by `--write`, and there are then **two** places it cannot
// drift:
//
// * `tools/figures/figures.js` keeps every quoted figure equal to
//   `screens.json`, in CI, with no corpus;
// * `--check` recomputes it from the corpus and fails on a mismatch, for
//   anybody who has one. It **skips** rather than passes when the corpus is
//   absent, and says so — the distinction `l2_testkit` draws between *absent*
//   and *wrong game*, for the same reason.
//
// Neither is maintained by the work that maintains the other: nobody in this
// project writes the corpus, and `figures.js` never reads it.
function writeCounts() {
  const inv = inventory();
  let moved = 0;
  for (const rec of inv) {
    const o = originalSites(rec.roots).length;
    if (rec.original !== o) { moved++; console.log(`  ${rec.screen} ${rec.name}: ${rec.original ?? '—'} -> ${o}`); }
    rec.original = o;
  }
  fs.writeFileSync(INV, JSON.stringify(inv, null, 2) + '\n');
  console.log(moved ? `${moved} counts rewritten in ${INV}` : 'every stored count already matched the corpus');
}

function checkCounts() {
  const inv = inventory();
  if (!byName.size) {
    console.log('SKIP screendraws --check: no decompiled corpus; the stored counts were not verified');
    return 0;
  }
  let bad = 0;
  for (const rec of inv) {
    const o = originalSites(rec.roots).length;
    if (rec.original !== o) {
      bad++;
      console.error(`screendraws: ${rec.screen} ${rec.name} stores original=${rec.original} ` +
        `but ${rec.roots.join(' + ')} makes ${o} draw calls.\n` +
        `  run: node tools/draws/screendraws.js --write`);
    }
    if (rec.module && !fs.existsSync(path.join(SCREENS_DIR, rec.module))) {
      bad++;
      console.error(`screendraws: ${rec.screen} names module ${rec.module}, which does not exist`);
    }
  }
  // A record with no `roots` is a record nobody enumerated; it would count 0
  // and read as a finished screen. Make that unrepresentable rather than
  // checkable — `docs/agents.md`, *prefer a shape that cannot be wrong*.
  //
  // The one legitimate empty is a screen that **is ours** and has no original
  // to walk: `screens/menu.rs` is a main menu we invented. Those declare
  // `"painter": null`, which is a claim ("there is no such function") rather
  // than an omission ("nobody looked"), and the two must not look alike.
  for (const rec of inv) {
    const ours = rec.painter === null || rec.painter === undefined;
    if ((!Array.isArray(rec.roots) || rec.roots.length === 0) && !ours) {
      bad++;
      console.error(`screendraws: ${rec.screen} ${rec.name} has no roots — an unenumerated ` +
        `screen counts 0 and reads as a finished one.\n` +
        `  if it is ours and the original has no painter, say so with "painter": null`);
    }
  }
  if (!bad) console.log(`screendraws: ${inv.length} screens, every stored count matches the corpus`);
  return bad ? 1 : 0;
}

function main() {
  if (process.argv.includes('--write')) return writeCounts();
  if (process.argv.includes('--check')) return process.exit(checkCounts());
  const inv = inventory();
  const wantSites = process.argv.indexOf('--sites');
  if (wantSites >= 0) {
    const id = process.argv[wantSites + 1];
    const rec = inv.find(r => r.screen === id);
    if (!rec) { console.error(`no screen ${id} in the inventory`); process.exit(2); }
    console.log(`=== ${rec.screen} ${rec.name} — the original`);
    for (const s of originalSites(rec.roots)) {
      console.log(s.missing ? `  !! ${s.missing} is not in the corpus`
        : `  ${'  '.repeat(s.depth)}${s.from}: ${s.text}`);
    }
    console.log(`=== ${rec.screen} ${rec.name} — ours (${rec.module || 'no module'})`);
    for (const s of (rec.module ? ourSites(rec.module) || [] : [])) {
      console.log(`  ${rec.module}:${s.line}  ${s.name}  — ${OURS.get(s.name)}`);
    }
    const k = rec.module ? ourKinds(rec.module) : null;
    if (k) {
      console.log(`=== ${rec.module}: ${k.real} through the game's own artwork, ` +
        `${k.placeholder} in our 5x7 font and our own rectangles`);
      for (const l of k.literals) console.log(`  ${rec.module}:${l.line}  English we wrote: "${l.text}"`);
    }
    return;
  }

  let to = 0, tw = 0, tm = 0, ti = 0, tr = 0, tp = 0, tl = 0;
  const rows = [];
  const seenModules = new Set();
  // **Our side is counted per *module*, once, and this is not a detail.**
  //
  // A module can serve several screens: `saveload.rs` is `0x35` and `0x36`,
  // `merchant.rs` is `0x08` and `0x0C`, `armoury.rs` is `0x0A` and `0x0D`, and
  // `setup.rs` is **thirteen** pages. Counting its draws once per screen made
  // the totals read 1,036 of 1,007 — *more* than the original, from a tree with
  // fifty-six enumerated omissions in it.
  //
  // That is exactly `docs/agents.md`'s worked example in the other direction: a
  // counting file whose count is wrong has no local evidence of being wrong,
  // and 1,036 would have been quoted as a headline. So the file is the unit on
  // our side, the first record naming it carries its figure, and every later
  // one shows `shared`.
  for (const rec of inv) {
    const o = originalSites(rec.roots).length;
    to += o;
    tm += (rec.missing || []).length;
    ti += (rec.invented || []).length;
    let w = null;
    if (rec.module && !seenModules.has(rec.module)) {
      seenModules.add(rec.module);
      const k = ourKinds(rec.module) || { real: 0, placeholder: 0, literals: [] };
      // **Every mark, not only the `Pen` ones.** `ours` counted `Pen` calls
      // alone and came to 397 against a `real + placeholder` of 500 — so the
      // ratio was quietly understating us by a fifth, because a chrome or
      // village-art blit (`ch.draw_misc`, `a.draw_scene`) is a draw and a
      // placeholder rectangle is a draw we should not be making. Both belong in
      // the numerator; the *split* between them is the separate question §8 is
      // about.
      w = k.real + k.placeholder;
      tw += w;
      tr += k.real; tp += k.placeholder; tl += k.literals.length;
    } else if (!rec.module) {
      w = 0;
    }
    rows.push([rec.screen, rec.name, rec.painter, o, w === null ? 'shared' : w,
      (rec.missing || []).length, (rec.invented || []).length,
      w === null ? '—' : o ? Math.round((100 * Math.min(w, o)) / o) + '%' : '—']);
  }
  const head = ['id', 'screen', 'painter', 'orig', 'ours', 'missing', 'ours-only', 'ratio'];
  const wid = head.map((h, i) => Math.max(h.length, ...rows.map(r => String(r[i]).length)));
  const line = r => r.map((c, i) => i <= 2 ? String(c).padEnd(wid[i]) : String(c).padStart(wid[i])).join('  ');
  console.log(line(head));
  console.log(wid.map(w => '-'.repeat(w)).join('  '));
  for (const r of rows) console.log(line(r));
  console.log(wid.map(w => '-'.repeat(w)).join('  '));
  console.log(line(['', `${rows.length} screens`, '', to, tw, tm, ti,
    to ? Math.round((100 * Math.min(tw, to)) / to) + '%' : '—']));
  console.log();
  console.log(`  ${to} draw calls in the original, ${tw} in ours.`);
  console.log(`  ${tm} enumerated as missing, ${ti} as ours and not the original's.`);
  console.log();
  console.log(`  And the half the count cannot see: of our ${tr + tp} marks across`);
  console.log(`  ${seenModules.size} modules, ${tr} go through the game's own artwork and`);
  console.log(`  ${tp} are our 5x7 debug font and our own rectangles.`);
  console.log(`  ${tl} English captions are written in our source where the original`);
  console.log(`  fetches an L2.eng string.`);
  console.log();
  console.log(`  Excluded from the denominator: the campaign map (tools/draws/mapdraws.js),`);
  console.log(`  Widget_Draw's records (counted separately), and the blitters.`);
}

/** Every screen module, whether or not the inventory has reached it yet. */
function allModules() {
  return fs.readdirSync(SCREENS_DIR).filter(f => f.endsWith('.rs') && f !== 'mod.rs');
}
module.exports = { originalSites, ourSites, ourKinds, allModules, inventory, OURS, LEAF };

if (require.main === module) main();
