//! Row placeholders-everywhere: with the install's art and the Ctrl+D overlay
//! off, no screen draws our 5x7 debug font or our own `widget` rectangles.
//! Ablation: set `debug_overlay = true` below and 17 screens go red.

#[path = "../common/mod.rs"]
#[macro_use]
mod common;
use common::*;

use l2_game::screens::county::Panel;
use l2_game::screens::options::Page;
use l2_game::screens::saveload::Mode;
use l2_game::screens::setup::SetupPage;

/// (debug-font `text::draw` calls, `widget::frame` calls) for one stack draw.
fn marks(id: ScreenId, game: &mut Game, assets: &Assets) -> (usize, usize) {
    let mut m = over_the_map(id);
    let (t, f) = (text::drawn(), l2_game::widget::framed());
    draw_stack(&mut m, game, assets);
    (text::drawn() - t, l2_game::widget::framed() - f)
}

fn screens() -> Vec<ScreenId> {
    let mut v = vec![
        ScreenId::Campaign,
        ScreenId::Armoury(1),
        ScreenId::Rack(1, 0),
        ScreenId::Castle(1),
        ScreenId::Village(1),
        ScreenId::Job(1, 0),
        ScreenId::Diplomacy,
        ScreenId::Nobles,
        ScreenId::MenuBar(1),
        ScreenId::SaveLoad(Mode::Save),
        ScreenId::SaveLoad(Mode::Load),
    ];
    for p in [Panel::Tax, Panel::Ration, Panel::Population, Panel::Happiness] {
        v.push(ScreenId::County(1, p));
    }
    for p in [Page::Advanced, Page::Sound, Page::Display, Page::Help] {
        v.push(ScreenId::Options(p));
    }
    use SetupPage::*;
    for p in [Title, Options, Load, Shield, Campaign, GameType, Custom, Skirmish, SkirmishFile] {
        v.push(ScreenId::Setup(p));
    }
    v
}

fn first(game: &Game, kind: l2_kingdom::UnitKind) -> Option<usize> {
    let units = &game.kingdom.campaign.units;
    units.iter().find(|(_, u)| u.kind == kind).map(|(id, _)| id)
}

fn unit_screens(game: &Game) -> Vec<ScreenId> {
    let army = first(game, l2_kingdom::UnitKind::Army).unwrap_or(0);
    let merchant = first(game, l2_kingdom::UnitKind::Merchant).expect("six merchants");
    vec![
        ScreenId::Merchant(merchant),
        ScreenId::Trade(merchant, 0),
        ScreenId::Siege(army),
        ScreenId::Divide(army),
        ScreenId::RaiseArmy(1),
        ScreenId::Supplies(1),
        ScreenId::Confirm(l2_game::screens::confirm::Ask::Quit),
        ScreenId::Ratings,
        ScreenId::Court,
        ScreenId::BattlePrompt,
        ScreenId::BattleResult,
        ScreenId::Battlefield,
        ScreenId::Conquest,
    ]
}

#[test]
fn no_screen_draws_the_debug_font_or_our_rectangles_with_the_overlay_off() {
    let (mut game, assets) = world!();
    game.prefs.debug_overlay = false;
    let mut bad = Vec::new();
    let units = unit_screens(&game);
    for id in screens().into_iter().chain(units) {
        let (t, f) = marks(id, &mut game, &assets);
        if t + f != 0 {
            bad.push(format!("{id:?}: {t} debug-font, {f} rectangles"));
        }
    }
    assert!(bad.is_empty(), "placeholders in normal play:\n{}", bad.join("\n"));
}
