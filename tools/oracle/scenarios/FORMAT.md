# Trace scenarios and records

One scenario drives both games: `native/l2-trace` (the original) and
`crates/l2-game/tests/trace` (ours, `cargo test -p l2-game --test trace`). Each writes
`<scenario>.orig.jsonl` / `<scenario>.ours.jsonl`; a differ compares them. Decision C246.

## Scenario: `<name>.jsonl`, one step a line

Blank lines and lines starting `//` are skipped. Numbers may be decimal or a `"0x.."` string.

| op | fields | meaning |
|---|---|---|
| `load` | `save` | a save by fixture name; the campaign map over it |
| `new-game` | | the world `main.rs` builds behind the title page |
| `click` | `x`, `y`, `button`? (`"right"`) | pointer to (x, y), press, one frame, release |
| `move` | `x`, `y` | pointer to (x, y), no button: the map's path preview needs hover frames before the click |
| `drag` | `x`, `y`, `x2`, `y2` | left button held from (x, y) to (x2, y2) over 8 moves |
| `record` | `frames` | run that many frames keeping their draw and text records (a painter that draws once is seen) |
| `key` | `key` | `Escape`, `Enter`, `Space`, `Backspace`, arrows, or one character |
| `wait-screen` | `id`, `max`? | frames until `g_screenId` (`0x004EAC50`) is `id` |
| `wait-screen-not` | `id` | frames until `g_screenId` is not `id` |
| `wait-event` | `event` | frames until one more raw event of that key (`call:save`); count taken at the step, so not after a click that already fired it |
| `wait-phase` | `phase`, `max`? | frames until `g_turnPhase` (`0x00569584`) is `phase` |
| `wait-idle` | `frames`, `max`? | frames until `frames` in a row emit nothing but `draw`, `text`, `rand` (the original: no sound, music, voice, message, ai, film, save or new-game call and no screen or phase change) |
| `advance` | `frames` | run that many frames |
| `snap` | | a `snap` record: FNV-1a of the 640x480 indexed frame |
| `anchor` | `label` | an `anchor` record; the differ aligns the two streams on anchors |

`max` defaults to 20000 frames; a wait that runs out fails the scenario.

## Record: one JSON object a line

```json
{"frame":12,"kind":"sound","addr":"0x00427990","name":"Sound_PlayFile","caller":null,"args":{"file":"s011_02.wav","speech":0}}
```

`addr` and `name` are the original's function (or global) as `docs/symbols.md` names it; `null`
where the event has no original function. `caller` is the return address; ours is always
`null`. Booleans are 0/1.

| kind | addr, name | args |
|---|---|---|
| `sound` | `0x00427990 Sound_PlayFile` | `file`, `speech` |
| `sound` | `0x004263AD Music_Play` | `file`, `looped` |
| `sound` | `0x00426216 Sound_RestartSlot`, `0x00426120 Sound_PlaySlot` | `file` (entry slot-1 of the last `Sound_LoadBank` table; null with no bank), `slot` |
| `screen` | `0x004EAC50 g_screenId` | `id`; emitted when the value changes. Ours adds `ours` |
| `message` | `0x00472BC5 Msg_Enqueue` | `group`, `category`, `to`, `from`, `county` |
| `phase` | `0x00569584 g_turnPhase` | `phase`, the value written |
| `ai` | `0x0049A581 AI_RunTurnStep` | `realm` stepped, `step` (its `g_realms` +0x00 before the call), `done` (1 once it is 999); at the return |
| `order` | `null GameCommand` | `slot`, `cmd` (ours; see below) |
| `rand` | `null Pcg32` | `count`: draws in the frame, one record a frame |
| `draw` | `0x0040A21A Pl8_DrawFrame` | `sheet` (`.pl8` name), `frame`, `x`, `y` |
| `draw` | `TileBlit_{Plain,M3,M4}_Z{0,1,2}` (`0x00452820` …) | the map terrain under `Map_DrawTile` `0x004063C1`: `sheet` (the bank), `frame` `DAT_005C9288`, `x`, `y` `g_drawX/Y` |
| `draw` | `Pl8_DrawFrameHere` `0x0040A567`, `Pl8_DrawFrameClipped` `0x0040A3D0` | `sheet`, `frame`, `x`, `y` |
| `draw` | `Blit_Raster` `0x0040A01D` | `sheet` (`?` for the minimap raster), `x`, `y`, `w`, `h` |
| `draw` | `Blit_Unclipped` `0x004B43B1`, `Blit_ClippedLeft`/`Right`, `FUN_004B4537`/`4814`/`48B8`, `FUN_004B49F7`/`4CB8`/`5333` | `sheet`, `frame` `DAT_005C9288`, `x`, `y` `g_drawX/Y`: painters that blit directly, told apart by `caller` (`Map_DrawArmies` `0x00408438`, `Map_DrawPathMarker`, `Widget_Draw` `0x0040CFD2`, the battle figures `FUN_004BDC31`) |
| `draw`, `text` | every other hooked draw leaf (`Ui_Draw*`, `Ui_OkButton`, `FUN_0040328E`, …) | the arguments by `docs/symbols.json` parameter name, `a<n>` where it has none; fonts dropped |
| `text` | `0x00402D37 Eng_DrawString` | `group`, `index`, `x`, `y` |
| `text` | `0x00402637 Ui_DrawText` (ours) | `s`, `x`, `y`, `font` (`body`, `heading`, `eight`, or `5x7` for our debug fallback); `group`, `index` when `s` is an `L2.eng` string |
| `film` | `0x0042D91B Smk_Play` | `file` |
| `call` | `Army_*`, `MoveOrder_*`, `Battle_OrderSelection`, `BattleUnit_Order`, `Battle_AutoResolve`, `Battle_ReturnToCampaign`, the battle's `Order_*`, `UnitPanel_Draw` `0x0041B19D` (cat `panel`) | `args`, the raw list: what a scenario proves it did |
| `snap`, `anchor` | `null` | `hash` / `label` |

`order` and `rand` have no one-to-one original function: an order is a direct call in single
player (`Net_SendCommand` `0x0043EDA0` in multiplayer), and our generator is `Pcg32`, not
`Rand_Advance` (`0x00404A46`; `docs/netcode.md` D-3a). The differ compares their counts only.

A draw leaf whose caller lies inside another hooked draw primitive (`hooktable.json` `size`, from the
decompilation headers) is that primitive's and is dropped, except `Pl8_DrawFrame` and `Eng_DrawString`,
which are kept as before.
