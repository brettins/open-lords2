# Trace scenarios and records

One scenario drives both games: `native/l2-trace` (the original) and
`crates/l2-game/tests/trace` (ours, `cargo test -p l2-game --test trace`). Each writes
`<scenario>.orig.jsonl` / `<scenario>.ours.jsonl`; a differ compares them. Decision CNEW-trace-sink.

## Scenario: `<name>.jsonl`, one step a line

Blank lines and lines starting `//` are skipped. Numbers may be decimal or a `"0x.."` string.

| op | fields | meaning |
|---|---|---|
| `load` | `save` | a save by fixture name; the campaign map over it |
| `new-game` | | the world `main.rs` builds behind the title page |
| `click` | `x`, `y`, `button`? (`"right"`) | pointer to (x, y), press, one frame, release |
| `key` | `key` | `Escape`, `Enter`, `Space`, `Backspace`, arrows, or one character |
| `wait-screen` | `id`, `max`? | frames until `g_screenId` (`0x004EAC50`) is `id` |
| `wait-phase` | `phase`, `max`? | frames until `g_turnPhase` (`0x00569584`) is `phase` |
| `wait-idle` | `frames`, `max`? | frames until `frames` in a row emit nothing but `draw`, `text`, `rand` |
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
| `sound` | `0x00426216 Sound_RestartSlot`, `0x00426120 Sound_PlaySlot` | `file` |
| `screen` | `0x004EAC50 g_screenId` | `id`; emitted when the value changes. Ours adds `ours` |
| `message` | `0x00472BC5 Msg_Enqueue` | `group`, `category`, `to`, `from`, `county` |
| `phase` | `0x00569584 g_turnPhase` | `phase`, the value written |
| `ai` | `0x0049A581 AI_RunTurnStep` | `realm`, `step`, `done` |
| `order` | `null GameCommand` | `slot`, `cmd` (ours; see below) |
| `rand` | `null Pcg32` | `count`: draws in the frame, one record a frame |
| `draw` | `0x0040A21A Pl8_DrawFrame` | `sheet` (`.pl8` name), `frame`, `x`, `y` |
| `text` | `0x00402D37 Eng_DrawString` | `group`, `index`, `x`, `y` |
| `film` | `0x0042D91B Smk_Play` | `file` |
| `snap`, `anchor` | `null` | `hash` / `label` |

`order` and `rand` have no one-to-one original function: an order is a direct call in single
player (`Net_SendCommand` `0x0043EDA0` in multiplayer), and our generator is `Pcg32`, not
`Rand_Advance` (`0x00404A46`; `docs/netcode.md` D-3a). The differ compares their counts only.
