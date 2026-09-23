//! Screen `0x21`, the value spinner: `Ui_OpenSlider` (`0x0040E761`) and
//! `Screen_SliderBox` (`0x0040CD58`). Two live openers, both `(…, step 10, max 100, min 0,
//! 0xA0, 0xA0, format 1)` — `Menu_GameSpeed` 0x00434CBE (prompt 1, `g_optGameSpeed`) and
//! `Menu_ScrollSpeed` 0x00434CEE (prompt 2, `g_optScrollSpeed`). `00430000.c:2118,2133`. [V]

use l2_view::Canvas;

use crate::game::Prefs;
use crate::input::{Event, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};

/// `L2.eng` group 12: index 0 the footer, 1..5 the prompts.
pub const GROUP_SLIDER: usize = 12;
/// `g_sliderX`, `g_sliderY` — every live call passes `(0xA0, 0xA0)`.
pub const AT: (i32, i32) = (0xA0, 0xA0);
/// `Ui_OkButton(g_sliderX + 0xB8, g_sliderY + 0x38, 0)`; `Ui_OkButtonClicked` 0x0040E7E4 tests 0x18².
pub const OK: Rect = Rect::new(AT.0 + 0xB8, AT.1 + 0x38, 0x18, 0x18);
/// `g_sliderWidgets` 0x004DDBE0: frame 35 at (32, 28) → `Ui_SliderUp`, frame 37 at (64, 28) →
/// `Ui_SliderDown`, both kind 4, 24 px (`widgets.js widgets 4ddbe0 2`). [V]
pub const UP: Rect = Rect::new(AT.0 + 32, AT.1 + 28, 24, 24);
pub const DOWN: Rect = Rect::new(AT.0 + 64, AT.1 + 28, 24, 24);
const FRAMES: [usize; 2] = [35, 37];

const ARROW: crate::press::Kind = crate::arm!("0x004BA9C8/slider-arrows", Repeat);
pub const WIDGETS: [Widget; 2] = [Widget::new(UP, ARROW), Widget::new(DOWN, ARROW)];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spin {
    /// `Menu_GameSpeed` 0x00434CBE → `g_optGameSpeed` 0x0053F230, prompt 12/1.
    GameSpeed,
    /// `Menu_ScrollSpeed` 0x00434CEE → `g_optScrollSpeed` 0x0053F234, prompt 12/2.
    ScrollSpeed,
}

impl Spin {
    pub fn prompt(self) -> usize {
        match self {
            Spin::GameSpeed => 1,
            Spin::ScrollSpeed => 2,
        }
    }

    fn ours(self) -> &'static str {
        match self {
            Spin::GameSpeed => "Adjusting game speed",
            Spin::ScrollSpeed => "Adjusting scroll speed",
        }
    }

    fn value(self, prefs: &mut Prefs) -> &mut i32 {
        match self {
            Spin::GameSpeed => &mut prefs.game_speed,
            Spin::ScrollSpeed => &mut prefs.scroll_speed,
        }
    }
}

pub struct SliderScreen {
    spin: Spin,
    press: Press,
    redraw: bool,
}

impl SliderScreen {
    pub fn new(spin: Spin) -> SliderScreen {
        SliderScreen { spin, press: Press::default(), redraw: true }
    }

    /// `Ui_SliderUp` 0x00434E9E / `Ui_SliderDown` 0x00434EC9 — a gate, not a clamp.
    fn fire(&mut self, ctx: &mut Ctx, widget: usize) {
        let v = self.spin.value(&mut ctx.game.prefs);
        *v = Prefs::step_speed(*v, widget == 0);
        self.redraw = true;
    }
}

impl Screen for SliderScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Slider(self.spin)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Value spinner - screen 0x21".into()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // Screen_FrameInput 0x0042FF10, `0x21` arm: right release → `g_screenId = g_sliderPrevScreen`.
            //
            // arm: 0x0042FF10/slider-right-exit right-release
            Event::RightClick { .. } => Transition::Pop,
            // Else `Ui_OkButtonClicked()` → the same exit, on the left release.
            //
            // arm: 0x0042FF10/slider-ok left-release
            Event::Release { x, y } => {
                let _ = self.press.event(&WIDGETS, event);
                if OK.contains(x, y) {
                    return Transition::Pop;
                }
                Transition::Stay
            }
            _ => {
                if let Some(i) = self.press.event(&WIDGETS, event) {
                    self.fire(ctx, i);
                }
                Transition::Stay
            }
        }
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        let fired: Vec<usize> = self.press.tick().collect();
        for widget in fired {
            self.fire(ctx, widget);
        }
        Transition::Stay
    }

    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    fn take_redraw(&mut self) -> bool {
        std::mem::take(&mut self.redraw) | self.press.take_redraw()
    }

    /// `Screen_SliderBox` 0x0040CD58, then `Screen_DrawWidgets`' `Widget_Draw(g_sliderX,
    /// g_sliderY, &g_sliderWidgets, 2)`. `DAT_0056D8B8 = 10` before the well is not modelled. [I]
    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let p = Pen {
            assets: &ctx.assets.shell,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        let (x, y) = AT;
        // Ui_DrawBox(x - 0x10, y - 0x10, 0xF, 7): Ui_DrawBoxBorder(0, …).
        p.window(canvas, x - 0x10, y - 0x10, 0xF, 7, 0);
        p.ok_button(canvas, OK.x, OK.y, 0);
        let prompt = ctx.assets.shell.text(GROUP_SLIDER, self.spin.prompt()).to_string();
        let prompt = if prompt.is_empty() { self.spin.ours().to_string() } else { prompt };
        p.body(canvas, x + 0x10, y + 8, &prompt, font::TEXT);
        let footer = ctx.assets.shell.text(GROUP_SLIDER, 0).to_string();
        let footer = if footer.is_empty() { "Click Right to Exit".to_string() } else { footer };
        p.body(canvas, x + 0x10, y + 0x3C, &footer, font::TEXT);
        p.box_interior(canvas, x + 0x5C, y + 0x1E, 3, 1);
        // Format 1: `Ui_DrawNumber(*g_sliderValue / 10, ' ', &DAT_004D2778 " ", x + 100, y + 0x22)`.
        let v = match self.spin {
            Spin::GameSpeed => ctx.game.prefs.game_speed,
            Spin::ScrollSpeed => ctx.game.prefs.scroll_speed,
        };
        p.number_in(crate::shell::Face::Body, canvas, x + 100, y + 0x22, v / 10, ' ', " ", font::TEXT);
        for (i, w) in WIDGETS.iter().enumerate() {
            let frame = FRAMES[i] + usize::from(self.press.is_pressed(i));
            let drawn = p.chrome.is_some_and(|c| c.draw_system(canvas, frame, w.rect.x, w.rect.y));
            if !drawn {
                crate::widget::button(canvas, &ctx.assets.ink, w.rect, ["+", "-"][i], false);
            }
        }
    }
}
