# Every place the original asks for a sound

**134 trigger sites across 66 functions. We reproduce 24 — and they carry 555 of the
install's 771 sounds, because the classes are wildly unequal in weight.**

A player has been finding missing sounds one at a time — no music, no clicks, no
industry voice, no clip-clop from the merchants — and each one costs a round
trip. This file is the audio equivalent of the input-arm audit (`docs/decisions.md`
C61): the list of every trigger the original has, so that *"I don't hear X"*
becomes a row somebody already knows about.

    node tools/oracle/sounds.js            # the table
    node tools/oracle/sounds.js --count    # the totals

**The denominator is generated, not typed.** It reads `tools/oracle/decomp/*.c`
and attributes each call to the function whose header it falls under. Every
number in this file comes out of that script, which is the standard
`docs/agents.md` sets: *a number that cannot drift beats a number that is
checked*.

## Why this was cheap, and why the input audit was not

The input audit had to read every screen, because `Screen_FrameInput` is a
hundred-arm ladder and an arm is a shape rather than a symbol. **Sound is the
opposite: the original funnels every sound in the game through eight leaf
functions.** So the denominator is a grep, and it took minutes.

| primitive | addr | class | sites | what it is |
|---|---|---|---:|---|
| `Sound_PlaySlot` | `0x00426120` | bank | 14 | **drops** the request if that buffer is still playing |
| `FUN_004262cf` | `0x004262CF` | bank | 13 | a 28-byte thunk onto `Sound_PlaySlot` |
| `Sound_RestartSlot` | `0x00426216` | bank | 22 | rewinds and plays regardless |
| `Sound_PlayFile` | `0x00427990` | file | 49 | one-shot by name; arg 2 picks the speech or effects flag |
| `Msg_PlayVoice` | `0x004B35C1` | voice | 16 | an `L2.eng` group to a filename, then `Sound_PlayFile` |
| `Sound_PlayTroopCry` | `0x00499CB1` | cry | 6 | 11 × 4 × 4, then `Sound_PlayFile` |
| `Music_StartCampaign` | `0x00499ACA` | music | 10 | the progress-bar ladder |
| `Music_StartBattle` | `0x00477B2F` | music | 4 | the alternating pair |

Calls that live *inside* one of those eight are the primitive's implementation
and are not counted — `Msg_PlayVoice`'s three `Sound_PlayFile` calls are one
voice line, not three triggers. `Sound_StopOneShot` (16 sites) and the two bank
preloads (8) are excluded for the same reason: neither starts a sound.

**Two enumerations disagreed, and the second was right.** The first pass counted
121 and missed `FUN_004262cf` entirely — a 28-byte forwarder with **13 callers
of its own**, all of them battlefield sounds. A wrapper is exactly what a
single-pass grep loses, and the whole melee is behind this one. `docs/method.md`
§4: an absence is evidence only in proportion to how hard it was looked for.

## What we reproduce: 24 of 134 — and why that fraction understates it badly

| class | sites | ours | **files it carries** | what is missing |
|---|---:|---:|---:|---|
| **voice** | 16 | **16** | **543** | the *chained* takes, below |
| music | 14 | 4 | 9 | `Msg_DrawWindow` ×3, `Smk_OnFinished`, `CastleBuild_Confirm`, two unnamed |
| file (fanfares, panels, the front end) | 49 | 4 | 3 | 45, listed by the script |
| bank (the world, the UI, the battlefield) | 49 | 0 | 0 | everything |
| cry | 6 | 0 | 0 | needs the battlefield's order path |

**A trigger-site count weights every site equally and a player does not.** Sixteen
of the 134 sites — twelve per cent — carry **543 of the 771 shipped sounds**,
because `Msg_PlayVoice` is a *table lookup* and the other primitives are mostly
constants. A player put it as *"that guy's voice acting is half the personality of
the game"*, and by file count he understates it: 646 of the 771 files are somebody
speaking, 84 %.

That is worth carrying beyond this file. Every 1:1 measure this project keeps —
input arms, draw calls, and now sound triggers — counts things that are equal
inside their own list and are not equal to a person. **The count is still the right
denominator**; it just should not be the only number quoted, and a second column
saying what each row *carries* costs nothing.

### The voice: the draw is the behaviour, so the trigger is a countdown

`Msg_DrawWindow` (`0x0047309E`) is 10,915 bytes and it is not a painter: it
dismisses, enqueues, sets its own timer and plays its own sound from inside the
draw. There is no call site to put a voice beside. All sixteen `Msg_PlayVoice`
calls are guarded by `g_messageTimer == <constant>`, and the timer counts **down**
from 2000, one per tick. `[V]`.

| category | ticks after opening | constant |
|---|---:|---|
| `0x02`, `0x03`, `0x05`…`0x09`, `0x0F`, `0x10`, `0x11`, `0x12` | 10 | `0x7C6` |
| `0x00` notice, `0x0D` capture (unanimated) | 90 | `0x776` |
| `0x0E` ending (unanimated) | 100 | `0x76C` |
| `0x01` letter, `0x0A` pay prompt, `0x0B` alliance prompt | 200 | `0x708` |
| `0x04` tip | 10 | `0x5A`, against a timer clamped to 100 |
| `0x13` help | — | silent |

**The five constants are one rule and a delay.** `0x7C6` is 1990 against a start of
2000 and `0x5A` is 90 against the tip's clamped 100: *both are ten ticks after the
window opened*. The three larger delays belong to the categories that play a
**fanfare** on the opening frame — the voice waits for the trumpet instead of
talking over it, and the longest, 200 ticks, is the one that also plays a lord's
sting at 90.

The five values were already on file: `crates/l2-game/src/screens/message.rs` lists
them and says they are *"recorded in `crate::message`"*, **where they had never been
written** — a citation that did not resolve, which is the doc form of a rule with no
way in. **Which category takes which** was not recorded at all, and that is the half
a caller needs.

### What is still missing inside the voice class

* **The chained takes.** `FUN_004B3ACD(group)` walks a five-wide table at
  `0x004E1E40` and plays `S201_02.wav + (n − 1) × 0x10` **one clip at a time as
  each finishes**, gated on `Sound_OneShotBusy()` and a 1,000 ms gap — so a notice
  is read as a *sequence* of takes rather than one line. That is why `S010_13.wav`
  exists. We play `_01` and stop. Wiring it needs the mixer to answer *"is a
  one-shot still playing?"* (`Mixer::is_playing` can) and a per-message cursor
  (nothing keeps one).
* **`FUN_004B3B92(lord − 1)`** — the sting a letter plays 90 ticks in,
  `S246_02.wav + lord × 0x10`, five lords.
* **`FUN_004B36C0`** — `g_msgVoiceS010`, sixteen entries, `S010_01.wav`…
* **Categories `0x0C` and `0x14`**, which `Msg_DrawWindow` delegates to
  `Msg_DrawDiplomacy` (`0x00475E07`) and `Msg_DrawBeyondLetter` (`0x00476488`).
  Each carries a voice call on its own schedule; **unread**, and recorded as unread
  rather than absent, because inventing a tick for them would be worse than silence.

**`g_msgVoice200`'s extent is 284, not 299.** `crates/l2-game/src/audio/names.rs`
said `200 ..= 299`, which invents fifteen groups. `docs/symbols.md` says 85 entries,
and the install settles it independently: the highest `S2xx` file that ships is
`S284_02.wav`. Corrected, and asserted.

### The industry lines a player asked for, which now need nothing from audio

`Industry_ToggleFromMap` (`0x0043D309`) ends with
`Msg_Enqueue(0, g_localPlayer, local_10 + 0xE6, …)`, `local_10` being
`industry × 2 + on` and `−1`/`−2` for the castle switch. So `L2.eng` **228/229 are
the castle** (*Building off/on*) and **230…237** the four industries, and all ten
`S2xx_01.wav` ship.

Because the voice hangs off `(category, group)` and nothing else, **those lines
speak the moment the enqueue lands** — the industry branch supplies the message, and
no further change is needed here. `audio_wiring.rs::the_industry_toggle_groups_all_have_a_voice_that_ships`
asserts our half of that today.

The four music sites we have are `Game_NewGame`, `Battle_Start`,
`County_ChangeOwner` and `Opt_ToggleMusic` — and the last two we get for free,
because `audio::Director` re-derives the scene every tick rather than reacting
to an event, so a conquest or a toggle changes the bed without a call site of
its own. That is a genuine reproduction of the effect and it is worth saying
plainly, because the site count alone would score it zero.

The three file sites are `Battle_ChooseSettlement`'s `ff_batl.wav` and one of
`Msg_DrawWindow`'s five `ff_msg.wav` calls, plus the approximation `Director`
makes for the second (`docs/decisions.md` C116).

## The clip-clop is the bank class, and it has a rate rather than a trigger

A player: *"I don't hear the clip-clop of the merchants on end turn that I'm
used to."* It is `Unit_MoveInFacing` (`0x00466D84`), whose **first statement
after unlinking the unit from its tile** is a four-arm ladder on the unit's kind:

```c
Unit_UnlinkFromTile(g_movingUnit);
if      (g_units[g_movingUnit].kind == 3) Sound_PlaySlot(0xb);   /* merchant.wav */
else if (g_units[g_movingUnit].kind == 4) Sound_PlaySlot(0xb);   /* merchant.wav */
else if (g_units[g_movingUnit].kind == 2) Sound_PlaySlot(5);     /* rioters.wav  */
else if (g_units[g_movingUnit].kind == 1) Sound_PlaySlot(0xc);   /* army.wav     */
```

`[V]`. Three facts follow, and the third is the one that decides the wiring:

* **It is the sample-bank class**, so wiring it reaches the rest of that class's
  49 sites through the same verb, not just this one sound.
* **It fires on every step of every moving unit**, with no guard on owner or
  visibility. It is a *rate*, not a trigger — the coordinator's question, and
  the answer is rate.
* **`Sound_PlaySlot` is the drop-if-busy verb**, which is the entire reason a
  per-step call does not become a roar. Twelve units crossing the map cost one
  voice per distinct sound. [`Audio::play_effect_if_idle`] is already exactly
  this verb, is already tested, and **has no caller**.

**Not wired here, deliberately.** The rate this fires at *is* the movement tick
rate, and a merchant-pacing agent is live in `crates/l2-game/src/turn.rs`
settling exactly that — the player also reports merchants stalling and then
sprinting. Attaching a sound to a tick whose rate is being changed underneath it
would mean two agents guessing at each other. **The hand-off is one line**: at
the point a unit advances one tile, call
`audio.play_effect_if_idle(names::slot(Bank::Kingdom, n))` with `n` from the
ladder above — 11 for a merchant or transport, 12 for an army, 5 for a peasant
mob. `names::slot` already maps those and is tested against the install.

## What a full inventory would cost

The three checks that made the arms audit worth having (`docs/agents.md`, *Rule 5
needs a check*) all apply, and two of the three are already paid for:

| | arms audit | this |
|---|---|---|
| enumerate the denominator | 3 agents × ~30 min, by reading screens | **done** — `sounds.js`, minutes, regenerable |
| resolve each site to a name | by hand | **mostly free** — the slot or the literal is on the same line |
| verdict per site | ~185 rows | ~134 rows, and 49 of them are one class |
| markers in our code + set equality | ~a day | ~a day, and unavoidable |

So: **the expensive half is the same and the cheap half is already done.** The
recommendation is to spend the day, because unlike the arms audit this one has a
generated denominator that cannot rot, and because the sites cluster — the 49
bank sites are `Unit_*` on the campaign and `BattleMan_*`/`Melee_*`/`Missile_*`
on the battlefield, so a handful of hooks close most of them.

The one row that is *not* cheap is `Sound_PlayFile`'s 49, which are scattered
across 37 functions with no common shape. Those are the ones to read last.

## A correction this produced

**The pointer click is not one call site behind one hit-tester.** It is four,
behind three: `Widget_Test` (`0x0040DA1E`) plays `Sound_RestartSlot(1)` at two
sites, and `FUN_0040D6AD` and `FUN_0040D7B8` are two more hotspot testers with
the same first statement —

```c
if ((g_mouseLeftPressed == '\0') && (g_mouseLeftDoubleClick == '\0')) return 0;
Sound_RestartSlot(1);
```

`crates/l2-game/src/audio/mod.rs` said *"one call site, because the whole game
shares one hit-tester"*, and that was wrong in a way that matters: whoever builds
the shared widget layer has **three** functions to reconcile, not one. Corrected
there.
