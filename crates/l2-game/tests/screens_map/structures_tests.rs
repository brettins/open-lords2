#![allow(unused_imports)]
use super::*;
use super::view_tests::*;
use super::interaction_tests::*;
use super::fog_and_march_tests::*;
use common::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::Screen;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::Panel;
use l2_game::screens::map;
use l2_game::screens::map::MapScreen;
use l2_game::shell::font;
use l2_game::Game;
use l2_view::campaign;
use l2_view::chrome;
use l2_view::Canvas;

/// A sprite's **exact ink**, found anywhere on the canvas.
///
/// The same idea as [`find_text`] and for the same reason: render the thing
/// Being looked for: keep the pixels it paints.
/// pattern. A PL8 blit copies only its opaque bytes, so a match is the frame's
/// own palette indices standing where the frame was blitted — several hundred
/// of them for a flag. That cannot arise from terrain.
///
/// Every place this frame's artwork stands, and how many opaque pixels had to
/// agree to make each one a match.
fn sprite_positions(
    canvas: &Canvas,
    frame: &l2_formats::pl8::DecodedFrame,
) -> (Vec<(i32, i32)>, usize) {
    let (w, h) = (frame.width as i32, frame.height as i32);
    let wanted: Vec<(i32, i32, u8)> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .filter(|&(x, y)| frame.opaque[(y * w + x) as usize])
        .map(|(x, y)| (x, y, frame.indices[(y * w + x) as usize]))
        .collect();
    let mut found = Vec::new();
    if wanted.is_empty() {
        return (found, 0);
    }
    for oy in 0..=(canvas.height as i32 - h) {
        for ox in 0..=(canvas.width as i32 - w) {
            let (fx, fy, fi) = wanted[0];
            if canvas.at((ox + fx) as usize, (oy + fy) as usize) != fi {
                continue;
            }
            if wanted.iter().all(|&(x, y, i)| canvas.at((ox + x) as usize, (oy + y) as usize) == i)
            {
                found.push((ox, oy));
            }
        }
    }
    (found, wanted.len())
}

/// The first of them, scanning rows then columns.
fn find_sprite(
    canvas: &Canvas,
    frame: &l2_formats::pl8::DecodedFrame,
) -> Option<((i32, i32), usize)> {
    let (found, ink) = sprite_positions(canvas, frame);
    found.first().map(|&p| (p, ink))
}

/// The map centred on a county's town, drawn.
fn town_view(game: &mut Game, assets: &Assets, county: u8) -> (MapScreen, Canvas) {
    let mut screen = MapScreen::new();
    {
        let ctx = Ctx { game, assets };
        let town = MapScreen::town(&ctx, county);
        let &tile = town.first().expect("a county has a town");
        let (tx, ty) = l2_kingdom::map::coords(tile);
        screen.centre_on_tile(tx as usize, ty as usize);
    }
    let canvas = draw(&mut screen, game, assets);
    (screen, canvas)
}

/// **The county town flies its owner's flag, and it waves.**
///
/// A player: *"I didn't see the colorful waving flag over my county."* It is
/// There: the assertion in numbers.
/// screenshot somebody has to open.
///
/// Three claims, each with its own pixels:
///
/// 1. **It is drawn.** `Flags1a.pl8` frame `(shield − 1) * 8 + phase` — several
///    hundred opaque palette indices — stands somewhere on the canvas, exactly.
/// 2. **The frame is keyed on the shield.** Move the owning realm's
/// `shield_index` and the flag at the *same pixel* becomes the other
///    shield's frame. Nothing else on the campaign map reads `shield_index` —
/// the minimap and the menu-bar banner both read `realm_colour` — so this
///    isolates the flag from everything drawn beside it.
/// 3. **The wave advances.** Sixteen ticks is one phase (`phase = tick >> 4`),
///    and after them the flag at the same pixel is the next frame of the eight.
///
/// If any of the three stops being true the feature has gone, and this fails
/// instead of a person noticing weeks later. C59.
#[test]
fn the_county_town_flies_its_owners_flag_and_the_wave_advances() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    let owner = game.kingdom.counties[county as usize].owner as usize;
    let shield = game.kingdom.realms[owner].shield_index;
    assert!(
        (1..=5).contains(&shield),
        "realm {owner} carries shield {shield}, which flies nothing"
    );

    let (mut screen, canvas) = town_view(&mut game, &assets, county);
    let sheet = assets.map.flag_sheet(screen.zoom()).expect("Flags1a.pl8 is in the install");
    let frame_of = |shield: u8, phase: u8| {
        let i = campaign::flag_frame(shield, phase).expect("a shield of 1 ..= 5 has a frame");
        sheet.frame(i).expect("Flags1a.pl8 holds forty 32 x 24 frames")
    };

    // 1 — it is drawn.
    let f = frame_of(shield, 0);
    assert_eq!((f.width, f.height), (32, 24), "the first forty frames are 32 x 24");
    let (at, ink) = find_sprite(&canvas, &f)
        .expect("the county town flies its owner's flag, and it is not on the canvas");
    assert!(
        ink >= 200,
        "the flag matched on only {ink} opaque pixels, which is too few to be the flag"
    );

    assert!(at.0 < campaign::PANEL_X, "the flag is on the map, not in the sidebar");

    // 2 — the frame is keyed on the shield, at the same pixel.
    let other = if shield == 5 { 1 } else { shield + 1 };
    game.kingdom.realms[owner].shield_index = other;
    let (_, moved) = town_view(&mut game, &assets, county);
    assert_eq!(
        find_sprite(&moved, &frame_of(other, 0)).map(|(p, _)| p),
        Some(at),
        "with shield {other} the same pixel must fly shield {other}'s flag"
    );
    game.kingdom.realms[owner].shield_index = shield;

    // 3 — the wave advances. `Map_DrawFrame`: `phase = (tick & 0x7F) >> 4`.
    for _ in 0..16 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        let _ = screen.update(&mut ctx);
    }
    let waved = draw(&mut screen, &mut game, &assets);
    assert_eq!(
        find_sprite(&waved, &frame_of(shield, 1)).map(|(p, _)| p),
        Some(at),
        "sixteen ticks is one phase, so the same pixel must now fly phase 1"
    );
}

/// **A garrisoned castle flies its *garrison's* shield, and an empty one flies
/// nothing.**
///
/// The second half of `FUN_004071A0`, and the half that is easy to get wrong by
/// reading the county instead of the unit standing in it:
///
/// ```c
/// else if (flags & 0x80) {                       /* the castle */
///   if (content <= 0x14 || !county.garrisonUnit) return;
///   shield = units[county.garrisonUnit].shield;  /* NOT county.owner */
/// }
/// ```
///
/// So a castle taken from somebody whose garrison is still theirs flies
/// **their** colours, and the two flags of one county disagree. That is the
/// case worth testing, and it is the case a fixture cannot supply: the position
/// is set up here — a county the player holds, its castle built, and somebody
/// *else's* army standing in it — so the two flags must be two different
/// Pictures. A save with a garrison would almost always
/// have one whose shield matched its host's, and would prove nothing about
/// which of the two fields the branch reads.
///
/// The measurement is a **count**, not a position: the town of the same county
/// is flying a flag of its own a few tiles away, so what is asserted is that
/// taking the garrison out removes **exactly one** flag and leaves the other
/// standing. C59.
#[test]
fn a_garrisoned_castle_flies_the_garrisons_shield_and_an_empty_one_flies_nothing() {
    let (mut game, assets) = world!();
    let county = game
        .kingdom
        .county_ids()
        .find(|&id| game.is_players(id as u8))
        .expect("the player holds a county");
    game.select(county as u8);

    let owner = game.kingdom.counties[county].owner as usize;
    let town_shield = game.kingdom.realms[owner].shield_index;
    // Somebody else's shield, so the castle's flag and the town's cannot be
    // confused for one another.
    let garrison_shield = if town_shield == 5 { 1 } else { town_shield + 1 };

    // A built castle with a foreign army inside it. `content <= 0x14` is the
    // bare plot and flies nothing even with a garrison standing on it, so the
    // castle has to be built for this to be the branch under test.
    let unit = game
        .kingdom
        .campaign
        .units
        .iter()
        .map(|(id, _)| id)
        .next()
        .expect("the fixture carries units");
    game.kingdom.campaign.units.get_mut(unit).expect("the slot exists").shield = garrison_shield;
    {
        let c = &mut game.kingdom.counties[county];
        c.castle_type = c.castle_type.max(1);
        c.garrison_unit = unit;
    }

    let mut screen = MapScreen::new();
    {
        let c = &game.kingdom.counties[county];
        screen.centre_on_tile(c.anchor_x as usize, c.anchor_y as usize);
    }
    let held = draw(&mut screen, &mut game, &assets);

    let sheet = assets.map.flag_sheet(screen.zoom()).expect("Flags1a.pl8 is in the install");
    let frame_of = |shield: u8| {
        sheet
            .frame(campaign::flag_frame(shield, 0).expect("a shield of 1 ..= 5 has a frame"))
            .expect("Flags1a.pl8 holds forty 32 x 24 frames")
    };
    let castle_flag = frame_of(garrison_shield);
    let town_flag = frame_of(town_shield);

    let (before, ink) = sprite_positions(&held, &castle_flag);
    assert!(
        !before.is_empty(),
        "the garrisoned castle of county {county} flies no flag: shield {garrison_shield}"
    );
    assert!(ink >= 200, "matched on {ink} opaque pixels, too few to be a flag");
    let towns_before = sprite_positions(&held, &town_flag).0.len();

    // Take the garrison away. `if (!county.garrisonUnit) return`.
    game.kingdom.counties[county].garrison_unit = 0;
    let empty = draw(&mut screen, &mut game, &assets);
    assert!(
        sprite_positions(&empty, &castle_flag).0.is_empty(),
        "an empty castle must fly nothing, and shield {garrison_shield}'s flag is still there"
    );
    assert_eq!(
        sprite_positions(&empty, &town_flag).0.len(),
        towns_before,
        "and the town's own flag reads the county's owner, so it must not have moved"
    );

    // The other half of the guard: a garrison on a bare plot flies nothing
    // either, whatever its shield.
    {
        let c = &mut game.kingdom.counties[county];
        c.castle_type = 0;
        c.garrison_unit = unit;
    }
    let bare = draw(&mut screen, &mut game, &assets);
    assert!(
        sprite_positions(&bare, &castle_flag).0.is_empty(),
        "`content <= 0x14` is the bare plot, and an unbuilt castle flies nothing"
    );
}

/// **A besieged castle carries the besieger's camp mark and the seasons he has
/// left** — `FUN_00407F82` (`0x00407F82`), called from `Sprite_TopIt`'s castle
/// arm before the garrison's banner.
///
/// The report was that a siege is invisible on the map. It was: we drew a dot
/// over the *army*, gated as a debug overlay because the original draws nothing
/// there, and nothing at all over the castle, where the original draws this.
///
/// ```c
/// besieger = g_units[county.garrisonUnit].besiegedBy;
/// if (besieger != 0 && g_mapZoom == 0)
///     FUN_00407f82(g_units[besieger].siegeSeasonsLeft, 8, -0x38);
/// ```
///
/// Two pictures, and the assertion is the pair: `Flags1a.pl8` frame `0x82`
/// (24 × 28, the sheet's **last** frame — it holds 131) at `(+8, −0x38)` from
/// the castle tile's origin, and the count centred in that frame's own width
/// ten pixels lower, flat and in `0xF9`. Neither is reachable from a fixture,
/// so the siege is staged here: the link the game keeps is on the *garrison*,
/// `+0x19A`, and the number is on the besieger, `+0x19C`.
///
/// **Ablated**: dropping either draw, moving the mark to the flag's own
/// `(+0x1A, −0x1C)`, putting the count on the garrison instead of the besieger,
/// or centring it in anything but frame `0x82`'s width turns this red.
#[test]
fn a_besieged_castle_carries_the_besiegers_mark_and_his_seasons_left() {
    let (mut game, assets) = world!();
    let county = game
        .kingdom
        .county_ids()
        .find(|&id| game.is_players(id as u8))
        .expect("the player holds a county");
    game.select(county as u8);

    // A built castle, an army inside it, and a second army camped outside with
    // four seasons of work left.
    let mut ids = game.kingdom.campaign.units.iter().map(|(id, _)| id);
    let garrison = ids.next().expect("the fixture carries units");
    let besieger = ids.next().expect("the fixture carries a second unit");
    drop(ids);
    const SEASONS: u8 = 4;
    {
        let c = &mut game.kingdom.counties[county];
        c.castle_type = c.castle_type.max(1);
        c.garrison_unit = garrison;
    }
    game.kingdom.campaign.units.get_mut(besieger).expect("the slot exists").siege_seasons_left =
        SEASONS;

    // The castle's own tile, found the way the painter finds it.
    let castle = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        let terrain = ctx.game.kingdom.campaign.map.terrain.clone();
        MapScreen::settlements_for_test(&ctx, county as u8)
            .into_iter()
            .find(|&t| {
                l2_kingdom::industry::map_toggle_for_graphic(terrain[t])
                    == Some(l2_kingdom::industry::MapToggle::Castle)
            })
            .expect("the county's castle plot is on the map")
    };
    let (cx, cy) = l2_kingdom::map::coords(castle);

    let mut screen = MapScreen::new();
    draw(&mut screen, &mut game, &assets);
    screen.centre_on_tile(cx as usize, cy as usize);
    let free = draw(&mut screen, &mut game, &assets);

    let mark = assets
        .map
        .flag_sheet(screen.zoom())
        .and_then(|s| s.frame(campaign::BESIEGER_MARKER_FRAME))
        .expect("Flags1a.pl8 frame 0x82");
    assert_eq!((mark.width, mark.height), (24, 28), "frame 0x82 is the 24 x 28 camp mark");
    assert!(
        sprite_positions(&free, &mark).0.is_empty(),
        "an unbesieged castle carries no camp mark"
    );

    // `+0x19A` on the *garrison* is the link, and it is what the arm reads.
    game.kingdom.campaign.units.get_mut(garrison).expect("the slot exists").besieged_by =
        besieger as u8;
    let besieged = draw(&mut screen, &mut game, &assets);

    let (row, col) = campaign::tile_to_cell(cx as usize, cy as usize);
    let (sx, sy) = campaign::cell_to_screen(screen.viewport(), screen.zoom(), row, col);
    let (mx, my) = (sx + screen.zoom().besieger_at.0, sy + screen.zoom().besieger_at.1);

    // The whole frame, pixel for pixel, at the one place the two literals name.
    let mut reference = free.clone();
    reference.blit_clipped(&mark, mx, my, screen.map_clip());
    // `Ui_DrawNumberRight(seasons, ' ', " ", x, y + 10, frameWidth, &g_fontBody,
    // 0xF9)` — which centres, and is flat because `DAT_005AEA40` is set for it.
    let count = format!(" {SEASONS} ");
    let style = l2_game::shell::font::Style {
        colour: campaign::BESIEGER_COUNT_INK,
        shadow: None,
        caps: None,
    };
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8 is in the install");
    let ny = my + campaign::BESIEGER_COUNT_DY;
    assert!(campaign::BESIEGER_COUNT_TOP < ny, "the count clears the menu bar on this tile");
    body.draw_centred(&mut reference, mx, ny, i32::from(mark.width), &count, &style);

    let moved = |a: &Canvas, b: &Canvas| -> Vec<usize> {
        a.pixels
            .iter()
            .zip(b.pixels.iter())
            .enumerate()
            .filter(|(_, (p, q))| p != q)
            .map(|(i, _)| i)
            .collect()
    };
    // A set of 640 x 480 indices is unreadable in a failure; its bounding box
    // and its size say where the ink went and are what a reader needs.
    let corner = |v: &[usize]| -> (usize, usize, usize, usize, usize) {
        let xs = v.iter().map(|i| i % besieged.width);
        let ys = v.iter().map(|i| i / besieged.width);
        (
            xs.clone().min().unwrap_or(0),
            ys.clone().min().unwrap_or(0),
            xs.max().unwrap_or(0),
            ys.max().unwrap_or(0),
            v.len(),
        )
    };
    let (drawn, expected) = (moved(&besieged, &free), moved(&reference, &free));
    assert!(!expected.is_empty(), "frame 0x82 writes nothing at the place the literals name");
    assert_eq!(
        corner(&drawn),
        corner(&expected),
        "the siege's ink (x0, y0, x1, y1, count) is not frame 0x82 at ({mx}, {my}) with the \
         count centred in its 24 pixels at ({mx}, {ny})"
    );
    assert_eq!(drawn, expected, "the siege's ink is the right size in the right place and is not the same pixels");
    // Independently of the set: the digits are findable where they were put.
    let off = ((i32::from(mark.width) - body.width(&count)) / 2).max(0);
    assert_eq!(
        find_body(&besieged, &assets, &count, campaign::BESIEGER_COUNT_INK),
        Some((mx + off, ny)),
        "the seasons left are the besieger's `+0x19C`, centred in frame 0x82's own width"
    );

    // **The far zoom draws nothing, and that is the original's.**
    // `Sprite_TopIt` calls `FUN_00407F82(…, 2, -0x28)` at `g_mapZoom == 2` and
    // the whole of that function is inside `if (g_mapZoom == 0)`, so the call
    // returns having drawn nothing. `docs/bugs.md`.
    let mut far = MapScreen::new();
    draw(&mut far, &mut game, &assets);
    send(&mut far, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
    assert_eq!(far.zoom().id, campaign::FAR.id, "Z is the zoom toggle");
    let zoomed = draw(&mut far, &mut game, &assets);
    if let Some(far_mark) =
        assets.map.flag_sheet(far.zoom()).and_then(|s| s.frame(campaign::BESIEGER_MARKER_FRAME))
    {
        assert!(
            sprite_positions(&zoomed, &far_mark).0.is_empty(),
            "the far zoom's call site is dead in the original and must be dead here"
        );
    }

    // And the mark goes when the siege does.
    game.kingdom.campaign.units.get_mut(garrison).expect("the slot exists").besieged_by = 0;
    let lifted = draw(&mut screen, &mut game, &assets);
    assert_eq!(lifted.diff_count(&free), 0, "no siege, no mark and no count");
}

/// **The minimap tints by owner, and a realm's ramp is its own.**
///
/// A player: *"the minimap had default colors, it didn't identify who
/// owned a county."* The number that settles it is the count of distinct ramp
/// **rows** standing in the minimap rectangle. `MINIMAP_REALM_RAMP` is six rows
/// of four shades — row 0 the raster's own shading for unowned land, rows 1 … 5
/// one per realm colour — so:
///
/// * a minimap that tints by owner shows **one row per owning colour, plus row
///   0 wherever land is unowned**;
/// * a minimap that has lost the tint shows **exactly one row**, because
///   `chrome::realm_colour` clamps a zero colour *up* to 1 and every county
///   would collapse onto ramp 1.
///
/// On the England fixture that is five owned counties flying five different
/// colours out of fourteen, so six rows against one. The two cannot be
/// confused, which is the property that makes this worth asserting.
///
/// It also asserts the thing that makes the count mean anything: **no palette
/// index appears in two rows**, so "which row is this pixel from" has one
/// answer. C59.
#[test]
fn the_minimap_paints_one_ramp_row_per_owning_realm() {
    let (mut game, assets) = world!();
    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);

    // The ramp must be unambiguous before it can be counted.
    let mut row_of_index = [None::<usize>; 256];
    for (row, shades) in chrome::MINIMAP_REALM_RAMP.iter().enumerate() {
        for &i in shades {
            assert_eq!(
                row_of_index[i as usize], None,
                "palette index {i:#04x} is in two ramp rows, so a pixel cannot name its realm"
            );
            row_of_index[i as usize] = Some(row);
        }
    }

    // **The measurement is a difference, not a census**, and it has to be.
    // `MAPnn.PL8`'s raster also carries pixels the overlay leaves alone —
    // coastline, borders, the panel round it — and some of those indices are
    // by coincidence entries of a ramp row (0x38, the beige, is row 3's
    // darkest). Counting colours across the whole rectangle would therefore
    // report a realm nobody owns. What is unambiguous is what *moves* when the
    // ownership moves: the raster is byte-for-byte the same in both renders, so
    // every differing pixel is one the tint wrote.
    let tinted = draw(&mut MapScreen::new(), &mut game, &assets);
    let owners: Vec<u8> =
        game.kingdom.county_ids().map(|id| game.kingdom.counties[id].owner).collect();
    for id in game.kingdom.county_ids() {
        game.kingdom.counties[id].owner = 0;
    }
    let flat = draw(&mut MapScreen::new(), &mut game, &assets);
    for (id, owner) in game.kingdom.county_ids().zip(owners) {
        game.kingdom.counties[id].owner = owner;
    }

    let mut seen = std::collections::BTreeSet::new();
    let mut moved = 0usize;
    let mut selected_pixels = 0usize;
    for y in 0..chrome::MINIMAP_DIM {
        for x in 0..chrome::MINIMAP_DIM {
            let (px, py) = ((chrome::MINIMAP_X + x) as usize, (chrome::MINIMAP_Y + y) as usize);
            let (a, b) = (tinted.at(px, py), flat.at(px, py));
            if a == chrome::MINIMAP_SELECTED {
                selected_pixels += 1;
            }
            if a == b {
                continue;
            }
            moved += 1;
            assert_eq!(
                row_of_index[b as usize],
                Some(0),
                "an unowned county must paint the raster's own shading, ramp row 0, \
                 and this pixel painted {b:#04x}"
            );
            let row = row_of_index[a as usize].unwrap_or_else(|| {
                panic!("owned land painted {a:#04x}, which is in no ramp row at all")
            });
            seen.insert(row);
        }
    }

    // What the save says the answer should be, worked out from the counties
    // From the painting.
    let mut wanted = std::collections::BTreeSet::new();
    for id in game.kingdom.county_ids() {
        let owner = game.kingdom.counties[id].owner as usize;
        if owner != 0 {
            wanted.insert(chrome::realm_colour(game.realm_colour[owner]) as usize);
        }
    }
    assert!(
        wanted.len() >= 2,
        "this fixture has {} owning colour(s), so it could not tell a tinted minimap \
         from an untinted one even if the tint were gone",
        wanted.len()
    );

    assert_eq!(
        seen, wanted,
        "the minimap's ramp rows must be exactly the ones the counties' owners ask for; \
         **one single row** is the shape of the failure to watch for — `realm_colour` \
         clamps a zero colour up to 1, so a tint that has lost its input does not go \
         blank, it goes uniformly red"
    );
    assert!(
        moved > 500,
        "only {moved} pixels changed when five counties changed hands, which is too few \
         to be five counties"
    );
    assert!(
        selected_pixels > 0,
        "the selected county's brightest shade is replaced by MINIMAP_SELECTED, and none was drawn"
    );
}

// ------------------------------------------------ seasons, fields, animation

/// **The map's artwork changes when the season does — driven by a real turn.**
///
/// `Gfx_LoadCountyMode` (`0x004984DC`) repoints all five near-zoom tile banks
/// at `(g_season - 1) * 8` in `g_resourceTable`, so the whole picture is
/// redrawn from different files. We hard-coded the `a` set and the map looked
/// the same in January and in August.
///
/// **This ends a turn.** A test that sets
/// `kingdom.season = 4` and then reads a lookup table is checking its own
/// fixture (`docs/agents.md`); the season has to be moved by the thing that
/// moves it in play. `l2_game::turn::end_turn` runs the whole phase machine.
#[test]
fn a_real_turn_turns_the_season_and_the_map_is_repainted_from_other_files() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let before_season = game.kingdom.season;
    let before = draw(&mut screen, &mut game, &assets);

    l2_game::turn::end_turn(&mut game).expect("the turn completes without asking");
    let after_season = game.kingdom.season;
    assert_ne!(after_season, before_season, "one turn is one season");

    let after = draw(&mut screen, &mut game, &assets);
    let moved = before.diff_count(&after);

    // The viewport is 480 x 450 under the menu bar — about 216,000 pixels — and
    // a whole-bank swap repaints essentially all of it. The floor is what
    // matters: hard-coding one season made this zero.
    assert!(
        moved > 50_000,
        "the season turned from {before_season} to {after_season} and only {moved} pixels moved"
    );

    // …and it is the *artwork* that changed, not our own markers: the two
    // frames must use meaningfully different palettes. Winter is the bright one
    // and autumn has no green in it (`campaign::SEASON_SUFFIX`).
    let histogram = |c: &Canvas| {
        let mut h = [0u32; 256];
        for &p in &c.pixels {
            h[p as usize] += 1;
        }
        h
    };
    let (a, b) = (histogram(&before), histogram(&after));
    let differing = (0..256).filter(|&i| a[i].abs_diff(b[i]) > 200).count();
    assert!(differing > 8, "only {differing} palette entries changed their share of the frame");
}

/// **A town drawn through `Overrides` in one season is still a town in the
/// next** — which is the thing the season swap could have broken.
/// `install.rs::the_four_seasons_of_a_bank_are_the_same_frame_table` exists.
///
/// The overrides plane stores a **frame index**, not a picture. If frame 47 of
/// `Town1c.pl8` were a different cell of the sheet than frame 47 of
/// `Town1a.pl8`, every county town on the map would turn back into a quarry in
/// autumn — a defect a player would report as *"my buildings disappear"*. The
/// frame tables agree, so it does not happen, and this asserts the consequence
/// At the pixel.
#[test]
fn a_towns_overridden_graphic_survives_every_season() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    let tile = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        *MapScreen::town(&ctx, 8).first().expect("county 8 has a town")
    };
    let (tx, ty) = l2_kingdom::map::coords(tile);
    screen.centre_on_tile(tx as usize, ty as usize);

    let slot = assets.slot(game.map_slot).expect("the map slot");
    let lattice = campaign::Lattice::build(&slot);
    let overrides = {
        let ctx = Ctx { game: &mut game, assets: &assets };
        MapScreen::tile_graphics(&ctx)
    };

    let paint = |season: u8, o: &campaign::Overrides| {
        let mut canvas = Canvas::screen();
        let mut tags = l2_view::Tags::screen();
        campaign::draw(
            &mut canvas,
            &slot,
            &lattice,
            &assets.map,
            screen.viewport(),
            screen.zoom(),
            &mut tags,
            o,
            season,
            None,
        );
        canvas
    };
    let bare = campaign::Overrides::new();
    let mut seasons_that_differ = 0;
    for season in 1..=campaign::SEASONS as u8 {
        let with = paint(season, &overrides);
        let without = paint(season, &bare);
        let moved = with.diff_count(&without);
        assert!(
            moved > 500,
            "season {season}: the override changed only {moved} pixels, so the town is not \
             being restamped"
        );
        seasons_that_differ += 1;
    }
    assert_eq!(seasons_that_differ, 4, "all four seasons keep their towns");
}

/// **The minimap closes the management surface from any screen**, which is
/// `Screen_FrameInput`'s epilogue and not any screen's own arm.
///
/// `docs/arms.json` `0x0042FF10/minimap-closes-the-surface`.
#[test]
fn a_press_on_the_minimap_drops_whatever_is_open_over_the_map() {
    let (mut game, assets) = world!();
    game.select(8);
    let hit = chrome::minimap_hit_area();
    let (mx, my) = (hit.x0 + 40, hit.y0 + 40);

    // The three graduated map overlays are here because graduating them out of
    // the shell table LOST this arm: the wrapper reproduced it once for all
    // seven shells and each screen now has to carry it. That regression was
    // invisible until this test and the graduation met in one merge.
    for over in [
        ScreenId::County(8, Panel::Tax),
        ScreenId::Job(8, 0),
        ScreenId::Court,
        ScreenId::Ratings,
        ScreenId::Supplies(8),
    ] {
        let mut m = over_the_map(over);
        send_stack(&mut m, &mut game, &assets, Event::Click { x: mx, y: my });
        assert_eq!(m.top_id(), Some(ScreenId::Campaign), "{over:?} gave way to the minimap");
        assert_eq!(m.depth(), 1, "{over:?}: the map is revealed, not rebuilt");
    }

    // The ablation: a press just OUTSIDE the raster leaves everything alone. An
    // unconditional `Pass` would close on both.
    let mut m = over_the_map(ScreenId::Job(8, 0));
    send_stack(&mut m, &mut game, &assets, Event::Click { x: hit.x1 + 4, y: my });
    assert_eq!(m.top_id(), Some(ScreenId::Job(8, 0)), "outside the raster nothing happens");
}

// --- the fog of war ----------------------------------------------------------
//
// `l2_kingdom::explore` has every reader and writer of the original's seen bit.
// These are the painters' half, and each assertion is about a *tile*: what is
// drawn on one the person has not seen, and what is drawn once he has.
//
// **Ablations, run on this branch** — each line deleted, and the test named:
//
// | deleted | red |
// |---|---|
// | the fog arm of `campaign::draw` (`Map_DrawTile`) | `a_dark_tile_…` |
// | `if fog.is_some() { 0 }` on the surround | `with_the_fog_on_the_sea_…` |
// | the `hides_tile` test in `draw_units` (`Map_DrawArmies`) | `a_county_in_the_dark_…` |
// | `.filter(lit)` on the town banner (`Sprite_TopIt` arm 1) | `a_county_in_the_dark_…` |
// | `.filter(lit)` on the mercenary marker (arm 2) | `a_county_in_the_dark_…` |
// | the `hides_tile` test in `draw_herds` (arm 4) | `a_county_in_the_dark_…` |
// | the `hides_tile` test on our owner marker | `a_county_in_the_dark_…` |
// | `exploration &&` in `l2_kingdom::explore::hides` | `a_county_in_the_dark_…` (the control), and `explore::tests::the_painters_test_…` |
//
// **Two of those rows were green the first time they were run, and the test
// was wrong both times, not the gate.** The herd gate: the field chosen already
// had a herd, so the herd was in both renders. The option test: the control
// also repainted a field's terrain, which moves the fog-off render whatever the
// option test says. The comments at `pasture` and `give_away` say what changed.
//
// **Not covered, and said so:** the castle's garrison banner (England at turn
// one has no garrison), the industry wheel's pause in the dark (a clock drives
// it, and no test here ticks the map), and our field markers (drawn only for
// the person's own county, which is never dark).

