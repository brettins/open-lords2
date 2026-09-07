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

**[I]** The season letter is `a`/`b`/`c`/`d`. `maps.md` reads the low 2 bits of
the scenario as the season selector; the four `*1a/b/c/d` variants of every layer
are consistent with that, and the game's own status bar says "Winter" while
holding a season's tile set.

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
| `0x40` | town | 1,736 | 0–3 | castle site, 2×2 **[V]** |
| `0x80` | base | 1,681 | 6–21 | settlement, 2×2 on grass **[V]** |
| `0x80` | town | 725 | 0, 20, 30 | extra settlement tile **[V]** |
| `0x82` | roads | 55 | 38–40 | settlement tile on a boundary **[V]** |

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
the file:

| byte | contents | agreement with file |
|---|---|---|
| +0 | object/state class, values 0,1,4,7,10,11,20,21,22,23 | runtime-only |
| +1 | plane 0 flags | 4026/4096 (see below) |
| +2 | `plane1` in bits `0x1c`; bits `0x01`, `0x20`, `0x80` set at run time | 4062/4096 |
| +3 | plane 2 frame index | 1872/4096 (variant randomisation, §4.1) |
| +4 | **plane 3, verbatim** | **4096/4096** |
| +5 | **plane 4**, then consumed | 4043/4096 |
| +6 | saved terrain frame index for tiles that get a building | runtime-only |
| +7 | **plane 5 county, verbatim** | **4096/4096** |

The 70 plane-0 differences are all load-time edits: `0x10 → 0x00` on 55 tiles
(the reserved plots, whose terrain index was copied to +6), `0x12 → 0x02` on 1,
and `0x00 → 0x80` on 14 — one per county — which simultaneously moved to bank
`0x0c` frame 10. Castle sites were rewritten from `Town` frames 0/1/2/3 to
47/48/49/50, which is exactly the 2×2 group `Town1a.pl8` frames 47–50 (the game
was started with "Starting Castle: keep"); `Town1a.pl8` has two more such
groups, 51–54 and 55–58, presumably the other castle options. Six tiles whose
plane 0 was exactly `0x01` received the values 1…6 in byte +5 — six numbered
road tiles in six different counties, purpose unknown.

**Caveat.** These runtime numbers come from **one** run of **one** map (slot 0,
custom game, England, winter, 5 players, starting castle "keep"). They are direct
observations of the shipped binary's own state and I would not expect them to
change, but they are not the 44-map invariants the rest of this document is built
on. The dumps are at `tools/maps/out/{tiles,lattice}.bin` (gitignored);
`tools/maps/dump.ps1` regenerates them — see §7 for why that is harder than it
sounds.

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
* **What gets built on the four `0x10` plots per county**, and why exactly four.
* **The six tiles per map with plane 0 == `0x01`** that the loader numbers 1…6
  (§5.3) — observed on one map only, and not distinguishable in the file from
  the other 12,000-odd road tiles.
* **Bank `0x10` (`Castle1?.pl8`) is never used on disk**, only assigned at run
  time. Its 25 four-frame groups are presumably the 25 castle designs; only the
  frames 88–91 group is confirmed in use (as the player's starting settlement),
  which is an odd place for it and may mean the file's name is historical.
* **County id `32`** — still unexplained, as in `maps.md`.
