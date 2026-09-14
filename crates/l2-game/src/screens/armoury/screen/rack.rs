#![allow(unused_imports)]
use super::*;
use super::render::*;
use super::armoury::*;
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

impl RackScreen {
    pub fn new(county: u8, troop: u8) -> RackScreen {
        RackScreen { county, troop, redraw: false }
    }

    pub fn troop(&self) -> u8 {
        self.troop
    }

    fn troop_type(&self) -> Option<TroopType> {
        TroopType::from_index(self.troop as usize)
    }

    /// The count `FUN_00418E2D` prints against 69/5: how many more men could
    /// still take this weapon.
    ///
    /// The original writes it `remaining[sel] - chosen[sel]`, clamped by the
    /// unequipped pool, and its `remaining` is the untouched stock the basket
    /// was seeded with. **Ours already is the difference** —
    /// [`l2_kingdom::LevyBasket::equip`] decrements `remaining` as it fills
    /// `chosen`, so the two conventions meet at the same number and only the
/// expression differs. Written out, because
    /// transcribing it would have subtracted `chosen` twice.
    fn spare(&self, ctx: &Ctx) -> i32 {
        let basket = &ctx.game.levy.basket;
        let slot = self.troop as usize;
        basket.slots.get(slot).map_or(0, |s| s.remaining).min(basket.unequipped()).max(0)
    }

    fn press(&mut self, ctx: &mut Ctx, button: Button) {
        let Some(troop) = self.troop_type() else { return };
        let basket = &mut ctx.game.levy.basket;
        let slot = self.troop as usize;
        match button {
            Button::EquipOne => {
                basket.equip(troop, 1);
            }
            Button::UnequipOne => {
                basket.unequip(troop, 1);
            }
            Button::UnequipAll => {
                let held = basket.slots[slot].chosen;
                basket.unequip(troop, held);
            }
            Button::EquipAll => {
                let room = basket.slots[slot].remaining;
                basket.equip(troop, room);
            }
        }
    }
}

impl Screen for RackScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Rack(self.county, self.troop)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        format!("The armoury — {}", self.troop_type().map_or("", |t| t.name()))
    }

    fn palette(&self) -> Option<&'static str> {
        Some("Armoury.256")
    }

    /// `Ui_DrawBox(0x60, 4, 0x1A, 7)` over the armoury, with no clear.
    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // Both ways out go back to the armoury: `g_screenId = 0x0A;
            // FUN_004180F6()`.
            Event::KeyDown(Key::Escape) | Event::RightClick { .. } => Transition::Pop,
            Event::KeyDown(Key::Right) => {
                self.press(ctx, Button::EquipOne);
                Transition::Stay
            }
            Event::KeyDown(Key::Left) => {
                self.press(ctx, Button::UnequipOne);
                Transition::Stay
            }
            Event::Click { x, y } => {
                if RACK_OK.contains(x, y) {
                    return Transition::Pop;
                }
                for (i, button) in BUTTONS.iter().enumerate() {
                    if button_box(i).contains(x, y) {
                        self.press(ctx, *button);
                        return Transition::Stay;
                    }
                }
                // **The racks are still live here**, on the first seven records
                // of the same hotspot table: the player can walk from one
                // weapon to the next without going back. `Create` is record 6
                // and is live too; `Change` and `Cancel` are records 7 and 8
                // and are not.
                //
                // **The rack already open is one of them.** `Screen_FrameInput`'s
                // `0x0D` arm runs `Armoury_GridClick` and the hotspot table with
                // no test against `g_armourySelectedType`, so clicking the weapon
                // whose panel is up runs `Armoury_ClickRack` for it again — and
                // that is `FUN_004AABD8(county, t)` with the old type equal to
                // the new one. A player who equipped men and clicks the same
                // weapon sends one of them to fetch it. Ours refused a click on
                // the open rack, so the walk came only from a *different* rack.
                let read = Ctx { game: ctx.game, assets: ctx.assets };
                if let Some(troop) = ArmouryScreen::rack_at(&read, x, y) {
                    if click_rack(ctx.game, troop) {
                        return Transition::Replace(ScreenId::Rack(self.county, troop));
                    }
                    return Transition::Stay;
                }
                if CREATE_BOX.contains(x, y) {
                    // Record 6 of the table, and the only one of the three the
                    // `0x0D` arm reaches. It is the armoury's button, so the
                    // armoury runs it: pass the click down.
                    return Transition::Pass;
                }
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    /// See [`ArmouryScreen`]: the panel is on top, so the panel is what steps
    /// the room's clock while it is open.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        self.redraw |= ctx.game.levy.anim.tick();
        Transition::Stay
    }

    fn take_redraw(&mut self) -> bool {
        core::mem::take(&mut self.redraw)
    }

    pub(crate) fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        let w = rack_window();
        pen.window(canvas, w.x, w.y, RACK_BOX_COLS, RACK_BOX_ROWS, 0);

        // The weapon's own picture in the four-line `Ui_DrawInsetRect` well the
        // painter opens for it. **It turns.** `Armoury_LoadScreen` draws frame
        // 0 once on the way in and `FUN_00418E2D` then draws `DAT_005AEA48`
        // every frame — the divider-chain counter that wraps at 24, which is
        // exactly how many frames each `Arm_<weapon>.pl8` holds.
        shell::inset_rect(canvas, WEAPON_WELL.x, WEAPON_WELL.y, WEAPON_WELL.w, WEAPON_WELL.h);
        let slot = (self.troop as usize).saturating_sub(1).min(WEAPON_TYPE_COUNT - 1);
        let turn = ctx.game.levy.anim.weapon as usize;
        if let Some(f) = a.sheet(WEAPON_SHEETS[slot]).and_then(|s| s.frame(turn)) {
            canvas.blit(&f, WEAPON_AT.0, WEAPON_AT.1);
        }

        // `Ui_DrawCount(chosen, 0x34 + t * 2)` — the count and the group 8
        // noun, singular or plural by the count.
        let held = ctx.game.levy.basket.slots[self.troop as usize].chosen;
        pen.box_interior(canvas, COUNT_WELL.x, COUNT_WELL.y, 0x0F, 2);
        let noun = noun(a, self.troop as usize, held, self.troop_type());
        // `Ui_DrawCount(chosen, sel * 2 + 0x34, 0xEC, 0x0D, &g_fontHeading, 0x3F)`
        // — one of the only two heading-face counts in the binary. Built by hand
        // as `"{held} {noun}"` it had no lead, so the digits and the noun both
        // sat four pixels left of the original's.
        let face = crate::shell::Face::Heading;
        pen.count_with_noun(face, canvas, COUNT_AT.0, COUNT_AT.1, i32::from(held), &noun, font::TEXT);

        // `Ui_DrawNumber(spare) + 69/5` — "N more could still be raised."
        let spare = self.spare(ctx);
        pen.box_interior(canvas, SPARE_WELL.x, SPARE_WELL.y, 0x10, 2);
        let x = pen.body(canvas, SPARE_AT.0, SPARE_AT.1, &format!("{spare}"), font::TEXT);
        pen.eng(canvas, GROUP, SPARE, x, SPARE_AT.1, font::TEXT);

        // The four buttons. `Widget_Draw(0x60, 4, &g_armouryBuyWidgets, 4)`
        // draws them out of the **button sheet** at the frames the records
        // carry — 68, 66, 58, 60 — and our own labelled boxes are the fallback
        // for an install without it, not the picture.
        for (i, button) in BUTTONS.iter().enumerate() {
            let r = button_box(i);
            if pen.system_frame(canvas, BUTTON_FRAMES[i], r.x, r.y) {
                continue;
            }
            // **Ours**, no artwork only.
            let label = match button {
                Button::EquipOne => "+1",
                Button::UnequipOne => "-1",
                Button::UnequipAll => "NONE",
                Button::EquipAll => "ALL",
            };
            widget::button(canvas, ink, r, label, false);
        }

        // `Ui_OkButton(0x1E4, 0x58, 0)`.
        pen.ok_button(canvas, RACK_OK.x, RACK_OK.y, 0);
    }
}

/// `Ui_DrawCount`'s noun: `L2.eng` group 8 at `0x34 + t * 2`, singular at **±1**
/// and plural at everything else including zero. Falls back to our own name
/// when `L2.eng` is not installed.
///
/// The `-1` arm is `Ui_DrawCount`'s (`0x0041AB67`) and not
/// `Ui_DrawUnitNoun`'s (`0x0041AC3E`), which takes the singular only at exactly
/// 1 — the rack panel draws the first, the army-division rows the second, and
/// they are two different ladders in the binary. [`shell::count_noun`].
fn noun(a: &shell::ShellAssets, troop: usize, n: i32, ty: Option<TroopType>) -> String {
    let index = shell::count_noun(n, NOUN_BASE + troop * 2);
    let s = a.text(NOUN_GROUP, index);
    if !s.is_empty() {
        return s.to_string();
    }
    let name = ty.map_or("", |t| t.name());
    if index % 2 == 0 {
        name.to_string()
    } else {
        format!("{name}s")
    }
}

/// `Levy_SetPercent`'s two refusals, reachable from this screen only.
pub fn would_refuse(men: i32, hiring: bool) -> Option<LevyRefusal> {
    levy::refuse_levy(men, hiring)
}


