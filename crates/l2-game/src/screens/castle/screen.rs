#![allow(unused_imports)]
use super::*;
use super::constants::*;
use super::tests_part::*;
use l2_kingdom::industry::{self, CastleRefusal};
use l2_view::{text, Canvas};
use crate::press::{Press, Widget};
use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastleChoice {
    None,
    Ordered(u8),
    Refused(CastleRefusal),
    Cancelled,
}

pub struct CastleScreen {
    county: u8,
    /// `DAT_0056D898` — the selected **level**, 0…4, which is `castleType - 1`.
    ///
    /// `None` until the player has clicked, because `Castle_OpenScreen`
    /// (`0x00436A88`) seeds it from the county and a screen cannot read the
    /// county at construction time. It is resolved on first use instead, which
    /// is the same value at the same moment.
    selected: Option<usize>,
    pub choice: CastleChoice,
    pub message: Option<&'static str>,
    press: Press,
}

impl CastleScreen {
    /// `Castle_OpenScreen` (`0x00436A88`) seeds the selection from the castle
    /// already standing, or 0 when there is none — so the screen opens showing
    /// what you have and the OK is refused until you move.
    pub fn new(county: u8) -> CastleScreen {
        CastleScreen {
            county,
            selected: None,
            choice: CastleChoice::None,
            message: None,
            press: Press::new(),
        }
    }

    pub fn county(&self) -> u8 {
        self.county
    }

    /// The selection, resolved against the county the way `Castle_OpenScreen`
    /// seeds `DAT_0056D898`: the castle already standing, or 0 for a bare plot.
    pub fn level(&self, ctx: &Ctx) -> usize {
        self.selected.unwrap_or_else(|| {
            ctx.game.kingdom.counties[self.county as usize].castle_type.saturating_sub(1) as usize
        })
    }

    pub fn castle_type(&self, ctx: &Ctx) -> u8 {
        self.level(ctx) as u8 + 1
    }

    /// `CastleBuild_Select` (`0x00436B22`) — `DAT_0056D898 = g_uiHotspotId`.
    pub fn select(&mut self, level: usize) {
        self.selected = Some(level.min(4));
    }

    pub fn materials(&self, ctx: &Ctx) -> (i32, i32) {
        let t = &ctx.game.kingdom.tables;
        let (mut wood, mut stone) = industry::castle_cost(t, self.castle_type(ctx));
        let standing = ctx.game.kingdom.counties[self.county as usize].castle_type;
        if standing != 0 {
            let (had_wood, had_stone) = industry::castle_cost(t, standing);
            wood -= had_wood;
            stone -= had_stone;
        }
        (wood, stone)
    }

    pub(super) fn confirm(&mut self, ctx: &mut Ctx) -> Transition {
        let want = self.castle_type(ctx);
        let t = ctx.game.kingdom.tables;
        let county = &mut ctx.game.kingdom.counties[self.county as usize];
        if let Some(why) = industry::castle_refusal(&t, county, want) {
            self.choice = CastleChoice::Refused(why);
            self.message = Some(match why {
                // `L2.eng` 147/1 and 290/1, the two the original sends.
                CastleRefusal::AlreadyBuilt => {
                    "THE CASTLE IN THIS COUNTY IS ALREADY OF THE TYPE YOU PROPOSE"
                }
                CastleRefusal::Downgrade => {
                    "YOUR CURRENT CASTLE IS STRONGER THAN THE ONE YOU PROPOSE, MY LORD"
                }
                CastleRefusal::NoSuchType => "THERE IS NO SUCH CASTLE",
            });
            return Transition::Pop;
        }
        let owner = county.owner as usize;
        let l2_kingdom::Kingdom { counties, realms, campaign, .. } = &mut ctx.game.kingdom;
        let Some(realm) = realms.get_mut(owner) else { return Transition::Pop };
        industry::order_castle(&t, &mut counties[self.county as usize], realm, want);
        l2_kingdom::map::stamp_castle_terrain(&mut campaign.map, self.county, want);
        self.choice = CastleChoice::Ordered(want);
        self.message = Some("CONSTRUCTION BEGINS");
        if ctx.game.prefs.animations {
            return Transition::Push(ScreenId::Movie(crate::movie::Film::Castle(want - 1)));
        }
        Transition::Pop
    }
}

impl Screen for CastleScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Castle(self.county)
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "Select a castle to build".to_string()
    }

    fn palette(&self) -> Option<&'static str> {
        Some("cas_back.256")
    }

    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        if let CastleChoice::Ordered(_) = self.choice {
            return Transition::Pop;
        }
        if let Some(widget) = self.press.tick().next() {
            if widget == 0 {
                return self.confirm(ctx);
            }
            self.choice = CastleChoice::Cancelled;
            return Transition::Pop;
        }
        Transition::Stay
    }

    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            Event::KeyDown(Key::Escape) => {
                self.choice = CastleChoice::Cancelled;
                Transition::Pop
            }
            Event::KeyDown(Key::Enter) => self.confirm(ctx),
            Event::DoubleClick { .. } => {
                let fired = self.press.event(&widgets(), event);
                debug_assert!(fired.is_none(), "both castle-build widgets are kind 5");
                Transition::Stay
            }
            // **And it is the release, not the press.** `Ui_OkButtonClicked`
            // (`0x0040E7E4`) opens `if (g_mouseLeftReleased == 0) return 0;`.
            //
            // arm: 0x0042FF10/castle-corner-ok left-release
            Event::Release { x, y } => {
                let fired = self.press.event(&widgets(), event);
                debug_assert!(fired.is_none(), "both castle-build widgets are kind 5");
                if CORNER_OK.contains(x, y) {
                    self.choice = CastleChoice::Cancelled;
                    return Transition::Pop;
                }
                Transition::Stay
            }
            Event::Pointer { .. } | Event::PointerLeft => {
                let fired = self.press.event(&widgets(), event);
                debug_assert!(fired.is_none(), "both castle-build widgets are kind 5");
                Transition::Stay
            }
            Event::Click { x, y } => {
                let table = widgets();
                let fired = self.press.event(&table, event);
                debug_assert!(fired.is_none(), "both castle-build widgets are kind 5");
                if table.iter().any(|w| w.rect.contains(x, y)) {
                    return Transition::Stay;
                }
                for level in 0..5 {
                    if type_rect(level).contains(x, y) {
                        self.select(level);
                        // `CastleBuild_Select` (`0x00436B22`) ends
                        // `FUN_004B3940(g_uiHotspotId)`, which is
                        // `Sound_PlayFile(&S071_02 + hotspot * 0x10, 1, 0)` —
                        // the same five files the information panel's tile
                        // branch speaks, indexed by the button
                        // `castleType - 1`.
                        //
                        // sfx: FUN_004b3940#1
                        if let Some(&line) = crate::audio::names::speech::PICKED_CASTLE.get(level) {
                            ctx.game.spoken = (ctx.game.spoken.0.wrapping_add(1), line);
                        }
                        break;
                    }
                }
                Transition::Stay
            }
            _ => Transition::Stay,
        }
    }

    /// Every line here is one call site of `Screen_CastleBuild` or
    /// `Screen_CastleBuildPanel`, in the original's order, at the original's
    /// coordinate, through the original's fonts and sheets. Nothing on this
    /// screen is a caption of ours: the words it shows are `L2.eng` group 71
    /// and the pictures are `cas_back.pl8`, `caspics.pl8` and `cas_bits.pl8`.
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
        let level = self.level(ctx);
        let castle_type = level as u8 + 1;
        let standing = ctx.game.kingdom.counties[self.county as usize].castle_type;

        // `FUN_00408FCB("cas_back.pl8", 0x1E0)`. **The five castle pictures and
        // the "Select a castle to build" heading are in this image** — the
        // painter draws neither, so with no install there is nothing to draw
        // and the strips below say so instead.
        let have_backdrop = shell::background(canvas, a, BACKDROP);
        if !have_backdrop {
            canvas.clear(ink.background);
        }

        pen.ok_button(canvas, CORNER_OK.x, CORNER_OK.y, 1);

        // `if (DAT_004D2DD0[sel] != 0) Blit_Raster(caspics[n - 1], 0x9E, 0x14, …)`
        // — and for the motte and bailey it is zero, so nothing is blitted.
        if let Some(frame) = PICTURE_FRAME[level].checked_sub(1) {
            if let Some(f) = a.sheet(PICS).and_then(|s| s.frame(frame)) {
                canvas.blit_opaque(&f, PICTURE.x, PICTURE.y);
            }
        }

        let bits = |canvas: &mut Canvas, frame: usize, x: i32, y: i32| {
            if let Some(f) = a.sheet(BITS).and_then(|s| s.frame(frame)) {
                canvas.blit(&f, x, y);
            }
        };
        let (frame, x, y) = NAME_PLATE[level];
        bits(canvas, frame, x, y);
        let (frame, x, y) = SELECTED_MARK[level];
        bits(canvas, frame, x, y);

        // `FUN_0040328E(71, sel + 1, 0x1F6, 0x18, 0xA0, 100, 0, 0, heading, 0x3F)`
        // — the castle's own name, wrapped at 160 pixels in the 22-pixel font,
        // which steps 0x18 a line and not 0x10.
        let name = a.text(GROUP, NAME_BASE + level).to_string();
        let mut line = NAME_AT.1;
        for part in heading_wrap(a, &name, NAME_WIDTH) {
            pen.heading(canvas, NAME_AT.0, line, &part, font::TEXT);
            line += NAME_LINE;
        }

        let t = &ctx.game.kingdom.tables;
        let (wood, stone) = self.materials(ctx);
        // `Ui_DrawNumber(…, '@', &DAT_004D4154 | &DAT_004D4158, 0x230, …)` in
        // `Screen_CastleBuildPanel`, both NUL. **[V]**
        let body = shell::Face::Body;
        bits(canvas, STONE_CAPTION, CAPTION_X, STONE_CAPTION_Y);
        pen.number_in(body, canvas, MATERIAL_NUM_X, STONE_NUM_Y, stone, '@', "", font::TEXT);
        bits(canvas, WOOD_CAPTION, CAPTION_X, WOOD_CAPTION_Y);
        pen.number_in(body, canvas, MATERIAL_NUM_X, WOOD_NUM_Y, wood, '@', "", font::TEXT);

        if standing != 0 {
            let x = STANDING_MARK_X[(standing as usize).min(5)] + STANDING_MARK_DX;
            bits(canvas, STANDING_MARK, x, STANDING_MARK_Y);
        }

        let work = industry::castle_workforce(t, castle_type);
        pen.eng(canvas, GROUP, WILL_TAKE, WILL_TAKE_AT.0, WILL_TAKE_AT.1, font::TEXT);
        pen.count(canvas, WORKFORCE_AT.0, WORKFORCE_AT.1, work, BUILDER_NOUN, font::TEXT);
        pen.count(canvas, SEASON_AT.0, SEASON_AT.1, 1, SEASON_NOUN, font::TEXT);
        pen.eng(canvas, GROUP, TO_BUILD, TO_BUILD_AT.0, TO_BUILD_AT.1, font::TEXT);

        // `Ui_DrawBox(0x70, 0x1AC, 0x1A, 3)` — border **set 0**, unlike the
        // court's and the trade panel's `FUN_004093E0`.
        pen.window(canvas, TAX_PLAQUE.0, TAX_PLAQUE.1, TAX_PLAQUE.2, TAX_PLAQUE.3, 0);
        let bonus = t.castle.tax_bonus_pct[level.min(t.castle.tax_bonus_pct.len() - 1)];
        let x = pen.eng(canvas, GROUP, BOOSTS_TAX, BOOSTS_TAX_AT.0, BOOSTS_TAX_AT.1, font::TEXT);
        pen.body(canvas, x, BOOSTS_TAX_AT.1, &format!(" {bonus} %"), font::TEXT);
        pen.eng(canvas, GROUP, START_CONSTRUCTION, START_AT.0, START_AT.1, font::TEXT);

        pen.window(
            canvas,
            BARRACKS_PLAQUE.0,
            BARRACKS_PLAQUE.1,
            BARRACKS_PLAQUE.2,
            BARRACKS_PLAQUE.3,
            0,
        );
        let cap = industry::garrison_cap(t, castle_type);
        pen.eng(canvas, GROUP, BARRACKS_FOR, BARRACKS_AT.0, BARRACKS_AT.1, font::TEXT);
        // `Ui_DrawNumber(cap, '@', &DAT_004D4160, 0xC, 0xE2, …)` — and that
        // suffix is **one space**, not a NUL, so the string is `"@40 "`. The old
        // `number(…, true)` drew `"40 "`: the space was right by accident and
        // the lead was missing, so the digits **and** *"troops."* both sat four
        // pixels left. **[V]**
        let (gx, gy) = GARRISON_AT;
        let x = pen.number_in(shell::Face::Body, canvas, gx, gy, cap, '@', " ", font::TEXT);
        pen.eng(canvas, GROUP, TROOPS, x + 2, GARRISON_AT.1, font::TEXT);

        pen.system_frame(canvas, THUMB_UP + usize::from(self.press.is_pressed(0)), OK.x, OK.y);
        pen.system_frame(
            canvas,
            THUMB_DOWN + usize::from(self.press.is_pressed(1)),
            CANCEL.x,
            CANCEL.y,
        );

        if !have_backdrop {
            for strip in 0..5usize {
                let r = type_rect(strip);
                let colour = if strip == level { ink.highlight } else { ink.dim };
                text::draw(canvas, r.x + 4, r.y + 8, &format!("{}", strip + 1), colour);
            }
            text::draw(canvas, 4, 470, "CAS_BACK.PL8 IS NOT INSTALLED - STRIPS ARE OURS", ink.dim);
        }
    }
}

/// `FUN_0040328E`'s wrap, measured in the **heading** font.
fn heading_wrap(a: &crate::shell::ShellAssets, s: &str, width: i32) -> Vec<String> {
    let measure = |t: &str| -> i32 {
        match &a.heading {
            Some(f) => f.width(t),
            None => l2_view::text::width(t),
        }
    };
    let mut out: Vec<String> = Vec::new();
    let mut line = String::new();
    for word in s.split_whitespace() {
        let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if !line.is_empty() && measure(&next) > width {
            out.push(std::mem::take(&mut line));
            line = word.to_string();
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}

