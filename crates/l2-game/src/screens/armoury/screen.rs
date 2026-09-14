#![allow(unused_imports)]
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

/// **The armoury's animation state** — the counters `Tick_Pulses` owns and the
/// soldier `Armoury_ClickRack` sends.
///
/// It is one struct because the original's `Screen_DrawWidgets` arm is one
/// three-call line shared by `0x0A` and `0x0D`, and because only the top screen
/// of our stack gets a tick: the rack panel is pushed *over* the armoury, so if
/// this lived on either screen the other's would stop. Both step this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Anim {
    /// Milliseconds of fixed tick accumulated toward the next 20 ms pulse.
    /// **Not a clock**: [`TICK_MS`] is a constant and this counts ticks.
    acc_ms: u32,
    /// `DAT_005AEB2C`, the 20 ms counter whose fourth step is `g_pulse80`.
    div: u8,
    /// `DAT_005AEA54`, 0…12 — the torches.
    pub torch: u8,
    /// `DAT_005AEA48`, 0…23 — the weapon turning in the rack panel's well.
    pub weapon: u8,
    pub walker: Walker,
}

impl Anim {
    /// One fixed tick. Returns whether anything on screen changed, which is
    /// what `Screen::take_redraw` is for: a still armoury with no soldier in
    /// it still has two torches, so this is true roughly every fifth tick and
    /// not every one.
    ///
    /// **At most one pulse a tick, and the remainder is thrown away.** Both are
    /// `Tick_Pulses` (`0x004BBC80`), which `Battle_Frame` calls once a frame:
    ///
    /// ```c
    /// now = timeGetTime();
    /// if (0x13 < (int)(now - stamp) || (int)(now - stamp) < 0) {
    ///     DAT_005AEB2C++;  DAT_0058FCB0 = 1;  stamp = now;     /* now, not +20 */
    /// }
    /// ```
    ///
    /// So the pulse comes on the **first frame at least 20 ms after the last
    /// pulse**, and on a 16 ms frame that is every second frame — 32 ms, not
    /// 20. `[V]` for the gate; *our tick is the frame* is the reading
    /// [`crate::press`] already makes of `FUN_004B20ED`, the 30 ms gate beside
    /// it, which resets its stamp the same way.
    ///
    /// **This used to subtract twenty and keep the rest**, which is the one
    /// reading under which the walk is exactly 200 pixels a second on every
    /// machine — a rate the original reaches only on a frame of exactly 20 ms
    /// and never above it. A player: *"his animation speed was faster than the
    /// regular game. not bad, but not the OG."* It was 1.6 times faster.
    /// `docs/decisions.md` C179.
    pub fn tick(&mut self) -> bool {
        self.acc_ms += TICK_MS;
        if self.acc_ms < PULSE_MS {
            return false;
        }
        self.acc_ms = 0;
        self.div += 1;
        let pulse80 = self.div >= PULSE80_DIVIDER;
        let mut moved = false;
        if pulse80 {
            self.div = 0;
            self.torch = (self.torch + 1) % TORCH_FRAMES;
            self.weapon = (self.weapon + 1) % WEAPON_FRAMES;
            moved = true;
        }
        if self.walker.active {
            self.walker.pulse(pulse80);
            moved = true;
        }
        moved
    }
}

// ------------------------------------------------------------- the painter

/// **The whole static page**, shared by `0x0A` and by the first frame of
/// `0x17`.
///
/// `buttons` is false for the raise-army screen, which draws the three labels
/// too — the painter is one function and does not know which screen called it —
/// but where they are *dead*: `0x17`'s input arm tests only its own three
/// widgets, so *Create*, *Change* and *Cancel* are visible and inert until the
/// player presses Continue. We draw them dimmed there,
/// because a label that is painted and does nothing is what the original shows.
pub fn page(ctx: &Ctx, canvas: &mut Canvas, buttons: bool) {
    let a = &ctx.assets.shell;
    let ink = &ctx.assets.ink;
    let pen = Pen {
        assets: a,
        ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    };

    if !shell::background(canvas, a, "Armoury.pl8") {
        canvas.clear(ink.background);
    }

    let realm = ctx.game.kingdom.realms.get(ctx.game.player as usize);
    let sheet = realm.map(|r| items_sheet(r.shield_index)).unwrap_or(ITEM_SHEETS[1]);

    // `FUN_00418426`: the walls. A weapon the realm does not own is not there.
    if let Some(sheet) = a.sheet(sheet) {
        for (slot, &(frame, x, y)) in WALL.iter().enumerate() {
            if realm.is_some_and(|r| r.weapons[slot] > 0) {
                if let Some(f) = sheet.frame(frame) {
                    canvas.blit(&f, x, y);
                }
            }
        }
    }

    // `FUN_004181EB`: the racks. Slot 0 is unconditional, 1..=6 need stock in
    // the armoury or a band of that type on offer, and slot 7 is never reached.
    let basket = &ctx.game.levy.basket;
    let band = ctx.game.kingdom.counties.get(ctx.game.levy.county as usize).map_or(0, |c| {
        if ctx.game.levy.hire {
            c.mercenary_offer
        } else {
            0
        }
    });
    let (band_troop, band_men) = if band == 0 {
        (usize::MAX, 0)
    } else {
        let rules = &l2_kingdom::mercenary::ROSTER[band as usize];
        (rules.troop.index(), rules.men)
    };

    for (slot, &(frame, sx, sy, nx, ny)) in RACKS.iter().enumerate().take(RACKS_DRAWN) {
        let extra = if slot == band_troop { band_men } else { 0 };
        if slot > 0 && basket.slots[slot].available <= 0 && extra == 0 {
            continue;
        }
        if let Some(f) = a.sheet(sheet).and_then(|s| s.frame(frame)) {
            canvas.blit(&f, sx, sy);
        } else {
            // **Ours**, and only with no artwork: name the rack where its
            // picture would have stood, so the row is still readable and still
            // visibly ours.
            let name = TroopType::from_index(slot).map_or("", |t| t.name());
            text::draw(canvas, sx, sy + 40, &name.to_uppercase(), ink.dim);
        }
        // `Armoury_DrawRacks`: `Ui_DrawNumber(v, '@', &DAT_004D403C, x, y,
        // &g_fontBody, 0x3F)` (and `&DAT_004D4040` on the other arm) — both
        // suffixes a NUL, read out of the image. **[V]**
        let v = basket.slots[slot].chosen + extra;
        pen.number_in(shell::Face::Body, canvas, nx, ny, v, '@', "", font::TEXT);
    }

    // The three labels, in their own hundred-pixel column.
    for (i, &index) in [CREATE, CHANGE, CANCEL].iter().enumerate() {
        let colour = if buttons { font::TEXT } else { ink.dim };
        pen.eng_centred(canvas, GROUP, index, LABEL_X, LABEL_Y[i], LABEL_W, colour);
    }
}

/// **`Screen_DrawWidgets`' `0x0A` arm, which is the whole of the moving room.**
///
/// ```c
/// 0x0A:  Armoury_RestoreWalkerStrip(); Armoury_DrawTorches(); Armoury_DrawWalker();
/// 0x0D:  Armoury_RestoreWalkerStrip(); Armoury_DrawTorches(); Armoury_DrawRacks();
///        FUN_00418E2D(); Armoury_DrawWalker();
/// ```
///
/// One pass, both screens, run after the page —
/// `Screen_Draw` has **no `'\r'` case at all**, so `0x0D` is painted once on
/// the way in and this arm is the only thing that runs on it afterwards.
///
/// The restore is [`walker_strip`] and is not called here; the racks and
/// `FUN_00418E2D` are the rack panel's own repaint and are in [`RackScreen`].
/// What is left is the two torches and the soldier, and both screens get them
/// because our rack panel is an overlay drawn over the armoury, which is the
/// same sharing.
pub fn overlay(ctx: &Ctx, canvas: &mut Canvas, anim: &Anim) {
    let a = &ctx.assets.shell;

    // `Armoury_DrawTorches` — one sheet, two positions, thirteen frames apart.
    if let Some(sheet) = a.sheet(TORCH_SHEET) {
        for (i, &(x, y)) in TORCH_AT.iter().enumerate() {
            let frame = anim.torch as usize + i * TORCH_SECOND;
            if let Some(f) = sheet.frame(frame) {
                canvas.blit(&f, x, y);
            }
        }
    }

    // `Armoury_DrawWalker` — the blit half. No centring: the original passes
    // `DAT_0056D630` straight to `Pl8_DrawFrameClipped`, and the negative x he
    // starts at is why the call is the clipped one.
    let w = &anim.walker;
    if !w.active {
        return;
    }
    let shield = ctx.game.kingdom.realms.get(ctx.game.player as usize).map_or(0, |r| r.shield_index);
    let sheet = walker_sheet(shield, w.slot);
    if let Some(f) =
        a.sheet(sheet).or_else(|| a.sheet(WALKER_FALLBACK)).and_then(|s| s.frame(w.frame))
    {
        canvas.blit(&f, w.x, WALKER_Y);
    }
}

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

/// Screen `0x0A` for one county's levy.
pub struct ArmouryScreen {
    county: u8,
    /// One line of feedback. **Ours.**
    status: String,
    /// What the last *Create* did, for a test that wants to know without
    /// reading the kingdom.
    pub outcome: Raised,
    /// Set when [`Anim::tick`] moved something, so the machine repaints without
    /// an event having arrived — the torches gutter on a screen nobody is
    /// touching. Same mechanism as the campaign map's edge scroll.
    redraw: bool,
}

/// What the *Create* button did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Raised {
    None,
    Army(usize),
    Refused(LevyRefusal),
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

// -------------------------------------------------------- 0x0D, one weapon

/// Screen `0x0D` — one rack, opened from the armoury.
pub struct RackScreen {
    county: u8,
    /// `DAT_00553F20`, 1…6. Held here as well as on the order because the
    /// screen is identified by it.
    troop: u8,
    /// See [`ArmouryScreen`] — the weapon in the well turns, and the soldier
    /// walks, on a screen nobody is touching.
    redraw: bool,
}

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

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
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

