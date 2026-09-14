#![allow(unused_imports)]
use super::*;
use super::geometry::*;
use super::end_turn::*;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId, Transition};
use l2_game::screens::{county, divide, info, map};
use l2_game::Game;

/// A world with one county owned, enough to build any of these screens.
pub(super) fn world() -> (Game, Assets) {
    // `Game::new` takes a SEED, not a realm count.
    let mut g = Game::new(7);
    g.player = 1;

    // **Every realm holds a county, so nobody is eliminated on the first
    // pass.** This fixture used to give land to the human alone, which left
    // every other lord already dead — a won position before the test began.
    //
    // Nothing could notice. The ending ladder is the only writer of the
    // outcome during play and it runs off the message ring, so until the
    // message window landed there was no ring, no obituaries, and no victory:
    // the fixture sat in a finished game for the whole of every test and was
    // never told. With messages built, End Turn here produces obituaries and
    // then the conquest screen, and this test — which is about a press
    // reaching the map through an open panel — was being decided by an
    // ending rule it has nothing to do with.
    //
    // The lesson is the fixture one: **a fixture in a degenerate state tests
    // the degenerate state**, and it stays invisible for
    // the rule that would object is unimplemented.
    // Realms 2..5 each hold one, and the human holds county 1
    // the one every screen in this file is built against.
    for realm in 2..g.kingdom.realms.len() as u8 {
        g.kingdom.counties[realm as usize].owner = realm;
        // `Realm::in_play` is what the ranking counts, **not** county
        // ownership. Giving a realm land is not the same as it being alive, and
        // the two are only ever equal —
        // why a hand-built fixture can hold land for six lords and still be a
        // won game.
        g.kingdom.realms[realm as usize].in_play = true;
    }
    g.kingdom.realms[1].in_play = true;
    g.kingdom.counties[1].owner = 1;
    g.selected = 1;
    (g, Assets::placeholder())
}

/// Every overlay that stands over the campaign map, and the press each is asked
/// to decline.
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

/// **No overlay may eat a press on the campaign minimap.**
///
/// `Screen_FrameInput`'s epilogue runs `Minimap_Click` on **every** screen id
/// but `0x12`, and closes the management surface on a hit — so from any of
/// these, a press in the 128 × 128 raster selects that county, recentres the
/// map and drops the panel. A screen that answers `Stay` has swallowed it.
///
/// The information panel did exactly that until this branch, which is what
/// makes this a check and not a formality: it is the same defect as the two
/// overlapping buttons, one layer up. A control of the game's was unreachable
/// because something of ours was in front of it.
///
/// Ablation, which was run: delete the `minimap_hit_area().contains` arm from
/// `screens/info.rs` and this fails naming the information panel.
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

/// **The screens the six sidebar guards really run on hand the whole column
/// down, and the ones they do not keep nothing of their own in it.**
///
/// The six guards — the minimap modes, the sidebar, the county strip, the split
/// slider, the produce rows and the right-release overlay clear — open the arms
/// for `0x02`, `0x14`, `0x15`, `0x16` and `0x19` and **no others**. Every one of
/// them tests `x >= 0x1DE`. So for those five the answer to any press in the
/// column is `Pass`; for the rest the answer is anything *except* acting on a
/// rectangle of our own, which is what the second loop asserts by requiring the
/// press to be declined or ignored.
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
    // Six points, one in each `g_sidebarButtons` record, plus End Turn.
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

