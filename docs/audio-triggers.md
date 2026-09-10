# Every place the original asks for a sound

**134 trigger sites across 66 functions. We reproduce 7.**

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

## What we reproduce: 7 of 134

| class | sites | ours | what is missing |
|---|---:|---:|---|
| music | 14 | **4** | `Msg_DrawWindow` ×3, `Smk_OnFinished`, `CastleBuild_Confirm`, and two unnamed |
| file (fanfares, panels, the front end) | 49 | **3** | 46, listed by the script |
| bank (the world, the UI, the battlefield) | 49 | **0** | everything |
| voice | 16 | 0 | needs the message window |
| cry | 6 | 0 | needs the battlefield's order path |

The four music sites we have are `Game_NewGame`, `Battle_Start`,
`County_ChangeOwner` and `Opt_ToggleMusic` — and the last two we get for free,
because `audio::Director` re-derives the scene every tick rather than reacting
to an event, so a conquest or a toggle changes the bed without a call site of
its own. That is a genuine reproduction of the effect and it is worth saying
plainly, because the site count alone would score it zero.

The three file sites are `Battle_ChooseSettlement`'s `ff_batl.wav` and one of
`Msg_DrawWindow`'s five `ff_msg.wav` calls, plus the approximation `Director`
makes for the second (`docs/decisions.md` CNEW-bottom).

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
