# Every place the original asks for a sound

**143 trigger sites across 70 functions. We fire 51 of them, and they carry 560
of the install's 771 sounds**, because the classes are wildly unequal in weight.
**Three are dead in the shipped game**, and they are named below.

    node tools/oracle/sounds.js            # the table, by primitive
    node tools/oracle/sounds.js --count    # the totals
    node tools/oracle/sounds.js --check    # docs/audio.json against the decompilation
    node tools/oracle/sounds.js --rebuild  # regenerate the rows, keeping the verdicts
    cargo test -p l2-game --test sfx       # docs/audio.json against the markers in crates/

**This file is the prose. `docs/audio.json` is the count**, one record per site,
and it is the thing to read and to edit. The numbers below come out of it.

## What changed, and why it is the more important half

This document used to be **hand-marked prose with no check behind it**. It said
*"134 trigger sites … we reproduce 24"*, and both halves were wrong: the
denominator was one primitive short, and the numerator had never been compared
with the code. `docs/arms.json` has had a test since C61 for exactly this reason
and this file did not.

It has one now, and it is the same shape as the arms audit's:

| artefact | compares | runs where |
|---|---|---|
| `sounds.js --check` | `docs/audio.json` against the **decompilation** | wherever the corpus is |
| `crates/l2-game/tests/sfx.rs` | `docs/audio.json` against the **`// sfx:` markers** | everywhere |
| a person | the marker against what the code does | nowhere mechanical |

The marker is `// sfx: <id>[,<id>…]` beside the call, and the id is
`<caller>#<n>` — the function the site is in, and its ordinal within that
function in source order. **A marker may claim several ids** and `arms.rs`'s may
not, which is not a relaxation: `Msg_PlayVoice` is asked for at sixteen places
guarded by sixteen values of one countdown, and `audio::voice_tick` is that
whole ladder in one function. Fourteen markers on one line would be fourteen
claims about one line. What the check forbids is an id claimed twice.

## The **ninth** primitive, which is the finding

The old enumeration opened with *"the original funnels every sound in the game
through **eight** leaf functions"*, and that sentence is what made the audit
cheap. It is also what made it wrong.

**`Music_Play` (`0x004263AD`) is a ninth**, with **nine call sites of its own**,
and every one of them is the front end:

| site | file | what |
|---|---|---|
| `App_WinMain#1` | `setup.wav` | the process opening |
| `FUN_00497a34#2` | `setup.wav`, looped | every return to the title |
| `Screen_DrawConquest#1` / `#2` | `setup.wav` / `setup2.wav` | the campaign interstitial, by map number |
| `Smk_OnFinished#1` | `setup.wav` | the intro films ending |
| `FUN_00497a34#1`, `FUN_00433155#1`, `Screen_FrameInput#1` / `#2` | `SETUP3.WAV` | the credits and the ending |

`Music_StartCampaign` and `Music_StartBattle` are *themselves* ladders that end
in `Music_Play`, which is why they read as leaves and are not.

**The cost of the omission was a player's report.** He said *"I don't hear
music"*; `docs/decisions.md` C116 found the campaign half of that and fixed it;
the title screen stayed silent, and `audio::scene` said so in a comment that was
true and complete and still lost the track:

> *"The original plays no music here: `Music_StartCampaign` is reached from the
> campaign coming up, and the title screen's only sound is `setup.wav`."*

Every clause is correct. `setup.wav` **is** the music. And because no row of the
inventory covered `Music_Play`, nothing anywhere said a sound was missing — the
count did not understate the gap, it could not see it.

> **An enumeration's denominator is a claim, and *"every X goes through these
> N"* is the most confident shape a claim can take.** The eight were found by
> grepping for the functions that *play* a sound; `Music_Play` was missed because
> it is called by two of the eight, so it looks like an implementation detail
> from every direction except the nine sites that call it directly.

The same mistake had already been made once inside this file — the first pass
counted 121 and missed `FUN_004262cf`, a 28-byte forwarder with thirteen callers
— and the lesson was written down as *an absence is evidence only in proportion
to how hard it was looked for*. It was written down and it did not prevent the
second instance. `sounds.js --check` is the version a machine enforces.

## The nine primitives

| primitive | addr | class | sites | ours | what it is |
|---|---|---|---:|---:|---|
| `Sound_PlaySlot` | `0x00426120` | bank | 14 | 4 | **drops** the request if that buffer is still playing |
| `FUN_004262cf` | `0x004262CF` | bank | 13 | 0 | a 28-byte thunk onto `Sound_PlaySlot` |
| `Sound_RestartSlot` | `0x00426216` | bank | 22 | 8 | rewinds and plays regardless — and the pointer click is two of these |
| `Sound_PlayFile` | `0x00427990` | file | 49 | 15 | one-shot by name; arg 2 picks the speech or effects flag |
| `Msg_PlayVoice` | `0x004B35C1` | voice | 16 | 14 | an `L2.eng` group to a filename, then `Sound_PlayFile` |
| `Sound_PlayTroopCry` | `0x00499CB1` | cry | 6 | 0 | 11 × 4 × 4, then `Sound_PlayFile` |
| `Music_StartCampaign` | `0x00499ACA` | music | 10 | 5 | the progress-bar ladder |
| `Music_StartBattle` | `0x00477B2F` | music | 4 | 4 | the alternating pair |
| `Music_Play` | `0x004263AD` | music | 9 | 2 | a named track, looped or not — the front end is all of these |

Calls that live *inside* one of those nine are the primitive's implementation
and are not counted. `Sound_StopOneShot` (16 sites) and the two bank preloads
(8) are excluded for the same reason: neither starts a sound.

## The three verdicts, and the count of each

`docs/audio.json` gives every site one of four statuses, and three of them are
the verdicts a reader wants:

| status | sites | means |
|---|---:|---|
| `reproduced` | **51** | we fire it; a `// sfx:` marker is on the line |
| `blocked` | **52** | the mechanic behind it is not built, and `note` **names** it |
| `missing` | **37** | reachable and unwired — no excuse, just not done |
| `dead` | **3** | the shipped game cannot reach it; `note` is the evidence and `docs/bugs.md` has the entry |

**Three are dead, and this section used to say none were.** `tests/sfx.rs` asserted
the `dead` list empty *so that the first entry costs a decision*, and these are that
decision — it now pins the three by name, so the fourth costs the same:

| id | why it cannot run | `docs/bugs.md` |
|---|---|---|
| `FUN_0040d6ad#1`, `FUN_0040d7b8#1` | the arrows of a slider widget whose hit-tester `FUN_0040D3F5` has **no caller** — zero rel32 calls, zero jumps, zero absolute references in the image, against 36 rel32 callers for `Widget_Test` as the control | `DNEW-slider-widget` |
| `Battle_PauseButton#1` | guarded `== 1` on a word that alternates between 0 and −1; the one writer that could make it 1, `FUN_00434E68`, is unreferenced too | D38 |

**The third was already on the bug list, and this file had it as `missing`.** The
assertion of emptiness is what kept it there: the inventory could not say what the
bug list already knew.

The unreachable audio on the *other* side of the question is unchanged: `Ff_win.wav`
ships and **no call site names it** — a file with no trigger rather than a trigger
with no path.

### What `blocked` is blocked on, in size order

| mechanic | sites | files it would add |
|---|---:|---:|
| the battlefield's per-man state machine | 25 | ≤17 — 15 battle-bank slots, `dest_ind`, `bathit2` `[I]` |
| Smacker playback | 8 | ≈0 — every one restarts a bed or a voice already reachable |
| the sibling voice tables' callers | 8 | ≈48 — `S010` 13, `S020` 5, `S035` 8, `S071` 6, the mercenary's `S016` 12, the lord sting's `S246` 4 `[I]` on the callers' index ranges |
| a channel from a click to the audio layer (the field brush) | 5 | 3 |
| the battle verdict (`ff_lose.wav`) | 2 | 1 |
| the two delegated message painters (categories `0x0C`, `0x14`) | 2 | 0 — both `ff_msg.wav` |
| **the tip screens** — `Tip_Update`, `Tip_Show` and the categories `0x05`…`0x09` window | 2 | **40** `[V]` — 27 chained takes and 13 first lines; the largest files-per-site ratio in the inventory |

**Read the second column before the first.** By sites the tip screens are the
second-smallest row; by what a player would hear they are the second largest. The
**66 troop-cry files are not here at all**: the six `Sound_PlayTroopCry` sites are
`missing`, not `blocked`.

**The battlefield's 25 is a limit of the design and not a to-do.**
`docs/netcode.md` D-3 says a sound may never affect the simulation, which is
enforced by shape: `Director::listen` takes `&Game` and derives what should be
audible from the world *after* the tick. A sword swing is an **event inside** a
tick — the state afterwards says where a man is, not that he struck. Everything
else in this table is work; this one needs a decision about the seam.

## A player's three reports, and where each landed

1. **"I don't hear the clip-clop of the merchants on end turn."**
   `Unit_MoveInFacing` (`0x00466D84`), four sites, **fired** —
   `Director::hear_the_march`, one hoofbeat per tile per unit. It was wired on
   the audio branch and it works.

2. **"I don't hear music or the click sounds or industry sounds when you right
   click them on the map."** Three things, three answers.
   * *Music* — fixed for the campaign (C116) and **now** for the front end, which
     is the ninth-primitive finding above.
   * *The click* — **fired**, and it was two sites rather than four: see the end
     of this file. `crates/l2-game/src/press.rs` is `Widget_Test`, and the rule a
     player can hear is that a spinner clicks on the press and **not** on its
     auto-repeat.
   * *The industry sounds on a right click* — **fired**. It is `TileInfo_Draw`
     (`0x0041C208`), the tile half of the information panel, which plays the
     site's work as it paints: slot 10 `iron.wav` for a mine, 8 `stonecut.wav`
     for a quarry **and for a smithy**, 9 `woodcut.wav` for a lumber mill. The
     same four graphic ranges `Industry_ToggleFromMap` uses.

3. **"Also sorely missing: 'All your people are fed by dairy.'"** — and it
   exists. See below.

## "All your people are fed by dairy" is a **voice line**, not a string

`docs/decisions.md` C133 searched every one of `L2.eng`'s 317 groups for that
sentence, found nothing, and concluded — correctly, about strings — that the
readout did not exist. It exists. `Panel_OpenRation` (`0x0043A846`) is four
statements and two of them are sounds:

```c
g_screenId = 0x19; Panel_Ration();
if (rationAchieved == 0)                                  Sound_PlayFile("S021_02.wav", 1, 0);
else if (herd && herdEaten == 0 && grainEaten == 0)       Sound_PlayFile("S021_01.wav", 1, 0);
```

`[V]`. The second condition **is** *"all your people are fed by dairy"*: the
county has a standing herd, and opening the larder took neither a cow nor a
sack. The game says it by **speaking**, on the frame the panel opens.

Both are fired now. `[I]` that the words are the ones he remembered, and that is
an oracle request rather than a claim — `docs/oracle-requests.md` §11: somebody
with the game needs to open a dairy-fed county's ration panel and listen.

> **C133's search was for the right condition in the wrong medium**, and the
> reason it looked exhaustive is that `L2.eng` really is where this game keeps
> its words. It keeps some of them in `.wav` files instead, and a group with one
> consumer is a screen's vocabulary only for the half of the vocabulary that is
> written down.

## The voice: the draw is the behaviour, so the trigger is a countdown

`Msg_DrawWindow` (`0x0047309E`) is 10,915 bytes and it is not a painter: it
dismisses, enqueues, sets its own timer and plays its own sound from inside the
draw. There is no call site to put a voice beside. All sixteen `Msg_PlayVoice`
calls are guarded by `g_messageTimer == <constant>`, and the timer counts
**down** from 2000, one per tick. `[V]`.

| category | ticks after opening | constant |
|---|---:|---|
| `0x02`, `0x03`, `0x05`…`0x09`, `0x0F`, `0x10`, `0x11`, `0x12` | 10 | `0x7C6` |
| `0x00` notice, `0x0D` capture (unanimated) | 90 | `0x776` |
| `0x0E` ending (unanimated) | 100 | `0x76C` |
| `0x01` letter, `0x0A` pay prompt, `0x0B` alliance prompt | 200 | `0x708` |
| `0x04` tip | 10 | `0x5A`, against a timer clamped to 100 |
| `0x13` help | — | silent |

**The five constants are one rule and a delay.** `0x7C6` is 1990 against a start
of 2000 and `0x5A` is 90 against the tip's clamped 100: *both are ten ticks after
the window opened*. The three larger delays belong to the categories that play a
**fanfare** on the opening frame — the voice waits for the trumpet instead of
talking over it.

**Thirteen of the sixteen are fired.** Two that are not — `Msg_DrawWindow#16`
and `#21` — are the **animated** capture and ending branches, which save the
group and variant, dismiss the message, play `cap_cty*.smk` and speak
*afterwards*. The trigger there is a film ending.

**The third is `#24`, and it was counted as fired until it was read.** It is the
branch for categories `0x05`…`0x09`, and the only function in the original that
posts one is `Tip_Show` (`0x00476DA9`), from `g_tipCategory` — those categories
*are* the tip screens. Nothing in our engine posts a tip, so the arm could not
sound, and the thirteen tip clips it would play were in the file count because
their names resolved.

### What is still missing inside the voice class

* **The chained takes — and they are the tip screens, not a cursor.** `[V]`:
  `FUN_004B3ACD(group)` has one caller, `Msg_DrawWindow`'s categories
  `0x05`…`0x09` branch, `else if (g_messageTimer < 0x780)` — so from 80 ticks in,
  every frame. It reads `n = table[group × 5 + cursor]` at `0x004E1E40`; if
  `Sound_OneShotBusy()` it stamps the time, otherwise once a full **1,000 ms** has
  passed since the last busy stamp it advances the cursor and plays
  `S201_02.wav + (n − 1) × 0x10`. The cursor `DAT_0052F004` is reset in exactly
  one place, **`Tip_Show`**, and the table's only live rows are groups 200…218:
  200 → 26, 27 · 201 → 1, 2, 3 · 202 → 4, 5, 6 · 207 → 7, 8, 9 · 209 → 10…13 ·
  210 → 14, 15, 16 · 212 → 17 · 214 → 18, 19 · 217 → 20…23 · 218 → 24, 25 — 27
  takes, all shipping. (Lower group numbers index the name pool, whose ASCII bytes
  the `n < 0x1F` guard rejects.) **This note used to blame the chain for
  `S010_13.wav`**, which belongs to `g_msgVoiceS010` below. The cursor and the
  gate are cheap; the blocker is that no tip window is ever posted —
  `Tip_Update` (`0x00476AA7`), `Tip_Show`, the paragraph window with its wrapped
  height and computed OK corner, and the 20-frame delay `FUN_00476E21` re-arms on
  every dismissal.
* **`FUN_004B3B92(lord − 1)`** — the sting a letter plays 90 ticks in,
  `S246_02.wav + lord × 0x10`. The function accepts indices 0…4 and the table
  holds **four** clips — `S246_02`, `_04`, `_03`, `_01` — and then `S000_00.wav`, a
  sentinel: four lords, not five. `[V]` from the bytes at `0x004E2470`.
* **Six sibling tables** — `g_msgVoiceS010`, `S016` (the mercenary offer, from
  `Sidebar_Button`), `S020`, `S035`, `S071` and two indirect ones. Each is one
  `Sound_PlayFile` behind one table index; the work is finding the caller.
* **Categories `0x0C` and `0x14`**, delegated to `Msg_DrawDiplomacy`
  (`0x00475E07`) and `Msg_DrawBeyondLetter` (`0x00476488`). **Unread**, and
  recorded as unread rather than absent, because inventing a tick for them would
  be worse than silence.

## The class nobody had wired: the sound is in the function that sets `g_screenId`

Fourteen sites, and the reason one mechanism reaches all of them is structural
rather than convenient. **The original's screen sounds are statements in the
handler that changes the screen.** `Panel_OpenRation` is the clearest case —
four statements, two of them sounds — and `Sidebar_Button`'s supplies arm,
`Panel_SplitButton`, `Map_ZoomOut`, `Panel_JobDetail`, `TileInfo_Draw` and the
four routes onto setup page 4 are all that shape.

So `Director` keeps the previous tick's screen stack and a screen **arriving** is
the trigger. That is not an approximation of the original's call site; it is the
same occasion, with the one difference that neither can fire twice for a screen
you are already on.

| site | on | plays |
|---|---|---|
| `Panel_OpenRation#1` / `#2` | the ration panel | `S021_02` / `S021_01` |
| `Sidebar_Button#1` | the supplies screen | `S033_01` |
| `Panel_SplitButton#1` | the division screen | `S017_01` |
| `Map_ZoomOut#1` | the far zoom | `S033_02` |
| `FUN_00432cc8#1` / `#2`, `Setup_ChooseCampaign#1` | setup page 4 | `S011_02` |
| `Panel_JobDetail#1`…`#3` | the job popup | `fire.wav` + `g_jobSound[job]` |
| `TileInfo_Draw#1`…`#4` | the information panel on a site | `iron` / `stonecut` / `woodcut` |

**`g_jobSound` (`0x004D2950`) is `0 7 4 6 0 10 8 9 0 0`** — read out of the
executable, twelve `i32`. Six of the nine jobs have a sound and the zeroes are
what says jobs 4 and 9 do not. `[V]`

**The blacksmith plays the quarry's sound at both of its sites** —
`Panel_JobDetail`'s job 8 and `TileInfo_Draw`'s graphic range 7…9 both take slot
8, `stonecut.wav`. `[V]` twice; `[I]` that it is because the kingdom bank has no
forge in it.

## The clip-clop is the bank class, and it has a rate rather than a trigger

`Unit_MoveInFacing` (`0x00466D84`), whose **first statement after unlinking the
unit from its tile** is a four-arm ladder on the unit's kind:

```c
Unit_UnlinkFromTile(g_movingUnit);
if      (g_units[g_movingUnit].kind == 3) Sound_PlaySlot(0xb);   /* merchant.wav */
else if (g_units[g_movingUnit].kind == 4) Sound_PlaySlot(0xb);   /* merchant.wav */
else if (g_units[g_movingUnit].kind == 2) Sound_PlaySlot(5);     /* rioters.wav  */
else if (g_units[g_movingUnit].kind == 1) Sound_PlaySlot(0xc);   /* army.wav     */
```

`[V]`, fired by `Director::hear_the_march`. **It fires on every step of every
moving unit**, with no guard on owner or visibility — a *rate*, not a trigger —
and `Sound_PlaySlot` being the drop-if-busy verb is the entire reason a per-step
call does not become a roar.

## A correction this produced, and a correction of it

**The pointer click is two live call sites behind one hit-tester.** This section
used to say *four, behind three*, and that was itself a correction — of *"one call
site, because the whole game shares one hit-tester"*. The first sentence was closer
to the truth than its correction.

`Widget_Test` (`0x0040DA1E`) plays `Sound_RestartSlot(1)` at two sites, and both
are guarded by `g_mouseLeftPressed || g_mouseLeftDoubleClick`:

* **`Widget_Test#1`**, the kind-4 arm — every spinner. The press clicks; the
  auto-repeat's hold branch calls the handler over and over **with no sound**.
* **`Widget_Test#2`**, the kind-5 arm — every gauntlet and checkbox. The press
  clicks; the handler runs twenty frames later out of the countdown loop, silently.

`FUN_0040D6AD` and `FUN_0040D7B8` do open with the same two lines, and they are the
arrows of a slider widget that **nothing in the executable instantiates** —
`docs/bugs.md` `DNEW-slider-widget`. The scan that proves it had to be run against
a control, because this binary calls with `E8` rel32 and an absolute-address search
finds nothing for *any* function, `Widget_Test` and its 36 callers included.

**Ours.** `crates/l2-game/src/press.rs` is the hit test, so the count is taken
there — in `Press::press` and `Press::press_delayed`, and deliberately not in
`Press::tick` or `Press::press_held`. A screen cannot reach `Audio`
(`docs/netcode.md` D-3), so the count is an outbox: `Screen::take_clicks` drains it,
`Machine::handle` accumulates it **before** applying the transition — a press that
closes its own screen is still heard — and `Director::hear_the_click` plays
`click3.wav` when `Machine::clicks` has moved. Slot 1 is `click3.wav` in both
banks, so the battlefield's confirm box clicks like the county's tax arrows.

`tests/click.rs` is mostly the silent cases, because a test that only checks that a
click sounds cannot see a spinner that clicks thirty-three times a second. **One of
its ablations stayed green**, and it is worth the paragraph: adding a click to
`Press::tick` went unnoticed, because `Machine::update` did not drain the outbox and
the stray click waited in the `Press` for the next event. It now drains on both
paths, and the held-arrow tests let go at the end so a click that is held back
cannot hide.

Writing it found a defect that was not audio's: **the county panel dropped a double
click on the floor**, where `Ration_SliderClick` and `Widget_Test` both take one as
a press. Six more screens own a widget table and still do; `docs/input.md` §5
counts them.
