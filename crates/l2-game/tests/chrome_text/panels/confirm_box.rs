#![allow(unused_imports)]
use super::*;
use super::drop_down_plate::*;
use super::title_and_build::*;
use super::tile_panel::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::screen::{Ctx, Screen};
use l2_game::shell::font::{self, Font, Style};
use l2_game::{scenario, Game};
use l2_kingdom::tables::Tables;
use l2_mods::Platform;
use l2_view::Canvas;

/// **The quit box's prompt is `L2.eng` 10/0 as the file spells it, at
/// `(g_confirmX + 0x10, g_confirmY + 0x10)`.**
///
/// ```c
/// /* Menu_Quit 0x004343F8 */       Ui_OpenConfirm(0, 0xA0, 0xA0, FUN_0043441C);
/// /* Screen_ConfirmBox 0x0040CCFA */
/// FUN_004093E0(g_confirmX - 0x10, g_confirmY - 0x10, 0xE, 8);
/// Eng_DrawString(10, g_confirmPrompt, g_confirmX + 0x10, g_confirmY + 0x10, &g_fontBody, 0x3F);
/// ```
///
/// `Eng_DrawString` draws the group's bytes. **Ablated** by putting
/// `.to_uppercase()` back in `ConfirmScreen::draw`: both assertions go red —
/// the caps are not the line, and they run past the box's right edge.
#[test]
fn the_quit_boxs_prompt_is_the_eng_line_inside_its_box() {
    use l2_game::screens::confirm::{Ask, ConfirmScreen};
    let (mut game, assets) = world!();
    let body = assets.shell.body.as_ref().expect("Fntl2_14.pl8");
    let prompt = assets.shell.text(10, 0).to_string();
    assert_eq!(prompt, "Exit the game?", "L2.eng 10/0");

    let mut screen = ConfirmScreen::new(Ask::Quit);
    let canvas = draw(&mut screen, &mut game, &assets);

    const AT: (i32, i32) = (0xA0 + 0x10, 0xA0 + 0x10);
    assert_eq!(
        find_on_row(&canvas, body, &prompt, font::TEXT, AT.1),
        Some(AT.0),
        "{prompt:?} is not at {AT:?}",
    );
    // `FUN_004093E0(0x90, 0x90, 0xE, 8)` — the interior is inset one cell.
    let right = 0x90 + 0xE * 16 - 16;
    assert!(
        AT.0 + body.width(&prompt) <= right,
        "{prompt:?} runs to {} and the box's interior ends at {right}",
        AT.0 + body.width(&prompt),
    );
}
