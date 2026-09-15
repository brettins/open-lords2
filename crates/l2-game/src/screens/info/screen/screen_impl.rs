#![allow(unused_imports)]
use super::*;
use super::methods::*;
use super::*;
use super::types::*;
use super::constants::*;
use super::painters::*;
use l2_kingdom::conquest::LeftCastle;
use l2_view::Canvas;
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Face, Pen};

impl Screen for InfoScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Info(self.target)
    }

    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    fn take_redraw(&mut self) -> bool {
        self.press.take_redraw()
    }

    fn title(&self, _ctx: &Ctx) -> String {
        match self.target {
            Target::Unit(id) => format!("Unit {id} — screen 0x04"),
            Target::Tile(t) => format!("Tile {t} — screen 0x04"),
        }
    }

    fn is_overlay(&self) -> bool {
        true
    }

    /// **`Widget_Test`'s countdown over `DAT_004DD640`**, which runs the garrison
    /// widget's pressed picture back up.
    ///
    /// This screen had no `update`, so the picture went down on the first press
    /// and stayed down. The repeat that a held press produces here is inert —
    /// `FUN_00438ACC` has already turned the panel into the unit half, which
    /// has no garrison widget — so nothing is done with it.
    fn update(&mut self, _ctx: &mut Ctx) -> Transition {
        let _ = self.press.tick();
        Transition::Stay
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        if matches!(event, Event::Release { .. } | Event::Pointer { .. } | Event::PointerLeft) {
            let fired = self.press.event(&garrison_widgets(), event);
            debug_assert!(fired.is_none(), "the garrison widget is kind 4, not kind 3");
        }
        let Event::Click { x, y } = event else {
            return match event {
                // arm: 0x0042FF10/info-right-close right-release
                Event::RightClick { .. } => Transition::Pop,
                // arm: ours/info-keyboard-close key
                Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
                // ```c
                // if (Map_EdgeScroll()) { g_screenId = 0; FUN_0043CC56(); }
                // ```
                //
                // `Map_EdgeScroll` (`0x00432221`) returns 0 **at the far zoom**
                // — `if (g_battlePhase == 0 && g_mapZoom == 2) return 0;` — so
// the gesture does nothing there, so this reads
// [`crate::game::Game::map_zoom_far`]
                // any edge. Its other refusal, a message scroll being up, is
                // vacuous here: we have no message scroll.
                //
                // arm: 0x0042FF10/info-edge-scroll-closes pointer
                Event::Pointer { x, y } if !ctx.game.map_zoom_far && at_screen_edge(x, y) => {
                    Transition::Pop
                }
                // **A double click reaches the garrison widget and nothing else
                // on this screen.** `0x04`'s arm runs `Ui_OkButtonClicked` and
                // the brush (`Hotspot_Test` kind 3), both of which read
                // `g_mouseLeftReleased`; then `FUN_00438A91`, whose
                // `Widget_Test` kind 4 reads `g_mouseLeftPressed ||
                // g_mouseLeftDoubleClick`; then the unit buttons, `Hotspot_Test`
                // kind 1, `g_mouseLeftPressed` alone; and the minimap epilogue
                // is guarded by `g_mouseLeftPressed || g_mouseRightPressed`.
                //
                // So of the five, one answers. `[V]` This screen dropped it.
                Event::DoubleClick { .. } => {
                    self.garrison_press(ctx, event);
                    Transition::Stay
                }
                _ => Transition::Stay,
            };
        };
        // **Only the raster, not the column.** `0x04`'s arm does not run the
        // six sidebar guards — the village's and the four county panels' do,
        // and this one does not —
        // split slider are all dead with the information panel up, and only the
        // 128 × 128 minimap is not. See `screens/court.rs` at the same arm and
        // `docs/arms.json` `0x0042FF10/minimap-closes-the-surface`, which is the
        // campaign map's half of it.
        //
        // arm: 0x0042FF10/minimap-under-the-info-panel left-press
        if l2_view::chrome::minimap_hit_area().contains(x, y) {
            return Transition::Pass;
        }
        // arm: 0x0042FF10/info-ok left-release
        if OK.contains(x, y) {
            return Transition::Pop;
        }
        // `FUN_00438990` — the brush, on left **release**, and every button
        // closes the panel afterwards because `Field_SetType` sets
        // `g_screenId = 0`.
        //
        // arm: 0x00438990/field-brush left-release
        //
        // `0x00438990/tile-panel-hotspots` is the same arm, filed a second time
        // while the table stood in a popup of ours on the campaign map; the
        // popup is gone and both records now name this one test.
        //
        // arm: 0x00438990/tile-panel-hotspots left-release
        if let Some(ids) = self.brush(ctx) {
            let xs: &[i32] = if ids.len() == 3 { &BRUSH_FIELD_X } else { &BRUSH_WASTE_X };
            for (i, &bx) in xs.iter().enumerate() {
                let box_ = Rect::new(bx, BRUSH_ROW_Y, BRUSH_DIM, BRUSH_DIM);
                if box_.contains(x, y) {
                    if let Target::Tile(tile) = self.target {
                        let county = ctx.game.kingdom.campaign.map.county[tile] as usize;
                        let kind = match ids[i] {
                            0 => l2_kingdom::field::FieldType::Waste,
                            1 => l2_kingdom::field::FieldType::Fallow,
                            2 => l2_kingdom::field::FieldType::Grain,
                            0x13 => l2_kingdom::field::FieldType::Pasture,
                            _ => l2_kingdom::field::FieldType::Reclaiming,
                        };
                        let _ = ctx.game.kingdom.paint_field(county, tile, kind);
                    }
                    return Transition::Pop;
                }
            }
        }
        // **`FUN_00438A91` — the tile half's one widget.** It is tested between
        // the brush and the unit buttons, and it is the only control on this
        // screen that changes what the panel is *about* without leaving it:
        //
        // `FUN_00438ACC` writes the county's garrison into `g_pickedTileUnit`
        // and calls the painter again, so the tile half becomes the unit half
// in place. `g_screenId` never moves, so this is a mutation
        // of `self.target` and not a transition.
        //
        // `FUN_00438ACC` opens `if (g_mapZoom != 2)` and does nothing at the far
        // zoom, which is [`crate::game::Game::map_zoom_far`] here. The arm is
        // declared on [`garrison_widgets`].
        if self.garrison_press(ctx, event) {
            return Transition::Stay;
        }
        // **`FUN_00437002` — the three army buttons, on left *press*** while the
        // brush above fires on release. Which three depends on
        // `unit.garrisonCounty`: `g_infoUnitButtons` (`0x004DC560`) in the field
        // and `0x004DC5A8` inside a castle, differing in the first slot only.
        if let Some((id, garrisoned)) = self.unit_buttons(&Ctx { game: ctx.game, assets: ctx.assets })
        {
            let l = self.layout(ctx);
            for (i, &bx) in BUTTON_X.iter().enumerate() {
                if !Rect::new(bx, l.y(BUTTON_DY - l.row * 16), BUTTON_DIM, BUTTON_DIM).contains(x, y)
                {
                    continue;
                }
                return match (i, garrisoned) {
                    // **`Panel_MoveButton` (`0x004371CE`) — the door to screen
                    // `0x10`.** Two statements: `g_screenId = 0` and
                    // `Map_BeginMoveSelection()`. Ours has no global to write,
                    // so the request goes on [`crate::game::Game`] and the map
                    // picks it up on its next tick; the pop is the `g_screenId
                    // = 0`.
                    //
                    // **The besieging case is not reproduced**: the original
                    // asks `L2.eng` 10/13 *"Lift the siege?"* through
                    // `Ui_OpenConfirm` first, and we have no confirm box. It is
// named in `docs/arms.json`.
                    //
                    // arm: 0x00437002/info-move left-press
                    (0, false) => {
                        ctx.game.begin_move_order = Some(id);
                        Transition::Pop
                    }
                    // `Army_LeaveCastle` (`0x004374C4`) — **leave the castle**,
                    // the garrisoned table's first slot, and `g_screenId = 0`
                    // is the pop. [`crate::game::Game::leave_castle`] is its
                    // body `FUN_00437535`, whose tail starts the sortie battle
                    // when the castle is besieged — `Battle_BeginFromCampaign`
                    // (`0x004A7158`), staged by `Game::leave_castle`; the
                    // prompt comes up on the map behind this pop.
                    (0, true) => match ctx.game.leave_castle(id) {
                        LeftCastle::Marched { tile, .. } => {
                            self.status = format!("MARCHED OUT TO {},{}", tile.0, tile.1);
                            Transition::Pop
                        }
                        LeftCastle::Destroyed => {
                            self.status = "NOWHERE TO STAND - THE GARRISON IS LOST".into();
                            Transition::Pop
                        }
                        LeftCastle::NotAGarrison => Transition::Stay,
                    },
                    // **`Panel_DisbandButton` (`0x0043733A`)**, in both tables.
                    //
                    // It picks the county the men would join — the home county,
                    // or the one the army stands in when the home county has
                    // changed hands — and asks *"Disband army?"* only when that
                    // county is the owner's. Otherwise it raises message `0x91`,
                    // `L2.eng` group 145, and closes the panel.
                    //
                    // [`l2_kingdom::divide::disband_county`] is the two clauses
                    // and [`crate::game::Game::disband_army`] the whole of it,
                    // including the refusal. **The confirm box is not
                    // reproduced** — `Ui_OpenConfirm(6, …)` is `L2.eng` 10/6 —
                    // and neither is the message scroll, so the refusal is a
                    // status line of ours.
                    //
                    // arm: 0x00437002/info-disband left-press
                    (1, _) => match ctx.game.disband_army(id) {
                        Ok((county, men)) => {
                            let name = super::super::super::county::county_name(&*ctx, county);
                            self.status = format!("{men} MEN WENT HOME TO {name}");
                            Transition::Pop
                        }
                        Err(_) => {
                            self.status =
                                "MARCH IT TO A COUNTY YOU RULE BEFORE DISBANDING".into();
                            Transition::Pop
                        }
                    },
                    // **`Panel_SplitButton` (`0x004378B3`)**, and it is a
// `Push` because `0x11` goes
                    // **back to `0x04`**: every one of the division screen's
                    // three ways out — the turn-ended latch, the right release
                    //
                    // That is `docs/arms.json`
                    // `0x0042FF10/back-one-rather-than-to-the-map`, whose note
                    // said ours reached the campaign map "because we have no
                    // unit panel to go back to". There is one now.
                    //
                    // arm: 0x004378B3/info-split left-press
                    (2, _) => Transition::Push(ScreenId::Divide(id)),
                    _ => Transition::Stay,
                };
            }
        }
        Transition::Stay
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
        let l = self.layout(ctx);
        let b = l.box_at();
        pen.window(canvas, b.x, b.y, b.w / 16, b.h / 16, 0);
        pen.ok_button(canvas, OK.x, OK.y, 0);

        let icon = |frame: usize, canvas: &mut Canvas| {
            if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
                canvas.blit(&f, ICON_AT.0, l.y(ICON_AT.1));
            }
        };

        match self.target {
            Target::Unit(id) => {
                let k = &ctx.game.kingdom;
                let Some(u) = k.campaign.units.get(id) else { return };
                use l2_kingdom::unit::UnitKind;
                let (heading, body, frame) = match u.kind {
                    UnitKind::Merchant => MERCHANT,
                    UnitKind::PeasantMob => PEASANTS,
                    UnitKind::Transport => TRANSPORT,
                    _ if u.owner == ctx.game.player => OWN_ARMY,
                    _ => ENEMY_ARMY,
                };
                icon(frame, canvas);
                // ```c
                // if (local_20 == 2) {                                /* transport */
                //   g_penAdvance = 0;
                //   Eng_DrawString(0x1f, 2, 0x18, R * 0x10 + 0x30, &g_fontHeading, 0x3f);
                //   Eng_DrawString(100, unit[+0x167] + g_scenarioIndex * 0x14,
                //                  g_penAdvance + 0x18, R * 0x10 + 0x30, &g_fontHeading, 0x3f);
                // } else if (local_20 != 6) {                         /* merchant, peasants */
                //   Eng_DrawString(0x1f, local_20, 0x28, R * 0x10 + 0x40, &g_fontHeading, 0x3f);
                // }
                // ...                                                 /* kind == 1 */
                // g_penAdvance = 0;
                // Eng_DrawString((char)unit.owner + 0x5d, unit.nameIndex, 0x28,
                //                R * 0x10 + 0x30, &g_fontHeading, 0x3f);
                // ```
                //
                // `+0x167` on a transport is [`l2_kingdom::unit::Unit::cargo_county`].
                match u.kind {
                    UnitKind::Transport => {
                        let y = l.y(TRANSPORT_HEADING_AT.1);
                        let w = pen.eng_in(
                            Face::Heading,
                            canvas,
                            UNIT_GROUP,
                            heading,
                            TRANSPORT_HEADING_AT.0,
                            y,
                            font::TEXT,
                        );
                        let name = super::super::super::county::county_name(ctx, u.cargo_county);
                        pen.heading(canvas, w, y, &name, font::TEXT);
                    }
                    UnitKind::Army => {
                        pen.eng_in(
                            Face::Heading,
                            canvas,
                            ARMY_NAME_GROUP + u.owner as usize,
                            u.name_index as usize,
                            HEADING_X,
                            l.y(ARMY_NAME_DY),
                            font::TEXT,
                        );
                    }
                    _ => {
                        pen.eng_in(
                            Face::Heading,
                            canvas,
                            UNIT_GROUP,
                            heading,
                            HEADING_X,
                            l.y(HEADING_DY),
                            font::TEXT,
                        );
                    }
                }
                if u.kind == UnitKind::Army {
                    // `docs/decisions.md` C110.
                    let w = pen.eng(canvas, UNIT_GROUP, ARMY_FROM, HEADING_X, l.y(0x4A), font::TEXT);
                    let name = super::super::super::county::county_name(ctx, u.home_county);
                    pen.body(canvas, w, l.y(0x4A), &name, font::TEXT);
                }
                if u.kind != UnitKind::Army {
                    let s = a.text(UNIT_GROUP, body).to_string();
                    pen.body_wrapped(canvas, BODY_X, l.y(BODY_DY), BODY_WRAP, &s, font::TEXT);
                } else {
                    // **The army's body line is the fourth thing `g_optArmiesEat`
                    // gates** — `UnitPanel_Draw` (`0x0041B19D`), the `kind == 1`
                    // arm. With foraging off it is one line at `+0x70`; with it
                    // on the body moves up to `+0x5E` and two more follow:
                    //
                    // ```c
                    // if (g_optArmiesEat == 1) {
                    //   FUN_0040328E(0x1f, local_1c,  0x68, R*0x10+0x5e, 0x120, …, 0x3f);
                    //   FUN_0040328E(0x1f, local_18,  0x68, R*0x10+0x72, 0x120, …, 0x3f);
                    //   local_8 = unit.starvation == 0 ? 0x3f : 0xf9;
                    //   FUN_0040328E(0x1f, unit.starvation + 0x1b, 0x68, R*0x10+0x86, 0x140, …, local_8);
                    // } else FUN_0040328E(0x1f, local_1c, 0x68, R*0x10+0x70, 0x120, …, 0x3f);
                    // ```
                    //
                    // `local_18` is 31/23…26 by the two owners — the supply
                    // state `docs/armies.md` §3.4 tabulates, and 31/26 is
                    // unreachable here because an enemy army in your county
                    // takes 24. The starvation line is 31/27…31 off `+0x155`,
                    // red once the counter leaves zero.
                    let armies_eat = k.options.armies_eat;
                    let s = a.text(UNIT_GROUP, body).to_string();
                    let dy = if armies_eat { 0x5E } else { 0x70 };
                    pen.body_wrapped(canvas, BODY_X, l.y(dy), BODY_WRAP, &s, font::TEXT);
                    if armies_eat {
                        let county_owner =
                            k.counties.get(u.county as usize).map_or(0, |c| c.owner as i32);
                        let supply = if u.owner == ctx.game.player {
                            if county_owner == ctx.game.player as i32 { SUPPLY0 } else { SUPPLY0 + 1 }
                        } else if county_owner == u.owner as i32 {
                            SUPPLY0 + 2
                        } else {
                            SUPPLY0 + 1
                        };
                        let s = a.text(UNIT_GROUP, supply).to_string();
                        pen.body_wrapped(canvas, BODY_X, l.y(0x72), BODY_WRAP, &s, font::TEXT);
                        let band = (u.starvation.clamp(0, 4)) as usize;
                        let colour = if band == 0 { font::TEXT } else { STARVING };
                        let s = a.text(UNIT_GROUP, HEALTH0 + band).to_string();
                        pen.body_wrapped(canvas, BODY_X, l.y(0x86), 0x140, &s, colour);
                    }
                }
                if u.kind == UnitKind::Transport {
                    // `troops[0]` is grain and `troops[2]` cattle — group 8
                    // nouns `0x44` and `0x46`, and the two `Misc_cty` icons.
                    pen.misc_frame(canvas, 0x21, 0x38, l.y(0xA0));
                    pen.count(canvas, 0x60, l.y(0xA4), u.troops[0], 0x44, font::TEXT);
                    pen.misc_frame(canvas, 0x26, 0x104, l.y(0xA0));
                    pen.count(canvas, 0x12E, l.y(0xA4), u.troops[2], 0x46, font::TEXT);
                }
                if u.kind == UnitKind::Army && u.owner == ctx.game.player {
                    let w = pen.eng(canvas, UNIT_GROUP, FORMED, HEADING_X, l.y(0xA0), font::TEXT);
                    // `Ui_DrawYear(yearFormed, g_penAdvance + 0x28, …, 0)` —
                    // **style 0, which appends `L2.eng` 26/1 "AD"**. We drew the
                    // bare number
                    // holds for this line was missing. `CLAUDE.md` rule 6.
                    pen.year(canvas, w, l.y(0xA0), u.year_formed, 0, font::TEXT);
                    let w = pen.eng(canvas, UNIT_GROUP, WAGES, 0xF8, l.y(0xA0), font::TEXT);
                    pen.count(canvas, w, l.y(0xA0), u.wages, 0, font::TEXT);
                    if u.garrison_county == 0 {
                        let left = (MOVE_ALLOWANCE - u.moves_used as i32).max(0);
                        // `Ui_DrawNumber(left, '@', &DAT_004D4228, 0xF8, …, body)`, and
                        // `DAT_004D4228` is a NUL: the lead holds a column and there is
                        // no suffix. The old `number(…, true)` dropped the one and
                        // invented the other, so the digit sat four pixels left and
                        // *"moves left"* landed where it should by coincidence. **[V]**
                        let face = crate::shell::Face::Body;
                        let w = pen.number_in(face, canvas, 0xF8, l.y(0x170), left, '@', "", font::TEXT);
                        pen.eng(canvas, UNIT_GROUP, MOVES_LEFT, w, l.y(0x170), font::TEXT);
                    }
                    let first = if u.garrison_county == 0 { icon::MOVE } else { icon::SORTIE };
                    for (i, &frame) in [first, icon::DISBAND, icon::SPLIT].iter().enumerate() {
                        if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
                            canvas.blit(&f, BUTTON_X[i], l.y(BUTTON_DY - l.row * 16));
                        }
                    }
                    for t in 0..7usize {
                        let row = (t / 2) as i32 * 13;
                        let (cx, nx) = if t % 2 == 0 { (0x38, 0x58) } else { (0xF8, 0x118) };
                        pen.misc_frame(canvas, 0x2F + t, cx, l.y(0xBE + row));
                        // `FUN_004224E7(unit, 0, 0x38, …, &g_fontBody, 2)` →
                        // `Ui_DrawCount(troops[t], t * 2 + 0x34, …, body)`.
                        pen.count(
                            canvas,
                            nx,
                            l.y(0xC2 + row),
                            u.troops[t],
                            0x34 + t * 2,
                            font::TEXT,
                        );
                    }
                    // ```c
                    // if (unit.mercMen == 0) {
                    //   Eng_DrawString(0x10, 0, 0x38, R * 0x10 + 0x130, &g_fontHeading, 0x3f);
                    // } else {
                    //   g_penAdvance = 0;
                    //   Ui_DrawNumber(mercMen, '@', &DAT_004d422c, 0x38, R * 0x10 + 0x130, &g_fontHeading, 0x3f);
                    //   Eng_DrawString(0x10, mercBand, g_penAdvance + 0x38, …, &g_fontHeading, 0x3f);
                    //   Ui_DrawUnitNoun(mercMen, mercTroop * 2 + 0x34, g_penAdvance + 0x38, …);
                    // }
                    // ```
                    //
                    // `Ui_DrawUnitNoun` (`0x0041AC3E`) is `value == 1 ? index : index + 1`
                    // — no `-1` arm, unlike `Ui_DrawCount`, and none is needed for a byte.
                    let y = l.y(MERC_LINE_AT.1);
                    match u.mercenaries {
                        Some(m) if m.men != 0 => {
                            let w = pen.number_in(
                                Face::Heading,
                                canvas,
                                MERC_LINE_AT.0,
                                y,
                                m.men(),
                                '@',
                                "",
                                font::TEXT,
                            );
                            let w = pen.eng_in(
                                Face::Heading,
                                canvas,
                                MERC_GROUP,
                                m.band as usize,
                                w,
                                y,
                                font::TEXT,
                            );
                            let noun = 0x34 + m.troop as usize * 2 + usize::from(m.men != 1);
                            pen.eng_in(
                                Face::Heading,
                                canvas,
                                crate::shell::COUNT_NOUN_GROUP,
                                noun,
                                w,
                                y,
                                font::TEXT,
                            );
                        }
                        _ => {
                            pen.eng_in(
                                Face::Heading,
                                canvas,
                                MERC_GROUP,
                                0,
                                MERC_LINE_AT.0,
                                y,
                                font::TEXT,
                            );
                        }
                    }
                }
            }
            Target::Tile(tile) => {
                let map = &ctx.game.kingdom.campaign.map;
                // `FUN_0041BEFE`, after the box and the OK button and before
                // `TileInfo_Draw`: the recessed well the tile panel's words sit in.
                pen.inset(canvas, Rect::new(0x20, l.y(0x38), 400, (0x18 - l.row) * 16));
                if let Some(field) = self.farmland(ctx) {
                    draw_farmland(ctx, &pen, canvas, l, field);
                }
                if l.headroom != 0 {
                    let name = super::super::super::county::county_name(ctx, map.county[tile]);
                    pen.heading_centred(canvas, 8, l.y(0x18), 0x1C0, &name, font::TEXT);
                }
                if let Some(ids) = self.brush(ctx) {
                    crate::shell::button_recess(
                        canvas,
                        BRUSH_BEVEL.x,
                        BRUSH_BEVEL.y,
                        BRUSH_BEVEL.w,
                        BRUSH_BEVEL.h,
                    );
                    let xs: &[i32] = if ids.len() == 3 { &BRUSH_FIELD_X } else { &BRUSH_WASTE_X };
                    for (i, &bx) in xs.iter().enumerate() {
                        let frame = match ids[i] {
                            0 => icon::BRUSH_ABANDON,
                            1 => icon::BRUSH_FALLOW,
                            2 => icon::BRUSH_GRAIN,
                            0x13 => icon::BRUSH_PASTURE,
                            _ => icon::BRUSH_RECLAIM,
                        };
                        if let Some(f) = a.sheet(ICON_SHEET).and_then(|s| s.frame(frame)) {
                            canvas.blit(&f, bx, BRUSH_ROW_Y);
                        }
                    }
                    let s = a.text(TILE_GROUP, BRUSH_CAPTION).to_string();
                    pen.body_wrapped(
                        canvas,
                        BRUSH_CAPTION_AT.0,
                        BRUSH_CAPTION_AT.1,
                        BRUSH_CAPTION_AT.2,
                        &s,
                        font::TEXT,
                    );
                }
                if let Some(castle) = self.castle_tile(ctx) {
                    draw_castle(ctx, &pen, canvas, l, castle, self.press.is_pressed(0), ink);
                }
                if let Some((site, c)) = self.resource_site(ctx) {
                    draw_resource_site(ctx, &pen, canvas, l, site, c);
                }
                // **The county-town arm, and the one line of English the
                // mercenary has anywhere in the game.** The map's marker
                // (`screens/map/paint.rs`'s `draw_flags`) is a picture with no words
                // on it; this is where the original says what it means, and a
                // player who had looked straight at the marker still reported
                // never having seen a mercenary. Rule 6: the strings are the
                // specification, and they come out of the player's `L2.eng`.
                if self.county_town(ctx).is_some() {
                    icon(COUNTY_TOWN_ICON, canvas);
                    let s = a.text(TILE_GROUP, COUNTY_TOWN_HEADING).to_string();
                    pen.heading(canvas, HEADING_X, l.y(HEADING_DY), &s, font::TEXT);
                    let s = a.text(TILE_GROUP, COUNTY_TOWN_BODY).to_string();
                    pen.body_wrapped(canvas, BODY_X, l.y(BODY_DY), TILE_BODY_WRAP, &s, font::TEXT);
                    if self.mercenary_offer(ctx) {
                        let zoom = &l2_view::campaign::ZOOMS[usize::from(ctx.game.map_zoom_far)];
                        let marker = ctx
                            .assets
                            .map
                            .flag_sheet(zoom)
                            .and_then(|s| s.frame(l2_view::campaign::MERCENARY_MARKER_FRAME));
                        if let Some(f) = marker {
                            canvas.blit(&f, MERC_MARKER_AT.0, l.y(MERC_MARKER_AT.1));
                        }
                        let s = a.text(TILE_GROUP, MERCENARIES_AVAILABLE).to_string();
                        pen.body_wrapped(
                            canvas,
                            MERC_TEXT_AT.0,
                            l.y(MERC_TEXT_AT.1),
                            MERC_TEXT_AT.2,
                            &s,
                            font::TEXT,
                        );
                    }
                }
                if let Some(kind) = self.tile_kind(ctx) {
                    draw_plain_tile(&pen, canvas, l, kind);
                }
            }
        }
        if ctx.game.prefs.debug_overlay && !self.status.is_empty() {
            l2_view::text::draw(canvas, 12, 452, &self.status, ink.highlight);
        }
    }
}


