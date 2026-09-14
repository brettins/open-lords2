#![allow(unused_imports)]
use super::*;
use super::render::*;
use super::rack::*;
use super::*;
use super::walker::*;
use super::tests::*;
use l2_kingdom::levy::{self, LevyRefusal};
use l2_kingdom::tables::WEAPON_TYPE_COUNT;
use l2_kingdom::unit::TroopType;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};
use crate::widget;

/// **`Armoury_ClickRack` (`0x004358B0`) in full**, which is one function in the
/// original and has to be one here too: the rack row is live on *both* screens,
/// so ours is reached from [`ArmouryScreen`] and from [`RackScreen`], and only
/// one of them may carry the marker.
///
/// ```c
/// if (g_levyBasket[id].available <= 0) return;      /* an empty rack is inert */
/// FUN_004AABD8(county, g_armourySelectedType);      /* the rack being LEFT    */
/// g_armourySelectedType = id;
/// g_screenId = 0x0D;
/// g_levyBasket[id].latch = g_levyBasket[id].chosen;
/// Armoury_LoadScreen();
/// ```
///
/// Returns whether the rack opens. The order is the original's and it is the
/// point: the walk is fired for the *previous* selection, before it is
/// overwritten. See [`Walker`].
// arm: 0x004358B0/armoury-rack-click left-press
pub fn click_rack(game: &mut crate::game::Game, troop: u8) -> bool {
    let slot = troop as usize;
    if game.levy.basket.slots.get(slot).is_none_or(|s| s.available <= 0) {
        return false;
    }
    let leaving = game.levy.rack;
    let chosen = game.levy.basket.slots.get(leaving as usize).map_or(0, |s| s.chosen);
    game.levy.anim.walker.start(leaving, chosen);
    game.levy.rack = troop;
    let now = game.levy.basket.slots[slot].chosen;
    game.levy.anim.walker.latch(troop, now);
    true
}

// --------------------------------------------------------- 0x0A, the room

impl ArmouryScreen {
    pub fn new(county: u8) -> ArmouryScreen {
        ArmouryScreen { county, status: String::new(), outcome: Raised::None, redraw: false }
    }

    pub fn county(&self) -> u8 {
        self.county
    }

    /// `FUN_0043582A` then `Hotspot_Test(&g_armouryHotspots, 6)`: the grid
    /// first, the rectangles second. Returns the weapon type under the pointer.
    pub fn rack_at(ctx: &Ctx, x: i32, y: i32) -> Option<u8> {
        if let Some(t) = ctx.assets.shell.armoury_grid(x, y) {
            return Some(t);
        }
        RACK_HOTSPOTS
            .iter()
            .find(|&&(x0, y0, x1, y1, _)| (x0..x1).contains(&x) && (y0..y1).contains(&y))
            .map(|&(.., t)| t)
    }

    /// `FUN_004358B0` — open a rack, **if it has anything in it**. An empty
    /// rack is inert: the guard is on `available`, the stock the realm owns,
    /// not on `chosen`.
    fn open_rack(&mut self, ctx: &mut Ctx, troop: u8) -> Transition {
        if !click_rack(ctx.game, troop) {
            let name = TroopType::from_index(troop as usize).map_or("", |t| t.name());
            self.status = format!("NO {} IN THE ARMOURY", name.to_uppercase());
            return Transition::Stay;
        }
        Transition::Push(ScreenId::Rack(self.county, troop))
    }

    /// `FUN_00435AE8`'s id 1 and id 3 — `Army_RaiseConfirm` with the answer set
/// either way. A no closes the screen and raises
    /// nothing, which is `g_screenId = 0` down both arms of the handler.
    fn confirm(&mut self, ctx: &mut Ctx, yes: bool) -> Transition {
        if !yes {
            return Transition::Pop;
        }
        let levy = ctx.game.levy;
        let band = ctx.game.kingdom.counties.get(self.county as usize).map_or(0, |c| c.mercenary_offer);
        let affordable = band != 0
            && ctx.game.gold() >= l2_kingdom::mercenary::ROSTER[band as usize].price;
        let hire = (levy.hire && affordable).then_some(band);
        match ctx.game.raise_army(self.county, &levy.basket, levy.happiness_cost, hire) {
            Ok(id) => {
                self.outcome = Raised::Army(id);
                ctx.game.levy = crate::game::LevyOrder { percent: levy.percent, ..Default::default() };
                Transition::Pop
            }
            Err(no) => {
                self.outcome = Raised::Refused(no);
                self.status = match no {
                    LevyRefusal::NoMen => "AN ARMY OF ZERO MEN IS NO ARMY".into(),
                    LevyRefusal::TooFew => "FEWER THAN 50 MEN IS IMPRACTICAL".into(),
                    LevyRefusal::NowhereToStand => "NOWHERE IN THE COUNTY TO STAND".into(),
                };
                Transition::Stay
            }
        }
    }
}

impl Screen for ArmouryScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Armoury(self.county)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "The armoury".to_string()
    }

    /// `armoury.256`, set by the painter's last call before it returns.
    fn palette(&self) -> Option<&'static str> {
        Some("Armoury.256")
    }

    /// A 640 × 480 background is a page, not an inset.
    fn is_overlay(&self) -> bool {
        false
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // The corner picture and the right button both leave for the map
            // without raising anything — `g_screenId = 0; Gfx_LoadCountyMode()`
            // down every arm of `Screen_FrameInput`'s `0x0A` case.
            Event::KeyDown(Key::Escape) | Event::RightClick { .. } => Transition::Pop,
            Event::KeyDown(Key::Enter) => self.confirm(ctx, true),
            Event::Click { x, y } => {
                if OK.contains(x, y) {
                    return Transition::Pop;
                }
                if CREATE_BOX.contains(x, y) {
                    return self.confirm(ctx, true);
                }
                if CHANGE_BOX.contains(x, y) {
                    return Transition::Replace(ScreenId::RaiseArmy(self.county));
                }
                if CANCEL_BOX.contains(x, y) {
                    return self.confirm(ctx, false);
                }
                let read = Ctx { game: ctx.game, assets: ctx.assets };
                if let Some(troop) = ArmouryScreen::rack_at(&read, x, y) {
                    return self.open_rack(ctx, troop);
                }
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    /// **`Tick_Pulses` runs whatever screen is up**, so both armoury screens
    /// step the same counters. Only the top screen of our stack is ticked, and
    /// the rack panel is pushed over this one — so this arm covers `0x0A` and
    /// [`RackScreen`]'s covers `0x0D`, and neither can stall the other.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        self.redraw |= ctx.game.levy.anim.tick();
        Transition::Stay
    }

    fn take_redraw(&mut self) -> bool {
        core::mem::take(&mut self.redraw)
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        page(ctx, canvas, true);
        overlay(ctx, canvas, &ctx.game.levy.anim);
        let ink = &ctx.assets.ink;

        // The corner picture, mode 1 — `Ui_OkButton(640 - 0x1C, 480 - 0x70, 1)`.
        // **Nothing else is drawn round the three buttons**: the painter draws
        // three words and the hotspots are invisible, so a frame of ours here
        // would be an invented interface on a screen that does not have one.
        let pen = Pen {
            assets: &ctx.assets.shell,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        pen.ok_button(canvas, OK.x, OK.y, 1);

        // **Ours**, both of them: one line of feedback and one warning that the
        // hit map is missing. The original draws neither.
        // The feedback line is debug overlay only; the missing-file warning
        // below is a fallback a normal install never shows.
        if ctx.game.prefs.debug_overlay && !self.status.is_empty() {
            text::draw(canvas, 8, 8, &self.status, ink.dim);
        }
        if !ctx.assets.shell.has_armoury_grid() {
            text::draw(canvas, 8, 20, "NO ARM_GRID.PL8 - RACKS ARE RECTANGLES", ink.dim);
        }
    }
}

