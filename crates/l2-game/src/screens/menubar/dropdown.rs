#![allow(unused_imports)]
use super::*;
use super::items::*;
use super::render::*;
use super::tests_part::*;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::confirm;
use crate::screens::options::Page;
use crate::screens::saveload::Mode;
use crate::shell::font;

pub struct DropdownScreen {
    /// `DAT_00522CB4`, but zero-based: which of the three titles is down.
    menu: usize,
    /// `DAT_00522CB0`, zero-based: the item under the pointer, if any.
    hover: Option<usize>,
    ///.
    status: String,
}

impl DropdownScreen {
    pub fn new(menu: usize) -> DropdownScreen {
        DropdownScreen { menu: menu.min(MENUS.len() - 1), hover: None, status: String::new() }
    }

    pub fn menu(&self) -> usize {
        self.menu
    }

    pub fn hover(&self) -> Option<usize> {
        self.hover
    }

    fn run(&mut self, ctx: &mut Ctx, item: Item) -> Transition {
        match item {
            // `Menu_NewGame`: `Ui_OpenConfirm(1, …)` — group 10 index 1,
            // *"Start a new game?"*, on screen 0x1E.
            //
            // arm: 0x0040DECA/file-new-game left-press
            Item::NewGame => Transition::Replace(ScreenId::Confirm(confirm::Ask::NewGame)),
            // arm: 0x0040DECA/file-load left-press
            Item::Load => Transition::Replace(ScreenId::SaveLoad(Mode::Load)),
            // arm: 0x0040DECA/file-save left-press
            Item::Save => Transition::Replace(ScreenId::SaveLoad(Mode::Save)),
            // `Menu_Quit` (`0x004343F8`): `Ui_OpenConfirm(0, 0xA0, 0xA0,
            // FUN_0043441C)`, *"Exit the game?"* — group 10 index 0, on screen
            // 0x1E. The quit is the box's yes, not this item.
            //
            // arm: 0x0040DECA/file-quit left-press
            Item::Quit => Transition::Replace(ScreenId::Confirm(confirm::Ask::Quit)),
            // arm: 0x0040DECA/options-and-help-pages left-press
            Item::Options(page) => Transition::Replace(ScreenId::Options(page)),
            // arm: 0x0040DECA/options-sliders left-press
            Item::Slider(what) => {
                self.status = format!("{what} IS ON THE ADVANCED PAGE - NO SPINNER (SCREEN 0x21)");
                Transition::Stay
            }
            // `Menu_HelpHowDoI` (`0x0043480C`) and its four siblings, read
            // whole: `Msg_Enqueue(0, g_localPlayer, 0x123, 0, 0x13, 0, 0, 0);
            // g_screenId = g_menuPrevScreen;`. Five consecutive ids, 0x123 …
            // 0x127, all category `0x13` — [`crate::message::category::HELP`],
            // whose window geometry is `g_helpWindowGeom` (`0x004D6EB8`). The
            // topic goes on the ring and the menu closes behind it.
            //
            // arm: 0x0040DECA/help-topics left-press
            Item::Help(id) => {
                let player = ctx.game.player;
                ctx.game.messages.enqueue(
                    crate::message::Record {
                        to: player,
                        from: 0,
                        group: id as u16,
                        variant: 0,
                        category: crate::message::category::HELP,
                        county: 0,
                        spare: 0,
                        payload: 0,
                    },
                    player,
                );
                Transition::Pop
            }
            // arm: 0x0040DECA/help-about left-press
            Item::About => Transition::Replace(ScreenId::About),
        }
    }

    /// `FUN_0040E099` against the pointer, for the menu that is open.
    fn item_at(&self, ctx: &Ctx, x: i32, y: i32) -> Option<usize> {
        let t = titles(ctx);
        (0..MENUS[self.menu].items.len()).find(|&i| item_rect(&t, self.menu, i).contains(x, y))
    }
}

impl Screen for DropdownScreen {
    fn id(&self) -> ScreenId {
        ScreenId::MenuBar(self.menu)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("Menu {} - screen 0x32", MENUS[self.menu].fallback)
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // **`FUN_0040DD92`'s button-up half, and it is the only hover arm in
            // the management interface.** Sliding onto a different title with
            // the button up switches the open menu; then the item under the
            // pointer is recomputed. Both are the same call, in that order.
            Event::Pointer { x, y } => {
                // arm: 0x0040DD92/slide-between-titles hover
                if let Some(t) = title_at(&*ctx, x, y) {
                    if t != self.menu {
                        self.menu = t;
                        self.status.clear();
                    }
                }
                // arm: 0x0040DD92/hover-item hover
                self.hover = self.item_at(&*ctx, x, y);
            }
            // The button-down half: an item under the pointer runs, anything
            // else closes. `FUN_0040DF62` is the close and it is unconditional
            // — a press on the title that opened the menu closes it too, which
            // is what makes the bar feel like a menu bar.
            Event::Click { x, y } => {
                self.hover = self.item_at(&*ctx, x, y);
                match self.hover {
                    // arm: 0x0040DD92/pick-item left-press
                    Some(i) => {
                        let item = MENUS[self.menu].items[i].2;
                        let t = self.run(ctx, item);
                        return match t {
                            Transition::Stay => Transition::Stay,
                            other => other,
                        };
                    }
                    // arm: 0x0040DF62/close-on-miss left-press
                    None => return Transition::Pop,
                }
            }
            // `if (FUN_0040DD92(...) == 0 && g_mouseRightReleased) FUN_0040DF62();`
            // arm: 0x0042FF10/dropdown-right-close right-release
            Event::RightClick { .. } => return Transition::Pop,
            // arm: ours/dropdown-escape-closes key
            Event::KeyDown(Key::Escape) => return Transition::Pop,
            _ => {}
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let ink = &ctx.assets.ink;
        let t = titles(ctx);
        let pen = crate::shell::Pen {
            assets: &ctx.assets.shell,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            // `FUN_0040C725` touches neither `DAT_005AEA40` nor `DAT_0058FE2C`
            // around the captions, so this is the ordinary embossed body pen.
            shadow: Some(font::SHADOW),
            caps: None,
        };

        // `FUN_00409429(x, y + 0x12, 0x0C, (count * 0x15) / 16 + 2)` — the
        // original's own plate. It used to be a `widget::panel` of ours, on the
        // belief that the original drew the captions onto the bare map.
        let plate = plate_rect(&t, self.menu);
        pen.window(
            canvas,
            plate.x,
            plate.y,
            PLATE_COLS,
            plate.h / PLATE_CELL,
            PLATE_SET,
        );

        for i in 0..MENUS[self.menu].items.len() {
            let picked = self.hover == Some(i);
            if picked {
                canvas.fill_rect(
                    plate.x + HIGHLIGHT_DX,
                    BAR_Y + MENUS[self.menu].items[i].0 + HIGHLIGHT_DY,
                    HIGHLIGHT_W,
                    HIGHLIGHT_H,
                    font::TEXT,
                );
            }
            let caption = item_text(ctx, self.menu, i);
            let colour = if picked { PICKED_INK } else { font::TEXT };
            pen.body(
                canvas,
                plate.x + CAPTION_DX,
                BAR_Y + MENUS[self.menu].items[i].0 + CAPTION_DY,
                &caption,
                colour,
            );
        }

        if ctx.game.prefs.debug_overlay && !self.status.is_empty() {
            text::draw(canvas, plate.x, plate.y + plate.h + 4, &self.status, ink.bad);
        }
    }
}

