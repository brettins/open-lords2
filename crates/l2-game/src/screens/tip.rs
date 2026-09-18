//! `Tip_Show` (`0x00476DA9`) saves the screen byte and writes `0x27`, and that
//! is the whole of what this screen is: a value of `g_screenId` under which the
//! message pump runs and no per-screen arm does. The window itself is the
//! message scroll's categories `0x05`…`0x09` — [`crate::screens::message`] —
//! and the ladder that decides which tip is [`crate::tip`].
//!
//! The decompilation compares `g_screenId` with `0x27` in three functions —
//! `Msg_Pump`, `Tip_Show` and `FUN_00476E21` — and nowhere else; the one
//! `case 0x27` in the image is `App_WndProc`'s VK_RIGHT. So `Screen_FrameInput`
//! has no arm for it and `Screen_Draw` no painter that a comparison would show.
//!
//! `[I]`: a jump-table dispatch would not appear as a comparison and was not
//! looked for. What the player sees is the screen underneath, still drawn, and
//! the scroll over it.
//!
//! * **`Screen_HandleInput` (`0x004BA9C8`) really does nothing.** Its body is
//!   one `if`/`else if` ladder on `g_screenId` with no `0x27` arm anywhere in
//! its 3,832 bytes, and it ends `return 0;`.
//!
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
//! `FUN_004323FE` (`0x004323FE`) is `Minimap_Click` outside a battle.
//!
//!   **press** — either button — on the minimap raster while a tip is up picks
//!   that county, centres the map on it and drops the byte straight to `0`,
//!   **without** `FUN_00476E21`: the screen the tip was shown over is not put
//!   back and `DAT_004F0358` is not re-armed. [`crate::tip::Tips::unhost`] is
//!   that assignment and [`crate::tip::Tips::restore`] is the other road.
//!
//!   `[V]`

use l2_view::Canvas;

use crate::input::Event;
use crate::screen::{Ctx, Screen, ScreenId, Transition};

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

    fn is_overlay(&self) -> bool {
        true
    }

    /// `FUN_004323FE` → `Minimap_Click` (`0x0043253A`) picks the county under
    /// the pointer and recentres the map on it before the byte is dropped. That
    /// needs the county raster, which belongs to `MapScreen` and is not reachable
    /// from here; `docs/arms.json` `0x0043253A/minimap-click` is that arm, and it
    /// is reproduced on the map's own ladder and not on this path.
    ///
    /// `g_mouseLeftPressed || g_mouseRightPressed`, so the right button does the
    /// *same* job here as the left on the *same* edge — the one place in the
    /// image where it does. This used to answer the left press alone and say the
    /// right half could not be said, our vocabulary having no right press;
    /// [`Event::RightPress`] is that edge now (`g_mouseRightPressed`,
    /// `0x004EABE0`)
    /// [`Event::RightClick`], the **release**, which would fire one edge late.
    ///
    // arm: 0x0042FF10/minimap-under-a-tip left-press
    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let down = match event {
            Event::Click { x, y } | Event::RightPress { x, y } => Some((x, y)),
            _ => None,
        };
        if let Some((x, y)) = down {
            if l2_view::chrome::minimap_hit_area().contains(x, y) {
                ctx.game.tips.unhost();
            }
        }
        Transition::Stay
    }

    fn draw(&mut self, _ctx: &Ctx, _canvas: &mut Canvas) {}
}
