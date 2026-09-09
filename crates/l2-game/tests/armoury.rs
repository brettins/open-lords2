//! **The armoury against the real artwork**, headlessly.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" \
//! LORDS2_FIXTURES="E:\dev\lords2-fixtures" cargo test -p l2-game --test armoury
//! ```
//!
//! `crates/l2-game/tests/military.rs` walks the whole verb on
//! `Assets::placeholder`, which is the configuration a broken hit test and the
//! picture agree in. This file is the other half: **the hit map and the sprites
//! it is supposed to sit on, out of the install.** `docs/decisions.md` C58 —
//! every campaign-map test on this project ran on the placeholder once, and a
//! near-miss reached a player three times.
//!
//! The strongest assertion here is the last one. It moves **one field of the
//! world** — a realm's stock of one weapon — and requires the *same pixels* to
//! appear and disappear, which is the shape the flag and minimap tests were
//! rewritten into after a diff-in-a-box passed a wrong sprite.

use std::path::PathBuf;

use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::armoury;
use l2_game::Game;
use l2_kingdom::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_view::Canvas;

fn install() -> Option<PathBuf> {
    l2_testkit::install_dir()
}

macro_rules! world {
    () => {{
        let Some(dir) = install() else {
            l2_testkit::skip!("no game install, so there is no armoury to walk into");
        };
        let platform =
            l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let save = l2_testkit::england!();
        let game = scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads");
        (game, assets)
    }};
}

/// A county the local player holds. The England fixture's realm→county
/// assignment is **rolled per game** (`docs/environment.md`), so this is found
/// rather than written down.
fn own_county(g: &Game) -> u8 {
    (1..=g.kingdom.county_count as u8)
        .find(|&id| g.is_players(id))
        .expect("the local player holds a county")
}

fn frame(m: &mut Machine, g: &mut Game, a: &Assets) -> Canvas {
    let mut c = Canvas::screen();
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, &mut c);
    c
}

/// The bounding box of one weapon's cells in `arm_grid.pl8`.
fn grid_box(a: &Assets, weapon: u8) -> Option<Rect> {
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for y in (0..480).step_by(8) {
        for x in (0..640).step_by(8) {
            if a.shell.armoury_grid(x, y) == Some(weapon) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 8);
                y1 = y1.max(y + 8);
            }
        }
    }
    (x1 > x0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
}

/// **`arm_grid.pl8` names the six weapon types and nothing else usable.**
///
/// The shipped file holds 26 cells outside 1…6 — column 0 down the left edge
/// and an eleven-cell sliver at y 216 — and in the original every one of them
/// is a live hotspot that indexes the eight-slot levy basket with 60-something.
/// `docs/bugs.md` N13. We answer `None`, and this is where that is asserted
/// against the file rather than against our own belief about it.
#[test]
fn the_hit_map_names_six_weapons_and_the_stray_cells_are_refused() {
    let (_g, assets) = world!();
    assert!(assets.shell.has_armoury_grid(), "arm_grid.pl8 is in the install");

    let mut seen: Vec<u8> = Vec::new();
    for y in (0..480).step_by(8) {
        for x in (0..640).step_by(8) {
            if let Some(t) = assets.shell.armoury_grid(x, y) {
                if !seen.contains(&t) {
                    seen.push(t);
                }
            }
        }
    }
    seen.sort_unstable();
    assert_eq!(seen, vec![1, 2, 3, 4, 5, 6], "one region per weapon type, and no seventh");

    // The two known stray patches, named by position so that a different file
    // fails loudly rather than quietly agreeing.
    assert_eq!(assets.shell.armoury_grid(0, 0), None, "the left-edge strip is not a rack");
    assert_eq!(assets.shell.armoury_grid(0, 112), None, "nor its last row");
    assert_eq!(assets.shell.armoury_grid(560, 216), None, "nor the sliver at y 216");
}

/// **Every rack's hit region sits on the weapon it opens.**
///
/// `g_armouryWallItems` says where each weapon is painted and `arm_grid.pl8`
/// says where it can be clicked; the two are different tables written by
/// different people, and if they had drifted the player would click a bow and
/// get a pike. The sprite's own width and height come out of the `.pl8`, so
/// this compares the picture with the hit map and nothing with itself.
#[test]
fn every_weapon_can_be_clicked_where_it_hangs() {
    let (_g, assets) = world!();
    let sheet = assets.shell.sheet(armoury::items_sheet(1)).expect("arm_it_r.pl8");

    for (slot, &(frame_index, x, y)) in armoury::WALL.iter().enumerate() {
        let weapon = slot as u8 + 1;
        let f = sheet.frame(frame_index).expect("a wall frame");
        let sprite = Rect::new(x, y, f.width as i32, f.height as i32);
        let region = grid_box(&assets, weapon).unwrap_or_else(|| panic!("weapon {weapon} has no cells"));

        // The grid's box and the sprite's box must be the same thing to within
        // a cell of rounding: same centre, and neither more than 16 pixels
        // adrift on any edge.
        for (name, a, b) in [
            ("left", region.x, sprite.x),
            ("top", region.y, sprite.y),
            ("right", region.x + region.w, sprite.x + sprite.w),
            ("bottom", region.y + region.h, sprite.y + sprite.h),
        ] {
            assert!(
                (a - b).abs() <= 16,
                "weapon {weapon}: the hit map's {name} edge is {a} and the sprite's is {b}",
            );
        }

        // And the hotspot rectangle — the fallback for a missing grid — covers
        // the rack sprite at the bottom of the screen rather than the wall.
        let h = armoury::RACK_HOTSPOTS.iter().find(|h| h.4 == weapon).expect("a hotspot");
        assert!(h.1 >= 396, "weapon {weapon}'s fallback rectangle is not on the bottom row");
    }
}

/// **The whole walk on the real fixture and the real artwork**: raise a levy in
/// a county the fixture actually gave the player, equip it at the rack the
/// player can actually see, and march out with an army.
///
/// `docs/agents.md` C27 — *a rule with no way in is not a rule the game has* —
/// with the placeholder taken away, so the clicks land on the grid rather than
/// on our rectangles.
#[test]
fn a_levy_raised_on_the_england_fixture_walks_out_of_the_armoury_armed() {
    let (mut g, a) = world!();
    let county = own_county(&g);
    g.selected = county;
    let realm = g.player as usize;

    // Whatever the fixture's realms were given. `docs/kingdom.md` says row 2 of
    // `g_startArmoury` — 50 swords, 50 pikes, 50 bows — but the row is a
    // setup choice, so the weapon is found rather than assumed.
    let stocked = (0..WEAPON_TYPE_COUNT)
        .find(|&s| g.kingdom.realms[realm].weapons[s] > 0)
        .expect("the fixture's realms have an armoury");
    let stock = g.kingdom.realms[realm].weapons[stocked];
    let troop = stocked as u8 + 1;

    let mut m = Machine::new(ScreenId::Campaign);
    let send = |m: &mut Machine, g: &mut Game, e: Event| {
        let mut ctx = Ctx { game: g, assets: &a };
        m.handle(e, &mut ctx);
    };

    send(&mut m, &mut g, Event::KeyDown(l2_game::input::Key::letter('r')));
    {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        m.update(&mut ctx);
    }
    assert_eq!(m.top_id(), Some(ScreenId::RaiseArmy(county)));

    // 40 % of the county, whatever that is on this fixture.
    send(
        &mut m,
        &mut g,
        Event::Click {
            x: l2_game::screens::army::SLIDER_X + 40,
            y: l2_game::screens::army::base(false) + 0x20,
        },
    );
    let men = g.levy.men;
    assert!(men > 50, "40 % of the county is {men} men, which is not enough to raise");

    let cont = l2_game::screens::army::continue_button(false);
    send(&mut m, &mut g, Event::Click { x: cont.centre_x(), y: cont.y + cont.h / 2 });
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(county)));

    // **Click the weapon where it hangs on the wall**, through `arm_grid.pl8`.
    let at = grid_box(&a, troop).expect("the weapon has a region");
    send(&mut m, &mut g, Event::Click { x: at.centre_x(), y: at.y + at.h / 2 });
    assert_eq!(m.top_id(), Some(ScreenId::Rack(county, troop)), "the wall is the button");

    let all = armoury::button_box(3);
    send(&mut m, &mut g, Event::Click { x: all.centre_x(), y: all.y + all.h / 2 });
    let armed = stock.min(men);
    assert_eq!(g.levy.basket.troops()[troop as usize], armed);

    send(
        &mut m,
        &mut g,
        Event::Click { x: armoury::RACK_OK.centre_x(), y: armoury::RACK_OK.y + 12 },
    );
    send(
        &mut m,
        &mut g,
        Event::Click { x: armoury::CREATE_BOX.centre_x(), y: armoury::CREATE_BOX.y + 12 },
    );

    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "Create closed the armoury");
    let (_, unit) = g
        .kingdom
        .campaign
        .units
        .iter()
        .find(|(_, u)| u.kind == l2_kingdom::unit::UnitKind::Army)
        .expect("an army on the map");
    assert_eq!(unit.men, men);
    assert_eq!(unit.troops[troop as usize], armed, "and it is carrying the fixture's weapons");
    assert_eq!(g.kingdom.realms[realm].weapons[stocked], stock - armed);
}

/// **Move one field of the world and the same pixels have to follow.**
///
/// A weapon hangs on the armoury wall exactly when the realm owns one —
/// `FUN_00418426`'s `if (realm.weapons[t - 1] > 0)` — and its rack along the
/// bottom appears exactly when `basket[t].available > 0`, which the seeding
/// fills from the same stock. So emptying one stock has to blank **two**
/// rectangles and nothing else, which a diff-in-a-box cannot claim: this
/// asserts what changed *and* where it did not.
///
/// The two rectangles being the answer is itself the finding. The test was
/// written expecting one, went red with 5,754 pixels adrift, and they were all
/// the crossbowman leaving the bottom row — a realm with no crossbows has
/// nobody who could carry one, and the picture says both things.
#[test]
fn emptying_one_rack_removes_that_weapon_from_the_wall_and_nothing_else() {
    let (mut g, a) = world!();
    let county = own_county(&g);
    let realm = g.player as usize;
    g.kingdom.realms[realm].weapons = [50; WEAPON_TYPE_COUNT];
    g.open_levy(county);

    let mut m = Machine::new(ScreenId::Armoury(county));
    let full = frame(&mut m, &mut g, &a);

    // The crossbow, weapon slot 0 — the tallest of the six and the easiest to
    // see go.
    let (frame_index, x, y) = armoury::WALL[0];
    let sheet = a.shell.sheet(armoury::items_sheet(g.kingdom.realms[realm].shield_index));
    let f = sheet.and_then(|s| s.frame(frame_index)).expect("the crossbow frame");
    let sprite = Rect::new(x, y, f.width as i32, f.height as i32);

    // The crossbowman's own rack, from the hotspot table — the strip along the
    // bottom that holds his portrait and the count under it.
    let rack = armoury::RACK_HOTSPOTS.iter().find(|h| h.4 == 1).expect("the crossbow rack");
    let rack = Rect::new(rack.0, rack.1, rack.2 - rack.0, rack.3 - rack.1);

    g.kingdom.realms[realm].weapons[0] = 0;
    g.seed_levy_basket();
    let short = frame(&mut m, &mut g, &a);

    let (mut wall, mut row, mut outside) = (0usize, 0usize, 0usize);
    for py in 0..480i32 {
        for px in 0..640i32 {
            if full.at(px as usize, py as usize) == short.at(px as usize, py as usize) {
                continue;
            }
            if sprite.contains(px, py) {
                wall += 1;
            } else if rack.contains(px, py) {
                row += 1;
            } else {
                outside += 1;
            }
        }
    }
    assert!(wall > 1_000, "the crossbow did not come off the wall: {wall} pixels changed");
    assert!(row > 500, "the crossbowman did not leave the bottom row: {row} pixels changed");
    assert_eq!(
        outside, 0,
        "{outside} pixels outside the crossbow's wall sprite and its own rack moved: the \
         picture is answering something other than the stock it was asked about",
    );
}

/// The five item sheets are five different pictures, all of them present, and
/// all of them carrying the twenty-one frames the two screens index — six
/// weapons, eight portraits, and the six little icons the levy screen prints
/// its stocks beside.
#[test]
fn all_five_armoury_sheets_carry_the_frames_both_screens_ask_for() {
    let (_g, assets) = world!();
    let mut sizes: Vec<usize> = Vec::new();
    for name in armoury::ITEM_SHEETS {
        let sheet = assets.shell.sheet(name).unwrap_or_else(|| panic!("{name} is in the install"));
        for &(frame, ..) in &armoury::WALL {
            assert!(sheet.frame(frame).is_some(), "{name} has no wall frame {frame}");
        }
        for slot in 0..armoury::RACKS_DRAWN {
            let frame = armoury::RACKS[slot].0;
            assert!(sheet.frame(frame).is_some(), "{name} has no rack frame {frame}");
        }
        for i in 0..WEAPON_TYPE_COUNT {
            let frame = l2_game::screens::army::ICON_FRAME_BASE + i;
            assert!(sheet.frame(frame).is_some(), "{name} has no levy icon {frame}");
        }
        sizes.push(sheet.frame(0).map(|f| f.indices.len()).unwrap_or(0));
    }
    assert!(sizes.iter().all(|&n| n > 0), "a sheet decoded to nothing");

    // And one per weapon type for the rack panel.
    for name in armoury::WEAPON_SHEETS {
        let sheet = assets.shell.sheet(name).unwrap_or_else(|| panic!("{name} is in the install"));
        let f = sheet.frame(0).expect("frame 0");
        assert_eq!((f.width, f.height), (100, 100), "{name}'s picture is not 100 x 100");
    }
}

// ---------------------------------------------------------------------------

/// Not a test: **a way to look at the screen.**
///
/// ```text
/// LORDS2_DIR=... LORDS2_FIXTURES=... \
///   cargo test -p l2-game --test armoury shoot -- --ignored
/// ```
///
/// Writes PNGs into `out/`, which `.gitignore` excludes — a render of the
/// game's own artwork is a derived asset and must never be committed
/// (`CLAUDE.md` rule 1). `docs/decisions.md` C21: *show screens early, to
/// someone who knows the game.* Three shots, which are the three the player's
/// sentence is about: the levy window standing on the armoury, the armoury with
/// the window lifted off it, and one weapon's rack.
///
/// **The palette is `armoury.256`, not the campaign one.** Both screens answer
/// `Screen::palette` with it, and a shot taken through `assets.palette` is the
/// right picture in the wrong colours — which is exactly the defect this whole
/// change is about, so getting it wrong here would hide it.
#[test]
#[ignore]
fn shoot() {
    let (mut g, a) = world!();
    let county = own_county(&g);
    g.selected = county;
    g.kingdom.realms[g.player as usize].weapons = [50, 0, 120, 40, 90, 0];
    g.open_levy(county);
    g.set_levy_percent(35);
    // What *Continue* does, and the reason the basket is not already full:
    // the slider writes g_levyMen and nothing else.
    g.seed_levy_basket();

    let mut m = Machine::new(ScreenId::RaiseArmy(county));
    save_png(&frame(&mut m, &mut g, &a), &a, "levy_on_the_armoury");

    let mut m = Machine::new(ScreenId::Armoury(county));
    save_png(&frame(&mut m, &mut g, &a), &a, "armoury");

    g.levy.basket.equip(l2_kingdom::unit::TroopType::Swordsman, 60);
    let mut m = Machine::new(ScreenId::Armoury(county));
    m.push(ScreenId::Rack(county, 3));
    save_png(&frame(&mut m, &mut g, &a), &a, "armoury_sword_rack");
}

fn save_png(canvas: &Canvas, assets: &Assets, name: &str) {
    let palette = assets.shell.palette("Armoury.256").unwrap_or(&assets.palette);
    let mut rgba = vec![0u8; 640 * 480 * 4];
    canvas.to_rgba(palette, &mut rgba);
let rgb: Vec<u8> = rgba.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    std::fs::create_dir_all("out").unwrap();
    std::fs::write(format!("out/{name}.png"), png::encode(640, 480, &rgb)).unwrap();
}

/// A stored-block PNG encoder with no dependency — the same forty lines
/// `tests/screens.rs` carries, and for the same reason: a `.rgb` dump needs a
/// converter and a remembered width before anyone glances at it.
mod png {
    fn crc32(data: &[u8]) -> u32 {
        let mut table = [0u32; 256];
        for (i, e) in table.iter_mut().enumerate() {
            let mut c = i as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *e = c;
        }
        let mut c = 0xFFFF_FFFFu32;
        for &b in data {
            c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
        }
        c ^ 0xFFFF_FFFF
    }

    fn adler32(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for &x in data {
            a = (a + x as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let mut all = kind.to_vec();
        all.extend_from_slice(body);
        out.extend_from_slice(&all);
        out.extend_from_slice(&crc32(&all).to_be_bytes());
    }

    pub fn encode(w: usize, h: usize, rgb: &[u8]) -> Vec<u8> {
        let mut raw = Vec::with_capacity(h * (1 + w * 3));
        for y in 0..h {
            raw.push(0);
            raw.extend_from_slice(&rgb[y * w * 3..(y + 1) * w * 3]);
        }
        let mut z = vec![0x78, 0x01];
        for (i, block) in raw.chunks(65_535).enumerate() {
            let last = (i + 1) * 65_535 >= raw.len();
            z.push(u8::from(last));
            z.extend_from_slice(&(block.len() as u16).to_le_bytes());
            z.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
            z.extend_from_slice(block);
        }
        z.extend_from_slice(&adler32(&raw).to_be_bytes());

        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(w as u32).to_be_bytes());
        ihdr.extend_from_slice(&(h as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        out
    }
}
