# Plan — the next stretch

Written to be attacked. `docs/method.md` §7 argues the current gap is **integration, not
knowledge**, and this is the plan that follows from it. An adversarial review of this
document is commissioned alongside it; where the review wins, this file changes.

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
