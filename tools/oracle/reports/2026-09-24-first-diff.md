# First trace diff: the original against ours, four scenarios

2026-09-24. `tools/oracle/trace-diff.js` over `launch`, `newgame-original`, `newgame-new`,
`zoom-map`. Original side: `trace-run.ps1 -Runs 2` (hidden, own install `E:\games\l2-trace-b4`);
ours: `L2_TRACE_OUT=<dir> cargo test --release -p l2-game --test trace -- --ignored`. Frame
numbers below are each side's own and are not compared. C246.

Reproduce:

```
powershell -File tools/oracle/trace-run.ps1 -Scenario tools/oracle/scenarios/<s>.jsonl -Out C:\Temp\t -Install E:\games\l2-trace-b4
L2_TRACE_OUT=C:\Temp\t cargo test --release -p l2-game --test trace -- --ignored
node tools/oracle/trace-diff.js --dir C:\Temp\t --a orig --b ours launch newgame-original newgame-new zoom-map
```

## Self-check

| check | result |
|---|---|
| each scenario's two original runs byte-identical (`trace-run.ps1` sha) | 4 of 4 |
| `--a orig --b orig.run2`, all four scenarios | "No divergence" x4, exit 0 |
| planted: ours traces `S011_02.wav` as `S011_09.wav` (scratch build, reverted) | `sound` in step `(start)` at #1, the planted line marked `>`; unplanted run's first `sound` divergence is at #3 |

## Findings, most visible first

Evidence lines are `f<frame> <function> <args> from <caller>`; `>` marks the divergence.
`[V]` measured in both traces; `[I]` the cause, inferred.

**1. Tip narration stops after its first file.** `[V]` every new game. The original plays the
rest of each tip's speech from `Sound_PlayS201Seq` (`0x004B3ACD`, caller `0x004B3B8A`); no
emit site or code of ours names it. Missing: `S200_02/03`, `S201_02/03/04`, `S202_02/03/04`.
```
original:   f373 Sound_PlayFile {"file":"S200_01.wav"} from 0x004B366C
          > f381 Sound_PlayFile {"file":"S200_02.wav"} from 0x004B3B8A
            f381 Sound_PlayFile {"file":"S200_03.wav"} from 0x004B3B8A
ours:       f86  Sound_PlayFile {"file":"S200_01.wav"}
          > f447 Sound_PlayFile {"file":"S201_01.wav"}
```

**2. No end-of-turn fade screens.** `[V]` both new-game scenarios, both End Turns. The
original passes `g_screenId` 0x23, 0x24, 0x23, 0 in five frames (0x24 is the fade
`Season_FinishFade` `0x0049A3E6` guards); ours never leaves 0. `[I]` ours may fade without
reporting a screen byte; either the fade or the traced id is missing.
```
original: > f602 g_screenId {"id":35}  f604 {"id":36}  f605 {"id":35}  f607 {"id":0}
ours:       (no screen record between the End Turn click and the next anchor)
```

**3. The third tip is a modal box in ours, a panel over the live map in the original.** `[V]`
Tip 202 (category 9) opens with `g_screenId` still 0 in the original and stays up; ours
raises screen 0x27 for it. Knock-on in `zoom-map`: the original queues tip 206 behind the
open 202 (no voice) and plays `S202_02..04`; ours opens 206 at once with its voice.
```
original screens: 437:0 439:39 502:0   (202's voice at f502, its OK drawn at f563, screen 0)
ours screens:     417:0 437:39 479:0 499:39 541:0
zoom-map voice:   original none;  ours > f613 Msg_PlayVoice {"group":206}
```

**4. AI lords move at game creation in ours, on the map in the original.** `[V]` Ours runs
realm 2's twenty `AI_RunTurnStep` steps at f56-75, before the first tip, with
`merchant.wav` x3 and `army.wav`; the original runs none in the front end or under the
first two tips. Its first step is f566, on the map with tip 202 still up, in phase 4, with
its slot sounds, before the player's first order (phase 6). In `zoom-map`, where the player
clicks at f564, the original ran no AI step in the 60 frames after.
```
original: step (start) ai 0, slot 0;  step map > f566 AI_RunTurnStep {"realm":1} ... f577 Sound_PlaySlot {"slot":12}
ours:     step (start) > f56 AI_RunTurnStep {"realm":2,"step":0}  f56 Sound_PlaySlot {"file":"merchant.wav"} x3
```

**5. One AI realm in ours, two in the original.** `[V]` counts over `newgame-original`:
original realm 2 x17954, realm 3 x17952, realm 1 x3; ours realm 2 x60, no realm 3 (same in
`newgame-new`). `[I]` either our "Play Now!" world has one opponent, or realm 3 runs
without reaching the trace site.

**6. Turn phases run in a different order.** `[V]`
```
original: 0 1 2 | map: 4 6 | End Turn: 1 4 | 6 1 4
ours:       -   | map, after End Turn: 2 3 4 5 6 | 7 1 | 2 3 4 5 6 7 1
```
The original's turn one is 1 (front end), 2 (tips closed), 4 (AI), 6 (player); End Turn goes
to 1 then 4. Ours emits nothing until End Turn, then steps 2..7 two frames apart and rests
at 1. `[I]` ours has no initial record (`trace_changed!` fires on writes only).

**7. Tip posting delay.** `[V]` The original posts 201 one frame after 200 closes and 202 the
frame after 201 opens; ours posts each 20 frames after the previous one closes (f417 to
f437, f479 to f499). Not in the differ's output (frames are not compared); read off the
records.

**8. Front end: small.** `[V]` The original starts at `g_screenId` 0, and between films returns
to 0 or 0x1F for a frame (f1626, f1750); ours holds 0x22 through all three. The original's
`Music_Play("setup.wav", looped 0)` from `0x0040EA43` at boot, stopped at once by
`0x0040EA4D`, is absent in ours (inaudible).

## What the instruments cannot yet say

| category | gap |
|---|---|
| draw | The original side has **no** `Pl8_DrawFrame` records in any scenario (`pl8Draws` 0); no draw comparison was possible. |
| text | Ours emits no `text` for tip boxes (drawn through `tip::words`, not `Pen::eng`); the original's 202 heading is `202/0@32,84`. |
| slot sounds | The original records the slot number, ours the file; only counts compare (e.g. `newgame-new` end-turn step: 53 / 28). |
| ai | The original's step byte is not captured (`null`); its hook counts calls (22458 in one step) where ours counts steps (20). Realm sequences only. |
| rand | Not comparable in size: the original counts `Rand_Advance` + `Rng_LfsrStep` (9 a frame on the title page); ours `Pcg32` (101 at world creation, 3 per End Turn). |
| order | No original side (FORMAT.md). Ours: one `EndTurn` a click. |
| not built | No original address outside our emit sites in these four scenarios. |

Harness artifacts excluded: message 298 "Non 256 colour display" and its voice (posted by
`0x004B9A23` because the fake DirectDraw reports a non-8-bit mode); ours skips the click a
scenario comment marks "(this harness)".

Traces are 1.8 KB to 5.6 MB a scenario and are not committed.
