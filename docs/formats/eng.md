# `.eng` — the game's text files

`.eng` is **not one format**. Three unrelated file layouts share the extension,
and the only thing they have in common is that they hold English text that a
localiser would want to replace:

| File | Size (Win / DOS) | Format |
|---|---:|---|
| `L2.eng` | 100,016 / 91,054 | **binary**: magic + 24-bit offset table + NUL-separated blob |
| `BATTLES.ENG` | 7,150 / — | **plain text**, CR/LF-delimited, read as fixed triples |
| `TROOPS.ENG` `TROOPS2.ENG` `TROOPS3.ENG` | 22,764 each / — | **plain text**, a commented numeric table |

Only `L2.eng` exists in the DOS install (`F:\games\LORDS2\L2.ENG`); the other
four shipped with the Windows release. `.eng` is also the extension the German
and French builds would swap, which is why the container matters for a mod
system.

Status legend as in [`maps.md`](maps.md): **[V]** verified against file bytes
and/or the shipped binaries, **[I]** inferred.

---

## 1. `L2.eng` — indexed string container

### 1.1 Layout

```
0x0000  char[8]   "Textfile"
0x0008  u32[N]    group table - only the low 3 bytes of each entry are used
        ...       string blob: NUL-terminated Latin-1 strings, back to back
```

**[V] Each table slot is 4 bytes but the game reads only 3.** `Lords2.exe` and
`mapl2.exe` both compute a group's offset as

```c
offset(g) = base[8 + g*4] | (base[9 + g*4] << 8) | (base[10 + g*4] << 16);
```

(`mapl2.exe` `FUN_0040ED0E`). The fourth byte is `0x00` in every slot of both
shipped files, so treating the table as `u32` works, but a 24-bit reading is
what the engine does and files are far below the 16 MB ceiling anyway.

**[V] There is no count field.** The table runs from offset 8 up to where the
first string starts, so

```
N = (offset(1) - 8) / 4
```

**[V] Slot 0 is always `0` and is not a group.** Group ids therefore run `1 … N-1`.

| File | `offset(1)` | N (slots) | Valid group ids | Strings |
|---|---:|---:|---|---:|
| Windows `L2.eng` | 1,280 | 318 | 1 … 317 | 3,229 |
| DOS `L2.ENG` | 1,208 | 300 | 1 … 299 | 2,349 |

Both divisions are exact.

### 1.2 Groups and strings

**[V] A group is a *run* of strings, not a single string.** `offset(g)` points at
the group's first string; the group continues until `offset(g+1)` (or EOF for
the last group). Within the run the strings are plain NUL-terminated Latin-1,
laid end to end.

Checked over both files: **every group region decomposes exactly into
NUL-terminated strings with zero leftover bytes** — 3,229 strings in 317 groups
and 2,349 in 299. If the group model were wrong, some region would end
mid-string; none does.

Consecutive slots may hold the *same* offset, which means an empty group
(28 of them in the Windows file, 33 in the DOS one — the game was clearly built
with a group id allocation that outran the content).

The blob contains **no control byte other than NUL** in either file. That
matters because the lookup routine skips bytes below `0x20`.

### 1.3 How the engine addresses a string

**[V]** `mapl2.exe` `FUN_0040ED72(group, index)`:

```c
p = base + offset(group);
while (index > 0) {
    if (*p == 0 && (p[-1] >= 0x20 || p[-1] == 0))   /* a real terminator */
        index--;
    p++;
}
while (*p < 0x20) p++;                               /* skip NULs / padding */
```

So a string is addressed by **(group id, index within group)** and the walk is
purely sequential — the format has no per-string index. A reimplementation
should build `group -> string[]` once at load, exactly as `eng.js` does.

Two consequences worth knowing:

* An empty group does not fail; the pointer simply walks on into the next
  group's strings. **[V]** `mapl2.exe` asks for group 41 indices 27–29 for its
  default map name, title and description.

  > **Corrected.** An earlier revision claimed group 41 is empty in the shipped
  > `L2.eng` and that those three strings appear in no shipped file. That is
  > true only of the **DOS** release. In the **Windows** `L2.eng` group 41 holds
  > **30 strings**, and indices 27–29 are exactly those three. The claim was
  > checked against the wrong install and generalised — see `docs/audit.md`.
  > `tools/audit/eng41.js` reproduces both readings side by side.

  The editor was built against a
  development `l2.eng` that the retail release does not include. A tool that
  wants the editor's default strings must supply them itself.
* Because the terminator test rejects a NUL preceded by a byte in `1 … 0x1F`,
  the format *could* carry control-character padding. No shipped file does.

### 1.4 Group ids are stable across releases

**[V]** Comparing the DOS (1996) and Windows (1997) files over the 299 group ids
they share: **286 groups are byte-identical, 13 differ**, and the Windows
release simply appends 18 new groups (ids 300–317). Ids `1 … 299` mean the same
thing in both. A translation table keyed on `(group, index)` is therefore a
reasonable design.

A few landmarks (Windows ids):

| Group | Contents |
|---:|---|
| 1 | `File, New Game, Load, Save, Quit` |
| 6 | goods: `No goods, Grain, Cattle, Sheep, Ale, Wool, Iron, Stone, Timber, Pikes, Bows, Maces, Crossbows, Swords, Mail` |
| 30 | campaign-map terrain names: `Scrubland., Road., Border., Sea., Mountain., Woodland., Farmland, County town., Castle., …` |
| 43 | main-menu / game-options screen |
| 100 | county names — 1,200 strings (`Here Be Dragons!`, `Duchy of Cornwall`, `Wiltshire`, …) |
| 101 | **campaign map names** — 60 strings, see below |
| 103 | settings-value words (`off/on/easy/normal/hard/impossible/…`) |
| 220 | tooltips (`Null tool tip`, `Kingdom view. Click on a county.`, …) |
| 293–295 | help topics |

### Group 101 answers an open question in `maps.md`

`maps.md` §6 lists "map **names** are not in `L2_maps.dat` … presumably in
`L2.eng`" as open. **[V] They are group 101**, and the alignment is exact:

```
 index  0..23  England, Scotland, Ireland, France, ... Pentagon, YinYang      24 names
 index 24..39  "map no 25" ... "map no 40"                                    16 placeholders
 index 40..59  Britain, Imperium, The World, Japan, ... Cubium, Snowflake      20 names
```

That is **[V]** precisely the used/empty slot census `maps.md` derived
independently from the map data — slots 0–23 used, 24–39 empty, 40–59 used.
Group 101 names slots `0 … 59`; the 20 empty tail slots 60–79 have no entry at
all, so the list is 60 long, not 80. **[I]** Group 100's 1,200 county names are
probably `60 maps x 20 county slots` on the same indexing, but that was not
checked against the county planes.

**[V] Group 41 indices 2–9 are the battle-map terrain names**, in `.skr` terrain
order. An earlier revision asserted no such group existed; that was the same
DOS-vs-Windows error as above, and it closed off an answer that was sitting in
the data. These names bear directly on the open terrain-value questions in
[`skr.md`](skr.md).

---

## 2. `BATTLES.ENG` — the built-in battle list

**[V] A plain CR/LF text file.** `Lords2.exe` `FUN_0042AA90` reads the whole
file into memory and then does exactly one thing to it:

```c
for (i = 0; i < size; i++) if (buf[i] < 0x20) buf[i] = 0;
```

Every byte below `0x20` — CR, LF, tab — becomes a NUL. The result is a flat
array of NUL-terminated strings, consumed as **triples**:

| Field | Corresponds to `.skr` |
|---|---|
| short name | 13-byte field |
| full name | 29-byte field |
| description | 141-byte field |

**[V] 171 non-empty fields = exactly 57 records**, no remainder:

| Records | Contents |
|---:|---|
| 0–34 | the 35 shipped battles and sieges (`Three Bridges`, `Isthmus`, … `Expanded Keep`) |
| 35–36 | `Siege14`, `Siege15` — placeholder text |
| 37–56 | `User battle1` … `User battle20` — **the 20 slots a `.skr` file fills** |

The 20 user slots line up one-for-one with the 20 maps in a `.skr`, and the
35 built-in battles line up one-for-one with the 35 rows of `TROOPS*.ENG`
(§3). This is the join between the three formats.

Blank lines produce empty fields; the parser must drop zero-length strings
before grouping into triples (the game does the same by construction, since it
indexes past them).

---

## 3. `TROOPS.ENG`, `TROOPS2.ENG`, `TROOPS3.ENG` — skirmish army tables

**[V] A plain text file with a free-form comment header**, ended by the first
`*`. Its own header says it:

```
Data file, to enable troop number modeling for Lords2 skirmish mode.
...
The first line is the adjudged overall defensive advantage in the skirmish - zero to ten
The second line is for the attackers - max troops    (Very easy)
The third line is for the defenders  - max troops
...
Only alter the NUMBERS below and keep them in the current format

 Pe   Xb   Ma   Sw   Pi   Ar   Kn   Ca   To   Ra   Oi
```

### 3.1 Parse

`Lords2.exe` `FUN_0042AC0C`: skip to the first `*`, then scan **decimal integer
tokens** and ignore literally everything else — `*Map one`, the difficulty
labels, the dashes. Tokens fill a state machine:

```
for each of 35 rows (battles):
    1 number  -> defensive advantage, clamped to 0..10
    for each of 5 difficulty groups (Very easy, Easy, Normal, Hard, Very hard):
        for each of 2 sides (attacker, then defender):
            11 numbers -> Pe Xb Ma Sw Pi Ar Kn Ca To Ra Oi
```

```
35 * (1 + 5 * 2 * 11) = 35 * 111 = 3885 numbers
```

**[V] All three files contain exactly 3,885 numeric tokens after the first `*`,
and exactly 35 `*` markers.** The arithmetic closes with nothing left over in
every file.

The in-memory table is `short[35][2][5][11]` — side before difficulty, not after
— with a side stride of `0x6E` and a difficulty stride of `0x16`. **Corrected:**
an earlier revision gave `[35][5][2][11]`, transposing the middle two dimensions;
verified twice against the battle code. The row stride is `0xDC` = 220
bytes, based at `0x00516AC0`; the per-row defensive advantage goes to
`0x0051FAE0`.

**[V]** After parsing, the game clamps columns **7–10** (`Ca To Ra Oi`) to a
maximum of 9. No shipped file exceeds 9 in those columns. This is what names
those four columns as siege equipment, and it is the same eleven-slot layout the
`.skr` army record uses — see [`skr.md`](skr.md).

### 3.2 Only the "Normal" group is authoritative

**[V]** Immediately after parsing, the game overwrites difficulty groups
0, 1, 3 and 4 with **group 2** ("Normal") for both sides, and then re-derives
them arithmetically (`FUN_00404D6B(x, p)` is just `x * p / 100`):

```
group 0 "Very easy" = Normal + 16%      group 3 "Hard"      = Normal -  8%
group 1 "Easy"      = Normal +  8%      group 4 "Very hard" = Normal - 16%
group 2 "Normal"    = as read from the file
```

and it does that only for **troop types 0–6**; the siege columns 7–10 keep the
Normal value at every difficulty. The other four groups as written in the file
are dead data.

`TROOPS2.ENG` and `TROOPS3.ENG` reflect this: their `Normal` rows are populated
throughout and the other four groups are **almost** empty, and the files still
parse to exactly 3,885 numbers. `TROOPS.ENG` (the oldest, 30 Apr 1997) carries
more — a leftover from before the layout change its own header announces.

**Corrected, and measured.** An earlier revision of this section said the other
four groups were "all zeros", and they are not. Counting them
(`crates/l2-mods/tests/corpus.rs`,
`the_shipped_difficulty_rows_are_dead_data_and_not_the_engines_curve`):

| file | non-Normal entries populated | in rows |
|---|---:|---|
| `TROOPS.ENG` | 402 of 3,080 | 0–4, 20–23 |
| `TROOPS2.ENG` | 169 of 3,080 | 15–20 |
| `TROOPS3.ENG` | 160 of 3,080 | 15–19 |

The conclusion is unchanged and is now better supported: these are scattered
leftovers in a handful of battles, not a filled-in table, and the engine
overwrites all of them.

**They are also not the curve.** Deriving the five percentages from the file
was tried and does not work: of `TROOPS.ENG`'s 402 populated entries only 43
equal `Normal × p / 100` for our `p`, and the ratios that are there run 1.20,
1.222, 1.225, 1.233, 1.25, 1.266, 1.30, 1.33, 1.40, 1.50, 1.60 and 2.00 in
group 0 alone — hand-authored per battle, with 116 % nowhere in them. So
**116/108/100/92/84 rests on the decompilation alone**; the corpus corroborates
that the rows are dead but cannot corroborate the numbers. That gap is real and
is recorded rather than closed.

### 3.3 Which file is used

**[V]** `FUN_0042AC0C` picks one of the three at load time from two globals:
one flag selects `troops.eng`, otherwise a comparison between two values selects
`troops2.eng` or `troops3.eng`. **[I]** given the file contents, that is
open-field battle vs. siege vs. castle assault, but the globals were not traced.

All three files are the same size (22,764 bytes) and differ from each other in
325–1,363 bytes.

---

## 4. Validation and reproduction

```powershell
node E:/dev/lords2/tools/skr/eng.js validate "F:/games/Lords of the Realm II"
node E:/dev/lords2/tools/skr/eng.js validate "F:/games/LORDS2"

node E:/dev/lords2/tools/skr/eng.js l2      "F:/games/Lords of the Realm II/L2.eng"
node E:/dev/lords2/tools/skr/eng.js battles "F:/games/Lords of the Realm II/BATTLES.ENG"
node E:/dev/lords2/tools/skr/eng.js troops  "F:/games/Lords of the Realm II/TROOPS2.ENG"
```

`validate` over the Windows install:

```
file: L2.eng  size 100016
  OK   magic is "Textfile"
  OK   table length closes: (entry[1]-8)/4 = 318 slots (group ids 0..317)
  OK   group 0 offset is 0 (unused)
  OK   4th byte of every table slot is 0 (offsets are 24-bit)
  OK   offsets are non-decreasing from group 1
  OK   blob contains no control byte other than NUL
  OK   every group region decomposes exactly into NUL-terminated strings
  info  3229 strings in 317 groups; 28 groups empty
file: BATTLES.ENG
  OK   non-empty field count 171 is a multiple of 3 -> 57 records
file: TROOPS.ENG / TROOPS2.ENG / TROOPS3.ENG
  OK   3885 numbers = 35 rows x (1 + 5 groups x 2 sides x 11 types)
  OK   siege columns Ca/To/Ra/Oi all <= 9 (the game clamps them to 9)
RESULT: all checks passed
```

The DOS install passes the `L2.ENG` checks (300 slots, 2,349 strings) and has
none of the other files.

---

## 5. The group census — what every one of the 317 groups is for

`L2.eng` is the strongest anchor in this project, because **index 0 of almost every
group is a descriptive label**: the file is 317 summaries the game wrote about itself.
This section maps every group to the mechanic it serves and the code that reaches it.

### 5.1 Coverage, and what "unreferenced" actually costs

The measurement, re-derived here by a scanner that does not use `anchor.js`
(`docs/method.md` §4 — re-derive anything load-bearing by a route that does not use the
tool). All five `L2.eng` primitives were scanned — `Eng_DrawString`, `Eng_Seek`,
`Ui_DrawCentred`, `Eng_CopyString` and the word-wrapper `FUN_0040328E` — plus every
indirect route found:

| how a group is reached | groups | notes |
|---|---:|---|
| a **literal** group id at an `L2.eng` primitive | **67** | 530 of the 599 primitive call sites pass a literal |
| **indirectly**, through a dispatcher or a table | **183** | see §5.4 |
| **not reached at all** | **67** | of which **28 hold no strings** and 39 hold real text |
| | **317** | |

**The two sets do not overlap by a single group.** Every one of the 183 indirect groups
is invisible to a literal search and every one of the 67 literal groups is invisible to
the message queue: they are two disjoint namespaces, screen text and notification text.
That is why a literal-only census reads as a catastrophe and is not one.

Several different numbers for "reached by a literal" are in circulation, and the
differences are all explicable:

* **67** — the direct-primitive count above, from a scanner written for this section.
  `anchor.js strings --limit 5000` now returns **the same 67 groups**, and the two agree
  set-for-set. That agreement is the check on both.
* **65** — what `anchor.js` returned *before* this commit. It was stale:
  `symbols.json` renamed `FUN_004017BF` to `Eng_CopyString`, and `anchor.js`'s
  `ENG_CALLS` table still keyed on the old name, so every `Eng_CopyString` site was
  skipped — silently, because a key that matches nothing looks exactly like a primitive
  nobody calls. It cost groups 7 (the lord titles, which is how new-game setup names the
  AI) and 89. Both spellings are now listed, with a comment saying why.
* **40** — what `anchor.js strings` *prints* with no `--limit`. It shows the top 40
  functions by size and nothing else, so counting groups off its output undercounts
  badly. **Pass `--limit 5000` before counting anything from it.**
* **182** — direct primitives *plus* `Msg_Enqueue`. This is the number
  `docs/method.md` §4 quotes as "135 of the 317 groups are unreached", and it is the
  same measurement seen from the other side (317 − 182 = 135).

A figure of **52** was quoted when this work was commissioned. It could not be
reproduced from any query here and is recorded as unexplained rather than adopted.

**39 non-empty groups are reached by nothing**, and they fall into four kinds, which is
the useful part:

* **Superseded panels** — an earlier, usually *richer* version of a panel whose shipped
  text is a later group. Group 64 (`Tax rate`, `People pay`) is a literal two-string
  prefix of group 86, which `Panel_Tax` draws. Also 9, 43, 44, 57, 60, 62, 63, 65, 88.
  **Group 62 is the biggest loss**: 39 strings of a food panel with per-food breakdowns,
  forager theft and the four grain stages, replaced by group 87's twelve.
* **Placeholders never written** — group 46 `GAME OVER` whose body is literally
  `"Medieval banter goes here......."`; groups 203–205 `Free lead in`; 213 `Battles2`;
  216 `Sieges3`; 67 `Some info goes here`.
* **Demo and cut content** — 158, 159, 169, 222, 223 are demo-build text; 47 is a cut
  battle "Group Information" panel whose unit classes (Heavy Infantry, Slingers,
  Auxiliaries) are not Lords II troop types at all; 25 and 28 are months and weeks for a
  calendar the game never shows.
* **A different executable** — group 41 is `mapl2.exe`'s, not `Lords2.exe`'s (§5.5).

The 28 empty groups are 42, 53–56, 58, 78, 79, 84, 90–93, 104–107, 198, 199 and 261–269.
One of them is load-bearing anyway: **group 93 is the base of `owner + 93`**, the army-name
lookup, so owner 0 addresses an empty group and the walk falls through into group 94.

### 5.2 Group 14 is the multiplayer divergence report, and it names the checksum's fields

> `Divergence error. Current count of` / `Hit Numpad 7 to re-sync.` /
> `COUNTY` `PLAYER` `PIECE` `CTY MAP` `FIGURE` `ARROW` `AI GROUP` `BAT MAP` /
> `- rogue no's` `- 001 to 200 bytes` … `- 601 to 800 bytes` /
> `Divergence error. Please wait.` / `The master machine will resync the game.`

**[V] Indices 2–9 are the eight bytes of the per-realm sync digest, in order.**
`Sync_BuildDigest` (`0x00440231`) fills a 10-byte record per realm at
**`g_syncDigest` (`0x00569530`)**, stride 10:

| byte | group 14 index | what is summed | records | stride | skipped prefix |
|---:|---:|---|---:|---:|---:|
| 1 | 2 `COUNTY` | `g_counties[1..16]` | 16 | `0x300` | 5 |
| 2 | 3 `PLAYER` | `g_realms[1..5]` | 5 | `0x160` | 6 |
| 3 | 4 `PIECE` | `g_units[1..150]` | 150 | `0x1A4` | 0 |
| 4 | 5 `CTY MAP` | **never written** | | | |
| 5 | 6 `FIGURE` | `g_battleMen[1..80]` | 80 | `0x1B0` | `0x12` |
| 6 | 7 `ARROW` | `g_missiles[1..100]` | 100 | `0x4C` | 4 |
| 7 | 8 `AI GROUP` | `g_battleUnits[1..80]` | 80 | `0x34` | 0 |
| 8 | 9 `BAT MAP` | **never written** | | | |

Byte 0 is the sum of bytes 1–8 and is the only byte the *routine* check compares.
Byte 9 is `(DAT_0057C998 & 0x7F) + 1` — the counter the shipped debug overlay labels `Gtime`, the frame tag that stops two peers comparing
different ticks.

**The check that could have failed, and did not.** The eight names line up 8-for-8 with
the eight byte slots, six of them agree semantically with arrays that were named
independently and years apart — `ARROW`↔missiles, `FIGURE`↔battle men, `PIECE`↔campaign
units, `AI GROUP`↔battle units — and **the two slots the code never writes are exactly
the two the strings call maps**. The same six (count, stride, skipped prefix) triples
appear again in `g_syncBlocks` (`0x004D5B10`), read straight out of the exe.

**[V] The digest is a whole-record byte sum, not a field list.** `Sync_RecordDigest`
(`0x0044015A`) is

```c
uint8 Sync_RecordDigest(void *base, int size, int skip) {
    uint8 s = 0;
    for (int i = skip; i < size; i++) s += ((char *)base)[i];
    return s;
}
```

— every byte of the record except a fixed prefix, truncated to 8 bits, summed again
across records into one byte per block. **This is the direct answer to C30.** Our
`Canonical` digest is a hand-written field list and four `County` fields fell out of it;
the original cannot lose a field, because it never enumerates them. Whatever we do about
that, the original's answer is *"hash the record, minus a documented head"*, and the
heads are 5, 6, 0, `0x12`, 4 and 0 bytes — small enough to be scratch or render state,
which is worth checking against `docs/kingdom.md`'s county layout before copying.

**[V] One block is deliberately disabled.** The last statement before the total is
`g_syncDigest[localPlayer][7] = 1` — the `AI GROUP` (battle-unit) sum is computed and
then thrown away, replaced by a constant. Whatever divergence that block was catching,
the shipped build stopped catching it.

**How the check runs, and what happens on a mismatch.**

1. Every peer broadcasts its own 10-byte record (`Net_WriteField(&g_syncDigest[me], 10)`).
2. `Sync_BlockAgrees(block)` (`0x00440DD2`) first requires every live human realm to
   carry the *same non-zero frame tag* — otherwise it returns 2, "not everyone is here
   yet", and the caller stalls rather than declaring a divergence. Then it compares byte
   `block` across those realms; any difference sets a per-block flag in a 10-entry array
   at `0x0053F090` — one flag per group-14 name — and returns 0.
3. On 0, `Sync_Rollback` (`0x0043F5C5`) restores the last agreed snapshot. In battle it
   copies back `g_battleMen`, `g_missiles`, `g_battleUnits` **and both PRNG states**
   (`g_randStateA`, `g_randStateB`) — the whole deterministic state, PRNG included — then
   re-derives the caches and carries on.
4. A counter (`DAcc`) escalates. At 3 consecutive divergences it re-sends two players'
   state; at 4 it gives up, reloads `lastturn.sva`/`.svb` or restarts the battle, and
   posts **`L2.eng` group 259, "Com-link error."**
5. `Sync_ResendState` (`0x0043F939`) is the "master machine will resync" path of group 14
   index 16: 400 packets of 200 bytes — 80,000 bytes of raw snapshot — followed by a
   rollback.

**What this means for us** (`docs/netcode.md`): the original tolerates divergence rather
than aborting on it. It rolls back to the last agreed frame, retries three times, and
only then declares the session lost. It also proves the PRNG state must be inside the
snapshot, which our lockstep already assumes.

**[V] The group-14 UI is not in the retail build.** `_DAT_00554404` (the diverging record
index) and `_DAT_0056845C` (the diverging byte offset — which is what indices 10–14
bucket) are written by `Sync_CompareState` and `FUN_0043FC58` and **read by nothing**.
The two debug overlays that *do* ship — `FUN_00423BA4` (the multiplayer HUD:
`divergances`, `Dchk`, `D-F8`, `DAcc`) and `FUN_0044098F` (per-record digests) — use
inline C string literals `"cty "`, `"plyr "`, `"pce "`, not `L2.eng`. So group 14 is the
localisable version of a display that was later rewritten with hard-coded strings, and
its "Hit Numpad 7 to re-sync" hotkey is not wired up anywhere in this build.

### 5.3 Tip screens: an option nothing in this tree documented

Groups **200–219** are the first-time hint pop-ups, gated by `g_optTipScreens` (the
"Tip screens" toggle on the help-options panel, group 45 index 1).

`Tip_Update` (`0x00476AA7`) watches `g_screenId` and, twenty frames after a screen is
first opened, calls `Tip_Show(group)` (`0x00476DA9`), which sets a per-group "already
shown" byte in `g_tipShown` (`0x004F0298`, indexed by group id) and posts the group as a
message. Fourteen groups are wired: 200, 201, 202 (campaign map), 206 (zoomed-out view),
207 (town centre, screen 0x02), 208 (blacksmith popup), 209 (armoury, 0x17), 210 (army
movement, 0x10), 211 (invasions), 212 (field battle), 214 and 215 (siege), 217 (castle
building, 0x1B), 218 (advanced options, 0x39).

**[V] The message category is a table keyed by group id, and it encodes the paragraph
count.** `Tip_Show` reads `g_tipCategory[group]` at `0x004D6ED8`. Read out of the exe,
groups 200–219 give `7,8,9,5,5,5,5,8,5,8,7,5,6,5,7,5,5,9,7,5` — and for every one of the
twenty, **category = (strings in the group − 1) + 4**. Twenty independent chances to
disagree; none does. Categories 5–9 are therefore "tip window with 1–5 paragraphs" in
`Msg_DrawWindow`'s layout ladder.

`FUN_00476A5D` clears exactly 20 bytes from `g_tipShown + 200`, which fixes
the tip range as groups **200–219** inclusive.

### 5.4 The indirect routes

A group is reached without a literal in one of six ways. Each is a table or a dispatcher
worth knowing on its own:

| route | groups | mechanism |
|---|---|---|
| **`Msg_Enqueue`** (`0x00472BC5`) | 115 | the 24-byte message record carries an `L2.eng` group id and a variant; `Msg_DrawWindow` draws `(group, 0)` as the heading and `(group, variant + 1)` as the body. 142 call sites, 133 with a literal id. |
| **`g_menuBarItems`** (`0x004DC428`) | 1, 2, 3 | the 16-byte menu-bar record holds `engGroup` at +6 and its items hold `engIndex` at +2 — the drop-downs are pure data. |
| **`owner + 93`** | 94–98 | `UnitPanel_Draw` draws `Eng_DrawString(unit.owner + 93, unit.nameIndex)`; five realms, 24 army names each. |
| **the `g_eventTable` deck** (`0x004D6108`) | 135–142, 302–317 | `Event_RollAll` deals a group id straight out of a 256-slot deck into `county.eventId`; `FUN_00448D7E` posts it. The 24 ids are `0x87`–`0x8E` and `0x12E`–`0x13D`. |
| **`Diplo_Reply*` locals** | 171–176, 178, 179, 185, 188, 196, 197 | the reply tier is chosen against the AI personality and lands in a local that is then passed to `Msg_Enqueue` — see `docs/diplomacy.md`. |
| **`Tip_Show`** | 200–202, 206–212, 214, 215, 217, 218 | §5.3. |
| **`Industry_ToggleFromMap`** | 228–237 | `0xE6 + industry * 2 + on`; see `docs/kingdom.md` 7.4.1. |

Two more group-keyed tables are worth recording because they are the only places outside
the code that know a group id:

* **`g_msgVoice100` (`0x004DF9B8`)**, **`g_msgVoice200` (`0x004DFE18`)** and
  **`g_msgVoiceLord` (`0x004E0458`)** — 16-byte `.wav` filenames per group, so a message
  group's *voice* is addressed by group id too (`Msg_PlayVoice`).
* **`g_helpWindowGeom` (`0x004D6EB8`)** — six `{x, y, w, h}` records for groups 291–296,
  the message-category-`0x13` help windows, with the paragraph count for the same six in
  the dwords beginning at `0x004D6F18`. `paraCount == strings − 1` for all six.

### 5.5 Group 41 belongs to `mapl2.exe`

**[V]** No function in `Lords2.exe` names group 41 by a literal, and no computed path
reaches it. `mapl2.exe` — the 253 KB Battlemap editor shipped beside the game, still a
debug build with `C:\tools\map_l2\Debug\mapl2.pdb` in it — asks for it directly (§1.3).
Group 41 is that editor's entire interface: tool palette, brush sizes, the eight `.skr`
terrain names at indices 2–9, and its default map name/title/description at 27–29. A
reimplementation of the *game* can ignore it; a reimplementation of the *editor* needs it,
and the terrain names bear on the open questions in [`skr.md`](skr.md).

### 5.6 Two conventions the file uses that a reader will otherwise misread

* **`FREE` at index 0 does not mean "unused".** It means *this message has no heading of
  its own*. `Msg_DrawWindow` draws the county name — `Ui_DrawCentred(100, scenario * 20 +
  county)` — as the heading whenever the message record carries a county byte, and the
  group's own index 0 is then never read. That is groups 114–134, 258, 273, 276 and 277.
  `FREE` *inside* a group (43 indices 7–9, 62 indices 3–4, 49 indices 5–6) does mean an
  unused slot.
* **A group's index may run past its own end.** §1.3 — an empty or short group does not
  fail the lookup, the cursor simply walks into the next group's strings. Group 93 relies
  on this; group 215 falls into it by accident.

### 5.7 What this closed, and what it did not

Closed:

* `maps.md` / §1.4's `[I]` that group 100 is 60 maps × 20 counties: the stride is `0x14`
  in `Msg_DrawWindow` and `Diplo_DrawCountyRequest`, so it is `[V]`.
* `hypotheses.json`'s `Battle_ReplayFrame`, whose caveat said "whether it is a replay, a
  demo loop or a network resync was NOT established". It is the network resync;
  promoted to `symbols.json` as `Sync_Rollback`.
* Group 14, group 41 and the tip-screen subsystem, none of which was in any document.

Not closed, and deliberately flagged rather than committed:

* The **superseded-panel** reading of groups 9, 43, 44, 57, 60, 62, 63, 64, 65 and 88 is
  internally coherent and touches almost nothing external — exactly the shape
  `docs/decisions.md` C3 warns about. Group 64 ⊂ group 86 is a real string-level check;
  the rest is the same argument repeated. It should not be cited as fact.
* **Which skipped prefix is which field.** The six `(size, skip)` pairs are `[V]`; what
  lives in the first 5 bytes of a county or the first `0x12` of a battle man is not
  checked here, and it matters before we copy the scheme.
* The **13 groups that differ between the DOS and Windows `L2.eng`** are still not
  diffed, and this census is of the Windows file only.

### 5.8 The table

Legend: **[V]** the binary names this group at that call site, or a table read out of
`Lords2.exe` holds the id. **[D]** derived — reached through a dispatcher with a computed
id, named in §5.4. **[I]** inferred from the strings; no code path reaches it.

| Group | n | Label (index 0) | What it is for | Reached by |
|---:|---:|---|---|---|
| 1 | 5 | `File` | Menu bar: File drop-down (New Game / Load / Save / Quit). | `g_menuBarItems`+0x6 [V] |
| 2 | 6 | `Options` | Menu bar: Options drop-down (Advanced / Sounds / Display / Game speed / Scroll speed). | `g_menuBarItems`+0x6 [V] |
| 3 | 8 | `Help` | Menu bar: Help drop-down (Game Help / How do I… / Grow grain / Build castle / Make weapons / Manage turn / About). | `g_menuBarItems`+0x6 [V] |
| 4 | 1 | `End turn` | The End turn button caption. | `Screen_DrawEndTurn` [V] |
| 5 | 8 | `Jock McTooth` | The twelve merchant names ("Jock McTooth", "Fat Barry"…) shown on the army/merchant panel. | `UnitPanel_Draw` [V] |
| 6 | 15 | `No goods` | The fifteen goods, index = goods id (0 none, 1 grain … 14 mail). | `FUN_0041608b`, `Trade_DrawPanel` [V] |
| 7 | 5 | `No player` | The four AI lord titles + "No player"; new-game setup copies index `realm[+0x07]` into `g_playerNames`. | [V] also `Eng_Seek(7, lord)` |
| 8 | 74 | `Crown.` | Countable nouns, singular/plural pairs — `Ui_DrawCount` picks the pair, `Ui_DrawUnitNoun` the troop type. | `Ui_DrawCount`, `Ui_DrawUnitNoun` [V] |
| 9 | 6 | `No player` | "Player1".."Player5" — generic player labels. Superseded by `g_playerNames` (group 7 + the typed name). | [I] dead |
| 10 | 15 | `Exit the game?` | The yes/no confirm box prompt list; the caller passes the index. | `Screen_ConfirmBox` [V] |
| 11 | 50 | `Lords of the Realm 2` | The whole front end: title, single/multi player, Play Now!, Custom game, the skirmish setup, and the five difficulty words the `TROOPS*.ENG` table is indexed by. | `FUN_0041ea14`, `FUN_0041eaa3`, `FUN_0041ec8a`, `FUN_0041ece6`, `FUN_0041ef57`, `FUN_0041f01f`, `FUN_0041f6c7`, `FUN_0041f77a`, `FUN_00420428`, `FUN_0042051c`, `FUN_00420630`, `FUN_004207c3`, `FUN_004209c1`, `FUN_00420de4`, `FUN_0042130f`, `FUN_0042150b` [V] |
| 12 | 6 | `Click Right to Exit` | The value-spinner captions (game speed, scroll speed, music level…). | `Screen_SliderBox` [V] |
| 13 | 3 | `Click to Continue` | "Click to Continue" / "Game Paused" overlay captions. | `FUN_0040cf09` [V] |
| 14 | 17 | `Divergence error. Current count of` | **Multiplayer divergence report.** Indices 2–9 name the eight `g_syncDigest` byte slots; 10–14 bucket the differing byte offset; 15–16 are the resync notice. See §5.2 — the detector ships, this UI does not. | [I] dead (the shipped overlay uses inline literals) |
| 15 | 2 | `Sovereign land` | County-strip banner: "Sovereign land of <county>". | `CountyStrip_Draw` [V] |
| 16 | 13 | `No mercenaries in the army.` | Mercenary nationalities + the "no mercenaries" line on the army panels. | `Screen_RaiseArmy`, `Screen_SplitArmyRows`, `UnitPanel_Draw` [V] |
| 17 | 2 | `Army Division.` | Army-division screen title and its split prompt. | `Screen_ArmyDivision` [V] |
| 18 | 3 | `Yes` | Yes / No / Cancel button captions. | `Screen_AdvancedOptions`, `Screen_DisplayOptions`, `Screen_HelpOptions`, `Screen_RaiseArmy` [V] |
| 19 | 3 | `On` | On / Off / Cancel button captions. | `Screen_SoundOptions`, `Screen_DisplayOptions` [V] |
| 20 | 5 | `Diseased` | The five health words, index = health band. | `Panel_Ration` [V] |
| 21 | 6 | `None` | The six ration levels (None … Double). | `CountyStrip_Draw`, `Panel_Ration` [V] |
| 22 | 7 | `Infertile - almost no production.` | The seven fertility phrases, index = fertility band. | `Village_Draw`, `Panel_JobGrain`, `TileInfo_DrawGrain` [V] |
| 23 | 1 | `Cancel` | A lone "Cancel". Superseded by group 18 index 2 / group 19 index 2. | [I] dead |
| 24 | 1 | `Cancel` | A second lone "Cancel", byte-identical to group 23. | [I] dead |
| 25 | 12 | `January` | The twelve month names. The shipped game keeps time in seasons (group 29) and weeks; no month is ever drawn. | [I] dead — calendar granularity that was cut |
| 26 | 2 | `BC` | "BC" / "AD" suffix for the year. | `Ui_DrawYear` [V] |
| 27 | 1 | `To` | A lone "To". | [I] dead |
| 28 | 4 | `Week 1` | "Week 1".."Week 4". Same story as group 25 — the season is the unit that shipped. | [I] dead |
| 29 | 5 | `No Season` | The four seasons + "No Season", drawn on the menu bar. | `Screen_DrawMenuBar` [V] |
| 30 | 88 | `Scrubland.` | Campaign-map terrain and tile descriptions, 88 strings — the right-click info text for every map graphic. | `TileInfo_Draw`, `FUN_0041c996` [V] |
| 31 | 32 | `Merchant.` | Unit/merchant/transport panel vocabulary; index 27+starvation is the army-discontent phrase `Readme.txt` describes. | `FUN_0041b081`, `UnitPanel_Draw` [V] |
| 32 | 1 | `Battle Paused` | "Battle Paused" overlay. | `FUN_00423b4f` [V] |
| 33 | 8 | `Send supplies` | The send-supplies screen. | `Screen_SendSupplies`, `FUN_0041aea2` [V] |
| 34 | 2 | `Year` | Campaign-map header ("Year" / "Click on the county you wish to view."). | `Screen_DrawCampaign` [V] |
| 35 | 8 | `Most counties,` | The seven "greatest noble" categories (most counties / castles / troops / crowns …). | `Screen_GreatestNoble` [V] |
| 36 | 19 | `Congratulations!!` | The Play Now!! conquest inter-map screen — `Readme.txt` calls it the eight-map campaign. | `Screen_DrawConquest` [V] |
| 37 | 6 | `Battle Master ratings` | Battle Master ratings table headings. | `Screen_BattleMasterRatings`, `Screen_BattleMasterRank` [V] |
| 38 | 12 | `Rank of Private` | The twelve Battle Master ranks (Private … ). | `Screen_BattleMasterRank` [V] |
| 39 | 6 | `Expansion pack installed, choose:-` | Expansion-pack / session-type chooser. | `FUN_0041f3e9`, `FUN_0041f4a1`, `FUN_0041f592`, `FUN_0041f5e8` [V] |
| 40 | 9 | `Loading a conquest.` | Save/load status lines, including the conquest (campaign) save. | `Screen_SaveLoad`, `FUN_004148e4`, `SaveLoad_DrawStatus`, `FUN_0042150b` [V] |
| 41 | 30 | `Lords2 Skirmish map editor.` | **`mapl2.exe`, not `Lords2.exe`.** The Battlemap editor: its tool palette, its eight `.skr` terrain names (indices 2–9) and its default map name/title/description (27–29). No function in `Lords2.exe` touches it. | [V] other executable |
| 42 | 0 | — | Empty. | [I] empty |
| 43 | 18 | `Lords II - Game Options` | An earlier new-game/skill screen ("Choose a Skill Level", "Run the LORDS II Tutorial"). Superseded by group 11. | [I] dead |
| 44 | 5 | `Novice` | Novice/Easy/Normal/Hard/Expert. Superseded by group 103 (off/on/easy/normal/hard/impossible) and group 11 42–46. | [I] dead |
| 45 | 4 | `Help Options` | Help-options panel (tip screens / tool tips / start game help). | `Screen_HelpOptions` [V] |
| 46 | 2 | `GAME OVER` | "GAME OVER" — index 1 is literally `"Medieval banter goes here......."`. **A placeholder that was never written and is never drawn**; the shipped endgame text is groups 224/225 and 238/239. | [I] dead placeholder |
| 47 | 10 | `Group Information` | A cut battle "Group Information" panel. Its unit classes — Heavy Infantry, Light Infantry, Slingers, Mixed Troops, Auxiliaries — are not Lords II troop types. | [I] dead — cut content |
| 48 | 3 | `Exit the game?` | The exit/save confirm box. | `FUN_00414790` [V] |
| 49 | 12 | `Congratulations!` | The tutorial shell (section-complete text, back/forward buttons, "Exit the Tutorial"). No code reaches it; the shipped tutorial is a scripted campaign, not this screen. | [I] dead |
| 50 | 5 | `Advanced options.` | Advanced options panel (Advanced farming / Army foraging / Exploration). | `Screen_AdvancedOptions` [V] |
| 51 | 4 | `Sounds` | Sound options panel. | `Screen_SoundOptions` [V] |
| 52 | 4 | `Display options` | Display options panel. | `Screen_DisplayOptions` [V] |
| 53 | 0 | — | Empty. | [I] empty |
| 54 | 0 | — | Empty. | [I] empty |
| 55 | 0 | — | Empty. | [I] empty |
| 56 | 0 | — | Empty. | [I] empty |
| 57 | 10 | `Music is` | An earlier combined sound/display options panel. Superseded by groups 51, 52 and 19. | [I] dead |
| 58 | 0 | — | Empty. | [I] empty |
| 59 | 3 | `Lords 2.` | The About box — version string and copyright. | `Screen_About` [V] |
| 60 | 3 | `People` | "People / Man. / Men." Superseded by group 8, whose 74 strings carry every singular/plural pair. | [I] dead |
| 61 | 2 | `Tax` | County-strip "Tax" / "Ration" labels. | `CountyStrip_Draw` [V] |
| 62 | 39 | `Wanted:` | **A cut food-and-rations panel**, 39 strings, far richer than the one that shipped: per-food "Dairy produce feeds N", "Cows will remain", forager theft, and the four grain stages. Superseded by groups 87 and 77. | [I] dead — cut content |
| 63 | 6 | `Fertility-` | "Fertility-" plus five field-change forecasts. Superseded by group 22. | [I] dead |
| 64 | 2 | `Tax rate` | "Tax rate" / "People pay" — a two-string subset of group 86, which is what `Panel_Tax` draws. | [I] dead |
| 65 | 6 | `Health-` | "Health-" plus births/deaths/migration lines. Superseded by groups 73 and 20. | [I] dead |
| 66 | 12 | `Frost` | The six weather events and their crop effects. | `Village_Draw` [V] |
| 67 | 2 | `Free` | "Free" / "Some info goes here" — an unfilled slot. | [I] dead placeholder |
| 68 | 20 | `Click on a price to trade with` | The merchant trade panel. | `Trade_DrawPanel` [V] |
| 69 | 17 | `crowns to hire.` | Mercenary hire, the raise-army screen **and the armoury**, which share it: 0…4 and 10…16 are the levy window, **6, 7 and 8 are the armoury's three buttons — *Create*, *Change*, *Cancel*** — 9 is *"Continue"*, the button that leaves the levy screen for the armoury, and 5 is the rack panel's *"more could still be raised."* `screens/shells.rs` filed screen 0x0A under group **16** (the nationalities) until this row was read the other way; `docs/decisions.md` C61. | `Screen_Armoury`, `Screen_ArmouryRepaint`, `Screen_RaiseArmy`, `FUN_00418e2d` [V] |
| 70 | 9 | `Gold` | The court screen: gold, arms, iron, stone, wood totals. | `Court_Draw` [V] |
| 71 | 20 | `Select a castle to build` | Castle selection and status; index 1..5 are the five castle types, 0x12 the "materials still needed" line. | `CountyStrip_DrawCastleIcon`, `Screen_CastleBuildPanel`, `TileInfo_DrawCastle`, `Castle_DrawStatusBlock` [V] |
| 72 | 25 | `Diplomacy.` | The diplomacy screen and its four action layouts. | `Diplo_DrawScreen`, `Diplo_DrawGiftGold`, `Diplo_DrawLetter`, `Diplo_DrawCountyRequest` [V] |
| 73 | 12 | `Population in` | The population panel (last season / births / deaths / migration). | `Panel_Population` [V] |
| 74 | 10 | `Idle people.` | The nine labour jobs, index = job id — the job popup title. | `Panel_JobDetail`, `Panel_JobBlacksmith` [V] |
| 75 | 3 | `Click on a weapon to change production.` | The armoury/blacksmith production panel ("Click on a weapon to change production", wood needed, iron needed). | `Panel_JobBlacksmith` [V] |
| 76 | 9 | `Working with an efficiency of` | Industry efficiency and output forecast lines. | `Panel_JobIndustry`, `Panel_JobBlacksmith` [V] |
| 77 | 31 | `from` | Grain, herd and reclamation forecast vocabulary — the four grain stages and the calf/cow birth-and-death lines. | `Panel_JobGrain`, `Panel_JobCattle`, `Panel_JobReclamation`, `TileInfo_DrawGrain`, `TileInfo_DrawHerd`, `Msg_DrawWindow` [V] |
| 78 | 0 | — | Empty. | [I] empty |
| 79 | 0 | — | Empty. | [I] empty |
| 80 | 8 | `A Battle is to be fought.` | The pre-battle prompt (take the field / autocalc). | `Screen_BattlePrompt` [V] |
| 81 | 9 | `The Battle is decided.` | The battle-decided screen. | `Screen_BattleResult` [V] |
| 82 | 14 | `The Battle is won.` | The seven battle outcome heading/body pairs `Battle_CheckOutcome` selects (C31). | `Screen_BattleOutcome` [V] |
| 83 | 9 | `Siege preparations.` | Siege preparation: the four siege-engine types and their counts. | `Screen_SiegePrep` [V] |
| 84 | 0 | — | Empty. | [I] empty |
| 85 | 10 | `Happiness in` | The happiness panel and its five contributing terms. | `Panel_Happiness` [V] |
| 86 | 5 | `Tax in` | The tax panel. | `Panel_Tax` [V] |
| 87 | 12 | `Ration` | The ration panel (wanted / achieved / health). | `Panel_Ration` [V] |
| 88 | 4 | `INVALID PLAYER` | Multiplayer refusal text ("INVALID PLAYER", "skirmish only version", the 256-colour warning). Duplicated verbatim by group 298, which is the one that ships. | [I] dead |
| 89 | 3 | `Smack file testing` | A Smacker-file test harness ("Smack file testing" / "Start test?"). | [V] developer screen |
| 90 | 0 | — | Empty. | [I] empty |
| 91 | 0 | — | Empty. | [I] empty |
| 92 | 0 | — | Empty. | [I] empty |
| 93 | 0 | — | Empty — but structurally the base of the army-name lookup, `Eng_DrawString(owner + 93, unit.nameIndex)`. Owner 0 lands here and falls through into group 94. | [V] base of `owner + 93` |
| 94 | 24 | `"The Lions."` | Realm 1's 24 army names ("The Lions", "The Dragons"…), indexed by the unit's own `nameIndex`. | `UnitPanel_Draw` `owner + 93` [D] |
| 95 | 24 | `"The Invincibles."` | Realm 2's 24 army names. | `UnitPanel_Draw` `owner + 93` [D] |
| 96 | 24 | `"The Greeks."` | Realm 3's 24 army names. | `UnitPanel_Draw` `owner + 93` [D] |
| 97 | 24 | `"The Crushers."` | Realm 4's 24 army names. | `UnitPanel_Draw` `owner + 93` [D] |
| 98 | 24 | `"The Swans."` | Realm 5's 24 army names. | `UnitPanel_Draw` `owner + 93` [D] |
| 99 | 1 | `"The people."` | "The people." — the neutral county militia's army name. | `Screen_BattlePrompt`, `Screen_BattleResult` [V] |
| 100 | 1200 | `Here Be Dragons!` | **County names, 60 scenarios × 20 slots**, indexed `g_scenarioIndex * 20 + county`. §1.4 inferred the 60×20 shape; the stride `0x14` in `Msg_DrawWindow` and `Ui_DrawCentred` makes it [V]. | `CountyStrip_Draw`, `Panel_Population`, `Panel_Happiness`, `Diplo_DrawCountyRequest`, `Screen_RaiseArmy`, `Screen_SendSupplies`, `UnitPanel_Draw`, `TileInfo_Draw`, `Screen_BattlePrompt`, `Screen_BattleResult`, `Msg_DrawWindow`, `Msg_DrawDiplomacy` [V] |
| 101 | 60 | `England` | The 60 campaign map names — see §1.4. | `Screen_DrawCampaign`, `Screen_DrawConquest`, `ScenarioList_Draw` [V] |
| 102 | 12 | `Advanced Farming` | The twelve custom-game option names. | `FUN_0041f86d` [V] |
| 103 | 46 | `off` | The 46 option *values* (off/on/easy/normal/hard/impossible/…), paired with group 102. | `FUN_0041f86d`, `FUN_0041fdd6` [V] |
| 104 | 0 | — | Empty. | [I] empty |
| 105 | 0 | — | Empty. | [I] empty |
| 106 | 0 | — | Empty. | [I] empty |
| 107 | 0 | — | Empty. | [I] empty |
| 108 | 2 | `A message has arrived.` | "A message has arrived." / "N messages are waiting." — an unused message-count banner. | [I] dead |
| 109 | 2 | `From` | "From" / "An envoy from" — the sender line every message window draws. | `Msg_DrawWindow`, `Msg_DrawDiplomacy`, `Msg_DrawBeyondLetter` [V] |
| 110 | 2 | `Sieges only!` | Refusal: a siege-only command used in a field battle. | `Msg_Enqueue` <- `FUN_0043bbe7` [D] |
| 111 | 2 | `No drawbridge!` | Refusal: no drawbridge to lower. | `Msg_Enqueue` <- `FUN_0043bbe7` [D] |
| 112 | 2 | `No right of rule!!` | Refusal: no right of rule in this county. | `Msg_Enqueue` <- `FUN_00436a88`, `Sidebar_Button`, `Map_Click` [D] |
| 113 | 2 | `No people to arm!` | Refusal: no people allocated to arm. Reachable by no path found. | [I] dead |
| 114 | 2 | `FREE` | Conquest narration, one rung per counties-held tally; index 0 is `FREE` because the heading is the county name (group 100). | `Msg_Enqueue` <- `FUN_004a72fe` [D] |
| 115 | 2 | `FREE` | Conquest narration, one rung per counties-held tally; index 0 is `FREE` because the heading is the county name (group 100). | `Msg_Enqueue` <- `FUN_004a72fe` [D] |
| 116 | 2 | `FREE` | Conquest narration, one rung per counties-held tally; index 0 is `FREE` because the heading is the county name (group 100). | `Msg_Enqueue` <- `FUN_004a72fe` [D] |
| 117 | 2 | `FREE` | Conquest narration, one rung per counties-held tally; index 0 is `FREE` because the heading is the county name (group 100). | `Msg_Enqueue` <- `FUN_004a72fe` [D] |
| 118 | 2 | `FREE` | Conquest narration, one rung per counties-held tally; index 0 is `FREE` because the heading is the county name (group 100). | `Msg_Enqueue` <- `FUN_004a72fe` [D] |
| 119 | 2 | `FREE` | Conquest narration, one rung per counties-held tally; index 0 is `FREE` because the heading is the county name (group 100). | `Msg_Enqueue` <- `FUN_004a72fe` [D] |
| 120 | 2 | `FREE` | Conquest narration, one rung per counties-held tally; index 0 is `FREE` because the heading is the county name (group 100). | `Msg_Enqueue` <- `FUN_004a72fe` [D] |
| 121 | 2 | `FREE` | Conquest narration, one rung per counties-held tally; index 0 is `FREE` because the heading is the county name (group 100). | `Msg_Enqueue` <- `FUN_004a72fe` [D] |
| 122 | 2 | `FREE` | Conquest narration, one rung per counties-held tally; index 0 is `FREE` because the heading is the county name (group 100). | `Msg_Enqueue` <- `FUN_004a72fe` [D] |
| 123 | 2 | `FREE` | Conquest narration, one rung per counties-held tally; index 0 is `FREE` because the heading is the county name (group 100). | `Msg_Enqueue` <- `FUN_004a72fe` [D] |
| 124 | 2 | `FREE` | The final-conquest congratulation of the same series. No call site passes 124; `FUN_004a72fe` stops at 123 and 125. | [I] dead |
| 125 | 2 | `FREE` | Conquest narration, one rung per counties-held tally; index 0 is `FREE` because the heading is the county name (group 100). | `Msg_Enqueue` <- `FUN_004a72fe` [D] |
| 126 | 2 | `FREE` | Conquest narration, one rung per counties-held tally; index 0 is `FREE` because the heading is the county name (group 100). | `Msg_Enqueue` <- `FUN_004a72fe` [D] |
| 127 | 2 | `FREE` | **Secession**: "Deeming itself too far from the heart of your empire…". | `Territory_SecedeMinorBlocks` [D] |
| 128 | 2 | `Your lands divide.` | "Your lands divide." — the kingdom-level companion to 127. | `Territory_SecedeMinorBlocks` [D] |
| 129 | 2 | `FREE` | Refusal: a county too far from the heart of your lands. | `FUN_004a72fe` [D] |
| 130 | 2 | `FREE` | County greeting/warning as an army enters, one per standing band. | `County_GreetArmy` [D] |
| 131 | 2 | `FREE` | County greeting/warning as an army enters, one per standing band. | `County_GreetArmy` [D] |
| 132 | 2 | `FREE` | County greeting/warning as an army enters, one per standing band. | `County_GreetArmy` [D] |
| 133 | 2 | `FREE` | County greeting/warning as an army enters, one per standing band. | `County_GreetArmy` [D] |
| 134 | 2 | `FREE` | County greeting/warning as an army enters, one per standing band. | `County_GreetArmy` [D] |
| 135 | 2 | `Rats!!` | Random county event, dealt from the `g_eventTable` deck (`docs/kingdom.md`). | `Event_RollAll` deck [D] |
| 136 | 2 | `Mad Cows !!` | Random county event, dealt from the `g_eventTable` deck (`docs/kingdom.md`). | `Event_RollAll` deck [D] |
| 137 | 2 | `Wolves.` | Random county event, dealt from the `g_eventTable` deck (`docs/kingdom.md`). | `Event_RollAll` deck [D] |
| 138 | 2 | `Plague.` | Random county event, dealt from the `g_eventTable` deck (`docs/kingdom.md`). | `Event_RollAll` deck [D] |
| 139 | 2 | `Grain found.` | Random county event, dealt from the `g_eventTable` deck (`docs/kingdom.md`). | `Event_RollAll` deck [D] |
| 140 | 2 | `Bad cattle stock` | Random county event, dealt from the `g_eventTable` deck (`docs/kingdom.md`). | `Event_RollAll` deck [D] |
| 141 | 2 | `Cow bonanza!!` | Random county event, dealt from the `g_eventTable` deck (`docs/kingdom.md`). | `Event_RollAll` deck [D] |
| 142 | 2 | `Wedding fever.` | Random county event, dealt from the `g_eventTable` deck (`docs/kingdom.md`). | `Event_RollAll` deck [D] |
| 143 | 2 | `Drought.` | Weather event: drought. | `Weather_UpdateAll` [D] |
| 144 | 2 | `Flooding.` | Weather event: flooding. | `Weather_UpdateAll` [D] |
| 145 | 2 | `Cannot disband army` | Refusal: army must disband in its county of origin. | `Panel_DisbandButton` [D] |
| 146 | 2 | `Uncertain times.` | Unrest level 0 warning. | `Unrest_UpdateAll` [D] |
| 147 | 2 | `Cannot alter castle.` | Refusal: castle already of the proposed type. | `Msg_Enqueue` <- `FUN_00436b59` [D] |
| 148 | 2 | `Army too small.` | Refusal: army below the minimum size. | `Msg_Enqueue` <- `FUN_00435b4d`, `FUN_00437afb` [D] |
| 149 | 2 | `Cannot split army` | Refusal: splitting needs 15 movement points. | `Msg_Enqueue` <- `FUN_004378b3` [D] |
| 150 | 2 | `Murmurs of unrest.` | Unrest level 1. | `Unrest_UpdateAll` [D] |
| 151 | 2 | `Trouble in the county.` | Unrest level 2. | `Unrest_UpdateAll` [D] |
| 152 | 2 | `Uproar in the shire.` | Unrest level 3. | `Unrest_UpdateAll` [D] |
| 153 | 2 | `Revolution in your lands.` | Unrest level 4 — revolution. | `Unrest_UpdateAll` [D] |
| 154 | 2 | `Revolution in your lands.` | Unrest spreading from a neighbouring county. | `FUN_004abd0f` [D] |
| 155 | 2 | `Trouble spreading.` | Unrest spreading from a neighbouring county. | `FUN_004abd0f` [D] |
| 156 | 2 | `People are troubled.` | Unrest spreading from a neighbouring county. | `FUN_004abd0f` [D] |
| 157 | 2 | `Drawbridge is down.` | Refusal: drawbridge already down. | `Msg_Enqueue` <- `FUN_0043bbe7` [D] |
| 158 | 2 | `Congratulations.` | Skirmish/demo victory teaser ("In the forthcoming Lords of the Realm…"). `Msg_Dismiss` still special-cases 158/159/238/239 into screen 0x26, but **nothing enqueues them**. | [I] dead — the poster was removed, the handler was not |
| 159 | 2 | `Defeat` | Skirmish/demo defeat teaser; see 158. | [I] dead |
| 160 | 2 | `Mercenaries desert!` | Mercenaries desert for want of pay. | `Wages_PayAll` [D] |
| 161 | 2 | `Supplies lost.` | Supply transport intercepted (yours). | `Unit_EnterOccupiedTile` [D] |
| 162 | 2 | `Supplies destroyed.` | Supply transport destroyed (theirs). | `Unit_EnterOccupiedTile` [D] |
| 163 | 8 | `Castle complete` | Castle complete, with the tax-revenue variants. | `Castle_BuildTick` [D] |
| 164 | 2 | `Cannot garrison castle.` | Refusal: castle under construction. | `Msg_Enqueue` <- `MoveOrder_ConfirmGarrison` [D] |
| 165 | 2 | `Cannot garrison castle.` | Refusal: castle is ruined. | `Msg_Enqueue` <- `MoveOrder_ConfirmGarrison` [D] |
| 166 | 5 | `Cannot garrison castle.` | Refusal: castle capacity, with the "N more troops" variants. | `Msg_Enqueue` <- `MoveOrder_ConfirmGarrison` [D] |
| 167 | 2 | `Cannot combine armies.` | Refusal: those mercenaries will not fight together. | `Msg_Enqueue` <- `MoveOrder_ConfirmGarrison`, `Army_Combine` [D] |
| 168 | 2 | `Cannot raise army.` | Refusal: army of zero men. | `Msg_Enqueue` <- `FUN_00435b4d` [D] |
| 169 | 2 | `Cannot build weapon` | "You cannot build this weapon type in the Lords 2 demo." | [I] dead — demo-only |
| 170 | 17 | `Invasion of` | Invasion declaration, 16 takes (4 lords × 4). | `Unit_EnterCounty` [D] |
| 171 | 17 | `Reply to gift.` | Reply to a generous gift (+10 standing). | `Diplo_ReplyGift` local [D] |
| 172 | 17 | `Reply to gift.` | Reply to an adequate gift (+5). | `Diplo_ReplyGift` local [D] |
| 173 | 17 | `Reply to gift.` | Reply to a mean gift (-8). | `Diplo_ReplyGift` local [D] |
| 174 | 17 | `Reply to compliment.` | Reply to a first compliment. | `Diplo_ReplyCompliment` local [D] |
| 175 | 17 | `Reply to compliment.` | Reply to a repeated compliment. | `Diplo_ReplyCompliment` local [D] |
| 176 | 17 | `Reply to compliment.` | Reply to compliment spam. | `Diplo_ReplyCompliment` local [D] |
| 177 | 17 | `Reply to Insult.` | Reply to an insult. | `Diplo_ReplyInsult` [D] |
| 178 | 17 | `Reply to alliance offer.` | Alliance offer refused. | `Diplo_ReplyAllianceOffer` local [D] |
| 179 | 17 | `Reply to alliance offer.` | Alliance offer accepted. | `Diplo_ReplyAllianceOffer` local [D] |
| 180 | 17 | `Accept alliance ?` | AI proposes an alliance. | `AI_Diplomacy` [D] |
| 181 | 17 | `End of alliance.` | AI ends an alliance. | `AI_Diplomacy` [D] |
| 182 | 17 | `Broken alliance.` | AI reacts to *your* broken alliance. | `Diplo_Offend` [D] |
| 183 | 17 | `Help in` | Help request refused. | `Diplo_ReplyHelpRequest` [D] |
| 184 | 17 | `Help in` | Help request granted. | `Diplo_ReplyHelpRequest` [D] |
| 185 | 17 | `Pay -` | Help request granted for a price. | `Diplo_ReplyHelpRequest` local [D] |
| 186 | 17 | `Attack of` | Attack request refused. | `Diplo_ReplyAttackRequest` [D] |
| 187 | 17 | `Attack of` | Attack request granted. | `Diplo_ReplyAttackRequest` [D] |
| 188 | 17 | `Pay -` | Attack request granted for a price. | `Diplo_ReplyAttackRequest` local [D] |
| 189 | 17 | `Warning.` | First border warning. | `Diplo_Offend` [D] |
| 190 | 17 | `Warning.` | Second border warning. | `Diplo_Offend` [D] |
| 191 | 17 | `Notice of revenge.` | Notice of revenge. | `Diplo_Offend` [D] |
| 192 | 17 | `Helpful advice.` | Unsolicited AI advice when you are doing badly. | `AI_Taunt` [D] |
| 193 | 17 | `How are you doing?` | Unsolicited AI boast. | `AI_Taunt` [D] |
| 194 | 17 | `Foiled again.` | AI lament on losing ground. | `FUN_0049b42b` [D] |
| 195 | 17 | `Just call me king.` | AI victory gloat. | `Score_RankRealms` [D] |
| 196 | 17 | `Retort to alliance offer.` | Alliance offer refused with hostility. | `Diplo_ReplyAllianceOffer` local [D] |
| 197 | 17 | `Reply to alliance offer.` | Alliance offer refused — already allied. | `Diplo_ReplyAllianceOffer` local [D] |
| 198 | 0 | — | Empty. | [I] empty |
| 199 | 0 | — | Empty. | [I] empty |
| 200 | 4 | `Game Objectives:` | Tip screen (§5.3) — Campaign map, first sight. | `Tip_Show` <- `Tip_Update` [D] |
| 201 | 5 | `Getting started:` | Tip screen (§5.3) — Getting started. | `Tip_Show` [D] |
| 202 | 6 | `Food and Happiness:` | Tip screen (§5.3) — Food and happiness. | `Tip_Show` [D] |
| 203 | 1 | `Free lead in` | Reserved tip slot; the string is the literal placeholder "Free lead in" and no code passes the id. | [I] dead placeholder |
| 204 | 1 | `Free lead in` | Reserved tip slot; the string is the literal placeholder "Free lead in" and no code passes the id. | [I] dead placeholder |
| 205 | 1 | `Free lead in` | Reserved tip slot; the string is the literal placeholder "Free lead in" and no code passes the id. | [I] dead placeholder |
| 206 | 2 | `Kingdom overview:` | Tip screen (§5.3) — Kingdom (zoomed-out) view. | `Tip_Show` [D] |
| 207 | 5 | `The Town Center:` | Tip screen (§5.3) — The town centre, screen 0x02. | `Tip_Show` [D] |
| 208 | 2 | `The Blacksmith:` | Tip screen (§5.3) — The blacksmith job popup. | `Tip_Show` [D] |
| 209 | 5 | `The Armoury:` | Tip screen (§5.3) — The armoury, screen 0x17. | `Tip_Show` [D] |
| 210 | 4 | `Army Movement:` | Tip screen (§5.3) — Army movement, screen 0x10. | `Tip_Show` [D] |
| 211 | 2 | `Invasions:` | Tip screen (§5.3) — Invasions. | `Tip_Show` [D] |
| 212 | 3 | `Battles:` | Tip screen (§5.3) — Field battle, first entry. | `Tip_Show` [D] |
| 213 | 1 | `Battles2` | Reserved second page of the battle tip; label only, never passed. | [I] dead placeholder |
| 214 | 4 | `Sieges:` | Tip screen (§5.3) — Siege, first entry. | `Tip_Show` [D] |
| 215 | 1 | `Sieges2` | Third siege tip. The id **is** passed (`0xD7`), but the group holds only its label, so the body draw at index 1 walks past the group end — §1.3 says the format permits that; the resulting text was not observed running. | [D] reached, content missing |
| 216 | 1 | `Sieges3` | Reserved third page of the siege tip; label only. | [I] dead placeholder |
| 217 | 6 | `Castle Building:` | Tip screen (§5.3) — Castle building, screen 0x1B. | `Tip_Show` [D] |
| 218 | 4 | `Advanced Game Options:` | Tip screen (§5.3) — Advanced game options, screen 0x39. | `Tip_Show` [D] |
| 219 | 2 | `Already in alliance.` | Refusal: that player is already in an alliance. | `Diplo_SendClicked` [D] |
| 220 | 35 | `Null tool tip` | The 35 tool tips, index = hotspot id; drawn by the tooltip layer when `g_optToolTips` is on. | `FUN_00476e95` [V] |
| 221 | 2 | `INTERNAL ERROR` | "INTERNAL ERROR — Unable to build an army." | `Msg_Enqueue` <- `FUN_00435b4d` [D] |
| 222 | 2 | `Demo  Version` | "This function is not available in the Lords2 Demonstration version." | [I] dead — demo-only |
| 223 | 2 | `No modem play` | "This version of the game does not yet support linked modem play." | [I] dead |
| 224 | 3 | `Defeat!` | Defeat — your realm is eliminated. | `Realm_Eliminate` [D] |
| 225 | 2 | `Victory!` | Victory — the crown is yours. | `Score_RankRealms` [D] |
| 226 | 4 | `Verily, your oppression of the weak and your flattery of the strong are worthy of emulation.  Pray, teach me more.` | The four ceremonial replies the diplomacy letter composer offers. | `FUN_004ae310` [V] |
| 227 | 2 | `No access` | Refusal: the session creator controls that option. | `Menu_AdvancedOptions` [D] |
| 228 | 1 | `Building off` | Castle building switched **off** from the map. | `Industry_ToggleFromMap` `0xE6 + i*2 + on` [D] |
| 229 | 1 | `Building on` | Castle building switched **on** from the map. | `Industry_ToggleFromMap` [D] |
| 230 | 1 | `Forestry off` | Forestry off. | `Industry_ToggleFromMap` [D] |
| 231 | 1 | `Forestry on` | Forestry on. | `Industry_ToggleFromMap` [D] |
| 232 | 1 | `Mining off` | Iron mining off. | `Industry_ToggleFromMap` [D] |
| 233 | 1 | `Mining on` | Iron mining on. | `Industry_ToggleFromMap` [D] |
| 234 | 1 | `Blacksmith off` | Blacksmith off. | `Industry_ToggleFromMap` [D] |
| 235 | 1 | `Blacksmith on` | Blacksmith on. | `Industry_ToggleFromMap` [D] |
| 236 | 1 | `Quarrying off` | Quarrying off. | `Industry_ToggleFromMap` [D] |
| 237 | 1 | `Quarrying on` | Quarrying on. | `Industry_ToggleFromMap` [D] |
| 238 | 2 | `Congratulations.` | Conquest win against the Knight — the Play Now!! ladder's first rung; see 158. | [I] dead in this build |
| 239 | 2 | `Defeat!.` | Conquest loss to the Knight; see 158. | [I] dead in this build |
| 240 | 2 | `Message not sent.` | Diplomacy compose refusals (no county selected / not theirs / not ours / no enemy there / same alliance). | `Diplo_SendClicked` [D] |
| 241 | 2 | `Message not sent.` | Diplomacy compose refusals (no county selected / not theirs / not ours / no enemy there / same alliance). | `Diplo_SendClicked` [D] |
| 242 | 2 | `Message not sent.` | Diplomacy compose refusals (no county selected / not theirs / not ours / no enemy there / same alliance). | `Diplo_SendClicked` [D] |
| 243 | 2 | `Message not sent.` | Diplomacy compose refusals (no county selected / not theirs / not ours / no enemy there / same alliance). | `Diplo_SendClicked` [D] |
| 244 | 2 | `Message not sent.` | Diplomacy compose refusals (no county selected / not theirs / not ours / no enemy there / same alliance). | `Diplo_SendClicked` [D] |
| 245 | 2 | `A gift.` | Incoming gift of gold. | `FUN_00445960` [D] |
| 246 | 1 | `A communication` | Incoming free-text communication — also the DirectPlay chat path. | `FUN_00445960`, `FUN_004b79e3` [D] |
| 247 | 1 | `A communication` | Incoming communication, second layout. | `FUN_00445960` [D] |
| 248 | 2 | `Offer of alliance.` | Incoming alliance offer, with accept/decline. | `FUN_00445960` [D] |
| 249 | 2 | `A communication` | Alliance termination notice. | `FUN_00445960` [D] |
| 250 | 2 | `Plea for help` | Incoming plea for help in a county. | `FUN_00445960` [D] |
| 251 | 2 | `Launch an attack.` | Incoming request to attack a county. | `FUN_00445960` [D] |
| 252 | 2 | `Reply to request` | Reply: yes, I will help. | `FUN_00445960` [D] |
| 253 | 2 | `Reply to request` | Reply: no, I will not help. | `FUN_00445960` [D] |
| 254 | 2 | `Reply to request` | Reply: yes, I will attack. | `FUN_00445960` [D] |
| 255 | 2 | `Reply to request` | Reply: no, I will not attack. | `FUN_00445960` [D] |
| 256 | 2 | `Offer of alliance.` | Alliance accepted. | `FUN_004456c1` [D] |
| 257 | 2 | `Offer of alliance.` | Alliance declined. | `FUN_004456c1` [D] |
| 258 | 2 | `FREE.` | A player has left the multiplayer game; their lands are forfeit. | `Realm_Eliminate` [D] |
| 259 | 2 | `Com-link error.` | "Com-link error." — posted by `Sync_Rollback` after four consecutive divergences (§5.2). | `Sync_Rollback` [D] |
| 260 | 2 | `Cannot change display.` | Refusal: cannot switch to windowed mode at this desktop colour depth. | `Opt_ToggleFullScreen` [D] |
| 261 | 0 | — | Empty. | [I] empty |
| 262 | 0 | — | Empty. | [I] empty |
| 263 | 0 | — | Empty. | [I] empty |
| 264 | 0 | — | Empty. | [I] empty |
| 265 | 0 | — | Empty. | [I] empty |
| 266 | 0 | — | Empty. | [I] empty |
| 267 | 0 | — | Empty. | [I] empty |
| 268 | 0 | — | Empty. | [I] empty |
| 269 | 0 | — | Empty. | [I] empty |
| 270 | 2 | `Unpaid troops.` | Unpaid troops, first warning. | `Wages_PayAll` [D] |
| 271 | 2 | `Mutinous troops.` | Mutinous troops — a year unpaid. | `Wages_PayAll` [D] |
| 272 | 2 | `Mutiny!!!.` | Mutiny: the whole army deserts. | `Wages_PayAll` [D] |
| 273 | 2 | `FREE` | Spy report: *another* lord's troops are mutinying. | `Wages_PayAll` [D] |
| 274 | 2 | `Army too large.` | Refusal: the merged army would be too large. | `MoveOrder_ConfirmCombine` [D] |
| 275 | 2 | `Already besieged!.` | Refusal: that castle is already besieged. | `MoveOrder_ConfirmSiege` [D] |
| 276 | 2 | `FREE` | "The following news has just come in, my Lord." — a generic news wrapper with no caller. | [I] dead |
| 277 | 2 | `FREE` | "You cannot perform this action, my Lord." — a generic refusal with no caller. | [I] dead |
| 278 | 2 | `Unfed troops.` | Unfed troops. | `Army_Starve` [D] |
| 279 | 2 | `Starving troops.` | Starving troops. | `Army_Starve` [D] |
| 280 | 2 | `Army perishes.` | Army perishes of hunger. | `Army_Starve` [D] |
| 281 | 2 | `Cannot siege castle` | Refusal: no siege engines built. | `Siege_LaunchAssault` [D] |
| 282 | 2 | `Too many men.` | Refusal: too many men for the castle. | `Msg_Enqueue` <- `FUN_00437afb` [D] |
| 283 | 2 | `Castle fully barracked.` | Refusal: castle already at full garrison. | `MoveOrder_ConfirmGarrison` [D] |
| 284 | 2 | `Cannot siege castle.` | Refusal: castle under construction. | `MoveOrder_ConfirmSiege` [D] |
| 285 | 2 | `Cannot siege castle.` | Refusal: castle deserted, fight in the county instead. | `MoveOrder_ConfirmSiege` [D] |
| 286 | 2 | `Castle in county.` | A garrisoned castle must be besieged before the county falls. | `MoveOrder_Confirm` [D] |
| 287 | 2 | `Angry troops.` | Angry troops — pay still owed. | `Wages_PayAll` [D] |
| 288 | 2 | `Retreating army lost.` | A retreating army was lost (the < 50-men rule, C31). | `Battle_ReturnToCampaign` [D] |
| 289 | 2 | `Cannot garrison castle.` | Refusal: castle under siege, no reinforcement. | `MoveOrder_ConfirmGarrison` [D] |
| 290 | 2 | `Cannot alter castle.` | Refusal: no downgrading a castle. | `Msg_Enqueue` <- `FUN_00436b59` [D] |
| 291 | 2 | `Frequently asked questions.` | Help > How do I… — the FAQ index page, message category 0x13. | `Menu_HelpHowDoI` [D] |
| 292 | 6 | `How do I grow grain?` | Help > Grow grain? | `Menu_HelpGrowGrain` [D] |
| 293 | 6 | `How do I build a castle?` | Help > Build a castle? | `Menu_HelpBuildCastle` [D] |
| 294 | 6 | `How do I make weapons?` | Help > Make weapons? | `Menu_HelpMakeWeapons` [D] |
| 295 | 11 | `What should I do each turn?` | Help > What should I do each turn? | `Menu_HelpManageTurn` [D] |
| 296 | 2 | `Cannot find files.` | "Cannot find files. Please ensure your Lords 2 cd is in the cdrom drive." Has a window-geometry record in `g_helpWindowGeom` but no caller in this build (GOG ships no CD check). | [I] dead |
| 297 | 2 | `Low memory` | Low-memory warning at start-up. | `FUN_0041e962` [V] |
| 298 | 2 | `Non 256 colour display` | Non-256-colour desktop warning. | `Battle_Frame` [D] |
| 299 | 2 | `Low resolutions...` | Sub-640x480 desktop warning. | `Opt_ToggleFullScreen` [D] |
| 300 | 1 | `English - DO NOT TRANSLATE THIS!!!!` | The language tag, `"English - DO NOT TRANSLATE THIS!!!!"` — the localiser's marker. | `FUN_0041a166` [V] |
| 301 | 11 | `1268 AD` | The opening narration ("1268 AD", "The king is Dead"…). | `FUN_0041a166` [V] |
| 302 | 2 | `Healthy eating.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 303 | 2 | `Mother nature.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 304 | 2 | `Weapons found.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 305 | 2 | `Donation.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 306 | 2 | `Treasure` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 307 | 2 | `Holy Relic.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 308 | 2 | `Witch !!` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 309 | 2 | `Stone found.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 310 | 2 | `Pests.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 311 | 2 | `Hags curse.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 312 | 2 | `Fraud.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 313 | 2 | `Corruption.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 314 | 2 | `No bull.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 315 | 2 | `Stop thief!.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 316 | 2 | `Pests.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |
| 317 | 3 | `No songs.` | Random county event, `g_eventTable` deck; one of the sixteen ids `0x12E`–`0x13D` added for the Windows release. | `Event_RollAll` deck [D] |

---

## 6. Open questions

* **String character set.** Treated as Latin-1 here. The German and French
  builds ship their own `.eng`; none is available to check, so whether the
  engine is codepage-aware is unknown.
* **Which of `troops.eng` / `troops2.eng` / `troops3.eng` applies when.** The two
  selecting globals were not traced.
* ~~**`L2.eng` groups have no names.** A reimplementation has to hard-code the ids
  it needs; nothing in the file says what a group is for.~~ **Closed by §5** — all 317
  are mapped, 250 of them to the code that reaches them.
* **The 13 groups that differ between the DOS and Windows files** were not
  diffed in detail.
* ~~`mapl2.exe`'s group 41 is absent from every shipped `L2.eng`.~~
  **False** — present with 30 strings in the Windows release, empty only in DOS.
