# HANDOFF — the `Industry` record base

**The job, in one line:** establish the base of the county `Industry` record array so the
four industry-row forecasts on the campaign sidebar can be drawn from the right record,
per `docs/draws-map.md` §5.5.

**Status: the base IS established. I wrote no code.** Everything below is a reading of the
decompilation; the corrections to `docs/records.json`, `crates/l2-kingdom/src/county.rs`,
`crates/l2-game/src/screens/county.rs` and `docs/draws-map.md` §5.5 are **not made yet** and
are the next step.

---

## 1. The answer

**The `Industry` record base is county `+0x294`, not `+0x290`. [V]**

Stride `0x18`, four records, `0x294 … 0x2F4`. `docs/records.json` says `+0x290` and it is
**four bytes too low**. Every absolute offset in that file is nevertheless *correct*, because
its field offsets are compensatingly `+4`; only the base and the field-relative numbers are
wrong. Nothing in the shipped code reads a field at record `+0x00` under the old base, which
is why the error never fired anywhere except here.

### The corrected record

| rec off (true) | old records.json off | county absolute | name |
|---|---|---|---|
| `+0x00` | `+0x04` | `0x294 + c*0x18` | `efficiency` (u8) |
| `+0x01` | `+0x05` | `0x295 + c*0x18` | `hasResource` (u8) |
| `+0x02` | `+0x06` | `0x296 + c*0x18` | `disabledSeasons` (u8) |
| `+0x03` | `+0x07` | `0x297 + c*0x18` | `enabled` (u8) |
| `+0x04` | `+0x08` | `0x298 + c*0x18` | `siteTile` (i32) |
| `+0x0A` | `+0x0E` | `0x29E + c*0x18` | `capacity` (i16) |
| `+0x0C` | `+0x10` | `0x2A0 + c*0x18` | `total` (i32) |
| `+0x10` | `+0x14` | `0x2A4 + c*0x18` | `totalSnapshot` (i32) |
| **`+0x14`** | *(off the end)* | **`0x2A8 + c*0x18`** | **`next_season`** — the row's forecast |

**County `+0x290` is not part of the array.** It is the standalone `weapon_type` byte
(C136 already says so): `Industry_LabourEstimate` indexes `&g_weaponCost + county[+0x290]*8`
and `FUN_004106C4` picks the blacksmith frame as `county[+0x290] + 0x30`. Ghidra's struct
swallowed it into `industry[0]`, and that single mistake is the origin of the whole
"every row reads the record above its own commodity" story.

**Commodity numbering is unchanged and confirmed:** `0` wood, `1` iron, `2` weapons,
`3` stone (`Industry_ToggleFromMap`'s own comment at `00430000.c:7688`, and
`County_RefreshEstimates`' four calls).

---

## 2. How it was established — three derivations, all agreeing

### (a) The span argument — self-verifying, and the strongest [V]

Every raw county address in the whole decompilation that is used with a `* 0x18` stride:

```
0x53fc44 0x53fc45 0x53fc46 0x53fc47 0x53fc48 0x53fc4c 0x53fc4e 0x53fc50 0x53fc54 0x53fc58
```

`g_counties = 0x0053F9B0` [V, `docs/symbols.json` line 651, C59]. Subtracting gives county
offsets `0x294 0x295 0x296 0x297 0x298 0x29C 0x29E 0x2A0 0x2A4 0x2A8`.

Lowest is `0x294`. Highest is `0x2A8`, a 4-byte field, ending at `0x2AC`.
`0x2AC − 0x294 = 0x18` — **exactly the stride, with no slack at either end.** A base of
`0x290` cannot hold `0x2A8+c*0x18` at all; a base of `0x294` holds all ten observed fields
and closes exactly on the stride. This is the invariant the task asked for — the equivalent
of `county+0x0F == 5 − taxRate` — and it needs no fixture to run.

Reproduce it with:
```
grep -rn "0x53f[bc][0-9a-f][0-9a-f]" tools/oracle/decomp/*.c | grep "0x18" \
  | grep -o "0x53f[bc][0-9a-f][0-9a-f]" | sort -u
```

### (b) The array's end lands exactly on the next named field [V]

`docs/records.json` puts `levySurcharge` at county `+0x2F4`.

* base `0x290` → array ends `0x2F0`, leaving a **4-byte unnamed hole at `0x2F0`** — which is
  precisely the word the stone row reads, and precisely the word §5.5 called "one whole
  record past the end".
* base `0x294` → array ends **exactly at `0x2F4`**, no hole, and the stone row's `0x2F0` is
  record 3's `+0x14`.

### (c) `records.json`'s own comment refutes itself [V]

The `Industry` entry's comment reads: *"The 0x18 stride is confirmed by the byte quad at
`+0x04..+0x07` repeating at county `0x294`, `0x2AC`, `0x2C4` and `0x2DC`."* Those four
addresses **are the four record bases.** Whoever measured them measured the right thing and
then wrote them down as "`+0x04` of a record at `0x290`".

### (d) Painter side vs producer side — the two ends the task asked for, and they agree [V]

**Painters** (`tools/oracle/decomp/00410000.c`), each one `Ui_DrawDelta(<operand>, 0, …,
0x22C, pitch*row + 0x139, &g_font10, 0xFA, 0xF9)`:

| painter | Ghidra operand | county abs | rec, field (base `0x294`) | commodity |
|---|---|---|---|---|
| `FUN_0041062E` | `*(int*)(industry + 1)` | `0x2A8` | `[0] +0x14` | **wood** |
| `FUN_00410502` | `*(int*)(industry + 2)` | `0x2C0` | `[1] +0x14` | **iron** |
| `FUN_004106C4` | `*(int*)(industry + 3)` | `0x2D8` | `[2] +0x14` | **weapons** |
| `FUN_00410598` | `*(int*)&field_0x2f0` | `0x2F0` | `[3] +0x14` | **stone** |

(Ghidra's operands are `industry + c + 1` only because its base is one field low; the
absolute addresses are an exact arithmetic progression of `0x18`.)

**Producer** (`Industry_LabourEstimate`, `0x0044F318`, `00440000.c`), tail:

```c
*(int *)(county * 0x300 + 0x53fc58 + industry * 0x18) = local_1c;   /* = +0x2A8 + c*0x18 */
```

Same expression, same stride, and `County_RefreshEstimates` calls it
`(1,4,15,1)` iron, `(3,5,15,2)` stone, `(0,6,20,1)` wood, `(2,7,15,4)` weapons — so the
`industry` argument is the commodity index and each painter reads **its own commodity's own
record**. The two ends agree with no residue.

**Therefore: there is no off-by-one in the original, and no out-of-bounds read.**
`docs/draws-map.md` §5.5's *"every one is the record above the commodity its row is for, and
the wood row's `0x2F0` is one whole record past the end of a four-record array"* is **wrong
in both halves** (and it also attributes `0x2F0` to wood; `0x2F0` is stone's). It is an
artefact of a Ghidra struct base that is 4 bytes low. **Third correction to that document
this week — the pattern the task asked me to name is real.**

---

## 3. The handoff I was given is stale, and this matters

The branch point `ff4a7c5` predates `ebf8dd5` *"Draw the five industry rows, turn the wheels,
and say 'Mining on'"*, which is on `main`. **On `main` today, all four industry deltas are
already drawn** — `crates/l2-game/src/screens/county.rs`, `draw_industry_rows`, the
`strip_delta(ctx, canvas, c.industry[commodity.index()].next_season, 0x22C, y + 0x139)` calls
— and `next_season` is computed by `l2_kingdom::industry` (C136) and carried in the save.

**And they are drawn correctly**, by luck of construction rather than by knowledge: our
`Industry` is a plain Rust struct indexed by commodity, so the Ghidra base error could not
reach the value. I did **not** verify this by running anything — see §5.

So the remaining work is **documentation and comments, not pixels.** Nothing needs reverting;
I drew nothing.

---

## 4. The single next step

Correct the base in the four places that carry the wrong story, and add the `CNEW-industry-base`
entry (already written into `docs/decisions.md` by this commit):

1. `docs/records.json` — `Industry` `"off"` `0x290` → `0x294`; shift all eight field offsets
   down by 4; add the `+0x14 next_season` field; rewrite the entry comment (it currently
   cites the four record bases as evidence for the wrong base); move `weapon_type` out of the
   array as a county field at `+0x290`.
2. `docs/draws-map.md` §5.5 — the "record above" / "past the end" paragraph is false; replace
   it with the span argument above.
3. `crates/l2-kingdom/src/county.rs` ~lines 181–206 — `next_season`'s doc comment, including
   the *"wood `[0]` | `+0x2A8` | iron's head"* table, which is the same error.
4. `crates/l2-game/src/screens/county.rs` ~line 1835 — *"which `docs/records.json` gives to
   `Industry[c + 1]`'s unnamed head word, and the stone row reads `0x2F0`, one whole record
   past the end of a four-record array. Reported, not guessed at."* Delete; it is settled.

A test for the invariant would be worth having: assert against the fixture saves that
county `+0x2F0 … +0x2F3` is a plausible stone forecast rather than a hole, or simply assert
the record-span arithmetic in a doc test. **Not written.**

---

## 5. What I believe but have not checked

* **[I]** That the drawn numbers are right today. I read the call sites; I ran **no tests at
  all** — not `cargo test`, not `cargo check`, nothing. The suite figure at `5338fe7` is
  reported as 2,203 and I have not confirmed it.
* **[I]** That `l2-scenario`'s `INDUSTRY_BASE = 0x290` with its `+0x05/+0x06/+0x07` reads
  lands on the right absolute bytes (`0x295/0x296/0x297`). The arithmetic says yes; the base
  constant should still be moved to `0x294` with the field offsets shifted, so the file stops
  teaching the wrong layout.
* **[I]** That nothing else in the tree reads an `Industry` field as `base + 0x00`. I did not
  grep for it.
* **Not investigated at all:** the `L2.eng` group 220 wording for the four rows (rule 6). The
  code's doc comment quotes *"Wood produced next season"* etc. as literals; whether the rows
  actually draw from the group or from our transcription, I did not check.
