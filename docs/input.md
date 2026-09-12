# The original's input model

*What kind of gesture a control answers, as opposed to which control it is.*

`docs/arms.json` counts **which** gestures a screen answers. This document is the other
half: **what kind** each one is. The two are independent, and until this was written we
had the first and none of the second — so an arm could be marked `reproduced`, be
genuinely present, and feel wrong to a player in every case.

A player reported three things in one breath and they are one thing:

> *"Clicking yes/no (gauntlet thumbs up and down) is instant, whereas the game waited on
> mouse-up, and the gauntlet would go down slightly when clicked."*
>
> *"Holding on a button doesn't seem to make it go up faster. I recall you could click an
> up arrow and after a few seconds the number would go up fast."*

All of it is one byte of one record, read by one of two functions.

Everything below is **[V]** — decoded from `Lords2.exe` and, for the tables and the ramp,
checked byte for byte against the player's own copy by install-gated tests
(`crates/l2-game/tests/arms.rs`, `crates/l2-game/tests/press.rs`).

---

## 1. There are two hit-testers and one record

The whole interface is data: arrays of **24-byte records**, walked by two functions.

| offset | field |
|---|---|
| `+0x00`, `+0x02` | x, y (`Widget_Test`) — or x0, y0 (`Hotspot_Test`) |
| `+0x04` | base sprite frame (`Widget_Test`) — or x1 (`Hotspot_Test`) |
| `+0x06` | side of the square hit box (`Widget_Test`) — or y1 (`Hotspot_Test`) |
| `+0x08` | **the handler**, a function pointer |
| `+0x0C` | latch state — the drawn on/off of a toggle |
| `+0x0D` | **the press timer**, in frames. Non-zero means the pressed frame is showing |
| `+0x0E` | **the auto-repeat counter**, in 30 ms steps |
| `+0x0F` | **the kind** |
| `+0x10`, `+0x14` | `g_uiHotspotId` and `g_uiHotspotArg`, published before the call |

* **`Widget_Test` (`0x0040DA1E`)** — the *visible* buttons. It hit-tests a **square** of
  side `+0x06` and its records are drawn by `Widget_Draw` (`0x0040CFD2`).
* **`Hotspot_Test` (`0x0040E3EE`)** — the *invisible* ones. Same record, read as a
  rectangle `{x0, y0, x1, y1}`, nothing drawn. The map sidebar, the battle HUD, the
  armoury racks and every setup page are these.

**The kind numbers do not overlap between the two**, which is why the tester is part of
the question: `Hotspot_Test`'s 3 is a release; `Widget_Test` has no 3 that fires at all.

## 2. The five kinds

| kind | tester | fires on | pressed frame | repeat | records | handlers |
|---|---|---|---|---|---|---|
| 1 | `Hotspot_Test` | `g_mouseLeftPressed` — the **down edge** | none | none | 121 | 38 |
| 2 | `Hotspot_Test` | down edge, **then every 320 ms while held** | none | flat | **23** | **7** |
| 3 | `Hotspot_Test` | `g_mouseLeftReleased` — the **up edge** | none | none | 7 | 2 |
| 4 | `Widget_Test` | down edge | `base + 1` for 3 frames, refreshed while held | **accelerating** | 139 | 53 |
| 5 | `Widget_Test` | **20 frames after the down edge** | `base + 1` for all 20 | none | 42 | 29 |

These are `docs/arms.json`'s gesture vocabulary — `left-press`, `left-press-held`,
`left-release`, `left-press-repeat`, `left-press-delayed` — and they are the original's
own, not a taxonomy of ours.

**The two right-hand columns are generated, not counted by hand.**
`node tools/oracle/kinds.js` walks both regions at a 24-byte stride, groups by `+0x0F`
and prints every handler; `--counts` prints just this table's numbers. It totals **332
records with a handler and not one whose kind byte is outside the five**, which is the
closure claim: there is no sixth kind in the tables. The script spot-checks two records
against handlers named from call sites before it will print anything, because a wrong
region bound decodes plausible rubbish rather than nothing.

> **Kind 2 is 23 records over 7 handlers, and this document said *"one pair, a
> setup-page scroll"*.** That was written from one call site and was wrong by a factor
> of eleven. It matters less than it looks — all seven handlers are skirmish and
> multiplayer setup pages, and §6's verdict on our side is unchanged — but it is the
> exact shape `CLAUDE.md`'s note about `[V]` warns of: a true statement about one table,
> promoted to a statement about a kind. The cure is the same one that found it, which is
> to make the count come out of the exe rather than out of a sentence.

### Answering the player, item by item

**Press or release?** *Both, and which is a property of the control.* The single
most-used exit in the game is a release: **`Ui_OkButtonClicked` (`0x0040E7E4`)** — the
24 × 24 corner picture — opens `if (g_mouseLeftReleased == 0) return 0;`, and
`Screen_FrameInput` calls it **twenty-six times**. Hotspot kind 3 is a release too.
Everything else on the list fires on the press.

**Is there a pressed frame?** Yes, and it is a *sprite index*, not a colour effect.
`Widget_Draw`:

```c
if (kind == 4 || kind == 5) {
    frame = rec[0x04];
    if (rec[0x0D] != 0) frame = rec[0x04] + 1;   /* the press timer is running */
}
```

and then it marks the rectangle with **`Gfx_MarkWidgetUrgent`** rather than the ordinary
`Gfx_MarkSpriteDirty`, so the depressed picture appears on the same frame as the press
instead of at the next general redraw. *"The gauntlet would go down slightly when
clicked"* is `base + 1`, urgent.

**Is there auto-repeat, and does one mechanism serve every spinner?** **Yes, and yes.**
Every `+`/`−`, every `<`/`>`, every up/down arrow in the game is `Widget_Test` kind 4 —
the armoury's equip/unequip, the supplies `+`/`−`, the army-division steppers, the
diplomacy gift stepper, tax, rations, castle build, siege engines, the save/load scroll
pair. One function, one ramp, one 30 ms clock. There is no second implementation anywhere.

## 3. The auto-repeat, exactly

`Widget_Test`'s kind-4 hold branch, in full:

```c
if (g_mouseLeftDown == 0) return 0;
rec[0x0D] = 3;                                   /* keep the pressed frame up */
if (DAT_004EA128 != 0) {                         /* the 30 ms gate */
    rec[0x0E]++;
    if (rec[0x0E] < 0x30) {
        if (rec[0x0E] < 8) return 0;             /* nothing for seven steps */
        if ((&DAT_004D2748)[rec[0x0E]] == 0) return 0;
    } else {
        rec[0x0E] = 0x2F;                        /* clamp — and SKIP the table */
    }
    ... publish the hotspot id and call the handler ...
}
```

**The clock.** `DAT_004EA128 = FUN_004B20ED()` once per frame in the whole-game frame
function. `FUN_004B20ED` returns `timeGetTime() - stamp` when that is **30 ms** or more
and **does not advance the stamp otherwise**. So the counter advances at most once per
30 ms and at most once per frame, whichever is slower.

**The ramp.** `DAT_004D2748` is a hand-authored 48-byte table, and the button fires on a
step whose entry is non-zero:

```
idx:  0  1  2  3  4  5  6  7 | 8  9 10 11 12 13 14 15 16 17 18 19 20 21 22 23
val:  8  8  8  8  8  8  8  8 | 1  0  0  0  0  0  1  0  0  0  0  1  0  0  0  1
idx: 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40 41 42 43 44 45 46 47
val:  0  0  1  0  0  1  0  0  1  0  1  0  1  0  0  1  1  1  1  1  1  1  1  0
```

Entries 0…7 are never read (the code returns first) and neither is entry 47 (the clamp
branch fires without consulting the table). So the schedule is

> **steps 8, 14, 19, 23, 26, 29, 32, 34, 36, 39, 40, 41, … 46, then every step**

— gaps of 6, 5, 4, 3, 3, 3, 2, 2, 3, 1, 1, … In milliseconds: the press fires at once,
the **first repeat 240 ms later**, and it is running flat out — 33 a second — from
**1.44 s**. There is a wobble at 36 → 39 that breaks the monotonic ramp. It is in the
game and it is reproduced rather than smoothed; a formula fitted to this table would be
a guess where a copy is a fact.

`crates/l2-game/src/press.rs` is this, and
`crates/l2-game/tests/press.rs` pins the 48 bytes against the player's own executable —
because every other test of the ramp is computed *from* the constant and would agree with
any table whatever (`docs/agents.md`, *ablating a constant while computing your probe from
that same constant tests nothing at all*).

## 4. The delayed press, which is the gauntlet

`Widget_Test`'s kind-5 branch does **not** call the handler:

```c
if (g_mouseLeftPressed || g_mouseLeftDoubleClick) {
    Sound_RestartSlot(1);        /* click3.wav */
    rec[0x0D] = 0x14;            /* twenty frames */
    rec[0x0C] = 1;
    return hit;                  /* and that is all */
}
```

The handler runs in the **countdown loop at the top of the next calls**, on the frame
`rec[0x0D]` reaches zero:

```c
if (rec[0x0D] != 0 && --rec[0x0D] == 0 && rec[0x0F] == 5) {
    g_uiHotspotId = rec[0x10]; g_uiHotspotArg = rec[0x14];
    (*rec[0x08])();
}
```

So: press → the picture goes down at once → **twenty frames** → the action happens. That
delay, with the button visibly held down through it, is what the player read as *"the game
waited on mouse-up"*.

**The countdown is one per record, not one per table.** `+0x0D` is a byte of each record,
and the loop above walks the whole table on every call and does not return after a call.
So two gauntlets pressed a few frames apart are both drawn down and **both act**, each
twenty frames after its own press; and pressing a record whose countdown is already running
just writes `0x14` again, so a button pressed twice within twenty frames acts once, twenty
frames after the second press. `crates/l2-game/src/press.rs` held a single timer and a
single pending widget until this was read: the second press overwrote the first, and a
spinner pressed while a thumb was waiting cancelled the thumb. It keeps a timer per record
now, and `Press::tick` returns every handler owed on a tick, in the order `Widget_Test`
calls them.

**Which controls are kind 5**, decoded from the tables: `g_confirmWidgets`
(`0x004DD310`) — **the yes/no box's two gauntlets**, `Ui_OpenConfirm`'s *"Exit the
game?"*, *"Autocalc battle?"*, *"Disband army?"*, *"Combine armies?"* — the
army-division confirm and cancel (`Army_SplitConfirm`), diplomacy's send and its six
verb buttons, `CastleBuild_Confirm`, the supplies dispatch/cancel pair, and **every
option checkbox** on the four options pages.

**Twenty *frames*, not milliseconds.** The frame loop is uncapped — `App_IdleFrame` calls
`App_Draw` whenever no window message is waiting — so the wall-clock duration depended on
the machine. Ours uses twenty 16 ms ticks, 320 ms, and that is a departure recorded
rather than hidden: nothing below the renderer may read a clock (`docs/netcode.md`).

## 5. The gestures that are not a kind byte

* **Drag** — `g_mouseLeftDown && g_mouseInputChanged`. `Ration_SliderClick` is the
  example and it **returns 0 on the release**: a drag is not a click with extra steps.
* **Hover** — the tool tip, `FUN_00476E95`. Not a record and not a kind: a frame
  function that waits for `g_mouseInputChanged` to stay clear for **more than 999 ms of
  `timeGetTime`** (63 of our 16 ms ticks), then asks one of two pointer ladders for an
  `L2.eng` group 220 index. A button change counts as a mouse change, so a click takes a tip
  away as a move does. `crates/l2-game/src/tooltip.rs`.
* **Double click** — `g_mouseLeftDoubleClick`, a *different flag* set from
  `WM_LBUTTONDBLCLK`. Windows sends it **instead of** the second press. **And it does not
  hold the button down**: `App_WndProc` (`0x004B29BE`) answers `0x203` with
  `DAT_004EADA1 |= 1` and nothing else, and only `0x201` sets the down bit, so
  `g_mouseLeftDown` stays clear for as long as the second press is held. A double click on
  a kind-4 spinner steps once and **does not auto-repeat**, and the `WM_LBUTTONUP` that
  ends it changes no bit and so raises no `g_mouseLeftReleased` either. `[V]` Ours still
  delivers an `Event::Release` for that second up; every release-gated control has already
  answered the first click's release by then, so nothing we have found acts on it twice.
* **The settled click** — release, then **300 ms with no second press**:
  `g_mouseClickPending` is set on the release, `g_mouseClickSettled` on the timeout, and
  `g_mouseClickX/Y` hold the *release* position. It has **exactly one reader** in the
  binary, `Village_ClickJob` (`0x0043A123`), which is why a click on a village cluster
  opens the job popup only after a beat. The village arm arms it with
  `g_mouseClickArm = 1` at the end of its ladder.
* **The click sound.** `Sound_RestartSlot(1)` — `click3.wav` — is played by
  **`Widget_Test` only**, only for kinds 4 and 5, and only on the initial press. Not on
  the auto-repeat, not by `Hotspot_Test`, not by `Ui_OkButtonClicked`. That is the answer
  to *"where is the one hook for the click sound"*: **there is no single hook**, and the
  original does not have one either — it has one *hit-tester* that happens to own the
  visible buttons.

  **This paragraph was right and `docs/audio-triggers.md` was not.** That file corrected
  it to *"four sites behind three hit-testers"*; the other two sites are the arrows of a
  slider widget nothing instantiates (`docs/bugs.md` `D39`). Reproduced
  now, in `Press::press` and `Press::press_delayed` — the kinds 4 and 5, the press only —
  and carried to the audio layer by `Screen::take_clicks` → `Machine::clicks` →
  `Director::hear_the_click`. `tests/click.rs` asserts the silent cases.

  **And the double click, which the click's own guard names.** This paragraph counted six
  screens that dropped `Event::DoubleClick`. **Five did; the battle prompt never had**,
  because `BattlePromptScreen::handle` hands every event to `Press::event`. What the
  original does on each, read from the arm and every guard in it (`[V]`), and what ours
  does now:

  | screen | answers a double click | does not |
  |---|---|---|
  | `0x12` battle prompt | both thumbs (kind 4) | — the arm reads no mouse flag at all |
  | `0x0B` diplomacy | the verb buttons (kind 5: restart twenty frames) | the lord cards, `FUN_004369BD`, `g_mouseLeftPressed`; the corner, a release |
  | `0x1A` compose | send and cancel (kind 5), the gift stepper (kind 4) | the county picker `FUN_0043B4CB`, `g_mouseLeftPressed`; the corner |
  | `0x11` divide | all eighteen widgets (kinds 4 and 5) | the corner; the minimap epilogue, `g_mouseLeftPressed \|\| g_mouseRightPressed` |
  | `0x04` information | the garrison widget (kind 4) | the corner and the brush (releases); the unit buttons (hotspot kind 1); the minimap |
  | message scroll | a prompt's two thumbs (kind 4), returning 1 | the 48 × 48 corner, `g_mouseLeftPressed`; anything else passes down |
  | `0x18` supplies | the spinners (kind 4) and thumbs (kind 5) | the two icons (hotspot kind 1); the minimap pick `FUN_0043B412` |

  Each is asserted through the screen stack: `tests/military.rs`, `tests/gestures.rs`,
  `tests/diplomacy.rs`.

## 6. What we reproduce, by kind

The honest denominator. The arm count in `CLAUDE.md` counts **arms**; this counts
**kinds**, and every arm has both — which is the whole reason a screen could answer 154
arms and still feel wrong in every one of them.

| kind | in the original | ours |
|---|---|---|
| `left-press` (hotspot 1) | 121 records, 38 handlers | reproduced — `Event::Click` is the down edge, and `Kind::Press` is it |
| `left-release` (hotspot 3, `Ui_OkButtonClicked`) | 26 `Ui_OkButtonClicked` call sites plus 7 kind-3 records | reproduced — `Event::Release`, and `Kind::Release` in the shared table |
| `left-press-repeat` (widget 4) | 139 records, 53 handlers | **built and wired on nine screens**: county tax and rations, supplies, army division, the battle prompt, the message scroll's five prompts, the diplomacy gift stepper, the info panel's garrison widget, **the save/load box's four widgets and the trade panel's six** — the last two answered raw clicks, with no click and no pressed picture, and the trade arrows did not repeat |
| `left-press-delayed` (widget 5) | 42 records, 29 handlers | **built and wired on seven**: the yes/no box, army division, supplies, diplomacy's six verb buttons and its send/cancel, the **twelve rows of the four options panels**, **the castle-build yes/no** and **the raise-army screen's Continue, tick and cross** — the last two acted on the click |
| `left-press-held` (hotspot 2) | 23 records, 7 handlers | **built and reaches nothing.** All seven handlers are skirmish and multiplayer setup pages this engine does not have; `Kind::Held` exists so the day one arrives it is a declaration and not a rewrite |
| the pressed frame (`base + 1`) | every kind-4 and kind-5 widget | **drawn on thirteen screens** — `Press::is_pressed(i)` is record `i`'s `+0x0D`, and the painter adds one |
| the repaint while held | the handler paints (`Tax_IncreaseCounty` ends `Panel_Tax()`; the gift stepper and `SaveLoad_Scroll` set `g_redrawRequest = 2`), or `Battle_Frame`'s every-frame `Screen_DrawWidgets` does (`Screen_SplitArmyRows`, `FUN_0041AEA2`, `Trade_DrawPanel`) | **reproduced** — `Press::take_redraw`, handed up through `Screen::take_redraw` by every screen that owns a `Press`. Ours stepped the number on every pulse and showed it at the release |

**Which yes/no boxes, enumerated.** Every record in the exe drawn with the mailed hands,
frames 29 and 31 — `node tools/oracle/kinds.js | grep -E "f29|f31"` — and its kind: the
yes/no box, the division confirm, supplies, diplomacy's send, the castle build and the
raise-army hire are kind **5**; the battle prompt, the five message prompts, the save/load
box and the trade panel are kind **4**. `g_smackTestWidgets`' pair (`FUN_00434F53`) is kind
5 on a screen this engine does not have. **Four of those were not answered through `Press`
at all** — save/load, castle build, raise army, trade — and a player met two of them.

**The `// arm:` marker for these kinds is now the declaration itself.** A widget's kind is
written `crate::arm!("<id>", Delayed)`, which *is* `Kind::Delayed` and is also the marker
`tests/arms.rs` reads, with the gesture taken from the identifier; and a comment marker may
not claim `left-press-repeat`, `left-press-delayed` or `left-press-held`. So the kind a
widget is declared with and the word its marker claims cannot disagree.

**What is built is the layer; what is not is the wiring, and the two are different
claims.** `crates/l2-game/src/press.rs` carries `Kind`, `Widget` and `Press::event`, so a
screen **declares** the kind of each rectangle and stops keeping press/release state of
its own — that is the same shape as the original, where the kind is a byte in the record
and the tester does the rest. What remains is that our screens name a small fraction of
the original's 332 records. That is countable rather than remembered:
`node tools/oracle/kinds.js` is the original's side and `docs/arms.json` is ours.

> **Say the boundary in the same sentence as the number.** Of the **five gesture kinds**,
> all five are now built and one of them is wired to nothing. Of the original's 332
> input records, `docs/arms.json` names a fraction. The gesture field is what makes both
> of those sentences checkable instead of an impression — and it earned that on this
> branch, which found **four records filed under a kind the exe contradicts**, three of
> them marked `reproduced`.

## 7. What checks this

Three, in rising order of what they can catch:

1. **The vocabulary is closed** (`crates/l2-game/tests/arms.rs`). A new kind costs a
   decision rather than a keystroke. It also caught `button-0` … `button-5`: **fourteen
   records had been filing a *position in a table* in the field that holds a *kind*.**
2. **The marker carries the gesture**, and the check is set equality on **(id, gesture)
   pairs**, not on ids. An arm answered with the wrong kind stops counting as reproduced.
3. **The kind byte, out of the player's own `Lords2.exe`** — install-gated, and the only
   one of the three that cannot be typed into agreement, because the other two compare two
   things one person maintains in one sitting. It classifies **39 records** and it found
   three wrong the day it was written.

**What check 3 cannot see, said next to the number.** Only records whose `addr` is a
*table handler*. An arm dispatched from `Screen_FrameInput`'s own ladder reads
`g_mouseLeftPressed` inline and has no kind byte to read; `Ui_OkButtonClicked`'s four arms
are in that blind spot, which is exactly where all four wrong ones were. The coverage
count is asserted so it cannot fall to zero quietly, but a bigger blind spot than that is
not something an instrument inside it can report.

### 7a. The blind spot had four arms in it, and closing half of it is one predicate

Check 3 asks *"is this record's `addr` a table handler?"* — and the four arms it could not
judge all have `addr` = `0x004BA9C8`, `Screen_HandleInput`, a 3,832-byte dispatcher that
is nobody's handler. They name their table **in their own prose** instead:

> *"`g_taxWidgets` (`0x004DD790`) and `g_rationWidgets` (`0x004DD7C0`), two 24-byte
> records each"*

That address is the thing the check wanted and could not find, sitting in the record, in
a field written for a different purpose by somebody who was not thinking about kinds. So
the check now scans `what` and `note` for addresses that land **exactly on a record base
inside the two regions** — not on a handler, which is the false-positive that made the
first version of this useless — and cross-checks those too.

Three things make it safe rather than clever, and they are worth copying:

* **Record bases only.** A record whose prose mentions a *function* mentions it for a
  hundred reasons; a record whose prose names an address in `0x004DC4D0 … 0x004DE400` is
  naming a widget table and nothing else lives there.
* **Ambiguity is reported, not resolved.** Prose that names two tables of different kinds
  produces a listed `ambiguous`, the same way a handler reachable at two kinds already
  did. A check that guesses is worse than one that declines.
* **The two artefacts have different authors in time.** The prose was written months
  before the gesture field existed, which is exactly the property `docs/agents.md` says a
  duplicated check needs and usually does not have.

It found `tax-and-ration-arrows`, `prompt-fight`, `prompt-decline` and
`info-garrison-widget` — the first of which is a player's bug report.
