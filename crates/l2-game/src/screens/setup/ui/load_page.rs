use super::*;
use crate::screens::saveload::SaveLoadScreen;

#[cfg(test)]
#[path = "load_page_tests.rs"]
mod tests;

impl SetupScreen {
    /// Page 3's box, opened by FUN_004335F0 hotspot 4 (`Files_Enumerate`, `Edit_Begin`). [V]
    pub(crate) fn load_box(&mut self) -> &mut SaveLoadScreen {
        self.load.get_or_insert_with(SaveLoadScreen::front_end)
    }

    /// Screen_HandleInput 0x004BA9C8, 004b0000.c:7355-7362: page 3 edits DAT_004EA130, runs
    /// FUN_004AD9F0, then `Widget_Test(0x60, 10, g_saveLoadWidgets, 4)`. [V]
    pub(crate) fn load_event(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // Screen_FrameInput 0x0042FF10, 00420000.c:7461-7466: `if (FUN_0043400b(1) != 0)` skips
        // the right release, else `g_setupPage = DAT_00553E5C`. [V]
        if let Event::RightClick { .. } = event {
            if self.load_list_hit {
                return Transition::Stay;
            }
            return self.load_closed(Transition::Pop);
        }
        // FUN_0043400B 0x0043400B: nonzero on a left press over a listed file (`< DAT_004eb25c`).
        // [I] our frame is the events before one `update`.
        if let Event::Click { x, y } = event {
            self.load_list_hit |= self.load_box().at(x, y).is_some();
        }
        let t = self.load_box().handle(event, ctx);
        self.load_closed(t)
    }

    pub(crate) fn load_update(&mut self, ctx: &mut Ctx) -> Transition {
        self.load_list_hit = false;
        let t = self.load_box().update(ctx);
        self.load_closed(t)
    }

    /// SaveLoad_Cancel 0x00434308 (00430000.c:1440): `if (g_screenId == 0x1F) g_setupPage =
    /// DAT_00553E5C`; FUN_004AD9F0 :5735 `Setup_StartGame()` after a read. [V]
    fn load_closed(&mut self, t: Transition) -> Transition {
        match t {
            Transition::Stay => Transition::Stay,
            Transition::Pop => {
                self.load = None;
                self.go(self.load_opener)
            }
            t => {
                self.load = None;
                self.go(self.load_opener);
                t
            }
        }
    }

    pub(crate) fn draw_load(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        self.load_box().draw_status(ctx, canvas);
    }

    /// DAT_004EA130 as page 3 shows it.
    pub fn load_name(&self) -> String {
        self.load.as_ref().map(|l| l.name()).unwrap_or_default()
    }
}
