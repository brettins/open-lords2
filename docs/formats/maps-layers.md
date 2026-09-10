# `L2_maps.dat` — what the layers mean

Companion to [`maps.md`](maps.md), which establishes the **container**: a flat
array of 32,961-byte slots, `6 * (64*64) + (65*129)`, 80 slots in the Windows
release, 44 of them used.

**This document supersedes `maps.md` in four places**, and is the correct
reading in each: plane 3 is `dx + W*dy` rather than "a 3-tile and a 5-tile
object"; 31 maps support five players, not 36; bit `0x20` is farmland, not
dwelling; and the tile→lattice mapping is settled rather than open. An earlier
revision of this line claimed nothing here contradicts that document, which was
the most misleading sentence in the knowledge base — it invited a reader to
trust both everywhere. See `docs/audit.md`.

Status legend: **[V]** verified — an exact invariant over all 44 used maps, or
read directly out of the shipped binary / the running game's memory. **[I]**
inferred — consistent with everything measured but not proven.

Everything below reproduces with

```bash
node E:/dev/lords2/tools/maps/verify_layers.js          # 15 checks, all exact
node E:/dev/lords2/tools/maps/render_map.js 0 a         # -> tools/maps/out/ (gitignored)
node E:/dev/lords2/tools/maps/mapnames.js               # slot names from L2.eng
```

---

## 0. The headline

**Plane 1 selects one of five isometric tile sets, plane 2 is the frame number
inside it, and plane 3 is the tile's position inside a multi-tile object.** With
the tile→screen mapping in §4, the six planes plus the tail render the map
directly. Rendering slot 0 with the assignment below produces a recognisable
**England and Wales**; slot 52 produces a recognisable **Australia**, complete
with Tasmania. `L2.eng` group 101 independently names those slots "England" and
"Australia" (§6). That is the strongest single piece of evidence in this
document: a wrong bank assignment or a wrong lattice mapping produces noise, not
coastlines.

---

## 1. Plane 1 — the tile-set (bank) selector  **[V]**

### 1.1 The resource table in `Lords2.exe`

`Lords2.exe` holds a table of `{char name[16]; u32 size;}` records at virtual
address **`0x004DA050`** (file offset via the `.data` section; `tools/maps/pe.js`
does the VA→offset conversion, no Ghidra needed). Its first 32 entries are the
map tile sets, **eight layers × four seasons**:

| Entry | Name | Declared size | Entries 8/16/24 |
|---|---|---:|---|
| 0 | `base1a.pl8` | 128300 | `base1b/c/d` |
| 1 | `mtns1a.pl8` | 29000 | `mtns1b/c/d` |
| 2 | `roads1a.pl8` | 148000 | `roads1b/c/d` |
| 3 | `town1a.pl8` | 90000 | `town1b/c/d` |
| 4 | `castle1a.pl8` | 122000 | `castle1b/c/d` |
| 5 | `sprite1a.pl8` | 289000 | (same for all seasons) |
| 6 | `sprite1b.pl8` | 63000 | (same) |
| 7 | `flags1a.pl8` | 55004 | (same) |

Entries 32–63 repeat the same eight-layer pattern with the zoom-2 sets
(`base2a`, `mtns2a`, …). Entries 64+ move on to the battle-map sets
(`t32_bat1`, `a2w_psnt`, …), so the table is the game's whole resource
directory, not just the map's.

The renderer (`FUN_004063C1`, quoted in `maps.md`) switches on
`plane1 & 0x1c` and selects one of five sprite tables. **The five banks are the
first five layers of that table, in order:**

| plane 1 | tile set | frames | plane 2 range on disk | tiles (all 44 maps) |
|---|---|---:|---|---:|
| `0x00` | `Base1?.pl8` | 140 | 6 … 121, 108 distinct | 135,319 |
| `0x04` | `Mtns1?.pl8` | **25** | **0 … 24, all 25 — saturated** | 9,046 |
| `0x08` | `Roads1?.pl8` | 140 | 0 … 83, 79 distinct | 33,398 |
| `0x0c` | `Town1?.pl8` | 61 | 0, 1, 2, 3, 20, 30 | 2,461 |
| `0x10` | `Castle1?.pl8` | 100 | *never used on disk* | 0 |

Four independent arguments pin this down, and no other assignment survives all
four:

1. **Frame-count containment.** Every plane-2 value is a valid frame index of its
   bank's file — 180,224/180,224 tiles. `Mtns1a.pl8` has exactly 25 frames and
   bank `0x04` uses **exactly all 25 and no more**, which no other file can
   satisfy.
2. **The 5th bank needs ≥ 92 frames.** At run time the game rewrites settlement
   tiles to bank `0x10`, frames 88–91 (§5). Of the five files only
   `Castle1a.pl8` (100) and the two 140-frame files are big enough, and the
   140-frame files are already claimed by banks `0x00`/`0x08`. `Town1a.pl8` (61)
   cannot be bank `0x10`, so bank `0x0c` is `Town` and bank `0x10` is `Castle` —
   which is exactly the resource-table order.
3. **Multi-tile geometry** (§3) matches the frame layouts of `Mtns1a.pl8` and the
   2×2 groups of `Town1a.pl8`/`Castle1a.pl8` byte-for-byte.
4. **It renders.** See §0.

**[V]** The season letter is `a`/`b`/`c`/`d`, and the selector is **`g_season`**,
not the scenario index. `Gfx_LoadCountyMode` (`0x004984DC`) computes the base
resource entry as

```c
base = (g_mapZoom == 2) ? 0x20 : 0;
if (0 < g_season && g_season < 5) base += g_season * 8 - 8;
```

and then loads eight consecutive entries into the five tile banks plus
`sprite1a`, `sprite1b` and `flags1a` — which is this section's bank order,
confirmed from the loader rather than from the frame counts. An earlier revision
here, and `maps.md`, read the low 2 bits of `g_scenarioIndex` as the season
selector. That is wrong: those bits pick which of four map slots inside a
`MAPnn.PL8` the **minimap** comes from. See `docs/screens.md` §2.1 and §3.1.

#### 1.1a The four seasonal files of a bank share a frame table  **[V]**

**This is the measurement the whole seasonal swap rests on**, because the swap repoints eight
pointers and leaves every frame *index* alone. Anything the game has already rewritten — a
town's 47 … 50, an industry site, a crop — keeps its stored index across the season boundary,
so if frame 47 of `Town1c.pl8` were a different cell of the sheet than frame 47 of
`Town1a.pl8`, every county town on the map would turn back into a quarry in autumn.

Measured over all five near-zoom banks and all 466 frames of each season:

* the **frame count** is identical in all four files of every bank — 140, 25, 140, 61, 100;
* the **canvas anchor** `(X, Y)` at record offsets `0x08`/`0x0A` — where the artist put the
  cell on the sheet — is identical for **1,398 of 1,398** frames compared;
* `width`, `height` and `shape` are identical for every frame of every bank.

The **only** structural difference anywhere is the overhang-row byte at `0x0D`, on **nine**
`Roads1?.pl8` frames — 109, 111 and 113 … 119 — differing by one or two rows. Those are inside
the four-frame crop blocks based at 108/112/116/120 (§5.5): a crop that grows needs one more
scanline of picture above the tile in the season it is taller. That is artwork varying, not an
index moving.

So frame *n* of a bank is the same cell of the same sheet in every season. Asserted in
`crates/l2-view/tests/install.rs::the_four_seasons_of_a_bank_are_the_same_frame_table`.

#### 1.1b The **zoom-2** half of the table is not seasonal at all  **[V]**

`Base2b.pl8`, `Mtns2c.pl8` and the other eleven zoom-2 seasonal files ship in the install and
**the game never opens one.** Entries 32 … 63 are four *identical* blocks:

```text
32 base2a  33 mtns2a  34 roads2a  35 town2a  36 castle2a  37 sprite2a  38 sprite2b  39 flags2a
40 base2a  41 mtns2a  42 roads2a  43 town2a  44 castle2a  …          (season 2)
48 base2a  …                                                        (season 3)
56 base2a  …                                                        (season 4)
```

so `base + (season - 1) * 8` lands on the same five filenames whichever season it is. The far
view does not change with the year, and that is shipped behaviour rather than an omission.

**And the dead files are not interchangeable with the live one.** `Town2a.pl8` has **61**
frames; `Town2b/c/d.pl8` have **94**, and their frame records do not line up with it. A
renderer that derived the far zoom's filenames from the season letter — which is the obvious
thing to do, and what §1.1's *"the suffix is the season"* invites — would draw a different
sheet for three seasons in four. `Flags1b/c/d.pl8` are dead in the same way: entries 7, 15, 23
and 31 all name `flags1a.pl8`.

The lesson generalises: **the resource table is the authority on which file a bank loads, and
the filename is not.** `l2_view::campaign::Zoom::banks` is that table rather than a suffix
rule, and `the_far_zoom_names_one_season_four_times_and_the_unused_files_do_not_match` asserts
both halves.

#### 1.1c Which letter is which season, from the artwork  **[V]**

The loader's arithmetic puts season 1 on `a` and season 4 on `d`. The pixels agree
independently. Over the sixteen grass frames (6 … 21) of each `Base1?.pl8`, with "green"
meaning `g > r + 8 && g > b + 8`:

| file | green pixels | mean luminance |
|---|---:|---:|
| `Base1a` | 99.1 % | 113 |
| `Base1b` | 68.0 % | 108 |
| `Base1c` | **0.3 %** | 112 |
| `Base1d` | 37.9 % | **135** |

`c` has no green grass and no green woodland — autumn — and `d` is much the brightest, which
is snow. Spring, summer, autumn, winter, matching `l2_kingdom::tables::Season`'s
`Spring = 1 … Winter = 4`.

### 1.2 Plane 2 — the frame index  **[V]**

Plane 2 is the frame number within the bank's PL8, decoded per
[`pl8-mode2.md`](pl8-mode2.md). Nothing is scaled or offset. Worked examples from
slot 0 ("England"), read straight out of the file:

| tile (x,y) | p0 | p1 | p2 | p3 | p4 | p5 | resolves to | lattice cell |
|---|---|---|---:|---:|---:|---:|---|---|
| (19, 3) | `0x40` | `0x0c` | 0 | 0 | 0 | 14 | `Town1a.pl8` frame 0 | row 23, col 40 |
| (20, 3) | `0x40` | `0x0c` | 2 | 1 | 1 | 14 | `Town1a.pl8` frame 2 | row 24, col 40 |
| (19, 4) | `0x40` | `0x0c` | 1 | 2 | 3 | 14 | `Town1a.pl8` frame 1 | row 24, col 39 |
| (20, 4) | `0x40` | `0x0c` | 3 | 3 | 5 | 14 | `Town1a.pl8` frame 3 | row 25, col 40 |
| (30,20) | `0x08` | `0x08` | 30 | 0 | 0 | 12 | `Roads1a.pl8` frame 30 (woodland) | row 51, col 37 |
| (16, 4) | `0x20` | `0x08` | 80 | 0 | 0 | 14 | `Roads1a.pl8` frame 80 (fallow field) | row 21, col 38 |
| (24,11) | `0x00` | `0x00` | 10 | 0 | 0 | 11 | `Base1a.pl8` frame 10 (grass) | row 36, col 38 |

The first four are one castle: a 2×2 block of `Town1a` frames 0/2/1/3.

---

## 2. Plane 0 — the flag bits, fully classified  **[V] structure, mixed for meaning**

**[V]** The pair *(plane 0 value, plane 1 bank)* determines a **disjoint** range
of plane-2 indices. Every one of the 180,224 tiles falls into exactly one of
these 16 classes; no class overlaps another's index range:

| plane 0 | bank | tiles | plane 2 indices | reading |
|---|---|---:|---|---|
| `0x00` | base | 59,676 | 6–21 | plain grass (16 variants) **[V]** |
| `0x01` | roads | 12,468 | 0–29 | road, 30 pieces **[I]** |
| `0x02` | roads | 5,286 | 38–40 | county-boundary marker, 3 variants **[V]** |
| `0x03` | roads | 963 | 46–71, 73–75 | road **on a boundary tile**, 30 pieces **[I]** |
| `0x04` | base | 72,342 | 22–29, 38–121 | not in any county: open sea (22–29) + coast/cliff **[V]** |
| `0x08` | mtns | 9,046 | 0–24 | mountain **[V]** |
| `0x08` | roads | 7,630 | 30–37 | woodland, 8 variants **[I]** |
| `0x0a` | roads | 1,349 | 41–45 | woodland on a boundary tile **[I]** |
| `0x10` | base | 1,620 | 6–21 | reserved plot on grass **[I]** |
| `0x12` | roads | 116 | 38–40 | reserved plot on a boundary tile **[I]** |
| `0x20` | roads | 5,339 | **80 only** | farm field **[V]** |
| `0x22` | roads | 192 | 81–83 | farm field on a boundary tile **[I]** |
| `0x40` | town | 1,736 | 0–3 | **the county town**, 2×2 — and these frames are a placeholder, see below **[V]** |
| `0x80` | base | 1,681 | 6–21 | castle plot / resource site, 2×2 on grass **[V]** |
| `0x80` | town | 725 | 0, 20, 30 | resource site: 0 stone, 20 wood, 30 iron **[V]** |
| `0x82` | roads | 55 | 38–40 | settlement tile on a boundary **[V]** |

**Two of those labels were the wrong way round until `docs/decisions.md` C25 and C41.**
Bit `0x40` is the county town — `County_FindTownTile` (`0x00467FD1`) scans for it and
`Map_Click`'s `0x40` arm opens the village — and bit `0x80` is the castle and the four
resource sites; `County_FindCastleTile` (`0x00468121`) scans for `0x80` and stamps terrain
`0x14` over it. The `0x80`/town rows' frames 0, 20 and 30 are not "extra settlement tiles":
`County_PlaceResourceSites` (`0x00468E61`) reads exactly those three values as stone, wood
and iron.

**The `0x40` row's frames 0–3 are never drawn.** They are the quarry artwork — frame 0 is
the same stone quarry the line above identifies — and `Counties_PlaceSites` (`0x00468D4F`)
overwrites the block at load, and again every season, with 47–50, 51–54 or 55–58 by the
county's population. A renderer that draws the file puts four quarries where every town
belongs, which is what ours did; `docs/decisions.md` C41 is the whole of it.

The regularity is the argument: bit `0x02` never changes what a tile *is*, only
which graphic range it draws from. Every category has a "plain" range and a
"…with `0x02`" range.

### 2.1 Bit `0x02` is the county boundary — exact  **[V]**

> **All 7,961 tiles with bit `0x02` are 4-adjacent to a tile of a different,
> non-zero county. 7961/7961, zero exceptions, across all 44 maps.**

By comparison only 14.6% of land tiles are boundary-adjacent, so this is not an
accident of density. `0x02` marks roughly half the boundary tiles (7,961 of
15,757) — the fence/hedge is drawn on one side of each border only. Rendering the
bit as a mask over a map produces the county partition as a line network.

This corrects the natural guess that `0x02` is a river; it is not.

### 2.2 Bit `0x04` — no county  **[V]**

Re-confirmed here: `(plane0 & 0x04) != 0` ⟺ `plane5 == 0`, 180,224/180,224.
All such tiles are bank `base`, indices 22–29 (the eight open-water frames) or
38–121 (coast, cliff and shore frames).

### 2.3 Bit `0x10` — exactly four per county  **[V] count, [I] meaning**

> **Every county in every map has exactly 4 tiles with bit `0x10` set.
> 1,736 tiles total; 0 of 44 maps disagree.**

They are scattered inside the county, not adjacent, and always sit on plain grass
(`0x10`) or on a boundary tile (`0x12`). At load the game **clears the bit** and
copies the tile's graphic index into a spare byte of the runtime tile record
(§5), i.e. it remembers the terrain so it can restore it. **[I]** These are
reserved plots where something gets built later.

### 2.4 Bits `0x01`, `0x08`, `0x20`  **[I]**

`0x01` (roads frames 0–29) draws as brown dirt tracks; only 12.1% of its tiles
are county-boundary-adjacent, i.e. it is *not* correlated with borders — it is
the road network. `0x08` covers all mountain tiles plus `Roads` frames 30–37,
which draw as dense woodland: rough/impassable terrain. `0x20` is always stored
as `Roads` frame 80 (a fallow ploughed field) and at run time is rewritten to
frames 80–83, 84–87 or 104–107, which are three different crop states — so
`maps.md`'s reading of `0x20` as "dwelling / housing site" should be read as
**farmland**; `FUN_00467a36`'s three "building sizes" `0x50`/`0x54`/`0x68` are
frames 80/84/104, the three field appearances.

---

## 3. Plane 3 — the part index inside a multi-tile object  **[V]**

`maps.md` inferred "a 3-tile object and a 5-tile object" from the equal-count
histograms. That reading is wrong. The objects are **rectangular blocks of
tiles** and plane 3 is the part's offset inside the block:

> **plane 3 = `dx + W * dy`**, where `(dx, dy)` is the tile's offset from the
> block's north-west tile and `W` is the block width in tiles.

A 2×2 block therefore uses `0,1,2,3` (three non-zero values in equal counts — the
"3-tile object") and a 3×3 block uses `0 … 8` (five extra values 4–8 in equal
counts — the "5-tile object").

**The PL8 frames of a multi-tile object are stored in isometric screen order**,
i.e. sorted by `dx + dy` and then by `dx`. This is visible directly in the frame
records' canvas-placement fields (`X` at 0x08, `Y` at 0x0A): the artist drew each
object assembled. `Mtns1a.pl8` frames 0–3:

```
frame 0  X= 73 Y= 80  shape 2      (dx,dy) = (0,0)      plane3 = 0
frame 1  X= 43 Y= 95  shape 3      (dx,dy) = (0,1)      plane3 = 2
frame 2  X=103 Y= 95  shape 4      (dx,dy) = (1,0)      plane3 = 1
frame 3  X= 73 Y=110  shape 1      (dx,dy) = (1,1)      plane3 = 3
```

X steps by 30 and Y by 15 — half a tile in each axis, the isometric diamond.
Frames 4–7, 8–11 and 12–15 are three more 2×2 mountains laid out identically, and
frames **16–24 are a single 3×3 mountain massif** (rows of 1, 2, 3, 2, 1 frames on
the art sheet — nine tiles). `Castle1a.pl8` is 25 castles × 4 quadrants in the
same arrangement; `Town1a.pl8` frames 47–50, 51–54 and 55–58 are three more 2×2
groups.

Exact checks over all 44 maps:

| Claim | Result |
|---|---|
| `plane3 == dx + W*dy` for every `Mtns` 2×2/3×3 tile and every `Town` castle-site tile | **10,971 / 10,971** |
| every `0x40` castle is a complete 2×2 of `Town` frames 0/2/1/3 with plane3 0/1/2/3 | **434 / 434** |
| settlement parts 1, 2, 3 each occur exactly once per county | **434 / 434 / 434** |
| every settlement anchor completes a 2×2 with plane3 0/1/2/3 | **434 / 434** |

434 is the total county count over the 44 maps, so **every county has exactly one
2×2 castle site and exactly one 2×2 settlement block**, plus 725 loose extra
settlement tiles (0–2 per county) drawn from `Town` frames 0, 20 and 30.

One wrinkle worth knowing: a member of a settlement block that happens to sit on
a county boundary switches to bank `roads`, frames 38/39 (the boundary variant),
so a decoder must not assume all four members share a bank. 17 of the 434 blocks
are like that.

Plane 3 is **not needed to draw a tile** — each tile already carries its own
bank+frame. Its job is object identity: it says which tile is the anchor and how
the others hang off it. Consistently, the loader copies plane 3 verbatim into the
runtime tile record (byte +4), 4096/4096 for the map that was checked live.

---

## 4. The 65 × 129 layer — the tile-to-lattice mapping, settled  **[V]**

> **tile (x, y)  →  lattice row = `x + y + 1`, col = `(x - y + 64) >> 1`**

Verified **4096 / 4096 exactly**, and with **zero** cells misclassified as
covered/uncovered, against the game's own runtime lattice read out of a live
`Lords2.exe` at `0x0055CEA0` while slot 0 was loaded. In that dump exactly 4,096
of the 8,385 cells held a tile-array byte offset — values `0x0000 … 0x7FF8` in
steps of 8, i.e. `tileIndex * 8` — and the remaining 4,289 kept their
`0x0FFF0000 + b` background form. The mapping is a pure screen-geometry formula
with no dependence on map content, so it holds for every slot.

Why the earlier brute-force affine search stalled at 72%: the column term needs
coefficients of **±½**, which an integer-coefficient search cannot express. The
row term it did find, `row = x + y`, was right all along — it was one short of
`x + y + 1` and the column was unreachable.

Geometry, at zoom 0 (58 × 30 tiles):

```
tile width 58, height 30, half-step 29 x 15
lattice cell (row, col) is drawn at screen  x = col*58 + (row odd ? 0 : 29) - 29
                                            y = row*15
covered rows 1..127 of 0..128, covered cols 0..63 of 0..64
lattice bounding box 65*58 x 129*15 = 3770 x 1935 px, exactly the map diamond
  (64+64)*29 x (64+64)*15 = 3712 x 1920 plus one tile of margin
```

### 4.1 What the two byte values are  **[V]**

The tail's alphabet is `{0x06, 0x16}` — and those are **`Base` tile-set frame
numbers**: frame 6 is grass, frame 22 (`0x16`) is open water. The uncovered
cells are the off-map surround, and the byte says whether the land runs off the
edge (grass) or the sea does (water).

Confirmed from the live process: the game **randomises the variant** when it
loads them. Where the file says `0x06` the runtime background cells held values
spread evenly over **6…21** — the 16 grass frames; where the file says `0x16`
they held **22…29** — the 8 water frames. It does the same to the map proper: of
slot 0's `0x04` (no-county) tiles, 1,967 had their stored index permuted within
22–29 at load. That is why a naive file-vs-runtime comparison of plane 2 only
matches 46% and is not a decoding error.

### 4.2 The tail is authored, not derived  **[V] — a negative result**

There is **no** function from the tile grid to the tail, and no point looking for
one. The tail describes the surround, which the map author drew by hand:

* Slots 16, 18 and 44 contain **zero** `0x06` cells — islands with nothing but
  sea around them.
* Slot 23 has exactly 136 `0x06` cells, the same count as the blank template.
* The **unused** slots' tails are not blank: rendered as a 65 × 129 grid they
  contain small hand-drawn ornaments (an arrow, a ship-like glyph, letter-shaped
  marks) in the two-value alphabet — leftover doodles in the "empty map"
  template. Two different templates, one per release, as `maps.md` notes.
* Under the correct mapping, the file's tail bytes over the **covered** cells
  are 1,822 `0x06` / 2,274 `0x16` for slot 0 — meaningless data that the runtime
  overwrites, which is exactly why fitting the land mask to it topped out in the
  70s.

---

## 5. Plane 4  **[V] structure, [I] the six lists**

> **Plane 4 is non-zero only on tiles with plane 0 bit `0x40` or `0x80`.**
> Zero exceptions over 180,224 tiles.

### 5.1 On settlement tiles — the player start table  **[V]**

Counts over all 44 maps: value 1 → 44 tiles, 2 → 44, 3 → 36, 4 → 36, 5 → 31.
Per map the multiset is:

| multiset | maps |
|---|---:|
| `1,2,3,4,5` | 31 |
| `1,2,3,4` | 5 |
| `1,2` | 8 |

so **31 maps support 5 players, 5 support 4, and 8 support 2** — every map has
1 and 2, and the values are contiguous from 1. (`maps.md` says 36 maps carry
five; the exact figure is 31.) Each value sits on the anchor tile of a different
county's settlement block, so it names that county as start `n`.

Confirmed live: on the England map the 5 counties carrying values 1–5 were
exactly the 5 whose settlement blocks the game upgraded at load — bank rewritten
from `base` to `0x10` (`Castle1a.pl8`) and frames rewritten to **88, 90, 89, 91**
for plane3 0, 1, 2, 3. The other settlements were left as stored. The plane-4
byte is then zeroed in the runtime record, i.e. consumed.

### 5.2 On castle tiles — the six county lists  **unresolved**

Counts over all 44 maps: `{1: 198, 2: 196, 3: 169, 4: 174, 5: 171, 6: 125}`.
Two exact quadrant invariants, both 434/434:

* the **top** quadrant (plane3 = 0) **always** has plane 4 = 0;
* the **right** quadrant (plane3 = 1) **always** has plane 4 ≠ 0.

So the right quadrant alone assigns every county to exactly one of six groups —
a partition — and the left and bottom quadrants add the county to further groups
(left is zero on 227 of 434, bottom on 42 of 434).

`FUN_00429153` turns this into six 16-entry lists of county ids. Their sizes vary
wildly between maps (`5,7,9,8,7,6` for England; `4,0,0,0,0,0` for "Equalizer";
`8,8,8,8,8,8` for "China") and do **not** correlate with the number of players
the map supports, the county count, or the start counties. **I could not
determine what the six lists mean.** Settling it needs the *consumer* of the
6 × 16 table traced in Ghidra — `FUN_00429153` only fills it.

### 5.3 The runtime tile record  **[V], one live run**

Read from `0x00522F90` (4096 × 8 bytes) with slot 0 loaded, and matched against
the file. The record is typed as `Tile` in [`docs/records.json`](../records.json)
and applied before every corpus rebuild, so the field names below are the ones
the decompiled corpus uses:

| byte | field | contents | agreement with file |
|---|---|---|---|
| +0 | `content` | what stands on the tile — see §5.4 | runtime-only |
| +1 | `flags` | plane 0 flags | 4026/4096 (see below) |
| +2 | `bank` | `plane1` in bits `0x1c`; bits `0x01`, `0x20`, `0x40`, `0x80` set at run time | 4062/4096 |
| +3 | `frame` | plane 2 frame index | 1872/4096 (variant randomisation, §4.1) |
| +4 | `part` | **plane 3, verbatim** (read as `& 0xF`) | **4096/4096** |
| +5 | `unit` | **plane 4**, then consumed and reused as the unit-occupancy plane | 4043/4096 |
| +6 | `savedFrame` | saved terrain frame index for tiles that get a building | runtime-only |
| +7 | `county` | **plane 5 county, verbatim** | **4096/4096** |

`RecordProbe` finds all 441 dereferences of this array in the binary are **one
byte wide**, at all eight offsets — so the eight-plane reading has no exception
anywhere in the code, not just in the one live dump.

**Bit `0x40` of `bank` was missing from this table.** It is a run-time draw bit
like `0x80`, and `Map_RenderIso`'s two half-row loops make its meaning exact:
`bank & 0x80` calls the building-overlay blitter and `bank & 0x40` calls
`Map_DrawPathMarker`. **[V]** — the path-preview ball, not a flag: the name this line
carried was corrected in `docs/decisions.md` C49.

### 5.4 Byte +0, `content` — the tile's occupant  **[V]**

Not one enumeration. The value is read under a mode chosen by the `flags` byte,
which is why the ranges overlap:

| on a tile with | `content` means |
|---|---|
| `flags & 0x80` (industry site or castle) | `1/2/3` iron, `4/5/6` stone, `7/8/9` weapons, `10/11/12` wood, as **idle / working / wrecked** (`Unit_TrampleTile` steps each triple to its wrecked value and disables that county's industry); `0x14` the castle plot, `0x15…0x19` a castle |
| `flags & 0x10` (dwelling plot) | `0x10…0x13`, largest to smallest — see §8 |
| `flags & 0x20` (farm field) | `0` wild, `1` fallow, `2…14` grain, `15…22` pasture, `23/24` neither, `25+` being reclaimed (`County_RecountFields`) |

`Unit_Step` is where the discrimination is visible: `Unit_TryEnterTile` returns
one code per flag bit, and only the `0x80` arm (code 6) then splits on
`content < 0x10` for an industry site versus `0x14 < content < 0x1A` for a
castle.

#### The castle rung, read from its writer  **[V]**

`0x14 + castleType`, so `0x14` is the bare plot and `0x15 … 0x19` are the five castle
types — and it is worth saying where that comes from, because it used to be
`docs/hypotheses.json` H6, which rested on two counties of one save. It is now
**`Castle_StampTile` (`0x0046826C`)**, an `if`/`else if` ladder over the five levels writing
`0x15, 0x16, 0x17, 0x18, 0x19` and nothing else, matched by `Unit_Step`'s
`0x14 < content < 0x1A` at the reading end. The two saved counties agree with it and are no
longer what it rests on.

The same function is the reason **a castle is not in `L2_maps.dat` at all**. Unlike the
mine, the quarry and the forest — which the file stores as real artwork that
`County_PlaceResourceSites` merely flags — the castle plot is plain ground in the base bank,
and every castle on the original's campaign map is stamped in at run time:

```c
if (castleDegraded == 0)   frame = level*4 + 0x50;   /* finished */
else if (percent < 0x32)   frame = level*4 + 0x28;   /* scaffolding, under half done */
else                       frame = level*4 + 0x3C;   /* half built or more */
Map_StampBlock(frame, 2, county.castleTile, 0x10, 0x15 + level);
```

Three appearances a level, twenty frames apart, re-stamped **every season** by
`Castle_BuildTick` — so a castle visibly goes up. Bank bits `0x10` are bank index
`(0x10 & 0x1C) >> 2 = 4`, `Castle1a.pl8` / `Castle2a.pl8`; the `a` is the season and the
whole bank is swapped by `Gfx_LoadCountyMode`, so the frame numbers are season-independent.
`Map_StampBlock`'s 2×2 quadrant offsets are `[0, 2, 1, 3]`, read out of `Lords2.exe` at
`0x004D80E0` — the same table the town's rewrite above already uses.

The 70 plane-0 differences are all load-time edits: `0x10 → 0x00` on 55 tiles
(the reserved plots, whose terrain index was copied to +6), `0x12 → 0x02` on 1,
and `0x00 → 0x80` on 14 — one per county — which simultaneously moved to bank
`0x0c` frame 10. **County towns** were rewritten from `Town` frames 0/1/2/3 to
47/48/49/50, which is exactly the 2×2 group `Town1a.pl8` frames 47–50.

An earlier revision of this paragraph called those tiles "castle sites" and explained the
rewrite as *"the game was started with Starting Castle: keep"*, guessing that 51–54 and
55–58 were "the other castle options". **Both halves are wrong** and `docs/decisions.md`
C41 has the code: the tiles are towns, and the three groups are the three **village sizes**,
chosen by the county's population (`< 801` → 47, `< 1201` → 51, otherwise 55) and
re-stamped every season by the population pass. The observation was right; the story
attached to it was invented, which is C21's shape in a caption. Six tiles whose
plane 0 was exactly `0x01` received the values 1…6 in byte +5 — six numbered
road tiles in six different counties, purpose unknown.

**Caveat.** These runtime numbers come from **one** run of **one** map (slot 0,
custom game, England, winter, 5 players, starting castle "keep"). They are direct
observations of the shipped binary's own state and I would not expect them to
change, but they are not the 44-map invariants the rest of this document is built
on. The dumps are at `tools/maps/out/{tiles,lattice}.bin` (gitignored);
`tools/maps/dump.ps1` regenerates them — see §7 for why that is harder than it
sounds.

### 5.5 The graphic for a terrain — `Terrain_Set` (`0x0046D7F4`)  **[V]**

> **It has a name now.** Four documents referred to this function only as
> `FUN_0046D7F4`; it is `Terrain_Set` in `docs/symbols.json`, and
> `l2_view::campaign::field_graphic` is the reimplementation.

**The single writer of a tile's `content` byte, and it picks the tile's picture at
the same time.** Every state change on the map goes through it — the field brush
(`0x00434...`), the seasonal crop pass, `County_DestroyField`, `Unit_TrampleTile`,
`Counties_PlaceSites` and `Map_PlaceStartingFields` all call it — so this one
function *is* the terrain → frame mapping the renderer needs, and nothing has to be
inferred from what the tiles look like.

```c
void FUN_0046d7f4(int tileByteOffset, int terrain, char variant)
```

It sets `content = terrain`, then

```c
frame = ((frame - oldBase) & 3) + base + variant * 4;
bank  = (((bank | 1) & 0xE3) | layer) & 0x7F;
if (0x0E < terrain && terrain < 0x17) bank |= 0x80;
```

where `oldBase` is `0x82` if the *previous* terrain was `0x17` or `0x18` and 0
otherwise, and `base` and `layer` come from a ladder on the new terrain:

| terrain | base | bank layer |
|---|---|---|
| `0` wild | 80 | `0x08` roads |
| `1` fallow | 84 | `0x08` |
| `2 … 0x12` (grain, and pasture up to 18) | 88 | `0x08` |
| `0x13 … 0x16` | 104 | `0x08` |
| `0x17` | 130 | `0x00` base |
| `0x18` | 134 | `0x00` base |
| `0x19` | 108 | `0x08` |
| `0x1A` | 112 | `0x08` |
| `0x1B` | 116 | `0x08` |
| `0x1C` | 120 | `0x08` |

**Two corrections to this table, both from re-reading the function.**

* **The last row is a bare `else`, not a range.** The `104` arm catches every terrain at
  `0x13` or above that is not one of `0x17 … 0x1C` — so `0x1D` and up land there too, and
  those are the values `County_RecountFields` buckets as *being reclaimed*. The table above
  reads as if `0x13 … 0x16` were exhaustive; it is not wrong about those four and it was
  silent about the rest. **[V]**
* ~~**The third parameter is dead.**~~ **FALSE, and it is the wheat.** This bullet read:
  *"`variant * 4` shifts the frame by a whole four-frame block, and all sixteen call sites in
  the shipped binary pass zero — including the two that forward a parameter (`FUN_00469D21`,
  whose only callers are `Grain_SeasonTick` and `Herd_UpdateCrowding`, and both pass
  `'\0'`). So the term contributes nothing to any picture the game draws, and the frame is
  exactly `base + (storedFrame & 3)`."* It was marked **[V]**.

  A player: *"The wheat fields don't show the wheat growing."* He is describing this bullet.

  There are **twenty-four** call sites, not sixteen. Twenty-three pass a literal `'\0'`. The
  twenty-fourth is `FUN_00469D21`'s forward, and of *its* two callers `Herd_UpdateCrowding`
  passes `'\0'` and **`Grain_SeasonTick` does not**:

  ```c
  band    = FUN_0044CF6F(county.crop[2], county.fieldsGrain);   /* 2, 3, 7 or 11 */
  variant = band < 3 ? 0 : (band - 3) / 4 + 1;                  /* 0, 1, 2 or 3  */
  FUN_00469D21(county, band, variant, 2, 0xE);   /* every grain tile of the county */
  ```

  `FUN_0044CF6F` is four sacks-per-field bands, `< 1` → 2, `< 0x29` → 3, `< 0x51` → 7, else
  11. **All four bands fall in `2 … 0x12`, so all four share base 88** — the `content` byte
  carries no information about the crop's stage at all, and the *variant* carries every bit
  of it. A renderer that drops the term draws a just-sown field in every season, which is
  exactly what ours did.

  Measured against the shipped `Roads1a.pl8`: frames 88 … 103 are sixteen 58-wide diamonds,
  four variants of four variations, and the ripe-gold pixel count rises strictly with the
  variant at every one of the four positions — 36/34/36/33, 45/43/46/43, 54/50/55/49,
  71/70/72/69 — while fallow (84 … 87) and pasture (104 … 107) carry 2 … 11. Four blocks of
  four from 88 end at 103 and 104 is where the next base begins. **[V]**
  `crates/l2-view/tests/install.rs::the_four_wheat_variants_ripen_and_the_block_ends_where_the_next_base_begins`.

  **How the claim came to be `[V]`, because that is the transferable part.** It was checked
  on *one* of the two functions that forward the parameter and stated about both. That is
  `docs/agents.md`'s *"a true statement about one branch, promoted to a statement about the
  subsystem"*, and the defence there is the same: **name the branch.** *"`Herd_UpdateCrowding`
  passes zero"* is the finding, and it cannot be promoted by accident.
  `docs/decisions.md` C124.

**A pasture is drawn twice, and §5.5a is the second time.** Everything below is exact for
the diamond and silent about the animals on top of it.

**Why `& 3` is enough, stated as the invariant it is.** Every base above is a multiple of
four *except* 130 and 134, and `oldBase` exists for precisely those two. So the low two bits
of a farm tile's frame **never change for the life of the game**, whatever happens to the
crop — which is what makes the picture a pure function of `(terrain, the frame the map file
stored)` and lets a renderer recompute it without tracking the tile's history.

**`& 3` is the point.** Every crop state is **four consecutive frames** and the tile
keeps whichever of the four it already had, so a repaint changes the crop without
changing the tile's variation. §1.2's own histogram is the check: farm tiles on disk
are bank `0x20`/roads **frame 80 only** and their boundary twins are 81 … 83 —
which is `base = 80`, the four variants of *wild*, exactly.

`Map_PlaceStartingFields` (`0x00467A36`) is the same three bases seen from the other
side: it writes `frame = base + ((frame + 0xB0) & 3)` with `base` 104 for pasture
(`content 0x14`), 84 for fallow (`content 1`) and 80 for wild (`content 0`), the mix
chosen by difficulty. Two functions, one table.

**Drawn.** `MapScreen::add_field_graphics` puts every farm tile's picture into the sparse
`campaign::Overrides` plane C41 added, from `l2_view::campaign::field_graphic`, which is this
function. Until it existed the field brush painted markers of our own and every field on the
map looked like the bare frame 80 the file stores. Tests:
`l2-game/tests/screens.rs::a_fields_picture_follows_its_crop_state` walks the ladder and
requires four crop states to be four different pictures at the tile.

What is still open is whether the *season* moves a field's `content` on its own — that is the
economy's business, not this file's.

### 5.5a The cattle on a pasture — `Sprite_TopIt`'s farm arm (`0x004071A0`)  **[V]**
> **It has a name now, and the name is the game's own — do not change it.** `docs/symbols.json`
> calls `0x004071A0` **`Sprite_TopIt`**. An earlier revision of this paragraph proposed
> renaming it `Map_DrawTileOverlay`, on the grounds that "Sprite_TopIt" reads as one blitter
> while the function is a dispatcher. **That was wrong about where the name came from.**
> `node tools/oracle/logstrings.js` prints this function's three `Log_Write` literals:
> `"ERR:top_it no data "` and `"ERR:top_it bad data "` twice. `top_it` is the *original
> authors'* name for it, in the same convention that gave us `write_sprite`, `gen_frame`,
> `mos_frame` and `mos_blank` — `docs/decisions.md` C72. Replacing a name recovered from the
> binary with a better-describing invention is the one trade this project does not make.
>
> The docstring is what needed fixing, and `docs/draws-map.md` §3 is it: the dispatcher has
> **six** arms, not four — the town's flag, the town's mercenary marker, a *razed* dwelling,
> the pasture herd, the four industry sites (which animate the terrain tile itself rather
> than blitting an overlay) and the castle's garrison flag — behind a fog-of-war gate and an
> early `(flags & 0xF0) == 0` return.


§5.5 says the picture is *"a pure function of `(terrain, the frame the map file stored)`"*.
**For a pasture that is only half of it.** The terrain byte fixes the ground; the animals come
from a second sheet in a second pass, and §5.5 was silent about them because the second pass
was not read.

**Bit `0x80` is what joins the two.** `Terrain_Set`'s last line is
`if (0x0E < terrain && terrain < 0x17) bank |= 0x80;` — and `0x0F … 0x16` is *exactly* the
pasture range of §5.4's table, so **a pasture is the only field state that gets a second blit
at all**. §5.3 already recorded that `bank & 0x80` calls "the building-overlay blitter"; that
blitter is `FUN_004071A0` and it has **four** arms, chosen by plane 0, not one:

| plane-0 bit tested | arm |
|---|---|
| `0x40` | the county town: the owner's flag, and the mercenary marker on quadrant 2 |
| `0x10` | a dwelling |
| `0x20` | **farmland — the animals** |
| else (`0x80`) | an industry site's animation, or a castle's garrison flag |

The farm arm in full:

```c
if (g_mapZoom == 2)   return;             /* no animals at the far zoom */
if (content < 0x0F)   return;
if (0x16 < content)   return;
if (content < 0x13) {                     /* the dead half, below      */
    dx = 0; dy = 0;
    if (2 < (byte)(content - 0x10)) return;
    frame = (content - 0x10) * 6 + phase + 0x67;
} else {
    dx = 4; dy = -4;
    if (2 < (byte)(content - 0x14)) return;
    frame = (content - 0x14) * 6 + phase + 0x55;
}
if (edge == 1) dx -= g_mapTileHalfStep;   /* the left half-tile of an offset row */
```

then it blits `g_flagsSheet` frame `frame` at `(g_drawX + dx, g_drawY + dy)` — the same
`Flags1a.pl8` the flags and the path markers come out of, and the frame record's `cx`/`cy`
are never read, exactly as §5.3's flag note says.

**`content == 0x0F` and `content == 0x13` fall through the unsigned compare and draw
nothing.** `0x13 - 0x14` is `0xFF` as a byte. `0x13` is the value an **empty herd** gets, so
a county that has lost every animal keeps its pasture and shows bare grass.

**The frame is the herd count.** `Herd_UpdateCrowding` (`0x0044D913`) writes the terrain, on
every pasture tile of the county at once through `FUN_00469D21(county, t, 0, 0x13, 0x16)`:

| condition | `content` | `Flags1a.pl8` frames |
|---|---|---|
| `herd < 1` | `0x13` | **none** |
| `herd / fieldsCattle < 11` | `0x14` | `0x55 … 0x5A` |
| `… < 21` | `0x15` | `0x5B … 0x60` |
| otherwise | `0x16` | `0x61 … 0x66` |

So this is a **rule**, not a graphic, and it is not the same ladder as the crowding *meter*,
which has four bands (11/21/31) where this has three: map states `0x15` and `0x16` cannot
tell *"Herd overcrowded."* from *"Massive overcrowding!!"*. `docs/decisions.md` C77.

**The sheet, measured.** `Flags1a.pl8` frames `0x55 … 0x66` are eighteen frames of
**58 × 30** — the near-zoom diamond exactly, so the sprite is a full-meadow overlay — with
opaque pixel counts of 344 / 706 / 930 by band. Frames `0x4F … 0x54` and `0x67 … 0x78` are
**2 × 2 stubs**.

**The `0x67` half is vestigial reclamation, not sheep.** The tile-info table at `0x004D2EC8`
gives `content 0x0F … 0x12` `L2.eng` group 30 descriptions 40 … 43 and mode 20 — *the same
four strings* it gives `0x19 … 0x1C`, the live reclamation ladder. And nothing writes
`0x0F … 0x12` onto a farm tile: every one of `Terrain_Set`'s sixteen call sites was
enumerated and they pass `0`, `1`, `2 … 0x0E`, `0x13 … 0x16`, `0x17`, `0x18`, `0x19 … 0x1C`
and the brush's `{0, 1, 2, 0x13, 0x19}`. `County_UpdateDwellings` does write those four, but
onto a tile carrying plane-0 bit `0x10`, which `FUN_004071A0` tests *before* `0x20`.

**One correction to §5.4's parenthetical.** Its table calls `15 … 22` pasture, which is
`County_RecountFields`' own bucketing and correct. But the binary holds **two disagreeing
enumerations** for `0x0F … 0x12`: the recount counts them as pasture, the tile-info table
calls them reclamation, and the AI's brush (`FUN_004697CD`, `FUN_0046988D`) tests pasture as
`0x12 < t < 0x17` and cannot see them. Only `0x13 … 0x16` is live pasture.

**Drawn.** `MapScreen::draw_herds` and `l2_view::campaign::draw_herd`, in the same pass as
the flags. The phase is `_DAT_0057D38C` — `DAT_0057D388` wrapped at **`0x60`**, not a power
of two, shifted right by four, so six phases — and `FUN_004CFB08` steps that counter behind a
**16 ms `GetTickCount`** gate. **It is not `Tick_Pulses`**: the campaign map has its own
clock and the village's 20 ms divider chain is a different rate. Tests:
`l2-view/tests/install.rs::the_pasture_herd_frames_are_full_tiles_and_grow_with_the_crowding`
against a real `Flags1a.pl8`, and
`l2-kingdom/tests/fields.rs::every_county_s_pasture_carries_the_picture_its_herd_calls_for`
against the England save's own 107 pasture tiles.

### 5.6 Twenty fields per county is a property of the map, not a cap  **[V]**

`County_CollectFieldTiles` (`0x0046DA4B`) is the last thing `Map_InitScenario` does, and it
fills `g_countyFieldTiles` — 17 rows of twenty `i32` byte offsets — by sweeping the tile array
in ascending order and dropping each farmland tile (`0x20`) into its county's first free slot.

**The twentieth slot is a wall, and what will not fit is destroyed.** The `else` of *"is there
a free slot"* writes `content = 0`, `frame = 6`, **`flags = 0`** and clears the bank back to
base: the tile stops being farmland at all, before the first season runs. So a county cannot
have twenty-one fields in the way a county cannot have five dwelling plots (§8) — except that
this one has a defined outcome instead of a corrupted neighbour.

No shipped map reaches the branch: the largest county on the 44 maps has twenty farm tiles or
fewer, which is why it has never been visible. It is exercised on a synthetic slot in
`crates/l2-scenario/tests/newgame.rs`, and it is one of only **two** places where the runtime
flags plane differs from the file's — the other being `County_PlaceBlacksmith`, which *adds*
bit `0x80` to a tile the file gave no flags at all, one per county. Over England, that pair is
the whole of the difference: 14 blacksmiths added and nothing razed. See `docs/decisions.md`
C62, which also records that county 4 of slot 8 gets no blacksmith because it has no `flags == 0`
tile for one to stand on.

---

## 6. Slot names — `L2.eng` group 101  **[V]**

`L2.eng` group 101 holds **60** strings that line up one-for-one with slots
0…59 (`tools/maps/mapnames.js` prints them beside the census):

| Slots | | |
|---|---|---|
| 0–11 | England, Scotland, Ireland, France, Germany, Italy, Europe, Crusades, Africa, India, China, Jigsaw | used |
| 12–23 | Rose, Bullseye, Crossroads, Rorschach, Centreville, Quaintville, Pretzelland, N. England, Equalizer, RubixWorld, Pentagon, YinYang | used |
| 24–39 | `"map no 25"` … `"map no 40"` | empty |
| 40–51 | Britain, Imperium, The World, Japan, U.S.A., S. America, Butterfly, Deadlock, Torus, Vortex, Patches, The Snake | used |
| 52–59 | Australia, Central Am., Waterworld, The Hub, Roundabout, Knotsville, Cubium, Snowflake | used |

The 60-vs-80 gap: the DOS release had 40 slots and 40 names, of which 24 were
real and 16 were `"map no 25"…"map no 40"` placeholders. The Windows release
doubled the **file** to 80 slots but only extended the **name list** to 60, and
filled slots 40–59 with the 20 new maps. **[I] Slots 60–79 are spare capacity in
the container with no UI entry at all** — they are byte-identical blank templates
and there is nothing to name them with.

This is a genuinely independent corroboration of the census: two unrelated files
agree on used = 0–23 and 40–59, empty = 24–39.

**And a third, from a completely different direction.** `Minimap_Load` reads a
slot's minimap out of `"map01.pl8" + (slot >> 2) * 0x10` — four map slots per
file — and the install ships **11** of those 15 names. 11 × 4 = **44**, exactly
the used-slot count, and the four names it does not ship, `map07…map10`, cover
exactly slots 24…39. `crates/l2-view/tests/install.rs` asserts both halves over
all 60 slots. See `docs/screens.md` §3.1.

As a smoke test, the names match the geometry: "Australia" (slot 52) renders as
Australia with Tasmania; "England" (slot 0) renders as England and Wales;
"Ireland" (slot 2) and "Japan" (slot 43) are islands; the abstract names
("Pentagon", "YinYang", "Torus") belong to the small 4–7-county maps.

---

## 7. Reproducing, and one approach that does not work

### Scripts (all in `tools/maps/`, all read-only against the install)

| Script | What it does |
|---|---|
| `verify_layers.js` | the 15 exact checks quoted above; also runs on the DOS `L2_MAPS.DAT` (skips the tile-set checks, which need the PL8s) |
| `pe.js` | PE VA→file-offset helper; how the `0x004DA050` table was dumped without Ghidra |
| `pl8.js` | mode-2 / raw PL8 frame decoder, per `pl8-mode2.md` §7 |
| `pl8tab.js`, `sheet.js`, `sheet2.js` | frame tables and contact sheets of a tile set |
| `render_map.js`, `render_map2.js`, `render_crop.js` | render a whole slot, a downscaled preview, or a crop around a tile |
| `banks.js`, `xtab.js`, `xtab2.js`, `flagidx.js`, `flagmask.js`, `p4.js`, `perctny.js` | the censuses behind §1, §2 and §5 |
| `mapnames.js` | `L2.eng` group 101 beside the slot census |
| `dump.ps1`, `rtdiff.js`, `rtdetail.js`, `verify_lattice.js` | live-process dump and the runtime comparison of §4/§5 |

**Every image these produce goes to `tools/maps/out/`, which `.gitignore`
excludes.** Renders of the game's tile art are derived assets and must never be
committed (CLAUDE.md rule 1).

### The live-process route

`dump.ps1` reads `0x00522F90` (tile array) and `0x0055CEA0` (screen lattice) out
of a running `Lords2.exe`. That is what settled §4, and it is worth the trouble
**once**. But driving the game from an agent is close to unusable:

* the game runs fullscreen and takes over the user's display for as long as it is
  up;
* it is DirectDraw, so `PrintWindow` capture returns pure black and only a raw
  screen grab works;
* `SetForegroundWindow` fails silently from a background process, and every
  PowerShell process you spawn can steal focus — which minimises the game, after
  which no synthetic input reaches it at all, and doing it during the first few
  seconds crashes it outright.

In this session the game reached a loaded map and both dumps were taken before it
was lost to a focus steal. If someone repeats it: launch, wait, then navigate
with as few separate process spawns as possible, and take the memory dumps
**before** doing anything else.

---

## 8. Still open

* **The six 16-entry county lists** from plane 4 on castle tiles (§5.2). Needs
  the consumer of the table traced, which needs Ghidra.
* **Plane 0 bit `0x01` = road, `0x08` = rough terrain, `0x10` = reserved plot,
  `0x20` = farmland** are all **[I]**. They are pinned to exact graphic ranges
  and to run-time behaviour, but nothing in the data *names* them.
* ~~**What gets built on the four `0x10` plots per county**, and why exactly four.~~
  **Settled: dwellings, and four because the array holds four.** `County_FindDwellingPlots`
  (`0x00468C41`), called once per county from `Counties_PlaceSites`, sweeps the grid for this
  county's `0x10` tiles and stores each one's byte offset in `county.dwellingPlots` — four
  `i32` slots at county `+0x80` — copying the tile's `frame` into `savedFrame` and zeroing
  `content`. `County_UpdateDwellings` (`0x004684C6`) is then called once per county per
  season from `Population_UpdateAll` with a count banded on population: **0 dwellings below
  601, 1 below 1001, 2 below 1401, 3 below 1601, 4 above**. Each of the first *n* plots steps
  `content` one size up the ladder `0x13 → 0x12 → 0x11 → 0x10` (frame `0x3C` for the two
  small sizes, `0x3B` for the two large), moves the tile to the Town bank with the overlay
  bit, and sets `flags` bit `0x10`; every plot beyond *n* is razed — `frame` restored from
  `savedFrame`, bank cleared back to base, `flags` bit `0x10` cleared, `content` zeroed.
  `Unit_BurnDwelling` is the inverse, driving one plot from `0x10` straight back to `0x13`.
  **The count is not a rule, it is the storage:** the loop has no bound check and the four
  slots end exactly on `county.fieldProgress` at `+0x90`, so a fifth `0x10` tile in one
  county would overwrite a field's reclamation progress. §2.3's exactly-four-per-county
  invariant over all 434 counties is what keeps that from happening.
* **The six tiles per map with plane 0 == `0x01`** that the loader numbers 1…6
  (§5.3) — observed on one map only, and not distinguishable in the file from
  the other 12,000-odd road tiles.
* **Bank `0x10` (`Castle1?.pl8`) is never used on disk**, only assigned at run
  time. Its 25 four-frame groups are presumably the 25 castle designs; only the
  frames 88–91 group is confirmed in use (as the player's starting settlement),
  which is an odd place for it and may mean the file's name is historical.
* **County id `32`** — still unexplained, as in `maps.md`.

---

## 9. The campaign screen is a scrolling viewport — moved to `screens.md`

§4 gives the lattice geometry. It does **not** say what the game puts on screen, and that
gap produced a real mistake: `crates/l2-view`'s first campaign painter drew the whole
64 × 64 map at once, and the result reads as a minimap because that is effectively what it
is. The user said so on sight.

`Map_RenderIso` (`0x0040526E`) does not walk the lattice, it walks a **window** into it, and
**neither zoom shows the whole map**. The full treatment — the viewport, the scrolling, the
chrome, the minimap, the layout rectangles — is now [`docs/screens.md`](../screens.md), with
its own `ui` section in `symbols.json`. Two things it corrects about the first reading of
this, which lived here:

**There are two campaign zooms, not three.** `Map_SetZoom` (`0x00451FCC`) has three cases,
and case 1 is unreachable: `g_mapZoom` has three writers in the binary and none can make it
1, no shipped PL8 holds 26 × 14 map tiles, and `Map_DrawTile` has no zoom-1 branch. Near is
58 × 30 tiles at a pitch of 60 showing 8 lattice columns; far is 10 × 6 at 12 showing 40.

**The 58-versus-60 discrepancy is resolved, and both numbers were right.** §4's 58 is the
*artwork*; the renderer's 60 is the *column pitch*, and **pitch = frame width + 2** at every
zoom, with **row step = frame height / 2**. The two extra pixels are the columns the
half-tile blitters drop at the vertical seam; they fall outside the viewport at both ends.
So §4's derived 3770 × 1935 bounding box is the artwork's and not the renderer's, and the
two numbers are still not interchangeable — but they are no longer in conflict. See
`screens.md` §1.2, checked over all 830 tile frames in
`crates/l2-view/tests/install.rs`.
