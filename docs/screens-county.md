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

**[D]** `g_screenId` (`0x004EAC50`) selects the whole interface. **Four** parallel
`if`/`else if` chains switch on it and on nothing else:

| what | function | what it does |
|---|---|---|
| draw | `Screen_Draw` (`0x0040F1A0`) | 39 cases; calls the screen's painter |
| overlay | `Screen_DrawWidgets` (`0x004BA26E`) | per-screen widget lists and animations |
| input | `Screen_HandleInput` (`0x004BA9C8`) | per-screen widget hit-test tables — **left button only** |
| **input, late** | **`Screen_FrameInput` (`0x0042FF10`)** | 49 hand-written arms over 50 ids: the right button, the corner picture, and everything the widget tables cannot express. **How every screen is left.** §2.6 |

An earlier revision of this section said there were three and that nothing else dispatched on
`g_screenId`. The fourth is the largest of them and the only one that reads the right mouse
button at all, which is why right-click looked like it did nothing in the original.

The cases, named from the `L2.eng` groups each painter draws and the PL8 files each loads.
**[V]** for every row that names a group; **[D]** for the rest.

| id | painter | screen | evidence |
|---:|---|---|---|
| 0x00 | `Screen_DrawCampaign` `0x0040F5FD` | the campaign map | group 34 — season and year |
| 0x02 | `Village_Draw` `0x00412143` | **the village** — the county's own picture, and where peasants are moved. **An inset over the campaign map**, 363 × 320 at (64, 64) — §3.1 | groups 22 (fertility), 66 (weather); `villani1/villani2/vill/villtops.pl8` |
| 0x04 | `0x0041B032` | the map information panel: `UnitPanel_Draw` when a unit is picked, `FUN_0041BEFE` otherwise. **Reached by right-clicking the map** — §2.6 — and `Readme.txt`'s "right-click an army for its county of origin" errata is `UnitPanel_Draw`'s group 31 index 9 line | group 31 index 9, *"An army from"*, then group 100 at `homeCounty + scenarioIndex*20` |
| 0x05 | *(no painter)* | **the village's rubber band** — §6.4 | `Village_BandStart` / `Village_BandRelease` |
| 0x06 | *(no painter)* | **the village carrying a selection** — §6.4 | `Village_Drop` |
| 0x08 | `Screen_Merchant` `0x00415FB7` | the merchant | `merchant.256` + `merchant.pl8`, `mercgrid.pl8` |
| 0x09 | `Court_Draw` `0x00416925` | **the court** — the realm's treasury and stores | group 70 |
| 0x0A | `Screen_Armoury` `0x00417EA7` | **the armoury** — the realm's weapons hanging on the walls, the eight troop racks along the bottom, and **Create / Change / Cancel**. `Army_RaiseConfirm` is a hotspot *here*, not on `0x17`. `docs/armies.md` §6.2a | `armoury.256` + `armoury.pl8`, `arm_grid.pl8`, `arm_it_<colour>.pl8`; group 69 |
| 0x0B | `Diplo_DrawScreen` `0x00416CF3` | the other lords, and the menu of what to send one | `faces.pl8`; group 72 |
| 0x0C | `Screen_TradeGoods` `0x00416308` | trade goods | group 68; `merchant.pl8` **again** as the background, then `icontrad.pl8` |
| 0x0D | `Armoury_LoadScreen` `0x004184C6` | **one weapon's rack** — its 24-frame picture, its count and the four buttons that move men one at a time. `Screen_Draw` has **no** arm for it: `Armoury_ClickRack` paints it once on the way in, and only the widget and input passes run afterwards. **Nothing here is bought** | `arm_<weapon>.pl8`; `g_armouryBuyWidgets`; group 69 index 5, group 8 nouns |
| 0x0F | `Panel_JobDetail` `0x00412B33` | **the job popup** — one of nine jobs, its workers and its output | group 74 |
| 0x11 | `Screen_ArmyDivision` `0x004192B1` | **army division** — the levy basket reused, parent from `slot.chosen` and daughter from `slot.available`, row 7 the mercenary band | group 17; `icon_tmp.pl8` |
| 0x14 | `Panel_Population` `0x004110B1` | **population** | group 73 |
| 0x15 | `Panel_Tax` `0x0041152F` | **tax** | group 86 |
| 0x16 | `Panel_Happiness` `0x004116FB` | **happiness** | group 85 |
| 0x17 | `Screen_RaiseArmy` `0x00418653` | **raise an army** — the levy slider, the six weapon stocks and the mercenary offer, **drawn over the armoury**: the arm is `if (firstFrame == 1) Screen_Armoury(1); Screen_RaiseArmy();`, so this is a window on `0x0A` in `armoury.256`, and its only way forward is *Continue*. There is no separate mercenaries screen; `docs/decisions.md` C45, C61 | groups 16, 18, 69, 100 |
| 0x18 | `Screen_SendSupplies` `0x0041AD5D` | send supplies to another county | group 33 |
| 0x19 | `Panel_Ration` `0x00411B72` | **rations** | groups 20, 21, 87 |
| 0x1A | `Screen_DiploDialog` `0x0041789B` | **the seven diplomacy dialogs**, on `g_diploKind` — §10.4 | group 72 |
| 0x1B | `Screen_CastleBuild` `0x00419789` | **the castle chooser** — five picture buttons and an OK, and the only way a castle is ever ordered. §11 | group 71; `cas_back.256` + `cas_back.pl8`, `caspics.pl8`, `cas_bits.pl8` |
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

| what | where | font | colour | field |
|---|---|---|---|---|
| county name, centred in 160 px | (480, 165) | **body, `Fntl2_14`** | `0x3F` | `L2.eng` group 100, index `scenarioIndex*20 + countyId` |
| population, **left-aligned** | (508, 189) | small | `0x3F` | `+0x24` |
| happiness, **left-aligned** | (602, 189) | small | `0x3F` | `+0x0C` |
| *"Tax"*, centred in 76 px | (480, 213) | small | `0x3F` | group 61 index 0 |
| tax rate, with a `%` | (506, 226) | small | `0x3F` | `+0xB9` |
| *"Ration"*, centred in 76 px | (564, 213) | small | `0x3F` | group 61 index 1 |
| ration achieved, centred in 76 px | (564, 226) | small | `0x3F` / `0xF9` | `+0x15D` as group 21, **red when it differs from `+0x15E`** |
| health thermometer | (552, 181) | — | — | `Misc_cty.pl8` frame `0x46 + healthBand`, 14 × 59 |

The 9-pixel font (`Fntl2_9.pl8`) is used here and **nowhere else in the game** — but not for
all of it. The county name is the 14-pixel body font, and `DAT_005AEA40` is set to 1 after
it and cleared after the numbers, so **the name is embossed and every number below it is
flat**. An earlier revision of this table said "all of it in the 9-pixel font" and gave both
numbers as bare coordinates; the 602 was then read as a right anchor. `Ui_DrawNumber` has no
anchoring argument — the population's call and the happiness's differ only in value and x —
so both are left origins. `docs/decisions.md` C42.

Verbatim, so nothing here has to be re-derived:

```c
Pl8_DrawFrameHere(g_miscCtySheet,0x37,0x1de,0x9c);      /* 55, 162x94  -> (478,156) */
Ui_DrawCentred(100, g_scenarioIndex*0x14 + g_selectedCounty, 0x1e0,0xa5,0xa0,&g_fontBody,0x3f);
DAT_005aea40 = 1;                                        /* emboss off */
Ui_DrawNumber(pop,       ' ', " ", 0x1fc,0xbd,&g_fontSmall,0x3f);
Ui_DrawNumber(happiness, ' ', " ", 0x25a,0xbd,&g_fontSmall,0x3f);
Ui_DrawCentred(0x3d,0, 0x1e0,0xd5,0x4c,&g_fontSmall,0x3f);              /* "Tax"    */
Ui_DrawNumber(taxRate,   ' ', "%", 0x1fa,0xe2,&g_fontSmall,0x3f);
Ui_DrawCentred(0x3d,1, 0x234,0xd5,0x4c,&g_fontSmall,0x3f);              /* "Ration" */
local_8 = (rationAchieved == rationWanted) ? 0x3f : 0xf9;
Ui_DrawCentred(0x15, rationAchieved, 0x234,0xe2,0x4c,&g_fontSmall,local_8);
DAT_005aea40 = 0;
Pl8_DrawFrame(g_miscCtySheet, healthBand + 0x46, 0x228,0xb5);           /* 14x59 -> (552,181) */
```

Group 100 is **twenty strings per map slot**: index 0 is the map's own name ("Here Be
Dragons!" for England), 1 … 14 its counties, 15 … 19 unused `CTY0` padding, so slot 1 begins
at index 20. `scenarioIndex * 20 + countyId` lands on the county with no off-by-one.

**Two more things this plate carries**, neither of which was in this section:

* **The 162 × 52 plate below it** — `Misc_cty` frame `0x42` (66) at (478, 250) — is drawn by
  `CountyStrip_Draw`, not by `Screen_DrawCampaign`, and it holds the **farm/industry labour
  split slider**: thumb frame `0x3D` (9 × 33) at (`share/2 + 532`, 262), or frame `0x55`
  (13 × 37) two pixels up and left **when the county has idle townsfolk** — §2.1.2.
  `FUN_00439122` hit-tests `x 478 … 639, y 257 … 296`: left of x = 531 steps the share down
  by four, right of x = 594 up by four, and on the track it is `((x - 531) * 2) & 0xFC` —
  masked, so the slider is not continuous — clamped to 0 … 100. **It is a drag** — §2.1.1.
* **The job rows** on the 162 × 128 plate at y = 302, laid out by `FUN_0040FEC1` into up to
  three farm rows and four industry rows at row heights 60 / 45 / 30 depending on the count,
  and clicked by `CountyStrip_JobClick` (`0x00438E3B`): farm left of x = 560, industry right
  of it, `row = (y - 302) / rowHeight`, opening screen `0x0F`.

### 2.1.1 The split slider is a **drag**, and three flags say so **[V]**

`FUN_00439122` is not a click handler. Its guard, verbatim:

```c
if (g_mouseLeftReleased == 0) {              /* DAT_004E65D8 — the up edge   */
    if (g_mouseLeftDown == 0)      return 0; /* DAT_004E65CC — the level     */
    else if (g_mouseMoved == 0)    return 0; /* DAT_004EA4B0                 */
    else                           ...set the share...
} else return 1;                             /* a release is eaten, not used */
```

Those globals are named by the frame poll at `0x004B2D5A`, which derives every one of them
from the window procedure's messages: `WM_LBUTTONDOWN` / `WM_LBUTTONUP` set and clear
`DAT_004EABC2 & 1`, from which the poll computes the level (`DAT_004E65CC`), the down edge
(`DAT_004EAFB4`), the up edge (`DAT_004E65D8`); `DAT_004EA4B0` is set whenever the pointer
moved or a button changed this frame.

So: **held and moved, every frame, and nothing on the release.** Press-and-track, not the
village's press-nine-pixels-release-click machinery of §6.4.1, and `g_screenId` is not
touched anywhere in the function. It is tested on the campaign map *and* on the village, in
that order, so it keeps working with the village inset open.

### 2.1.2 The blue outline: eleven frames, `0x4B` … `0x55` **[V]**

A player who had played the original reported *"there's no 'blue outline' for idle peasants
(eg too many on dairy)"* and *"the peasant slider I think had a blue outline if there were
idle peasants as well."* Both are in `CountyStrip_Draw`, and they are **two different
tests**.

**What the outline is.** Eleven frames of `Misc_cty.pl8`, a contiguous run. Each is drawn
two pixels up and left of the plain frame it replaces, and ten of the eleven are exactly
four pixels wider and four taller — a two-pixel ring around an unchanged picture. Every
non-transparent pixel of that border is one of **three palette entries of `Base01.256`, all
blue**: `95` = `rgb(0,0,121)`, `65` = `rgb(157,202,234)`, `64` = `rgb(194,230,255)`.
`crates/l2-view/tests/install.rs` asserts both properties against the shipped file.

| plain | ringed | what | condition |
|---|---|---|---|
| `0x21` | `0x4B` | the sheaf — grain, slot 0 | `labour[0].useful < labour[0].workers` |
| `0x26` | `0x4C` | the cow — cattle, slot 1 | `labour[1].useful < labour[1].workers` |
| `0x3F` | `0x4D` | field reclamation, slot 2 | `labour[2].useful < labour[2].workers` |
| `0x40` | `0x4E` | the castle, slot 3 | `labour[3].useful < labour[3].workers` |
| `0x30 + n` | `0x4F + n` | the industry, slot 7 | `labour[7].useful < labour[7].workers` |
| `0x3D` | `0x55` | **the split slider's thumb** | `labour[8].workers != 0` |

**The two tests are not the same one.** A produce icon is ringed when *that job* has more
people on it than its own useful ceiling; the slider is ringed when *anybody at all* in the
county is idle. Merging them is the mistake this section exists to prevent — and
`crates/l2-game` had the second one written down as *"when the castle job has workers"*,
which is slot 3 rather than slot 8.

**It is the other end of a record already in use.** `labour[slot]` is three `i32` — workers,
a wanted floor, a useful ceiling (§8, `docs/kingdom.md` §14). `Panel_JobDetail` colours the
worker count red **below** the floor; the ring marks **above** the ceiling; and
`Village_RebuildIcons` (§6.4.3) uses the same two words to decide which peasants in the
village are drawn as the seated *idle* figure rather than as the job's own.

**Two exceptions worth having written down.** The castle's `0x4E` is 32 × 34 against
`0x40`'s 23 × 26 and is drawn six left and three up — it is a *different, larger picture*
that also carries the ring, not the plain one inside one. And the industry pair is indexed
`n` by a county byte at `+0x290` that is not named here; the file has six of each
(`0x30` … `0x35`, `0x4F` … `0x54`), so `n` reaches 5.

Three of the five right-hand rows — iron (`0x2C`), stone (`0x2D`) and wood (`0x2E`) — have
**no** state test at all, and that follows: those three jobs are the ones whose ceiling is
100,000, so nobody can ever be past it.

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

| button | x range | sets | action |
|---|---|---|---|
| 1 | 478 … 510 | `g_screenId = 0x17` | **raise an army** — `Levy_SetPercent(sel, g_levyPercent)`, `FUN_004AA90A(sel, g_levyMen)`, and the mercenary band loaded on top when `county.mercenaryOffer != 0`. Refused with message `0x70` if the county is not yours |
| 2 | 512 … 542 | `= 0x09` | the court. Ungated |
| 3 | 544 … 574 | `= 0x18` | send supplies. Gated on ownership |
| 4 | 576 … 606 | `= 0x1B` | **castle building** (`FUN_00436A88`). Gated |
| 5 | 608 … 638 | `= 0x0B` | **the other lords** (`FUN_0043611B`). Ungated |
| end turn | 478 … 639, y 460 … 479 | — | `Turn_End`, which writes 999 into the realm's `+0x00` |

The table's five rectangles start at x-offsets 0, 34, 66, 98, 130 and end at 33, 65, 97,
129, 161, and `Hotspot_Test` (`0x0040E3EE`) is **half-open** — `x0 + off <= mx < x1 + off` —
so the widths are 33, 31, 31, 31, 31 with a one-pixel dead column between each pair.
`34 + 32 × 4 = 162`. It closes.

**`Hotspot_Test` is half-open on `y` as well, and that costs a row.** The reject is
`my < y0 + oy || y1 + oy <= my`, and the five records are `(x, 0) … (x, 29)` at the `0x1AE`
offset — so the strip is **y 430 … 458, 29 pixels tall**, and *y 459 is dead*: the same
one-pixel gutter the table leaves horizontally, once, across the whole strip. End turn is
record 5, `(0, 30) … (161, 49)` — **161 × 19**, so its last column (x 639) and its last row
(y 479) are dead too. The **plate** `Misc_cty` frame 59 really is 162 × 20 and the strip
above it really is 162 × 30; the hotspots are one smaller in each direction, and deriving
the hit box from the plate is what put a live pixel where the game has none.
`crates/l2-game/tests/right_column.rs` reads both tables out of the player's own
`Lords2.exe` and asserts our constants against them. `docs/decisions.md` CNEW-dead-row.

Screen `0x17` is the **raise-army** screen and not merely the mercenary offer: `L2.eng`
group 69 index `0x10`, which `Screen_RaiseArmy` (`0x00418653`) draws as its heading, reads
*"Raising an army in"*, and the mercenary band is a conditional sub-panel worth three extra
window rows. `crates/l2-game/src/screens/shells.rs` calls it "Hire mercenaries", which names
the smaller half.

The strip also carries **no text of its own**. The only caption in the bottom fifty pixels
is the End Turn one, `Ui_DrawCentred(4, 0, 0x1DE, 0x1CE, 0xA2, &g_fontSmall, 0x16)` — group
4, centred in 162 at (478, 462), in the 9-pixel font — and it is **suppressed once the turn
has been ended**, because `Screen_DrawEndTurn` (`0x0041A734`) guards it on
`g_realms[g_localPlayer].aiStep < 999`. `docs/decisions.md` C43 is what we had there
instead.

### 2.5 The four minimap mode buttons  **[V]**

`Minimap_Draw` (`0x00410AA9`) draws a 29 × 123 strip — `Misc_cty` frame `0x5C`, or `0x5B`
whenever a mode is active — at (611, 32), and `g_minimapModeButtons` (`0x004DC620`) is four
hotspots tested at offset (610, 32) by `FUN_0043292D`. All four go to
`Minimap_ModeButton` (`0x0043AB76`):

| id | rect | what |
|---:|---|---|
| 1 | x 610 … 636, y 32 … 62 | `g_minimapMode = 1` — the labour rating, county `+0x03` |
| 2 | x 610 … 636, y 64 … **97** | `= 2` — the food rating, county `+0x02` |
| 3 | x 610 … 636, y 96 … 126 | `= 3` — happiness, county `+0x01` |
| 4 | x 610 … 636, y 128 … 158 | mode 0 → `Map_ToggleZoom()`; any other mode → back to mode 0 |

The fourth is **the zoom toggle**, which `docs/screens.md` §7 records us having replaced
with a key. While a mode is active ids 1–3 do nothing at all and only id 4 responds; a
right-click anywhere in `x >= 478, y 24 … 152` also clears it (`FUN_00439079`).

Record 1's `y1` is `0x42` (66) where the pattern wants `0x3F` (63), so band 2 is 34 pixels
tall and overlaps band 3's first two rows. `Hotspot_Test` returns on the first match, so
y 96 and 97 select mode 2. That is the original's own data.

The ratings themselves come from `FUN_00451BBA`, which `Minimap_DrawOverlay` calls before
painting: happiness `/ 20`; the food rating 0 when achieved is below wanted and otherwise
1 … 5 by achieved; the labour rating 0 if job 0 or 1 is understaffed, 6 if nobody is
surplus, else 5. The overlay skips any county rated above 5 or not the local player's and
indexes a **second** ramp, `g_minimapRatingRamp` (`0x004D28F8`) — not the realm ramp at
`0x004D2900` that `l2-view` already transcribes.

---

## 2.6 The right mouse button, which is how you leave almost everything  **[V]**

**Player-reported and then confirmed from the code.** He said *"right click would close a
bunch of popups in the game"*, which turned out to understate it: the right button is the
game's universal *back*, and the game says so in its own words — `Screen_SliderBox`
(`0x0040CD58`) prints `L2.eng` group 12 index 0, **"Click Right to Exit"**, under the value
spinner's caption.

### The dispatcher this document did not know about

§1 used to say three functions switch on `g_screenId` and nothing else does. **There is a
fourth**, and it is the one that decides how every screen is *left*: **`Screen_FrameInput`**
(`0x0042FF10`), 8,140 bytes, called once per frame — named only after this pass, having sat
in `docs/hypotheses.json` as `Ui_DispatchPointer` with 6% of its body read and a warning
attached. The whole body has been read now; what it dispatches *to* has not, so the symbol
is `inferred` rather than `verified`.

```c
Screen_HitRegion();
iVar2 = FUN_0047685d();                                  /* the message scroll  */
if ((iVar2 == 0) && (iVar2 = Screen_HandleInput(), iVar2 == 0)) {
  if ((DAT_0052afa8 == 0) || (DAT_004e6900 == '\0')) {
     ...one hand-written arm per g_screenId...           /* the "back" branches */
  }
  ...
}
```

`Screen_HandleInput` (`0x004BA9C8`) contains **no reference to any right-button global at
all**; nor do `Hotspot_Test` or `Widget_Test`, both of which test only the left button's
press, release or held flags. Every right-button behaviour in the game is in the function
above, copy-pasted about forty times: there is no shared helper.

**It is not only "back", which is why it is not called `Screen_HandleBack`.** Of its 49
arms, 19 are close-only (36% of the body), **17 run a real left-button hotspot chain as
well** — including the campaign map, the battle map, the village and the four county panels
— and 13 are neither: the village's drag state machine (`0x05`/`0x06`), the setup pages
(`0x1F`, the longest arm, which calls `Net_SendCommand(6, 0)` twice), `Setup_StartGame`
(`0x1C`), `Battle_Decline` (`0x12`), and an explicit do-nothing at `0x23`. It also commits
orders — `Map_ConfirmMoveOrder` on `0x10`, the levy on `0x17` — loads `vill_gd8.pl8` and
`demo1.pl8`, clears the `WM_KEYDOWN` edge `DAT_004EABB4`, and carries a falling-edge
"the map scrolled" latch (`DAT_0057CAC4` → `DAT_00565488`) across the frame boundary.

**A second gesture is as common as the right button, and it is not a gesture at all.**
Twenty-nine arms open with `if (DAT_00553FC8 != 0 || (DAT_0055403C != 0 && DAT_00553018 == 0))`
→ force-close, where `DAT_0055403C` is what `Turn_End` writes and `DAT_00553FC8` is the
multiplayer sync-wait latch. **A third of this function's job is tearing every open panel
down when the turn ends or the network blocks**, with no user input involved.

**Where it sits in the frame matters.** Its one caller is `Battle_Frame` (`0x004B99C0`) —
the whole-game per-frame function, not just the battle's — and it is the *last* thing in
the frame, after every draw pass and after the cursor is chosen. So the `g_redrawRequest`
it sets is consumed at the top of the **next** frame, and the scroll latch at line 6555 of
the next frame: **its effects are one frame late by construction.** Anything that tries to
reproduce the original's frame ordering has to know that.

### The button flags

`FUN_004B3441`'s window procedure sets `DAT_004EABC2` bit 0 for the left button and bit 1
for the right; `FUN_004B191E` derives the per-frame edges from it.

| global | meaning |
|---|---|
| `DAT_004EAFB4` | left **pressed** this frame — 38 reads, the one this document already used |
| `DAT_004E65D8` | left **released** this frame |
| `DAT_004EABE0` | right **pressed** this frame — 8 reads |
| **`DAT_004E6900`** | **right released this frame — 56 reads, and the one that does all the work** |
| `DAT_004EA4B4`, `DAT_004EA51C` | right double-click and right debounced-single — computed every frame, **never read** |

The asymmetry is real: the right button acts on *release* almost everywhere. The five reads
of the right *press* are the two battle-master sheets, `Minimap_Click` (which returns
immediately while the right button is down, so a right-click on the minimap deliberately
does not re-centre), `BattleMap_Click` (right-down switches to scrolling the battle view),
and `FUN_0040E12E`, the drop-down menu's modal loop, which spins `while (DAT_004EABE0 == 0)`
so a right press cancels an open menu.

### What it closes

`FUN_0047685D` runs before anything else on every screen: if a message scroll is up, a right
release dismisses it and the click is consumed. That is one right-click dismissing a popup
regardless of what is on screen, and it is probably the behaviour the player remembers most.

Then the per-screen arms. Right-release closes, or steps back one level, on `0x02`, `0x04`,
`0x06`, `0x08`, `0x09`, `0x0A`, `0x0B`, `0x0C`, `0x0D`, `0x0F`, `0x10`, `0x11`, `0x13`,
`0x14`, `0x15`, `0x16`, `0x17`, `0x18`, `0x19`, `0x1A`, `0x1B`, `0x1D`, `0x1F` (pages 3, 9,
0xA, 0xD), `0x20`, `0x21`, `0x25`, `0x26`, `0x28`, `0x2A`, `0x2B`, `0x31`, `0x32`, `0x35`,
`0x36`, `0x39`, `0x42` and `0x43`. Five of them go *back one* rather than to the map:
`0x0C → 0x08`, `0x0D → 0x0A`, `0x11 → 0x04`, `0x17 → 0x0A`, `0x2A → 0x29`.

The four county panels are all the same shape — `0x14`'s arm, verbatim:

```c
if (g_screenId == '\x14') {
  if ((DAT_00553fc8 == 0) && ((DAT_0055403c == 0 || (DAT_00553018 != 0)))) {
    if ( FUN_0043292d() == 0 && FUN_00432967() == 0 &&      /* minimap modes, sidebar   */
         CountyStrip_Click() == 0 && FUN_00439122() == 0 && /* the strip, the split     */
         CountyStrip_JobClick() == 0 && FUN_00439079() == 0 ) {
      if (DAT_004e6900 == '\0') { if (FUN_0040e7e4()) { g_screenId = 0; g_redrawRequest = 2; } }
      else                      {                       g_screenId = 0; g_redrawRequest = 2;   }
    }
  }
  else { g_screenId = '\0'; g_redrawRequest = 2; }          /* the turn ended under it  */
}
```

**Built, and it was the largest single gap in this document's area.** `CountyScreen::handle`
tested the four strip quadrants itself and swallowed everything else in the column, so with a
panel open the minimap, the five sidebar buttons, the split slider, the produce rows and End
Turn were all dead. It is one predicate now — `crates/l2-game/src/screens/mod.rs`'s
`belongs_to_the_right_column`, shared with the village, whose arm opens with the same six —
and `docs/arms.json` `0x0042FF10/inset-runs-the-sidebar-guards` is the record.

Two things follow that §2.3 does not say. **The guards are tested first**, and every one of
them fires on a *left* click, so clicking the strip or the sidebar while a panel is open
**switches** panel rather than closing it — which is how you go from population to tax
without a trip via the map. And the outer condition is not a keyboard test: `DAT_0055403C`
is what `Turn_End` writes, `DAT_00553FC8` is the multiplayer turn state and `DAT_00553018`
is the F12 debug override, so its `else` **force-closes the panel when the turn ends under
it**. `0x19` is identical plus a `Ration_SliderClick()` guard, so dragging the ration slider
does not dismiss the panel.

**So a panel has three ways out and none of them is a key**: the corner picture (`FUN_0040E7E4`, a
left release inside the 24 × 24 box the last `Ui_OkButton` call stashed), a right release
anywhere, and a click on the campaign minimap — `Screen_FrameInput`'s tail runs `Minimap_Click`
from any screen and closes to the map on a hit. The only `VK_ESCAPE` handler in the game
quits it (`Menu_Quit`, or the main-loop exit flag); **no key dismisses a panel**, verified by
absence.

The one exception to all of it is **screen `0x1E`, the yes/no box**, which appears nowhere in
`Screen_FrameInput` and has no `Ui_OkButton`. It is served solely by `Screen_HandleInput`'s
`g_confirmWidgets` thumb-up and thumb-down: the confirmation box is genuinely modal and must be
answered.

### What it opens

On the campaign map the same button does the reverse. `Screen_FrameInput`'s `g_screenId == 0` arm
has exactly three right-button branches, in this order:

1. `FUN_00439079` — right release in `x >= 478`, `24 <= y < 153` with a minimap mode active:
   clear the mode and swallow the click.
2. a message scroll is up: `Msg_Dismiss()`.
3. `Map_PickTile` found a tile: **`g_screenId = 4; FUN_0043CAF4();`**

`FUN_0043CAF4` (`0x0043CAF4`) calls `Map_ResolvePick` and remembers whatever was under the
cursor, and the painter branches on it: `UnitPanel_Draw` (`0x0041B19D`) for a unit,
`FUN_0041BEFE` for a bare tile. It is gated on neither ownership nor hitting anything.

**`Readme.txt`'s errata is this panel**: *"Disbanding Armies (pg75) Right-clicking on an army
accesses an information pop-up that includes the army's county of origin."* The line it names
is two `Eng_DrawString` calls — group 31 index 9, *"An army from"*, then group 100 at
`unit.homeCounty + scenarioIndex * 20` — inside the branch that requires the unit to be an
army of the local player's, so the county of origin shows for your own armies only. Screen
`0x04` carries its own `Ui_OkButton(0x1AC, 0x1B6, 0)` and its own right-release arm, so the
same button opens it and closes it.

**Loose end.** `DAT_0052AFA8` gates the whole per-screen chain as a one-shot "swallow the
next right release", and **nothing in the corpus ever writes it a non-zero value**. Either
its writer is outside the decompiled range or it is vestigial; it is not guessed at here.

---

## 2.7 The corner of a panel: a close button whose picture is the instruction  **[V]**

**Player-reported, then confirmed from the artwork.** He described the bottom-right corner
of a county panel as *"an arrow pointing to a little black hole. It's a close button… a
weird one"*, and then, of the same picture, *"i mean it is an instruction"*. Both, and that
is the resolution rather than a contradiction: **it is a hotspot whose artwork depicts the
action it performs.**

The sheet is **`System.pl8`**, not `Misc_cty.pl8` or `Panels.pl8`. `Ui_OkButton`
(`0x0040D1BC`) draws frame **`0x33`** for mode 0 and frame **`0x10`** for mode 1, both
24 × 24, and both hold the same drawing: **a cursor arrow pointing into a small black
hole**, on the parchment ground the rest of the sheet uses. Mode 1 adds a raised bevel;
mode 0 has none. There is no tick anywhere in either.

The picture is measurable rather than a matter of opinion, which is what makes this **[V]**:
of the sheet's 84 frames those two are the **4th and 5th darkest**, 15.3% and 11.5% of their
area in near-black ink against a median frame's 1.2%. The widgets that really are thin
strokes sit where you would expect — the tax-up arrow at 0.2%, the slider knob at 0.0%. A
tick cannot come 4th out of 84. `crates/l2-view/tests/install.rs` asserts that rank against
the user's own file.

**And it is live.** `Ui_OkButtonClicked` (`0x0040E7E4`) hit-tests a 24 × 24 box at
`(DAT_0055CE78, DAT_0057C8A0)` on a **left release** and consumes the click; `Ui_OkButton`
is the only writer of those two globals (`00400000.c:7114`). So a panel has two ways out and
the game offers both: this corner, and the right button anywhere (§2.6). The one consequence
worth knowing is that `Ui_OkButton` stashes only the **last** call's position, so on a screen
that draws two corner pictures only the second is clickable — safe only because the draw
pass runs before the input pass in the frame.

**Both first readings were half right, and neither had looked at the sheet.** This document
had a *button* with the wrong *picture* — it called the corner "the tick" in five places, and
§4.2 labelled frame `0x33` "the tick that closes a panel". The reading that replaced it
inferred from `L2.eng` group 12 index 0, *"Click Right to Exit"*, that the corner must
therefore be **signage** rather than a target — a clean story that the artwork does not
support. Each was a plausible account of one half built from evidence about the other, which
is C3's shape at the scale of a single 24 × 24 frame, and the fix in both directions was to
decode the frame and look at it. `docs/decisions.md` C46.

**Why the name is kept.** `Ui_OkButton` and `l2-view`'s `system::OK` stay as they are,
because renaming a symbol every reader already knows costs more than the word "OK" misleads
— but the comment beside each now says what the frame holds. The name is ours; the picture
is the game's.

The same pass corrected `g_confirmWidgets`' pair, §4.2's other guess: frames `0x1D` and
`0x1F` are **a mailed hand giving a thumb up and a thumb down**, not a tick and a cross.

His interface recollections have now been right five times running — the village being an
inset (`docs/decisions.md` C22), the town square (C41), the sidebar icons (C43), right-click
(§2.6), and this. A player who has looked at the real screen is the cheapest oracle in this
project; `docs/decisions.md` C46 records the whole of it.

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
| `Ui_OkButton` `0x0040D1BC` | `(x, y, mode)` | the picture that closes a panel - a mouse pointer going into a black hole, button-sheet frame 0x33 (mode 0) or 0x10 (mode 1). Section 2.7 |

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
index 0** — every frame in the table below except `Ui_OkButton`'s — while **none of
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
| 0x1D / 0x1F | 32 × 32 | `g_confirmWidgets` — **a mailed hand, thumb up and thumb down**, not a tick and a cross |
| 0x10 / 0x11 | 24 × 24 | `Ui_OkButton` mode 1 — the same picture as 0x33, with a raised bevel round it |
| 0x33 | 24 × 24 | `Ui_OkButton` mode 0 — **a mouse pointer going into a black hole.** §2.7 |
| 0x4A | 24 × 24 | **slider left cap** |
| 0x4B | 24 × 24 | **slider right cap** |
| 0x4C | **10 × 32** | **slider knob** |

Two of those rows used to say something else, and both were guesses that read as
descriptions. **0x33 is not a tick** and 0x1D / 0x1F are not a tick and a cross; the frames
were decoded and looked at rather than named from the function that draws them.

**The skin trap has a sharper edge than "69 blank frames" suggests**, and it is worth
knowing which way round it falls. In `System2.pl8`, `Ui_OkButton`'s **mode 0** frame
(`0x33`) is painted — the same drawing on a stone ground rather than parchment — while its
**mode 1** frame (`0x10`) is entirely index 0. Mode 1 is exactly what the village, the
merchant, the armoury, castle building and the greatest-noble page pass, all at
`(g_screenStride - 0x1C, g_screenHeight - 0x1C)`; every other call site passes mode 0. So
loading the wrong skin does not blank every corner — it blanks the corners of those five
screens and leaves the rest looking correct, which is the hardest kind of wrong to notice.
`crates/l2-view/tests/install.rs` asserts both halves against the user's own files.

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
mine (`villani2.pl8` frame 0x2B) and the quarry (frame 0x28) are painted at the same spot,
`(0x4C, top + 0x0C)`. `Village_ClusterHasJob` (`0x0045183A`) refuses that one cluster, and
only when the county has neither — the other seven never refuse, and
**`Village_DrawPeasants` loops 0 … 7 with no test at all**, so a county with no mine still
shows the slot. It simply has nobody in it.

**The three buildings, and the file they come from `[V]`.** This document said `Misc_cty.pl8`
until `docs/decisions.md` C57; the frame numbers were right and the file was not. `Village_Draw`
(`0x00412143`) ends with three consecutive blits out of **`villani2.pl8`**, each gated on one
`hasResource` byte, and there are **three** of them rather than the two that share a spot:

| industry | record | frame | at | what |
|---|---|---|---|---|
| 0 wood | `+0x295` | `0x29` | `(0xAC, top + 0xE5)` | the lumber camp, bottom right |
| 3 stone | `+0x2DD` | `0x28` | `(0x4C, top + 0x0C)` | the quarry |
| 1 iron | `+0x2AD` | `0x2B` | `(0x4C, top + 0x0C)` | the mine, drawn **after** the quarry |

The mine overwriting the quarry is safe because no county has both: over the England
turn-one fixture the two are complementary in thirteen of the fourteen counties and absent in
the fourteenth (`crates/l2-scenario/tests/import.rs`).

### 6.3a `Village_Animate`'s six overlays, and its clock **[V]**

`Village_Animate` (`0x00412421`) is called from `Screen_DrawWidgets` (`0x004BA26E`) — the
per-screen overlay pass, whose `g_screenId == 0x02` arm is this and nothing else — so it runs
on every frame the village is up, whatever the player is doing. It draws **six** overlays,
in this order:

| # | gate | sheet | frames | at | counter | pulse |
|---|---|---|---:|---|---|---:|
| 1 | none | `villani2` | 0x19 … 0x20 (8) | `(0x50, top + 0xAF)` | `DAT_004D293C` | 160 ms |
| 2 | none | `villani2` | 0x21 … 0x27 (7) | `(0x72, top + 0xC3)` | `DAT_004D2940` | **80 ms** |
| 3 | none | `villani2` | 0x0F … 0x18 (10) | `(0x16E, top + 0x125)` | `DAT_004D294C` | 160 ms |
| 4 | `industry[0]` wood | `villani2` | 7 … 14 (8) | `(0xF3, top + 0x104)` | `DAT_004D2944` | 160 ms |
| 5 | `industry[3]` stone | `villani2` | 0 … 6 (7) | `(0xA3, top + 0x1B)` | `DAT_004D2930` | 160 ms |
| 6 | `industry[1]` iron | **`villani1`** | 0 … 0x11 (18) | `(0xA4, top + 0x0C)` | `DAT_004D2938` | 160 ms |

**Three of the six are unconditional**, which had not been noticed: every village animates
whatever the county holds, and only three of the six are the resource buildings' own.

**`villani1.pl8` is the iron mine's animation and that is its only use in the executable.**
It goes into `DAT_0053E918` and overlay 6 is the single read of that buffer anywhere. The
file had been recorded on this project as loaded by nothing.

**`villani2.pl8`'s frame table confirms all five of its runs independently.** Its 44 frames
fall into five blocks of equal-sized cells laid out in rows on the artist's sheet — 0 … 6 at
26 × 29, 7 … 14 at 39 × 40, 15 … 24 at 15 × 12, 25 … 32 at 32 × 42, 33 … 39 at 19 × 18 —
which is exactly the table above, start index and length, five times over. What is left is
frames 40, 41 and 43, the three static buildings, and a 2 × 2 stub at 42. Nothing over,
nothing short. The counter bounds were read from the decompilation and the block boundaries
from the file; they agree, and
`l2-game/tests/screens.rs::the_animation_runs_are_the_blocks_the_sheet_is_laid_out_in`
asserts it including the frames either side of each run.

**The clock is `Tick_Pulses` (`0x004BBC80`), and it is not a frame counter.** It is a divider
chain that sets eight booleans, each cleared at the top of every call and true only on the
frame it fires: a **20 ms** gate on `timeGetTime`, `g_pulse80` every fourth of those, and
`g_pulse160` every second `g_pulse80`, plus six slower ones at 320, 640, 1040, 1280, 1920 and
2560 ms. So the village's slow overlays run at **6.25 Hz** and overlay 2 at **12.5 Hz**. The
same two pulses drive `FUN_004071A0`, the campaign map's waving flag.

**Two more counters are stepped and drawn by nothing** — `DAT_004D2934` (21 states, 160 ms)
and `DAT_004D2948` (16 states, 80 ms) — with no other reader anywhere in the binary.
`docs/bugs.md` B65, which also records that the dead 21-state counter has exactly as many
states as `villani1.pl8` has frames, and declines to make anything of it.

**Drawn.** `l2_view::village::OVERLAYS` is the table, `AnimationClock` is the two pulses, and
`VillageScreen` owns one. The clock is **display state**: it lives on the screen, not in
`Game` or `Kingdom`, nothing in a save or the lockstep digest depends on it, and
`the_village_animates_and_the_iron_mine_comes_out_of_villani1` asserts that a hundred ticks of
it leave the kingdom byte-identical (`docs/netcode.md` D-12).

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

### 6.4.1a The **double click** is a fourth verb, and Windows counts it **[V]**

`Screen_FrameInput`'s screen-`0x02` ladder has an arm between `Village_BandStart` and
`Village_ClickJob` that this document did not have: `Village_DoubleClick` (`0x00439DF0`).

**The game never times the clicks.** The window procedure (`0x004B29BE`) handles message
`0x203` — `WM_LBUTTONDBLCLK` — with `DAT_004EADA1 |= 1`, and the frame poll turns that into
`DAT_004EABC5`. Windows decides, against the user's own `GetDoubleClickTime()`, and sends
the double click **instead of** the second `WM_LBUTTONDOWN`, which is why the button-level
flag never rises for it. `DAT_004EABC5` is read in exactly one place in the whole binary,
and this is it. (Its right-button twin, `DAT_004EA4B4`, is computed and never read.)

**The single click is deferred 300 ms to make room for it.** `Village_ClickJob` — the job
popup — is gated not on the release but on `DAT_004EABF0`, which the poll sets only when
`DAT_004E65E8` (a release that armed a pending click, position stashed in `DAT_004EAC04` /
`DAT_004EAC08`) has stood for more than 300 milliseconds. A double click clears
`DAT_004E65E8` in the poll itself, so the popup never opens behind the reassignment. **The
job popup opening a fraction late is deliberate, not a stutter.**

The hit region is `x 0x40 … 0x1FF, y top … top + 0x178` — the same box as the band except
that it starts at the *picture's* left edge rather than at x = 0 — and `Village_GridAt`
names the cluster, as it does for a drop.

**What it does** is `FUN_00439EDB` → `FUN_00439F6A(county, cluster, fill)`:

```c
short   = wanted < 1      ? 0 : wanted - workers;
surplus = useful < 99999  ? workers - useful : 0;
if (short < 1 || !fill)  { if (surplus < 1) return 0;
                           Labour_Move(county, cluster, 6, surplus); }
else                     { if (idle == 0) return 0;
                           Labour_Move(county, 6, cluster, min(idle, short)); }
```

Cluster 6 is *Idle townsfolk*. So **a job below its floor fills from the idle pool, and a
job above its ceiling empties into it** — which is the player's *"double click idle peasants
in a task to remove them from the task"*, and it is the same ceiling §2.1.2's blue ring
tests. The count is in **people**, not icons: `popBand` is not involved.

A double click on the idle cluster itself runs the whole village: every job sheds
(`fill = 0`), and only then does every job fill (`fill = 1`). One pass would let whichever
job came first take people the later ones needed.

**And that loop reads two words past the end of its table.** It is
`for (i = 0; i < 10; i++)` over `g_jobClusterToSlot`, which has **eight** entries. The two
past the end are the head of the next table and are **4** and **6** — read out of the
shipped binary at `0x004D67A0` by `crates/l2-view/tests/install.rs`. The effect is that the
gesture covers all nine labour slots, because slot 4 (iron mining) is otherwise reachable
only through cluster 0's quarry/mine override. Whether that was intended or is an overrun
that happens to work, it is what the game does; `docs/bugs.md` carries it.

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

That single special case pins the industry order **[V]**: the village draws `villani2.pl8`
frame 0x2B (a mine) on `+0x2AD` = industry **1** and frame 0x28 (a quarry) on `+0x2DD` = industry **3**,
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
| the corner picture | `Ui_OkButton(0x180, g_villageTopY + 0x118, 1)` |

`g_villageTopY` is 64, or 132 with *Advanced Farming*. The menu bar (y 0 … 23) and the
county sidebar (x 478 … 639) are outside all of it, and so is a strip of campaign map on
either side of the picture even inside the band.

**And the caller settles it without any of the above.** `g_screenId = 2` occurs **exactly
once in the binary**, in `Map_Click` (`0x0043CE1A`): the town-square branch does
`Map_CentreOnTile(county[+0x70])` and one `FUN_004050C0` — which is `Map_DrawFrame` —
*before* setting the screen. **The game recentres the campaign map on the town in order to
open the village over it.** `Village_Draw` then repaints the map itself, by the same route,
every time it is called with `reload != 0`.

### 6.4.4a The sidebar stays **live** under the village, and dies for the length of a drag  **[V]**

§6.4.4 established that the sidebar is still *visible*. Whether it is still *clickable* is a
separate question and the arm answers it outright. `Screen_FrameInput`'s `g_screenId == 0x02`
ladder, in order, before a single village verb:

| # | guard | what it is |
|---|---|---|
| 1 | `FUN_0043292d` | `Hotspot_Test(0x262, 0x20, &g_minimapModeButtons, 4)` — the four mode icons |
| 2 | `FUN_00432967` | `Hotspot_Test(0x1DE, 0x1AE, &g_sidebarButtons, 6)` — the six sidebar buttons |
| 3 | `CountyStrip_Click` | the 2 × 2 quadrant hotspot into the four county panels |
| 4 | `Labour_SplitSliderDrag` (`0x00439122`) | the farm/industry split, `x 478 … 639, y 257 … 296` |
| 5 | `CountyStrip_JobClick` | the produce rows |
| 6 | `FUN_00439079` | untraced; also guard 2 of the `g_screenId == 0` arm |

Only then `Ui_OkButtonClicked`, `Village_BandStart` → `0x05`, `Village_DoubleClick`,
`Village_ClickJob` → `0x0F`, and finally the right release → `0`.

Two facts fall out, and they are opposite ones.

**The whole right-hand column keeps working.** Every one of the six hit-tests `x >= 0x1DE`, so
the rule is *the column at 478 and nothing else*: `Map_Click` is **not** in this ladder, and a
click on the strip of campaign map either side of the inset does nothing at all. A player who
went and checked: *"everything is still clickable with the town square open… The slider does
indeed still work with town square open and causes no issues."*

**The village's other two screen ids test none of them.** `0x05` runs `Village_BandRelease`
and `Village_BoxSelect`; `0x06` runs `Village_Drop`. Neither looks at the sidebar. So the
column is dead from the moment a rubber band starts until the peasants land — reproduced on
purpose, `docs/bugs.md` B63a, `docs/decisions.md` C59.

**One thing the arm settles that our engine still conflates.** `Map_EdgeScroll` is guard 1 of
the `g_screenId == 0` arm and appears **nowhere** in the `0x02` arm, so the campaign map does
*not* edge-scroll under an open village. The flag wave is a different matter — it lives in the
draw pass (`Map_DrawFrame`'s `tick`), which `Village_Draw` re-enters on every repaint, so flags
keep waving behind the inset. Ours runs both from `MapScreen::update`, and `Machine::update`
ticks only the top screen, so with the village open we stop both. **Neither half is right**:
the scroll should stay stopped for a different reason than it currently is, and the wave should
not have stopped at all. Untouched, and written down here so whoever separates them meets the
distinction rather than making one of the two behaviours match and calling it done.

**And a screen opened from that sidebar does not come back to the village.** `g_screenId` is
one byte; 57 of the 100 writes to it in `Screen_FrameInput` are the literal `0`, and only 15
restore a remembered screen — 11 `g_menuPrevScreen`, 2 `g_screenIdSaved`, 2
`g_sliderPrevScreen`, none of them the village. The one exception is the job popup, and it is a
constant too: `0x0F`'s arm is `if (DAT_005533F4 == 0) g_screenId = 0x02; else g_screenId = 0;`
— a flag set at the call sites saying *"opened from the map"*, not a memory of the stack.
`docs/bugs.md` B63.

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
* ~~**The village's own layout.**~~ *Done — §6.4, and `Village_Animate` with it (§6.3a): the
  six overlays, the eight counters, the two that draw nothing, and the 80 ms / 160 ms pulse
  chain in `Tick_Pulses` that steps them. `villani1.pl8` turned out to be the iron mine's
  eighteen-frame loop and nothing else.*
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

**Built.** `crates/l2-game/src/screens/menubar.rs` is this section: the three measured
titles, the sixteen items, screen `0x32` and its four arms. Six of the sixteen reach a
screen — load, save, quit and the four option pages — and the rest refuse in one line naming
the screen or the message id they want, because the confirm box (`0x1E`), the value spinner
(`0x21`) and the message scroll are not built. `docs/arms.json` group `menu-bar`, and
`docs/decisions.md` C76 on what one *"not reproduced"* table row was hiding.

**One thing the geometry forces and it is worth stating here.** `Ui_DrawMenuTitles` writes
each caption's measured right edge *back into the table* at draw time, so the bar cannot be
hit-tested until it has been painted — the hit boxes are an output of the draw pass. Any
reimplementation that lays the titles out from constants will get the dead 32-pixel gaps
wrong, and any that hit-tests before drawing will hit nothing on the first frame.

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

* **Button sheet frame pairs.** 29 / 31 is yes and no, 35 / 37 a scroll pair, 68 / 66 a
  minus and plus, 21 / 23 an up and down. Every yes/no pair found sits at (x, y) and
  (x + 40, y + 4) — the *no* is four pixels lower, on all six screens that use one.
  **29 / 31 are not a tick and a cross**: decoded, they are 32 × 32 pictures of a mailed
  hand with its thumb up and its thumb down. §4.2.
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
(`0x004DDD78`) holds a thumb-up at (304, 64) frame 29, a thumb-down at (352, 64) frame 31, and the
list's two scroll arrows at (384, 144) and (384, 176), frames 35 and 37, carrying the
deltas −3 and +3 with list id 1. Read as absolute screen coordinates all four sit above or
on the top edge of a window that begins at y = 144, which would put the two hands
outside the panel they belong to. Read relative to the box origin they land at (320, 208)
and (368, 208) — level with the name field, whose rectangle ends at x = 232 — and at
(400, 288) and (400, 320), immediately right of the list, whose rectangle ends at x = 392.
The deciding argument is that **one table serves two screens whose boxes are at different
origins**, which an absolute table cannot do. It is still an inference; it is marked as one
in the module that acts on it.

---

## 11. `0x1B`, the castle chooser — five picture buttons and an OK  **[V]**

The door to castles, which are the door to sieges. It was a shell; it is
`crates/l2-game/src/screens/castle.rs` now.

### 11.1 The two widget tables

Neither had been decoded, and between them they are the whole interface:

| table | records | handler | what |
|---|---|---|---|
| `g_castleTypeWidgets` `0x004DC818` | 5, **kind 1** (a rectangle: `x1, y1, x2, y2`) | `CastleBuild_Select` `0x00436B22` | the five castle pictures, hotspot ids 0 … 4 |
| `g_castleBuildWidgets` `0x004DDB80` | 2, kind 5 (a sprite) | `CastleBuild_Confirm` `0x00436B59` | tick frame 29 at (432, 440), hotspot 1; cross frame 31 at (472, 444), hotspot 0 |

```text
(17, 270)-(95, 415)   (96, 270)-(209, 415)   (210, 270)-(290, 415)
(291, 270)-(414, 415) (415, 270)-(618, 415)
```

**They tile x 17 … 618 with no gap and no overlap**, and their widths differ — 79, 114, 81,
124, 204 — because each is as wide as its castle's picture in `caspics.pl8`. That is also
what says the table was decoded at the right base address: a mis-aligned read does not
produce five abutting rectangles. `the_five_castle_strips_tile_the_row_exactly` asserts it.

`CastleBuild_Select` is a bare `DAT_0056D898 = g_uiHotspotId` with **no guard**, so a player
may select a castle smaller than the one he has and learns otherwise only from the OK button.

### 11.2 What the panel draws, and the two refusals

`Screen_CastleBuildPanel` (`0x004198AA`), redrawn only when `DAT_005440B8` is set:

```text
Blit_Raster(caspics.pl8[type], 0x9E, 0x14, 0x140, 200)   the big picture
FUN_0040328E(71, sel + 1, 0x1F6, 0x18)        "Wooden palisade." … "Royal castle."
Ui_DrawNumber(stone, '@', 0x230, 0x60) + 71/6            "of stone needed,"
Ui_DrawNumber(wood,  '@', 0x230, 0x80) + 71/7            "of wood needed."
Eng_DrawString(71, 8, 0x20C, 0xB4) + workforce + 71/9    "will take N to build."
Ui_DrawBox(0x70, 0x1AC, 0x1A, 3) + 71/0x10 + bonus       "Boosts tax revenues by N%"
Eng_DrawString(71, 0xF, 0xE0, 0x1C8)                     "Start construction?"
Ui_DrawBox(8, 200, 8, 3) + 71/0xB + cap + 71/0xC         "Barracks for N troops."
```

**The stone and the wood are net of the castle already standing** — the panel subtracts
`g_castleMaterial[existing type]`, the same difference `Castle_Order` charges, and it can
come out negative because upgrading to a stonier castle refunds wood. It is printed as it
comes.

`CastleBuild_Confirm` has exactly **two** guards, and both close the screen rather than
staying on it:

| condition | message | `L2.eng` |
|---|---|---|
| the type picked is the one standing | `0x93` | 147/1 *"already of the type you are proposing to change it to!!"* |
| the type picked is smaller | `0x122` | 290/1 *"Your current castle is stronger than the one you propose to upgrade to, my lord."* |

**There is no third guard** — no affordability test at all. `docs/kingdom.md` §7.5.1 has what
ordering a castle you cannot pay for actually does.

### 11.3 The way in

`Castle_OpenScreen` (`0x00436A88`), the sidebar's fourth button (§6's table, x 576 … 606). It
refuses a county that is not `g_localPlayer`'s with message `0x70`, seeds `DAT_0056D898` from
`castleType - 1` (0 on a bare plot), and sets `g_screenId = 0x1B`. On the way out, with
animations on, it plays `Castle1.smk … Castle5.smk` — one construction movie per type.
