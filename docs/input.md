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

| kind | tester | fires on | pressed frame | repeat |
|---|---|---|---|---|
| 1 | `Hotspot_Test` | `g_mouseLeftPressed` — the **down edge** | none | none |
| 2 | `Hotspot_Test` | down edge, **then every 320 ms while held** | none | flat |
| 3 | `Hotspot_Test` | `g_mouseLeftReleased` — the **up edge** | none | none |
| 4 | `Widget_Test` | down edge | `base + 1` for 3 frames, refreshed while held | **accelerating** |
| 5 | `Widget_Test` | **20 frames after the down edge** | `base + 1` for all 20 | none |

These are `docs/arms.json`'s gesture vocabulary — `left-press`, `left-press-held`,
`left-release`, `left-press-repeat`, `left-press-delayed` — and they are the original's
own, not a taxonomy of ours.

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
* **Double click** — `g_mouseLeftDoubleClick`, a *different flag* set from
  `WM_LBUTTONDBLCLK`. Windows sends it **instead of** the second press.
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

## 6. What we reproduce, by kind

The honest denominator, and it is not 151 of 211. That number counts **arms**; this
counts **kinds**, and every arm has both.

| kind | in the original | ours |
|---|---|---|
| `left-press` (hotspot 1) | the majority of the interface | reproduced — `Event::Click` is the down edge |
| `left-release` (hotspot 3, `Ui_OkButtonClicked`) | 26 `Ui_OkButtonClicked` call sites plus 7 kind-3 records | **four arms enumerated, and all four answered on the press until this branch.** Now on `Event::Release` |
| `left-press-repeat` (widget 4) | ~139 records — every spinner in the game | mechanism built (`press::Press`), **wired on one screen** (army division) |
| `left-press-delayed` (widget 5) | ~40 records, including every yes/no gauntlet | mechanism built, **wired nowhere** |
| `left-press-held` (hotspot 2) | one pair, a setup-page scroll | **not built** — the 320 ms constant is written down and nothing uses it |
| the pressed frame (`base + 1`) | every kind-4 and kind-5 widget | **drawn nowhere** |

> **Say the boundary in the same sentence as the number.** *"151 of 211 arms"* is a count
> of things that exist, over the arms somebody has enumerated. Of the **five gesture
> kinds**, we reproduce two outright, have two implemented and barely wired, and have not
> built one. `docs/arms.json`'s new gesture field is what makes that sentence checkable
> instead of an impression.

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
