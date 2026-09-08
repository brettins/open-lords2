# Plan — the next stretch

Written to be attacked. The review landed in `docs/plan-review.md` and won on the substance,
so this file has changed. **Revision 2**; the original reasoning is kept below where it
survived, and marked where it did not.

## What the review changed

Its verdict was that the central claim is *half* true and the false half was load-bearing:
five crates do work and nothing is joined up, but **"the gap is integration, not knowledge"
fails for three of the five slice items**. I verified its four sharpest findings myself
rather than take them:

1. **Persistence is the cheapest unblock and I had it last.** `tools/kingdom/savedump.js`
   already reads `lastturn.sav` and prints a complete turn-1 England — fourteen counties
   with ownership, population, happiness, health, neighbours, castles, grain and herd. It
   runs instantly, today. Importing the shipped scenario is not a stretch goal; it is the
   cheapest item on the list and it unblocks real verification.
2. **`crates/l2-kingdom/tests/reproduction.rs` is correction C12 again — a test that cannot
   fail.** It is headed "the reproduction from the shipped save" and never reads the save.
   It hardcodes `OWNED = 4`, gives counties 1–4 to the human realm, and asserts those store
   happiness 72. The save's owner bytes are `5` at index 1, `4` at 4, `1` at 8, `3` at 11
   and `2` at 13: **five owned counties, one per realm, nine unowned**, and the human owns
   county 8 alone. The *rules* reproduce exactly — 72 = 65+5+1+1 is right, and matches real
   stored values. The *scenario* is fiction, and `docs/kingdom.md` §9 carries the same wrong
   count. Verified with `node tools/kingdom/savedump.js county`.
3. **Workstream B is not mechanical.** There was no unit layer in `l2-sim` at all —
   `Battle` is a flat `Vec<Figure>` — and `crates/l2-sim/src/unit.rs` is being written as
   this is revised. Worse for the slice: `docs/battle-ai.md` §6 says **fourteen of the
   seventeen handlers are siege-only**, and the slice excludes sieges. Wiring them is still
   correct work; it is not what unblocks a playable turn.
4. **A structural call I missed.** `BattleRunner` and `Battlefield` — positions, occupancy,
   deployment, the drive loop, all simulation state — live in `l2-view`, behind `winit` and
   `pixels`. Every other crate is dependency-free for determinism and says so at length.
   Moving them into `l2-sim` is a file move today and a spine-wide change once `l2-game`
   depends on them.

It also killed a claim in `method.md` §7: **"the remaining 90% is mostly CRT and glue" is
false.** Ghidra's FID has already named the CRT; below `0x004C0000` there are 2,206
functions, 1,952 unnamed, and 418 of those directly reference `g_counties`, `g_units` or
`g_tiles`. The conclusion — don't go and name them — probably still holds. The reason given
for it did not, and a right answer resting on a wrong reason is one bad day from becoming a
wrong answer.

## Revision 3 — priority correction from the user

**Modding goes behind the game.** Direct instruction, and it corrects a real drift: three of
the last four agents worked on rules and mods while there is still no menu, no screen, no
input and nothing a person could sit down and play. The mod platform is genuinely good —
every kingdom rule is now something a `.toml` can change, proved by tests that run the
season pipeline and get different numbers out — and that is *further ahead than it needed to
be at this stage*.

Nothing is reverted; the work is done and it is sound. But no further mod work is scheduled
unless the framework needs it. The ordering rule from here:

> **Framework first. Then playing the game. Then mods.**

The one exception the instruction allows: if building the application spine *requires*
something from the mod layer — loading the core ruleset at startup, most likely — that is
framework work and it proceeds.

This also reprioritises what was item 4 below. The 14 siege AI handlers, the missile flight
path, `Battlefield_BuildCastle` — all real, none of them on the path to a playable turn.

## Revised order

1. **Import the shipped scenario** from `lastturn.sav`, and **make `reproduction.rs` read
   it.** Turns the project's central kingdom test from self-consistent into a real oracle,
   and hands the slice a real starting position for free.
2. **Move `BattleRunner` and `Battlefield` into `l2-sim`.** Cheap now, structural later.
   **Done** — with `Dir_FromDelta` and the figure's motion, which are simulation state for
   the same reason. `l2-view` is now a reader with no state of its own, and `l2-sim` still
   depends on nothing but `l2-formats` and `l2-net`.
3. **`l2-game`, the application spine** — unchanged in substance, but it now starts from a
   real scenario rather than a synthetic one.
4. **Workstream B**, rescoped: `Tables` threading is real and larger than stated (21 of the
   consts are not fields of `Tables` at all, so threading alone does not make `kingdom.toml`
   take effect). The order handlers are worth doing and are not on the slice's path.
   **B2 is done**, and the review was right that it was not mechanical: it needed the unit
   layer first. `crates/l2-sim/src/runner.rs` raises units the way `Battle_RaiseSide` does,
   dispatches `Battle_UpdateAllUnits` every frame and reforms what it orders, and
   `tests/lockstep.rs` runs the whole thing over a socket. What that bought is the three
   *field* handlers actually driving a battle; the fourteen siege ones are still called by
   nothing, because there is still no castle — see `docs/battle-ai.md` §13.

The original plan follows, unchanged except where the review struck it.

---

## The claim this plan rests on

Five crates work and are tested — battle simulation, kingdom economy, lockstep netcode,
mod overlay, renderer — **and none of them are joined into a game anyone can play.** 711
tests pass and there is no main loop, no input, no screen state machine, no save format,
and no way to get from a county to a battle and back.

If that claim is right, the highest-value work is a vertical slice, and naming more of the
binary is a distraction. If it is wrong, this plan is wrong.

## What "playable vertical slice" means, concretely

The smallest thing that is recognisably the game:

1. Start on the campaign map of a shipped scenario.
2. See counties, select one, see its economy.
3. Set taxes/rations/labour; end the turn; watch the seasons and the economy move.
4. Move an army into a neighbouring county, fight the battle on the real battlefield, get
   the result back into the kingdom.
5. Save, quit, reload, continue.

Deliberately excluded: castle designer, sieges, diplomacy, AI opponents beyond the crudest,
sound, video, multiplayer UI. All exist as subsystems or are known; none is needed to prove
the spine works.

## Workstreams

**A — `l2-game`, the application spine.** A new crate: screen state machine (menu → campaign
→ county → battle → result), mouse and keyboard input, and the campaign-map screen drawing
the real map with selectable counties. This is the integration point and everything else
plugs into it. Highest risk, because it is the one piece with no prior art in the repo.

**B — Make the rules actually take effect.** Two known, stated gaps: `l2-kingdom`'s modules
read module-level `const`s rather than the `Tables` they are handed, so a modded
`kingdom.toml` is loaded, validated, reported — and ignored; and the 17 decompiled battle AI
order handlers are documented but not wired into `l2-sim`, so figures advance and brawl but
no unit ever behaves like the original's. Both are mechanical rather than exploratory, and
both are needed before any comparison against the original means anything.

**C — Persistence.** Our own save format for campaign state, deterministic and versioned.
Reading the original's `lastturn.sav` is a stretch goal — the container arithmetic is
already proven exactly (`267,028 + 16 × 12,800 = 471,828`), so a scenario importer is
plausible, but our own format is what the slice needs.

## Sequencing

B is uncontroversial and mechanical, so it starts immediately and does not wait for the
review. A is the contested one — it is a new crate, a new dependency direction, and the
place a wrong structural call is expensive — so it waits for the critique. C follows A,
because a save format for a state machine that does not exist yet would be guesswork.

## Known risks

- **A new UI crate is where scope goes to die.** The original's interface is enormous.
  Mitigation: the slice list above is a hard boundary, and anything not on it is refused.
- **`Tables` threading touches ~30 function signatures** in a crate other agents are editing.
  Mitigation: do it in one pass, alone, and land it before anything else touches the crate.
- **We still have never compared any output against the original's framebuffer.** Every
  rendering claim is arithmetic over the binary and the shipped art. The slice does not
  depend on that comparison, but it should not be quietly forgotten either.
- **10% of functions are named.** This plan asserts most of the remaining 90% is CRT and
  glue and does not need naming. That assertion is untested and could be wrong in a way that
  bites during integration.
