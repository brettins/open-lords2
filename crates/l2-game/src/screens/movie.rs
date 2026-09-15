//! `Smk_Play` (`0x0042D91B`) opens the film and parks `g_screenId` here;
//! `Battle_Frame` then calls `Smk_PlayLoop` (`0x0042DBC7`) once a frame, and
//! `Smk_OnFinished` (`0x0042E060`) puts the screen back when the last frame
//! comes due or the player skips. Screen `0x22` has **no painter**:
//!
//! `Screen_FrameInput`'s `0x22` arm is four `if`s, each calling `Smk_Skip`
//! (`0x0042DF30`) — whose log line is the game's own word for it, *"OK :SMK user
//! ends"*:
//!
//! ```c
//! if (DAT_00553FC8 != 0)      Smk_Skip();   /* the multiplayer sync latch   */
//! if (g_mouseRightReleased)   Smk_Skip();
//! if (g_mouseLeftReleased)    Smk_Skip();
//! if (DAT_004EABB4 != 0)      Smk_Skip();   /* WM_KEYDOWN, any key at all   */
//! ```
//!
//! **A release, not a press, and either button**; and **any key**, because
//! `App_WndProc` sets `DAT_004EABB4` on every `WM_KEYDOWN` before it looks at
//! which key it was, and `Screen_FrameInput` clears it on its way out. The
//! first `if` is a network state
//!
//! A skip is `Smk_OnFinished` — so a skip during
//! the start-up sequence moves on to the next film. Two
//! other callers skip without the player: `FUN_0043AD25`, which `Turn_Tick`'s
//! end-of-season phase runs, and `Net_LeaveGame`. Ours cannot reach either
//! state — a film pauses our turn, because only the top screen is stepped — and
//! `docs/arms.json` records both as missing.

use l2_view::canvas::shade_table;
use l2_view::Canvas;

use crate::input::Event;
use crate::movie::{self, Film, Player, Step};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// `L2.eng` group 300 — the language tag `FUN_0041A166` compares.
const GROUP_LANGUAGE: usize = 300;
/// `L2.eng` group 301 — the intro's eleven subtitle lines.
const GROUP_SUBTITLES: usize = 301;
const SUBTITLE_Y: [i32; 2] = [400, 0x1A0];
const SUBTITLE_COLOUR: u8 = 0xF5;

/// `FUN_004093E0`'s border set for every window drawn here.
const BOX_SET: usize = 1;

enum State {
    Unopened,
    Failed,
    Playing(Box<Player>),
}

pub struct MovieScreen {
    film: Film,
    state: State,
    redraw: bool,
}

impl MovieScreen {
    /// **No compensation, and there used to be one.** The castle chooser's tick
    /// is a kind-5 widget — `Widget_Test` (`0x0040DA1E`) shows it pressed and
    /// runs `CastleBuild_Confirm` (`0x00436B59`) **twenty frames after the
    /// press** — so by the time `Smk_Play` runs, the button has been let go and
    /// that release was answered by the chooser, nineteen frames before this
    /// screen existed. `CastleBuild_Confirm`'s own first call (`FUN_004B18E3`)
    /// throws the click away as well.
    ///
    /// `FUN_00432B05` shows the original guarding the same thing by hand for
    /// the Lords of Magic trailer — `g_mouseLeftReleased = 0` on the line
    /// before its `Smk_Play` — and ours needs no such guard there either,
    /// because the setup page fires that item on the release as the original
    /// does.
    pub fn new(film: Film) -> MovieScreen {
        MovieScreen { film, state: State::Unopened, redraw: true }
    }

    pub fn film(&self) -> Film {
        self.film
    }

    pub fn frame(&self) -> Option<usize> {
        match &self.state {
            State::Playing(p) => Some(p.frame()),
            _ => None,
        }
    }

    fn finish(&mut self, ctx: &mut Ctx) -> Transition {
        ctx.game.films.finished = Some(self.film);
        self.film.then()
    }

    fn skip(&mut self, ctx: &mut Ctx) -> Transition {
        match self.state {
            State::Playing(_) => self.finish(ctx),
            _ => Transition::Stay,
        }
    }

    fn open(&mut self, ctx: &Ctx) {
        if !matches!(self.state, State::Unopened) {
            return;
        }
        // `FUN_0041A166`'s language test, made once: the cues are `intro.smk`'s
        // alone, and only a translated `L2.eng` has them.
        let cued = self.film == Film::Intro
            && !movie::is_english(ctx.assets.shell.text(GROUP_LANGUAGE, 0));
        self.state = match ctx
            .assets
            .films
            .open(self.film.file())
            .and_then(|smk| Player::open(smk, cued).ok())
        {
            Some(p) => State::Playing(Box::new(p)),
            None => State::Failed,
        };
        self.redraw = true;
    }
}

impl Screen for MovieScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Movie(self.film)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        self.film.title().to_string()
    }

    fn is_overlay(&self) -> bool {
        self.film.is_over_a_screen()
    }

    fn live_palette(&self) -> Option<l2_formats::Palette> {
        match &self.state {
            State::Playing(p) => Some(l2_formats::Palette::from_entries(*p.decoder().palette())),
            _ => None,
        }
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        self.open(ctx);
        if matches!(self.state, State::Failed) {
            return Transition::Pass;
        }
        match event {
            // arm: 0x0042FF10/film-skip-right-release right-release
            Event::RightClick { .. } => self.skip(ctx),
            // arm: 0x0042FF10/film-skip-left-release left-release
            Event::Release { .. } => self.skip(ctx),
            // `DAT_004EABB4` is set on every `WM_KEYDOWN`, whichever key.
            //
            // arm: 0x0042FF10/film-skip-key key
            Event::KeyDown(_) => self.skip(ctx),
            _ => Transition::Stay,
        }
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        match &mut self.state {
            State::Unopened => {
                self.open(ctx);
                Transition::Stay
            }
            State::Failed => self.film.on_failure(),
            State::Playing(player) => {
                let before = player.frame();
                match player.tick() {
                    Ok(Step::Playing) => {
                        self.redraw |= player.frame() != before;
                        Transition::Stay
                    }
                    Ok(Step::Finished) | Err(_) => self.finish(ctx),
                }
            }
        }
    }

    fn take_redraw(&mut self) -> bool {
        std::mem::take(&mut self.redraw)
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        if !self.film.is_over_a_screen() {
            // `FUN_004B11CE` → `FUN_004B1867`: the back buffer cleared.
            canvas.clear(0);
        }
        let pen = Pen {
            assets: &ctx.assets.shell,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        match self.film {
            Film::Capture { record, .. } => draw_capture_window(&pen, ctx, canvas, &record),
            Film::Ending { record, .. } => draw_ending_window(&pen, ctx, canvas, &record),
            _ => {}
        }
        let State::Playing(player) = &self.state else { return };
        let (w, _) = player.decoder().size();
        let (x, y) = self.film.at();
        // **`_SmackToBuffer@28` (`0x403AF0`) writes a doubled film's even rows
        // only**, and the three that are doubled are
        // clear above covers, so the odd rows are the black it left.
        canvas.blit_raster(&player.decoder().display(), w, x, y, 1);
        for (line, at) in player.subtitles.lines.iter().zip(SUBTITLE_Y) {
            if let Some(i) = line {
                let s = ctx.assets.shell.text(GROUP_SUBTITLES, *i).to_string();
                pen.body_centred(canvas, 0, at, 0x280, &s, SUBTITLE_COLOUR);
            }
        }
    }
}

/// ```c
/// FUN_004B1310();                                    /* the screen dimmed  */
/// FUN_004093E0(0x10, 0x30, 0x1C, 0x17);
/// Ui_DrawInsetRect(0x27, 0x68, 0x192, 0xC2);         /* the film's well    */
/// Ui_OkButton(0x1A0, 0x170, 0);
/// Ui_DrawCentred(100, scenario * 0x14 + county, 0x10, 0x46, 0x1C0, heading);
/// FUN_0040328E(group, 1, 0x30, 0x142, 0x180, 400, 0, 0, body);
/// ```
///
/// The dimming reads the campaign palette, which is the one on screen when a
/// capture is announced; `[I]` for a capture announced over anything else.
fn draw_capture_window(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &crate::message::Record) {
    canvas.remap(&shade_table(&ctx.assets.palette));
    pen.window(canvas, 0x10, 0x30, 0x1C, 0x17, BOX_SET);
    shell::inset_rect(canvas, 0x27, 0x68, 0x192, 0xC2);
    pen.ok_button(canvas, 0x1A0, 0x170, 0);
    let county = super::message::county_name(ctx, record.county);
    pen.heading_centred(canvas, 0x10, 0x46, 0x1C0, &county, font::TEXT);
    let body = ctx.assets.shell.text(record.group as usize, 1).to_string();
    pen.body_wrapped(canvas, 0x30, 0x142, 0x180, &body, font::TEXT);
}

/// ```c
/// FUN_004B1310();
/// FUN_004093E0(0x10, 0x30, 0x1C, 0x18);
/// Ui_DrawInsetRect(0x58, 0x68, 0x12A, 0xBA);
/// Ui_OkButton(0x1A0, 0x180, 0);
/// FUN_004025D7(g_playerNames + (group == 0xE1 ? local : from) * 0x2C, 0x10, 0x46, 0x1C0, heading);
/// Ui_DrawCentred(group, 0, 0x10, 0x132, 0x1C0, heading);
/// FUN_0040328E(group, variant + 1, 0x30, 0x152, 0x180, 400, 0, 0, body);
/// ```
fn draw_ending_window(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &crate::message::Record) {
    canvas.remap(&shade_table(&ctx.assets.palette));
    pen.window(canvas, 0x10, 0x30, 0x1C, 0x18, BOX_SET);
    shell::inset_rect(canvas, 0x58, 0x68, 0x12A, 0xBA);
    pen.ok_button(canvas, 0x1A0, 0x180, 0);
    let who = if record.group == l2_kingdom::victory::MSG_VICTORY { ctx.game.player } else { record.from };
    pen.heading_centred(canvas, 0x10, 0x46, 0x1C0, &super::message::lord_name(ctx, who), font::TEXT);
    pen.heading_centred(canvas, 0x10, 0x132, 0x1C0, &super::message::label(ctx, record.group), font::TEXT);
    let body = ctx.assets.shell.text(record.group as usize, record.body_index()).to_string();
    pen.body_wrapped(canvas, 0x30, 0x152, 0x180, &body, font::TEXT);
}
