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
pub(super) fn draw_combine_box(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx) {
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

pub(super) fn draw_units(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip) {
    let ink = &ctx.assets.ink;
    for id in units_in_paint_order(ctx.game) {
        let Some(unit) = ctx.game.kingdom.campaign.units.get(id) else { continue };
        // **`Map_DrawArmies`' whole body is inside the fog test**:
        // `if (g_optExploration != 1 || (tile.bank & 0x20) != 0) { ...every unit
        // on the tile... }`. A unit in the dark is not drawn — not its figure,
        // not its banner — and so neither are the marks this function adds of
        // its own. Its *click* is not gated: no arm reads the bit.
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
        // A garrisoned unit is inside the castle. The original does not draw it
        // on the map at all — `draw_flags` flies the garrison's banner over the
        // castle instead — so ours draws nothing either.
        // that used to say "your garrison is in there" is **debug overlay
        // only**. Never the figure, which would say it was standing outside.
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
        // The square is the fallback for an install with no sprite sheet, and a
        // normal install never reaches it — except for a garrison, which is the
        // overlay's.
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
        // The selection ring: `g_selectedUnit`.
        // for it. **Ours**, debug overlay only — `Map_DrawArmies` draws the
        // figure and its banner and nothing round them; the original's answer
        // to "which army is picked" is the path under the cursor
        // (`draw_path_preview`).
        if debug && screen.selected_unit == Some(id) {
            let r = h + 3;
            widget::frame(canvas, Rect::new(cx - r, cy - r, r * 2 + 1, r * 2 + 1), ink.highlight);
        }
        // **A besieger's mark is not drawn here**.
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
/// ```c
/// if      (flags & 0x40)  /* the town  */ { quadrant 0: county.shield;  quadrant 2: merc offer }
/// else if (flags & 0x80)  /* the castle*/ { if (content <= 0x14 || !county.garrisonUnit) return;
///                                           shield = units[county.garrisonUnit].shield; }
/// frame = shield * 8 - 8 + phase;
/// ```
///
/// Three things worth stating because each is a decision:
///
/// * **The colour is the frame index**, not a palette remap — `Flags1a.pl8`'s
///   first forty frames are five shields by eight wave phases, and `shield = 5,
///   phase = 7` lands on the fortieth exactly.
/// * **The castle flag carries the *garrison's* shield, not the county's.** A
///   captured castle whose garrison is still somebody else's flies the
/// garrison's colours,
/// * **`content == 0x14` returns**: `0x14` is the bare castle plot and
///   `0x15 … 0x19` are castle types 1 … 5, so an unbuilt castle flies nothing
///   even with a garrison standing on it.
///
/// **`docs/screens.md` §5 attributed all of this to `FUN_004081A6`, which is
/// not the flag at all** — it is the gold path-preview ball, and its `bank`
/// bit `0x40` is the path mark `Path_MarkPreviewTiles` sets. C49.
pub(super) fn draw_flags(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip) {
    let k = &ctx.game.kingdom;
    let phase = screen.flag_phase;
// A free function; the mercenary arm below needs the
    // canvas too, and a closure that captured it would hold the borrow for the
    // whole loop.
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
    // **`Sprite_TopIt`'s first statement is the fog test**, before it looks at
    // a single flag bit: `if (g_optExploration == 1 && (tile.bank & 0x20) == 0)
    // return 0;`. Every arm below is behind it, on the tile it would draw on,
    // so a town in the dark flies nothing
    // garrison — and its owner is not given away by the banner.
    let lit = |tile: &usize| !ctx.game.hides_tile(*tile);
    for id in k.county_ids() {
        let county = &k.counties[id];
        // The town's 2 x 2 block. Plane-3 quadrant 0 is its north-west tile —
        // the lowest tile index.
        let shield = k.realms.get(county.owner as usize).map_or(0, |r| r.shield_index);
        if let (Some(nw), Some(frame)) = (
            MapScreen::town_quadrant(ctx, id as u8, 0).filter(lit),
            campaign::flag_frame(shield, phase),
        ) {
            flag(screen, canvas, ctx, clip, nw, frame);
        }
        // **`county.mercenaryOffer != 0` puts frame `0x81` on quadrant 2**, the
        // tile one map row south of the origin — a standing band, advertised on
        // the map. See [`MapScreen::town_quadrant`] for why that is `town()[2]`
        // and not `town()[1]`, which is what this drew and which is a tile the
        // original's overlay pass never runs on at all.
        //
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
        // The castle: built, and holding a garrison.
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
        //
        // So the count belongs to the army camped outside and is written on the
        // castle it is camped outside of. Nothing here is drawn when the
        // garrison is free.
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
/// [`campaign::besieger_marker`] blits the frame and answers where the number
/// goes; this puts the number there. The pen is the function's own:
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
///
/// A player who had the build in front of him: *"why do the pastures not have
/// cows in them?"* Because this pass had three of its four arms and not the
/// fourth. It is the same overlay pass as [`draw_flags`], on the same bank bit
/// `0x80`, off the same `Flags1a.pl8` — `Terrain_Set` sets that bit for
/// `0x0E < terrain < 0x17`,
/// the one field state that gets a second blit at all.
///
/// **Which of three pictures is drawn is `herd ÷ fieldsCattle`**, so this is a
/// rule wearing a graphic's clothes: `l2_kingdom::land::herd_graphic` bands the
/// density at 11 and 21 and `Herd_UpdateCrowding` writes the answer onto every
/// pasture tile of the county. The terrain byte carries it, this reads it back,
/// and neither end guesses. An empty herd is terrain `0x13` and draws nothing.
///
/// The phase is [`MapScreen::herd_phase`] and is **display state**: it never
/// reaches [`l2_kingdom::Kingdom`].
pub(super) fn draw_herds(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip) {
    let map = &ctx.game.kingdom.campaign.map;
    for tile in 0..map.terrain.len() {
        if map.flags[tile] & l2_kingdom::map::flags::FARMLAND == 0 {
            continue;
        }
        // `Sprite_TopIt`'s fog test, ahead of the farm arm like every other.
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
/// The original sets bit `0x40` of each path tile's runtime record and then
/// draws `g_flagsSheet` frame `0x38 + cost` on every tile carrying it, so the
/// frame index *is* the accumulated cost and everything past the remaining
/// budget collapses to frame `0x38`. `docs/armies.md` §2.3.
///
/// **The balls are the game's own art now.** A player who has played it:
/// *"there are colored dot images for the army walking dots"* — and they are
/// `Flags1a.pl8` frames `0x38 … 0x4D`, 23 pictures of one 15 × 15 ball in
/// rising amounts of colour, indexed by the **accumulated cost** of reaching
/// that tile. Nothing about the realm, the shield or the unit's kind selects
/// the colour; see [`campaign::path_marker_frame`], which carries the
/// measurement that settled it.
///
/// The small dot is the fallback for an install with no `Flags1a.pl8`, and for
/// the placeholder assets the tests use — bright while the army can still reach
/// the tile this season, dim beyond that, greying at exactly the step the
/// original greys at. `docs/decisions.md` C21.
///
/// **This is the feedback the player was missing.** `Unit_OrderMove` writes
/// nothing at all when no path is found, and an *unreachable* destination is an
/// accepted order with an empty path —
/// perfectly good one all looked the same on screen. The path is the original's
/// own answer to that, and drawing it is how a player tells our bug from his own
/// mis-click.
pub(super) fn draw_path_preview(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx, clip: Clip) {
    let ink = &ctx.assets.ink;
    let Some(sel) = screen.move_order.as_ref() else { return };
    let Some(unit) = ctx.game.kingdom.campaign.units.get(sel.unit) else { return };
    let left_at_start = unit.moves_left();
    // **The route under the cursor, not the route already ordered.** This loop
    // used to walk `unit.path` — the *committed* path — and re-derive each
    // step's cost by hand from the road flag. Both were wrong in the same way:
    // the original never draws balls for an order that has been placed (the
    // only writer of the bank bit runs only on screen `0x10`), and it never
    // re-derives a cost, because the flood fill already holds one.
    //
    // `local_c = g_moveDistLocal[tile] - 1`, and `if (allowance - used <
    // local_c) local_c = 0` — a step past the army's remaining moves draws
    // frame `0x38`, the one recolouring in the run with no colour in it.
    for &(x, y) in &sel.path {
        let spent = sel.field.cost_to(x, y).unwrap_or(0);
        let in_range = spent <= left_at_start;
        // `local_14`: the gold ball on a tile a click would act on, tested
        // before the cost is looked at. See [`path_marker_is_action`].
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

/// **Ours, and it is deliberately not in the right column.**
///
/// The original's army panel is `UnitPanel_Draw` (`0x0041B19D`) with `L2.eng`
/// group 31's own field labels beside the record's offsets — *Wages*, *Formed*,
/// *Morale*, *N moves left.*, the supply line
/// `0x04`, it is a **shell**, and a right-click is how the original opens it —
/// which is a different gesture on a different screen from this one.
///
/// This is one line of the same numbers, drawn in our font at the bottom-left
/// of the viewport while an army is selected, so that a player *placing a march
/// order* can see what he is ordering without leaving move-order mode. The
/// right column belongs to the county strip. When `0x04` graduates, this stays:
/// they answer different questions.
pub(super) fn draw_unit_banner(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx) {
    // Debug overlay only: the original draws nothing over the foot of the map.
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


