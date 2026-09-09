# The management screens

The first reading of the game's **user interface**. `docs/kingdom.md` says what a county
*is*; this says what the player *sees* and what the player can *change*, with the pixel
rectangles and the widget tables that decide both.

It exists because the interface had never been looked at once. 394 named symbols covered
battle, battle AI, kingdom, units, sprites, maps, file I/O and text, and not one of them
was a screen. The cost of that showed up the first time we drew something: a campaign map
built by inference was wrong in a way one screenshot exposed.

Status legend, as in [`kingdom.md`](kingdom.md), meant literally:

* **[V] verified** — read out of the binary *and* cross-checked against a second,
  independent source: an `L2.eng` string, a shipped file's bytes, an arithmetic invariant
  that closes, or the shipped `lastturn.sav`.
* **[D] decompiled** — a straightforward reading of decompiled C or of the instruction
  stream, with no second source. Probably right about what the code does; the *name* may
  be wrong.
* **[I] inferred** — consistent with everything measured, not proven.

Addresses are the GOG Windows build, `ImageBase 0x400000`, no ASLR. Every name below is in
[`symbols.json`](symbols.json) under `"section": "ui"`.

---

## 0. The headline

**There is no county panel.** There are *four* of them, plus a village, plus a job popup,
and the thing that ties them together is a 162-pixel-wide sidebar down the right edge of
the map.

```text
 x=0                                                     478        640
 y=0   ┌────────────────────────────────────────────────┬──────────┐
       │  menu bar                                 y=24 │          │  the plate is
 y=24  ├────────────────────────────────────────────────┤ frame 54 │  not drawn
       │                                                │ 162x132  │  above y=24
       │                                                ├──────────┤
 y=156 │                                                │ frame 55 │  county — a
       │            the campaign map                    │ 162x94   │  2x2 hotspot
 y=250 │                                                ├──────────┤
       │                                                │ frame 66 │
 y=302 │                                                ├──────────┤
       │                                                │ frame 56 │  jobs, armies
       │                                                │ 162x128  │
 y=430 ├────────────────────────────────────────────────┼──────────┤
       │                                                │ frame 57 │  five buttons
 y=460 │                                                ├──────────┤
       │                                                │ frame 59 │  END TURN
 y=480 └────────────────────────────────────────────────┴──────────┘
```

**[V] The sidebar accounts for the screen exactly.** `Screen_DrawCampaign` (`0x0040F5FD`) and
`CountyStrip_Draw` (`0x0040F7D3`) draw `Misc_cty.pl8` frames 0x36, 0x37, 0x42, 0x38, 0x39
and 0x3B at y = 24, 156, 250, 302, 430 and 460, and the frame table in the shipped
`Misc_cty.pl8` gives those six frames heights 132, 94, 52, 128, 30 and 20:

```text
24 + 132 + 94 + 52 + 128 + 30 + 20 = 480
```

Every one of the six is **162 pixels wide**, and `640 − 478 = 162`. The unowned-county
branch replaces the middle three with frame 0x3A alone, which is 162 × **274**, and
`156 + 274 = 430` — the same closure by a different route. Neither number was chosen;
both fall out of a file we did not write.

---

## 1. Screens are a byte, and the byte is `g_screenId`

**[D]** `g_screenId` (`0x004EAC50`) selects the whole interface. Three parallel
`if`/`else if` chains switch on it and on nothing else:

| what | function | what it does |
|---|---|---|
| draw | `Screen_Draw` (`0x0040F1A0`) | 39 cases; calls the screen's painter |
| overlay | `Screen_DrawWidgets` (`0x004BA26E`) | per-screen widget lists and animations |
| input | `Screen_HandleInput` (`0x004BA9C8`) | per-screen widget hit-test tables |

The cases, named from the `L2.eng` groups each painter draws and the PL8 files each loads.
**[V]** for every row that names a group; **[D]** for the rest.

| id | painter | screen | evidence |
|---:|---|---|---|
| 0x00 | `Screen_DrawCampaign` `0x0040F5FD` | the campaign map | group 34 — season and year |
| 0x02 | `Village_Draw` `0x00412143` | **the village** — the county's own picture, and where peasants are moved. **An inset over the campaign map**, 363 × 320 at (64, 64) — §3.1 | groups 22 (fertility), 66 (weather); `villani1/villani2/vill/villtops.pl8` |
| 0x04 | `0x0041B032` | the map information panel: `UnitPanel_Draw` when a unit is picked, `FUN_0041BEFE` otherwise, and that branches on the same `g_pickedTileFlags` bits `Map_Click` does. **[D]**, and what it draws is not read | |
| 0x05 | *(no painter)* | **the village's rubber band** — §6.4 | `Village_BandStart` / `Village_BandRelease` |
| 0x06 | *(no painter)* | **the village carrying a selection** — §6.4 | `Village_Drop` |
| 0x08 | `Screen_Merchant` `0x00415FB7` | the merchant | `merchant.256` + `merchant.pl8`, `mercgrid.pl8` |
| 0x09 | `Court_Draw` `0x00416925` | **the court** — the realm's treasury and stores | group 70 |
| 0x0A | `Screen_Armoury` `0x00417EA7` | the armoury | `armoury.256` + `armoury.pl8`, `arm_grid.pl8` |
| 0x0B | `Diplo_DrawScreen` `0x00416CF3` | the other lords, and the menu of what to send one | `faces.pl8`; group 72 |
| 0x0C | `Screen_TradeGoods` `0x00416308` | trade goods | group 68; `merchant.pl8` **again** as the background, then `icontrad.pl8` |
| 0x0D | `Screen_Armoury` + a list | the armoury, buying — `Screen_Draw` has **no** arm for it; only the widget and input passes do | `g_armouryBuyWidgets` |
| 0x0F | `Panel_JobDetail` `0x00412B33` | **the job popup** — one of nine jobs, its workers and its output | group 74 |
| 0x11 | `0x004192B1` | army division | group 17; `icon_tmp.pl8` |
| 0x14 | `Panel_Population` `0x004110B1` | **population** | group 73 |
| 0x15 | `Panel_Tax` `0x0041152F` | **tax** | group 86 |
| 0x16 | `Panel_Happiness` `0x004116FB` | **happiness** | group 85 |
| 0x17 | `Screen_Armoury` then `Screen_RaiseArmy` | hire mercenaries / raise an army | groups 16, 69 |
| 0x18 | `Screen_SendSupplies` `0x0041AD5D` | send supplies to another county | group 33 |
| 0x19 | `Panel_Ration` `0x00411B72` | **rations** | groups 20, 21, 87 |
| 0x1A | `Screen_DiploDialog` `0x0041789B` | **the seven diplomacy dialogs**, on `g_diploKind` — §10.4 | group 72 |
| 0x1B | `0x00419789` | castle building | `cas_back.256` + `cas_back.pl8`, `caspics.pl8`, `cas_bits.pl8` |
| 0x1C | `0x0041E1DD` | **the campaign interstitial** — *not* the front end; §1.1 | group 36, group 101; `gateway.pl8`, `panels2.pl8` |
| 0x1D | `0x00421F14` | siege preparations | group 83; `sgeplans.pl8` |
| 0x1F | `0x0041E7E1` | **the front end**, and game setup: thirteen sub-pages on `g_setupPage` (`0x005530F0`) — §1.2 | groups 11, 39, 40, 101, 102, 103 |
| 0x1E | `Screen_ConfirmBox` `0x0040CCFA` | **the yes/no box** — one dialog for fifteen questions — §10.3 | group 10 |
| 0x20 | `Screen_GreatestNoble` | the standings | group 35 |
| 0x21 | `Screen_SliderBox` `0x0040CD58` | **the value spinner** — game speed, scroll speed, volumes — §10.3 | group 12 |
| 0x25 | `Screen_About` `0x0041543F` | about | group 59 |
| 0x28 / 0x29 / 0x2A | `Screen_DrawBattlefield` `0x004233F7` | the battlefield; the main loop treats 0x28 … 0x2A as one range | `g_battleIsSiege` picks the palette |
| 0x2B | `Screen_BattleOutcome` `0x00423241` | **the battle result banner**, seven outcomes | group 82 |
| 0x2E | `Screen_BattleMasterRatings` `0x00421707` | battle-master ratings | group 37; `score1.256` + `score1.pl8` |
| 0x2F | `Screen_BattleMasterRank` `0x00421D09` | the rank sheet | groups 37, 38; `score2.256` + `score2.pl8` |
| 0x31 | `Screen_HelpOptions` `0x004154EA` | help options | group 45 |
| 0x32 | `Menu_RestoreBackdrop` `0x0040C928` | **a menu-bar drop-down is open** — §10.1 | |
| 0x35 / 0x36 | `Screen_SaveLoad` `0x00414819` | **load / save** — one painter, one flag; §10.7 | group 40 |
| 0x39 | `Screen_AdvancedOptions` `0x00414F68` | advanced options | group 50 |
| 0x42 | `Screen_SoundOptions` `0x0041515C` | sound options | group 51 |
| 0x43 | `Screen_DisplayOptions` `0x004152EA` | display options | group 52 |
| 0x44 | `0x00425A6A` | the Smacker test page, left in the shipped build | group 89 |
| 0x45 | `Screen_LordsOfMagicAd` `0x0041E5D0` | the *Lords of Magic* advertisement | `lom.256` + `lom.pl8` |

**Our five-screen model is not the game's, and the gap is wider than this section first
said.** The management surface is a *campaign map plus insets*: the four county panels are
windows floating over whatever was underneath — and so, it turns out, is the **village**,
which this document called a full screen until a player looked at it (`docs/decisions.md`
C22, and §3.1 below).

**There is no screen clear anywhere in this engine.** `Screen_Draw` dispatches on
`g_screenId` and the painter it picks fills a rectangle; everything outside that rectangle
is still there from the last frame. So "screen" in the table above means *"a value of
`g_screenId`"* and nothing at all about how much of the display it owns. To learn that, read
the painter's **rectangle** — and read the site that *sets* `g_screenId`, which is usually
one grep and is the only place the question is actually answered.

**Every screen with a full-screen `.pl8` also reads a `.256` of its own.** The management
popups run under the campaign palette; `0x08`, `0x0A`, `0x1B`, `0x1C`, `0x1F` and `0x2E`
each do `File_ReadChunk("<name>.256", 0x004EA8A0, 0x300)` and then `Palette_Set`. A canvas
of palette *indices* means nothing without knowing which one — which is why
`l2_game::screen::Screen::palette` exists and the presenter asks the top screen.

### 1.1 `0x1C` is not the front end. It is the campaign interstitial.

**Corrected.** The row above used to read *"the front end"*, inferred from the two files
`FUN_0041E1DD` loads — `gateway.pl8` and `panels2.pl8`, which really are the front end's
artwork. The strings settle it the other way. The painter draws **group 36**:

```text
 0  "Congratulations!!"          4  "You have lost."
 1  "You have conquered"         5  "You have failed to conquer"
 2  "Events move on apace…"      6  "Until this country falls under your"
 3  "…now awaits you in"         7  "rule there can be no thought"
                                 8  "of further conquests, my lord."
```

with a map name from group 101 between the halves of each sentence: the map just fought
over (`DAT_00553E78`), and on a win the next one (`g_scenarioIndex`). A third branch fires
when the campaign counter `DAT_0053F258` reaches **8** — eight campaign maps — and drops
indices 2 and 3 for 9 … 15, *"The whole of Christendom … now lies firmly within your iron
fist."* **[V]**: three branches, each of which reads as one whole sentence, and the
`g_scenarioIndex` lookup happens only in the branch that mentions a *next* country.

The window is `FUN_00409346(panels2, 0x70, 8, 0x1A, h)` — 416 pixels wide from x = 112,
which is centred on 640 — 14 cells tall for the two short outcomes and 18 for the long one.

### 1.2 `0x1F`, and its thirteen sub-pages

`FUN_0041E7E1` is one `if`/`else if` chain on `g_setupPage` (`0x005530F0`) with thirteen
arms; `FUN_0041E61D` in front of it loads the background. **[D]**, with **[V]** wherever a
row names a group.

| page | painter | background | what it is |
|---:|---|---|---|
| 1 | `0x0041EA14` | `gateway` + `panels2` | **the title menu** — 11.0 *"Lords of the Realm 2"*, 11.1 *"The siege is on"*, items 11.2, 11.3, 11.47, 11.4 |
| 2 | `0x0041EC8A` | `gateway` + `panels2` | 11.5 *"Your options"* — items 11.6, 11.7, 11.19, 11.8, 11.9 |
| 3 | `0x0041EF42` → `FUN_004148E4(5)` | `gateway` + `panels2` | load a game — group 40.5 |
| 4 | `0x0041EF57` | `gateway` + `panels2` | 11.10 *"Choose your title and your shield."* |
| 5 | `0x0041F592` | `gateway` + `panels2` | 39.0, then 39.4 / 39.5 — the original campaign or the expansion's |
| 6 | `0x0041F3E9` | `gateway` + `panels2` | 39.0, then 39.1 / 39.2 — full game or skirmish |
| 7 | `0x0041F6C7` | `custom.256` + `custom.pl8` | **the custom game**, single player |
| 8 | `0x0041F77A` | `custom` | the custom game, multiplayer: page 7 plus five player cards, a chat log and a fourth button |
| 9 | `0x0041FDD6` | *the page underneath* | one open drop-down over page 7, 8, 11 or 12 |
| 10 | `0x00420428` | `gateway` + `panels2` | 11.16 *"No Lords of the Realm CD"* — 11.17, 11.18, 11.48, 11.49 |
| 11 | `0x00420630` | `skirmish.256` + `skirmish.pl8` / `skircust.pl8` | skirmish setup, multiplayer |
| 12 | `0x0042051C` | `skirmish` | skirmish setup, single player |
| 13 | `0x0042150B` | `skirmish` | the skirmish file box, over page 12 |

**The menu geometry.** Pages 1, 2 and 4 put every item in `FUN_00403EE4(x, y, 0xC0, 0x18)`
— a 192 × 24 recess whose top and right edges are colour `0x35` and bottom and left `0x28`,
the *opposite* lighting to `Ui_DrawInsetRect` — with the caption centred in the same 192
pixels five below the top. Pages 1 and 2 step them 36 apart from y = 0x5B at x = 0xE0.

**Page 4's shields come from `Panels2.pl8`, not from `Misc_cty`/`Misc_sel`.** `FUN_0041F1DD`
blits from `DAT_004EABEC`, the general scratch buffer, which on this page holds
`panels2.pl8`. `Misc_sel.pl8` has seventeen frames and the indices run to 215;
`Panels2.pl8` has 216. The file confirms it: frames 205 … 214 are five pairs of ~60 × 65
shields, 204 is a 224 × 32 name plate and 215 is a 54 × 27 plaque. **[V]** — nothing else
in either file is that shape, and the reimplementation draws them and they are shields.

**The custom game's twelve options close exactly.** **[V]** Three `.data` tables:

| table | shape | what |
|---|---|---|
| `0x004D3098` | 12 × `(x, boxY, labelY)` | four columns of three; the value box is `FUN_004093E0(x, boxY, 6, 3)` and the label is the wrapped group 102 string at `(x, labelY)`, width 100 |
| `0x004D3128` | 12 × `i32` | each option's base index into group 103 |
| `0x004D3158` | 12 × `(x, y, rows)` | the open list; `rows` is the item count **plus two** |

Bases `0, 2, 5, 9, 11, 15, 19, 25, 29, 34, 37, 44`; counts from the third table
`2, 2, 4, 2, 4, 4, 6, 4, 5, 3, 7, 2`. Every run ends exactly where the next begins, the
twelve runs use **45 of group 103's 46 strings**, and the one left over is index 4,
*"one"* — because *Nobles* starts at *"two"*. Twelve labels in group 102, twelve bases,
twelve counts, no remainder. None of it was chosen by us.

**One thing is unresolved.** `FUN_00432B05` and `FUN_00432CC8` are the two click handlers,
and they branch on `g_uiHotspotId` in an order that does not match the order the painters
draw the items: on page 1, id 3 sets the quit flag while id 4 plays `lom.smk`, and the
painter draws *"Lords of Magic?"* third and *"Exit game"* fourth. Either the widget table
is not in drawing order or one of the two readings is wrong. `crates/l2-game`'s front end
keys its destinations to the **captions**, which are [V], and says so at the call site.

---

## 2. The county strip, and how the four panels are reached

`CountyStrip_Draw` (`0x0040F7D3`) draws the strip for the selected county, and it draws two
completely different things depending on whether you own it. **[D]**

### 2.1 Owned

| what | where | field |
|---|---|---|
| county name, centred in 160 px | (480, 165) | `L2.eng` group 100, index `scenarioIndex*20 + countyId` |
| population | (508, 189) | `+0x24` |
| happiness | (602, 189) | `+0x0C` |
| *"Tax"*, centred in 76 px | (480, 213) | group 61 index 0 |
| tax rate, with a `%` | (506, 226) | `+0xB9` |
| *"Ration"*, centred in 76 px | (564, 213) | group 61 index 1 |
| ration achieved, centred in 76 px | (564, 226) | `+0x15D` as group 21, **red when it differs from `+0x15E`** |
| health thermometer | (552, 181) | `Misc_cty.pl8` frame `0x46 + healthBand`, 14 × 59 |

All of it in the 9-pixel font (`Fntl2_9.pl8`), which is the only place that font is used.

### 2.2 Unowned

Frame 0x3A, the county name, and — for a county held by another realm — group 15,
*"Sovereign land / of"*, and the owner's name read out of `g_playerNames` (`0x00553D54`,
stride 0x2C), all in that realm's own colour (realm `+0x08`).

### 2.3 The 2 × 2 hotspot, which is the whole navigation

`CountyStrip_Click` (`0x00438CEB`) **[V]**:

```c
if (mx > 487 && mx < 630 && my > 181 && my < 241) {
    if (my <  212) { if (mx < 548) Population(); else if (mx > 567) Happiness(); }
    else           { if (mx < 548) Tax();        else if (mx > 567) Ration();    }
}
```

The four quadrants are exactly the four things drawn above them, in the same corners.
The dead band `548 … 567` is **19 pixels wide, and the health thermometer is 14 pixels wide
and drawn at x = 552**: 552 … 566 sits inside 548 … 567 with two pixels to spare on each
side. The bar is deliberately not clickable, and the gap exists for it.

So: **population, tax, happiness and rations are four separate windows, and the county
strip is the menu.** Nothing else opens them.

### 2.4 The rest of the sidebar

**[D]** Below the strip, `0x00438E3B` turns the 162 × 128 plate at y = 302 into two columns
of job rows — farm jobs left of x = 560, industry right of it, row height 60, 45 or 30 px
depending on how many rows there are — and a click opens the job popup (screen 0x0F) for
that job.

The five buttons in the 162 × 30 strip at y = 430 are hotspot table `g_sidebarButtons`
(`0x004DC680`), and the full-width button at y = 460 is **end turn**
(`Turn_End`, `0x0043AC23`, which writes 999 into the realm's `+0x00`):

| button | x range | action |
|---|---|---|
| 1 | 478 … 511 | the county's army / hire mercenaries (screen 0x17) |
| 2 | 512 … 543 | the court (screen 0x09) |
| 3 | 544 … 575 | send supplies (screen 0x18) |
| 4 | 576 … 607 | `0x00436A88` |
| 5 | 608 … 639 | `0x0043611B` |
| end turn | 478 … 639, y 460 … 479 | `Turn_End` |

The table's five rectangles start at x-offsets 0, 34, 66, 98, 130 and end at 33, 65, 97,
129, 161. `34 + 32 × 4 = 162`. It closes.

---

## 3. How anything is drawn

Thirteen primitives account for every panel in this document. **[D]** throughout, from
their own bodies.

| function | signature | what it does |
|---|---|---|
| `Ui_DrawBox` `0x00409397` | `(x, y, wCells, hCells)` | a framed window, `w × h` cells of **16 px**, with its interior filled |
| `Ui_DrawBoxBorder` `0x00409934` | `(style, x, y, w, h)` | just the border |
| `Ui_DrawBoxInterior` `0x00409C93` | `(x, y, w, h)` | just the interior |
| `Ui_DrawInsetRect` `0x00403DEB` | `(x, y, w, h)` | a two-tone recessed rectangle: colour 0x10 top and right, 0x1F bottom and left |
| `Ui_DrawText` `0x00402637` | `(str, x, y, font, colour)` | one plain string, with a drop shadow |
| `Eng_DrawString` `0x00402D37` | `(group, index, x, y, font, colour)` | one `L2.eng` string |
| `Ui_DrawCentred` `0x00402C5E` | `(group, index, x, y, width, font, colour)` | one `L2.eng` string, centred in `width` |
| `Ui_DrawNumber` `0x00402F64` | `(value, lead, suffix, x, y, font, colour)` | a number with a **leading character** and a suffix |
| `Ui_DrawNumberRight` `0x004030C6` | `(value, lead, suffix, x, y, width, font, colour)` | the same, right-aligned in `width` |
| `Ui_DrawDelta` `0x00402E0C` | `(value, mode, prefix, suffix, x, y, font, colPos, colNeg)` | a signed number, **drawn as nothing at all when it is zero** |
| `Ui_DrawCount` `0x0041AB67` | `(value, unitIndex, x, y, font, colour)` | a number plus a **singular/plural noun** |
| `Ui_DrawHappinessDelta` `0x0041AC95` | `(value, x, y, font, colPos, colNeg)` | `( ±n ☺ )` — a signed number wrapped in brackets with the happiness face after it |
| `Ui_DrawYear` `0x0041A900` | `(year, x, y, style)` | a year with **BC / AD** from group 26 |
| `Ui_OkButton` `0x0040D1BC` | `(x, y, mode)` | the tick that closes a panel: the button sheet's frame 0x33 (mode 0) or 0x10 (mode 1) |

`Ui_DrawText` advances a pen width in `g_penAdvance` (`0x005CD404`), which every caller
resets to 0 and then adds to the next x — that is how a label and its value are laid out
without either knowing the other's width. It adds **4 pixels of trailing space** to every
string it draws.

Four details worth carrying into any reimplementation:

* **`Ui_DrawDelta` draws nothing when the value is zero and `mode == 0`.** The happiness and
  population panels are full of blank rows in a quiet season, on purpose.
* **Numbers reserve a character for their sign.** `Ui_NumberToBuffer` (`0x004022BD`)
  formats from buffer index 1, leaving index 0 for the caller to fill with `'+'`, `'-'`,
  `' '` or `'@'`. `'@'` has an empty glyph, so a zero still occupies the sign column and a
  column of numbers stays aligned.
* **Plurals come out of `L2.eng` group 8**, singular at even index and plural at odd:
  `Ui_DrawCount(n, i)` uses index `i` when `|n| == 1` and `i + 1` otherwise. Job worker
  counts pass `job*2 + 30`, which is *"Farmer / Farmers"* for grain, *"Dairy maid"* for
  cattle, *"Serf"* for reclamation, then *"Builder"*, *"Miner"*, *"Quarrier"*,
  *"Forester"*, *"Blacksmith"* and *"Peasant"* — nine jobs, in order. **[V]**
* **Every string is drawn three times**, at y−1 and y+1 in two shadow colours and then at y
  in the real colour. Text on these panels is embossed, not flat.

### 3.1 There are **two** ways to float something over the screen  **[V]**

Written down because the absence of one of them actively points the wrong way, and did.

| | how it is drawn | who uses it |
|---|---|---|
| **a framed window** | `Ui_DrawBox` (`0x00409397`) → `Ui_DrawBoxBorder` + `Ui_DrawBoxInterior`, a kit of 16-pixel cells out of `Panels.pl8` (§4.1) | the four county panels, the job popup (`Ui_DrawBox(0x30, 0x60, 0x19, rows)`), the merchant, the court |
| **a raw blit** | one sprite straight into the framebuffer at a fixed origin — no border, no interior, no clear | **the village**: `vill.pl8` frame 0, 363 × 320, at (`0x40`, `g_villageTopY`) |

Neither clears the screen, so both leave whatever was underneath showing around them. The
trap is that only the first is *recognisable* as a window: reading *"`Village_Draw` contains
no `Ui_DrawBox` call"* as evidence that the village is a full-screen page is exactly the
false step `docs/decisions.md` C22 records. It means only that the village is the other kind.

**How to tell what a painter actually covers**, when it is the second kind and there is no
box call to read:

1. the sprite blit's origin and the frame's own width and height — `FUN_0040A682(frame, x, y)`
   takes them from the PL8's frame table;
2. any **save/restore band** the painter manages. `Village_Draw` ends with
   `g_drawX`, `g_drawY`, `g_spriteWidth`, `g_spriteHeight` and `FUN_004B3F0A(buffer, 0xA0)`,
   whose twin `FUN_004B3EC0` restores it — that pair bounds everything the screen may dirty;
3. and, decisively, the site that sets `g_screenId`.

**`FUN_004B3F0A` copies dwords, which is the one number here that does not read at face
value.** `g_spriteWidth` is `0x78` and the copy advances an `undefined4 *` that many times a
row — 0x78 × 4 = **480 bytes**, one byte a pixel — then adds the argument `0xA0` = 160 to
reach the next row. `480 + 160 = 640`, the screen stride, exactly. So the village's band is
**480 × 320 at (0, `g_villageTopY`)** and not 120 wide; the picture inside it is narrower
still, and the county sidebar (x ≥ 478) and the menu bar (y ≤ 23) are outside it.

---

## 4. The chrome: which files, and which frames

**[V]** `Res_LoadStatic` (`0x00499859`) walks a table of thirteen `{char name[16]; u32 size}`
records at `g_preloadTable` (`0x004D9F48`), each into a fixed `.data` buffer. That table
*is* the interface's asset list, and nobody had read it:

| # | file | buffer | what it is |
|---:|---|---|---|
| 0–2 | `base01.256`, `t32_stn1.256`, `t32_bat1.256` | | palettes |
| 3 | `Fnt_8.pl8` | `0x005CBFB0` | 8 px font, the battle overlay |
| 4 | `Fntl2_9.pl8` | `0x005C9A90` | **9 px font — the county strip, and only that** |
| 5 | `Font_10.pl8` | `0x005AEBA0` | 10 px font |
| 6 | `Fntl2_14.pl8` | `0x005AF8F0` | **14 px font — every panel body line** |
| 7 | `Fntl2_22.pl8` | `0x005B2FA0` | **22 px font — every panel heading** |
| 8 | `mouse.pl8` | `0x0058FEC0` | the pointer |
| 9 | `System2.pl8` | `0x005BB540` | the button sheet — but see §4.2, where the kingdom screens swap `System.pl8` into the same buffer |
| 10 | `Panels.pl8` | `0x0057D3D0` | **every window frame** |
| 11 | `l2.eng` | `0x00591580` | the strings |
| 12 | `vill_gd8.pl8` | `0x00542CE0` | **the village's drop grid** — 45 × 40 cells of 8 px naming the cluster under each part of the picture. §6.4 |

`System.pl8` is the same size and frame layout and is swapped into the same buffer by
`Res_LoadButtons` (`0x00499A1C`) with a different index. §4.2 is why that matters.

Alongside those, one mode-dependent sheet lives in `g_miscCtySheet` (`0x005530C8`):
**`Misc_cty.pl8`** for the kingdom, `Misc_ske.PL8` / `Misc_bat.PL8` for battle,
`Misc_sel.PL8` for setup.

### 4.1 `Panels.pl8` — 262 frames, every one accounted for

**[V]**, and this is the arithmetic that pins the window model. `Ui_DrawBoxBorder` indexes
0 … 51 and `Ui_DrawBoxInterior` indexes `52 + (col % 12) + (row % 12) * 12`:

| frames | count | size | what |
|---|---:|---|---|
| 0 … 3 | 4 | 16 × 16 | corners: top-left, top-right, bottom-right, bottom-left |
| 4 … 15 | 12 | 16 × 16 | top edge, twelve variants, cycled along the run |
| 16 … 27 | 12 | 16 × 16 | bottom edge |
| 28 … 39 | 12 | 16 × 16 | left edge |
| 40 … 51 | 12 | 16 × 16 | right edge |
| 52 … 195 | **144 = 12 × 12** | 16 × 16 | the interior tile field |
| 196 … 203 | 8 | 24 × 24 | |
| 204 … 255 | 52 | 16 × 16 | the **second** border set — `style > 0` adds 0xCC = 204 |
| 256 … 260 | 5 | 13 × 16 | |
| 261 | 1 | 48 × 48 | |

**The `0xCC` belongs to the border and not to the interior.** `FUN_004093E0` — the box
every management popup opens — is literally `Ui_DrawBoxBorder(1, x, y, w, h)` followed by
`Ui_DrawBoxInterior(x + 0x10, y + 0x10, w - 2, h - 2)`, and the interior function takes no
style argument at all. Adding 0xCC to an interior index sends `0x34 + n` past 255 and into
the five 13 × 16 banner frames at the end of the file. Ours did, the first time anything
asked for set 1, and the custom-game screen's twelve option boxes came out full of shields.

`Panels2.pl8` has the same layout for its first 204 frames and then diverges: 196 … 203 are
the 24 × 24 strip, 204 is a 224 × 32 name plate, 205 … 214 are the five shield pairs and
215 is a 54 × 27 plaque. It has no second border set, which is why the setup pages only
ever draw boxes from it in set 0.

4 + 48 = 52 border frames; 144 interior frames; 52 + 144 = 196, and frame 196 is the first
frame in the file that is not 16 × 16. The size histogram of the shipped file is 248 frames
of 16 × 16, 8 of 24 × 24, 5 of 13 × 16 and one 48 × 48, and 52 + 144 + 52 = 248.
**Nothing is left over and nothing is missing.**

### 4.2 The buttons are `System.pl8`, not `System2.pl8`

**[V]** `Res_LoadButtons` (`0x00499A1C`) loads `system2.pl8` for skin 0 and `system.pl8`
for skin 1 into the same buffer, and **the kingdom screens ask for skin 1** — the call in
`0x004BA000`'s frame loop is guarded on the in-game state being 3.

The two files are byte for byte the same size, 56,518, with the same 84-frame table, so it
looks like a cosmetic skin. It is not. **69 of `System2.pl8`'s 84 frames are entirely
index 0** — every frame in the table below except the tick — while **none of
`System.pl8`'s are**. Drawing a panel from `System2.pl8` draws its arrows and its slider as
nothing at all, which is exactly how this was found: the reimplementation loaded
`System2.pl8`, its slider test could not tell two knob positions apart, and the files' own
bytes said why.

84 frames into a 56,600-byte buffer. Every button is a **normal/pressed pair**:
`Widget_Draw` adds 1 to the frame while the widget's press timer is running.

| frames | size | what |
|---|---|---|
| 0x15 / 0x16 | 24 × 24 | **tax up** |
| 0x17 / 0x18 | 24 × 24 | **tax down** |
| 0x10 / 0x11 | 24 × 24 | `Ui_OkButton` mode 1 |
| 0x33 | 24 × 24 | `Ui_OkButton` mode 0 — the tick that closes a panel |
| 0x4A | 24 × 24 | **slider left cap** |
| 0x4B | 24 × 24 | **slider right cap** |
| 0x4C | **10 × 32** | **slider knob** |

### 4.3 `Misc_cty.pl8` — the kingdom-mode icons

**[V]** 97 frames. The ones this document identifies:

| frame | size | what | how it is known |
|---|---|---|---|
| 0x17 | 20 × 18 | the happiness face | drawn after every happiness number |
| 0x18 | 8 × 18 | a person | ration panel, beside *"Fed"* |
| 0x21 | 36 × 27 | grain sack | ration panel |
| 0x26 | 37 × 24 | cattle | ration panel |
| 0x2A | 23 × 19 | sheep | ration panel |
| 0x2C / 0x2D / 0x2E | | iron / stone / wood | court screen, beside those three lines |
| 0x30 … 0x35 | | six weapon types | court screen |
| 0x36 | 162 × 132 | sidebar top plate | §0 |
| 0x37 | 162 × 94 | sidebar county plate, owned | §0 |
| 0x38 | 162 × 128 | sidebar jobs plate | §0 |
| 0x39 | 162 × 30 | sidebar button strip | §0 |
| 0x3A | 162 × 274 | sidebar county plate, **not** owned | §0 |
| 0x3B | 162 × 20 | end-turn strip | §0 |
| 0x3E | 60 × 77 | the tax vignette | `Panel_Tax` draws it at (320, 160) |
| 0x42 | 162 × 52 | sidebar middle plate | §0 |
| 0x46 … 0x4A | 14 × 59 | health thermometer, five levels | `frame = 0x46 + healthBand` |

### 4.4 The fonts, and the one table that makes them readable

**[V]** A font is a `.pl8` plus ninety-six bytes of `.data`. `Glyph_Draw` (`0x00402A14`) is
four lines of arithmetic:

```text
frame   = g_glyphWidths[c - 0x20] - 1      // 0 means no glyph at all
y      += frameRecord[0x0D]                // the glyph's own vertical offset
advance = frame.width + 1
```

and `FUN_004014F0`, which measures a string for `Ui_DrawCentred`, indexes **the same
bytes** from `0x004D71D0` with the raw character — `0x004D71F0 - 0x20`. A space is never
looked up: it advances 4 and draws nothing, which is also what makes `'@'` the invisible
sign column `Ui_NumberToBuffer` relies on.

`g_glyphWidths` is therefore a **character-to-frame map**, not the widths its name
suggests; the widths are in the file's own frame records. The name is kept because it is
the one in `symbols.json`.

The mapping is self-checking, which is what makes it [V] rather than [D]. It sends `'a'` to
frame 0 and `'A'` to frame 26 of `Fntl2_14.pl8`, and the frames it sends `'g'`, `'j'`,
`'p'`, `'q'` and `'y'` to are exactly the frames in that file that are four pixels taller
than their neighbours. Descenders land on the descending letters. Nothing else would.

Four more details a reimplementation needs:

* **Every string is drawn three times** — `Ui_DrawText` blits each glyph at `y - 1` in one
  shadow colour, at `y + 1` in another, then at `y` in the real one. The pair is `0x10` and
  `0x1F` everywhere **except** when `g_screenId` is `0x1C` or `0x1F`, where the same
  function uses `0x36` and `0x2C` instead — those two screens run under `gateway.256` and
  different indices read as shadow. That branch is the first thing in the function and the
  only thing in it that knows which screen it is on.
* **`DAT_005AEA40` switches the emboss off.** When it is non-zero the glyph is drawn once,
  at `y`, in its own colour. The front end sets it around every menu item, every button
  caption and every body line and clears it for the heading, so on those pages **the
  heading is embossed and nothing else is** — which is not what a reimplementation would
  guess, and looks wrong on every page at once if it guesses.
* **`DAT_0058FE2C` picks the capitals out.** When it is non-zero, characters `0x41 … 0x5A`
  — `A` through `Z`, nothing else — are drawn in **colour 1** rather than the caller's. The
  setup pages set it around their heading and `0x1C` sets it around everything it draws. On
  the title screen that is what makes the `L` and the `R` of *"Lords of the Realm 2"* red
  and the rest of the line black.
* **Wrapped text steps by the font.** `FUN_0040328E(group, index, x, y, width, …)` wraps to
  `width` and ends `if (font == &g_fontHeading) y += 0x18; else y += 0x10;` — 24 pixels for
  the 22-pixel font, 16 for the 14-pixel one — stripping a leading space from every line
  but the first.

**The capitals hang below the baseline, and that is the font.** Rendering `Fntl2_14.pl8`
through the formula above puts every lowercase letter's last ink row on `y + 11` —
`'l'`, `'t'`, `'i'`, `'d'` and `'f'` (12-row frames, overhang 0) land there and so do
`'a'`, `'o'`, `'c'` and `'n'` (9-row frames, overhang 3) — while `'A'` runs to `y + 13`.
The two extra rows are `..#######....###.` and `##...##.....###.#`: a blackletter swash
foot. `Fnt_8.pl8` and `Fntl2_9.pl8` align capitals and lowercase exactly under the same
formula, and `Fntl2_22.pl8` bottoms its capitals with its ascenders. Nothing is off by two;
one of the five fonts has decorated capitals.

---

## 5. The four county panels, line by line

Every coordinate below is read out of the painter. **[D]** unless the row names an `L2.eng`
group, in which case the string is the second source and it is **[V]**.

### 5.1 Population — `Panel_Population` (`0x004110B1`), screen 0x14

Window `Ui_DrawBox(16, 48, 28, 23)` → **(16, 48) to (464, 416)**, 448 × 368.

| y | what | field |
|---:|---|---|
| 56 | *"Population in"* (group 73.0) at x = 20, then the county name (group 100) | |
| 84 | the **history graph**, 402 × 155 at (32, 84) — §7 | |
| 240 | the graph's x axis: first year, `"-"`, `g_year`, all through `Ui_DrawYear` | |
| 240 | *"Greatest population"* (73.8) at x = 176, then the peak and *"people."* (73.9) | from the graph |
| 266 | *"Last season"* (73.1) at x = 48, value at x = 336 | `+0x28` |
| 298 | *"Births"* (73.2) | `+0x30` |
| 314 | *"Deaths"* (73.3), **negated** | `+0x34` |
| 330 | *"Army"* (73.4) | `+0x38` |
| 346 | *"Emigrants to"* (73.5) plus the destination county's name, negated — or *"No emigration."* (73.10) | `+0x3C`, `+0x58` |
| 362 | *"Total immigrants"* (73.6) — or *"No immigration."* (73.11) | `+0x40` |
| 386 | *"This Season"* (73.7) | `+0x24` |

Labels at x = 48, values right-anchored from x = 336. Button: `Ui_OkButton(436, 388, 0)`.

### 5.2 Happiness — `Panel_Happiness` (`0x004116FB`), screen 0x16

Window `Ui_DrawBox(16, 48, 28, 24)` → **(16, 48) to (464, 432)**, 448 × 384.

The same shape: heading at y = 56, the graph at (32, 84) in happiness mode,
*"Average happiness"* (85.8) and `+0x18` at (176, 241) with a face sprite after it, then

| y | label | field |
|---:|---|---|
| 268 | *"Last season"* (85.1) | `+0x0D` |
| 298 | *"From taxes"* (85.2) | `+0x12` |
| 314 | *"From ration"* (85.3) | `+0x13` |
| 330 | *"From health"* (85.4) | `+0x14` |
| 346 | *"From army"* (85.5) | `+0x15` |
| 362 | *"From ale"* (85.6) | `+0x194` |
| 378 | *"From events"* (85.9) | `+0x17` |
| 402 | *"This Season"* (85.7) | `+0x0C` |

The six middle rows go through `Ui_DrawDelta`, so **a zero row is blank**. Button:
`Ui_OkButton(436, 404, 0)`.

### 5.3 Tax — `Panel_Tax` (`0x0041152F`), screen 0x15

Window `Ui_DrawBox(80, 144, 20, 9)` → **(80, 144) to (400, 288)**, 320 × 144.

| y | label at x = 96 | value | field |
|---:|---|---|---|
| 168 | *"Tax rate"* (86.1) | at x = 256, suffix `%` | `+0xB9` |
| 200 | *"People pay"* (86.2) | immediately after the label — `Ui_DrawCount(…, 0)` → *"Crown / Crowns"* | `+0xC0` |
| 232 | *"This county"* (86.3) | `( ±n ☺ )` at x = 240 | realm `+0x28` **plus** county `+0x0F` |
| 256 | *"Other counties"* (86.4) | `( ±n ☺ )` at x = 240 | `+0x16` |

Vignette frame 0x3E at (320, 160); button `Ui_OkButton(372, 260, 0)`.

**The two arrow buttons are the widget table `g_taxWidgets` (`0x004DD790`)** — two records,
both 24 × 24, kind 4 (auto-repeating press):

| # | position | frames | callback |
|---:|---|---|---|
| 0 | (192, 162) | 0x15 / 0x16 | `Tax_Increase` `0x0043AA32` |
| 1 | (224, 162) | 0x17 / 0x18 | `Tax_Decrease` `0x0043AAD5` |

### 5.4 Rations — `Panel_Ration` (`0x00411B72`), screen 0x19

Window `Ui_DrawBox(128, 96, 18, 15)` → **(128, 96) to (416, 336)**; with *Armies Eat* on the
height is 17 cells and it ends at y = 368 instead.

| y | what |
|---:|---|
| 104 | *"Ration"* (87.0), centred across the window's 288 px |
| 136 | *"Wanted:"* (87.1) at x = 144; the level's name (group 21) at x = 240 |
| 161 | *"Achieved:"* (87.2); the level's name at x = 240, **red when it differs from wanted**; `( ±n ☺ )` at x = 340 from `+0x11` |
| 186 | *"Health:"* (87.3); the band's name (group 20) at x = 240; `( ±n ☺ )` at x = 340 from `+0x10` |
| 216 | the **grain ⇄ livestock slider** — §6.2 — flanked by a grain sack at x = 144 and cattle at x = 370 |
| 256 | grain, cattle and sheep icons at x = 224, 284, 344 |
| 286 | *"Fed"* (87.5) at x = 160; three right-aligned numbers at x = 208, 266, 324 from `+0x170`, `+0x174`, `+0x16C` |
| 308 | *"Eaten"* (87.4) at x = 144; two numbers at x = 208, 266 from `+0x178`, `+0x17C` |
| 336 | with *Armies Eat*: `+0x198 + +0x19C`, then *"men foraging in the county."* (87.8) |

Two arrows at (344, 130) and (368, 130) — table `g_rationWidgets` (`0x004DD7C0`), frames
0x15 and 0x17, callbacks `Ration_Increase` (`0x0043A1E9`) and `Ration_Decrease`
(`0x0043A2B2`) — set the *wanted* level. Button `Ui_OkButton(388, 308, 0)`.

**`+0x16C`, `+0x170` and `+0x174` are three fields this project has never named.** They are
the "Fed" row — how many mouths each of the three food sources actually fed — and
`docs/kingdom.md` §1.3 documents only the two "Eaten" fields beside them. See §8.

---

## 6. The controls, and their real limits

### 6.1 The widget record

**[V]** Both hit-testers and the drawer agree on a **24-byte record**, and a widget table
is a plain array of them in `.data`:

| off | type | meaning |
|---|---|---|
| 0x00 | i16 | x |
| 0x02 | i16 | y |
| 0x04 | i16 | base sprite frame |
| 0x06 | i16 | **size** — the square hit box's side **and** the sheet selector |
| 0x08 | u32 | callback, called with no arguments |
| 0x0C | u8 | latched state; adds 1 to the frame |
| 0x0D | u8 | press countdown; adds 1 to the frame while it is non-zero |
| 0x0E | u8 | auto-repeat counter, capped at 0x2F |
| 0x0F | u8 | kind: 2 = toggle, 3 = momentary, 4 = repeating push, 5 = delayed |
| 0x10 | u32 | argument 1, published in `g_uiHotspotId` (`0x0059154C`) |
| 0x14 | u32 | argument 2, published in `g_uiHotspotArg` (`0x00591550`) |

`Widget_Draw` (`0x0040CFD2`) takes the sprite from `g_miscCtySheet` when the size is below 24
and from the button-sheet buffer otherwise — so **the size field decides both the hit box
and the sheet**, which is why every button in this document is 24 or 32 pixels square.
`Widget_Test` (`0x0040DA1E`) is the hit test and the auto-repeat clock.

A second table format, hit-tested by `Hotspot_Test` (`0x0040E3EE`) and never drawn, reuses
the same 24 bytes as `{x0, y0, x1, y1, callback, …}`. The sidebar's five buttons and the
end-turn strip are that.

### 6.2 What the player can actually set on a county

| order | widget | limit | evidence |
|---|---|---|---|
| **tax rate** | two 24 × 24 arrows on the tax panel | **0 … 50, one point a click** | §6.3, **[V]** |
| **ration wanted** | two 24 × 24 arrows on the ration panel | **0 … 5** — `Ration_Increase` guards `< 5`, `Ration_Decrease` guards `> 0`, and group 21 has exactly six names | **[V]** |
| **grain ⇄ livestock split** | a **slider**, `Ration_SliderClick` `0x0043A379` | **0 … 100** | **[V]** |
| **peasants between jobs** | rubber-band select on the village, then click a job cluster | §6.4 | **[D]** |

**Every one of them is refused outright on a county you do not own.**
`Ration_SliderClick` opens `if (county.owner == g_localPlayer)`, `CountyStrip_Click` the
same, and the strip does not even draw the tax and ration rows otherwise.

**The slider closes arithmetically.** `Panel_RationSlider` (`0x00411FDE`) draws cap 0x4A at
x = 200 and cap 0x4B at x = 324, both 24 wide → 200 … 224 and 324 … 348; the track runs
x = 224 … 323, exactly 100 pixels, exactly between them; the knob (frame 0x4C, 10 wide) is
drawn at `220 + value`, so its centre tracks 225 … 325. `Ration_SliderClick`'s three hit
boxes are (200, 220, 24, 24) → step −1, (325, 220, 24, 24) → step +1, and
(224, 220, 102, 24) → jump to `mouseX − 224`, clamped 0 … 100.

### 6.3 The tax ceiling is **50**

**[V].** `Tax_Increase` (`0x0043AA32`) and `Tax_IncreaseCounty` (`0x0043AA83`) both guard

```c
if (county[+0xB9] < 0x32) county[+0xB9]++;
```

and `Tax_Decrease` (`0x0043AAD5`) guards `!= 0`. `0x32` is 50. There is no other
player-facing writer of `+0xB9`: the only others are `AI_SetTaxRates`, whose four ladders
top out at 15, and the multiplayer message handler at `0x004434EB`, which applies a peer's
already-clamped value.

**And a table confirms it independently.** `Tax_RecomputePreview` (`0x0044B80B`) sets the
"Other counties" happiness term from `g_taxHappinessOther` (`0x004D63D8`), an i32 array
indexed by the rate. Read out of the file:

| rate | 0 … 19 | 20–23 | 24–27 | 28–31 | 32–34 | 35–37 | 38–39 | 40–41 | 42–43 | 44 | 45 | 46 | 47 | 48 | 49 | **50** | 51 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| value | 0 | −1 | −2 | −3 | −4 | −5 | −6 | −7 | −8 | −9 | −10 | −11 | −12 | −13 | −14 | **−15** | *0* |

**The table has exactly 51 entries and the 52nd word is the start of the next table.** An
array indexed by a rate that could reach 100 would need 101 entries; it has 51, and 50 is
the last index that is not zero. Two independent readings — the branch and the data — give
the same ceiling.

`crates/l2-game`'s `MAX_TAX_RATE` was **100**, and its own doc comment said so:
*"this is the arithmetic bound, not a reading of `Lords2.exe`"*. It is now 50, and it is a
reading.

### 6.4 Moving peasants — the village, not a panel

**[D]** There is no "assign labour" control anywhere in the four panels. Peasants are moved
on the **village screen** (0x02), which is **an inset over the campaign map** — see §6.4.4,
where a paragraph that used to say the opposite is kept and corrected.

1. `Village_DrawPeasants` (`0x00412666`) draws **eight clusters** of up to **25 icons**, at
   offsets from `g_jobClusterOrigins` (`0x004D85A8` — eight `{i32 x, i32 y}` pairs:
   (22,50) (147,30) (271,26) (268,188) (117,200) (15,245) (146,120) (19,122)), each icon
   placed by `g_peasantIconOffsets` (`0x004D85E8` — 25 `{u8 x, u8 y}` pairs forming a 5 × 5
   grid, 16 px apart in x and 12 in y, with the rows offset 0, 4, 8, 0, 4).
2. `Village_BoxSelect` (`0x0043958A`) marks every icon inside a **rubber-band box**, and
   **cancels the whole selection if the box straddles two clusters**.
3. Clicking another cluster calls `Labour_Move` (`0x00439B52`) with
   `workers = selectedIcons × county[+0xB8]`, clamped to what the source job has.

**`+0xB8` is `popBand = (pop − 1)/25 + 1`**, so **one icon is one twenty-fifth of the
county's people, rounded up** — which is exactly why there are 25 icon slots. The grid and
the field are the same fact seen twice.

`g_jobClusterToSlot` (`0x004D6780`) maps cluster → labour slot: `[5, 0, 1, 3, 6, 7, 8, 2]`,
and `Job_SlotForCluster` (`0x004517CA`) overrides cluster 0 to slot **4** when the county
has industry 1's resource but not industry 3's.

**There are eight clusters and nine jobs.** Iron and stone *share* cluster 0, because the
mine (`Misc_cty` frame 0x2B) and the quarry (frame 0x28) are painted at the same spot,
`(0x4C, top + 0x0C)`. `Village_ClusterHasJob` (`0x0045183A`) refuses that one cluster, and
only when the county has neither — the other seven never refuse, and
**`Village_DrawPeasants` loops 0 … 7 with no test at all**, so a county with no mine still
shows the slot. It simply has nobody in it.

### 6.4.1 The gesture is three screen ids **[V]**

`Screen_HandleInput` gives the drag its own screens, and reading them settles what the
gesture actually is rather than leaving it to be guessed:

| id | what | leaves when |
|---|---|---|
| `0x02` | the village, idle | `Village_BandStart` (`0x004393EB`) sees the pointer **9 pixels** from where the button went down → `0x05` |
| `0x05` | the band | `Village_BandRelease` (`0x00439541`) sees the button **released**: `0x06` if anything is selected, back to `0x02` if not |
| `0x06` | carrying | `Village_Drop` (`0x004399B0`) sees the next **press**, and drops there |

So: **press, drag, release, then a second click** — not drag-and-drop. A press that never
travels nine pixels is a *click*, and `Village_ClickJob` (`0x0043A123`) turns that into the
job popup for whatever cluster it landed on.

Two rules inside the band that are not obvious from the outside: a band reaching into a
second cluster **abandons the whole selection**, and **shortfall icons cannot be picked up**
— `Village_BoxSelect` skips icon value 1 explicitly, because those figures stand for
workers the job wants and has not got.

### 6.4.2 Where a drop lands is a **painted file** **[V]**

Not a rectangle. `Village_GridAt` (`0x004398F5`) reads

```c
(&DAT_00542CF8)[((x - 0x40) >> 3) + ((y - g_villageTopY) >> 3) * 0x2D]
```

and `0x00542CF8` is `0x00542CE0 + 0x18` — preload entry 12, **`vill_gd8.pl8`**, past its
24-byte header. It closes three ways:

* `0x2D` is 45, and the tested x range `0x40 … 0x1A8` is 360 pixels — 45 cells of 8;
* the y range is 320 pixels — 40 cells of 8;
* **the shipped file is 1,824 bytes**, which is 24 + 45 × 40 exactly.

The byte is the cluster, 1-based, clamped to 8, and 0 is ground that belongs to nobody.
`vill.pl8` is one frame of 363 × 320 at `(0x40, g_villageTopY)`, and `g_villageTopY` is
**64**, or **132** with *Advanced Farming* — which is what makes room for `villtops.pl8`,
whose **six** 363 × 70 frames are the six weathers of `L2.eng` group 66, indexed by the same
county byte.

### 6.4.3 The icons are `Misc_cty` frames 0 … 0x16 **[V]**

`g_jobIconValue` (`0x004D6808`) is nine `i32` by labour slot: `4 8 10 14 16 18 20 22 2`. The
stored value is the frame **plus one**, and a selected icon adds one more, so each entry
names a *(normal, highlighted)* pair — and value 1 is the **shortfall** icon (frame 0, never
selectable) while value 2 is the **surplus** icon, which is also what every idle townsman is
drawn as.

That accounts for §9's guess that `Misc_cty` frames 0 … 0x16 were "almost certainly the top
menu bar". They are not. Nineteen of those twenty-three frames are 16 × 32 icons; the four
the table never names — **5, 6, 11 and 12** — are exactly the four frames in that range of
the shipped file that are **2 × 2 stubs**. Nothing is left over.

`Village_RebuildIcons` (`0x0045161E`) decides how many of each: `ceil(workers / popBand)`
normally, plus `ceil((wanted − workers) / popBand)` shortfall icons below the job's floor,
or `(workers − useful) / popBand` surplus icons above its ceiling **split off** the normal
count rather than added to it. The fill orders are three permutations in `.data` —
`g_iconFillOrder` (25 slots), and `g_iconFillOrderMain` + `g_iconFillOrderOther` for a
cluster showing both states, which partition the 25 slots as 13 + 12 with nothing over.

That single special case pins the industry order **[V]**: the village draws frame 0x2B (a
mine) on `+0x2AD` = industry **1** and frame 0x28 (a quarry) on `+0x2DD` = industry **3**,
at the same spot — so industry 1 is iron (slot 4, *"Iron mining"*) and industry 3 is stone
(slot 5, *"Stone quarrying"*); industry 0 is wood (frame 0x29, trees, on `+0x295`) and
industry 2 is weapons. That is the order `crates/l2-kingdom`'s `Commodity` already has,
confirmed from the artwork rather than from the production code.

Labour slot 0 is grain: `Grain_Sow(county, county[+0xC4], …)` passes slot 0 directly, so
`JOB_GRAIN_FARMING = 0` is right.

### 6.4.4 The village is an **inset**, and this document said otherwise  **[V]**

Kept rather than quietly edited, because the wrong version was in three documents and a
module header and it is worth knowing how it got there. `docs/decisions.md` C22 is the full
entry; this is what the section now claims.

> ~~It is a full screen, not a window over the county panels: it has its own painter, loads
> its own artwork, and neither draws the campaign sidebar nor calls `CountyStrip_Draw`.~~

Every clause of that is true and the conclusion does not follow, because **nothing in this
engine clears the screen** (§3.1). Not redrawing the sidebar means the sidebar is still
there. A player opened the game, clicked the town square and reported *"a dialog… still
being able to see the map around it and the rest of the screen"*, and he was right.

What the village actually covers:

| | rectangle |
|---|---|
| the picture — `vill.pl8` frame 0 | 363 × 320 at (64, `g_villageTopY`) |
| `villtops.pl8`, *Advanced Farming* only | 363 × 70 at (64, 64) |
| the band it saves and restores | **480 × 320 at (0, `g_villageTopY`)** — §3.1 |
| the tick | `Ui_OkButton(0x180, g_villageTopY + 0x118, 1)` |

`g_villageTopY` is 64, or 132 with *Advanced Farming*. The menu bar (y 0 … 23) and the
county sidebar (x 478 … 639) are outside all of it, and so is a strip of campaign map on
either side of the picture even inside the band.

**And the caller settles it without any of the above.** `g_screenId = 2` occurs **exactly
once in the binary**, in `Map_Click` (`0x0043CE1A`): the town-square branch does
`Map_CentreOnTile(county[+0x70])` and one `FUN_004050C0` — which is `Map_DrawFrame` —
*before* setting the screen. **The game recentres the campaign map on the town in order to
open the village over it.** `Village_Draw` then repaints the map itself, by the same route,
every time it is called with `reload != 0`.

### 6.4.5 Clicking the map is the whole of this navigation  **[V]**

`Map_Click` (`0x0043CE1A`) is 1,263 bytes and dispatches every left click on the campaign
map. `Map_ResolvePick` (`0x0046D5FE`) gives it four values — the picked county, that
county's owner, `g_pickedTileFlags` (the attribute plane at `0x00522F91`) and
`g_pickedTileGraphic` (`g_tiles[tile]`) — and then, in order:

| what was clicked | what happens |
|---|---|
| a unit of type 1 (an army) that is yours | its orders, or siege preparation (screen `0x1D`) |
| a unit of type 3 (a merchant) that is yours | the merchant (screen `0x08`), after centring on the town |
| flags bit **0x80** — an industry building | `Industry_ToggleFromMap` **switches that industry on or off** |
| flags bit **0x40** — the town square | **the village** (screen `0x02`) |
| flags bit **0x20** | screen `0x04` |
| any of those, in a county that is not yours | `Msg_Enqueue(…, 0x70, …)` |

The industry branch is worth its own line, because it is the writer of a byte this project
already depended on and could not place: `Industry_ToggleFromMap` (`0x0043D309`) XORs
`+0x297 + industry*0x18`, **the enable flag `Labour_Allocate` gates each mining job on**
(§14 of `docs/kingdom.md`). Which industry is decided by a ladder on the tile's *graphic*:
0 … 3 iron, 4 … 6 stone, 7 … 9 weapons, 10 … 12 wood, 13 … 20 nothing at all, 21 and up
castle building — and castle building toggles `+0x1B0` instead, the other gate in the same
allocator. So **a county's industries are switched on and off by clicking their buildings on
the campaign map**, and nothing on any county panel does it.

### 6.5 There is no sow control, and no harvest control

**[D]** `Panel_JobGrain` (`0x00413590`) *reports* what will be sown — group 77.1
*"to be sown, yielding"* from `+0x230`, and `+0x230 × g_grainYieldPerSack` *"in 4
seasons."* — and reports what is growing and when it will be harvested. Nothing on it is
clickable. Sowing and harvesting are consequences of the number of farmers and the number
of grain fields, and the season pipeline does them.

The **field types** are painted on the campaign map, not set from a county panel:
`Field_SetType` (`0x00438BEC`) is called from a map click with a brush id in
`g_uiHotspotId`. The five brushes and the two menus they sit in are
`docs/kingdom.md` §7.2.

**Ale is bought at the merchant** (screen 0x08), not from a county panel — `L2.eng` group
68.19 is *"Buy ale for your county, as a gift for its people."* and group 6.4 is *"Ale"*,
one of fifteen tradeable goods. The happiness it grants is `docs/kingdom.md` §4.4.

---

## 7. The history graph, and the array behind it

`Ui_HistoryGraph` (`0x004156A7`) draws the centrepiece of both the population and the
happiness panel: a recessed **402 × 155** box at (32, 84), a background from `graphs.pl8`
frame 0 or 1, and one bar per recorded turn, scaled so the tallest fills the box.

**This is the finding with the most weight for the simulation.** The bars come from a
per-county history array, and `crates/l2-kingdom` does not have one.

**[V]** Ghidra renders the read as `(&DAT_0056D8B8)[slot * 0x20 + county * 2]`, which is
wrong in a way that matters — `0x0056D8BC` is a live `void*` elsewhere in the binary, so
the two cannot be the same array, and taking the C at face value would have produced a
confident wrong stride. The instruction stream settles it:

```asm
a1 3c c9 57 00        MOV EAX, [0x0057C93C]          ; g_selectedCounty
8b 4d e0              MOV ECX, [EBP-0x20]            ; turn slot
c1 e1 07              SHL ECX, 7                     ; slot * 128
8b 84 c1 b8 d8 56 00  MOV EAX, [ECX + EAX*8 + 0x0056D8B8]
...
8a 94 c1 bc d8 56 00  MOV DL,  [ECX + EAX*8 + 0x0056D8BC]
81 7d e0 90 01 00 00  CMP [EBP-0x20], 0x190          ; wraps at 400
```

So the record is **128 bytes a turn — sixteen counties of eight — with population as a u32
at `+0` and happiness as a u8 at `+4`**, and because the county index is 1-based the array
really begins at `0x0056D8B8 + 8` = **`0x0056D8C0`**.

Three things confirm it, and none of them is the disassembly:

* **`g_saveBlocks` block 10 is `{0x0056D8C0, 51200}`**, and `400 × 16 × 8 = 51,200`. The
  base and the length both land exactly.
* Blocks 67 and 68 are `g_historyHead` (`0x00568DA8`) and `g_historyLength`
  (`0x00553F30`), four bytes each — the circular buffer's cursor and its fill.
* **The shipped `lastturn.sav` reads back correctly.** At the block's file offset,
  head = 0, length = 1, and turn 0 holds, for counties 1 … 14 in order:

  ```text
  435/72  456/77  456/77  435/72  456/77  456/77  456/77
  435/72  456/77  456/77  435/72  456/77  435/72  456/77
  ```

  — exactly `docs/kingdom.md` §9's population and happiness, owned counties 435/72 and
  unowned 456/77. Nothing but the right layout produces that.

---

## 8. What the panels show that we do not compute

Five things, in descending order of how much they matter.

1. **The 400-turn history.** §7. It is 51,200 bytes of the save, it is what both graph
   panels are *for*, and `l2-kingdom` keeps no history at all. Adding it is one array, one
   append at end of turn and one wrap at 400 — but until it exists, the population and
   happiness panels are two-thirds empty. **[V]**

2. **`taxHapOther` comes from a table, not from a formula.** *Fixed — this entry is kept
   for the record.* `crates/l2-kingdom`'s `tax::empire_contribution` was
   `min(5 − rate, 0)`, reasoned from the save. The binary's only writer of `+0x16` is
   `Tax_RecomputePreview` (`0x0044B80B`), and it reads `g_taxHappinessOther[rate]` (§6.3).
   The two agree at rate 0 — which is every rate in the England turn-one fixture, which is why the
   inference survived — and **disagree from rate 6 upward**: ours gave −1 at rate 6, the
   table gives 0 until rate 20 and only reaches −15 at rate 50, where ours would give −45.
   All 51 entries are now `l2_kingdom::tables::TAX_HAPPINESS_OTHER`, checked against the
   executable by `tools/oracle/kingdom.ps1`, and `5 − rate` survives as the separate
   `+0x0F` field it always was. **[V]**

3. **The labour record is three integers a job, not one.** `+0xC4 + slot*0x0C` is what
   `docs/kingdom.md` records and what we store — but the stride is 12 and
   `Panel_JobDetail` reads all three: it colours the worker count **red** when
   `+0xC4 + slot*12 < +0xC8 + slot*12`, and a second colour when
   `+0xCC + slot*12 < +0xC4 + slot*12`. So the second and third words are a wanted-workers
   floor and a useful-workers ceiling, and `Village_RebuildIcons` (`0x0045161E`) uses the
   same three to decide how many icons to draw in the "wrong" state. `l2-kingdom` has
   `labour: [i32; 9]` and no room for them. **[D]**

   *Fixed — this entry is kept for the record.* **All three words now import**, and the
   two that did not are a **wanted floor** (`+0x04`) and a **useful ceiling** (`+0x08`).
   Their writers are `Grain_LabourEstimate` (`0x0044D374`) and `Herd_LabourEstimate`
   (`0x0044DD4D`), which each walk `workers = 0 … population` and store the first count
   that stops the job going backwards; every other job writes −1 for the floor, and for the
   ceiling **100,000** where more workers always help (iron, stone, wood) or **0** where the
   county has no such resource.

   The ceiling is a rule's output rather than a panel's hint: `Labour_Allocate`
   (`0x0044F6E7`) fills each of slots 0 … 7 **up to it and no further**. The shipped save
   proves the reading — seven of the nine floors are −1 in all fourteen counties, wood's
   ceiling is exactly 100,000 in every owned county and exactly 0 in every unowned one, and
   `l2_kingdom::labour` rebuilds all fourteen counties' worker counts from those numbers
   alone. **[V]**

4. **Three "Fed" fields with no name.** `+0x16C`, `+0x170` and `+0x174` are drawn on the
   ration panel next to the sheep, grain and cattle icons, beside the two "Eaten" fields we
   do have (`+0x178`, `+0x17C`). They are how many people each food source fed. **[D]**

5. **`+0x22C`, `+0x230`, `+0x24C`, `+0x278` and `+0x2FC`** are read by the grain job popup
   as an overall change, sacks to be sown, the weather's effect on the store, the random
   event's effect on the store, and next season's harvest estimate. None of them is in
   `docs/kingdom.md` §1.3. **[D]**

Two smaller ones: the panels distinguish `rationWanted` from `rationAchieved` by colouring
the mismatch red, which needs both fields live (they are); and `Ui_DrawYear` prints
**BC/AD** from group 26, so the year is signed and we treat it as unsigned.

---

## 9. What this document does **not** establish

Named here so nobody mistakes silence for coverage.

* **The fonts.** `Fntl2_9`, `Fntl2_14` and `Fntl2_22` are identified as files and as
  buffers. Their glyph metrics, the width table at `0x004D71F0`, and the exact shadow
  colours in `Ui_DrawText` are read but not reproduced. Our text is still our own 5 × 7
  font, and `crates/l2-game` says so where it draws.
* **The palette indices.** Panels draw in colour `0x3F`, with `0xF9` for negatives and
  `0xFC` for a second warning state. Which actual colours those are depends on
  `base01.256`, which this document does not decode.
* ~~**Screens 0x04 and 0x1A**~~ *Half done — **0x1A is the seven diplomacy dialogs**, §10.4.
  **0x04 is still unread**; it is the map information panel, and the only thing established
  about it is which branch it takes. The two sidebar buttons at `0x00436A88` and
  `0x0043611B` are now known to open screens 0x1B and 0x0B — castle building and the other
  lords — but what they do first is not read.*
* ~~**The field-painting brush** on the campaign map: `Field_SetType` is seen, not
  understood.~~ *Done — `docs/kingdom.md` §7.2 and `crates/l2-kingdom/src/field.rs`. The
  brush is five 48 × 48 buttons in two hotspot tables at `0x004DC4D0` (three: fallow, grain,
  pasture) and `0x004DC530` (two: begin reclaiming, abandon), all five calling
  `FUN_00438B02`, which passes the button's id to `Field_SetType` as a raw terrain value.
  Read out of the executable, not inferred.*

  The three percentages at `+0x130`, `+0x134` and `+0x138` that used to be listed here with
  it **are not the brush's**, and they are not three. They are the first three of **eight
  job percentages** at `+0x130 + job*4` — the labour allocator's only instruction about
  where a county's people should go. `Labour_RecomputeShares` (`0x00450000`) rewrites them
  in two groups, jobs 0 … 2 and jobs 3 … 7, each renormalised to exactly 100, and both of
  the binary's own default setters close on both halves: 33 / 50 / 17 with 0 / 0 / 0 / 100 /
  0, and 33 / 50 / 17 with 40 / 15 / 15 / 15 / 15. **[V]**

  **Both readings were half right, and the caller is what separates them.** The function
  this section pointed at — `FUN_00450639`, 677 bytes, redistributing three percentages —
  is `Labour_ToggleShare(county, job, on, divisor)`, and its **only** caller is
  `Field_SetType`, which uses it to give *field reclamation* a share of the farm the moment
  the county has a field under reclamation and to take it away again when it has none. So
  the call really does come from painting a field; the thing it writes is labour. Its
  five-member twin `FUN_004502CA` does the same for the industry group and is called from
  `Industry_ToggleFromMap`. `g_shareTable` (`0x004D6768`) is `{100, 50, 33, 25, 20, 0, 5, 0}`
  — `100 / (n + 1)` for the entries either can reach. **[V]**
* ~~**The village's own layout.**~~ *Done — §6.4. The clusters, the icons, the drop grid and
  the three-state drag are read and reproduced. What is still not is
  **`Village_Animate`** (`0x00412421`) and its six counters at `0x004D2930 …`, which redraw
  smoke, water and a cart from `villani1`/`villani2` over the still scene.*
* ~~**`Misc_cty.pl8` frames 0 … 0x16.**~~ *Wrong — they are the **peasant icons**, not the
  menu bar. §6.4.3, and the four 2 × 2 stubs among them are exactly the four the icon table
  never names. This was the only **[I]** in this document that turned out to be false, and
  it was false because nobody had looked at the one screen that draws them.*
* **Groups 62 and 63** — *"Click on a food to swap its priority."*, *"Barrels swilled."*,
  *"A new field will be ready next season."* — describe a richer ration and field panel
  than the one that shipped, and **no call site in the decompiled corpus passes 62 or 63 as
  a group id**. Five of group 62's strings are literally `FREE`. It looks like dead
  content; the claim is bounded by "no *literal* group id in 2,452 decompiled functions",
  which does not exclude a computed one.

---

## 10. The menu bar, the two generic dialogs, and the option screens

Written after §9 listed *"screens 0x04 and 0x1A"* as unestablished and nobody had yet
followed the menu bar past the three words painted on it. Everything below was reached by
one method: **read the table, not the painter.** The interface is data — 24-byte widget
records with a function pointer in them, and 12-byte menu records with another — and a table
in `Lords2.exe` cannot be talked into agreeing with a story. Those pointers appear in no
instruction, so `xref.js` cannot see them; `tools/oracle/widgets.js` decodes them out of the
executable, and every table quoted below came out of it:

```bash
node tools/oracle/widgets.js menu 4dc428 3      # the menu bar and its three drop-downs
node tools/oracle/widgets.js widgets 4ddc10 4   # one widget table
node tools/oracle/widgets.js ref 434d33         # who *points at* a function
```

### 10.1 The menu bar is three tables of function pointers, and every one is accounted for **[V]**

`g_menuBarItems` (`0x004DC428`) is three 16-byte records. Read as shorts and dwords:

```text
  0x4DC428   x=10  measuredX=0  y=6  group=1   items=0x004DC360  count=4
  0x4DC438   x=10  measuredX=0  y=6  group=2   items=0x004DC390  count=5
  0x4DC448   x=10  measuredX=0  y=6  group=3   items=0x004DC3D0  count=7
```

`Ui_DrawMenuTitles` writes the measured right edge back into `measuredX`, and `Menu_HitTitle`
(`0x0040E00A`) hit-tests `x … measuredX` by a fixed 12-pixel height. A hit sends
`Menu_OpenDropdown` (`0x0040DECA`), which saves `g_screenId` into `g_menuPrevScreen`, sets
`g_screenId` to **0x32** and calls `Menu_SaveBackdrop`.

**Screen 0x32 is "a drop-down is open", and its painter only puts the background back.**
`Menu_SaveBackdrop` (`0x0040C8D1`) copies the band at (0, 24) with `g_spriteWidth` 100 and
`g_spriteHeight` 180 through `FUN_004B3F0A(buf, 0xF0)`; 100 dwords is 400 bytes a row and
400 + 240 = **640**, the screen stride. So the band is **400 × 180 at (0, 24)** — under the
menu bar, wide enough for the widest title and tall enough for seven 20-pixel rows.
`Menu_RestoreBackdrop` (`0x0040C928`) is the same numbers through the restore twin. This is
the §3.1 pattern again: a screen id that owns a rectangle and nothing else.

Each drop-down item is **12 bytes**: `{short y; short stringIndex; void (*handler)(); int 0}`.
The three `items` pointers above land exactly on the first record of each run, which is the
independent check — the run boundaries were derived from the handler addresses first and the
pointer agreed afterwards.

| menu | y | `L2.eng` | caption | handler | what it does |
|---|--:|---|---|---|---|
| File | 0 | 1.1 | New Game | `Menu_NewGame` | confirmation prompt 1 |
| | 20 | 1.2 | Load | `Menu_LoadGame` | globs `*.sav` → screen 0x35 |
| | 40 | 1.3 | Save | `Menu_SaveGame` | → screen 0x36 |
| | 60 | 1.4 | Quit | `Menu_Quit` | confirmation prompt 0 |
| Options | 0 | 2.1 | Advanced | `Menu_AdvancedOptions` | → 0x39 |
| | 20 | 2.2 | Sounds | `Menu_SoundOptions` | → 0x42 |
| | 40 | 2.3 | Display | `Menu_DisplayOptions` | → 0x43 |
| | 60 | 2.4 | Game Speed | `Menu_GameSpeed` | slider on `g_optGameSpeed` |
| | 80 | 2.5 | Scroll Speed | `Menu_ScrollSpeed` | slider on `g_optScrollSpeed` |
| Help | 0 | 3.1 | Game Help | `Menu_GameHelp` | → 0x31 |
| | 20 | 3.2 | How do I... | `Menu_HelpHowDoI` | message 0x123 |
| | 40 | 3.3 | Grow grain? | `Menu_HelpGrowGrain` | message 0x124 |
| | 60 | 3.4 | Build a castle? | `Menu_HelpBuildCastle` | message 0x125 |
| | 80 | 3.5 | Make Weapons? | `Menu_HelpMakeWeapons` | message 0x126 |
| | 100 | 3.6 | Manage each turn.? | `Menu_HelpManageTurn` | message 0x127 |
| | 120 | 3.7 | About | `Menu_About` | → 0x25 |

Four items, five items, seven items; groups 1, 2 and 3 hold exactly four, five and seven
strings after their label. The `y` column is 0, 20, 40, … with no gaps, the string indices
are 1, 2, 3, … with no gaps, and the five help topics use five **consecutive** message ids.
Nothing here was chosen by us.

**`Menu_ScrollSpeed` is the row that proves the table.** It passes `&g_optScrollSpeed` — a
global named earlier for unrelated reasons — and the caption above it is *"Scroll Speed"*.
That is a check that could have failed and did not.

### 10.2 The four option screens, and the twelve rows behind them **[V]**

Each painter is the same shape: `FUN_004093E0(x, y, w, h)` for the box, group index 0 as the
heading, indices 1 … n as rows 32 pixels apart, and beside each row a **Yes/No from group 18**
or an **On/Off from group 19** chosen by one global. Then the painter writes its own **row
count** into a global that `Screen_DrawWidgets` and `Screen_HandleInput` read back.

| screen | painter | group | rows | widget table | count global |
|---|---|--:|--:|---|---|
| 0x39 advanced | `Screen_AdvancedOptions` | 50 | 4 | `g_advancedOptWidgets` | `g_advancedOptWidgetCount` |
| 0x42 sound | `Screen_SoundOptions` | 51 | 3 | `g_soundOptWidgets` | `g_soundOptWidgetCount` |
| 0x43 display | `Screen_DisplayOptions` | 52 | 2 | `g_displayOptWidgets` | `g_displayOptWidgetCount` |
| 0x31 help | `Screen_HelpOptions` | 45 | 3 | `g_helpOptWidgets` | `g_helpOptWidgetCount` |

**The geometry closes to the pixel.** The advanced painter draws its labels at y = 160, 192,
224 and 256; its four widget records sit at y = 156, 188, 220 and 252 — each label y **minus
four**, the same offset on all four screens, and 24-pixel buttons whose caption baseline sits
4 below their top. Sound: labels 160/192/224, widgets 156/188/220. Display: labels 208/240,
widgets 204/236. Help: labels 192/224/256, widgets 188/220/252.

| row | caption | global | handler |
|---|---|---|---|
| 50.1 | Advanced farming | `g_optAdvancedFarming` | `Opt_ToggleAdvancedFarming` |
| 50.2 | Army foraging | `g_optArmiesEat` | `Opt_ToggleArmyForaging` |
| 50.3 | Exploration | `g_optExploration` | `Opt_ToggleExploration` |
| 50.4 | Fight humans only? | `g_optFightHumansOnly` | `Opt_ToggleFightHumansOnly` |
| 51.1 | Music | `g_optMusic` | `Opt_ToggleMusic` |
| 51.2 | Sound effects | `g_optSoundEffects` | `Opt_ToggleSoundEffects` |
| 51.3 | Speech | `g_optSpeech` | `Opt_ToggleSpeech` |
| 52.1 | Animations | `g_optAnimations` | `Opt_ToggleAnimations` |
| 52.2 | Full screen | `g_optFullScreen` | `Opt_ToggleFullScreen` |
| 45.1 | Tip screens | `g_optTipScreens` | `Opt_ToggleTipScreens` |
| 45.2 | Tool tips | `g_optToolTips` | `Opt_ToggleToolTips` |
| 45.3 | Start game help | — | `Opt_GameHelpContents` |

Five of these have a second, independent anchor, which is why the block is **[V]** and not
**[D]**:

* **`g_optAdvancedFarming` and `g_optArmiesEat` were already named**, for farming reasons, and
  they land on rows 1 and 2 of a group whose captions are *"Advanced farming"* and
  *"Army foraging"*.
* **`g_optFullScreen`**: group 52 index 3 is *"(F5 key re-sizes window to 640x480)"* and the
  painter draws it **only while the flag is 0**. A hint about the window appears exactly when
  there is a window.
* **`g_optAnimations`**: `Screen_BattleOutcome`, four screens away, branches on the same flag
  to draw a taller box with an inset animation panel.
* **`Opt_GameHelpContents`** calls `WinHelpA(hwnd, "l2help.hlp", HELP_CONTENTS, 1)` — an
  import, which is not a matter of opinion.

**The four advanced rules are frozen in a network game.** Each of their four handlers is
`if (g_multiplayer == 0) { flip } else { tip 0x32; g_screenId = g_menuPrevScreen; }`.
`Opt_ToggleArmyForaging` additionally re-runs `Ration_Apply` and `County_RefreshEstimates`
over every county, because the rule changes this turn's food.

`g_optFightHumansOnly` is **stored inverted**: the painter shows *"Yes"* when it is 0, and
when it is 0 — and the local player is not a participant — the battle resolver skips the
prompt on screen 0x12 and jumps straight to the result on 0x13.

### 10.3 Two dialogs serve the whole game **[V]**

Neither is a screen anybody wrote by hand; both are opened with arguments.

**The yes/no box, screen 0x1E.** `Ui_OpenConfirm(prompt, x, y, onAnswer)` saves the screen,
stores its four arguments and paints a 14 × 8 cell box at (x − 16, y − 16) with **`L2.eng`
group 10 index `prompt`** inside it. `g_confirmWidgets` is a tick at (64, 46) and a cross at
(112, 50) — frames 29 and 31, hotspot ids 1 and 0 — and both call `Ui_ConfirmClicked`, which
writes the hotspot id into `g_confirmAnswer` and then calls the stored `g_confirmCallback`.
Group 10 is fifteen strings and they are the whole game's confirmations: *"Exit the game?"*,
*"Start a new game?"*, *"Overwrite File?"*, *"Create this army?"*, *"Slaughter villagers?"*,
*"Combine armies?"*, *"Disband army?"*, *"Garrison castle?"*, *"Besiege castle?"*,
*"Autocalc battle?"*, *"Destroy field?"*, *"Surrender castle?"*, *"Retreat from field?"*,
*"Lift the siege?"*. `Menu_Quit` passes 0 and `Menu_NewGame` passes 1.

**The value spinner, screen 0x21.** `Ui_OpenSlider(prompt, value, step, max, min, x, y, format)`
paints a 15 × 7 box with **group 12 index `prompt`** as the caption and group 12 index 0,
*"Click Right to Exit"*, under it. Two arrow widgets, frames 35 and 37, call `Ui_SliderUp`
and `Ui_SliderDown`, which move `*g_sliderValue` by `g_sliderStep` between `g_sliderMin` and
`g_sliderMax`. `format` 1 divides by ten for display, 3 appends a per cent sign. Group 12 is
*"Adjusting game speed"*, *"Adjusting scroll speed"*, *"Adjusting music level"*,
*"Adjusting sound level"*, *"Number of samples"* — and `Menu_GameSpeed` passes 1 with
`&g_optGameSpeed`, `Menu_ScrollSpeed` passes 2 with `&g_optScrollSpeed`.

### 10.4 Screen 0x1A is the seven diplomacy dialogs **[V]**

§1 recorded 0x1A as *"not identified"*. `Screen_DiploDialog` (`0x0041789B`) is a seven-arm
chain on `g_diploKind`, and every arm draws `L2.eng` group **72**:

| kind | painter | group 72 | the dialog |
|--:|---|---|---|
| 0 | `Diplo_DrawGiftGold` | 10 *"Send gift of gold to"*, 23 *"Last gift was"*, 18 *"Gift of"*, 17 *"Dispatch ?"* | a gift of gold |
| 1 | `Diplo_DrawLetter(0)` | 11 *"Give a compliment to"* | a compliment |
| 2 | `Diplo_DrawLetter(1)` | 12 *"Insult"* | an insult |
| 3 | `Diplo_DrawLetter(2)` | 13 *"Ask for an alliance with"* | offer an alliance |
| 4 | `Diplo_DrawLetter(3)` | 14 *"End alliance with"* | end an alliance |
| 5 | `Diplo_DrawCountyRequest(0)` | 15 *"Plead for help from"*, then 19 or 21 | ask an ally for help |
| 6 | `Diplo_DrawCountyRequest(1)` | 16 *"Plan strategic attack with"*, then 20 or 22 | ask an ally to attack |

The two request dialogs draw index 19 / 20 — *"Choose the county you want help in."* /
*"...attacked."* — while `g_pickedCounty` is 0, and index 21 / 22 plus the county's name from
group 100 once one is picked. The prompt and the state agree with each other.

**Where they are opened from closes the loop.** `g_diploWidgets` (`0x004DD940`) holds six
records at (400, 102 + 50n), and their handlers are exactly the six openers above, **in the
order of group 72 indices 2 … 8**: *Dispatch a gift*, *Send a compliment*, *Send an insult*,
*Offer an alliance* / *Terminate alliance*, *Ask ally for help*, *Ask ally to attack*. Six
widgets for seven captions because one widget covers both alliance rows — and
`Diplo_OpenAlliance` picks kind 4 over kind 3 exactly when the target is already this realm's
ally.

### 10.5 Conventions that hold across every table read here **[V]**

Worth writing down because they turn an unread widget table into a legible one:

* **Button sheet frame pairs.** 29 / 31 is tick and cross, 35 / 37 a scroll pair, 68 / 66 a
  minus and plus, 21 / 23 an up and down. Every tick/cross pair found sits at (x, y) and
  (x + 40, y + 4) — the cross is four pixels lower, on all six screens that use one.
* **Hotspot id 1 is confirm, 0 is cancel.** Both halves of a pair share one handler and read
  `g_uiHotspotId` to find out which was pressed. `Ui_ConfirmClicked`, `Diplo_SendClicked`,
  `SendSupplies_Close`, `CastleBuild_Close` and `SmackTest_Close` are all this shape.
* **A scroll widget's hotspot fields are its arguments.** `SaveLoad_Scroll` is one function
  for two lists: the save/load box passes deltas −3 and +3 with list id 1 and a 15-row window,
  the skirmish box −1 and +1 with list id 2 and a 5-row window.
* **The painter publishes its own widget count.** The four option screens, the merchant, the
  armoury, the court, diplomacy and the trade panel each write a row count into a private
  global that the widget and input passes read back. A screen's widget list is therefore not
  a constant, and reading only the table understates it.
* **`g_redrawRequest` (`0x0057D340`) is how any of this reaches the screen.** Every handler
  ends by writing 2 to it; the main loop is
  `if (g_redrawRequest != 0) { Screen_Draw(g_redrawRequest); g_redrawRequest = 0; }`, so the
  value it was given becomes `Screen_Draw`'s `firstFrame` argument. It has exactly one reader.

### 10.6 `anchor.js screens` had six screen ids wrong, and it is fixed **[V]**

The first run of `node tools/oracle/anchor.js screens` reported the merchant at id **0x62**,
the court at **0x74**, the armoury at **0x6E**, the other lords at **0x76**, trade goods at
**0x66** and the campaign map at **0x30** — every management screen the game has. The
decompiler prints those cases as `g_screenId == '\b'`, `'\t'`, `'\n'`, `'\v'`, `'\f'` and
`'\0'`, and `litNum`'s regex swallowed the backslash and read the *letter*: `'b'` is 0x62,
`'t'` is 0x74, `'0'` is 0x30. The fix is a proper C escape table; the check is `Screen_Draw`'s
39 arms read by hand, which now agree with the tool on all 43 ids.

This is the failure mode a hypothesis generator is *most* dangerous in: the output was
self-consistent, three columns joined across three dispatchers, and simply displaced. It cost
nothing here only because §1 of this document already had the right ids to disagree with.

### 10.7 `0x35` / `0x36` — the save and load box, whole **[V]**

Read out of `Screen_SaveLoad` (`0x00414819`) and `SaveLoad_DrawStatus` (`0x004149EC`) while
wiring them up, and both screens are implemented now rather than shelled
(`crates/l2-game/src/screens/saveload.rs`).

**One painter, one flag.** `Screen_SaveLoad(saving)` uses its argument as the `L2.eng`
group 40 string index, so `0x35` draws index 0 *"Loading a conquest."* and `0x36` index 1
*"Saving a conquest."* and nothing else about them differs. `SaveLoad_DrawStatus` is shared
with the front end's page 3 (`FUN_004148E4`) and switches its origin on a flag: the box is
at `(0x10, 0x90)` in game and `(0x60, 0x0A)` on the front end. Everything below is an
offset from that origin `o`.

| what | call | in game |
|---|---|---|
| window | `Ui_DrawBox(0x10, 0x90, 0x1C, 0x14)` | (16, 144), 448 × 320 |
| heading | `Eng_DrawString(40, saving, 0x20, 0xA0, heading, 0x3F)` | (32, 160) |
| the lower area | `Ui_DrawInsetRect(0x20, 200, 400, 0x100)` | (32, 200), 400 × 256 |
| name field | `Ui_DrawInsetRect(0x28, 0xD0, 0xC0, 0x20)` | (40, 208), 192 × 32 |
| file list | `Ui_DrawInsetRect(0x28, 0xF8, 0x160, 0xA4)` | (40, 248), 352 × 164 |
| status line | `Ui_DrawInsetRect(0x28, 0x1A4, 0x180, 0x1C)` | (40, 420), 384 × 28 |
| the name being typed | `Ui_DrawText(DAT_004EA130, o.x + 0x20, o.y + 0x48)` | (48, 216) |
| list interior | `Ui_DrawBoxInterior(o.x + 0x1E, o.y + 0x6A, 0x15, 10)` | (46, 250), 336 × 160 |
| status text | `Eng_DrawString(40, 2 / 3 / 4, o.x + 0x20, o.y + 0x11A)` | (48, 426) |

**The list is three columns of ten.** The loop starts at `(o.x + 0x20, o.y + 0x6C)` =
(48, 252), steps x by `0x78` twice, then resets x and steps y by `0x10`, and breaks once it
has drawn thirty — which is exactly the ten 16-pixel rows the interior above covers. The
names come from a table of **65-byte records** at `0x004E8790`, indexed `base + i * 0x41`,
so a save name is at most 64 characters. The selected row is a 6 × 16 mark at `(x − 2,
y − 1)` and its text in colour `0x20` rather than `0x3F`; the status text is drawn only
while `DAT_0057D3C4` is set, so the line is blank until something is happening.

**The scroll clamp is off by half a page in the original.** `SaveLoad_Scroll` clamps the top
row at `g_fileListCount − 15` and zeroes it below 30 entries, while thirty are on screen —
so a list of, say, twenty scrolls into empty space. Recorded, not reproduced.

**The four widget records are box-relative, and that is `[I]`.** `g_saveLoadWidgets`
(`0x004DDD78`) holds a tick at (304, 64) frame 29, a cross at (352, 64) frame 31, and the
list's two scroll arrows at (384, 144) and (384, 176), frames 35 and 37, carrying the
deltas −3 and +3 with list id 1. Read as absolute screen coordinates all four sit above or
on the top edge of a window that begins at y = 144, which would put the tick and cross
outside the panel they belong to. Read relative to the box origin they land at (320, 208)
and (368, 208) — level with the name field, whose rectangle ends at x = 232 — and at
(400, 288) and (400, 320), immediately right of the list, whose rectangle ends at x = 392.
The deciding argument is that **one table serves two screens whose boxes are at different
origins**, which an absolute table cannot do. It is still an inference; it is marked as one
in the module that acts on it.
