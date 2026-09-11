# HANDOFF — the click sound, and the 55 `blocked` audio triggers

**The job, in one line.** Unblock `docs/audio.json`'s *shared hit-tester* group — the
pointer click, 4 sites — then pick the next blocked group by player-visible value.

Branched from `ff4a7c5` ("Update the starting position"), fast-forwarded to `main` at
`5338fe7`. **Nothing here has been compiled or tested.** `cargo` was never run on this
branch. Treat every code change below as unverified.

---

## 1. What was established — the part that dies with me

### 1a. **Two of the four "click" sites are dead code in the shipped game.** `[V]`

This is the most valuable thing on this page and it cost a scan of the player's own
executable to get.

`docs/audio-triggers.md` ends with *"A correction this produced: the pointer click is not
one call site behind one hit-tester. It is four, behind three."* **That correction is
itself half wrong.** It is **two, behind one**.

`FUN_0040D6AD` and `FUN_0040D7B8` are the decrement and increment **arrows of a slider
widget**. Their only caller is `FUN_0040D3F5` (the slider hit-tester), whose sibling
`FUN_0040D271` is the slider's painter — it draws `g_systemSheet` frames `0x4A` (left
arrow), `0x4B` (right arrow), `0x4C` (the thumb), and a trough between them.

**`FUN_0040D3F5` and `FUN_0040D271` are called by nothing in `Lords2.exe`.** Verified
three ways against `F:\games\Lords of the Realm II\Lords2.exe` (1,031,680 bytes, image
base `0x400000`, `.text` VA `0x1000` raw `1024` size `846848`):

| scan | `FUN_0040D3F5` | `FUN_0040D271` | control: `Widget_Test` `0x0040DA1E` |
|---|---:|---:|---:|
| `E8` rel32 CALL over all of `.text` | **0** | **0** | **36** |
| `E9`/`EB`/`0F 8x` jumps over all of `.text` | **0** | **0** | — |
| little-endian absolute dword, whole image (catches any pointer table; `.reloc` would have to fix one up) | **0** | **0** | 0 |

The control matters: `Widget_Test` shows 0 absolute references and 36 relative calls, which
is what proves the absolute scan's zero is not simply how this binary calls things.

So the three inner functions each have exactly one caller and it is `FUN_0040D3F5`
(call sites at `0x0040D4A6` → `FUN_0040D6AD`, `0x0040D50D` → `FUN_0040D5A0`,
`0x0040D566` → `FUN_0040D7B8`; all three inside `FUN_0040D3F5`, which spans
`0x0040D3F5`…`0x0040D591`). Nothing reaches `FUN_0040D3F5`. It is an unused toolkit
slider.

**Consequence:** `FUN_0040d6ad#1` and `FUN_0040d7b8#1` in `docs/audio.json` should become
`status: "dead"` with this as the `note`. `crates/l2-game/tests/sfx.rs`'s
`no_trigger_is_dead_in_the_shipped_game` asserts the dead list is **empty** and its doc
comment says the first entry must cost a decision — follow its own instructions: add the
two to `docs/bugs.md`'s dead-code list and change the assertion to *name these two*, so a
third still costs a decision. **I had not done this yet.**

> A nice detail worth keeping even though the widget is dead: within `FUN_0040D3F5` the
> **two arrows click and the thumb drag does not** — `FUN_0040D5A0` is gated on
> `g_mouseLeftDown` and has no `Sound_RestartSlot`. That is the same rule `docs/input.md`
> §5 already states for drags.

### 1b. **The click's exact rule, from `Widget_Test` (`0x0040DA1E`).** `[V]`

Read at `tools/oracle/decomp/00400000.c:7471`–`7577` (the corpus lives in the **main
checkout**, `E:\dev\lords2\tools\oracle\decomp`, not in the worktree — it is gitignored).

`Sound_RestartSlot(1)` — `click3.wav`, slot 1 of **both** banks, so it is bank-independent
— appears at exactly two places, and both are guarded by
`g_mouseLeftPressed || g_mouseLeftDoubleClick`:

* **`Widget_Test#1`** — the **kind-4** arm (auto-repeat buttons: every `+`/`−`, `<`/`>`,
  up/down arrow). Sound, then `rec[0x0D]=3`, publish the hotspot, call the handler.
* **`Widget_Test#2`** — the **kind-5** arm (delayed press: gauntlets, checkboxes,
  diplomacy's verbs). Sound, `rec[0x0D]=0x14`, `rec[0x0C]=1`, **return without calling the
  handler** — the handler runs twenty frames later out of the countdown loop.

**What is silent, and this is the audible half of the finding:**

* the **kind-4 auto-repeat pulses**. The hold branch (`g_mouseLeftDown`, counter `+0x0E`,
  ramp `DAT_004D2748`) calls the handler over and over with **no** `Sound_RestartSlot`.
  Only the initial press clicks.
* **kind 2** (`Widget_Test`'s toggle arm, `+0x0F == 2`): flips the state bit, calls the
  handler, **no sound**.
* kinds 1 and 3, and all of `Hotspot_Test` (`0x0040E3EE`) — the majority of the interface.
* `Ui_OkButtonClicked` (`0x0040E7E4`), the 24×24 corner exit, used 26 times.

`docs/input.md` §5 already states this rule correctly ("`Widget_Test` only, only for kinds
4 and 5, and only on the initial press") — **except** that it says *only* `Widget_Test`,
which contradicts `docs/audio-triggers.md`'s four-sites correction. Given 1a, **`input.md`
is the one that is right about the shipped game** and `audio-triggers.md`'s closing section
needs the correction above. Neither document currently says the dead half.

### 1c. **The blocked 55, grouped, with counts *and* file weight**

Counts are from `docs/audio.json`; weights are distinct install `.wav` files each group can
reach, counted from the install (771 files) and from the 16-byte name tables in the exe.
`CLAUDE.md` insists on the second column; here it is.

| mechanic | sites | files | notes on the weight |
|---|---:|---:|---|
| the battlefield's per-man state machine | 25 | **≤17** `[I]` | 15 distinct battle-bank slot constants plus `dest_ind.wav` and `bathit2.wav`. **The 66 troop-cry files are *not* here** — the six `Sound_PlayTroopCry` sites are filed `missing`, not `blocked` |
| Smacker playback (6 music restarts + 2 animated voice) | 8 | **≈0 new** | every one restarts a bed we already reach (`setup.wav`, the scroll ladder); the two `Msg_PlayVoice` ones index the same narrator pool `voice_tick` already plays |
| the sibling voice tables' callers | 6 | **≈32** `[I]` | `g_msgVoiceS010` (13 ship), `S020` (5), `S035` (8), `S071` (6), plus two indirect tables. Bound is the caller's index range, which is exactly what is unread |
| a channel from a click to the audio layer (the field brush) | 5 | **3** `[V]` | kingdom-bank slots 4, 6, 7 → `rioters.wav`, `wheat.wav`, `stonecut.wav` |
| **a shared hit-tester (the pointer click)** | **4** | **1** `[V]` | `click3.wav`. **2 of the 4 are dead — see 1a** |
| the battle verdict | 2 | **1** `[V]` | `ff_lose.wav`, named by both arms (`ff_win.wav` ships and nothing plays it) |
| the two delegated message painters (`0x0C`, `0x14`) | 2 | **0 new** | both are `ff_msg.wav`, already fired |
| a per-message take cursor (the chained takes) | 1 | **27** `[V]` | the table at `0x004E1E40` walks `S201_02` + n·0x10 through `S200_03` — 27 real names before three `S000_00.wav` sentinels: S201/202/207 ×3, S209 ×4, S210 ×3, S212 ×1, S214 ×2, S217 ×4, S218 ×2, S200 ×2. All 27 ship and **none is reachable today** |
| the mercenary offer | 1 | **12** `[V]` | `S016_01` + n·0x10; the table holds 16 slots, 12 of them ship |
| the letter's lord sting | 1 | **4** `[V]` | `S246_02`(`0x004E2470`), `_04`, `_03`, `_01`, then an `S000_00.wav` sentinel. **The note in `docs/audio.json` says "five lords" and the table holds four.** Small correction, unmade |

Blocked total ≈ **97 files**, against 572 of 771 already reachable.

### 1d. **Which group I had decided to take next, and why**

**The per-message take cursor — one site, 27 files, and it is the largest
files-per-site ratio in the whole inventory.**

The reasoning, which is the point of the second column: by a 1:1 count it is the *smallest*
group on the page and by weight it is the second largest. It is the difference between the
narrator reading you one sentence and the narrator reading you the notice. A player already
has `S010_13.wav` on disk with no way to hear it. `docs/audio-triggers.md` says the two
missing pieces are (i) *"is a one-shot still playing?"*, which `Mixer::is_playing` already
answers, and (ii) a per-message take cursor, which nothing keeps — and `Director` is
already the thing that keeps per-tick memory (`stack`, `zoom_far`, `tiles`), so a cursor is
one more field of the same kind, on the same object, outside `Game` and outside the digest.
The 1,000 ms gap is the only part that needs care, because `Director` counts ticks and must
not read a clock.

Second choice would be **the field brush (5 sites, 3 files)** — its note already proposes
the wiring ("Director remembers the tile the information panel is about and notices its
terrain change") and the mechanism I built for the click makes the cheaper version
available too.

I would **not** take the sibling voice tables next despite their 32 files: every one is
blocked on *finding a caller*, which is reading work with an unknown floor.

---

## 2. What I was doing at this exact moment

Implementing the click. The design, which I still believe is right:

**`crates/l2-game/src/press.rs` *is* `Widget_Test`** — same function, same position — so the
sound is counted there and nowhere else. It cannot call `Audio` (`docs/netcode.md` D-3
keeps `Audio` out of `Ctx` on purpose), so it keeps a **write-only outbox** that is drained
upward:

1. `Press` gains `clicks: u8` and a private `fn click()`, called from `Press::press`
   (kind 4 → marker `// sfx: Widget_Test#1`) and `Press::press_delayed` (kind 5 → marker
   `// sfx: Widget_Test#2`), and **not** from `press_held` (kind 2) or `tick` (the repeat
   pulses and the delayed fire). That is literally where the two `Sound_RestartSlot(1)`
   calls sit. `pub fn take_clicks(&mut self) -> u8` drains it.
2. `Screen` gains a defaulted `fn take_clicks(&mut self) -> u8 { 0 }`, the shape of
   `take_redraw`. Nine screens own a `Press` and override it: `BattlePromptScreen`,
   `BattlefieldScreen`, `CountyScreen`, `DiplomacyScreen`, `ComposeScreen`, `DivideScreen`,
   `InfoScreen`, `MessageScreen`, `SuppliesScreen`.
3. `Machine` gains a **monotone** `clicks: u32`, drained in `Machine::handle`
   **immediately after the screen's `handle` and before `apply_at`** — because the
   transition can pop the screen that clicked, and a sum over the live stack would lose
   exactly the presses that open a screen.
4. `Director` keeps the previous tick's `machine.clicks()` and plays `click3.wav` when it
   has risen — the same diffing it already does for `stack`, `zoom_far` and `tiles`.

**Steps 1–3 are written. Step 4 is not.** Nothing has been compiled.

### The single next step

Add to `Director` (in `crates/l2-game/src/audio/mod.rs`):

```rust
/// The click, from `Widget_Test`'s two `Sound_RestartSlot(1)` sites.
clicks: u32,
```

and, inside `Director::listen`, before `self.stack = now;`:

```rust
// `Widget_Test` (`0x0040DA1E`) plays `click3.wav` from inside the hit test,
// at the kind-4 and kind-5 arms, on the initial press only.
// `crates/l2-game/src/press.rs` is that hit test and `Machine::clicks` is the
// wire out of it. Slot 1 is `click3.wav` in BOTH banks, so it does not matter
// which is loaded.
if machine.clicks() != self.clicks {
    self.clicks = machine.clicks();
    if let Some(n) = names::slot(names::Bank::Kingdom, 1) {
        audio.play_effect(n);          // RestartSlot semantics: restart, not drop
    }
}
```

Note the marker ids must **not** be repeated here — `sfx.rs` forbids an id claimed twice,
and both are already claimed in `press.rs`.

Then: `docs/audio.json` — `Widget_Test#1`/`#2` → `reproduced`, `ours:
"crates/l2-game/src/press.rs"`, `sound: "click3.wav"`; `FUN_0040d6ad#1`/`FUN_0040d7b8#1` →
`dead` with §1a as the note. Then `sfx.rs`'s dead assertion, `docs/bugs.md`,
`docs/audio-triggers.md`'s closing correction, `docs/input.md` §5, and the `CNEW` entry.

### The test and the ablation (neither written)

* Test: drive `Machine::handle` with a `Event::Click` on a county tax arrow through a
  headless `Audio` + `Director`, assert `click3.wav` lands in `audio.heard()`; then hold
  the arrow for a second of ticks and assert the count of clicks is **one**, not one per
  repeat pulse. That second half is the whole finding and is the assertion worth having.
* **Stated ablation:** delete the `self.click()` call in `Press::press_delayed` and the
  gauntlet test must go red while the tax-arrow test stays green — the two arms are
  separately observable, which is what makes the ablation meaningful rather than a
  tautology. Beware the trap the audio branch already hit: do **not** ablate by comparing
  two `mix` buffers, because the campaign bed advances thousands of frames between calls
  and the buffers differ whatever the effect did.

---

## 3. What I believe but have not checked

* That `Press::press` is never called for anything but an initial kind-4 press. I grepped
  the screens and found no direct caller outside `Press::event`, but `press_delayed` **is**
  called directly once — `crates/l2-game/src/screens/diplomacy.rs:445`, the six verb
  buttons, hand-rolled around a menu-row hit test. Putting the count in `press`/
  `press_delayed` rather than in `Press::event` is deliberate for exactly that reason and I
  think it is right; it has not been exercised.
* That one click per *tick* rather than per *event* is inaudible. `Sound_RestartSlot`
  restarts the same buffer, so two presses inside one 16 ms tick would be one sound in the
  original too — but I have not reasoned it through for the auto-repeat edge.
* That nothing else in the tree constructs a `Machine` in a way my field addition breaks.
  `Machine::new` is the only constructor I saw.
* That `docs/audio.json`'s "five lords" for `FUN_004b3b92#1` is simply wrong and the answer
  is four. The table says four. I did not find the caller's bound.

## 4. Dead ends

Nothing expensive. One thing worth not repeating: **the absolute-address scan of the
executable proves nothing on its own** — `Widget_Test`, which has 36 callers, also scores
zero, because this binary calls with `E8` rel32. Always run the control.
