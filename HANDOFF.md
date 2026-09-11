# HANDOFF — build while the game is running

**The job, in one line.** Make `cargo build` / `cargo test --workspace` succeed while
`l2-game.exe` is running on Windows, and make the desktop icon always launch the newest
build. Finish, test and document the three files that existed only as uncommitted changes
in the main checkout at `E:/dev/lords2`.

**Branched from `ff4a7c5`, which was 37 commits behind; fast-forwarded to `5338fe7`
before doing anything.** That matters — the starting worktree was stale.

---

## 0. The three files are on this branch now

`crates/l2-game/build.rs` (modified), `tools/run/play.ps1` and `tools/run/play.cmd` (new)
were copied verbatim out of `E:/dev/lords2`'s uncommitted working tree. **Nothing in them
has been edited yet.** They are exactly as the coordinator left them, including the defect
in §3 below.

## 1. The clarified requirement, from the player, so it is not lost

> *"we should be able to build new versions while I have it open, and that the desktop icon
> should always open the newest version."*

Two claims, and they must **both** hold:

1. a build must succeed while the game is running — **always**, not only when the build
   happens to follow a commit;
2. the icon must **always** launch the newest build.

The coordinator's own revision of the brief: the **launcher is the primary mechanism**, not
the fallback. It satisfies both at zero cost — it builds first (so the icon is newest) and
runs a *copy* (so `target/debug/l2-game.exe` is never locked). The `build.rs` rename is the
safety net for somebody who runs the exe directly; `L2_ALWAYS_UNLOCK` is the switch for
somebody who does that and still wants a guaranteed build.

## 2. What was established

### [V] Renaming a running Windows exe succeeds where overwriting it fails

Verified **beyond** the coordinator's `PING.EXE` test — the real path was driven under
cargo, in this worktree:

* A stand-in long-running console process (`PING.EXE -t 127.0.0.1`, copied over
  `target/debug/l2-game.exe`, started hidden — no window, no focus, no input queue) makes
  the file genuinely locked. `[System.IO.File]::OpenWrite` on it returns
  *"The process cannot access the file … because it is being used by another process."*
* With that file locked, `cargo build -p l2-game` **succeeded, exit 0**, after
  `make_room_for_the_link` renamed the running image aside. Cargo printed:

  > `warning: l2-game@0.1.0: l2-game: the game is running, so the old binary was moved to l2-game.old-1789097284052.exe. The build continues; the running copy is unaffected and is swept on a later build.`

* The stand-in process **kept running** out of the renamed image afterwards
  (`Get-Process -Id 6728` still alive), and a fresh 19,444,224-byte `l2-game.exe` was
  linked at the original path beside it. Both files listed side by side.

**So the mechanism itself is proven end to end under cargo.** That is the single most
valuable result here.

### [V] The recompile cost — measured, and it *contradicts* the comment in the file

Measured in this worktree, warm cache, `cargo build -p l2-game`:

| case | times observed |
|---|---|
| no-op build, default | **0.21s, 0.18s, 0.17s, 0.19s** |
| `L2_ALWAYS_UNLOCK=1`, repeated | **2.61s, 2.68s, 2.81s, 2.68s** (and 3.12s under `-v`) |

* The coordinator's brief said **2.7–7s against 0.17s**. The low end and the no-op figure
  reproduce. **The 7s did not** — nothing here exceeded 3.2s. It is plausible that 7s came
  from the main checkout's much larger, colder `target/` (9.5 GB) or from contention.
* **The comment inside `build.rs` says "5 to 7 seconds against 0.19".** The `0.19` is fine.
  **The "5 to 7 seconds" is wrong on this machine** — nothing reached 5s. This is one of the
  numbers that must be corrected or removed (brief item 2, `CLAUDE.md` rules 4 and 6).
* `cargo build -v` with the switch on shows **two `rustc` invocations** (the `l2_game` lib
  and the `l2-game` bin) with every dependency `Fresh`, so *"every build recompiles
  `l2-game`"* is accurate — it is a crate recompile, not merely a relink.

### [V] `target/debug` sizes

`E:/dev/lords2/target/debug` is **9.5 GB** (`du -sh`), so the comment's 9.5 GB is right. A
debug `l2-game.exe` is **19,444,224 bytes ≈ 19 MB**, so *"about 19 MB"* is right and the
arithmetic *"ten stranded copies are two per cent"* follows.

### [V] The default `build.rs` path is **not** sufficient, which is the coordinator's point 1

`build.rs` emits `cargo:rerun-if-changed` for only the git dir's `HEAD` and `index`, and
emitting any `rerun-if-changed` **replaces** cargo's default of *"any file in this package"*.
So an ordinary source edit — the case the player actually means — does not rerun the script,
the rename never happens, and the link fails. Only a build that follows a commit, a
checkout, a rebase or a `git add` is protected.

**A caveat on my own evidence for this, stated honestly.** My first "default" run *did*
rerun the script and succeed — because an earlier `L2_ALWAYS_UNLOCK=1` run had left the
non-existent `build.rs.always-rerun` path stored in the unit's fingerprint, so cargo kept
rerunning the script afterwards. **That run was contaminated and should not be cited as
proof that the default path works.** I was in the middle of flushing that marker with clean
builds when I was stopped. The reasoning above (emitting `rerun-if-changed` replaces the
default set) is cargo's documented behaviour and I believe it, but **the clean
demonstration — locked exe, plain source touch, plain `cargo build`, expect a link failure —
was not completed.** Mark it `[I]`, not `[V]`, until somebody runs it.

**How to run it cleanly** (this is the single next step, see §5):

1. `cargo build -p l2-game` twice with **no** `L2_ALWAYS_UNLOCK` in the environment, to make
   the script re-emit a clean `rerun-if-changed` set and flush the stale marker.
2. Copy `C:\Windows\System32\PING.EXE` over `target/debug/l2-game.exe`; start it hidden with
   `-t 127.0.0.1`; confirm `[System.IO.File]::OpenWrite` throws.
3. Bump the mtime of `crates/l2-game/src/lib.rs` (no commit, no `git add`).
4. `cargo build -p l2-game` — **expect the link to fail**, with no `cargo:warning` from
   `make_room_for_the_link`.
5. `$env:L2_ALWAYS_UNLOCK=1; cargo build -p l2-game` — expect success and the rename warning.

## 3. The defect in `build.rs` that is still there, unedited

`fn main()` carries **two comment blocks that contradict each other.** The first (correct)
explains that `L2_ALWAYS_UNLOCK` is opt-in *because* the forced rerun costs a recompile. The
second, immediately below it, is the leftover of the false claim the coordinator said he had
deleted — **he deleted the sentence but not the block**:

> ```
> // **Always rerun.** … Naming a path that does not exist is how a
> // build script asks to be rerun unconditionally.
> //
> // This is cheap and it is not a rebuild: the script re-emits the same
> // `L2_BUILD_ID`, and cargo compares a build script's output before
> // invalidating anything that depends on it. Measured, not assumed — a
> // second `cargo build` with nothing changed still reports `Finished` with
> // no compilation.
> ```

**Every sentence in that second paragraph is false**, and it is labelled *"Measured, not
assumed"*, which is the worst possible way to be wrong on this project (`CLAUDE.md` rule 4;
`docs/decisions.md` C124 and C149 are both cases where a confident comment *produced* a bug).
It also describes behaviour — *"always rerun"* — that the code above it makes conditional.
**Delete the whole block.** That is the first edit anyone picking this up should make.

## 4. What has NOT been done

* **Nothing in the three files has been edited.** The false block in §3 is still there and
  the "5 to 7 seconds" is still wrong.
* **`docs/environment.md` has not been touched.** It needs a Commands-section entry in that
  file's voice, with the launcher as the primary mechanism.
* **`docs/decisions.md` has no `CNEW-` entry yet.** The interesting content is the
  measurement and the trade, not the feature; `docs/plan.md` §3.1 (*A development affordance
  can be on the critical path, and this one was*) is the section it belongs beside.
* **The launcher (`play.ps1`) was never run.** None of its behaviour is verified.
* **The full suite was not run.** The brief's baseline is 2,203 passing at `5338fe7` with
  `LORDS2_FIXTURES="E:\dev\lords2-fixtures"`.
* **The sweep-test question was not answered.**

## 5. What I was doing at this exact moment, and the single next step

Flushing the stale `build.rs.always-rerun` fingerprint marker so I could run the clean
"default path fails" demonstration in §2's last block.

**Single next step: delete the false comment block in §3**, then run the five-step
demonstration in §2.

## 6. Believed but NOT checked — treat all of these as `[I]`

* **`play.ps1` passes launcher arguments through with `@args` while declaring
  `[CmdletBinding()]`.** I believe this is **broken**: in a PowerShell *advanced* script,
  `$args` is unavailable and an unbound argument is an error
  (*"A positional parameter cannot be found that accepts argument"*). If so,
  `play.cmd <game dir>` — the normal way anyone would launch the game, since `l2-game`
  *requires* a game directory argument — fails outright. **Untested. Check this first; it
  may mean the launcher has never actually launched anything.** The fix is a
  `[Parameter(ValueFromRemainingArguments=$true)] [string[]]$GameArgs` parameter.
* **`l2-game -h` / `--help` is a console-only, immediately-exiting path** (`usage()` then
  `std::process::exit(2)`, `crates/l2-game/src/main.rs:418`). I intended to use it to drive
  the launcher end to end **without opening a window**. Read from the source, not run.
* **"Always the newest" is provable by hash**, not by output: after `play.ps1` runs, the
  copy it launched should be byte-identical to `target/<profile>/l2-game.exe`. Make a real
  source change, run the launcher again, and require the new copy to differ from the old and
  match the new build output. Designed, not run.
* **`OUT_DIR.ancestors().nth(3)`** in `make_room_for_the_link` resolves to the profile
  directory. I reasoned it through (`<target>/<profile>/build/<pkg>-<hash>/out`) and the
  rename in §2 landed in `target/debug`, so this is very likely right — but it was only
  exercised for the default `debug` profile with no explicit `--target`.
* **On the sweep test, my leaning was: no test, because the shape already cannot be wrong**
  (`docs/agents.md`, *Prefer a shape that cannot be wrong to a check that notices when it
  is*). The sweep deletes every `l2-game.old-*.exe` and a running image simply refuses,
  which is the desired outcome rather than an error; there is no count-based policy to get
  wrong. **This is a leaning, not a decision, and it was not written up.**

## 7. The desktop-shortcut line — UNTESTED

The coordinator asked for the exact one-line instruction for re-pointing an existing
shortcut, and to report it verbatim. **I never tested this.** What I believe it should be:

* **Target:** `E:\dev\lords2\tools\run\play.cmd "F:\games\Lords of the Realm II"`
* **Start in:** `E:\dev\lords2`

The `Start in` is believed **not** strictly required — `play.ps1` derives the repo root from
`$PSScriptRoot`, two levels up from `tools\run` — but setting it costs nothing and makes the
`cargo` invocation's working directory unambiguous. **And see §6: if the `@args` defect is
real, the game-directory argument does not reach the exe at all and this line does not
work.** Do not hand it to the player until the launcher has actually been run.

**An existing shortcut aimed straight at `target\debug\l2-game.exe` is unaffected either
way.** The binary is still built at the same path under the same name — which is precisely
why `build.rs` renames the *stale* file rather than versioning the new one. That is the
point `docs/environment.md` must state plainly.

## 8. Housekeeping left behind in `target/` (disposable, not committed)

`target/debug/l2-game.exe` may be a `PING.EXE` stand-in rather than the real binary, and
`target/debug/l2-game.old-1789097284052.exe` is a stranded 45,056-byte copy of it. A hidden
`PING.EXE` process (pid 6728 in this session) may still be running; kill it and rebuild.
Nothing under `target/` is committed.
