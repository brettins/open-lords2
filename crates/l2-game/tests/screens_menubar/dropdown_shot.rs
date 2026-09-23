use super::*;
use l2_game::screen::Screen;
use l2_game::screens::menubar::DropdownScreen;

/// `FUN_0040C725` 0x0040C725: plate at `y + 0x12`, caption `Eng_DrawString(…, x + 0x10, item.y + y + 0x20)`;
/// `Glyph_Draw` 0x00402A14 adds `record[0x0D]` to `g_drawY`, so y is the cell's top [V].
#[test]
fn the_open_drop_down_is_drawn_where_fun_0040c725_draws_it() {
    let (mut game, assets) = world!();
    let Some(chrome) = assets.chrome.as_ref() else {
        l2_testkit::skip!("no Panels.pl8");
    };
    let ctx = Ctx { game: &mut game, assets: &assets };
    let t = menubar::titles(&ctx);
    let mut shot = Canvas::screen();
    DropdownScreen::new(0).draw(&ctx, &mut shot);
    let mut plate = Canvas::screen();
    chrome.draw_box(&mut plate, t[0].x, 24, 0x0C, (4 * 0x15) / 16 + 2, 2);

    let (mut top, mut bottom) = (i32::MAX, 0);
    for y in 0..60 {
        for x in t[0].x + 0x10..t[0].x + 0xB0 {
            if shot.at(x as usize, y as usize) != plate.at(x as usize, y as usize) {
                top = top.min(y);
                bottom = bottom.max(y);
            }
        }
    }
    if let Ok(dir) = std::env::var("LORDS2_SHOT_DIR") {
        let mut out = b"P6 640 480 255\n".to_vec();
        let mut rgba = vec![0u8; 640 * 480 * 4];
        shot.to_rgba(&assets.palette, &mut rgba);
        out.extend(rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
        std::fs::write(format!("{dir}/dropdown.ppm"), out).unwrap();
    }
    eprintln!("plate top 24, first caption ink rows {top}..={bottom}");
    // Ablation: CAPTION_DY 0x20 -> 0x1F in menubar/mod.rs, and this fails.
    // 6 + 0 + 0x20 = 38 is the cell top; the shadow pass (`Ui_DrawText` y - 1) reaches 37.
    assert_eq!(top, 37, "first caption ink row; the plate's top is 24");
}
