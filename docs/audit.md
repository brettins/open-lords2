# Documentation audit

An adversarial re-derivation of the load-bearing claims in `CLAUDE.md`, `docs/`,
`docs/formats/` and `crates/l2-formats/`, done against the shipped data files and
binaries rather than against the documents.

**Nothing outside this file and `tools/audit/` was modified.** Every finding below is a
report, not a fix. Where two documents disagree I say which one the bytes support.

Scripts: `tools/audit/` — see [§7](#7-how-to-reproduce). Everything marked *verified*
below re-derives from `F:\games\Lords of the Realm II`, `F:\games\LORDS2` or the two
shipped executables in a single run. [§6](#6-what-reproduced-exactly) lists what checked
out; [§8](#8-what-i-could-not-check) lists what I had to take on trust and why.

Audited on 2026-09-07 against commit `4ddb5d5`. `docs/status.html` and `docs/symbols.md`
were being rewritten by other agents while this ran, so findings against those two are as
of the state I read; the rest of the tree was unchanged throughout. I found nothing wrong
in `symbols.md` — every claim in it I could reach from the file bytes (image base, no
ASLR, section layout, the ~955 KB `.data` figure, the import surface, `mapl2.exe` having
no `l2_maps.dat` reference, the `DAT_005BB478` correction) checked out.

---

## 1. Wrong, stated as verified, and consequential

### F1 — `L2.eng` group 41 is **not** empty in the shipped Windows file

`eng.md` §1.3, marked **[V]**:

> `mapl2.exe` asks for group 41 indices 27–29 to get its default map name / title /
> description, and group 41 is **empty in the shipped `L2.eng`**, so the strings
> `My map` / `My battle map` / `A short description of the battle map, I have created.`
> found in `USER.SKR` are not in any shipped file. The battle-map editor was built
> against a development `l2.eng` that the retail release does not include. A tool that
> wants the editor's default strings must supply them itself.

Repeated in `eng.md` §5 ("absent from every shipped `L2.eng`").

**This is false for the Windows release.** Group 41 of
`F:\games\Lords of the Realm II\L2.eng` holds **30 strings** — the complete battle-map
editor UI set — and indices 27, 28 and 29 are *exactly* the three strings the document
says are missing:

```
41[ 0..11]  Lords2 Skirmish map editor. | Build yourself a battlefield!! |
            Land | Woods | Water | Bridge | Rocks | Scrub | Pitch | Stakes |
            Attacker | Defender
41[12..26]  Small brush | Medium brush | Large brush | Troop numbers | Map text |
            Toggle radar | Main screen | Map | Exit without saving?. | Exit |
            Edit screen | New map set | Load a map set | Save a map set | Exit editor
41[27..29]  My map | My battle map | A short description of the battle map, I have created.
```

Group 41 **is** empty in the DOS `F:\games\LORDS2\L2.ENG`. The document was almost
certainly checked against the DOS file. Its own §1.4 already contains the evidence: group
41 is one of the 13 group ids that differ between the two releases (verified: ids
11, 36, 37, 38, 39, 40, **41**, 59, 88, 100, 101, 259, 299).

Consequences, all of which a reader would act on:

* the "development `l2.eng`" story is wrong — the retail Windows file ships the strings;
* "a tool that wants the editor's default strings must supply them itself" is wrong;
* §1.3's worked example of the empty-group walk ("an empty group does not fail; the
  pointer simply walks on into the next group's strings") rests on the wrong file, and
  should be re-illustrated with a group that is genuinely empty in the Windows file —
  there are 28 to choose from (42, 53–56, 58, 78, 79, 84, 90–93, 104–107, 198, 199,
  261–269);
* the §5 open question should be struck.

Reproduce: `node tools/audit/eng41.js`.

### F2 — `eng.md`: "there is no group of battle-map terrain names"

`eng.md` §1.4:

> There is **no group of battle-map terrain names**, so the `.skr` terrain byte values
> cannot be named from `L2.eng`.

**False**, and it is the same group. Group 41 indices 2–9 are the battle-map editor's
own terrain palette:

```
Land  Woods  Water  Bridge  Rocks  Scrub  Pitch  Stakes      (then Attacker, Defender)
```

Eight terrain names plus the two deployment markers. `skr.md` documents ten terrain byte
values in `USER.SKR` (`0x00, 0x02, 0x09, 0x0A, 0x10/0x12/0x14, 0x15, 0x20`, plus the two
markers `0x04`/`0x0F`) and lists three of them as unresolved:

| `skr.md` open question | Candidate from group 41 |
|---|---|
| `0x15` — "Fence? Palisade? Road? Not settled." | `Stakes` or `Pitch` |
| `0x20` — "'rocks' fits … but is not proven" | `Rocks` |
| `0x02` — "**[I]** hills / high ground" | `Scrub` |
| `0x50` — "decoded by both binaries, used by no shipped file" | the remaining palette entry |

This is the most expensive kind of documentation error: a wrong negative that closes off
an answer already sitting in the data. I have **not** established the byte→name mapping
(that needs `mapl2.exe`'s palette-button order traced), only that the names exist.

### F3 — `maps.md`: "36 of the 44 maps" support five players

`maps.md` §2, Plane 4, marked **[V]**:

> In 36 of the 44 maps there are exactly **5** settlement tiles with a non-zero plane-4
> value, carrying the values `1,2,3,4,5` — one each. The remaining maps have 2 or 4.

**Verified false. The figure is 31.** Per-map multiset of plane-4 values on settlement
tiles, over all 44 used maps:

| multiset | maps |
|---|---:|
| `1,2,3,4,5` | **31** |
| `1,2,3,4` | 5 |
| `1,2` | 8 |

`maps-layers.md` §5.1 already gives the right number and flags the discrepancy, but
`maps.md` still carries the wrong one under a **[V]**.

Where "36" came from: it is the number of *tiles* carrying value 3 (and, separately,
value 4), not the number of maps. Total tile counts are `1→44, 2→44, 3→36, 4→36, 5→31`.
The tile count and the map count were conflated.

Reproduce: `node tools/audit/mapsaudit.js`, or `node tools/maps/verify_layers.js`.

### F4 — `maps.md`: "every non-zero plane-3 tile also has `plane0 & 0x08` set"

`maps.md` §2, Plane 3, marked **[V]** and offered as the exact half of a mostly-inferred
section.

**Verified false.** Over the 44 used maps, 9,877 tiles have a non-zero plane 3, and
**2,604 of them do not have bit `0x08`**:

| plane 0 of the counterexample | tiles |
|---|---:|
| `0x40` (castle site) | 1,302 |
| `0x80` (settlement) | 1,270 |
| `0x82` (settlement on a boundary) | 32 |

That is every non-anchor member of every castle block and every settlement block — a
quarter of the population, not an edge case. `maps-layers.md` §1.2's own worked example
contradicts it directly: tile (20,3) of slot 0 is `p0=0x40, p3=1`.

Reproduce: `node tools/audit/maps2.js`.

### F5 — `maps-layers.md`: "Nothing here contradicts that document"

`maps-layers.md`'s opening paragraph:

> Companion to [`maps.md`](maps.md) … Nothing here contradicts that document; this one
> resolves the questions it left open.

**False, four times over.** `maps-layers.md` contradicts `maps.md` on:

1. **plane 3** — §3 says outright "That reading is wrong" of `maps.md`'s 3-tile/5-tile
   object model (`maps-layers` is right: `plane3 = dx + W*dy`, 10,971/10,971);
2. **the five-player map count** — §5.1 says "(`maps.md` says 36 maps carry five; the
   exact figure is 31)" (`maps-layers` is right, F3);
3. **plane 0 bit `0x20`** — §2.4 says `maps.md`'s "dwelling / housing site" "should be
   read as **farmland**";
4. **the tile→lattice mapping** — §4 settles it exactly (4096/4096), while `maps.md` §3
   still asserts **[V]** "the best fit is only 72.15%, so no exact tile-to-lattice
   correspondence was established."

The sentence is the single most misleading line in the knowledge base, because it tells a
reader they may trust both documents everywhere. They may not; where the two differ,
`maps-layers.md` is right in every case I checked.

### F6 — plane 0 bit `0x20`: dwelling vs farmland, unresolved in the code

* `maps.md` §2: `0x20` → "dwelling / housing site", marked **[V] from `FUN_00467a36`".
* `maps-layers.md` §2.4: farmland — `0x20` is stored as `Roads` frame 80 (a fallow
  ploughed field) on **every one of its 5,339 tiles**, and the three "building sizes"
  `0x50`/`0x54`/`0x68` are frames 80/84/104, three crop states.
* `crates/l2-formats/src/maps.rs` still names it `flags::DWELLING`, documented
  "Dwelling. Confirmed against the housing-placement routine."

**Verified:** class `(plane0 0x20, bank roads)` is 5,339 tiles and uses plane-2 index
**80 and nothing else** (1 distinct value). That is far more consistent with farmland
than with a dwelling site, and it is a stronger argument than the decompiler reading —
but `maps-layers.md` labels the conclusion **[I]** while `maps.md` labels the losing
reading **[V]**. Two documents and the code now disagree, and the confidence labels point
the wrong way.

---

## 2. Numbers that do not reproduce

### F7 — "15 files" is 18

`pl8.md` and `pl8-failures.md` both say fifteen, and both then list eighteen:

> **15 files: dispatching on the family byte instead of the shape byte.**
> `Base2a`, `Roads2a`, `Castle1a-d`, `Castle2a-d`, `Town1a-d`, `Town2a-d` …

`Base2a` + `Roads2a` + 4 + 4 + 4 + 4 = **18**. The documents' own arithmetic demands 18:
they say the 23 previously-failing files decompose as *these* + 4 overhang files +
`Font_c2`. 18 + 4 + 1 = 23; 15 + 4 + 1 = 20.

**Verified from the data.** Exactly **18** files with header byte 0 ≠ 2 contain
isometric frames — the eighteen named — and exactly **23** files fail under the old
family-byte model. Both counts are exact; only the label "15" is wrong.

`pl8-failures.md` §1's companion figure is right: those 18 files add **1,667** isometric
frames (4,031 total − 2,364 in the mode-2 files = 1,667 ✓).

Reproduce: `node tools/audit/oldmodel.js`.

### F8 — "6, 10, 61 and 190 … the largest such block per file" — they are the *first*

`pl8.md`:

> The apparent "undershoots" of 6, 10, 61 and 190 bytes were just the largest such block
> per file.

`pl8-failures.md` §4 says the same ("the largest such block in each file"). The same claim
is made about the overshoot side ("overshoot by exactly 24 / 840").

**Verified false.** The largest stored overhang block per file is:

| File | doc says | largest block | *first* block |
|---|---:|---:|---:|
| `Fntl2_14` | 6 | **18** | **6** |
| `Font_10` | 10 | **18** | **10** |
| `T16_bat1` | 61 | **74** | **61** |
| `T32_bat` | 190 | **242** | **190** |

6/10/61/190 are the residuals of the **first frame that fails** under the old model —
which is what a validator that stops at the first mismatch reports. Re-running the old
model reproduces exactly `−6, −10, −61, −190`.

The same correction applies to the overshoot side, and there it hides a fourth residual:
under the old model the first failure is `+24` for `Base2a`, `Castle2a`, `Roads2a`,
`Town2a-d`; `+840` for `Castle1a-d`, `Town1a-d`; and **`−36` for `Castle2b`, `Castle2c`,
`Castle2d`** — a value neither document nor the stale comment in `corpus.rs` mentions.

`pl8-failures.md` §1 does hedge ("the modal residual, not the whole story"), so the
mechanism was understood; the specific sentence in both files is still wrong.

Reproduce: `node tools/audit/pl8scope.js` and `node tools/audit/oldmodel.js`.

### F9 — "Previously: 268 / 291 files, 19,606 / 21,344 frames"

`pl8-failures.md`, top of file. **268 reproduces; 19,606 does not.**

Reconstructing the old family-byte model exactly:

| Quantity | Value |
|---|---:|
| files failing | 23 → **268 passing** ✓ |
| frames in the 268 passing files | **18,937** |
| frames decoded before the first failure, all 291 files | **19,085** |
| doc's figure | 19,606 |

No natural counting rule I tried lands on 19,606 (difference 669 from the closest). It is
a historical figure and harmless in itself, but it is presented alongside two figures that
*do* reproduce, which makes it look equally solid.

### F10 — "24 frames … declare rows and still hold exactly `height²`"

`pl8.md` says the corpus figure is 24. `pl8-mode2.md` §3 says "24 `Batlfix2` frames",
which is **correct within that document's mode-2 scope**.

**Verified: the whole-corpus figure is 32** — `Batlfix2` 24, plus `Town1a`, `Town1b`,
`Town1c`, `Town1d` at 2 each. `pl8.md` generalised a scoped number to "the corpus"
without re-counting.

The same wrong number is in the crate, twice:
`crates/l2-formats/src/pl8.rs` — "24 frames in the corpus declare rows and still occupy
exactly h^2" (decode comment) and "24 frames in the real corpus declare rows on shape 1"
(test `a_diamond_ignores_its_overhang_row_count`).

Behaviourally harmless — shape 1 ignores the byte either way — but it is a count stated
as fact that is 33% low.

### F11 — `maps-layers.md`: "17 of the 434 blocks"

§3:

> a member of a settlement block that happens to sit on a county boundary switches to bank
> `roads`, frames 38/39 (the boundary variant), so a decoder must not assume all four
> members share a bank. 17 of the 434 blocks are like that.

**Verified: 40 blocks, not 17.** Distribution of roads-bank members per settlement block:

| roads members | blocks |
|---:|---:|
| 0 | 394 |
| 1 | 26 |
| 2 | 13 |
| 3 | 1 |

26 + 13 + 1 = **40** blocks; 26 + 26 + 3 = **55** tiles, which is exactly the document's
own `0x82 | roads` class count of 55. No reading of the data gives 17. (Minor: the class
uses frames 38, 39 **and 40**, not just 38/39.)

The load-bearing half of the claim — "a decoder must not assume all four members share a
bank" — stands, and is understated rather than overstated.

Reproduce: `node tools/audit/maps3.js`.

### F12 — county id 32: "0 to 1213 tiles per map"

`maps.md` §6. **Verified: the maximum is 1,813**, over 29 of the 44 used maps. Looks like
a digit transposition. Reproduce: `node tools/audit/maps4.js`.

### F13 — `decisions.md` C4's "verified figure is 16,435"

> **C4 — Frame count reported as 16,638.** That was the Node checker counting frames
> inside files that later failed validation. The verified figure is 16,435.

Present tense, in the corrections log, which is where a reader goes for the *current*
truth. 16,435 corresponds to nothing derivable today: the corpus is 21,344 frames in
291/291 files, and even the old model's passing-file count is 18,937. `docs/status.html`
still carries the *retracted* 16,638 alongside "236/291" (see F22).

`decisions.md` is append-only and lead-owned, so this is reported rather than proposed as
an edit — but the sentence reads as a live number.

---

## 3. Documents that disagree with the code

### F14 — `pl8-failures.md` says its findings are "recorded, not applied". They are applied.

§4: "Two corrections to `docs/symbols.md` fall out of this (the file is owned by someone
else, so they are recorded here rather than applied)". §6: "What this changes for
`crates/l2-formats` — Owned by someone else; recorded, not applied", listing five items.

All five have since been done, and `symbols.md` has both corrections:

| Recorded change | State in the tree |
|---|---|
| dispatch on `rec[0x0C]` for every family | done — `pl8.rs` `decode_inner` |
| consume `rec[0x0D]` RLE rows after a shape-0 rectangle | done — `decode_rle_rows` |
| `Font_c2` raw fallback | done — whole-file span reclassification in `Pl8::parse` |
| empty `KNOWN_FAILING`, `VALIDATED_BASELINE` = 291 | done — `corpus.rs` |
| `symbols.md`: `DAT_005BB478` is the blitter row counter | done — `symbols.md` Globals table |

A reader of `pl8-failures.md` today would think there is outstanding integration work.
There is none.

### F15 — frame record `0x0E` is documented as a `u16`; `0x0F` is not always zero

* `pl8.md` frame-table: `0x0E | u16 | Padding`.
* `crates/l2-formats/src/pl8.rs` module doc: `0x0E u16 padding`.
* `pl8-mode2.md` §3 splits them: `0x0E u8` — "never non-zero in any of the 21,344 frames
  in the corpus"; `0x0F u8` — "non-zero in 32 frames, none of them mode 2".

**Verified: `pl8-mode2.md` is right.** Byte `0x0E` is zero in all 21,344 frames; byte
`0x0F` is non-zero in exactly **32** frames — 16 in `Fntl2_22.pl8` and 16 in
`Font3c2.pl8`, both header family 0, neither mode 2. So the field is *not* a `u16`
padding: read as a `u16` it is non-zero 32 times. Same result on the DOS corpus (32).

### F16 — `pl8.md` states as fact what `pl8-failures.md` records as not established

`pl8.md`, on shape-0 overhang rows:

> A rectangle may store `rows` extra RLE-encoded rows immediately after it, drawn *above*
> it — real artwork continuous with the rectangle, such as the accent on a glyph …
> `Glyph_Draw` (`0x00402A14`) shifts the destination down by that row count before
> clipping, reserving exactly those rows.

`pl8-failures.md` §4, on the same 70 frames:

> **Not established:** the shipped English text renderer never paints those overhang rows.
> … So the accents stored in `Fntl2_14` and `Font_10` appear to be present but unpainted.
> I could not find a second font path.

Both cannot be presented at the same confidence. `pl8.md` is the document a newcomer
reads first, has no **[V]/[I]** legend at all, and drops the caveat — a `CLAUDE.md`
rule 4 breach. The *decoder* behaviour is not in dispute (consume the bytes); the claim
that the rows are drawn is.

### F17 — `corpus.rs`: `KNOWN_FAILING` carries two contradictory doc comments

`crates/l2-formats/tests/corpus.rs` has two `///` blocks stacked on the same constant.
The first still describes the pre-fix world:

> Files that use a supported storage mode but still do not decode cleanly. These are
> unexplained, not excused … overshoot by exactly 24 bytes: Base2a, Roads2a, Castle2a,
> Town2a, Town2b-d / overshoot by exactly 840 (24 * 35): Castle1a-d, Town1a-d /
> undershoot: Fntl2_14 (6), Font_10 (10), T16_bat1 (61), T32_bat (190) / row overrun:
> Font_c2

The second says the list is empty and all 291 files decode. Both render into the same
rustdoc. Beyond being stale, the first block's census is **wrong independently of F8**:
it names 20 files, not 23, omitting `Castle2b`, `Castle2c` and `Castle2d` entirely —
whose first residual is `−36`, not `+24`. (Reported, not fixed: `crates/` is out of
scope for this task.)

### F18 — `crates/l2-formats/src/maps.rs` confidence labels are inverted

| Item | Code says | Documents say |
|---|---|---|
| `Plane::GfxBank` | "Selects one of five sprite banks. *Inferred.*" | `maps.md` §2 **[V]** — read out of `FUN_004063c1`; `maps-layers.md` §1 **[V]** with four independent arguments |
| `Plane::GfxIndex` | "Indexes a descriptor within the bank. *Inferred.*" | as above, **[V]** |
| `Plane::ObjectPart` | "Part index within a multi-tile object. *Inferred.*" | `maps-layers.md` §3 **[V]**, 10,971/10,971 |
| `flags::DWELLING` | "Dwelling. Confirmed against the housing-placement routine." | `maps-layers.md` §2.4 — farmland (F6) |

Three under-claims and one over-claim, all in the same enum. `CLAUDE.md` rule 4 applies to
code comments as much as to prose, and this is the file a future implementer reads.

### F19 — `Error::UnsupportedStorage` doc comment is stale

`crates/l2-formats/src/lib.rs`: "Header byte 0 held a storage mode we cannot decode yet
(mode 2)." Mode 2 has been fully decoded since `073c168`; the variant now only fires for
byte values ≥ 3, which no shipped file has.

### F20 — `pl8-failures.md` §2 describes a decode model the crate does not implement

The document's §2 "Verified decode model" gates on header bytes:

```
if header[0x00] == 1 and header[0x01] == 0:   # RLE sprite stream
if header[0x00] == 0 and rows > 0:            # rectangle overhang
```

The crate decides both **structurally**: RLE-vs-raw by whether *every* frame in the file
spans exactly `w*h` (`Pl8::parse`), and rectangle overhang by whether the bare rectangle
lands on the next frame's offset (`rect_overhang` in `decode_inner`). Both models validate
291/291 — I ran each — and `pl8-failures.md` §4 *offers* the structural test as an
alternative, but §2 is labelled "Verified decode model" and is what a reimplementer would
copy. `pl8.md`'s "decide structurally" matches the code; `pl8-failures.md` §2 does not.

Note the document's own warning applies here: the `header[0x01] == 0` half of the RLE
test "rests on a single file" (`Font_c2`). The crate does not rely on it, which is the
better design and is worth saying explicitly.

---

## 4. Stale references

35 scratch scripts were deleted and the Node decoders retired (`decisions.md` D7). The
documents were not swept.

| Document | Reference | State |
|---|---|---|
| `environment.md` "Commands" | `powershell -File tools/pl8diff.ps1` | **gone**; the harness it runs is retired by D7. The document that exists to stop commands wasting time now costs time. |
| `README.md` "Testing" | `.\tools\pl8diff.ps1`, plus five paragraphs describing the Node-vs-Rust differential test | **gone**, and contradicts D7 outright |
| `README.md` "Tools" | `tools/pl8dump.js`, `tools/pl8check.js`, `tools/pl8digest.js`, `tools/pl8diff.ps1` | all four **gone** |
| `README.md` | `crates/l2-formats/examples/pl8digest.rs` | **gone** (`crates/l2-formats/` has no `examples/`) |
| `maps.md` §5 | `node tools/maps/validate.js` (×2), `node tools/maps/export_png.js` (×2) | **gone**. The quoted `validate.js` output is still reproducible — `tools/maps/verify_layers.js` prints the same invariants — but not by the command given. |
| `pl8-mode2.md` §8 | `node tools/pl8mode2check.js` | **gone**. Its quoted output still reproduces (`tools/pl8fail/final.js`, and `tools/audit/pl8audit.js`). |
| `maps-layers.md` §7 script table | 15 of 21 entries | **gone**: `pl8tab.js`, `sheet.js`, `sheet2.js`, `render_map2.js`, `render_crop.js`, `banks.js`, `xtab.js`, `xtab2.js`, `flagidx.js`, `flagmask.js`, `p4.js`, `perctny.js`, `rtdiff.js`, `rtdetail.js`, `verify_lattice.js`. Surviving: `verify_layers.js`, `pe.js`, `pl8.js`, `render_map.js`, `mapnames.js`, `dump.ps1`. Unlisted but present: `png.js`, `merchants.js`. |

Only `tools/pl8fail/final.js`, `tools/skr/skr.js`, `tools/skr/eng.js` and
`tools/maps/verify_layers.js` survive of the documented validators — and all four run
today and reproduce their quoted output verbatim (§6).

### F21 — `decisions.md` "Open questions" is stale in four of six bullets

| Bullet | Status |
|---|---|
| "the four map planes whose meaning is inferred … and the exact tile → lattice mapping, whose best affine fit reaches only 72%" | **resolved** — `maps-layers.md` §1/§3/§4; the lattice mapping is exact at 4096/4096 |
| "23 files that use supported encodings but fail the end-offset invariant, pinned in `KNOWN_FAILING`" | **resolved** — `KNOWN_FAILING` is empty; 291/291 |
| "`Font_c2.pl8` declares RLE but its frames occupy exactly `width × height`" | **resolved** — `pl8-failures.md` §5 |
| "`Title.pl8` decodes with correct geometry but no shipped palette colours it" | still open |
| "the type-4 apex pair" | still open |
| "PL8 header fields at 0x04, 0x06, 0x07" | still open |

Two thirds of the list is answered elsewhere. Same problem in `maps.md` §6, whose "map
names are not in this file … presumably in `L2.eng`" is answered by both `eng.md` and
`maps-layers.md` §6 (group 101, 60 names, exact).

### F22 — `docs/status.html`

Carries `236/291` and `16,638 frames` — the figure `decisions.md` C4 explicitly
retracted — plus rows for `pl8diff.ps1`, `pl8check.js` and `pl8dump.js`, none of which
exist. **Caveat:** `status.html` is lead-session-owned and is modified in the working tree
right now, so this may already be in hand; listed for completeness only.

### F23 — `skr.md`: "a recursive search of `F:\games` … found no others"

There is now a second path, `F:\games\lords2-instrumented\USER.SKR` — the hard-link
sibling directory from `decisions.md` D8. It is the **same inode** (verified:
`844424930179566` for both), so the substance of "exactly one `.skr`" holds and the
19-blank-maps argument is unaffected. Worth a line so nobody thinks a second corpus
appeared. `environment.md`'s Paths table does not mention that directory at all.

---

## 5. Smaller things

* **`maps.md` §2, plane 3:** "per map, values `1,2,3` occur in **exactly equal counts**"
  is contradicted by the document's own table two lines later — slot 4 reads
  `1,2,3 -> 112,111,112`. Verified: the histogram is exact. (The whole model is
  superseded by `maps-layers.md` §3 anyway, which explains the asymmetry.)
* **`maps.md` §2, plane 1:** "Bits `0x01` and `0x20` of the runtime field are set at run
  time"; `maps-layers.md` §5.3 says `0x01`, `0x20` **and `0x80`**. Untestable statically;
  noted only as a two-document disagreement.
* **Lattice orientation notation** drifts: `maps.md` writes "65 x 129" (65 wide),
  `maps-layers.md` writes both "65 × 129" and "129x65 dword array", `symbols.md` writes
  "65x129 layer" with a `for(row < 0x81) for(col < 0x41)` loop. All are the same object
  and `maps.rs` (`LATTICE_W = 65`, `LATTICE_H = 129`) is unambiguous, but a reader has to
  work that out.
* **`pl8-mode2.md` §9** points `DumpBytes.java` at `0x004D9FC0` for "the resource-name
  table" while `maps-layers.md` §1.1 and `maps.md` locate it at `0x004DA050`. Verified:
  the table starts at **`0x004DA050`** (`base1a.pl8`, 128300). The `0x004D9FC0` dump is
  400 bytes and does cover it, so the command works — but the address given is not the
  table's.
* **`eng.md` §3.2:** "`TROOPS2.ENG` and `TROOPS3.ENG` … only their `Normal` rows are
  populated, every other difficulty row is all zeros" and "`TROOPS.ENG` … still has all
  five groups filled in", both **[V]**. Verified as overstatements in both directions:
  non-Normal groups are non-zero in **6** of `TROOPS2`'s 35 rows (15–20), **5** of
  `TROOPS3`'s (15–19), and only **9** of `TROOPS.ENG`'s (0–4, 20–23). The qualitative
  point — the non-Normal groups are dead data the engine overwrites — is unaffected.

---

## 6. What reproduced exactly

Everything in this section was re-derived from the shipped files by
`tools/audit/`, independently of `crates/l2-formats/` and of the existing `tools/`
scripts, and matched the documented value to the digit.

**PL8** (`pl8audit.js`, `pl8claims.js`, `pl8m2.js`, `pl8scope.js`)

* 291/291 files, **21,344**/21,344 frames, zero end-offset failures (GOG); 222/222 files,
  **14,648**/14,648 frames (DOS `F:\games\LORDS2\PL8`).
* `pl8-failures.md` §2's rule census, every cell: `rle` 9,603 · `raw` 7,636 ·
  `iso1/2/3/4` 2,425 / 812 / 432 / 362 · `raw`+overhang 70 · region grid 4. Sums to
  21,344.
* Mode 2: 32 files, 2,502 frames, shape histogram `{0:138, 1:1808, 2:344, 3:104, 4:108}`
  = 2,364 isometric + 134 raw + 4 grids.
* Diamond geometry `w == 2h − 2`, `h` even: **4,031 / 4,031** isometric frames (2,364 of
  them in mode-2 files, as `pl8-mode2.md` §4 claims).
* Zoom byte ↔ tile size, with no exceptions: `2:0`→58×30, `2:1`→26×14, `2:2`→10×6, and
  the family-0 isometric files agree (`0:0`→58×30, `0:2`→10×6).
* Header `0x04` range 0..287; `0x06` non-zero in exactly one file of 291 (`T16_bat1`,
  value 1); `0x07` range 0..15; first frame's `dataOffset == 8 + count*16` in 291/291.
* `Base2a.pl8` vs `Base2b.pl8`: both 7,288 bytes, 140 frames, 2,248-byte header+table
  differing in **exactly one byte — byte 0**, and 805 of 5,040 pixel bytes.
* `Font_c2.pl8` vs `Fntl2_9.pl8`: 108 frames each, **103** matching records, differing at
  frames 80, 81, 105, 106, 107; headers differ only at `0x00` (1 vs 2) and `0x04`
  (107 vs 82); every `Font_c2` frame spans exactly `w*h`.
* Byte `0x0D` on shape-0 frames, all six files: `Fntl2_14` 47, `Fntl2_9` 47, `Font_10` 7,
  `Font_c2` 47, `T16_bat1` 8, `T32_bat` 8 — 70 stored, 94 not, nothing in between, and
  the structural detection has zero false positives across both corpora.
* Region grids: `Arm_grid` 640×480→4,800; `Mercgrid` 640×480→4,800; `Villgrid`
  448×376→2,632; `Vill_gd8` 360×320→1,800. Value alphabets 9–13 distinct small values
  (0–14, 32, 60–63).
* Type-4 apex, scoped to mode 2 as `pl8-mode2.md` §5 intends: **400** overhang records,
  bytes 0/1 non-zero in **100**/**86**, bytes 28/29 in **19**/**6**. (Over all 291 files
  the same statistics are 1,472 / 794 / 679 / 62 / 46 — worth a scope note in the doc.)
* Bank frame counts: `Base1a` 140, `Mtns1a` **25**, `Roads1a` 140, `Town1a` 61,
  `Castle1a` 100.
* Engine string references, exactly as `pl8-failures.md` §5 claims: `fnt_8`, `fntl2_9`,
  `fntl2_14`, `fntl2_22`, `font_10`, `t32_bat1`, `t32_bat2`, `base2a` present in
  `Lords2.exe`; `font_c2`, `font3c2`, `t16_bat1`, `base2b` absent.
* `cargo test -p l2-formats` with `LORDS2_DIR` set: 23 tests, all pass, "291 validated
  (21344 frames), 0 failing".

**`L2_maps.dat`** (`mapsaudit.js`, `maps2.js`, `maps3.js`, `maps4.js`)

* 2,636,880 = 80 × 32,961; 1,318,440 = 40 × 32,961; `0x80C1` = 32,961 =
  `6*4096 + 65*129`; DOS file is a byte-identical prefix.
* Census 44 used (0–23, 40–59) / 36 empty; **180,224** tiles; both "used" definitions
  (planes not all zero, some plane non-constant) give the same 44 slots.
* Empty slots 24–39 mutually byte-identical, 60–79 mutually byte-identical, the two
  groups different; slot 40 vs slot 0 = **18.69 %** of bytes, county plane differs by
  **1** byte, tails identical; the other 19 new slots differ from every DOS map by
  **27.1–46.1 %**; all 44 used slots unique.
* Plane alphabets, all six: 14 / 4 / 122 / 9 / 7 / 18 distinct, with the exact value sets
  `maps.md` §2 lists. Counties per map 4–16, summing to **434**.
* `(plane0 & 0x04) ⟺ plane5 == 0`: **180,224 / 180,224**.
* `0x40` tiles: **1,736** forming **434** complete 2×2 blocks, zero leftover, and
  `castle tiles == 4 × counties` in every map.
* `maps-layers.md` §1.1 bank census: 135,319 / 9,046 / 33,398 / 2,461 / **0**, with plane-2
  ranges 6–121 (108 distinct), 0–24 (all 25, saturated), 0–83 (79 distinct), 0–30
  (6 distinct).
* `maps-layers.md` §2's 16 `(plane 0, bank)` classes: every count and every index range,
  including `0x03` at 46–71, 73–75 (29 distinct of the 30-wide span — 72 is the gap).
* Bit `0x02`: **7,961 / 7,961** boundary-adjacent; 15,757 of 107,882 land tiles are
  boundary-adjacent (**14.6 %**).
* Bit `0x10`: **1,736** tiles, exactly 4 per county in every county of every map.
* Plane 4: settlement `{1:44, 2:44, 3:36, 4:36, 5:31}`, castle
  `{1:198, 2:196, 3:169, 4:174, 5:171, 6:125}`, non-zero only where `plane0 & (0x40|0x80)`.
* Plane 3: castle blocks 434/434 are `Town` frames 0/2/1/3 with plane3 0/1/2/3;
  settlement parts 1,2,3 occur exactly once per county (434 each); `plane3 == dx + W*dy`
  10,971/10,971 via `tools/maps/verify_layers.js`.
* Lattice: `row = x+y+1, col = (x−y+64)>>1` is injective over 4,096 tiles into 65×129,
  leaving 4,289 uncovered; alphabet `{0x06, 0x16}` over used slots; slots 16, 18, 44 have
  zero `0x06` cells; slot 23 has 136 (as does the DOS blank template — the Windows
  template has 2,544, so "the blank template" is ambiguous); slot 0's covered cells are
  **1,822** `0x06` / **2,274** `0x16`.
* Resource table at VA **`0x004DA050`**: `base1a.pl8` 128300, `mtns1a.pl8` 29000,
  `roads1a.pl8` 148000, `town1a.pl8` 90000, `castle1a.pl8` 122000, `sprite1a.pl8` 289000,
  `sprite1b.pl8` 63000, `flags1a.pl8` 55004, then `b`/`c`/`d` at entries 8/16/24 and the
  zoom-2 sets from 32.
* `l2_maps.dat` occurs **twice** in `Lords2.exe` and **zero** times in `mapl2.exe`.

**`.eng`** (`engaudit.js`, `engdiff.js`, `eng41.js`)

* `L2.eng` 100,016 bytes, magic `Textfile`, `offset(1)` 1,280 → **318** slots, ids 1–317,
  **3,229** strings, **28** empty groups, 4th byte of every slot zero, no control byte but
  NUL, every group region decomposing exactly into NUL-terminated strings.
* DOS `L2.ENG` 91,054 bytes → 300 slots, **2,349** strings, 33 empty groups.
* **286** of the 299 shared groups byte-identical, **13** differ, Windows appends
  ids 300–317 (**18** groups).
* All of `eng.md`'s group landmarks: 1 (File/New Game/Load/Save/Quit), 6 (15 goods),
  30 (terrain names beginning `Scrubland.`), 100 (**1,200** county names),
  **101 (60 map names, aligning exactly with the 0–23 / 24–39 / 40–59 census)**,
  103 (settings words), 220 (35 tooltips).
* `BATTLES.ENG` 7,150 bytes → **171** non-empty fields = **57** records, first triple
  `Three Bridges` / `A Bridge Too Far`, records 35–36 `Siege14`/`Siege15`, last
  `User battle20`.
* `TROOPS*.ENG` all 22,764 bytes, each **3,885** numbers after the first `*` and **35**
  `*` markers; siege columns 7–10 never exceed 9; pairwise byte differences 325 / 1,247 /
  1,363 (doc says "325–1,363"); `TROOPS.ENG` mtime 1997-04-30.

**`.skr`** (`skraudit.js`, `mapl2.js`)

* `USER.SKR` 133,748 bytes = `40*44 + 20*183 + 328 + 20*6400` = 1,760 + 3,660 + 328 +
  128,000, and every hex literal in the document: `0x6E0`, `0xE4C`, `0x1F548`, `0x20A74`,
  `0x152C`, `0x1674`, `0x1900`, `0xB7`.
* 328-byte pad at 5,420 is entirely zero.
* Map 0 armies exactly as quoted; records 2–39 all equal the default template
  `{50,0,0,50,0,50,0,0,0,0,0}` (**38/38**); fields 7–10 zero in all 40 records.
* Text block: 60/60 fields NUL-terminated inside their slot; map 0's stale `st` residue
  after `Sample Map\0` is present.
* Terrain: **19** maps byte-identical to the editor's blank template with
  `(x=40,y=20)=0x04, (x=40,y=60)=0x0F` — and **0** under the transposed reading, which
  independently confirms `grid[y*80 + x]`. Alphabet
  `{00,02,04,09,0a,0f,10,12,15,20}`, a strict subset of the twelve decoded values. Every
  map carries exactly one `0x04` and one `0x0F`.
* `mapl2.exe`: offset table at `0x00433D88` = `0x148, 0x1A48, … 0x1DC48` (first 328,
  stride 6,400, last + 6,400 = 128,328); default army record at `0x00434040`; the dialog
  labels `Peasants`/`Crossbowmen`/`Macemen`/`Swordsmen`/`Pikemen`/`Archers`/`Knights`,
  `Attacker`/`Defender`, `Full name`/`Short name`/`Brief description` and the German
  `Voller Name`/`Namenskürzel`/`Kurzbeschreibung`, all present as UTF-16 resources.
* `L2MAP.INF` 268 bytes, `u32[0..2]` = 19 / 32 / 14, grid 80 × 80, last file `Eric.skr`.

**`Lords2.exe`** (`pe.js`)

* 1,031,680 bytes, `ImageBase = 0x400000`, `DllCharacteristics = 0` → **no ASLR**.
* Sections exactly as `symbols.md`: `.text` `0x401000`–`0x4CFB06` (846,598 B),
  `.rdata` `0x4D0000`, `.data` `0x4D2000` with 1,035,872 virtual against 80,384 raw →
  **955,488 bytes** of zero-initialised globals, `.idata` `0x5CF000`.
* Imports: **one** function from `DDRAW.dll` (`DirectDrawCreate`), **two** from
  `DPLAYX.dll` by ordinals 1 and 2, and no `sierranw.dll` / `snwvalid.dll` entry —
  though both names are present as strings.

**Documented validators still in the tree, run today**

`tools/pl8fail/final.js`, `tools/maps/verify_layers.js` (15 checks, 0 failed),
`tools/skr/skr.js validate` and `tools/skr/eng.js validate` all run and print their quoted
output verbatim.

---

## 7. How to reproduce

Node is required; use forward-slash paths.

```bash
node tools/audit/pl8audit.js "F:/games/Lords of the Realm II" "F:/games/LORDS2/PL8"
node tools/audit/pl8claims.js "F:/games/Lords of the Realm II"   # F7 evidence, Base2a/Font_c2
node tools/audit/pl8detail.js "F:/games/Lords of the Realm II"   # F10, F15 per-file
node tools/audit/pl8scope.js  "F:/games/Lords of the Realm II"   # F8, type-4 apex scoping
node tools/audit/pl8m2.js     "F:/games/Lords of the Realm II"   # mode-2 shape histogram
node tools/audit/oldmodel.js                                     # F7, F8, F9

node tools/audit/mapsaudit.js      # census, alphabets, invariants, F3
node tools/audit/maps2.js          # F4, lattice, tail, castle blocks
node tools/audit/maps3.js          # F11
node tools/audit/maps4.js          # F12

node tools/audit/engaudit.js       # container, BATTLES, TROOPS
node tools/audit/enggroups.js      # group landmarks
node tools/audit/engdiff.js        # DOS vs Windows groups
node tools/audit/eng41.js          # F1, F2
node tools/audit/troops.js         # §5 TROOPS overstatement

node tools/audit/skraudit.js       # .skr container and terrain
node tools/audit/mapl2.js          # mapl2.exe literals and dialog labels
node tools/audit/pe.js "F:/games/Lords of the Realm II/Lords2.exe" 4DA050 40
```

All are read-only against the installs and write nothing outside stdout.

---

## 8. What I could not check

Stated so the audit is not mistaken for a clean bill of health on the whole knowledge
base.

* **Every claim derived from Ghidra decompilation.** All the `FUN_xxxxxxxx` behavioural
  readings — blitter loop shapes, `Clip_Vertical` overwriting `DAT_005BB478`, the
  `FUN_00406673` overhang dispatch, `FUN_00429153`'s six 16-entry lists,
  `FUN_0047B8B2`'s terrain→id map, `FUN_0042AC0C`'s difficulty re-derivation, the
  type-4 `ADD ESI, 0x2` — were taken on trust. I verified only what is visible in the
  files themselves: section layout, image base, imports, string presence and counts, and
  static data at the quoted addresses (`0x004DA050`, `0x00433D88`, `0x00434040`), all of
  which checked out. Running Ghidra would have contended for the project lock with agents
  working now.
* **The live-process observations** in `maps-layers.md` §4.1 and §5.3 (variant
  randomisation, the eight runtime tile-record bytes, the 88/90/89/91 settlement rewrite).
  The document already labels these as one run of one map. Repeating them means launching
  the game, which `agents.md` and `decisions.md` D8 both say to avoid, and `dump.ps1`'s
  own preconditions make it a poor use of an audit.
* **Everything visual.** "Renders as England and Wales", "Australia with Tasmania",
  "`Backgrnd.pl8` … pixel-exact", "crisp isometric diamonds", the ASCII plates in
  `pl8-failures.md` §4 and `skr.md`. The map-name evidence is strong corroboration —
  `L2.eng` group 101 names slots 0–59 and its used/empty pattern matches the census
  exactly, which no wrong lattice mapping would produce — but I did not render anything.
* **The 72.15 % affine-fit figure** in `maps.md` §3. Superseded by `maps-layers.md` §4
  either way, and the search that produced it no longer exists in `tools/`.
* **`pl8-mode2.md`'s prior-art comparison** with `pl8image` / OpenLotR2. Both are GPL-3;
  `CLAUDE.md` rule 3 and the project's own licence hygiene make reading them to check the
  overlap the wrong move for an auditor.
* **Byte→name mapping for the `.skr` terrain palette** (F2). I established that the names
  exist in `L2.eng` group 41; matching them to `0x02`/`0x15`/`0x20`/`0x50` needs
  `mapl2.exe`'s palette-button order, which is a Ghidra task.

## 9. Summary

23 findings. Four are wrong facts carrying a **[V]** label (F1, F3, F4, and F6's half),
one is a false assurance that two documents agree (F5), seven are counts that do not
reproduce (F7–F13), six are document-vs-code disagreements (F14–F20), and the rest are
stale references.

The corpus-level numbers the project leans on are sound: **291/291 files and 21,344
frames**, **222/222 and 14,648** on the DOS install, **180,224 tiles**, **434 castle
blocks**, **3,229 strings**, **44 used map slots**, **32,961-byte slots**, and
**133,748 = 40*44 + 20*183 + 328 + 20*6400** all re-derive exactly, independently, and
so do the great majority of the finer counts. The failures cluster in two places: numbers
carried across a scope boundary without re-counting (F7, F8, F10, the type-4 apex
scoping), and claims about *files* that were checked against the wrong install (F1, F2).
