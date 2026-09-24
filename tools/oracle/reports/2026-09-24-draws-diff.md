# Draw and text diff: eight scenarios, the original against ours

2026-09-24. Branch `agent/trace-draws`. Original: the `trace-run.ps1 -Runs 2` traces of
`2026-09-24-full-diff.md` (`C:\Temp\b2d\t`), not re-run. Ours: `L2_TRACE_SCENARIOS=<list>
L2_TRACE_OUT=<dir> cargo test --release -p l2-game --test trace -- --ignored`. Differ:
`node tools/oracle/trace-diff.js --dir <dir> --a orig --b ours <names>`. Closes harness gap H4. C246.

## What ours now emits

| draw | ours' site | record |
|---|---|---|
| map terrain | `campaign::view::blit_cell` | `TileBlit_{Plain,M3,M4}_Z{zoom}`, at the tile origin (`g_drawX/Y`), not the overhang's top |
| map figures | `render::units::draw_unit` | `Blit_Unclipped` / `ClippedLeft` / `ClippedRight` |
| flags, banners, path balls, herds, besieger mark | `render::markers`, `draw_unit_banner` | `FUN_004b4537` / `4814` / `48b8` |
| everything else from a `.pl8` | `Canvas::blit_full` | `Pl8_DrawFrame`; the sheet is now named for every `Sheet`, not only `Assets::sheet` |
| Ui leaves | `Pen::window`, `box_interior`, `inset`, `outline`, `ok_button` | `Ui_DrawBox`/`Ui_DrawBoxStyle1`, `Ui_DrawBoxInterior`, `Ui_DrawInsetRect`, `Ui_DrawRectOutline`, `Ui_OkButton`; their inner blits dropped, as the original's are |
| text | `Pen::eng`/`eng_in` | `Eng_DrawString` (as before) |
| | `eng_centred`, `body_centred`, `body_wrapped`, `number_in`, `number_centred`, `count_*` | `Ui_DrawCentred`, `Ui_DrawTextCentredIn`, `FUN_0040328E`, `Ui_DrawNumber`, `Ui_DrawNumberRight`, `Ui_DrawCount` |
| | `Pen::body`/`heading`/`text_in` | `Ui_DrawText` with `s`, `font` (`5x7` = our debug fallback), and `group/index` when `s` is an `L2.eng` string |

The map's terrain is painted into a cache (`MapScreen::ensure`); its records are replayed on every
frame the cache is copied out, as the original repaints the terrain every frame. The differ now
keys a Ui leaf by its arguments and a literal by its text, and compares distinct draws per snap
(the original repaints some leaves 2 to 9 times in one frame). Not instrumented: the minimap
(`Blit_Raster`) and the battlefield's leaf names (its blits record as `Pl8_DrawFrame`); no battle
scenario reaches the battlefield in ours.

## Game differences, most visible first

`[V]` both traces show it; `[I]` the cause, inferred. "Snap n" is the n-th `snap` step.

**1. After loading a save, ours paints the new game's map.** `[V]` army-panel and move-army load
`battle-before.sav`. Ours' first map frame draws the same 263 terrain tiles, tile for tile, as
ours' new game (zoom-map snap 0): 263 of 263 shared; the original's two share 5. Run alone, the
army-panel scenario gives the same (no cached state from an earlier scenario). The army the
scenario clicks is therefore not on screen in ours: the original's `sprite1a.pl8` 78 at
(214, 246) and its banner are missing; ours flies a banner of the save's colour (frame 32)
where the new game's banner stands. `[I]` ours keeps the new game's view origin (or map) and
takes only the units from the save; this is full-diff finding 4 (the right click opens the
tile panel) and why move-army's click opens a county (6).
```
original: base1d.pl8:49@360,459 base1d.pl8:7@300,459 mtns1d.pl8:7@240,459  sprite1a.pl8:78@214,246  flags1a.pl8:32@232,225
ours:     base1d.pl8:28@360,459 base1d.pl8:25@300,459 base1d.pl8:25@240,459  (no sprite1a)       flags1a.pl8:32@266,161
ours zoom-map (new game): base1d.pl8:28@360,459 base1d.pl8:25@300,459 base1d.pl8:25@240,459  flags1a.pl8:3@266,161
```

**2. A quarter of the terrain tiles show another picture.** `[V]` The England save
(county-panels, options-pages, endturn-england, snap 0): all 263 tiles are drawn at the
original's positions; 182 have the original's frame, 65 the same place with another frame of
the same sheet, mostly one or two frames higher. `[I]` a tile's frame variant (`stored & 3`,
the field variant) or an animation phase.
```
original: base1d.pl8:100@240,459  base1d.pl8:22@180,459  base1d.pl8:27@120,459  base1d.pl8:28@60,459
ours:     base1d.pl8:101@240,459  base1d.pl8:24@180,459  base1d.pl8:29@120,459  base1d.pl8:29@60,459
```

**3. Tip windows are up on different frames.** `[V]` On the first map frame after a load, ours
has tip 202 open over the map; the original has no window in that frame. At zoom-map's far zoom
the original shows tip 206, ours none. The tip body differs in shape too: the original draws each
paragraph once, `FUN_0040328E(group, index)`, wrapped inside; ours draws line by line, so only
lines that are a whole string resolve to a group.
```
original zoom-map snap 1: Ui_DrawBoxStyle1(16,128,28,12) Ui_OkButton(416,272,0) 206/0@32,148 206/1@32,128
ours     snap 0 (loaded): Ui_DrawBoxStyle1(16,64,28,19) Ui_OkButton(416,320,0) 202/4@32,272 202/5@32,304
                          "Wheat can be bought from a merchant and planted"@32,176 ...
```

**4. Options pages: ours is not showing the page the original shows.** `[V]` options-pages.
At `advanced` the original repaints four `system.pl8` 25 toggles; ours paints only the map. At
`sound` the original repaints three toggles; ours paints a different window (box, inset, four
`icon_tmp.pl8` pictures, `L2.eng` 30/52, 100/8, 77/28). At `display` and `help` the toggles
match (2 of 2, 3 of 3); ours adds `system.pl8` 51 in each.
```
original advanced: system.pl8:25@280,156 @280,188 @280,220 @280,252
ours     advanced: (map only)
ours     display:  system.pl8:51@392,256    help: system.pl8:51@404,262
```

**5. County panels: no hover outline, and the caption drawn as two lines.** `[V]`
county-panels. The original frames the hovered button with `Ui_DrawRectOutline` on every panel;
ours draws no outline record on any panel. The hover caption is one `Eng_DrawString` in the
original, two literal lines at the same place in ours (and the same two again at (0, 0), `[I]`
an off-screen measure). Rations: the original's `FUN_00403a8f` lines and `FUN_0040437d` bar at
(224, 229..234) have no record in ours (drawn by `fill_rect` or not at all; not settled). The
widget frames match: tax 2 of 2, rations 5 of 5, court 1 of 1.
```
original population: Ui_DrawRectOutline(280,225,160,40,63)  220/6@284,229
ours     population: "View population"@284,229  "report"@284,245  "View population"@0,0
original rations:    FUN_00403a8f(224,229,323,229) FUN_0040437d(224,230,100,4,63) Ui_DrawRectOutline(380,255,128,22,63)
```

**6. After the move order ours is on a village screen.** `[V]` move-army snap 3 (`ordered`):
the original shows the army moved (`sprite1a.pl8:78@394,246`, banner at (412, 225)); ours paints
`vill.pl8:0@64,64`, `villani1.pl8`, `villani2.pl8`. Follows from 1.

**7. Flag animation phase.** `[V]` options-pages snap 5: the nine county flags are at the
original's places with frame 96 against the original's 91. Low visibility.

**8. Ours paints flags off screen.** `[V]` 34 to 117 `flags1a.pl8` draws a frame, of which 9 or 10
fall near the view (e.g. `flags1a.pl8:8@446,-469`, `@1286,-79`); the rest are clipped away.
Invisible; cost only.

## Harness notes (not the game)

| | note |
|---|---|
| D1 | The original repaints only what changed. 10 of 31 snaps have no draw on its side ("original repainted nothing"), and a panel's snap holds only the hover caption. Panels need `record N` steps to compare whole. |
| D2 | The menu bar and sidebar (`panels.pl8` 27 frames, `misc_cty.pl8` 13 to 16) and the menu bar's text (`#1268@360,6`, `29/4@414,6`, `8/1@500,6`) are in ours on every frame and in no snap of the original's (painted once). |
| D3 | zoom-map and newgame-original: ours' new world is not the original's (10 of 263 tiles exact); terrain there is not comparable. |
| D4 | raise-army: ours stops at `wait-screen 0` (f832) and `ok` (f863); no snap. |
| D5 | Rule 5: `FUN_004b4814` as the left-clipped and `FUN_004b48b8` as the right-clipped flag blitter, and `TileBlit_M3`/`M4` as the left/right seam, are `[I]` from where the original calls them (M3 at x 0, M4 at x 450, `48b8` at x 454). The differ does not compare names. |
