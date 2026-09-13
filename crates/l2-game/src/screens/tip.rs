//! **`g_screenId` `0x27` — the screen a tip is shown on.**
//!
//! `Tip_Show` (`0x00476DA9`) saves the screen byte and writes `0x27`, and that
//! is the whole of what this screen is: a value of `g_screenId` under which the
//! message pump runs and no per-screen arm does. The window itself is the
//! message scroll's categories `0x05`…`0x09` — [`crate::screens::message`] —
//! and the ladder that decides which tip is [`crate::tip`].
//!
//! # It draws nothing and answers nothing
//!
//! The decompilation compares `g_screenId` with `0x27` in three functions —
//! `Msg_Pump`, `Tip_Show` and `FUN_00476E21` — and nowhere else; the one
//! `case 0x27` in the image is `App_WndProc`'s VK_RIGHT. So `Screen_FrameInput`
//! has no arm for it and `Screen_Draw` no painter that a comparison would show.
//! `[I]`: a jump-table dispatch would not appear as a comparison and was not
//! looked for. What the player sees is the screen underneath, still drawn, and
//! the scroll over it.
//!
//! **Almost every event is consumed.** A click the scroll does not want falls
//! through to here, and here it stops, because the screen underneath is not
//! `g_screenId` any more and its arms do not run.
//!
//! # The two passes this paragraph used to guess at, now read
//!
//! It used to say both were *"not offered … neither has been driven against the
//! original"*. Both have been read now, out of the decompilation, and they go
//! opposite ways:
//!
//! * **`Screen_HandleInput` (`0x004BA9C8`) really does nothing.** Its body is
//!   one `if`/`else if` ladder on `g_screenId` with no `0x27` arm anywhere in
//!   its 3,832 bytes, and it ends `return 0;`. So there is no widget pass on
//!   this screen — not "a table that happens to be empty", but no arm at all.
//!   `[V]`
//! * **`Screen_FrameInput`'s epilogue does run**, and it is this screen's one
//!   live control. Every arm of that function's ladder ends in
//!   `goto LAB_00431F25`, which jumps *over* the epilogue; `0x27` has no arm,
//!   so it falls out of the nest and reaches it:
//!
//!   ```c
//!   if ((g_mouseLeftPressed || g_mouseRightPressed) && g_screenId != 0x12
//!       && FUN_004323FE()) {
//!       if (g_screenId == 0x0F) { Sound_StopOneShot(); FUN_0041438C(); }
//!       if (g_battlePhase == 0) g_screenId = 0;
//!       g_mapRedraw = 2;
//!   }
//!   ```
//!
//!   `FUN_004323FE` (`0x004323FE`) is `Minimap_Click` outside a battle. So a
//!   **press** — either button — on the minimap raster while a tip is up picks
//!   that county, centres the map on it and drops the byte straight to `0`,
//!   **without** `FUN_00476E21`: the screen the tip was shown over is not put
//!   back and `DAT_004F0358` is not re-armed. [`crate::tip::Tips::unhost`] is
//!   that assignment and [`crate::tip::Tips::restore`] is the other road.
//!   `[V]`

use l2_view::Canvas;

use crate::input::Event;
use crate::screen::{Ctx, Screen, ScreenId, Transition};

/// Screen `0x27`. See the module header.
#[derive(Debug, Default)]
pub struct TipScreen;

impl TipScreen {
    pub fn new() -> TipScreen {
        TipScreen
    }
}

impl Screen for TipScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Tip
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Tip".into()
    }

    /// The screen underneath is still painted; `0x27` has no painter.
    fn is_overlay(&self) -> bool {
        true
    }

    /// No per-screen arm for `0x27`, so nothing reaches the screen underneath —
    /// **except the minimap**, which is not a per-screen arm but
    /// `Screen_FrameInput`'s epilogue and runs on every id but `0x12`. See the
    /// module header.
    ///
    /// The press drops the byte: [`crate::tip::Tips::unhost`], so
    /// [`crate::screen::Machine::seat_tip_host`] takes this screen off and — the
    /// delay not being re-armed — the ladder may post the next tip at once.
    ///
    /// **The window stays up, and that is the original.** The epilogue writes
    /// `g_screenId = 0` and nothing else; `Msg_Pump` runs on `0x00` as well as
    /// on `0x27`, so the tip window the player was reading is still there over
    /// the map. Returning [`Transition::Stay`] rather than [`Transition::Pass`]
    /// is what says that: the campaign map underneath is **not** `g_screenId`
    /// while this screen is up and its own ladder must not run — `Map_Click`'s
    /// first guard is `Msg_DismissUnlessQuestion`, which would throw the window
    /// away.
    ///
    /// **Not reproduced, and it is the other half of the epilogue's `if`:**
    /// `FUN_004323FE` → `Minimap_Click` (`0x0043253A`) picks the county under
    /// the pointer and recentres the map on it before the byte is dropped. That
    /// needs the county raster, which belongs to `MapScreen` and is not reachable
    /// from here; `docs/arms.json` `0x0043253A/minimap-click` is that arm, and it
    /// is reproduced on the map's own ladder and not on this path. So a minimap
    /// press under a tip closes the tip's hold and does not yet move the map.
    ///
    /// **The left press only**, exactly as the six other screens that carry this
    /// arm do. The epilogue's guard is `g_mouseLeftPressed || g_mouseRightPressed`
    /// and our vocabulary has no right *press*: [`Event::RightClick`] is the
    /// right **release**, because that is what all fifty-odd of the original's
    /// right-button arms read. The half we cannot say is recorded here rather
    /// than approximated with the release, which would fire on the wrong edge.
    ///
    /// **Only the raster, not the column.** Answering the whole column would
    /// invent five more arms; see `crates/l2-game/src/screens/job.rs` at the
    /// same arm.
    // arm: 0x0042FF10/minimap-under-a-tip left-press
    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        if let Event::Click { x, y } = event {
            if l2_view::chrome::minimap_hit_area().contains(x, y) {
                ctx.game.tips.unhost();
            }
        }
        Transition::Stay
    }

    fn draw(&mut self, _ctx: &Ctx, _canvas: &mut Canvas) {}
}
