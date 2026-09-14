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

/// Half the side of a unit's marker, in pixels.
///
/// **The three size classes are the original's** — `Army_Tick` picks sprite
/// bank `0x48`, `0x60` or `0x78` at **301 and 601 men**, which the player
/// described as one, two or three figures (`docs/armies.md` §2.4) —
/// marker grows with them so that the same thing is legible. **The square is
/// ours**: `Sprite1a.pl8` holds the actual figures and we do not place them.
pub(super) fn unit_marker_half(zoom: &Zoom, unit: &l2_kingdom::Unit) -> i32 {
    let base = if zoom.id == FAR.id { 1 } else { 3 };
    base + unit.size_class() as i32
}

/// **What `Map_DrawArmies` (`0x00408438`) draws one unit with**: the sheet; the
/// frame its tick handler wrote on the way into the last sweep
/// ([`crate::game::UnitFrames`]); the per-kind nudge;
/// its facing and `+0x149`, which is how far across its tile it is
/// ([`campaign::walk_offset`]).
///
/// One function for the painter
/// across a tile is clicked where it is seen.
pub(super) fn unit_sprite(zoom: &Zoom, game: &crate::game::Game, id: usize, unit: &l2_kingdom::Unit) -> campaign::UnitSprite {
    campaign::UnitSprite {
        sheet: unit.sprite_sheet(),
        frame: game.unit_frame(id, unit),
        nudge: unit.sprite_nudge(),
        walk: campaign::walk_offset(zoom, unit.facing, unit.sub_tile),
    }
}

/// **`Map_DrawArmies` (`0x00408438`)** — every unit on the map, as the figure
/// the original draws.
///
/// The sheet, the frame
/// `Sprite1a.pl8` for armies, mobs **and merchants**, `Sprite1b.pl8` for
/// transports alone, `frame = bank + 3*((facing+1)&7) + walk` for the first two
/// and `6*((facing+1)&7) + phase` for the other two, anchored on the tile's
/// bottom vertex, then **dragged back toward the tile it is leaving** by the
/// walk tables. See [`unit_sprite`], [`l2_kingdom::Unit::sprite_frame`] and
/// [`campaign::walk_offset`]. This used to leave the walk tables out as needing
/// *"a sub-tile step counter we do not keep"*; `docs/decisions.md` C134 gave
/// the unit that counter and nothing here read it, so every army jumped from
/// tile to tile.
///
/// **What is still ours** is the *selection*: the original shows a selected
/// army by flood-filling its reachable tiles. A garrisoned unit is drawn hollow
/// because it is inside the castle, not on the tile — the
/// original draws it not at all and flies a flag over the castle instead (see
/// [`draw_flags`], which also carries the besieger's mark).
///
/// The square marker is the fallback for an install with no `Sprite?a.pl8`, and
/// for the placeholder assets the tests use. It says *there is something here*
/// without claiming to be the game's art. `docs/decisions.md` C21.
/// **The order `Map_DrawArmies` (`0x00408438`) is called in** — which is not
/// the order the unit array is in, and that was the defect.
///
/// The army pass is `FUN_00405487` (`0x00405487`) and it does not walk
/// `g_units` at all. It walks the **lattice**: the top aligned row's `cols`
/// cells, then `FUN_004059AF` / `FUN_00405862` alternating down the viewport,
/// each cell setting `g_tileCursor` and calling `Map_DrawArmies`, whose body is
/// a list walk over that one tile —
///
/// ```c
/// local_28 = g_tiles[g_tileCursor].unit;
/// for (; local_28 != 0; local_28 = g_units[local_28].field_0x4) { …draw… }
/// ```
///
/// So the painter's order is **lattice row, then column, then the tile's own
/// list**, and a figure standing on a lower row is painted over one standing
/// behind it whatever their array slots are. Ours drew in array order, so which
/// of two overlapping armies was on top was decided by which had the lower id —
/// a unit that marched *behind* another could be drawn in front of it.
/// answer flipped when a slot was reused.
///
/// `l2_view::campaign::tile_to_cell` is the lattice address, so sorting by it
/// **is** the walk: the walk visits every cell of a row left to right and every
/// row top to bottom, and skips nothing a unit could be standing on.
///
/// **`[D]` on the within-tile tie-break.** The original's is the tile's linked
/// list, which is insertion order and which this crate does not keep; ascending
/// id is what stands in for it. It decides only which of two units *on the same
/// tile* is on top.
pub fn units_in_paint_order(game: &crate::game::Game) -> Vec<usize> {
    let mut order: Vec<(i32, i32, usize)> = game
        .kingdom
        .campaign
        .units
        .iter()
        .map(|(id, u)| {
            let (row, col) = campaign::tile_to_cell(u.x as usize, u.y as usize);
            (row, col, id)
        })
        .collect();
    order.sort_unstable();
    order.into_iter().map(|(.., id)| id).collect()
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

/// **`Map_DrawPathMarker`'s `local_14` (`0x004081A6`) — is this a tile a click
/// would act on?**
///
/// ```c
/// local_14 = flags & 0x50;                                   /* the town, or a dwelling plot */
/// if ((flags & 0x80) != 0 && content != 0x14) local_14 = 1;  /* a site or a castle, not a bare plot */
/// ```
///
/// The plane-0 bits
/// the reach, not a unit standing there. So your own town is gold, a town past
/// the budget is gold, and an enemy army on open ground is coloured by its cost
/// like any other tile — which is narrower than *"an attack"*, and is the
/// original's. `Map_HoverUnitTarget` asks the same bits separately, with an
/// owner test, to decide what a click would *do*; the ball does not.
pub(super) fn path_marker_is_action(map: &l2_kingdom::map::CampaignMap, x: u8, y: u8) -> bool {
    use l2_kingdom::map::{flags, terrain};
    let tile = l2_kingdom::map::index(x, y);
    let f = map.flags[tile];
    f & (flags::CASTLE | flags::PLOT) != 0
        || (f & flags::SETTLEMENT != 0 && map.terrain[tile] != terrain::CASTLE_PLOT)
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

/// What a settlement click switched, in words. **Ours** — the original enqueues
/// one of `L2.eng`'s "mining stopped / started" messages instead.
pub(super) fn toggle_name(what: industry::MapToggle) -> &'static str {
    match what {
        industry::MapToggle::Industry(c) => match c {
            l2_kingdom::Commodity::Wood => "WOOD CUTTING",
            l2_kingdom::Commodity::Iron => "IRON MINING",
            l2_kingdom::Commodity::Weapons => "THE SMITHY",
            l2_kingdom::Commodity::Stone => "STONE QUARRYING",
        },
        industry::MapToggle::Castle => "CASTLE BUILDING",
    }
}

/// **Ours.** One colour per field use,
/// without a legend.
pub(super) fn field_colour(ink: &Ink, kind: FieldType) -> u8 {
    match kind {
        FieldType::Grain => ink.good,
        FieldType::Pasture => ink.highlight,
        FieldType::Fallow => ink.dim,
        FieldType::Reclaiming => ink.panel,
        FieldType::Waste => ink.bad,
    }
}

/// A filled square
/// cannot spill into the panel.
pub(super) fn fill_clipped(canvas: &mut Canvas, x: i32, y: i32, side: i32, colour: u8, clip: Clip) {
    for yy in y..y + side {
        for xx in x..x + side {
            if clip.contains(xx, yy) {
                canvas.set(xx as usize, yy as usize, colour);
            }
        }
    }
}

/// `Ui_DrawYear(g_year, 0x168, 6, 3)` — where the year starts.
/// one of the bar's three readings sits on.
pub(super) const CLOCK_X: i32 = 0x168;
pub(super) const CLOCK_Y: i32 = 6;
/// `Eng_DrawString(0x1D, g_season, g_penAdvance + 0x16C, …)` — the season's
/// **base**, which the year's own width is added to. Four pixels right of
/// [`CLOCK_X`], and that four is the original's, not a rounding of ours.
pub(super) const SEASON_X: i32 = 0x16C;
/// `L2.eng` group 29: `"No Season"`, `"Spring"`, `"Summer"`, `"Autumn"`,
/// `"Winter"` — indexed by `g_season` with no adjustment.
pub const SEASON_GROUP: usize = 29;
/// `Ui_DrawCount(gold, 0, 500, 6, …)` — the treasury, and its group 8 noun
/// index. 0/1 is *"Crown."* / *"Crowns."*.
pub(super) const GOLD_X: i32 = 500;
pub(super) const GOLD_NOUN: usize = 0;

/// `Screen_DrawMenuBar`, as far as we can reproduce it.
///
/// **The original's:** the 640 × 24 background tiled from `Panels.pl8` frames
/// 196 + (c mod 8) — 25 cells from x 0 and 2 more from x 592 — and one 13 × 16
/// `Misc_cty` banner per live realm at `x = 270 + 16i, y = 4`.
///
/// **The File / Options / Help titles are drawn now**, out of `L2.eng` groups
/// 1, 2 and 3 index 0, measured the way `Ui_DrawMenuTitles` (`0x0040C5B0`)
/// measures them — see [`menubar`](crate::screens::menubar). This comment used
/// to say they were not, which was true and was nineteen input arms.
///
/// **And so are the year, the season
/// line here: *"ours: the clock and treasury are our font at the original's x
/// positions."* A player read that off the screen —
///
/// > *"still placeholder font in the top right for gold and summer"*
///
/// — and he was looking at two `l2_view::text::draw` calls in the 5 × 7 debug
/// font
/// `Screen_DrawMenuBar` back turned up three things beyond the face:
///
/// ```c
/// g_penAdvance = 0;
/// Ui_DrawYear(g_year, 0x168, 6, 3);
/// Eng_DrawString(0x1D, g_season, g_penAdvance + 0x16C, 6, &g_fontBody, 0x3F);
/// ...
/// Ui_DrawCount(g_realms[g_localPlayer].gold, 0, 500, 6, &g_fontBody, 0x3F);
/// ```
///
/// * **the year comes first
///   than by a coordinate. We drew `"{season} {year}"`, in that order, at 360.
/// * **the treasury is a count, not a caption**: `Ui_DrawCount(gold, 0, …)`
///   draws the number and then `L2.eng` group 8's *"Crown."* / *"Crowns."*. We
///   drew the word `GOLD`, which is not in `L2.eng` at all.
/// * **`g_fontBody` is `Fntl2_14.pl8`**, one of the game's two *blackletter*
///   faces — so "the right font" here is the display one, not the plain one,
///   which is the opposite of where [`crate::build_id`] lands and worth stating
///   because the instinct is to reach for legibility. Verified twice:
/// `docs/symbols.md` `0x005AF8F0`,
///   gives `fntl2_14.pl8` a buffer of `0x36B0` bytes, which is exactly
///   `g_fontHeading - g_fontBody`.
///
/// # `battle` — the one branch inside this function, and it is not the map's
///
/// `Screen_DrawMenuBar` is **not the campaign map's painter**. Its guard is a
/// list of screen ids it *refuses* — `0x08 … 0x0D`, `0x17`, `0x1B … 0x20`,
/// `0x22`, `0x2C … 0x2F` — and `0x29`, `0x2A` and `0x2B`, the three battle
/// screens, are in none of them. **[V]** So the bar is up through a battle, and
/// the function carries the difference itself:
///
/// ```c
/// Ui_DrawTileStrip(0, 0, 0x19, 1);  Ui_DrawTileStrip(0x250, 0, 2, 1);
/// Ui_DrawBevelRect(0, 0, 0x280, 0x18);
/// if (g_battlePhase == 0) { ...the realm banners... }
/// Ui_DrawMenuTitles(&g_menuBarItems, 3);
/// if (g_battlePhase == 0) { Ui_DrawYear(...); Eng_DrawString(0x1D, g_season, ...); }
/// Ui_DrawCount(g_realms[g_localPlayer].gold, 0, 500, 6, &g_fontBody, 0x3F);
/// ```
///
/// **Two guards, and neither is on a title.** What a battle takes off the bar
/// is the realm shields
/// treasury stay. `battle` is `g_battlePhase != 0`.
pub(crate) fn draw_menu_bar(canvas: &mut Canvas, ctx: &Ctx, battle: bool) {
    let ink = &ctx.assets.ink;
    let game = &ctx.game;
    let k = &game.kingdom;

    match &ctx.assets.chrome {
        Some(c) => {
            c.draw_menu_bar_background(canvas);
            // `Screen_DrawMenuBar` (`0x00419C78`): realms 1..=5 under
            // `strength != 0 && aiStep < 999`, banner at 270 + 16 * slot.
            //
            // **The shield row is the turn clock.** A realm's banner is up
            // while it has a turn still to play and goes the moment it ends
            // one — the person's own on the click, each AI's as it finishes —
            // and all of them come back when the next turn begins.
            // `turn::realm_turn_ended` is the second clause, and says why it
// is
            //
            // `slot` is `local_c`, which the original advances after every
            // `Pl8_DrawFrame` it calls — so the row closes up leftwards over a
            // realm that is out, and a frame that fails to load still holds
// its place. Neighbours shift off it.
            //
            // `if (g_battlePhase == 0)` wraps the whole loop,
            // takes every shield off the bar at once.
            let mut slot = 0;
            for id in 1..k.realms.len() {
                if battle {
                    break;
                }
                if !k.realms[id].in_play || turn::realm_turn_ended(game, id) {
                    continue;
                }
                let raw = game.realm_colour.get(id).copied().unwrap_or(0);
                let colour = chrome::realm_colour(raw);
                c.draw_banner(canvas, slot, colour);
                slot += 1;
            }
        }
        None => widget::panel(canvas, ink, Rect::new(0, 0, canvas.width as i32, TOP_BAR)),
    }

    // `Screen_DrawMenuBar` touches neither `DAT_005AEA40` nor `DAT_0058FE2C`
    // around these three, so it is the ordinary embossed body pen — the same
    // one `menubar::draw_titles` uses two lines below.
    let pen = crate::shell::Pen {
        assets: &ctx.assets.shell,
        ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    };

    // `if (g_battlePhase == 0) { Ui_DrawYear(...); Eng_DrawString(0x1D, ...) }`
    // — the second of the function's two battle guards, and it takes the clock
    //
    // original stops printing the date while one is being fought.
    if !battle {
        // `Ui_DrawYear(g_year, 0x168, 6, 3)` — style 3 is
        // `Ui_DrawNumber(year, ' ', &DAT_004D41F0, x, y, &g_fontBody, 0x3F)`,
        // the bare number with a leading and a trailing space and no BC/AD.
        let after_year = pen.year(canvas, CLOCK_X, CLOCK_Y, k.year, 3, font::TEXT);
        // `Eng_DrawString(0x1D, g_season, g_penAdvance + 0x16C, 6, &g_fontBody, 0x3F)`.
        //
        // **`g_penAdvance` is a width and `Pen::year` returns an absolute x** —
        // the confusion `docs/decisions.md` C61 records four agents making seven
// times. The subtraction is written out.
        // line reads the way the decompilation does.
        let advance = after_year - CLOCK_X;
        let season = season_text(ctx.assets, k.season);
        pen.body(canvas, SEASON_X + advance, CLOCK_Y, &season, font::TEXT);
    }
    // `Ui_DrawCount(g_realms[g_localPlayer].gold, 0, 500, 6, &g_fontBody, 0x3F)`
    // — the number, then group 8 index 0 or 1, *"Crown."* or *"Crowns."*.
    // `Ui_DrawCount` opens the number with `'@'`, so the digits start at 504,
    // not at 500; they were four pixels left until `Pen::count` stopped taking
    // a lead of its caller's.
    pen.count(canvas, GOLD_X, CLOCK_Y, game.gold(), GOLD_NOUN, font::TEXT);

    // `Ui_DrawMenuTitles(&g_menuBarItems, 3)`. Nothing here is open — the
    // drop-down is its own screen and draws its own title lit.
    menubar::draw_titles(ctx, canvas, None);

    // **OURS, and it has been evicted from the menu bar.**
    //
    // The turn number
    // which is where the original draws *File*, *Options* and *Help* — so two
    // lines of ours were sitting on the three words that are the way into every
// menu in the game.
// the bar holds three measured titles, up to five
    // 13 × 16 realm banners from x = 270, the clock at 360
    // 500, and every one of those is `Screen_DrawMenuBar`'s.
    //
    // So they go **under** the bar, on the map's own top-left corner, where
    // nothing of the original's is drawn. Still ours, still marked, and now they
    // cannot hide a control.
    //
    // And now debug overlay only: the original draws nothing there at all.
    if game.prefs.debug_overlay {
        text::draw(canvas, 6, 28, &format!("TURN {}", k.turn_count), ink.dim);
        let held = format!("COUNTIES {}/{}", game.owned_by(game.player), k.county_count);
        text::draw(canvas, 6, 38, &held, ink.dim);
    }
}

/// The right column: the original's seven `Misc_cty` frames, our numbers inside
/// them.
pub(super) fn draw_right_panel(screen: &MapScreen, canvas: &mut Canvas, ctx: &Ctx) {
    let ink = &ctx.assets.ink;
    let game = &ctx.game;
    let k = &game.kingdom;
    let own = game.selected != 0
        && (game.selected as usize) < k.counties.len()
        && k.counties[game.selected as usize].owner == game.player;

    match &ctx.assets.chrome {
        Some(c) => {
            c.draw_right_panel(canvas, own);
        }
        None => widget::panel(canvas, ink, PANEL),
    }

    // The minimap, tinted from `Lords2.exe`'s own ramps.
    if let Some(m) = &screen.minimap {
        let owner = |county: u8| -> u8 {
            let id = county as usize;
            if id == 0 || id >= k.counties.len() {
                return 0;
            }
            // Ramp row 0 is the unowned shading — the raster's own indices,
            // unchanged — so an unowned county must reach it, and only an owned
            // one goes through the 1..=5 clamp.
            match k.counties[id].owner as usize {
                0 => 0,
                realm => chrome::realm_colour(game.realm_colour.get(realm).copied().unwrap_or(0)),
            }
        };
        // **The three statistic overlays colour the local player's counties and
        // nothing else** — `Minimap_DrawOverlay` tests `owner == g_localPlayer`
        // in each of its three branches
        // rival's county keeps the raster's own grey.
        let band = |county: u8| -> Option<u8> {
            let id = county as usize;
            let c = k.counties.get(id)?;
            if id == 0 || c.owner != game.player {
                return None;
            }
            let bands = c.minimap_bands();
            Some(match screen.minimap_mode {
                MinimapMode::Labour => bands.labour,
                MinimapMode::Food => bands.food,
                MinimapMode::Happiness => bands.happiness,
                MinimapMode::Owner => return None,
            })
        };
        let tint = match screen.minimap_mode {
            MinimapMode::Owner => MinimapTint::Owner(&owner),
            _ => MinimapTint::Rating(&band),
        };
        chrome::draw_minimap(canvas, m, game.selected, &tint);
        if let Some(c) = &ctx.assets.chrome {
            // `Minimap_Draw` draws the strip and then the badge, both after the
            // overlay, so both sit on top of it.
            c.draw_minimap_side(canvas, screen.minimap_mode);
            c.draw_minimap_badge(canvas, screen.minimap_mode);
        }
    } else {
// No `MAPnn.PL8`: state it.
        text::draw_centred(canvas, PANEL_X + PANEL_W / 2, 84, "NO MINIMAP", ink.dim);
    }

    // **The county strip, at the original's own coordinates.** `Misc_cty` frame
    // 55 is the 162 x 94 plate at (478, 156) and `CountyStrip_Draw`
    // (`0x0040F7D3`) fills it: the county's name, its population and happiness,
    // the tax rate, the achieved ration — red when it is not the wanted one —
    //
    // `docs/screens-county.md` §2.1
    // screen, which draws the same plate.
    //
    // This plate used to be left empty while a box of ours went over the jobs
    // plate below it. The player was looking at our text where the game's own
// numbers belong, and at four blank quadrants that are the menu.
    if game.selected != 0 {
        // The strip.
        // `CountyStrip_Draw`'s, both shared with the county screen.
        county::draw_strip(ctx, canvas, game.selected, None);
    } else if game.prefs.debug_overlay {
        text::draw_centred(canvas, PANEL_X + 80, 200, "NO COUNTY SELECTED", ink.dim);
    }

    // **Ours, and it should look it.** `0x00438E3B` turns the 162 x 128 plate
    // at y = 302 into two columns of job rows — farm jobs left of x = 560,
    // industry right of it — and a click opens the job popup for that job
    // (`docs/screens-county.md` §2.4). We do not lay those rows out yet, so the
    // plate carries a dark box of ours with the county's stores in it
    // one status line this interface has. A stub that says so beats one that
    // looks finished.
    //
    // **Both columns are now drawn**, by `county::draw_produce_rows` and
    // `county::draw_industry_rows` with the rest of the strip. The left is
    // where the blue outline lives — the cow gains a ring the moment more
    // people are milking than the herd can use —
    // industry rows, which stood behind a box of ours reading
    // `INDUSTRY / NOT DRAWN` until the seventeen draw calls under it were
    // enumerated. The reason the box was there — *"three of its five rows are
    // flat icons
    // not settled"* — was checkable and wrong on both halves; see
    // `county::draw_industry_rows`.
    //
    // What is left here is the one status line this interface has, which is
    // ours and is marked so in §7's count.
    let x = PANEL_X + 8;
    if game.prefs.debug_overlay {
        text::draw(canvas, x, chrome::PANEL_OWN_C_Y + 110, &screen.status, ink.dim);
    }

    // **The five sidebar buttons.** `Misc_cty` frame 57 already drew them; all
    // this adds is which one the pointer is over — an outline and a caption of
    // ours. **Debug overlay only.** A player: *"still seeing debug outlines and
    // text for the 4 icons at the bottom right … it's not in the OG."*
    // `Sidebar_ButtonClicked` is one `Hotspot_Test`, which draws nothing, and
    // `Screen_DrawCampaign` paints the strip as one frame with nothing over it.
    // The original's words for these buttons are the tooltip layer's
    // (`FUN_00476E95`, group 220), which is not built.
    if let (Focus::Sidebar(i), true) = (screen.focus, game.prefs.debug_overlay) {
        let r = SIDEBAR_BUTTONS[i].rect();
        widget::frame(canvas, r, ink.highlight);
        text::draw_centred(canvas, r.centre_x(), r.y - 10, SIDEBAR_BUTTONS[i].name, ink.highlight);
    }

    // `Screen_DrawEndTurn` (`0x0041A734`):
    // `Ui_DrawCentred(4, 0, 0x1DE, 0x1CE, 0xA2, &g_fontSmall, 0x16)` — `L2.eng`
    // group 4, centred in 162 pixels at (478, 462), in the strip's own 9-pixel
    // font. We had it two lines lower and in words of ours.
    //
    // # The label goes away while the turn runs, and that is a *conditional
    // draw*, not a pressed frame
    //
    // A player: *"in the original, the text 'END TURN' would disappear when you
    // click it, until the new turn was ready."* He is exactly right.
    // original's shape is one line:
    //
    // ```c
    // Pl8_DrawFrameHere(g_miscCtySheet, 0x3B, 0x1DE, 0x1CC);       /* the strip, always */
    // if (g_realms[g_localPlayer].aiStep < 999)
    //     Ui_DrawCentred(4, 0, 0x1DE, 0x1CE, 0xA2, &g_fontSmall, 0x16);
    // ```
    //
    // The strip is opaque artwork and is blitted unconditionally, so drawing it
// *is* the erase; the label is not put back.
    // move, is not redrawn pressed, and is not removed by the sidebar's own
    // gate** — the other three explanations that fitted the report.
    //
    // **The flag is `aiStep`, and it is a per-realm turn program counter rather
    // than a boolean.** `Turn_BeginPlayersTurn` sets every living realm's to 0
    // and a dead one's to 999; `AI_RunTurnStep` walks it up and parks it at 999
    // when that realm is finished; `Turn_End` (`0x0043AC23`) sets the local
    // player's to 999 the instant this button is clicked. So **999 means "this
    // realm's turn is over"**
    // the click
    //
    // `turn::turn_in_flight` is that interval here,
// `game.turn` is `Some` from the click until the
    // turn completes, which is when `Turn_BeginPlayersTurn` would clear the
    // counter. **This is a draw that only became possible to reproduce when the
    // turn started being paced over frames** — before that there was no
    // interval to be inside.
    //
    // **Two other things read the same flag.** `Screen_DrawMenuBar`'s banner
    // loop is `strength != 0 && aiStep < 999`, so each realm's banner vanishes
    // from the menu bar as that realm finishes its turn
    // the new one begins — reproduced in `draw_menu_bar`, through
    // `turn::realm_turn_ended`. And `FUN_0041A639`'s turn timer reads it too —
    // as one half of `DAT_0055403C < 1 || aiStep == 999`, which keeps the
    // timer up through the person's own turn *and* after he ends it. That one
    // is drawn by `Machine::draw`, not here, because the original calls it from
// the frame loop. See
    // `crate::turn_clock`. This paragraph used to say we had no turn timer, and
    // before that that we reproduced neither of the other two, and both times it
    // sat twenty lines from the draw it described as missing.
    // `docs/draws-map.md` §5.11, `docs/decisions.md`
    // C152 and C158.
    if !turn::turn_in_flight(&ctx.game) {
        // The hover colour is ours: `Screen_DrawEndTurn` passes `0x16` whatever
        // the pointer does. Debug overlay only.
        let end = if screen.focus == Focus::EndTurn && game.prefs.debug_overlay {
            ink.highlight
        } else {
            ink.text
        };
        let label = ctx.assets.shell.text(4, 0);
        let label = if label.is_empty() { "END TURN" } else { label };
        county::strip_centred_at(ctx, canvas, PANEL_X, 462, PANEL_W, label, end);
    }
}
