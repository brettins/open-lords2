//! `Ui_OpenConfirm` (`0x0040E6F2`) is four arguments — prompt index, `x`, `y`,
//! callback — and its body is `g_confirmPrompt = prompt; g_confirmX/Y = …;
//! g_confirmCallback = cb; g_screenId = 0x1E;` before `Screen_ConfirmBox`
//! (`0x0040CCFA`) paints. A screen of its own, so nothing under it runs: that
//! is why this is a screen here too, and not a flag on the map.
//!
//! * `Menu_Quit` (`0x004343F8`) — `Ui_OpenConfirm(0, 0xA0, 0xA0,
//!   FUN_0043441C)`, group 10 index 0, *"Exit the game?"*.
//!
//! * `Menu_NewGame` (`0x00433DBD`) — `Ui_OpenConfirm(1, 0xA0, 0xA0,
//!   FUN_00433DEB)`, group 10 index 1, *"Start a new game?"*.
//!
//! **What we do not build.** `FUN_0043441C`'s yes, read whole: a network game
//! is `Net_LeaveGameChecked(); g_screenId = 0x1F; g_setupPage = 1;` — out to
//! the front end. A single-player game counts `DAT_0053F644` up to three and,
//! for each of the first three exits, shows `lom.256` on screen `0x45`
//! (`FUN_004AF767`) before `g_quitRequest = 1`; the fourth is `g_quitRequest =
//! 3` with no picture. **Screen `0x45` and its counter are not built** — ours
//! quits on the yes, which is `g_quitRequest` without the send-off. Separately,
//! `ExitConfirm_Draw` (`0x00414790`) draws a second, three-line box — `L2.eng`
//! group 48, *"Exit the game?"* / *"Save the game and exit ?"* / *"Return to
//! the game ?"*, a `0x12 × 9` window at `(0x80, 0xA0)` with its lines at
//! `x 0x90`, `y 0xB8/0xE0/0x108`. **No caller reaches it** in the xref, and
//! neither quit path above leads to it; it is recorded here and left alone

use l2_view::Canvas;

use crate::input::{Event, Rect};
use crate::press::Press;
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::battlefield::{
    BOX_SET, CONFIRM_BOX, CONFIRM_COLS, CONFIRM_NO_FRAME, CONFIRM_ROWS, CONFIRM_WIDGETS,
    CONFIRM_YES_FRAME, GROUP_CONFIRM,
};
use crate::screens::setup::SetupPage;
use crate::shell::{font, Pen};

/// `L2.eng` group 10 index 0, *"Exit the game?"* — `Menu_Quit`'s prompt.
pub const QUIT_PROMPT: usize = 0;
/// `L2.eng` group 10 index 1, *"Start a new game?"* — `Menu_NewGame`'s.
pub const NEW_GAME_PROMPT: usize = 1;

const PROMPT_INSET: i32 = 0x20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// `Menu_Quit` → `FUN_0043441C`.
    Quit,
    /// `Menu_NewGame` → `FUN_00433DEB`.
    NewGame,
}

impl Ask {
    /// The `L2.eng` group 10 index `Ui_OpenConfirm` was passed.
    pub fn prompt(self) -> usize {
        match self {
            Ask::Quit => QUIT_PROMPT,
            Ask::NewGame => NEW_GAME_PROMPT,
        }
    }

    /// Our transcription, for an install whose `L2.eng` has nothing at that
    /// index. `CLAUDE.md` rule 6: the group is the source, this is the
    /// fallback.
    fn ours(self) -> &'static str {
        match self {
            Ask::Quit => "EXIT THE GAME?",
            Ask::NewGame => "START A NEW GAME?",
        }
    }
}

pub struct ConfirmScreen {
    ask: Ask,
    press: Press,
    redraw: bool,
}

impl ConfirmScreen {
    pub fn new(ask: Ask) -> ConfirmScreen {
        ConfirmScreen { ask, press: Press::default(), redraw: true }
    }

    /// `Ui_ConfirmClicked` (`0x00434E1F`) is `g_confirmAnswer = g_uiHotspotId;
    /// (*g_confirmCallback)();` — index 0 is the thumb up, index 1 the thumb
    /// down.
    fn answer(&mut self, yes: bool) -> Transition {
        if !yes {
            return Transition::Pop;
        }
        match self.ask {
            Ask::Quit => Transition::Quit,
            // **Inferred.** `FUN_00433DEB`'s single-player yes is
            // `DAT_005C9274 = 1; DAT_005AEB8C = 0; FUN_004B11CE();` — the game
            // is torn down and the front end comes back. `FUN_004B11CE` is not
            // named, so which page it lands on is not established; ours is the
            // title page, the front end's own first.
            Ask::NewGame => Transition::Replace(ScreenId::Setup(SetupPage::Title)),
        }
    }
}

impl Screen for ConfirmScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Confirm(self.ask)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Confirm - screen 0x1E".into()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, _ctx: &mut Ctx) -> Transition {
        let fired = self.press.event(&CONFIRM_WIDGETS, event);
        if fired.is_some() || self.press.busy() {
            self.redraw = true;
        }
        Transition::Stay
    }

    fn update(&mut self, _ctx: &mut Ctx) -> Transition {
        if let Some(widget) = self.press.tick().next() {
            self.redraw = true;
            return self.answer(widget == 0);
        }
        Transition::Stay
    }

    fn take_redraw(&mut self) -> bool {
        let r = self.redraw || self.press.take_redraw();
        self.redraw = false;
        r
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let p = Pen {
            assets: &ctx.assets.shell,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        p.window(canvas, CONFIRM_BOX.x, CONFIRM_BOX.y, CONFIRM_COLS, CONFIRM_ROWS, BOX_SET);
        // Rule 6: the player's own `L2.eng` group 10 says the words.
        let from_eng = ctx.assets.shell.text(GROUP_CONFIRM, self.ask.prompt());
        let s = if from_eng.is_empty() {
            self.ask.ours().to_string()
        } else {
            from_eng.to_uppercase()
        };
        p.body(canvas, CONFIRM_BOX.x + PROMPT_INSET, CONFIRM_BOX.y + PROMPT_INSET, &s, font::TEXT);
        draw_gauntlets(&p, canvas, ctx, &self.press);
    }
}

/// The two pictures of `g_confirmWidgets` (`0x004DD310`): frames 29 and 31,
/// `Widget_Draw` (`0x0040CFD2`) drawing `base + 1` while one is held.
fn draw_gauntlets(p: &Pen, canvas: &mut Canvas, ctx: &Ctx, press: &Press) {
    for (i, (w, label)) in CONFIRM_WIDGETS.iter().zip(["YES", "NO"]).enumerate() {
        let frame = if i == 0 { CONFIRM_YES_FRAME } else { CONFIRM_NO_FRAME };
        let frame = if press.is_pressed(i) { frame + 1 } else { frame };
        let drawn = p.chrome.is_some_and(|c| c.draw_system(canvas, frame, w.rect.x, w.rect.y));
        if !drawn {
            let r: Rect = w.rect;
            crate::widget::button(canvas, &ctx.assets.ink, r, label, false);
        }
    }
}
