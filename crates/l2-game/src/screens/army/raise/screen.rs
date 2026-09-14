#![allow(unused_imports)]
use super::*;
use super::impl_screen::*;
use super::*;
use l2_kingdom::mercenary::ROSTER;
use l2_kingdom::unit::TroopType;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::armoury;
use crate::shell::{self, font, Pen};
use crate::widget;
use crate::screens::armoury::Raised;

impl RaiseArmyScreen {
    pub fn new(county: u8) -> RaiseArmyScreen {
        RaiseArmyScreen {
            county,
            status: "DRAG THE SLIDER, THEN CONTINUE".into(),
            left_down: false,
            press: Press::new(),
        }
    }

    /// The table as it stands: Continue, and the tick and cross only when the
    /// band can be afforded.
    fn table(&self, ctx: &Ctx) -> Vec<Widget> {
        let offer = self.offer(ctx) != 0;
        widgets(offer, offer && self.affordable(ctx))
    }

    /// **`Screen_HandleInput`'s `0x17` arm** — `Widget_Test(0, 0,
    /// &DAT_004DD340, n)` over the three kind-5 records.
    ///
    /// The return is what the original's `if (Screen_HandleInput() == 0)`
    /// tests: *did a record take this event*. It is not *did a handler run* —
    /// every record here is kind 5, so a press consumes the event and runs
    /// nothing, and [`Screen::update`] runs the handler twenty ticks later.
    /// That distinction is the reason this is a hit test and not the
    /// `Option<usize>` [`Press::event`] returns.
    fn widget_press(&mut self, ctx: &mut Ctx, event: Event) -> bool {
        let table = self.table(&Ctx { game: ctx.game, assets: ctx.assets });
        let fired = self.press.event(&table, event);
        debug_assert!(fired.is_none(), "every DAT_004DD340 record is kind 5");
        // `Widget_Test`'s guard on both arms is `g_mouseLeftPressed ||
        // g_mouseLeftDoubleClick`, so those are the two events a record can
        // consume. A release, a move and a pointer leaving are bookkeeping:
        // they go to `Press` above and consume nothing.
        matches!(
            event,
            Event::Click { x, y } | Event::DoubleClick { x, y }
                if table.iter().any(|w| w.rect.contains(x, y))
        )
    }

    pub fn county(&self) -> u8 {
        self.county
    }

    /// The band on offer in this county — county `+0x1AD`, which
    /// `Mercenary_AdvanceAll` refreshes once a season.
    pub fn offer(&self, ctx: &Ctx) -> u8 {
        ctx.game
            .kingdom
            .counties
            .get(self.county as usize)
            .map_or(0, |c| c.mercenary_offer)
    }

    /// `Levy_SliderClick` (`0x00435CEF`) — the three zones, on the original's
    /// numbers. Returns whether the slider took the input, which is the
    /// function's own return: `Screen_FrameInput`'s `0x17` arm reads the right
    /// release and the corner picture **only when this answers 0**.
    ///
    /// **The three zones read three different things**, and that is the whole
    /// of the gesture:
    ///
    /// ```c
    /// if (x < 0xC4)       { if (!pressed && !doubleClick) return 0;  percent--; }
    /// else if (x < 0x129) { if (!g_mouseLeftDown)         return 0;  percent = x - 0xC4; }
    /// else                { if (!pressed && !doubleClick) return 0;  percent++; }
    /// clamp 0..100;  Levy_SetPercent(sel, percent);  g_redrawRequest = 2;  return 1;
    /// ```
    ///
    /// The arrows step once on a press or a double click and **do not repeat**:
    /// they are not a widget record, so `Widget_Test`'s kind-4 ramp never sees
    /// them. The track reads the **level** of the button, not an edge and not
    /// `g_mouseInputChanged`, so it answers on every frame the button is down
    /// inside the band — which is a drag, and a drag that also starts from a
    /// press anywhere and slides in. Off the track the level does nothing: a
    /// drag carried past either end leaves the knob where the track last put
    /// it, and only an arrow's own press reaches the clamp. `[V]`
    ///
    /// Ours was reachable from `Event::Click` alone, so the knob jumped to the
    /// press and then ignored the pointer: *"Slider bar in army recruitment
    /// cant be dragged."* `pressed` is the edge, `down` the level.
    ///
    /// **It calls `Levy_SetPercent` and nothing else.** An earlier revision of
    /// this module said the slider also ran `FUN_004AA90A` and that *"re-seeding
    /// the basket is what makes a slider move throw away the equipment"*. The
    /// tail of the function is two statements and neither is that call. The
    /// equipment does get thrown away, but by the *door into the armoury*,
    /// which re-seeds on every entry — so the visible effect survives the
    /// correction and the mechanism does not.
    fn slider_click(&mut self, ctx: &mut Ctx, x: i32, y: i32, pressed: bool, down: bool) -> bool {
        let b = base(self.offer(&Ctx { game: ctx.game, assets: ctx.assets }) != 0);
        if !(SLIDER_HIT_X.0..SLIDER_HIT_X.1).contains(&x)
            || !((b + SLIDER_HIT_DY.0)..(b + SLIDER_HIT_DY.1)).contains(&y)
        {
            return false;
        }
        let percent = if x < SLIDER_X {
            if !pressed {
                return false;
            }
            ctx.game.levy.percent - 1
        } else if x < SLIDER_RIGHT {
            if !down {
                return false;
            }
            x - SLIDER_X
        } else {
            if !pressed {
                return false;
            }
            ctx.game.levy.percent + 1
        };
        ctx.game.set_levy_percent(percent);
        self.status = format!(
            "{} MEN, {} HAPPINESS",
            ctx.game.levy.men, ctx.game.levy.happiness_cost
        );
        true
    }

    /// `FUN_00435CBF` — *Continue*: `g_screenId = 0x0A` and
    /// `FUN_004AA90A(g_selectedCounty, g_levyMen)`, in that order. The re-seed
    /// is why walking back and forth strips the levy.
    fn open_armoury(&mut self, ctx: &mut Ctx) -> Transition {
        ctx.game.seed_levy_basket();
        Transition::Replace(ScreenId::Armoury(self.county))
    }

    /// The band's price, and whether the treasury can meet it. The painter asks
    /// this twice — once to pick `DAT_00522F58` and once to choose between
    /// `69/3` and the *"Hire mercenaries ?"* block — and both times against
    /// `g_realms[g_localPlayer].gold`.
    fn affordable(&self, ctx: &Ctx) -> bool {
        let band = self.offer(ctx) as usize;
        band != 0 && ctx.game.gold() >= ROSTER[band].price
    }
}

