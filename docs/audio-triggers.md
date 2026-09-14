# Every place the original asks for a sound

**143 trigger sites across 70 functions. We fire 89 of them, and they carry 703
of the install's 771 sounds**, because the classes are wildly unequal in weight.
(Two of the sites are the tip screens' first line and chained takes: forty files
by name, thirty-five that anything can ask for — see *What `blocked` is blocked
on*. Twenty-two are the battlefield's, from its event stream.)
**Four are dead in the shipped game**, and they are named below.

**These two numbers rot and the ones under them do not.** `docs/audio.json` is
checked in both directions and this sentence is not, so it has twice been five
sites behind the file it summarises; the count in the status table below comes
out of `node tools/oracle/sounds.js --count` plus the file itself, and if this
line and that table disagree, the table is right.

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
`Msg_PlayVoice` is asked for at sixteen places
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
| `Smk_OnFinished#1` | `setup.wav` | a film ending over setup page 1 — the *Lords of Magic?* trailer; the start-up chain's end is `FUN_00497A34` |
| `FUN_00497a34#1`, `FUN_00433155#1`, `Screen_FrameInput#1` / `#2` | `SETUP3.WAV` | the credits and the ending |

`Music_StartCampaign` and `Music_StartBattle` are *themselves* ladders that end
in `Music_Play`, so they read as leaves.

**The cost of the omission was a player's report.** He said *"I don't hear
music"*; `docs/decisions.md` C116 found the campaign half of that and fixed it;
the title screen stayed silent, and `audio::scene` said so in a comment that was
true and complete and still lost the track:

> *"The original plays no music here: `Music_StartCampaign` is reached from the
> campaign coming up, and the title screen's only sound is `setup.wav`."

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
| `Sound_PlaySlot` | `0x00426120` | bank | 14 | 11 | **drops** the request if that buffer is still playing |
| `FUN_004262cf` | `0x004262CF` | bank | 13 | 8 | a 28-byte thunk onto `Sound_PlaySlot` |
| `Sound_RestartSlot` | `0x00426216` | bank | 22 | 8 | rewinds and plays regardless — and the pointer click is two of these |
| `Sound_PlayFile` | `0x00427990` | file | 49 | 16 | one-shot by name; arg 2 picks the speech or effects flag; **drops** the request while its one buffer is busy |
| `Msg_PlayVoice` | `0x004B35C1` | voice | 16 | 14 | an `L2.eng` group to a filename, then `Sound_PlayFile` |
| `Sound_PlayTroopCry` | `0x00499CB1` | cry | 6 | 6 | 11 × 4 × 4 by a round robin, then `Sound_PlayFile` |
| `Music_StartCampaign` | `0x00499ACA` | music | 10 | 5 | the progress-bar ladder |
| `Music_StartBattle` | `0x00477B2F` | music | 4 | 4 | the alternating pair |
| `Music_Play` | `0x004263AD` | music | 9 | 2 | a named track, looped or not — the front end is all of these |

Calls that live *inside* one of those nine are the primitive's implementation
and are not counted. `Sound_StopOneShot` (16 sites) and the two bank preloads
(8) are excluded for the same reason: neither starts a sound.

### **A sound that will not stop is a defect this inventory cannot count**

`Sound_StopOneShot`'s sixteen sites are outside the census by construction, and
a player found what that costs: *"VO doesn't seem to stop when the dialogue that
produces it is closed, eg tutorial it will finish the line."* The inventory was
green, every voice line was `reproduced`, and the engine had **no
`stop_one_shot` call site at all** — the verb existed and nothing used it.

Two of the sixteen are now built, and they are the two a player meets:

| site | the statement | ours |
|---|---|---|
| `Msg_Dismiss` (`0x00476768`) | `if (g_messageGroup != 0xc2) Sound_StopOneShot();` — closing a window cuts the narrator, **except** group 194, *"Foiled again."* | `Director::listen`, on the open record changing |
| `Opt_ToggleMusic` (`0x004349A4`) | `if (g_optMusic == 0) { Music_Stop(0); Sound_StopOneShot(); }` — the Music row silences speech too | `Director::listen`, on the switch going off |

They carry no `// sfx:` marker:
`tools/oracle/sounds.js` scans for the nine *play* primitives. That is the right
denominator for *"what does the game ask for"* and the wrong one for *"does our
audio behave like the game's"*
closing — adding stops to the census would put sixteen rows in it that no file
depends on. `crates/l2-game/tests/audio_wiring/main.rs` is where the two are
asserted.

## The three verdicts, and the count of each

`docs/audio.json` gives every site one of four statuses, and three of them are
the verdicts a reader wants:

| status | sites | means |
|---|---:|---|
| `reproduced` | **89** | we fire it; a `// sfx:` marker is on the line |
| `blocked` | **19** | the mechanic behind it is not built, and `note` **names** it |
| `missing` | **31** | reachable and unwired — no excuse, just not done |
| `dead` | **4** | the shipped game cannot reach it; `note` is the evidence and `docs/bugs.md` has the entry |

**Four are dead, and this section used to say none were.** `tests/sfx.rs` asserted
the `dead` list empty *so that the first entry costs a decision*, and these are that
decision — it now pins them by name, so the fifth costs the same:

| id | why it cannot run | `docs/bugs.md` |
|---|---|---|
| `FUN_0040d6ad#1`, `FUN_0040d7b8#1` | the arrows of a slider widget whose hit-tester `FUN_0040D3F5` has **no caller** — zero rel32 calls, zero jumps, zero absolute references in the image, against 36 rel32 callers for `Widget_Test` as the control | `D39` |
| `Battle_PauseButton#1` | guarded `== 1` on a word that alternates between 0 and −1; the one writer that could make it 1, `FUN_00434E68`, is unreferenced too | D38 |
| `FUN_004b39e8#1` | the degraded castle's three lines, `S075_02/03/04` — the thunk that plays them has no caller either, by the same scan, with its own sibling `FUN_004B3714` (one rel32 caller, inside `Sidebar_Button`) as the near control | **D40** |

**The third was already on the bug list, and this file had it as `missing`.** The
assertion of emptiness is what kept it there: the inventory could not say what the
bug list already knew.

**The fourth was found by looking for a *caller*.** All
eight of the sibling voice thunks were filed *blocked on finding the caller*, and
seven of the callers were one `grep` of the corpus away. The eighth has none, which
is a different answer to the same question, and no amount of building would have
produced it.

The unreachable audio on the *other* side of the question is unchanged: `Ff_win.wav`
ships and **no call site names it** — a file with no trigger
with no path.

### What `blocked` is blocked on, in size order

| mechanic | sites | files it would add |
|---|---:|---:|
| ~~Smacker playback~~ **built** (`crates/l2-smk`, `crate::movie`): 5 of its 8 now sound. What still blocks the other 3: `County_ChangeOwner`'s capture letters, which nothing posts (`#15`, `#16`), and the CD's fast-media branch (`#19`) | 3 | 0 — a bed and a voice already reachable |
| ~~the sibling voice tables' callers~~ **six of the eight are gone.** Four are built — the mercenary offer (`FUN_004B3714`, `Sidebar_Button` hotspot 1, 12 files), the population panel's health line (`FUN_004B3768`, `Panel_OpenPopulation`, 4), the information panel's picked unit or castle (`FUN_004B37BC`, both openers of screen `0x04`, 9) and **the standings page's category** (`FUN_004B3994`, `crates/l2-game/src/screens/nobles.rs`, 7 of its 8) — and one is **dead**, `FUN_004B39E8` above. What is left: `S010`'s confirm box (`Ui_OpenConfirm`, and we have one of its thirteen call sites, as a flag) and the lord sting's `S246` | 2 | ≈17 — `S010` 13, `S246` 4 |
| `FUN_004B3940`, the castle chooser's five buttons speaking their own name — **the one site left here whose trigger is a click *inside* a screen**, and the selection is `CastleScreen`'s own field. `S035` above was the same shape and stopped being blocked when the selection was put where the original keeps it: `DAT_0055CE7C` is a global, so `Game::nobles_category` is the faithful placement *and* the one the director can see. `DAT_0056D898` is a global too | 1 | 0 — `S071_02`…`06` already sound from `FUN_004B37BC` |
| a channel from a click to the audio layer (the field brush) | 5 | 3 |
| the battle verdict (`ff_lose.wav`) | 2 | 1 |
| the two delegated message painters (categories `0x0C`, `0x14`) | 2 | 0 — both `ff_msg.wav` |
| state 17's own loose — `BattleMan_StateCloseToAttack` ×2 | 2 | 0 — the bow and crossbow already sound from state 5 |
| a realm eliminated mid-battle — `FUN_0047FE0B` | 1 | 0 — `deadguy4.wav` already sounds |

**Four rows left this table together**: battlefield fire (`BattleMan_BurnTick` ×2 and the
bridge fire `FUN_0048551D`), boiling oil (`FUN_0047A814`), a siege tower docking
(`FUN_00491492`) and the catapult shot on a rampart four high (`Missile_Step#2`) — six
sites and four files, `dest_ind.wav`, `pouroil.wav`, `siegedoc.wav` and `catmiss.wav`.
They were blocked on mechanics, and the mechanics are `crates/l2-sim/src/fire/mod.rs` and
the tower half of `siege.rs`; `docs/battle.md` §17 is what was read to build them.

**Read the second column before the first.**

**The tip screens left this table**, and they were the row a player would have heard
most of: two sites and forty files by name — 13 first lines and 27 chained takes, the
largest files-per-site ratio in the inventory. They are fired now
(`crates/l2-game/src/tip/mod.rs`), and **35 of the 40 can sound**: tips 212, 214
and 215 are guarded on `g_screenId == 0` during a battle, which no path was found to
hold, so `S212_01`, `S212_02`, `S214_01`, `S214_02` and `S214_03` ship silent in the
original as well as here. `[I]` on *"no path"*.

## The battlefield: the gap was the event stream, not the sounds

**This section replaces a paragraph that called the battlefield's 25 sites *"a
limit of the design and not a to-do"*.** Its premise was right — `Director::listen`
takes `&Game` and derives sound from the world after the tick, and a sword swing
is an event *inside* one — and its conclusion did not follow. The world simply
kept no record of the event: `l2-sim` resolved figure and unit state and wrote
nothing a listener could hear. That was the gap, and it was ours
design's.

`l2_sim::cue` is the record: monotone counts of the occasions the original's per-man
code sounds on — a man falling, keyed by the troop that struck him; a figure's last
man, keyed by its side; a missile loosed, hitting, felling and killing, keyed by
weapon; a wall shot and a wall smashed. The battle writes them and nothing in the
battle reads them.

**Counters are exact, not an approximation, because of the throttle.** Every one of
these calls is `Sound_PlaySlot`, its thunk `FUN_004262CF`, or `Sound_PlayFile`, and
**all three drop the request while their buffer is sounding** — `GetStatus` against
`DSBSTATUS_PLAYING` for a bank slot, `Sound_OneShotBusy` for the one-shot buffer. `[V]`
from the three bodies. Ten men falling to swords in one frame is one `sword2.wav` and
nine dropped requests. So what a tick can make audible is *whether each kind of event
happened*, and a count that moved since the last listen is exactly that. **The
original has no throttle beyond those drops** — no per-frame cap, no distance test,
no rate limit — and we add none.

| site | occasion | slot / file |
|---|---|---|
| `Melee_Tick#1`…`#4` | a man falls, by the striker: maceman / swordsman / knight / anyone else | 4 `sword5` · 5 `sword2` · 5 `sword2` · 6 `sword3` |
| `Melee_Tick#5`, `#6` | a figure's last man, side 0 / side 4 | `0xB` `deadguy2` · `0xC` `deadguy3` |
| `Missile_Step#3`, `#4` | a bolt / an arrow strikes a man | 10 `cros_hit` · 8 `bow_hit` |
| `Missile_Step#5`, `#6` | …and crosses the casualty threshold — the same slot, always dropped | 10 · 8 |
| `Missile_Step#7` | …and it was the last man | `0xD` `deadguy4` |
| `Missile_Step#1` | a catapult shot counted against a wall | `0xF` `cathit` |
| `BattleMan_FireMissile#1`, `#2` | a crossbow / a bow looses | 9 `crossbow` · 7 `bowmen1` |
| `BattleMan_StateEngineFire#1` | a catapult fires | `0xE` `catfire` |
| `FUN_0049694f#1` | `Wall_Smash` | `bathit2.wav`, the one-shot buffer |
| `BattleMan_BurnTick#1`, `#2` | a figure's last man dies in fire, side 0 / side 4 | `0xB` `deadguy2` · `0xC` `deadguy3` |
| `Missile_Step#2` | a catapult shot reaches a wall four or more high and is **not** counted | `0x10` `catmiss` |
| `FUN_0047a814#1` | a pot of boiling oil is poured | 3 `pouroil` |
| `FUN_00491492#1` | a siege tower docks | `0x11` `siegedoc` |
| `FUN_0048551d#1` | a bridge catches fire | `dest_ind.wav`, the one-shot buffer |

Every slot is `[V]` from the call's constant, and every one lands on a file whose
name says what the occasion is — `bowmen1` on the loose and `bow_hit` on the hit — which
is the third independent confirmation that bank slots are 1-based.

**Why it cannot feed back.** The record is outside the lockstep checksum on purpose,
the listener holds `&Game`, and two tests hold both halves: `l2-sim`'s
`a_battle_whose_cues_are_wiped_every_tick_is_the_same_battle` zeroes one copy's record
every tick and requires every other field to agree, and `tests/audio_battle.rs`'s
`sound_does_not_change_the_battle` plays one battle with a director listening and one
without and compares the whole `LiveBattle` at every tick. Both proofs have a siege twin
over `l2_sim::proving`'s constructed field, which pours oil, docks a tower, burns a bridge
and the men on it, and bounces shots off a wall four high inside two thousand frames:
`a_siege_whose_cues_are_wiped_every_tick_is_the_same_siege` and
`sound_does_not_change_a_siege_that_burns`, the second with real decoded audio as well.

## The troop cries: a round robin, and no random number anywhere

`Sound_PlayTroopCry(class)` (`0x00499CB1`), `[V]`:

```c
counter[unit][class] += 1;  if (3 < counter[unit][class]) counter[unit][class] = 0;
take = (class == 3) ? 0 : counter[unit][class];
Sound_PlayFile(g_troopSounds + class*0x40 + take*0x10 + unit*0x100, 1, 0);
```

* **`unit` is `DAT_0055408C`**, the troop type most of the local player's picked
  figures belong to — `Battle_CountMenByType` keeps the first strictly larger count,
  so a tie goes to the lower troop index and nothing picked is peasants. `Battle_Frame`
  reruns that census every frame.
* **The take is a round robin per (troop, class)**, stepped before it is read, so the
  first cry of each pair is take 1. `g_troopCryCounter` (`0x0053EF60`) is in `.bss` and
  this function is its only writer, so it starts at zero with the process and is never
  reset between battles.
* **The class is the order**: 0 `_U` a selection committed, 1 `_P` an order or the `H`/`V`
  keys, 2 `_E` an order onto an enemy, 3 `_M` an order onto surface 2. Class 3 is always
  take 0 — `docs/bugs.md` D34.
* **The throttle is `Sound_PlayFile`'s one buffer.** A cry asked for while any file is
  sounding — another cry, the narrator, a wall coming down — is not played, and **the take
  it would have played is spent**, because the counter stepped first.

So the determinism question — *which generator does a cry draw from?* — has the answer
**none**, and there is nothing to keep away from the battle's `Pcg32`. Presentation
randomness, if a future site needs any, belongs to a generator on the audio side and
never to the simulation's.

The table is `names::TROOP_CRIES`, asserted cell for cell against the executable. **66
files are reachable** and all ship; nine unreachable names do not, two more than D34
said.

## A player's three reports, and where each landed

1. **"I don't hear the clip-clop of the merchants on end turn."**
   `Unit_MoveInFacing` (`0x00466D84`), four sites, **fired** —
   `Director::hear_the_march`, one hoofbeat per tile per unit. It was wired on
   the audio branch and it works.

2. **"I don't hear music or the click sounds or industry sounds when you right
   click them on the map."** Three things, three answers.
   * *Music* — fixed for the campaign (C116) and **now** for the front end, which
     is the ninth-primitive finding above.
* *The click* — **fired**, and it was two sites: see the end
     of this file. `crates/l2-game/src/press/mod.rs` is `Widget_Test`, and the rule a
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
an oracle request — `docs/oracle-requests.md` §11: somebody
with the game needs to open a dairy-fed county's ration panel and listen.

> **C133's search was for the right condition in the wrong medium**
> reason it looked exhaustive is that `L2.eng` really is where this game keeps
> its words. It keeps some of them in `.wav` files instead, and a group with one
> consumer is a screen's vocabulary only for the half of the vocabulary that is
> written down.

## The voice: the draw is the behaviour, so the trigger is a countdown

`Msg_DrawWindow` (`0x0047309E`) is 10,915 bytes: it
dismisses, enqueues, sets its own timer and plays its own sound from inside the
draw. All sixteen `Msg_PlayVoice`
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

**Fourteen of the sixteen are fired.** `Msg_DrawWindow#16` and `#21` are the
**animated** capture and ending branches, which save the group and variant,
dismiss the message, play a film and speak. **This paragraph used to say they
speak *afterwards*, with the film's end as the trigger. They do not:** `Smk_Play`
returns as soon as `Smk_Open` has put the first frame up, so `Msg_PlayVoice` runs
with the film's opening and the narrator reads over it. `#21`, the ending, now
fires on the film's screen arriving; `#16`, the capture, is the same code and is
still not fired, because nothing in our engine posts a capture letter.

**The third is `#24`, and it was counted as fired twice: once before it could
sound, and now.** It is the branch for categories `0x05`…`0x09`, and the only
function in the original that posts one is `Tip_Show` (`0x00476DA9`), from
`g_tipCategory` — those categories *are* the tip screens. It was reverted when that
was read, because nothing posted a tip. `crate::tip` posts them now, and
`crates/l2-game/tests/tips.rs` hears `S200_01.wav` by name on the tick
`Msg_DrawWindow` tests.

**And the chained takes are fired with it.** `[V]`: `FUN_004B3ACD(group)` has one
caller, `Msg_DrawWindow`'s categories `0x05`…`0x09` branch, `else if
(g_messageTimer < 0x780)` — so from 81 ticks in, every frame. It reads
`n = table[group × 5 + cursor]` at `0x004E1E40`; if `Sound_OneShotBusy()` it stamps
the time, otherwise once **more than 999 ms** have passed since the last busy stamp
it advances the cursor and plays `S201_02.wav + (n − 1) × 0x10`. The cursor
`DAT_0052F004` is reset in
live rows are groups 200…218: 200 → 26, 27 · 201 → 1, 2, 3 · 202 → 4, 5, 6 ·
207 → 7, 8, 9 · 209 → 10…13 · 210 → 14, 15, 16 · 212 → 17 · 214 → 18, 19 ·
217 → 20…23 · 218 → 24, 25 — 27 takes, all shipping, read out of the executable into
`audio::names::TIP_TAKES`. `Director::chain_takes` is the function, and the one
divergence is stated there: `timeGetTime()` becomes the director's own tick count, so
*"more than 999 ms"* is 63 ticks at 16 ms. The test asserts the tick each take starts
on against when the mixer last had the narrator sounding.

### What is still missing inside the voice class
* **`FUN_004B3B92(lord − 1)`** — the sting a letter plays 90 ticks in,
  `S246_02.wav + lord × 0x10`. The function accepts indices 0…4 and the table
  holds **four** clips — `S246_02`, `_04`, `_03`, `_01` — and then `S000_00.wav`, a
  sentinel: four lords, not five. `[V]` from the bytes at `0x004E2470`.
* **Six sibling tables** — `g_msgVoiceS010`, `S016` (the mercenary offer, from
  `Sidebar_Button`), `S020`, `S035`, `S071` and two indirect ones. Each is one
  `Sound_PlayFile` behind one table index; the work is finding the caller.
* **Categories `0x0C` and `0x14`**, delegated to `Msg_DrawDiplomacy`
  (`0x00475E07`) and `Msg_DrawBeyondLetter` (`0x00476488`). **Unread**, and
recorded as unread, because inventing a tick for them would
  be worse than silence.

## The class nobody had wired: the sound is in the function that sets `g_screenId`

Fourteen sites, and the reason one mechanism reaches all of them is structural
**The original's screen sounds are statements in the
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

## The clip-clop is the bank class, and it has a rate

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
`docs/bugs.md` `D39`. The scan that proves it had to be run against
a control, because this binary calls with `E8` rel32 and an absolute-address search
finds nothing for *any* function, `Widget_Test` and its 36 callers included.

**Ours.** `crates/l2-game/src/press/mod.rs` is the hit test, so the count is taken
there — in `Press::press` and `Press::press_delayed`, and deliberately not in
`Press::tick` or `Press::press_held`. A screen cannot reach `Audio`
(`docs/netcode.md` D-3), so the count is an outbox: `Screen::take_clicks` drains it,
`Machine::handle` accumulates it **before** applying the transition — a press that
closes its own screen is still heard — and `Director::hear_the_click` plays
`click3.wav` when `Machine::clicks` has moved. Slot 1 is `click3.wav` in both
banks, so the battlefield's confirm box clicks like the county's tax arrows.

`tests/click.rs` is mostly the silent cases, because a test that only checks that a
click sounds cannot see a spinner that clicks thirty-three times a second. **One of
its ablations stayed green**, adding a click to
`Press::tick` went unnoticed, because `Machine::update` did not drain the outbox and
the stray click waited in the `Press` for the next event. It now drains on both
paths, and the held-arrow tests let go at the end so a click that is held back
cannot hide.

Writing it found a defect that was not audio's: **the county panel dropped a double
click on the floor**, where `Ration_SliderClick` and `Widget_Test` both take one as
a press. Six more screens own a widget table and still do; `docs/input.md` §5
counts them.
