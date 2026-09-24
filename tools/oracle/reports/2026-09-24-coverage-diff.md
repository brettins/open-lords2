# Coverage trace diff: 69 scenarios, both sides fresh

2026-09-24. Branch `agent/trace-coverage` on `integrate/w42`. Original: `trace-run.ps1 -Runs 2`
(own install `E:\games\l2-trace-cov`, DLLs rebuilt from `native/l2-trace`), **69 of 69 reached their
last step, identical x2**. Ours: `cargo test --release -p l2-game --test trace -- --ignored`, all 69
written; three end in a `failed` record (below). Differ: `trace-diff.js --dir … --a orig --b ours`
over all 69 (555 KB, not committed). Per-position draw comparisons below are from the same traces.

New scenarios: `zoom-far-new`, `zoom-far-england`, `custom-map-00` … `custom-map-43`
(`tools/oracle/custom-maps.js` writes them), `conquest-siege`, `conquest-field`, `battle-orders`,
`army-panel-kinds`, `ai-turn-shields`.

## What each coverage row reached

| row | reached on the original | note |
|---|---|---|
| 1 far zoom | yes: 56 056 `TileBlit_Plain_Z2`, 704 `M3_Z2`, 704 `M4_Z2` (zoom-far-new); zoom-map had none because the snap came before the redraw | the pointer at x 1 / x 476 does not scroll the far view: far-left and far-right snaps hash the same (`ddb9f52e`) |
| 2 new games | 44 custom maps: `g_scenarioIndex` `0x0053F034` peeked 0 … 59, 44 distinct first map frames. Campaign starts: newgame-original/new | no battle is offered in turn one on any of the 44; the battle leg is not covered |
| 3 winning a county | **no**: both fixtures' battles are losses on the original (`ff_lose.wav` from `0x004AB80D`, then screen 0x13) | no fixture holds a winnable battle and the harness has no save step; not built |
| 4 battle orders | yes, markers recorded | |
| 5 army panel | partly: after the far-zoom click-to-zoom-in, the original opens screen 4 only on the *next* right click (`army-0` anchor f781, screen 4 f791, after the closing click) | the scenario is misaligned on the original; ours' texts are recorded |
| 6 AI turn shields | yes, 1500 recorded steps | |

## Divergences a player sees, ranked

**1. Custom game: the map list never scrolls; 39 of 44 maps cannot be chosen.** `[V]` both traces.
The original's down arrow (`FUN_004337B4` hotspot 1, rect 603..624 × 209..229, `.data` `0x4DCD90`)
steps `DAT_0053E90C` and reloads the map: peeks 0, 1, 2 … 39, then rows 1..4 give 40..43 (slot 59).
Ours: every `custom-page` snap draws the same list text (`101/1`, `101/2` …); ours' first map
frames take **5** distinct hashes over the 44 scenarios, the original's **44**. `hits.rs` registers
no arrow rect on page 7.

**2. AI turn: the menu-bar realm shields do not count down.** `[V]` ai-turn-shields.
Original, after End Turn: `misc_cty.pl8` 86, 87, 89, 90 at x 270, 286, 302, 318 (y 4) at +183
steps; 86, 87, 90 at +224; 86, 90 at +242; 86 at +300 — one shield leaves as each lord finishes,
the rest close up leftwards, and 88 is never among them. Ours: 86 … 90 at 270 … 334 from +91,
unchanged to the end of the 1500 steps.

**3. AI turn: every lord moves in the same step.** `[V]` ai-turn-shields `ai` records.
Original: one `AI_RunTurnStep` a step, realms 1..5 in turn from +76 (`76:1/0 77:2/0 … 81:1/1
82:2/2`); ours: four realms in one step from +124 (`124:1/0 124:2/0 … 125:2/1 125:3/1 125:4/1
125:5/1`), realm 1 not stepped after step 0. 44 custom maps: `ai` counts differ on every one
(original 2107, ours 386 / 186).

**4. Far zoom: armies are not drawn.** `[V]` zoom-far-england `far`.
Original: `sprite2a.pl8` 78 at (154, 50), (316, 125), (394, 254), (142, 278), each with a
`flags2a.pl8` shield at +24, −21. Ours: no `sprite2a` draw at all.

**5. Far zoom: the ground outside the play square is flat.** `[V]` zoom-far-england `far`.
1447 of 4142 `base2a.pl8` positions differ: original frames 23..29, 43, 63, 10 …; ours 22
(and 6 inside). The differing cells are the margins (y < 100, y > 300, x ≥ 360). E.g.
`0,21 base2a:23 vs 22`, `12,21 29 vs 22`, `84,21 43 vs 22`, `108,21 10 vs 6`.

**6. Far zoom: towns and their flags.** `[V]` same snap. Town frame 20 where ours draws 22 at five
towns (`192,99`, `324,135`, `372,237`, `156,279`); `114,210` 24 vs 23; town flag frames one
lower in ours (`flags2a` 15/14, 23/22, 7/6, 31/30, 39/38) and ours draws a flag over the town at
`210,78` the original has not.

**7. Battle order: no destination marker.** `[V]` battle-orders `ordered`.
Original: `misc_bat.pl8` 63 at (226, 321), the click at (240, 328). Ours: none. Selection
markers (`misc_bat` 62) sit at y 108 / 140 on the original ten steps after the order, ours at
122 / 126 (the men have closed ranks in ours, not in the original); on the overview order ours'
are +2, +2 of the original's.

**8. Battle: the player's men drawn from the other colour's sheet.** `[V]` battle-orders
`selected`: original `a2b_swor.pl8` 49 @0,104, `a2b_psnt.pl8` 41 @184,104 …; ours `a2r_swor` /
`a2r_psnt`, same frames and places (16 figures).

**9. A lost siege: no screen 0x13, a film, three tips.** `[V]` conquest-siege `after-banner`.
Original: `ff_lose.wav` (`0x004AB80D`) and screen 0x13, no film, no message. Ours: `cas_los1.smk`
before the banner (f2097), then the map with messages 200, 201, 202 and voices `S200_01..03`;
ours posts message 312 on the load frame the original has not.

**10. A hand-fought field battle never ends in ours.** `[V]` conquest-field, field-battle-manual.
Original: every man charged, screen 0x2B at f4039, `ff_lose.wav`, `scroll1.wav`. Ours: still on
0x29 at the 20 000-step `wait-screen 43` limit (`failed` record); ours shows 0x2A → 0x29 at f20177.

**11. Near zoom: the left edge column.** `[I]` 15 tiles a snap: the original records the
clipped left column at x 0, ours at x −30 (far: −6), same frames. Probably the original's
`g_drawX` after clipping, not a pixel difference; not checked against pixels.

**12. Custom-map turn one.** Every custom map: ours plays `merchant.wav` where the original plays
`army.wav` slot 12 repeatedly (`Sound_PlaySlot` from `0x00466E57`) at the map; message 206 comes
one step earlier in ours on six maps (01, 10, 16, 18, 33, 41). Worlds differ (D3 of the draws
report), so terrain is not compared.

## Carried over from the paced report, still present

AI lords idle outside End Turn in ours (every map step: army-panel, county-panels, options-pages,
move-army, zoom-*); turn phases (ours emits no phase before its first change); tip 209's OK
(raise-army: ours `failed` at screen 0x27 `[Campaign, RaiseArmy(8), Tip, Message]`); launch's
pre-title screens and music. Field-battle and siege autocalc: messages/voices at `confirm`.

## Harness artifacts

| # | what | reason |
|---|---|---|
| H1 | the original's snaps have no chrome (panels, `misc_cty`, menu-bar text) and no custom-page text; ours repaints all of it every snap | the original paints those once and only the changed area after; ours redraws the frame. Every `text` finding of the form "original side recorded no text" is this |
| H2 | load scenarios: ours ahead in absolute frames | the original walks the setup pages to Load (paced report H1) |
| H3 | `rand`, `slot`, `order` counts | counts only (FORMAT.md) |
| H4 | army-panel-kinds on the original | its right clicks open screen 4 one click late after a far-zoom click; the scenario needs a wait on the panel before it is a comparison |
| H5 | the near/far edge `move` does not scroll | measured on the original; no scroll step exists yet |
