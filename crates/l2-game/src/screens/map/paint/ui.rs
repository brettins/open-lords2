#![allow(unused_imports)]
use super::*;
use super::units::*;
use super::*;

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

