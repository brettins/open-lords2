
#[macro_use]
mod common;

use common::*;

use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::input::Key;
use l2_game::screen::Ctx;
use l2_game::screen::Machine;
use l2_game::screen::ScreenId;
use l2_game::screen::Transition;
use l2_game::screens::county::{self as county};
use l2_game::screens::map::MapScreen;
use l2_view::Canvas;


/// **PNG, not raw RGBA.** `docs/decisions.md` C21's conclusion is *show screens
/// early, to someone who knows the game*, and it cost this project a whole map
/// screen to learn. A `.rgb` dump does not do that — it needs a converter and a
/// remembered width before anyone can glance at it, which is enough friction
/// that nobody glances.
pub(crate) mod png {
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

fn save_png(canvas: &Canvas, assets: &Assets, name: &str) {
    let mut rgba = vec![0u8; 640 * 480 * 4];
    canvas.to_rgba(&assets.palette, &mut rgba);
    let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    std::fs::create_dir_all("out").unwrap();
    std::fs::write(format!("out/{name}.png"), png::encode(640, 480, &rgb)).unwrap();
}

#[test]
#[ignore]
fn shoot() {
    let (mut game, assets) = world!();
    let mut screen = MapScreen::new();
    for (name, zoomed) in [("near", false), ("far", true)] {
        if zoomed {
            send(&mut screen, &mut game, &assets, Event::KeyDown(Key::Char('Z')));
        }
        let canvas = draw(&mut screen, &mut game, &assets);
        save_png(&canvas, &assets, &format!("campaign_{name}"));
    }

    let county = (1..=game.kingdom.county_count as u8)
        .find(|&id| game.is_players(id))
        .expect("the player holds a county");
    game.select(county);
    let (tile, _) = game
        .kingdom
        .field_tiles(county as usize)
        .into_iter()
        .find(|&(_, k)| k == l2_kingdom::field::FieldType::Fallow)
        .expect("a fallow field");

    let mut screen = MapScreen::new();
    let (x, y) = on_screen(&mut screen, tile);
    let canvas = draw(&mut screen, &mut game, &assets);
    save_png(&canvas, &assets, "brush_before");

    let opened = send(&mut screen, &mut game, &assets, Event::Click { x, y });
    let target = l2_game::screens::info::Target::Tile(tile);
    assert_eq!(opened, Transition::Push(ScreenId::Info(target)));
    let mut panel = l2_game::screens::info::InfoScreen::new(target);
    let mut canvas = draw(&mut screen, &mut game, &assets);
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        panel.draw(&ctx, &mut canvas);
    }
    save_png(&canvas, &assets, "brush_open");

    send(&mut panel, &mut game, &assets, Event::Click { x: 328, y: 400 });
    let canvas = draw(&mut screen, &mut game, &assets);
    save_png(&canvas, &assets, "brush_after");

    let mut screen = MapScreen::new();
    let ctx = Ctx { game: &mut game, assets: &assets };
    if let Some(&tile) = MapScreen::town(&ctx, county).first() {
        let (tx, ty) = l2_kingdom::map::coords(tile);
        screen.centre_on_tile(tx as usize, ty as usize);
    }
    let canvas = draw(&mut screen, &mut game, &assets);
    save_png(&canvas, &assets, "county_town");

    {
        let realm = game.kingdom.realms[game.player as usize].clone();
        let basket = l2_kingdom::LevyBasket::seed(&realm, 300);
        let _ = game.raise_army(county, &basket, 10, None);
        let (ax, ay) = {
            let c = &game.kingdom.counties[county as usize];
            (c.anchor_x, c.anchor_y)
        };
        let merchant = game
            .kingdom
            .campaign
            .units
            .iter()
            .find(|(_, u)| u.kind == l2_kingdom::UnitKind::Merchant)
            .map(|(id, _)| id);
        if let Some(u) = merchant.and_then(|id| game.kingdom.campaign.units.get_mut(id)) {
            u.x = ax.saturating_sub(1);
            u.y = ay;
            u.county = county;
        }
        let mut screen = MapScreen::new();
        let canvas = draw(&mut screen, &mut game, &assets);
        save_png(&canvas, &assets, "units_and_flags");
    }

    {
        let c = &mut game.kingdom.counties[county as usize];
        let cattle = l2_kingdom::tables::JOB_CATTLE_FARMING;
        c.labour[l2_kingdom::tables::JOB_IDLE_TOWNSFOLK] = 40;
        c.labour[cattle] = c.labour[cattle].max(120);
        c.labour_useful[cattle] = 40;
        c.labour_wanted[cattle] = -1;
        c.herd = c.herd.max(300);
        c.fields_cattle = c.fields_cattle.max(3);
    }
    let mut m = Machine::new(ScreenId::Campaign);
    let mut c = Ctx { game: &mut game, assets: &assets };
    let mut canvas = Canvas::screen();
    m.draw(&c, &mut canvas);
    let _ = &mut c;
    save_png(&canvas, &assets, "sidebar_blue_outline");

    for panel in county::PANELS {
        let mut m = Machine::new(ScreenId::County(county, panel));
        let mut c = Ctx { game: &mut game, assets: &assets };
        let mut canvas = Canvas::screen();
        m.draw(&c, &mut canvas);
        let _ = &mut c;
        save_png(&canvas, &assets, &format!("panel_{panel:?}").to_lowercase());
    }
}
