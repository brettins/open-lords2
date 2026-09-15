#![allow(unused_imports)]
use super::*;
use super::helpers::*;
use super::*;
use super::ui::*;
use super::*;

/// **`Screen_ConfirmBox` (`0x0040CCFA`) over the campaign map** — the same box
/// the battlefield draws, at `Ui_OpenConfirm`'s own `(0xA0 − 0x10, 0xA0 − 0x10)`
/// with `g_confirmWidgets`' frames 29 and 31.
///
/// The words are `L2.eng` group 10 index 5 — *"Combine armies?"* — read from
/// the group, never typed here (rule 6).
pub(crate) fn draw_combine_box(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx) {
    if ctx.game.combine_ask.is_none() {
        return;
    }
    let p = Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    };
    p.window(canvas, CONFIRM_BOX.x, CONFIRM_BOX.y, CONFIRM_COLS, CONFIRM_ROWS, BOX_SET);
    let s = ours_or(ctx.assets.shell.text(GROUP_CONFIRM, COMBINE_PROMPT), "COMBINE ARMIES?");
    p.body(canvas, CONFIRM_BOX.x + 0x20, CONFIRM_BOX.y + 0x20, &s, font::TEXT);
    for (i, (w, label)) in CONFIRM_WIDGETS.iter().zip(["YES", "NO"]).enumerate() {
        let frame = if i == 0 { CONFIRM_YES_FRAME } else { CONFIRM_NO_FRAME };
        // `Widget_Draw` (`0x0040CFD2`): the pressed picture is `base + 1`.
        let frame = if screen.press.is_pressed(i) { frame + 1 } else { frame };
        let drawn =
            ctx.assets.chrome.as_ref().is_some_and(|c| c.draw_system(canvas, frame, w.rect.x, w.rect.y));
        if !drawn {
            crate::widget::button(canvas, &ctx.assets.ink, w.rect, label, false);
        }
    }
}

pub(crate) fn draw_units(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip) {
    let ink = &ctx.assets.ink;
    for id in units_in_paint_order(ctx.game) {
        let Some(unit) = ctx.game.kingdom.campaign.units.get(id) else { continue };
        if ctx.game.hides_tile(l2_kingdom::map::index(unit.x, unit.y)) {
            continue;
        }
        let Some((cx, cy)) =
            campaign::tile_centre(screen.view, &screen.zoom, unit.x as usize, unit.y as usize)
        else {
            continue;
        };
        let h = unit_marker_half(&screen.zoom, unit);
        let debug = ctx.game.prefs.debug_overlay;
        let drawn = !unit.is_garrisoned()
            && campaign::draw_unit(
                canvas,
                &ctx.assets.map,
                screen.view,
                &screen.zoom,
                (unit.x as usize, unit.y as usize),
                unit_sprite(&screen.zoom, ctx.game, id, unit),
                clip,
            );
        if !drawn && (debug || !unit.is_garrisoned()) {
            let colour = ink.realm.get(unit.owner as usize).copied().unwrap_or(ink.dim);
            fill_clipped(canvas, cx - h - 1, cy - h - 1, h * 2 + 3, ink.background, clip);
            fill_clipped(canvas, cx - h, cy - h, h * 2 + 1, colour, clip);
            if unit.is_garrisoned() {
                fill_clipped(
                    canvas,
                    cx - h + 1,
                    cy - h + 1,
                    (h * 2 - 1).max(1),
                    ink.background,
                    clip,
                );
            }
        }
        if debug && screen.selected_unit == Some(id) {
            let r = h + 3;
            widget::frame(canvas, Rect::new(cx - r, cy - r, r * 2 + 1, r * 2 + 1), ink.highlight);
        }
        // is gone. The original marks a siege over the *castle*, not over the
        // army: `Flags1a.pl8` frame `0x82` with the seasons left under it,
        // `FUN_00407F82` called from `Sprite_TopIt`'s castle arm. It is built,
        // in [`draw_flags`], on the tile the original puts it on.
    }
}

/// **`FUN_004071A0`'s two flags** — the county town's owner-coloured banner and
/// the castle's garrison banner, both waving.
///
/// A player who has played the original: *"each county's town square would have
/// a coloured flag waving on it, and castles with armies in them have a flag."*
/// Both are in `FUN_004071A0` (`0x004071A0`), the pass `Map_DrawFrame` runs
/// between the terrain
/// **bank bit `0x80`** — which `County_FindTownTile` and `County_FindCastleTile`
/// set on their anchor quadrants. The branch inside then splits on plane 0:
///
/// **`docs/screens.md` §5 attributed all of this to `FUN_004081A6`, which is
/// not the flag at all** — it is the gold path-preview ball, and its `bank`
/// bit `0x40` is the path mark `Path_MarkPreviewTiles` sets. C49.
pub(crate) fn draw_flags(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip) {
    let k = &ctx.game.kingdom;
    let phase = screen.flag_phase;
    fn flag(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip, tile: usize, frame: usize) {
        let (x, y) = l2_kingdom::map::coords(tile);
        campaign::draw_flag(
            canvas,
            &ctx.assets.map,
            screen.view,
            &screen.zoom,
            (x as usize, y as usize),
            frame,
            clip,
        );
    }
    let lit = |tile: &usize| !ctx.game.hides_tile(*tile);
    for id in k.county_ids() {
        let county = &k.counties[id];
        let shield = k.realms.get(county.owner as usize).map_or(0, |r| r.shield_index);
        if let (Some(nw), Some(frame)) = (
            MapScreen::town_quadrant(ctx, id as u8, 0).filter(lit),
            campaign::flag_frame(shield, phase),
        ) {
            flag(screen, canvas, ctx, clip, nw, frame);
        }
        // **It goes through its own painter**, because its offset is not the
        // banner's: `(+0x10, −0x12)` at the near zoom against the banner's
        // `(+0x1A, −0x1C)`. Sharing [`campaign::draw_flag`] is how it came to be
        // ten pixels out.
        if county.mercenary_offer != 0 {
            if let Some(tile) = MapScreen::town_quadrant(ctx, id as u8, 2).filter(lit) {
                let (x, y) = l2_kingdom::map::coords(tile);
                campaign::draw_mercenary_marker(
                    canvas,
                    &ctx.assets.map,
                    screen.view,
                    &screen.zoom,
                    (x as usize, y as usize),
                    clip,
                );
            }
        }
        if county.castle_type == 0 || county.garrison_unit == 0 {
            continue;
        }
        let castle = MapScreen::settlements(ctx, id as u8)
            .into_iter()
            .find(|&t| industry::map_toggle_for_graphic(k.campaign.map.terrain[t])
                == Some(industry::MapToggle::Castle));
        let Some(tile) = castle.filter(lit) else { continue };
        // **The besieger's mark goes first, under the garrison's banner** —
        // `Sprite_TopIt` calls `FUN_00407F82` before it sets the banner's frame,
        // and it reads the *besieging* unit through the garrison:
        //
        // ```c
        // besieger = g_units[county.garrisonUnit].besiegedBy;
        // if (besieger != 0 && g_mapZoom == 0)
        //     FUN_00407f82(g_units[besieger].siegeSeasonsLeft, 8, -0x38);
        // ```
        let besieger = k.campaign.units.get(county.garrison_unit).map_or(0, |u| u.besieged_by);
        if besieger != 0 {
            let seasons =
                k.campaign.units.get(besieger as usize).map_or(0, |u| u.siege_seasons_left);
            draw_besieger_mark(screen, canvas, ctx, clip, tile, i32::from(seasons));
        }
        let garrison_shield =
            k.campaign.units.get(county.garrison_unit).map_or(0, |u| u.shield);
        let Some(frame) = campaign::flag_frame(garrison_shield, phase) else { continue };
        flag(screen, canvas, ctx, clip, tile, frame);
    }
}

/// **`FUN_00407F82`'s two draws** — the mark, then the count under it.
///
/// `&g_fontBody`, colour `0xF9`, and **flat** — `DAT_005AEA40 = 1` is set for
/// the `Ui_DrawNumberRight` and cleared after it, which is the emboss kill, so
/// this is not the map's ordinary shadowed body text.
pub(super) fn draw_besieger_mark(
    screen: &MapScreen,
    canvas: &mut Canvas,
    ctx: &Ctx,
    clip: Clip,
    tile: usize,
    seasons: i32,
) {
    let (x, y) = l2_kingdom::map::coords(tile);
    let Some((nx, ny, w)) = campaign::besieger_marker(
        canvas,
        &ctx.assets.map,
        screen.view,
        &screen.zoom,
        (x as usize, y as usize),
        clip,
    ) else {
        return;
    };
    let pen = Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: None,
        caps: None,
    };
    // `Ui_DrawNumberRight(seasons, ' ', " ", …)`, and both of those are the
    // call site's: `DAT_004D2094` is a single space.
    pen.number_centred(canvas, nx, ny, w, seasons, ' ', " ", campaign::BESIEGER_COUNT_INK);
}

/// **`FUN_004071A0`'s farm arm — the cattle in the pastures.**
pub(crate) fn draw_herds(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip) {
    let map = &ctx.game.kingdom.campaign.map;
    for tile in 0..map.terrain.len() {
        if map.flags[tile] & l2_kingdom::map::flags::FARMLAND == 0 {
            continue;
        }
        if ctx.game.hides_tile(tile) {
            continue;
        }
        let (x, y) = l2_kingdom::map::coords(tile);
        campaign::draw_herd(
            canvas,
            &ctx.assets.map,
            screen.view,
            &screen.zoom,
            (x as usize, y as usize),
            map.terrain[tile],
            screen.herd_phase,
            clip,
        );
    }
}

/// **`Path_MarkPreviewTiles` (`0x004A91BA`) and `Map_DrawPathMarker`** — the
/// gold balls along an ordered path.
///
/// The small dot is the fallback for an install with no `Flags1a.pl8`, and for
/// the placeholder assets the tests use — bright while the army can still reach
/// the tile this season, dim beyond that, greying at exactly the step the
/// original greys at. `docs/decisions.md` C21.
///
/// *"There is no red X when I click to tell an army to move."* `Screen_DrawWidgets`
/// (`0x004BA26E`) `0x10` arm is one statement, `Map_HoverUnitTarget()`. That calls
/// `Path_MarkPreviewTiles` (`0x004A91BA`), the binary's only writer of tile bank bit
/// `0x40`, and `Map_DrawPathMarker` (`0x004081A6`) clears the bit as it draws
/// (`bank = bank & 0xbf`), so the trail is one frame and only on screen `0x10`.
///
/// Every `g_flagsSheet` frame the binary names: `shield * 8 - 8 + phase` (`0x00 … 0x27`,
/// `Map_DrawArmies` `0x00408438`), `0x38 + cost` and `0x4E` (`Map_DrawPathMarker`),
/// `DAT_0057D390 + 0x79` (the merchant, `Map_DrawArmies`), `0x81`
/// (`Pl8_DrawFrameClipped(g_flagsSheet, 0x81, …)`), `0x82` (`FUN_00407F82`). `FUN_00407E38`
/// takes a frame parameter and has 0 callers. No cross, no destination marker, and
/// `g_cursorByScreen[0x10]` (`0x004E3098`) is kind 14, `g_cursorScythe` (`0x004EABE4`) —
/// not `g_cursorCross` (`0x004E659C`). **[V]**, rule 5: we could not find it.
pub(crate) fn draw_path_preview(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip) {
    let ink = &ctx.assets.ink;
    let Some(sel) = screen.move_order.as_ref() else { return };
    let Some(unit) = ctx.game.kingdom.campaign.units.get(sel.unit) else { return };
    let left_at_start = unit.moves_left();
    for &(x, y) in &sel.path {
        let spent = sel.field.cost_to(x, y).unwrap_or(0);
        let in_range = spent <= left_at_start;
        let action = path_marker_is_action(&ctx.game.kingdom.campaign.map, x, y);
        let drawn = campaign::draw_path_marker(
            canvas,
            &ctx.assets.map,
            screen.view,
            &screen.zoom,
            (x as usize, y as usize),
            campaign::path_marker_frame(spent, in_range, action),
            clip,
        );
        if drawn {
            continue;
        }
        let Some((cx, cy)) = campaign::tile_centre(screen.view, &screen.zoom, x as usize, y as usize)
        else {
            continue;
        };
        let colour = if in_range { ink.highlight } else { ink.dim };
        fill_clipped(canvas, cx - 1, cy - 1, 3, colour, clip);
    }
}

/// The original's army panel is `UnitPanel_Draw` (`0x0041B19D`) with `L2.eng`
/// group 31's own field labels beside the record's offsets — *Wages*, *Formed*,
/// *Morale*, *N moves left.*, the supply line
/// `0x04`, it is a **shell**, and a right-click is how the original opens it —
/// which is a different gesture on a different screen from this one.
pub(crate) fn draw_unit_banner(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx) {
    if !ctx.game.prefs.debug_overlay {
        return;
    }
    let Some(id) = screen.selected_unit else { return };
    let Some(unit) = ctx.game.kingdom.campaign.units.get(id) else { return };
    let ink = &ctx.assets.ink;
    let bar = Rect::new(0, NEAR.bottom() - 26, PANEL_X, 26);
    widget::panel(canvas, ink, bar);
    // `L2.eng` 31/22 prints `moveAllowance - movesUsed` as "moves left."
    let home = unit.home_county;
    text::draw(
        canvas,
        4,
        bar.y + 4,
        &format!(
            "#{id} {} - {} MEN, {} MOVES LEFT, MORALE {}",
            unit.kind.name().to_uppercase(),
            unit.men,
            unit.moves_left(),
            unit.morale,
        ),
        ink.text,
    );
    let where_to = if unit.besieging_county != 0 {
        format!("BESIEGING COUNTY {}", unit.besieging_county)
    } else if unit.garrison_county != 0 {
        format!("GARRISONING COUNTY {}", unit.garrison_county)
    } else if !unit.path.is_empty() {
        format!("{} STEPS TO GO", unit.path.len())
    } else {
        "IDLE - CLICK A TILE TO MARCH, A FOR ORDERS".into()
    };
    text::draw(canvas, 4, bar.y + 14, &format!("FROM COUNTY {home}. {where_to}"), ink.dim);
}


