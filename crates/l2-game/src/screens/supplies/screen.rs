#![allow(unused_imports)]
use super::*;

use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use l2_kingdom::Kingdom;
use crate::shell::{font, Face, Pen};

impl SuppliesScreen {
    pub fn new(to: u8) -> SuppliesScreen {
        SuppliesScreen {
            from: to,
            to,
            cart: Cart::default(),
            outcome: Dispatch::None,
            opened: false,
            press: Press::new(),
        }
    }

    /// One widget's handler, whether it was reached from the press
    /// ([`crate::press::Kind::Repeat`]) or from the countdown
    /// ([`crate::press::Kind::Delayed`]).
    pub(crate) fn fire(&mut self, ctx: &mut Ctx, widget: usize) -> Transition {
        match widget {
            // The two spinners' own arms are marked on [`Cart::minus`] and
            // [`Cart::plus`], which is where their rule lives.
            i if i < THUMB_UP_INDEX => {
                let row = &ROWS[i / 2];
                if i % 2 == 0 {
                    self.cart.minus(row.id);
                } else {
                    self.cart.plus(row.id);
                }
                Transition::Stay
            }
            // `FUN_0043B04C`, both thumbs; the arms are declared on [`widgets`].
            THUMB_UP_INDEX => self.dispatch(ctx),
            _ => {
                self.outcome = Dispatch::Cancelled;
                Transition::Pop
            }
        }
    }

    pub fn cart(&self) -> Cart {
        self.cart
    }

    pub fn destination(&self) -> u8 {
        self.to
    }

    /// `Sidebar_Button`'s seeding, done on the first frame because that is when
    /// the screen first sees a [`Ctx`].
    fn open(&mut self, ctx: &Ctx) {
        if self.opened {
            return;
        }
        self.opened = true;
        self.from = ctx.game.selected;
        if self.to == 0 {
            self.to = self.from;
        }
        let c = ctx.game.kingdom.counties.get(self.from as usize);
        self.cart = Cart::open(c.map_or(0, |c| c.grain), c.map_or(0, |c| c.herd));
    }

    fn county_name(&self, ctx: &Ctx, id: u8) -> String {
        super::county::county_name(ctx, id)
    }

    /// `FUN_0043B04C`'s thumb-up.
    fn dispatch(&mut self, ctx: &mut Ctx) -> Transition {
        if self.to == self.from {
            // `Ui_OpenConfirm(0xE, 0xA0, 0xA0, FUN_0043B101)` — group 10/14,
            // *"Quit? (no destination)."* We have no confirm screen, so the
            // question is answered the way its **no** answers it: stay here.
            self.outcome = Dispatch::NoDestination;
            return Transition::Stay;
        }
        let (grain, cattle) = (self.cart.grain.1, self.cart.cattle.1);
        // `FUN_0043B145` — `Transport_Spawn`, then the source county's ration,
        // labour, crowding and estimate passes. The second of the original's
        // two identical passes is not reproduced: nothing between them moves.
        let k = &mut ctx.game.kingdom;
        let owner = k.counties[self.from as usize].owner;
        let sent = l2_kingdom::supply::spawn(
            &k.campaign.map,
            &mut k.counties,
            &mut k.campaign.units,
            owner,
            self.from,
            self.to,
            grain,
            cattle,
        );
        self.outcome = match sent {
            l2_kingdom::supply::Sent::Unit(_) => {
                let tables = k.tables;
                let Kingdom { counties, campaign, .. } = k;
                l2_kingdom::field::herd_update_crowding(
                    &tables,
                    &mut counties[self.from as usize],
                    &mut campaign.map,
                );
                Dispatch::Sent(grain, cattle)
            }
            l2_kingdom::supply::Sent::Nowhere => Dispatch::Nowhere,
        };
        Transition::Pop
    }
}

impl Screen for SuppliesScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Supplies(self.to)
    }

    /// `Widget_Test`'s `Sound_RestartSlot(1)`, carried up to the audio
    /// layer. See [`Screen::take_clicks`].
    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    /// A held spinner moved the cart: `Screen_DrawWidgets`' `0x18` arm runs
    /// `FUN_0041AEA2` every frame. See [`Press::take_redraw`].
    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Send supplies — screen 0x18".into()
    }

    fn is_overlay(&self) -> bool {
        true
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        // `Screen_FrameInput`'s epilogue: a press in the minimap raster
        // selects that county and returns to the map. The shell wrapper did
        // this for all seven shells; graduating them lost it.
        // arm: 0x0042FF10/minimap-under-supplies left-press
        if let Event::Click { x, y } = event {
            if l2_view::chrome::minimap_hit_area().contains(x, y) {
                return Transition::Pass;
            }
        }
        let Event::Click { x, y } = event else {
            return match event {
                // `Screen_FrameInput`'s only other gesture on this screen.
                // arm: 0x0042FF10/supplies-right right-release
                Event::RightClick { .. } => Transition::Pop,
                // **Ours.**
                // arm: ours/supplies-keyboard-close key
                Event::KeyDown(Key::Escape) => Transition::Pop,
                // The release ends a hold and the pointer leaving a spinner
                // stops it repeating. No widget here is a release widget, so
                // nothing can fire.
                Event::Release { .. } | Event::Pointer { .. } | Event::PointerLeft => {
                    let fired = self.press.event(&widgets(), event);
                    debug_assert!(fired.is_none(), "no supplies widget is a release widget");
                    Transition::Stay
                }
                // **A double click reaches the eight widgets and nothing else.**
                // `Screen_HandleInput`'s `0x18` arm is `Hotspot_Test` on the two
                // icons — kind 1, `g_mouseLeftPressed` alone — then
                // `Widget_Test` on the six widgets, kinds 4 and 5, whose guards
                // read `g_mouseLeftPressed || g_mouseLeftDoubleClick`; the
                // minimap pick `FUN_0043B412` opens `if (g_mouseLeftPressed ==
                // 0) return 0`. So a spinner steps once more, a thumb restarts
                // its twenty frames. `[V]`
                // This screen dropped it.
                Event::DoubleClick { .. } => match self.press.event(&widgets(), event) {
                    Some(i) => self.fire(ctx, i),
                    None => Transition::Stay,
                },
                _ => Transition::Stay,
            };
        };
        // `Screen_DrawWidgets` tests the two icon hotspots **before** the six
        // widgets, so the toggle wins wherever they overlap. They do not, but
        // the order is the original's.
        for row in &ROWS {
            if row.toggle.contains(x, y) {
                self.cart.toggle(row.id);
                return Transition::Stay;
            }
        }
        // The six spinners and the two thumbs, each with its own kind: the
        // spinners fire here and go on firing while held, the thumbs fire from
        // [`Screen::update`] twenty ticks later.
        let table = widgets();
        if let Some(i) = self.press.event(&table, event) {
            return self.fire(ctx, i);
        }
        if table.iter().any(|w| w.rect.contains(x, y)) {
            // A thumb was pressed and is counting down. It has consumed the
            // click; nothing below may also answer it.
            return Transition::Stay;
        }
        // `FUN_0043B412(0x60, 0x68)` — the minimap pick, which is the only
        // way the destination ever moves.
        // arm: 0x0043B412/supplies-pick left-press
        if MINIMAP_HIT.contains(x, y) {
            // `Minimap::county_at` subtracts the SIDEBAR hit origin, so the
            // pixel is rebased onto it here
            // raster being written.
            if let Some(m) = ctx.assets.minimap(ctx.game.map_slot) {
                let id = m.county_at(
                    x - MINIMAP_HIT.x + l2_view::chrome::MINIMAP_HIT_X,
                    y - MINIMAP_HIT.y + l2_view::chrome::MINIMAP_HIT_Y,
                );
                if id != 0 {
                    self.to = id;
                    self.outcome = Dispatch::None;
                }
            }
        }
        Transition::Stay
    }

    /// `Widget_Test`'s per-frame pass: the spinners' ramp and the thumbs'
    /// twenty-frame countdown.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        for i in self.press.tick() {
            let t = self.fire(ctx, i);
            if t != Transition::Stay {
                return t;
            }
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        // `open` is deferred to the first draw for the same reason the map's
        // `ensure` is: the screen is built from a `ScreenId` and does not see
        // the game until it is given one.
        let mut me = std::mem::replace(self, SuppliesScreen::new(self.to));
        me.open(ctx);
        *self = me;

        let a = &ctx.assets.shell;
        let pen = Pen {
            assets: a,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        pen.window(canvas, BOX.0, BOX.1, BOX.2, BOX.3, BOX.4);

        // The minimap, at the position the original draws it — two pixels away
        // from where it tests it.
        if let Some(m) = ctx.assets.minimap(ctx.game.map_slot) {
            let owner = |c: u8| ctx.game.kingdom.counties.get(c as usize).map_or(0, |c| c.owner);
            l2_view::chrome::draw_minimap_at(
                canvas,
                &m,
                MINIMAP_DRAW,
                self.to,
                &l2_view::chrome::MinimapTint::Owner(&owner),
            );
        }

        let title = a.text(GROUP, TITLE).to_string();
        pen.heading(canvas, TITLE_AT.0, TITLE_AT.1, &title, font::TEXT);
        let prompt = a.text(GROUP, PROMPT).to_string();
        pen.body_wrapped(canvas, PROMPT_AT.0, PROMPT_AT.1, PROMPT_AT.2, &prompt, font::TEXT);

        pen.eng(canvas, GROUP, FROM, FROM_AT.0, FROM_AT.1, font::TEXT);
        let from = self.county_name(ctx, self.from);
        pen.body(canvas, FROM_NAME_AT.0, FROM_NAME_AT.1, &from, font::TEXT);
        pen.eng(canvas, GROUP, TO, TO_AT.0, TO_AT.1, font::TEXT);
        let to = self.county_name(ctx, self.to);
        pen.body(canvas, TO_NAME_AT.0, TO_NAME_AT.1, &to, font::TEXT);
        pen.eng(canvas, GROUP, DISPATCH, DISPATCH_AT.0, DISPATCH_AT.1, font::TEXT);

        // The per-frame half.
        pen.box_interior(canvas, WELL.x, WELL.y, 0x14, 5);
        pen.inset(canvas, WELL);
        // `Widget_Draw`'s `base + 1` while `+0x0D` runs. The index is
        // [`widgets`]'
        let press = &self.press;
        let frame = |i: usize, base: usize| if press.is_pressed(i) { base + 1 } else { base };
        for (n, row) in ROWS.iter().enumerate() {
            let (left, cart) = self.cart.get(row.id);
            pen.eng(canvas, GROUP, row.label, row.label_at.0, row.label_at.1, font::TEXT);
            // `FUN_0041AEA2`: `Ui_DrawNumber(…, ' ', &DAT_004D4208 … &DAT_004D4214,
            // …)`, four suffixes of one space each. **[V]**
            let y = row.label_at.1;
            pen.number_in(Face::Body, canvas, row.left_x, y, left, ' ', " ", font::TEXT);
            pen.misc_frame(canvas, row.icon, row.icon_at.0, row.icon_at.1);
            pen.number_in(Face::Body, canvas, row.cart_x, y, cart, ' ', " ", font::TEXT);
            pen.system_frame(canvas, frame(n * 2, MINUS_FRAME), row.minus.x, row.minus.y);
            pen.system_frame(canvas, frame(n * 2 + 1, PLUS_FRAME), row.plus.x, row.plus.y);
        }
        pen.system_frame(canvas, frame(THUMB_UP_INDEX, THUMB_UP_FRAME), THUMB_UP.x, THUMB_UP.y);
        pen.system_frame(
            canvas,
            frame(THUMB_UP_INDEX + 1, THUMB_DOWN_FRAME),
            THUMB_DOWN.x,
            THUMB_DOWN.y,
        );

        if self.outcome == Dispatch::NoDestination {
            // `L2.eng` group 10 index 14 — the original's own words for it.
            let s = a.text(10, 14).to_string();
            let s = if s.is_empty() { "QUIT? (NO DESTINATION)".into() } else { s };
            pen.body_centred(canvas, BOX.0, 0x148, BOX.2 * 16, &s, font::TEXT);
        }
    }
}

