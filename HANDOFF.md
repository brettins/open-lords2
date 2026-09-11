# HANDOFF — "the campaign map's top-right is still a placeholder font"

**Branch** `worktree-agent-a410cf5ede1d9c7ff`, branched from `main` at `5338fe7`; `main`
had not moved, so there was nothing to fast-forward.

**The job, in one line.** Establish which of the game's three faces the campaign map's
gold and season, the build stamp and the title screen each take — from the call site, not
by guess — then fix whichever are still wrong.

**No code was changed.** Nothing needed to be. The working tree at the moment of the stop
was clean; this file and a `docs/decisions.md` entry are the whole of the commit.

---

## 1. The headline: all four were already fixed on `main`, and I looked at them

`fonts-chrome-title` landed as `36abe2b`, merged as `c06b13b`; `git merge-base
--is-ancestor` confirms both are ancestors of `5338fe7`. **[V]**

`LORDS2_DIR="F:/games/Lords of the Realm II" LORDS2_FIXTURES="E:/dev/lords2-fixtures"
cargo test -p l2-game --test chrome_text` — **7 passed, 0 failed, 1 ignored.** Then the
ignored `shoot` was run with `L2_SHOT_DIR` pointed at the scratchpad and **every one of
the four renders was read by eye**, cropped and upscaled 4×:

| element | what the render shows |
|---|---|
| the treasury | `1000 Crowns.` in `Fntl2_14.pl8` blackletter, at the bar's right |
| the season | `1268  Winter` in `Fntl2_14.pl8`, year first, season after |
| the build stamp | `BUILD 5338FE756 2026-09-10` in `Fntl2_9.pl8`, plain, fully on screen, legible |
| the title | `Lords of the Realm 2` in `Fntl2_22.pl8` with the red drop capitals, `"The siege is on"` under it |

The menu bar's *File / Options / Help* are in `Fntl2_14.pl8` too. The only 5 × 7 debug
text left on the campaign map is `TURN n` / `COUNTIES n/m` at (6, 28) and (6, 38), which
is **ours on purpose** and marked as such in `draw_menu_bar`'s comment.

So the player's three reports are **stale — they pre-date `c06b13b`.** This is the third
time this project has spent an agent explaining a bug that was already fixed, which is the
exact thing `crates/l2-game/src/build_id.rs`'s header says the stamp exists to stop.
**Route the next report through the build id before routing it to an agent.**

## 2. Which face each element takes — the expensive part, verified from the decompilation

Read out of `E:\dev\lords2\tools\oracle\decomp\` (the corpus lives in the **main**
checkout, not in a worktree — `tools/oracle/decomp/` here is empty, and re-running
`decompile-all.ps1` to get it would be a waste). All **[V]**.

**`Screen_DrawMenuBar` (`0x00419C78`)**, `decomp/00410000.c:3635`, the tail verbatim:

```c
Ui_DrawMenuTitles((short *)&g_menuBarItems, 3);
if (g_battlePhase == 0) {
    g_penAdvance = 0;
    Ui_DrawYear(g_year, 0x168, 6, 3);
    Eng_DrawString(0x1d, g_season, g_penAdvance + 0x16c, 6, &g_fontBody, 0x3f);
}
Ui_DrawCount(g_realms[g_localPlayer].gold, 0, 500, 6, &g_fontBody, 0x3f);
```

* **the season** — `&g_fontBody`, named at that call site. `L2.eng` group `0x1D` = 29,
  indexed by `g_season` unadjusted.
* **the gold** — `&g_fontBody`, named at that call site, and it is the font argument
  `Ui_DrawCount` forwards to **both** the number and the noun.
* **the year** — no font at the call site; it is inside `Ui_DrawYear` (`0x0041A900`,
  `decomp/00410000.c:3848`). Style 3 is
  `Ui_DrawNumber(year, ' ', &DAT_004D41F0, x, y, &g_fontBody, 0x3f)` — **`g_fontBody`**.
  Worth knowing: **style 1 is the only one that uses `&g_fontHeading`**, so the year's face
  is a function of the style byte and 3 is the bar's.
* `g_fontBody` = `Fntl2_14.pl8` — `docs/symbols.md` `0x005AF8F0`, and independently the
  preload table at `0x004D9F84` whose `fntl2_14.pl8` size `0x36B0` equals
  `g_fontHeading (0x5B2FA0) − g_fontBody (0x5AF8F0)`.

**The build stamp** has **no original function** — it is ours, and rule 5's *"we could not
find it"* applies honestly: the game has no build id. `crates/l2-game/src/build_id.rs`
picks `Fntl2_9.pl8` (`ShellAssets::small`) deliberately, because **only `Fntl2_9` is a
plain text face**; 14 and 22 are blackletter display faces. Seven hex characters in
blackletter is an ornament, not an identifier. Its `y` is computed from the font's own
height, not a literal, which is what stopped it hanging off the bottom.

**The title** is `setup.rs` page 1, painted by **`FUN_0041EA14`** — `L2.eng` 11/0 in
`&g_fontHeading` (`Fntl2_22.pl8`) at `y = 0x1E`, embossed, with `DAT_0058FE2C`'s drop
capitals in colour 1. `menu.rs`'s title screen is unreachable dead code.

**What we were using before `c06b13b`:** all of the gold, the season and the year went
through `l2_view::text::draw`, our 5 × 7 debug font; the treasury drew the word `GOLD`,
which is in no `.eng` group; and the build stamp was drawn in `Fntl2_14.pl8`, the
blackletter, which is the illegibility the player reported.

## 3. What is left, and it is real — the treasury's digits are 4 pixels left

This was the previous agent's open item 1, deferred on a **mistaken mechanism**. I verified
the mechanism and the item stands, but for a different reason than was written down.

**`Ui_DrawCount` (`0x0041AB67`, `decomp/00410000.c:3900`) hard-codes its lead and its
suffix:**

```c
Ui_DrawNumber(value, '@', &DAT_004D41F4, x, y, font, colour);
```

`DAT_004D41F4` is a **NUL** — read straight out of `F:\games\Lords of the Realm II\Lords2.exe`
at VA `0x004D41F4`, file offset `0xD23F4`: the eight bytes from `0x004D41F0` are
`20 00 00 00  00 00 00 00`. So `Ui_DrawYear` style 3's suffix (`0x004D41F0`) is **one
space** and `Ui_DrawCount`'s (`0x004D41F4`) is the **empty string**. **[V]**

**`'@'` advances 4 pixels when drawn, and 0 when measured.** This is the bit the tree has
wrong. `Ui_DrawText` (`0x00402637`) never reaches `Glyph_Draw` for a glyph-less character:

```c
local_c = local_c - 0x20;
if ((&g_glyphWidths)[local_c] == '\0') { local_14 = 4; }   /* <- hard-coded, not Glyph_Draw */
...
g_drawX = g_drawX + local_14;  g_penAdvance = g_penAdvance + local_14;
```

whereas the measure `FUN_004014F0` (`0x004014F0`) special-cases only `0x20` and adds
**nothing** for any other zero entry. `Glyph_Draw` itself returns 0 for a zero entry, which
is true and irrelevant — it is not called. `crates/l2-game/src/shell/font.rs`'s
`SPACE_ADVANCE` doc comment states the mechanism as *"`Glyph_Draw` adds nothing at all for
a zero entry, which is what makes `'@'` an invisible sign column that still occupies its
place"*, which is self-contradictory and names the wrong function. The conclusion it
reaches is right; the reason is wrong. **[V]**

**Therefore**, with gold = 1000 at x = 500:

* original: buffer `"@1000"`, digits begin at **504**, noun at `500 + 4 + W + 4`;
* ours: `Pen::number(blank_lead = true)` maps the lead to the **empty string**
  (`crates/l2-game/src/shell/mod.rs:876`) and hard-codes a trailing `" "`, so the digits
  begin at **500** and the noun lands at `500 + W + 4 + 4` — the **same** place. The two
  errors cancel at the noun and do not at the digits.

That is `docs/decisions.md` C127 — *every number in the game reserves a sign column, and we
were dropping it* — unfixed for the treasury. Four pixels, on the readout the brief names.

**And a second finding, cleaner than the sweep.** `Pen::count`'s `blank_lead: bool`
parameter is **spurious**: `Ui_DrawCount` has no such choice, it always passes `'@'` and
always passes the empty suffix. Every `Pen::count` call site that passes `false` — three of
them in `crates/l2-game/src/screens/info.rs` (lines 852, 854, 860) — is drawing a lead the
original does not draw, and puts its noun 4 pixels right of where the original puts it.

### The next step, exactly

1. Add a private `Pen::number_with(canvas, x, y, value, lead: &str, suffix: &str, colour)`
   in `crates/l2-game/src/shell/mod.rs`; have `Pen::count` call it with `("@", "")` and
   **drop `count`'s `blank_lead` parameter** (update its ~20 call sites — the `false` ones
   in `info.rs` are the behaviour change).
2. **Leave `Pen::number` alone.** Making `blank_lead: true` emit `"@"` there would move
   ~30 numbers across nine screens 4 pixels right, and their suffixes are *also* the call
   site's rather than the hard-coded `" "`. That is the rest of C127's 352-site sweep and
   it needs its own task; doing half of it silently is worse than doing none.
3. `crates/l2-view/src/text.rs:102` gives `'@'` a **real at-sign bitmap**. The fallback
   font would print a literal `@` in the treasury on an install with no `Fntl2_*.pl8`.
   Map `'@'` to `BLANK` (the const is already there at line 64) and say why in the comment:
   it is the game's blank sign column, not an at-sign. **This is a prerequisite for step 1,
   not an optional tidy.**
4. Correct `font.rs`'s `SPACE_ADVANCE` doc to name `Ui_DrawText`'s `local_14 = 4` branch
   rather than `Glyph_Draw`.

### The tests I had planned, with their ablations

* **install-gated, `crates/l2-game/tests/chrome_text.rs`:** the treasury's *digits* are
  found by `find_in(canvas, body, &gold.to_string(), font::TEXT)` at **x = 504**, which is
  `GOLD_X + font::SPACE_ADVANCE`. Assert the coordinate, **not** a diff count — the brief's
  trap, and it applies here exactly, because adding the lead changes the drawn string's
  width as well as its position. Ablation: revert the lead to `""` → the digits land at 500
  → red.
* **the noun does not move:** `"Crowns."` is found at `GOLD_X + body.width("@1000") +
  shell::TRAILING`. This is the guard on the compensation; without it a fix to the lead can
  silently push the noun 4 right. Ablation: restore the trailing `" "` in `count` → red.
* **no install needed, `crates/l2-view/src/text.rs`:** `glyph('@')` is all-zero and
  `width("@")` is unchanged at `ADVANCE`. Ablation: restore the at-sign bitmap → red.

## 4. What I tried that did not work

* `Add-Type System.Drawing` in a PowerShell 5.1 `param()` script to crop and upscale the
  renders: the `[int]` parameters arrived as `Object[]` and every arithmetic line threw
  `op_Multiply`. Do not spend time on it. A ~80-line Node script using the built-in `zlib`
  to inflate the IDAT and nearest-neighbour upscale worked first try; it lives in the
  session scratchpad and is trivial to rewrite.
* Nothing else failed. Ghidra was **not** needed and was not started — every function in
  section 2 came out of the existing `tools/oracle/decomp/` corpus in the main checkout by
  `grep`, in seconds.

## 5. What I believe and have not checked

* **[I]** That the player's reports pre-date `c06b13b`. I verified the renders are correct
  at `5338fe7`; I did not see the build id on the player's screenshot, so I cannot prove
  which build he was on. The stamp exists precisely so this can be checked — ask for it.
* **[I]** That every `Pen::number(blank_lead: true)` call site corresponds to an original
  `Ui_DrawNumber(..., '@', ...)`. The flag was presumably transcribed from the call sites,
  but I read only the treasury's. Do not run the sweep in step 2 until they have been read.
* **[I]** That `Font::width`'s charging `SPACE_ADVANCE` for `'@'` (where `FUN_004014F0`
  charges 0) is harmless. It only matters for a *centred* draw containing `'@'`, and all
  twenty `Ui_DrawNumberRight` call sites pass `' '`. Believed, not measured.
