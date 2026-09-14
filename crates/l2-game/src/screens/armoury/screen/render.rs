#![allow(unused_imports)]
use super::*;
use super::armoury::*;
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

