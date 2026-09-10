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

> **The audit that followed this pilot is in §7 onwards, and it kept the estimate below
> honest by leaving it alone.** The pilot guessed *"perhaps fifteen screens at roughly half
> an hour each"*. The audit covered **45 more**, in six parallel readings of about half an
> hour of wall time each — so the *per-screen* estimate held and the *count* was three times
> low, because the pilot was counting `docs/screens-county.md` §1's rows and the setup page
> alone turned out to be thirteen screens. Read §7 for the method and §11b for the result.

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

---

# The audit proper

*Everything above is the pilot, left exactly as it was written so that its estimate can be
checked against what the audit actually cost. Everything below is the audit that followed
it, over the remaining screens. The campaign map is a separate job with its own tool
(`tools/draws/mapdraws.js`) and is **not** counted here.*

## 7. The unit, stated once, and it is the same unit on both sides

The pilot's §5 asked for **one number per screen: draw calls in the original against draw
calls in ours, both counted by the same script, printed and never typed.**
`tools/draws/screendraws.js` is that script, and this is the rule it implements.

> **One draw call is one call site, in the source text, of a leaf draw primitive.**

A *leaf* is a function that puts a picture, a glyph run or a rectangle on the frame buffer
and takes its subject as an argument. A call to another *painter* is a recursion and is
followed rather than counted. A call inside a loop counts **once**; a call inside a branch
nothing can reach counts **once** and the record says so, because a reachability claim is a
separate finding from a drawing one.

Three things are excluded, and the exclusion is by construction rather than by a hand-kept
list:

* **The campaign map's own drawing.** Every county panel is an inset over the map and its
  painter's *first call* repaints the map beneath it. Following that counts the map's
  drawing once per panel: it turned `Panel_Tax`'s nine real calls into thirty-four. So
  anything reachable from `Screen_DrawCampaign`, `Screen_DrawMenuBar`, `Screen_DrawEndTurn`
  or `Widget_Draw` is the map's, and the walk stops there.
* **`Widget_Draw` itself**, which is shared. But **each widget record is one thing on the
  screen**, so the record counts them separately in `widgets` — and `g_sendSuppliesWidgets`
  is why: eight records, every caller passes six, two buttons that exist and are never
  drawn.
* **The blitters.** `Blit_Unclipped` / `ClippedLeft` / `ClippedRight` are one logical blit
  seen through three clip states, and they are the implementation of every primitive above.
  Counting them multiplies every sheet draw by three.

**Say what your denominator excludes, every time.** An audit keyed on painters reports a
comfortable number, and this one would too if the exclusions were silent.

### `Ui_DrawNumberRight` centres, and `Ui_OkButton` is not a tick

Two primitives were misdescribed everywhere, and both were settled by reading them for this:

* **`Ui_DrawNumberRight` (`0x004030C6`) centres.** It and `Ui_DrawCentred` both end in
  `FUN_004025D7`, which is `Ui_DrawText(s, x + max(0, (width - w) / 2), y, …)`.
  Right-alignment would be `x + width - w`. The name in `symbols.json` was wrong for every
  caller; `Pen::number_centred` is the correct counterpart and is named for the behaviour.
* **`Ui_OkButton` (`0x0040D1BC`) draws `System.pl8` frame `0x33`** — an arrow pointing into
  a small black hole, measured as among the darkest of that sheet's 84 frames. It is **not
  a tick and it is not the word "OK"**, which matters because a dozen of our screens draw
  the literal letters `OK` there.
* And a quirk with teeth, already sitting in the symbol comment and acted on nowhere: it
  **stashes only the last call's position**, so *a screen that draws two OK buttons makes
  only the second one clickable.* `Screen_BattleOutcome` draws **three**,
  `Screen_DiploDialog` three, `Armoury_LoadScreen` and `Panel_JobDetail` two each.

## 8. The half the count cannot see, which is the player's actual complaint

**A screen can reproduce every draw call the original makes and still be entirely
placeholder, and nothing was counting that.** This is the finding the audit did not go
looking for, and it is worth more than the count.

Our draws are one of two kinds:

* **real** — the draw goes through the game's own assets. Every `shell::Pen` method draws
  with `Fntl2_14.pl8` / `Fntl2_22.pl8` through `crates/l2-game/src/shell/font.rs`, and
  every `draw_*` on the chrome or the village art blits a frame out of a `.pl8` the player
  owns.
* **placeholder** — `l2_view::text` is our own hand-authored **5 × 7 bitmap font**, and
  `widget::panel` / `frame` / `button` are our own rectangles.

Neither number is a verdict alone: a placeholder mark is *correct* on `screens/index.rs`,
which is ours on purpose and says so, and on an honest diagnostic like
*"NO ARM_GRID.PL8 - RACKS ARE RECTANGLES"*. The split is the lead; the record's
`literals_ours` is where a deliberate one is defended.

**And the sharpest form of it, which is fully mechanical: an English caption written in our
source where the original fetches an `L2.eng` string.** That is an invention in the
strictest sense — words on a screen the original never puts there — and it splits three
ways once you read the list rather than counting it:

1. **Honest diagnostics of ours.** *"NO MINIMAP"*, *"NOT SIMULATED"*, *"NOT DRAWN"*,
   *"NO COUNTY SELECTED"*. These say the engine is incomplete, which is true. They stay,
   and they are named as ours at the site.
2. **A word where the original draws no word at all.** *"OK"*, *"X"*, *"YES"*, *"NO"*,
   *"CLOSE"*, *"MAX"*, *"ALL"*, *"AUTO"*, *"SPLIT"*, *"DISBAND"*, *"CANCEL"*. In the
   original every one of these is a `Widget_Draw` frame out of `System.pl8` — the tick at
   29, the cross at 31, plus at 68, minus at 66. Drawing the letters is not a wrong string;
   it is text in a place that has none.
3. **Game text we wrote.** *"SELECT A CASTLE TO BUILD"*, *"BOOSTS TAX REVENUES BY %"*,
   *"1 SEASON TO BUILD."*, *"SIEGE PREPARATIONS."*, *"TOTAL MEN"*. These are the real
   defect, and the fix is always available, because the group, the index and the coordinate
   are all literal in the painter.

**The one that says the most is `screens/menu.rs`'s `"LORDS OF THE REALM II"`.** The game's
own title is `L2.eng` group 11 index 0 and it reads **"Lords of the Realm 2"** — a fact
`crates/l2-game/tests/shell.rs` has asserted for weeks. We had the string, we had a test on
it, and we drew a different spelling of it in a font of ours anyway.

### Why this happened, which is more useful than the count

`crates/l2-view/src/text.rs`'s header said, until this audit:

> *"the original's glyphs live in `Font_c2.pl8` … that file is an open question … so the
> interface draws its own letters **until the real font is decoded**."*

**The real font had been decoded.** `shell/font.rs` reads `Fntl2_14.pl8` through the
128-byte character-to-frame table that `Glyph_Draw` (`0x00402A14`) indexes, and the mapping
is self-checking on descenders. `Font_c2.pl8` was never the file the game draws from —
`docs/audit.md` records that `font_c2` does not appear among `Lords2.exe`'s strings at all
and that it shares 103 of its 108 frame records with `Fntl2_9.pl8`, and the RLE puzzle the
header cited as the open question is marked **resolved**.

So the sentence outlived its condition, and every screen written in that window reached for
the 5 × 7 font because a header told it to. That is the general lesson, and it is worth
more than this instance:

> **A document that promises "until X" keeps promising it long after X.** Nothing goes red
> when the condition it names is met, because the condition is in prose. A counted number
> would have moved on the day the real font landed.

## 9. Reachability, which a draw audit needs for the reason the arms audit did

`docs/agents.md` records that *counting arms cannot tell you whether a screen is
reachable*: `0x28` has a live-looking input arm and a live-looking draw arm, and **nothing
in the binary writes that screen id**, so every arm audited on it counted toward a
denominator it should not have been in. A draw audit has the identical hole, and it has it
in **both** directions:

* **In the original** — a painter no `mov byte ptr [g_screenId], imm8` can select is
  drawing nobody sees. The 212-site scan that settled `0x28` settles these too.
* **In ours** — `screens/menu.rs` is a two-item main menu of ours, drawn entirely in the
  5 × 7 font, that **the shipped binary cannot reach**: `crates/l2-game/src/main.rs` boots
  to `ScreenId::Setup(SetupPage::Title)`, the real front end, which `setup.rs` reproduces
  with 41 real draws. `menu.rs` is kept alive only by `tests/machine.rs`. It is an
  invention *and* dead, and it inflates the placeholder count with marks no player will
  ever see.

So every record carries `reachable`, and **`dead` is a distinguishable status rather than
an absence** — the same requirement `docs/arms.json` was given, for the same reason.

## 10. The shape, and it is one shape for both audits

`tools/draws/screens.json` is the inventory, and `tools/draws/mapdraws.js` writes into the
same file. **Two schemas would be worse than either**; amend this one once if it is wrong,
and say why.

```json
{
  "screen": "0x15",
  "name": "The tax panel",
  "painter": "Panel_Tax",
  "addr": "0x0041152F",
  "roots": ["Panel_Tax"],
  "module": "county.rs",
  "original": 11,
  "widgets": { "table": "g_taxWidgets", "records": 2, "drawn": "2" },
  "missing":  ["..."],
  "invented": ["..."],
  "literals_ours": ["NOT SIMULATED - an honest diagnostic, not a caption"],
  "reachable": true,
  "excluded": "the campaign-map repaint beneath the inset (FUN_004050C0)",
  "eng": { "group": 86, "indices": [1, 2, 3, 4] },
  "sheets": [{ "sheet": "Misc_cty.pl8", "frames": [62] }],
  "unexercised": "no fixture has a county at a non-zero tax rate",
  "notes": "..."
}
```

### The one amendment the schema needed, and why

**`original_objects` / `ours_objects`, on the front end only.** The call-site rule breaks
on the thirteen setup pages and it breaks for a reason worth stating rather than papering
over: **the original unrolls where we loop.** `FUN_0041EAA3` writes four recesses out
longhand; our `draw_item` is one helper called four times. By the call-site rule page 1 is
*original 11 / ours 5*, which reads as catastrophic and means nothing.

So those records carry **both** numbers: `original` and `ours` by the call-site rule, and
`original_objects` / `ours_objects` with loops unrolled to their real bounds. The second
pair is what the front end's percentages quote, and the record says which.

This is the amendment, taken once and stated here so that a second one has to argue with
it. It is **not** extended to the other screens, deliberately: an unrolled count is a
judgement about a loop's bounds, and a judgement cannot be recomputed by `--check`. Where
the two rules agree, the checkable one wins.

**`literals_ours` is the complete list, not an allowlist.** Every English caption the
module draws goes in it with a one-line verdict — *"an honest diagnostic"*, or *"an
invention: `Ui_OkButton` draws `System.pl8` frame `0x33`, an arrow into a hole, and no
letters"*. `crates/l2-game/tests/draws.rs` asserts **set equality** between that list and
the module's source, in both directions: a caption nobody wrote a verdict for fails, and a
verdict for a caption somebody deleted fails. The point is not that the list is short; it
is that it cannot grow while nobody is looking, which is exactly what happened while
`text.rs`'s header said the font was undecoded.

`original` is **stored** rather than always recomputed, and that is deliberate. The
decompiled corpus is gitignored and lives only in the checkout that built it, so CI cannot
count it — and the number that matters most is the one CI must be able to check. So it is
written by `--write`, and there are then two places it cannot drift, maintained by
different work:

* `tools/figures/figures.js` keeps every quoted figure equal to `screens.json`, in CI,
  with no corpus at all;
* `screendraws.js --check` recomputes it *from the corpus* and fails on a mismatch for
  anybody who has one — and **skips loudly rather than passing** when the corpus is absent,
  which is the distinction `l2_testkit` draws between *absent* and *wrong game*.

Nobody in this project writes the corpus and `figures.js` never reads it, so this is not
`docs/agents.md`'s *"two artefacts maintained by the same person, at the same time, for the
same reason"* — which is the pattern that lies.

**`ours`, by contrast, is *not* the number the table prints, and that is deliberate.** The
field holds the auditor's own count, made by hand while reading the screen;
`screendraws.js` ignores it and recounts our side from the source on every run. Two
enumerations from different directions, and the disagreement is the point. On the first
pass they agreed exactly on 7 screens of 24 and were within ±2 on fifteen more — and it was
a *systematic* disagreement, five screens all short by one, that turned up **two bugs in
the counting tool**: a scan that read call sites out of the decompiler's own doc comments
and counted them, and a line-by-line scan that could not see a call whose name Ghidra had
wrapped onto its own line. The second under-reported the **denominator**, which is the
direction that flatters. Neither was findable from inside the tool.

**All three clauses of `--check` were ablated and all three go red**: a stored count edited
by hand; a record whose `roots` is empty (an unenumerated screen counts 0 and reads as a
finished one, so that is made unrepresentable rather than checkable); and a `module` a
rename took away.

## 11. Still no `// draw:` markers, and the pilot was right about that

The pilot recommended against set-equality markers on draw calls — ~230 lines against 26
arms, most carrying no decision — and nothing found since changes that. What replaced them
is cheaper and fires on ordinary work:

1. **`screendraws.js --check`**: the stored count against the binary.
2. **`figures.js --check`**: the quoted count against the stored one.
3. **The `L2.eng` check** in `crates/l2-game/tests/shell.rs`: every `(group, index)` a
   screen's named constants declare exists in the player's own `L2.eng`.
4. **The font split**, printed with the count and never typed.

**And the limit on check 3, stated where it lives and repeated here because it is the one
that flatters.** *A check on existence is not a check on meaning.* The armoury was filed
under group 16 and **group 16 index 6 exists** — it is a mercenary nationality — so the
check passes on a screen still filed under the wrong group. What caught that was a person
reading the strings, and there is still no proposal for the second. Every group in this
audit was therefore verified against the *words* rather than against the indices resolving,
and a group that could not be so verified says so at the constant.

## 11a. A **fourth** place drawing hides, and no dispatch table mentions it

The pilot found three: outside the painter, behind a variable widget count, and inside a
ladder nothing can reach. The audit found a fourth and it is the worst of the four, because
the other three are at least reachable by reading *a* table.

**`Menu_RestoreBackdrop` (`0x32`, *a menu-bar drop-down is open*) makes zero draw calls.**
Its whole job is to put back what the drop-down covered. The drop-down itself is drawn by
**`FUN_0040C725`**, whose only caller is the **application frame loop** at `0x004B99C0`,
two lines after `Screen_DrawMenuBar()`, guarded on `g_screenId == 0x32` **inside itself**.

So it is not in `Screen_Draw`, not in `Screen_DrawWidgets`, and not in a widget table.
An enumeration that walked all three dispatch ladders would report screen `0x32` as drawing
nothing at all — and the drop-down is the thing the player is looking at.

The general form, and it is the reason this is a section:

> **The dispatch tables are a map of where drawing is *organised*, not of where it
> happens.** Anything the frame loop calls directly is off that map, and the only way to
> find it is to read the frame loop.

Two more of its callees are worth the same suspicion and are recorded rather than
enumerated: `FUN_00420316` (a multiplayer chat banner) and `FUN_0042476B` (a network-wait
glyph). Both belong to the campaign map's audit.

**And a related correction to `docs/symbols.json`, which named `0x004B99C0`
`Battle_Frame`.** It calls `Screen_Draw`, `Screen_DrawWidgets`, `Screen_DrawMenuBar`,
`CountyStrip_Draw`, `Smk_PlayLoop`, `Msg_Pump` and `Cursor_Set`. It is the **application**
frame loop and the name sent at least one reader past it.

## 11b. What the audit found, screen by screen

**The figures below are a frozen measurement — the state at the end of the first pass — and
must keep their values.** The live ones are in `docs/plan.md` §0, where
`tools/figures/figures.js` maintains them, and

    node tools/draws/screendraws.js

prints them from the tree. Nothing here is typed twice on purpose; this paragraph is a
record of a moment, which is the one thing `figures.js`'s own header says must **not** be
marked.

At the end of the first pass: **51 screens; 1,012 draw calls in the original; 500 in ours.**
Of our 500 marks, **421 go through the game's own artwork** and 79 are our 5 × 7 debug font
and our own rectangles, with **19 English captions written in our source** where the
original fetches an `L2.eng` string. **56** things the original draws are enumerated as
missing and **37** are things we draw that it does not — the figure nobody had, and the half
of 1:1 that an omission audit cannot see.

The screens that are worst, in order:

| screen | original | ours | what it is |
|---|---:|---:|---|
| `0x2B` battle outcome | 13 | 0 | the banner after **every** battle, seven outcomes, not built |
| `0x21` the value spinner | 8 | 0 | reachable from Options ▸ Game Speed and ▸ Scroll Speed, both dead ends |
| `0x20` the standings | 11 | 0 | a four-line bar per realm; the seven scoring rules are undocumented |
| `0x2F` rank sheet | 9 | 0 | one full-screen picture plus nine draws; needs `score.dat` |
| `0x1E` the confirm box | 2 | 0 | **one dialog for fifteen questions** |
| `0x0F` the job popup | 114 | 8 | nine jobs; the single largest gap in the audit |
| `0x04` map information | 174 | 24 | eleven layouts behind a right-click |

**`0x0F` and `0x04` are between them a third of everything missing**, and both are the same
shape: one screen id with a ladder of layouts behind it, of which we draw one or two.

Two of the seven are **cheap and disproportionate**: `0x1E` is two draw calls and it is the
game's only yes/no dialog — fifteen questions route through it — and `0x21` is eight.

## 12. `L2.eng` 31/21 *"Morale"* — the pilot was right, and the `[V]` has to go

The pilot found that `docs/armies.md` rests a **`[V]`** on unit `+0x166` on group 31 index
21 being an army-panel label, and that group 31 has two consumers and neither uses index 21.
That is now settled twice over, and the second way is the one that matters, because the
first would have missed a variable index.

**One — no literal.** Every literal group-31 index in the whole corpus:

```text
Eng_DrawString(0x1F, 2)   (0x1F, 8)   (0x1F, 9)   (0x1F, 0x14)   (0x1F, 0x16)
Ui_DrawCentred(0x1F, 0x11)   (0x1F, 0x12)
```

Seven, and `0x15` is not among them.

**Two — the one variable index is pinned to four values.** `UnitPanel_Draw`
(`0x0041B19D`) has `Eng_DrawString(0x1F, local_20, …)`, and `local_20` is assigned in a
four-arm ladder over `g_units[…].kind`:

```c
kind == 3 -> local_20 = 0;   kind == 2 -> local_20 = 5;
kind == 4 -> local_20 = 2;   kind == 1 -> local_20 = 6;
```

and the call is guarded by `else if (local_20 != 6)`, so the indices that reach `L2.eng`
are **{0, 2, 5}** and nothing else. The wrapped-paragraph draw beside it,
`FUN_0040328E(0x1F, local_1c, …)`, is pinned the same way to **{12, 13, 14, 15, 16}**.

**So nothing in `Lords2.exe` draws group 31 index 21**, and `docs/armies.md`'s `[V]` on unit
`+0x166` has no second source. It must be demoted to `[D]` or resourced from something else
— a field that a panel does not label is not thereby unlabelled, but it is certainly not
*verified by a label*.

### And a second dead thing in the same twenty lines

The same ladder assigns **`local_14`** five times — `0x23`, `0x24`, `0x25`, `0x26`, `0x27`
— and `local_14` is **read nowhere in the function**. Five values, one per unit kind, in
exactly the shape of the two indices beside them that *are* used, going nowhere. Whether
`0x23`…`0x27` are group 31 indices 35–39, or indices into something else entirely, is
**unestablished, and this document declines to guess** — `docs/agents.md` records
`docs/bugs.md` B65 as the good case, where an agent found a number that matched and a story
available for free and refused to build on it. This is the same shape: a plausible story is
available and there is no evidence for it.

What is *verified* is the absence: the local is dead. Recorded so that the next reader of
`UnitPanel_Draw` does not spend the hour again.
