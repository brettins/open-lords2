# The draw-call inventory — a pilot over seven screens

**This is the answer to one question: is enumerating what a screen *shows* as tractable as
enumerating what it *responds to*?**

`docs/arms.json` exists because C61 measured that we reproduced 80 of 185 input arms and
every miss was a behaviour nobody had looked for. Two player reports an hour apart —
*"I see placeholder shit everywhere"* and *"why do the pastures not have cows in them?"* —
are the same measurement from the other side, and **no check we have can see either.** We
count input arms and struct fields. Nothing counts pictures.

This file is what fell out of building seven screens with that question held open. It is
**deliberately not a schema**. `docs/arms.json`'s shape was settled by an agent with fifty
records blocked on it; nothing here is blocked on anything, and proposing a JSON file before
knowing whether the enumeration works would be the same mistake in a new place.

## 1. The finding: it is tractable, and it is cheaper than the arms audit was

**Per screen, reading a painter's draw calls out of the decompilation cost about as much as
reading its input arms, and produced more.** Concretely, on the seven:

| screen | painter | draw calls | how long | what the enumeration found |
|---|---|---:|---|---|
| `0x25` About | `Screen_About` | 7 | minutes | the screen was **already complete**; the shell's apology was a measurement |
| `0x09` The court | `Court_Draw` | 26 | ~20 min | four lines drawn in the **wrong font** by our table; one realm field nobody had |
| `0x0B` Diplomacy | `Diplo_DrawScreen` + card | 31 | ~30 min | a **60-row thermometer** nobody knew was there; four menu layouts |
| `0x18` Send supplies | two functions | 20 | ~25 min | the numbers are drawn **outside the painter**; a complete cut **sheep row** |
| `0x2E` Ratings | `Screen_BattleMasterRatings` | 46 | ~30 min | the shape was wrong: **7 × 3 twice**, not 7 rows; and the **scoring rule**, undocumented anywhere |
| `0x04` Map info | two painters, eleven layouts | ~90 | ~60 min | three false claims in our own table; **`L2.eng` 31/21 is dead text and a `[V]` rested on it** |
| pasture cattle | `Sprite_TopIt` farm arm | 12 | ~40 min | **the cows**, and that the picture is a rule |

Total: about four hours of agent time across five parallel readers, ~230 draw calls, seven
screens. The arms audit was *"three agents about half an hour of wall time each"* for ~185
rows. So the cost is comparable and the yield is higher — because **a draw call carries its
own arguments**. An input arm tells you a gesture exists; a draw call tells you the group,
the index, the frame, the coordinate *and* the state it reads, and every one of those is
independently checkable against the player's own data files.

## 2. What it found that no other check could have

Seven things, and each is a different shape of failure:

1. **A screen we thought was a placeholder was finished.** `0x25`'s painter is 171 bytes.
2. **A screen was in the index twice.** `0x0F` sat in the shell table beside the
   `screens/job.rs` that already owned it. The only check dedupped *within* the table.
3. **Our own table asserted something false about a painter.** `0x04`'s row said
   `FUN_0041B032` draws no `Ui_DrawBox`; both its halves open with one.
4. **A `[V]` rested on a string nothing draws.** `docs/armies.md` cites `L2.eng` 31/21
   *"Morale"* as an army-panel label and rests the verification of unit `+0x166` on it.
   Group 31 has exactly two consumers and **neither ever uses index 21.**
5. **A widget is drawn 192 pixels from where we draw it.** `map.rs`'s field brush.
6. **A rule was hiding inside a graphic.** The pasture herd.
7. **A whole formula was undocumented.** Battle Master scoring, `FUN_0042C64F`.

Note the direction: **five of the seven are things we draw *wrongly* or *claim* wrongly**,
not things we fail to draw. A "missing pictures" audit would have found two of them.

## 3. The three places drawing hides, and they are the same three the arms audit found

The coordinator's warning about input — *the keyboard is in the window procedure, the hover
is in the draw step, the blacksmith's hotspots exist only for one value of a variable* — has
an exact counterpart here. **An enumeration keyed on "read `Screen_Draw`'s arm" would miss
all three.**

**One — drawing outside the painter.** `0x18`'s *numbers* are not in `Screen_SendSupplies`
at all. They are in `FUN_0041AEA2`, called every frame from `Screen_DrawWidgets`, which is
why they update without the frame being repainted. Read only the painter and you get a
window with two empty rows. Same shape: `0x0F`'s blacksmith fire animation is in
`Screen_DrawWidgets`; the court's and diplomacy's buttons are drawn by `Widget_Draw` from
the same place and appear nowhere in either painter.

**Two — drawing behind a variable count.** `g_diploWidgets` has six records and
`Widget_Draw` is passed `g_diploWidgetCount`, which the *painter's prologue* sets to 0, 3, 4
or 6. `g_sendSuppliesWidgets` has **eight** records and every caller passes 6 — so two
buttons exist, are never drawn, and are only findable by reading the table rather than the
call. That is the blacksmith hotspot exactly.

**Three — drawing inside a ladder whose arms are unreachable.** `Sprite_TopIt`'s farm arm
has six frame slots and three of them cannot be reached, because nothing writes the terrain
values that select them. Counting slots gives six; counting *reachable* pictures gives three.
This is `docs/agents.md`'s *"counting arms cannot tell you whether a screen is reachable"*,
one level down.

**So the pilot's answer to the falsification question is: yes, and only if the unit of
enumeration is the *sheet-and-frame reference*, not the painter.** An audit that walks
painters reports a comfortable number.

## 4. What it would cost across all of them

There are 29 screens in `docs/screens-county.md` §1. Seven are done here, and the campaign
map, the village, the county panels, the battlefield and the setup pages carry their layouts
in module headers already — so the honest remaining figure is **perhaps fifteen screens at
roughly half an hour each, plus the campaign map, which is worth a day on its own** because
its sidebar is redrawn by six different functions.

Two costs beyond that, and they are the real ones:

* **The `L2.eng` and sheet-frame checks are cheap and worth doing first.** *Every group and
  index this engine draws exists in the player's own `L2.eng`* is a check that runs today —
  `crates/l2-game/tests/shell.rs` does it for these seven — and it is the one that would
  have caught the armoury's group 16. Likewise *every frame index we pass exists in the sheet
  we pass it to*. Neither needs an inventory file; both need only the constants to be named
  rather than inline, which is already the house style.
* **A `// draw:` marker set-equality check, the way `// arm:` works, is the expensive half
  and I do not recommend starting there.** Markers on input arms work because an arm is a
  handler — one place, one function. A draw is a *line*, there are ~230 of them across seven
  screens against 26 arms, and most carry no decision. Marking them all would be a day of
  typing for a ratio of noise to signal an order of magnitude worse than `arms.json`'s.

## 5. What I would build instead, if this becomes an audit

**The unit that pays is the screen, not the call.** Each of the seven modules here now opens
with its painter as a literal listing, address by address, with the coordinates resolved.
That listing is the inventory, it lives next to the code it describes, and it went stale
zero times because writing it *is* how the screen got built.

So the mechanical check I would add is the cheap half of §4 plus **one number**: per screen,
*draw calls in the listing* against *draw calls in our painter*, both counted by the same
script, printed by the census and never typed. `tools/figures/figures.js` already does
exactly this for other counts and the precedent there — *"a number that cannot drift beats a
number that is checked"* — applies unchanged. A screen whose ratio falls is a screen somebody
simplified.

**And one thing that is not mechanical and is worth more than the check: the listing has to
be written from the decompilation and not from the running game.** Every one of the seven
findings in §2 came from reading what the original *does*, then noticing our version differs.
None of them would have come from screenshotting both and diffing, because five of the seven
are wrong in ways that look plausible.

## 6. What this pilot could not settle

* **Whether the ratio in §5 is stable enough to be a gate.** Seven screens is not enough to
  know what a normal ratio is, and a gate set from seven samples is a gate set from the
  status quo — `docs/agents.md`'s third accidental-pass instance exactly.
* **The campaign map.** It is the screen the player looks at for 95% of a session, it has
  the most drawing by far, and it was not enumerated here. Any cost estimate that does not
  include it is an estimate of the easy part.
  **Done: `docs/draws-map.md`**, in this file's §5 shape — a listing beside the code with the
  number derived by `tools/draws/mapdraws.js` rather than typed. **139 draw calls, 121 live,
  59 reproduced**, against 20 of 25 input arms on the same two screen ids: *we answer four
  gestures in five and draw one picture in two.* The day it cost was the day estimated.
* **Whether the `L2.eng` check would have caught the armoury's group 16 in practice.** It
  would have caught the *index* being absent. Group 16 index 6 exists — it is a mercenary
  nationality — so the check passes and the screen is still filed under the wrong group. The
  thing that caught that was a person reading the strings. **A check on existence is not a
  check on meaning**, and I have no proposal for the second.
