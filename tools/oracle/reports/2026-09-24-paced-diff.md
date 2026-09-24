# Paced trace diff: both sides step one Battle_Frame, one tick a step

2026-09-24. Branch `agent/trace-pacing` (on `integrate/w38` + `agent/trace-draws`). Original:
`trace-run.ps1 -Runs 2`, all 18 scenarios reached their last step, identical x2. Ours: every
scenario but `field-battle-manual` (its `.ours.jsonl` is 1.7 GB with draw records; the differ
fails with `ERR_STRING_TOO_LONG`). Before: the same scenarios on `integrate/w38` unchanged.

## What changed in the harness

| | before | after |
|---|---|---|
| original's lockstep unit | a present (`frames`) | a step: each `Battle_Frame` `0x004B99C0`, blocked at `local_c = FUN_004bb978()` `0x004B9A78`; a present with no `Battle_Frame` since the last (films) is a step too |
| ticks per unit, original | 0..n, from the virtual clock (`FUN_004BB978` throttle `local_8 < iVar3*10+2`) | the clock moves the throttle's interval each step: 1 whenever not withheld `[V]` measured, e.g. newgame-original 8701 steps / 8195 ticks / 2891 presents |
| recorded | nothing | `frame` records carry cumulative `ticks` and `loops`; the summary line `presents`, `ticks` |
| click / rclick / key / drag, ours | 1 frame | the controller's own counts: 2 steps before press, 2 before release; key 1 step |
| `dismiss` | screen 0x27 only; ours waited 24 frames, the original none | both: up to 24 steps for a box on 0x27 **or over the map** (`g_messageGroup` `0x0053E910`; ours `messages.is_open()`) |
| message 298 | ours posted it on every map | dropped: the original posts none since `H_GetDeviceCaps` reports 8 bits `[V]` no 298 in any original trace; zoom-map's harness click on its OK removed |
| `load`, ours | map at once | map, then 60 steps, as `$LoadSteps` ends |

## The four pacing artifacts

| artifact | before (original / ours) | after |
|---|---|---|
| tips 20 frames late | 200→201→202 at +62/+65 / +22/+62 frames; voice same frame / +12 | **gone**. newgame-original: tips 89/393/458 vs 87/391/456, voice +10 on both; load scenarios +63/+65, voice +10 on both |
| narration `S2xx_02+` | 10–12 takes / 3 | **gone**: both play `_02+` 71 steps after `_01` while the box is open (newgame-original f170/f171 vs f168/f169); `dismiss` closes most tips first on both sides |
| options tip 218 voice | `S218_01..03` / none | **gone**: 0x39 opens 10 steps after the box, before the voice is due; neither side speaks |
| AI steps per frame | e.g. raise-army armoury 253 / 0 | pacing part gone (original 88, one `AI_RunTurnStep` a tick); ours still runs **none** on the map or a panel: game difference 2 |

## Game differences that remain (`[V]` both traces)

1. **Right-click on an army:** now reaches the unit panel in ours (the right click was being
   eaten by a tip over the map); ours emits `UnitPanel_Draw` in the `army-panel` step, the
   original once before the anchor. `[I]` alignment only; not checked further.
2. **AI lords idle outside End Turn in ours:** army-panel, county-panels (5 ai records every panel
   step in the original, 0 ours), move-army, newgame `map`/`turn-2-ended`. Turn-order branch.
3. **Tip 209's OK does not close it in ours** (raise-army): ours narrates `S209_02..05` at +71.
4. **Turn phases:** ours emits no phase record before its first change (every scenario
   `(start)`); endturn-england `loaded` 2/5, `end-turn-clicked` 5/2; newgame `map` 2/5.
5. **End Turn season-4** (endturn-year): original 3 ai records, ours 0.
6. **Battles:** field-battle `confirm`/`outcome` screens 1/3, 7/9; ours a message and a voice at
   `confirm` the original has not; phase 0/7 at `outcome`.
7. **Launch:** screens 6/2 and music 2/0 before the title (ours has no pre-title screens).
8. **endturn-noorders:** message 110 original only (harness H1 of the full diff: the fixture loads
   into a battle).
9. Draws/text: see `2026-09-24-draws-diff.md`; counts differ on every scenario.

## Harness artifacts that remain

| # | what | reason |
|---|---|---|
| H1 | load scenarios: ours is 197 steps ahead in absolute frames | the original walks the setup pages to Load; ours loads directly. Spacing inside the map matches to 0–10 steps |
| H2 | `(start)` screen/music/film/phase counts | the same setup pages, the original's intro film skipped |
| H3 | `rand`, `slot`, `order` counts | FORMAT.md: counts only |
| H4 | field-battle-manual not diffed | ours' trace 1.7 GB |
| H5 | `DAT_0056D8B4` (5 s hold in `FUN_004BB978`) | still virtual-clock time; now ~12 ms a step at speed 90. Not seen to matter here |
