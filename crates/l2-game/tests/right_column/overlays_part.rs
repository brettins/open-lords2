#![allow(unused_imports)]
use super::*;
use super::geometry::*;
use super::end_turn::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId, Transition};
use l2_game::screens::{county, divide, info, map};
use l2_game::Game;

pub(super) fn world() -> (Game, Assets) {
    let mut g = Game::new(7);
    g.player = 1;

    for realm in 2..g.kingdom.realms.len() as u8 {
        g.kingdom.counties[realm as usize].owner = realm;
        g.kingdom.realms[realm as usize].in_play = true;
    }
    g.kingdom.realms[1].in_play = true;
    g.kingdom.counties[1].owner = 1;
    g.selected = 1;
    (g, Assets::placeholder())
}

fn overlays() -> Vec<(&'static str, ScreenId)> {
    vec![
        ("information panel 0x04", ScreenId::Info(info::Target::Tile(0))),
        ("county population 0x14", ScreenId::County(1, county::Panel::Population)),
        ("county tax 0x15", ScreenId::County(1, county::Panel::Tax)),
        ("county happiness 0x16", ScreenId::County(1, county::Panel::Happiness)),
        ("county rations 0x19", ScreenId::County(1, county::Panel::Ration)),
        ("village 0x02", ScreenId::Village(1)),
        ("job popup 0x0F", ScreenId::Job(1, 0)),
        ("court 0x09", ScreenId::Court),
        ("send supplies 0x18", ScreenId::Supplies(1)),
        ("ratings 0x2E", ScreenId::Ratings),
    ]
}

#[test]
fn no_overlay_swallows_the_campaign_minimap() {
    let (mut g, a) = world();
    let raster = l2_view::chrome::minimap_hit_area();
    let at = ((raster.x0 + raster.x1) / 2, (raster.y0 + raster.y1) / 2);

    for (name, id) in overlays() {
        let mut screen = id.build();
        let mut ctx = Ctx { game: &mut g, assets: &a };
        let t = screen.handle(Event::Click { x: at.0, y: at.1 }, &mut ctx);
        assert!(
            matches!(t, Transition::Pass),
            "{name} answered {t:?} to a press on the minimap raster at {at:?}; \
             the epilogue's Minimap_Click is live on every screen id but 0x12, so this \
             screen has to hand the press down. `docs/arms.json` 0x0042FF10/minimap-closes-the-surface.",
        );
    }
}

#[test]
fn only_the_five_screens_with_the_sidebar_guards_claim_the_column() {
    let (mut g, a) = world();
    let with_guards = [
        ScreenId::Village(1),
        ScreenId::County(1, county::Panel::Population),
        ScreenId::County(1, county::Panel::Tax),
        ScreenId::County(1, county::Panel::Happiness),
        ScreenId::County(1, county::Panel::Ration),
    ];
    let mut points: Vec<(String, (i32, i32))> = map::SIDEBAR_BUTTONS
        .iter()
        .map(|b| (b.name.to_string(), (b.rect().centre_x(), b.rect().y + b.rect().h / 2)))
        .collect();
    points.push((
        "END TURN".into(),
        (
            map::END_TURN_BUTTON.centre_x(),
            map::END_TURN_BUTTON.y + map::END_TURN_BUTTON.h / 2,
        ),
    ));

    for id in with_guards {
        for (what, at) in &points {
            let mut screen = id.build();
            let mut ctx = Ctx { game: &mut g, assets: &a };
            let t = screen.handle(Event::Click { x: at.0, y: at.1 }, &mut ctx);
            assert!(
                matches!(t, Transition::Pass),
                "{id:?} answered {t:?} to a press on {what} at {at:?}. \
                 The six guards this screen's arm opens with all test x >= 0x1DE and the \
                 column stays live; a BACK TO MAP button of ours used to be here, on End Turn.",
            );
        }
    }
}

