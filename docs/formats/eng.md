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

The in-memory table is `short[35][5][2][11]` with a row stride of `0xDC` = 220
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

`TROOPS2.ENG` and `TROOPS3.ENG` reflect this: **only their `Normal` rows are
populated**, every other difficulty row is all zeros, and the files still parse
to exactly 3,885 numbers. `TROOPS.ENG` (the oldest, 30 Apr 1997) still has all
five groups filled in — a leftover from before the layout change its own header
announces.

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

## 5. Open questions

* **String character set.** Treated as Latin-1 here. The German and French
  builds ship their own `.eng`; none is available to check, so whether the
  engine is codepage-aware is unknown.
* **Which of `troops.eng` / `troops2.eng` / `troops3.eng` applies when.** The two
  selecting globals were not traced.
* **`L2.eng` groups have no names.** A reimplementation has to hard-code the ids
  it needs; nothing in the file says what a group is for.
* **The 13 groups that differ between the DOS and Windows files** were not
  diffed in detail.
* ~~`mapl2.exe`'s group 41 is absent from every shipped `L2.eng`.~~
  **False** — present with 30 strings in the Windows release, empty only in DOS.
