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
    pub(super) fn confirm(&mut self, ctx: &mut Ctx, yes: bool) -> Transition {
        if !yes {
            return Transition::Pop;
        }
        let levy = ctx.game.levy;
        let band = ctx.game.kingdom.counties.get(self.county as usize).map_or(0, |c| c.mercenary_offer);
        let affordable = band != 0
            && ctx.game.gold() >= l2_kingdom::mercenary::ROSTER[band as usize].price;
        let hire = (levy.hire && affordable).then_some(band);
        let order = crate::command::RaiseArmy {
            county: self.county,
            basket: levy.basket,
            happiness_cost: levy.happiness_cost,
            hire,
        };
        match ctx.issue(order) {
            Ok(id) => {
                self.outcome = Raised::Army(id);
                ctx.game.levy = crate::game::LevyOrder { percent: levy.percent, ..Default::default() };
                Transition::Pop
            }
            Err(no) => {
                self.outcome = Raised::Refused(no);
                // `Army_RaiseConfirm` 0x00435B4D `00430000.c:3120`, `:3123`, `:3128`; `g_screenId = 0` `:3134` closes on all three.
                use crate::message::group;
                let g = match no {
                    LevyRefusal::NoMen => group::ZERO_MEN,
                    LevyRefusal::TooFew => group::UNDER_FIFTY,
                    LevyRefusal::NowhereToStand => group::NOWHERE_TO_STAND,
                };
                crate::message::tell_player(ctx.game, g);
                Transition::Pop
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

    fn palette(&self) -> Option<&'static str> {
        Some("Armoury.256")
    }

    fn is_overlay(&self) -> bool {
        false
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            Event::RightClick { .. } => Transition::Pop,
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

        let pen = Pen {
            assets: &ctx.assets.shell,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        pen.ok_button(canvas, OK.x, OK.y, 1);

        if ctx.game.prefs.debug_overlay && !self.status.is_empty() {
            text::draw(canvas, 8, 8, &self.status, ink.dim);
        }
        if !ctx.assets.shell.has_armoury_grid() {
            text::draw(canvas, 8, 20, "NO ARM_GRID.PL8 - RACKS ARE RECTANGLES", ink.dim);
        }
    }
}

#[cfg(test)]
mod refusal_tests {
    use super::*;

    /// `Army_RaiseConfirm` `00430000.c:3120` then `:3134`: a levy of zero is
    /// scroll 0xA8 (`L2.eng` 168) and the screen closes. Ablate `tell_player`
    /// and the queue is empty.
    #[test]
    fn a_levy_of_zero_raises_scroll_168_and_closes() {
        let mut game = crate::Game::new(3);
        game.kingdom.set_county_count(3);
        game.player = 1;
        game.kingdom.counties[1].owner = 1;
        let assets = crate::Assets::placeholder();
        let mut screen = ArmouryScreen::new(1);
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        assert!(matches!(screen.confirm(&mut ctx, true), Transition::Pop));
        assert_eq!(screen.outcome, Raised::Refused(LevyRefusal::NoMen));
        assert!(game.messages.pull(), "nothing queued");
        assert_eq!(game.messages.open().map(|r| r.group), Some(0xA8));
    }
}

