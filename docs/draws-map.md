# The campaign map's draw calls, enumerated

`docs/draws.md` is the pilot over seven screens. It ends by saying the campaign map *"was
not enumerated here … any cost estimate that does not include it is an estimate of the easy
part."* This is that enumeration, in the shape §5 of that document asks for: **the listing
is the inventory, and the number beside it is derived by a script rather than typed.**

```bash
node tools/draws/mapdraws.js           # the table below
node tools/draws/mapdraws.js --sites   # every call site, one line each
```

---

## 0. The headline

> **The campaign map makes 139 draw calls. 121 of them are live, 10 are behind debug
> switches and 8 are dead code. We reproduce 59 of the 121, and we make about 29 draws the
> original does not.**

That is **49 %**. `docs/arms.json` holds **25** input arms for the same two screen ids
(`0x00` and `0x10`) and marks **20** of them reproduced — **80 %**. So on the screen a player
spends most of the game looking at:

> **We answer four gestures in five and we draw one picture in two.**

That gap is the whole argument for this audit existing. It is not visible from the input
side, it is not visible from any test, and it is the shape both of the player's reports —
*"I see placeholder shit everywhere"* and *"why do the pastures not have cows in them?"* —
were about. Neither of those is an arm.

The misses are not evenly spread. Three areas hold 48 of the 62, and each is one of
`docs/draws.md` §3's three hiding places:

| area | live | ours | missing | which hiding place |
|---|---:|---:|---:|---|
| `Sprite_TopIt`'s tile overlays | 18 | 4 | **14** | a ladder whose arms nobody enumerated |
| the sidebar's produce and industry rows | 31 | 10 | **21** | a variable widget count |
| drawn from `Battle_Frame`, not from a painter | 16 | 3 | **13** | outside the painter |
| everything else | 56 | 42 | 14 | — |

---

## 1. What a draw call is here, and what the denominator excludes

**A draw call is one static site that puts a picture, a glyph run or a rectangle on the
frame buffer, counted once per distinct (sheet, frame expression, position) in the source.**
Deliberately excluded, and each exclusion is a decision somebody could disagree with:

* **The five unrolled tile blitters.** `FUN_00452820`, `FUN_0045337B`, `FUN_004538F6` and
  their zoom-2 twins are one diamond seen through five clip modes. Counted as **one** draw.
  Counting them out would multiply the terrain by ten and make this screen incomparable
  with the other seven.
* **The three clip-state tails.** `g_clipState` picking between `FUN_004B4537`,
  `FUN_004B4814` and `FUN_004B48B8` is the same blit clipped. One draw.
* **`Map_DrawTileApex`'s four shape arms**, for the same reason. One draw.
* **Everything dynamic.** The map draws ~250 tiles a frame across three lattice walks; this
  counts the *sites*, not the blits. A per-frame count would be ~800 and would say nothing
  about coverage.
* **Screens that are drawn over the map but are their own screen.** `Msg_Pump`'s message
  scroll, `Tip_Update`'s tip page, `Screen_DrawWidgets`' widget layer and the battle
  overview. The drop-down menu **is** counted (`FUN_0040C725`), because it is drawn from
  `Battle_Frame` on top of the live map rather than replacing it, and because the map's own
  module already owns the three titles under it.
* **The palette.** `Palette_Set(Base01.256)` and `FUN_004B0CB4`'s sixteen-step fade change
  every pixel on the screen and are not a draw call in this sense.

---

## 2. The listing

Status: **live** = reachable in a shipped game; **debug** = behind a switch only the command
dispatcher sets; **dead** = no caller, or an unreachable zoom.

| function | addr | paints | status | draws | ours |
|---|---|---|---|---:|---:|
| `Screen_DrawCampaign` | `0x0040F5FD` | the full repaint | live | 8 | 4 |
| `Map_DrawTile` | `0x004063C1` | the terrain diamond | live | 1 | 1 |
| `Map_DrawTileApex` | `0x00406673` | its overhang | live | 1 | 1 |
| `Map_DrawSurroundTile` | `0x0042A7F1` | an off-map tile | live | 1 | 1 |
| `FUN_0042A8D1` | `0x0042A8D1` | surround, left half | live | 1 | 1 |
| `FUN_0042A9AB` | `0x0042A9AB` | surround, right half | live | 1 | 1 |
| `FUN_00405602` | `0x00405602` | a zoom-1 edge fill | live¹ | 1 | 0 |
| **`Sprite_TopIt`** | `0x004071A0` | **six tile overlays** | live | **18** | **4** |
| `FUN_00407F82` | `0x00407F82` | the besieger's banner and its count | live | 2 | 0 |
| `Map_DrawPathMarker` | `0x004081A6` | a path ball | live | 2 | 1 |
| `Map_DrawArmies` | `0x00408438` | a unit, and its banner | live | 3 | 1 |
| `Screen_DrawMenuBar` | `0x00419C78` | the 640 × 24 bar | live | 8 | 4 |
| `CountyStrip_Draw` | `0x0040F7D3` | the county strip | live | 18 | 18 |
| `FUN_004100AF` | `0x004100AF` | strip row: cattle | live | 5 | 4 |
| `FUN_0041023A` | `0x0041023A` | strip row: grain | live | 5 | 4 |
| `FUN_004103C5` | `0x004103C5` | strip row: reclamation | live | 4 | 2 |
| `FUN_00410502` | `0x00410502` | strip row: stone | live | 2 | 0 |
| `FUN_00410598` | `0x00410598` | strip row: wood | live | 2 | 0 |
| `FUN_0041062E` | `0x0041062E` | strip row: iron | live | 2 | 0 |
| `FUN_004106C4` | `0x004106C4` | strip row: weapons | live | 3 | 0 |
| `CountyStrip_DrawCastleIcon` | `0x004107D1` | strip row: castle | live | 8 | 0 |
| `Minimap_Draw` | `0x00410AA9` | the minimap plate | live | 6 | 6 |
| `Minimap_DrawOverlay` | `0x00410CBD` | its county tint | live | 1 | 1 |
| `Screen_DrawEndTurn` | `0x0041A734` | the End Turn strip | live | 2 | 2 |
| `FUN_0040C725` | `0x0040C725` | an open drop-down | live | 4 | 3 |
| `FUN_00420316` | `0x00420316` | the multiplayer chat banner | live | 3 | 0 |
| `FUN_0042476B` | `0x0042476B` | the network-wait glyph | live | 1 | 0 |
| `FUN_0041A639` | `0x0041A639` | the turn timer | live | 2 | 0 |
| `FUN_0041A844` | `0x0041A844` | the multiplayer heartbeat | live | 2 | 0 |
| **`FUN_00476E95`** | `0x00476E95` | **the tooltip** | live | 4 | 0 |
| `FUN_00408C50` | `0x00408C50` | two per-tile debug numbers | debug | 2 | 0 |
| `FUN_004248A3` | `0x004248A3` | a four-number debug box | debug | 5 | 0 |
| `FUN_004247F5` | `0x004247F5` | `DUMB VIEW ON` | debug | 3 | 0 |
| `FUN_00406BBA` | `0x00406BBA` | a second tall-tile pass | dead | 1 | — |
| `FUN_0041424A` | `0x0041424A` | zoom-1 left edge | dead | 5 | — |
| `FUN_0041432B` | `0x0041432B` | zoom-2 left edge | dead | 2 | — |

¹ `FUN_00405602`'s single fill is inside `if (g_mapZoom == 1)`, which `docs/screens.md` §2.2
proves unreachable. **120 live and 9 dead** is the tighter reading; the script cannot see
inside an arm, so the table's 121/8 is the mechanical one and this note is the correction.

The nine row-walker functions (`Map_RenderIso`, `Map_RenderAlignedRow`,
`Map_RenderOffsetRow`, `FUN_00405487`, `FUN_00405862`, `FUN_004059AF`, `FUN_00405EB5`,
`FUN_00405FAC`, `Map_DrawFrame`) draw nothing of their own and are omitted from the table;
they are in the script's tree so that adding a draw to one of them would show up.

---

## 3. `Sprite_TopIt` dispatches six things, not four, and its name is the game's own

`docs/formats/maps-layers.md` §5.5a says the name *"reads as one blitter, and the function is
a four-way dispatcher … Something like `Map_DrawTileOverlay` would carry that. Flagged rather
than renamed."*

**Do not rename it.** `node tools/oracle/logstrings.js` shows this function's three
`Log_Write` literals are `"ERR:top_it no data "` and `"ERR:top_it bad data "` twice —
**`top_it` is what the original's own authors called it**, in the same convention that gave
us `write_sprite`, `gen_frame`, `mos_frame` and `mos_blank` (`docs/decisions.md` C72).
Replacing a name recovered from the binary with an invented one that describes it better is
the one trade this project has said it will not make. The right fix is the docstring, and
this section is it.

**The arms, in the order they are tested.** The dispatch is on **plane 0** (`tile.flags`),
after two gates:

```c
if (g_optExploration == 1 && (tile.bank & 0x20) == 0) return;   /* unexplored */
if ((tile.flags & 0xF0) == 0)                          return;   /* nothing here */
```

| # | tested | condition | draws |
|---|---|---|---|
| 1 | `flags & 0x40`, `part == 0` | `county.shield != 0` | the town's owner banner, `Flags1a` `(shield−1)*8 + phase`, at `(+0x1A, −0x1C)` |
| 2 | `flags & 0x40`, `part == 2` | `county.mercenaryOffer != 0` | the mercenary marker, frame `0x81`, at `(+0x10, −0x12)` |
| — | `flags & 0x40`, `part 1` or `3` | — | **nothing.** Two of the town's four quadrants are silent |
| 3 | `flags & 0x10` | `content == 0x13` | the damage animation, `0x28 + counter`, at `(+0x0C, −0x0E)` |
| 4 | `flags & 0x20` | `0x14 ≤ content ≤ 0x16` | the pasture herd, `0x55 + (content−0x14)*6 + phase`, at `(+4, −4)` |
| 4b | `flags & 0x20` | `0x10 ≤ content ≤ 0x12` | the dead half, `0x67 + …`, at `(0, 0)` — nothing writes those values |
| 5 | `flags & 0x80`, `content ≤ 0x0C` | idle (`1/4/7/10`) | **nothing** |
| 5b | " | working (`2/5/8/11`) | **no overlay at all — it rewrites the tile's own frame** |
| 5c | " | wrecked (`3/6/9/12`) and `disabledSeasons ≥ 3` | the damage animation, `0x28 + counter` |
| 6 | `flags & 0x80`, `0x15 ≤ content ≤ 0x19` | `county.garrisonUnit != 0` | the garrison's banner, `(shield−1)*8 + phase`; and first, if that unit is besieged, `FUN_00407F82` for the besieger's |

Six arms, four of which have sub-arms, and **fourteen of the eighteen frame selections are
not drawn by us.**

### 3.1 Arm 5b is a rule, and the rule is the animation's *speed*

An industry site that is *working* does not get an overlay. It steps its own terrain frame:

```c
n = county.industry[k].total - county.industry[k].totalSnapshot;   /* this season's output */
if      (n < 0x0A) step = DAT_0058FD08;
else if (n < 0x19) step = DAT_0057D3C8;
else if (n < 0x32) step = g_pulse160;
else               step = g_pulse80;
if (step) { tile.frame++; if (tile.frame > last) tile.frame = first; }
```

**The busier the mine, the faster its wheel turns.** Four output bands, four pulse rates,
and the picture is the only place the player is told. This is `docs/draws.md` §2.6's *"a rule
was hiding inside a graphic"* a second time, on a different mechanic, found the same way.

The four frame runs, read out of the four arms, and every one lands inside `Town1a.pl8`'s 61
frames — an independent check on `maps-layers.md` §2's *"0 stone, 20 wood, 30 iron"*:

| industry | idle | working | wrecked | `county.industry[]` |
|---|---:|---|---:|---|
| stone | 0 | 1 … 4 | 9 | `[3]` |
| weapons | — | 10 … 18 | 19 | `[2]` |
| wood | 20 | 20 … 28 | 29 | `[0]` |
| iron | 30 | 30 … 45 | 46 | `[1]` |

### 3.2 Frames `0x28 … 0x37` are damage, not "industry shut-down"

`docs/screens.md` §7 calls this block *"`FUN_004071A0`'s industry-shut-down marker"*. It is
that **and** the marker over a razed dwelling, and the pair is what identifies it.

* Arm 3 draws it when a `0x10` dwelling plot has `content == 0x13`.
  `County_UpdateDwellings` (`0x004684C6`) never *leaves* a live dwelling at `0x13` — every
  branch steps it up to `0x12` or beyond — and **`Unit_BurnDwelling` (`0x00468AE2`) is the
  only writer that puts `0x13` back**, on a plot that was `0x10`, quartering the county's
  population and calling `Sound_RestartSlot(3)`. **[V]**
* Arm 5c draws the same sixteen frames over an industry site trampled to its wrecked value
  and shut down for three seasons or more. **[V]**

So the block means *something here was destroyed*. Measured against the player's own
`Flags1a.pl8`, the sixteen frames are 32 × 24 with warm (flame-coloured) pixel counts rising
608 → 226 at frame `0x2E` and then falling while grey rises to **344 by `0x37`** — fire
burning down to smoke. **[I]** as to what the picture *is*; **[V]** that it is one 16-frame
block shared by both events.

`DAT_0053E9A4` is stepped 0 … 15 by `FUN_00451F1A` on `g_pulse80`, from `Battle_Frame`, and
wraps. It **never exceeds 15**, so the `if (0xF < counter) return` guard inside all five call
sites is dead and the animation loops for ever. A burnt dwelling burns until the season
raises it again.

---

## 4. The cattle band mismatch is **not** an out-of-bounds

`docs/decisions.md` C77 records that the herd graphic has three bands (11, 21) where the
crowding *meter* has four (11, 21, 31). The question asked of this audit was whether that is
a live overflow. **It is not, and the fit is exact:**

```
content   = 0x14 … 0x16          (Herd_UpdateCrowding, guarded: `if (2 < (byte)(content - 0x14)) return`)
phase     = DAT_0057D388 >> 4,   DAT_0057D388 wrapped at 0x60  ->  0 … 5
frame     = (content - 0x14) * 6 + phase + 0x55
max frame = 2 * 6 + 5 + 0x55 = 0x66
```

`Flags1a.pl8` frames `0x55 … 0x66` are exactly eighteen frames. The ladder **saturates**
rather than overflowing: a herd of 25 per field and a herd of 45 per field are the same
picture. The mismatch is a loss of information in the graphic, not a fault.

The dead half is equally safe: `0x10 … 0x12` maps to `0x67 … 0x78`, which exist (as 2 × 2
stubs).

**Where an unclamped index does exist on this screen**, so that it is not confused with the
above:

* **`Map_DrawPathMarker`: `frame = 0x38 + cost`, with no bound.** `cost` is the flood fill's
  distance minus one, clamped to 0 only when it exceeds the unit's *remaining* allowance —
  so an army with more than 22 moves left walks the index past `0x4E` and off the end of the
  ramp. Ours clamps at `0x4D`; that divergence is already recorded at
  `campaign::path_marker_frame`.
* **`Sprite_TopIt` arm 6 does not guard the garrison's shield.** Arm 1 tests
  `county.shield == 0` and returns; arm 6 tests only `garrisonUnit == 0` and then computes
  `shield * 8 - 8 + phase`, which for `shield == 0, phase == 0` is **−8**. The frame record
  read then fails the `dataOffset < 1` check, writes `"ERR:top_it no data"` and sets
  `g_quitRequest = 1`. `docs/screens.md` §5.1 says *"`shield` is a realm's `shieldIndex`,
  clamped 1 … 5, so a zero shield flies nothing"* — that is true of the **town** and false of
  the **castle**. **CNEW-garrison-shield.** No shipped save on this machine has a
  zero-shield garrison, so this is `[D]`, not an observed crash.

---

## 5. Nine things the audit found that nothing else could have

### 5.1 The original has a generic tooltip layer, and it covers this sidebar

`FUN_00476E95` runs every frame from `Battle_Frame`, gated on `g_optToolTips`. It saves the backdrop it will cover, waits **one second** of `timeGetTime` with the mouse still, resolves a
hotspot id through a per-screen table `DAT_004D6FB8[g_screenId]`, and draws `L2.eng` group
220 index *id* in a box beside the cursor, flipping side at x 321 and y 241.

`FUN_00477320` is the campaign map's resolver — **1,082 bytes**, and it partitions the whole
sidebar. Twenty-four of group 220's thirty-five strings are reachable from it, and they are
a **self-written description of a screen we have been reverse-engineering by hand**:

```
 1 Kingdom view. Click on a county.        11 Create an army
 2 Labour, red if needed, purple if idle.  12 Go to treasury
 3 Ration status                           13 Send supplies
 4 Overall happiness                       14 End your turn
 5 Overview map                            15 Cattle, and change next season
 6 View population report                  16 Wheat, and change next season
 7 View happiness report                   17 Seasons left to reclaim a field
 8 Set tax rate                            18/19/20 Wood/Stone/Iron produced next season
 9 Set rations                             21 Weapons produced. Click for smithy.
10 Adjust labor allocation                 22 Seasons left to build castle
31 Return census map to empire mode        32 Build or update a castle
33 Diplomatic initiatives                  34 The health of the county
```

Ids 11, 12, 13, 32 and 33 are the **five sidebar buttons in order**, and they confirm
`map.rs`'s `SIDEBAR_BUTTONS` table from a completely independent direction — `ARMY`,
`COURT`(treasury), `SUPPLY`, `CASTLE`, `LORDS`(diplomacy). Id 15/16/17 are the three farm
rows in `FUN_0040FEC1`'s order; 18–22 are the five industry rows.

Two documents are wrong about this and one is right. `docs/formats/eng.md` §5 has it exactly
(*"the 35 tool tips, index = hotspot id; drawn by the tooltip layer when `g_optToolTips` is
on"*, `FUN_00476E95`, `[V]`). `docs/screens.md` §7 says **"this engine has no generic tooltip
mechanism"** in a paragraph about the original's merchant plaque, which reads as a claim
about `Lords2.exe` and is false of it. And `map.rs`'s `minimap_mode_name` says *"the original
labels them only with the button icons and the badge, and `L2.eng` has no strings for them"*
— group 220 indices 2, 3 and 4 are exactly those labels. **CNEW-tooltips.**

### 5.2 Fog of war gates six of the map's draw functions, and bank bit `0x20` is *explored*

`maps-layers.md` §5.3 lists the runtime tile record's `bank` byte as *"`plane1` in bits
`0x1C`; bits `0x01`, `0x20`, `0x40`, `0x80` set at run time"* and says what three of them
are. **`0x20` is `explored`:**

* `FUN_0046E067(x, y, w, h)` sets it over a rectangle, called from `Army_Create`,
  `Unit_Step` and `FUN_0046DFD5`; `FUN_00469370` sets it on a county's field tiles when the
  county is the local player's.
* `g_optExploration` is a real, toggleable advanced option — `Opt_ToggleExploration`
  (`FUN_00447E0B` arm 3), reachable from `Screen_AdvancedOptions`, defaulted by
  `Options_SetDefaults`, committed by `Setup_CommitOptions` and carried in the save.
* Six draw functions test the pair: `Map_DrawTile` (draws **base bank frame 0** instead),
  `Map_DrawTileApex` (draws **nothing**, so an unexplored tile is flat), `Map_RenderIso`,
  `Map_RenderAlignedRow` and `Map_RenderOffsetRow` (the off-map surround becomes frame 0
  too), `Map_DrawArmies` (a unit in the dark is not drawn) and `Sprite_TopIt`.

`l2_kingdom::Options::exploration` is stored and honoured by nothing, which
`docs/mechanics.md` already carries as a gap. What is new is that **the whole of its
behaviour is drawing**, and it is five call sites plus one bit.

### 5.3 Every army carries a banner, and every peasant mob carries a different one

`Map_DrawArmies` blits the figure and then, without leaving the loop:

```c
if (unit.kind == 1 && unit.shield != 0)   frame = (shield-1)*8 + phase, at (+0x12, -0x15)
else if (unit.kind == 2)                  frame = 0x79 + phase,          at (+0x12, -0x12)
```

both from `g_flagsSheet`, relative to the sprite's own top-left after `x -= w/2; y -= h`. At
the far zoom the offsets are `(+0x18, −0x15)` and `(+4, −4)`. A merchant (kind 3) and a
transport (kind 4) get none.

So **`Flags1a.pl8` frames `0x79 … 0x80` are the peasant mob's eight-phase banner** — the
block immediately after the 2 × 2 stubs `maps-layers.md` §5.5a measured and left unnamed. And
an army's owner is legible on the map without clicking it. `docs/screens.md` §5.2 describes
the sprite, the anchor, the per-kind nudge and the walk tables and **says nothing about
either banner**; neither does §7's list of what we leave out. **CNEW-unit-banner.**

### 5.4 The far-zoom box has words in it, and we have not been drawing them

`docs/screens.md` §7 says of the far zoom's `Ui_DrawBox(0, 412, 30, 4)` that *"the box is the
original's; the words in it are ours."* The words are the original's too — four draws, at
literal coordinates:

```c
Eng_DrawString(101, g_scenarioIndex, 0x40, 0x1A8, heading);   /* the map's name */
Eng_DrawString(34, 0, penAdvance + 0x50, 0x1A8, heading);     /* "Year"         */
Ui_DrawYear(g_year, penAdvance + 0x60, 0x1A8, 1);
Eng_DrawString(34, 1, 0x50, 0x1C6, body);   /* "Click on the county you wish to view." */
```

So the far view reads **`England   Year 1268`** over **`Click on the county you wish to
view.`** — which is also the game telling us, in its own words, what the far zoom is *for*,
and it agrees with `Map_Click` doing nothing at zoom 2 while `FUN_004350A1` zooms in on the
clicked tile. **CNEW-far-zoom-caption.**

### 5.5 The sidebar's produce rows are eight painters behind two variable counts

This is `docs/draws.md` §3's second hiding place, at its worst on this screen.
`FUN_0040FEC1` fills two little arrays and two counts:

```
left  list (DAT_0053F690, count DAT_00553C60, 0..3):  cattle | grain | reclamation
right list (DAT_00553FD0, count DAT_005440B4, 0..5):  iron | stone | wood | weapons | castle
row pitch:  left  0x3C if count < 3 else 0x2D
            right 0x3C if count < 3, 0x2D if < 4, else 0x1E
```

`CountyStrip_Draw` then walks both, dispatching to one of eight painters. **Read only
`CountyStrip_Draw` and the strip has eighteen draws; follow the two loops and it has
forty-nine.** The row *pitch* being a function of the count is the same shape as
`g_diploWidgets`' variable count, and the same reason a hit test written from the painter is
wrong: `FUN_00477320` divides `(mouseY − 0x130)` by that pitch to find the row.

We draw the left list (`county::draw_produce_rows`, and the blue idle ring in it) and none of
the right one — a decision already recorded there, resting on two county bytes nobody has
named. The **eight `Ui_DrawDelta` calls** — every row's change since last season — are absent
on both sides, for the same reason.

### 5.6 Three widgets are drawn over the map from `Battle_Frame` and are in no document

None of these is in `Screen_DrawCampaign`; all three are per-frame calls from the whole-game
frame function, which is `docs/draws.md` §3's first hiding place.

| what | function | draws |
|---|---|---|
| **the turn timer** | `FUN_0041A639` | `Misc_cty` frame `0x60` at **(404, 430)** plus a right-anchored count at (424, 442), when `g_optTimeLimit > 0` and `DAT_004D2E80[g_screenId] == 0` |
| **the multiplayer chat banner** | `FUN_00420316` | a shaded 480 × 32 plate at (0, 24) and two lines in the two realms' own colours, on a countdown that ends by forcing a full map redraw |
| **the network-wait glyph** | `FUN_0042476B(10, 0x1E)` | `Panels.pl8` frame **`0x105`** — the file's one 48 × 48 glyph — at (10, 30) while a peer is behind, for at most 1,600 ms |
| **the heartbeat** | `FUN_0041A844` | a 8 × 3 filled bar at (2, 476) and a 10 × 5 outline at (1, 475), recoloured every twelve frames from the tick counter |

The turn timer sits **inside the map viewport**, 74 pixels left of the sidebar, which is the
only thing on this screen that does.

### 5.7 The surround is not redrawn unless it is water

`Map_RenderAlignedRow` and `Map_RenderOffsetRow` skip the off-map tile entirely unless
`g_mapRedraw != 0` **or** the surround byte is greater than 6:

```c
if (g_mapRedraw != 0 || 6 < DAT_005C9288) Map_DrawSurroundTile(...);
```

Stored `0x06` is grass and `0x16` is water (`maps-layers.md` §4.1), so **land surround is
painted once per full repaint and sea is painted every frame** — because the sea has a
cycling variant and the grass does not. `Map_RenderIso`'s own first and last rows have no
such test. Ours repaints everything; the difference is invisible and the reason is worth
knowing before anybody optimises.

### 5.8 The menu bar refuses to redraw unless the treasury moved

`Screen_DrawMenuBar`'s whole body is inside

```c
if (<screen is one of the map-like ids> && (g_realms[local].gold != DAT_004E59C4 || DAT_0056D6A0 != 0))
```

so the bar — banners, titles, year, season and gold — is repainted only when the local
realm's gold changes or a flag is set. It is why the year can be a frame late after a season
turn. And the banner loop's test is `strength != 0 && aiStep < 999`, so a **defeated** realm
loses its banner mid-game.

### 5.9 A whole tile pass is dead

`FUN_00406BBA` (1,500 bytes) is a second `Map_DrawTile` that skips the `base` and `roads`
banks and draws only mountains, towns and castles — the tall things. Its two row walkers,
`FUN_0040619D` and `FUN_004062FB`, have **no callers at all**, and neither does it. It
honours `g_optExploration`, so it post-dates the fog. `[I]` a scrapped
"repaint the tall stuff over the units" pass.

---

## 6. What could not be exercised, and why

`docs/agents.md` C26: every shipped fixture is turn one with every realm holding one county.
That leaves these arms enumerated but not observed, and I have not inferred past them:

* **the burning dwelling** (arm 3) — needs an enemy army to have razed a `0x10` plot;
* **the wrecked-industry animation** (arm 5c) — needs `Unit_TrampleTile` *and* three seasons
  of `disabledSeasons`;
* **the working-industry animation rates** (arm 5b) — needs four different output bands in
  one county over time;
* **the besieger's banner and its count** (`FUN_00407F82`) — needs a live siege on the
  campaign map, which the battle triple does not carry;
* **a garrison whose shield differs from the county's** — the case §4's missing guard is
  about;
* **the multiplayer four** (chat, heartbeat, wait glyph, turn timer) — need a second peer;
* **fog of war** — needs `g_optExploration` set at new-game time;
* **the far-zoom caption** — reachable, and the only one of these that is cheap.

---

## 7. What we draw that the original does not

About **29** sites, hand-counted from the `OURS`-marked code in `screens/map.rs`,
`screens/county.rs` and `screens/menubar.rs`, excluding the eight fallbacks that appear only
when the artwork is missing:

the county outline; the county marker squares (2); the field markers (2); the far-zoom status
line; `TURN n`; `COUNTIES n/m`; the clock and the treasury in our own font (2); the
`INDUSTRY / NOT DRAWN` box (3); the sidebar status line; the sidebar focus frame and its name
(2); the unit banner panel and its two lines (3); the field brush popup (5); the county
panel's focus outline; the drop-down's recess and its status line (2); `NO MINIMAP`;
`NO COUNTY SELECTED`.

Every one is marked in the source. Eleven of them exist because a thing the original draws is
not drawn yet, so the number should fall as §2's right-hand column rises — which makes it the
same kind of number `docs/arms.json`'s `ours` count is, and worth printing beside it.

---

## 8. Two placements that are ours and are measurable

Both are already marked in the code as ours; what was missing was the original's number.

* **The path ball.** `Map_DrawPathMarker` blits at `(tileOrigin + 0x14, tileOrigin + 6)`
  with **no centring at all** — the frame's `cx`/`cy` are never read — and subtracts
  `g_mapTileHalfStep` from the x when it is the offset row's first cell.
  `campaign::draw_path_marker` centres the 15 × 15 ball on the tile instead, which is
  **(+2, +2)** off at the near zoom and **(+22, +10)** off at the far one, where the tile is
  10 × 6 and the original's fixed `+0x14` puts the ball two tiles to the right. The
  original's own placement looks wrong at zoom 2 and is what it does.
* **The menu bar's bevel.** `Ui_DrawBevelRect(0, 0, 0x280, 0x18)` outlines the whole bar,
  top and right in palette `0x1F` and bottom and left in `0x10`.
  `Chrome::draw_menu_bar_background` draws the two tile strips and no bevel.

---

## 9. What this says about the method

`docs/draws.md` §1 measured the pilot at *"about as long as reading its input arms, and it
produced more."* The campaign map cost about a day, as estimated, and the ratio held: **139
draw calls against `docs/arms.json`'s 25 input arms for the same screen group**, and nine
findings, of which
**five are things we draw or claim wrongly** rather than things we fail to draw — the same
direction the pilot's §2 reported.

Two things the pilot could not settle, that this screen can:

* **The unit of enumeration has to be the frame *selection*, not the call.** On this screen
  most drawing goes through blitters that take their subject in two globals, so a
  painter-keyed or call-keyed count reports **0** for the terrain and **1** for
  `Sprite_TopIt`. Counting `DAT_005C9288 = …` is what turns that 1 into the 18 that matters.
  Any per-screen script that follows this one has to know that.
* **The ratio is not a gate here either, and for a new reason.** 10 of 139 are debug-gated
  and 8 are dead; a ratio computed without classifying them would read 42 % where the honest
  figure is 49 %, and the classification is a judgement no script can make.
