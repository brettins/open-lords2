//! C257: what a joiner and the host see in the menus of a network game.

use super::dropdown::DropdownScreen;
use super::items::*;
use crate::game::{Assets, Game};
use crate::input::Event;
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::screens::leave::{Choice, LeaveScreen, Leaving};
use crate::screens::setup::SetupPage;

fn seat(hosting: bool) -> Game {
    let mut g = Game::new(1);
    (g.net.lockstep, g.net.hosting) = (true, hosting);
    g
}

fn file_menu(game: &mut Game) -> Vec<(Item, String)> {
    let assets = Assets::placeholder();
    let ctx = Ctx { game, assets: &assets };
    let rows = rows(ctx.game, 0);
    (0..rows.len()).map(|i| (rows[i].2, item_text(&ctx, 0, i))).collect()
}

/// Ablation: the `(0, true, false)` arm of `rows` (the joiner sees New Game, Load, Save, Quit).
#[test]
fn a_joiner_sees_only_leave_game_in_the_file_menu() {
    let menu = file_menu(&mut seat(false));
    assert_eq!(menu, vec![(Item::LeaveGame, "Leave game".to_string())]);
}

/// Ablation: the `(0, true, true)` arm of `rows`.
#[test]
fn the_host_sees_save_and_end_game_for_everyone() {
    let menu = file_menu(&mut seat(true));
    let items: Vec<Item> = menu.iter().map(|m| m.0).collect();
    assert_eq!(items, vec![Item::Save, Item::EndGame]);
    assert_eq!(menu[1].1, "End game for all");
}

#[test]
fn single_player_keeps_the_originals_four() {
    let mut g = Game::new(1);
    let items: Vec<Item> = file_menu(&mut g).iter().map(|m| m.0).collect();
    assert_eq!(items, vec![Item::NewGame, Item::Load, Item::Save, Item::Quit]);
    g.net.lockstep = true;
    let assets = Assets::placeholder();
    let options = rows(Ctx { game: &mut g, assets: &assets }.game, 1);
    assert_eq!(options.len(), 5, "the other menus are unchanged");
}

/// Ablation: `Item::LeaveGame | Item::EndGame => Replace(Leave)` in `DropdownScreen::run`.
#[test]
fn the_joiners_item_opens_the_leave_box_and_its_yes_asks_to_leave() {
    let assets = Assets::placeholder();
    let mut game = seat(false);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    let t = super::items::titles(&ctx);
    let r = row_rect(&t, 0, 0);
    let mut menu = DropdownScreen::new(0);
    let opened = menu.handle(Event::Click { x: r.x + 4, y: r.y + 4 }, &mut ctx);
    assert_eq!(opened, Transition::Replace(ScreenId::Leave));
    assert_eq!(LeaveScreen::choices(&ctx), vec![Choice::Leave, Choice::Back]);
    let mut leave = LeaveScreen::new();
    let yes = LeaveScreen::rects(&ctx)[0];
    let t = leave.handle(Event::Click { x: yes.x + 4, y: yes.y + 4 }, &mut ctx);
    assert_eq!(t, Transition::Goto(ScreenId::Setup(SetupPage::Title)));
    assert_eq!(ctx.game.leaving, Some(Leaving::Leave));
}

/// Ablation: `crate::saves::write` in `LeaveScreen::press` (no save before the end).
#[test]
fn the_hosts_save_then_end_writes_a_save_and_ends_the_game_for_everyone() {
    let dir = std::env::temp_dir().join(format!("l2-c257-end-{}", std::process::id()));
    let _scope = crate::saves::scoped_dir(&dir);
    let assets = Assets::placeholder();
    let mut game = seat(true);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    assert_eq!(LeaveScreen::choices(&ctx), vec![Choice::SaveAndEnd, Choice::EndWithoutSaving, Choice::Back]);
    let mut leave = LeaveScreen::new();
    let _ = leave.handle(Event::KeyDown(crate::input::Key::Enter), &mut ctx);
    assert_eq!(ctx.game.leaving, Some(Leaving::EndForEveryone));
    assert_eq!(crate::saves::list().len(), 1, "the save before the end");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Ablation: `AdvancedFarming if lockstep => {}` in `options::page::toggle`.
#[test]
fn advanced_farming_only_shows_in_a_network_game() {
    let assets = Assets::placeholder();
    let mut game = seat(false);
    let mut ctx = Ctx { game: &mut game, assets: &assets };
    crate::screens::options::toggle(crate::screens::options::Setting::AdvancedFarming, &mut ctx);
    assert!(ctx.game.net.outbox.is_empty(), "a seat changed the rule for everyone");
    ctx.game.net.lockstep = false;
    let was = ctx.game.kingdom.options.advanced_farming;
    crate::screens::options::toggle(crate::screens::options::Setting::AdvancedFarming, &mut ctx);
    assert_ne!(ctx.game.kingdom.options.advanced_farming, was, "single player still flips it");
}
