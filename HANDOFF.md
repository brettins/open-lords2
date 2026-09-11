# HANDOFF — comparing *where* a screen draws, not only how many times

Branch `worktree-agent-add2587f15d580aa2`, branched from `ff4a7c5`, fast-forwarded to
`main` at `5338fe7`. **Unverified WIP.** Killed mid-task by a machine restart.

## The job, in one line

`tools/draws/screendraws.js` pairs our draw call sites with the original's and calls the
pair *reproduced*; it has never compared a coordinate. Build the instrument that does,
measure it honestly, then fix what it finds.

## What exists on this branch

* `tools/draws/screendraws.js` — `callsIn` refactored from a line-by-line scan to a
  document-wide one so that each call site now carries its **argument list** (`args`),
  with `balanced()` and `splitArgs()` exported. **[V]** The refactor does not move any
  count: `node tools/draws/screendraws.js` still prints **1012 / 517** and `--check`
  still reports *"51 screens, every stored count matches the corpus"*, both re-run after
  the change.
* `tools/draws/drawcoords.js` — **new, the instrument.** Runs. Not yet wired to any test,
  and **nothing has been fixed**.

## The finding, and it is the thing worth keeping

> **Most reproduced draws cannot be compared by coordinate, and the honest number is
> that we could pair 77 of them.** Of those 77, **66 agree exactly (86%)** and 11 do not.

Measured output (`node tools/draws/drawcoords.js`, at this commit) **[V]**:

```
44 screens over 22 modules
415 of the original's leaf call sites carry a literal (x, y);  515 do not
171 of ours carry one;                                         895 do not
 77 pairs could be compared      ->  66 exact, 11 near misses
338 original points had nothing of ours within 24 px
 94 of our points had nothing of the original's near them
```

**So the draw figure means less than we have been quoting it as meaning.** `screendraws.js`
says 517 of 1012 call sites are reproduced; this says that for **77** of them do we have
any evidence at all that the thing is in the right place.

### What defeats extraction, in order of how much it costs

1. **A computed coordinate — by far the largest cause.** `Ui_DrawCount(taxShown, 0,
   g_penAdvance + 0x60, 200, …)`: the y is literal, the x depends on how wide the
   previous string came out. Ours is the same shape (`pen.count(canvas, x, 200, …)` with
   `x` from the previous draw's return). Both sides are right and neither is a number.
   **[V]** — this is what `blind: 'computed'` counts, and it is most of the 515 + 895.
2. **A primitive whose signature does not name a coordinate.** `Sprite_WGenSprite(int a,
   int b, int c)` — three unnamed integers in `docs/symbols.json`. The `FUN_*` leaves in
   `screendraws.js`'s `LEAF` set have no signature at all, so every one of their call
   sites is uncomparable. `info.rs` is the extreme case: **172 blind against 2 points**,
   because the map information panel's eleven layouts are drawn almost entirely through
   unnamed leaves. **[V]**
3. **Our own helpers.** `delta_row(&pen, canvas, y, label, value)` fixes x inside itself.
   `drawcoords.js` does **one level** of inlining with parameter substitution
   (`inlineHelpers`), which is what recovers `county.rs` at all. Two levels would be a
   Rust interpreter. **[V]** that one level works; **[I]** that a second level would pay.
4. **The campaign map is not in this at all.** Its coordinates are tile arithmetic. The
   mercenary marker that was ten pixels out and on the wrong tile (C150) is **not**
   findable by this instrument, and that is the most important sentence in this file.

## The disagreements I actually observed — and read them before believing them

All eleven, at this commit:

```
24 px  county.rs  orig(88,240) Ui_DrawYear   <- Panel_Happiness   ours(96,256) body :2272
20 px  county.rs  orig(48,314) Eng_DrawString<- Panel_Population  ours(62,320) misc_frame :1158
19 px  setup.rs   orig(15,60)  Ui_DrawText   <- FUN_00420147      ours(0,64)  eng_heading_centred :1515
18 px  county.rs  orig(48,362) Eng_DrawString<- Panel_Population  ours(38,370) misc_frame :1158
16 px  county.rs  orig(88,240) Ui_DrawYear   <- Panel_Population  ours(96,232) body :2264
12 px  county.rs  orig(16,48)  Ui_DrawBox    <- Panel_Population  ours(20,56) heading :2186
12 px  county.rs  orig(16,48)  Ui_DrawBox    <- Panel_Happiness   ours(20,56) heading :2225
10 px  merchant.rs orig(96,256) Eng_DrawString<- Trade_DrawPanel  ours(96,246) body_wrapped :1007
10 px  setup.rs   orig(160,0)  Pl8_DrawFrame <- FUN_0041f6c7      ours(160,10) window_from :1553
 8 px  county.rs  orig(128,96) Ui_DrawBox    <- Panel_Ration      ours(128,104) heading_centred :2277
 8 px  setup.rs   orig(0,0)    Pl8_DrawFrame <- FUN_00420630      ours(4,4)   draw :488
```

**[I] — and this is the belief I have not checked, so treat it as the next thing to do.**
I think **most or all of these eleven are mispairings, not defects.** `Ui_DrawBox(16,48)`
paired against our `heading(20,56)` is a window frame matched to a caption: they are
different things that happen to be within 24 px of each other. The matcher pairs by
proximity alone and has no idea what a draw *is*.

**The sharp version, and it is the real result:** the defect class this was built to catch
— C140's two pixels, the group-87 caption's four — sits at **d = 2 to 4**, and there are
**zero pairs at d ≤ 4**. Either the four known placement defects were all already fixed
before this ran (they were, on `main`), or the instrument cannot see that class. I believe
the former **[I]** but did not get to prove it, and proving it is the single next step:

> **Next step: ablate.** Edit one coordinate in `county.rs` — move the ration panel's
> `heading_centred` x from 128 to 126 — and confirm `drawcoords.js` reports a `d=2` pair
> naming that line. A tool whose failure has never been read is a tool whose message is
> untested, and this one has never been observed firing on a real defect. **Do this
> before trusting any number above.** If it does not go red, the 86% is a statement about
> the tree and not about the instrument (`docs/agents.md`, *ablate the line*).

Then: raise the bar on pairing. Proximity alone is too weak. Pair only within a *kind*
class (a box to a box, a string to a string, a sheet frame to a sheet frame) — the
mapping already exists as `OURS` in `screendraws.js`, which names each of our methods
against the original it stands for. That should collapse most of the eleven and leave a
denominator that means something.

## What I would not trust

* **`895 of our 1066 have no literal (x, y)`** is inflated. `OURS_POS` is harvested from
  every `fn` with `x`/`y` parameters in `shell/mod.rs`, `chrome.rs`, `text.rs` and
  `widget.rs`, and that sweeps in non-draws (`merchant_grid(x, y)`, hit tests). Our
  blind count is therefore an over-count of unknown size. The **original's** side
  (`415` / `515`) is clean, because its leaf set is the one `screendraws.js` pins by name.
* `ALLOWED` in `drawcoords.js` is empty and `--check` is written but never run to
  completion. No test references the tool. `figures.js` knows nothing about it.
* No number here is in `tools/figures/figures.js` yet. **Do not type any of these into a
  document** — generate them, per the project rule.

## Not done

Nothing was fixed. No correction was written (no `CNEW-` placeholder was taken, because
the one finding worth recording is the instrument's own result and it needs the ablation
above before it is a finding rather than a draft). No test was added. The suite was not
run.
