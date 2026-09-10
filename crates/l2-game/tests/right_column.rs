//! **Nothing of ours may sit on a control of the game's.**
//!
//! Three times now a rectangle we invented has been placed in the original's
//! coordinate space without anyone checking what was already there:
//!
//! 1. a COUNTY PANEL button of ours, with a status line written across it, drawn
//!    over the five `g_sidebarButtons` icons — a player reported the icons "did
//!    nothing and had text over them"; they were live artwork under a dead
//!    rectangle;
//! 2. a BACK TO MAP button at (478, 460) 162 × 20 on the four county panels,
//!    which is `g_sidebarButtons` record 5 — **End Turn** — to the pixel, and
//!    hit-tested first;
//! 3. a CANCEL button at (264, 446) 100 × 18 on the army-division screen, which
//!    overlapped `g_splitWidgets` record 0 — **the confirm tick** at (288, 420)
//!    32 × 32 — in a 32 × 6 strip. Our cancel sat on the original's split.
//!
//! Each was found by a person looking. The third had a test of its own that
//! *passed*: it asserted the three buttons were left of `OK`, calling `OK`
//! *"the original's tick"*. `OK` is the corner picture at (428, 436); the tick
//! is somewhere else entirely. **A check that names a rectangle can name the
//! wrong one**, which is why neither test here names one.
//!
//! # The two halves, and why they are different in kind
//!
//! * [`the_right_columns_geometry_is_the_exes_own_tables`] compares our
//!   constants against **the player's `Lords2.exe`**, decoded at run time. That
//!   is two artefacts maintained by different work — ours by us, the table by
//!   Impressions in 1996 — which is the shape `docs/agents.md` says actually
//!   catches things. It is install-gated and skips on CI.
//! * [`no_overlay_swallows_the_campaign_minimap`] drives the machine with
//!   `Event` values and asks a behavioural question no geometry can:
//!   **does this screen consume a press that belongs to something underneath?**
//!   It needs no install, so it runs everywhere, and it is the half that would
//!   have caught defects 1 and 2 on the day they were written.
//!
//! Neither replaces reading the binary. Together they mean an invented hotspot
//! in the campaign column, or an overlay that eats the minimap, goes red on the
//! commit that introduces it.

use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId, Transition};
use l2_game::screens::{county, divide, info, map};
use l2_game::Game;

/// `g_sidebarButtons` (`0x004DC680`) — six 24-byte hotspot records, tested at an
/// offset of (`0x1DE`, `0x1AE`) by `Sidebar_ButtonClicked`.
const SIDEBAR_TABLE: u32 = 0x004D_C680;
const SIDEBAR_OFFSET: (i32, i32) = (0x1DE, 0x1AE);
/// `g_minimapModeButtons` (`0x004DC620`) — four, at (610, 32).
const MINIMAP_MODE_TABLE: u32 = 0x004D_C620;
/// `g_splitWidgets` (`0x004DD388`) — 24-byte **widget** records, tested at (0, 0).
const SPLIT_TABLE: u32 = 0x004D_D388;

/// One hotspot record: `x0, y0, x1, y1` as four `i16`, then the handler.
/// `Hotspot_Test` is **half-open** — `x0 + off <= mx < x1 + off` — so the width
/// is `x1 - x0`.
fn hotspot(t: &l2_testkit::pe::Table, i: usize, off: (i32, i32)) -> Rect {
    let s = |k: usize| t.u16_at(i * 12 + k) as i16 as i32;
    let (x0, y0, x1, y1) = (s(0), s(1), s(2), s(3));
    Rect::new(x0 + off.0, y0 + off.1, x1 - x0, y1 - y0)
}

/// One **widget** record: `x, y` as two `i16`, then frame, size, handler. The
/// box is `size` square at `(x + offx, y + offy)`.
fn widget(t: &l2_testkit::pe::Table, i: usize, off: (i32, i32)) -> Rect {
    let s = |k: usize| t.u16_at(i * 12 + k) as i16 as i32;
    let dim = s(3);
    Rect::new(s(0) + off.0, s(1) + off.1, dim, dim)
}

/// **The tables, out of the player's own copy of the game.**
///
/// Ablation, which was run: change `SIDEBAR_BUTTONS[0].w` from 33 to 32 and the
/// first assertion fails naming record 0. The probe is the exe and the subject
/// is our constant, so there is no way for one to be computed from the other —
/// which is the trap `docs/agents.md` records: *ablating a constant while
/// computing your probe from that same constant tests nothing at all.*
#[test]
fn the_right_columns_geometry_is_the_exes_own_tables() {
    let exe = l2_testkit::executable!();
    let sidebar = l2_testkit::pe::Table::at(&exe, SIDEBAR_TABLE);

    for (i, b) in map::SIDEBAR_BUTTONS.iter().enumerate() {
        assert_eq!(
            b.rect(),
            hotspot(&sidebar, i, SIDEBAR_OFFSET),
            "g_sidebarButtons record {i} ({}) is not where the exe puts it",
            b.name,
        );
    }
    // **Record 5 is End Turn**, and it is the one a button of ours was drawn on
    // top of. The table has six records and the module's array has five,
    // because the sixth dispatches to `Turn_End` rather than to
    // `Sidebar_Button` — so it is a constant of its own and this is what pins
    // it.
    assert_eq!(
        map::END_TURN_BUTTON,
        hotspot(&sidebar, 5, SIDEBAR_OFFSET),
        "the End Turn strip is not record 5",
    );

    let modes = l2_testkit::pe::Table::at(&exe, MINIMAP_MODE_TABLE);
    for (i, r) in map::MINIMAP_MODE_BUTTONS.iter().enumerate() {
        assert_eq!(*r, hotspot(&modes, i, (610, 32)), "g_minimapModeButtons record {i}");
    }
    // The second record's `y1` is `0x42` where the pattern wants `0x3F`, so band
    // 2 is 34 pixels tall and overlaps band 3's first two rows. That is the
    // original's own data and this is where it is pinned rather than described.
    assert_eq!(map::MINIMAP_MODE_BUTTONS[1].h, 34, "the overlapping band is the exe's");

    // And the army-division screen, which is where the third overlap was.
    let split = l2_testkit::pe::Table::at(&exe, SPLIT_TABLE);
    assert_eq!(divide::SPLIT_TICK, widget(&split, 0, (0, 0)), "the confirm tick");
    assert_eq!(divide::SPLIT_CROSS, widget(&split, 1, (0, 0)), "the cancel cross");
    for row in 0..8usize {
        assert_eq!(divide::parent_button(row), widget(&split, 2 + row * 2, (0, 0)), "row {row} parent");
        assert_eq!(
            divide::daughter_button(row),
            widget(&split, 3 + row * 2, (0, 0)),
            "row {row} daughter",
        );
    }
}

/// A world with one county owned, enough to build any of these screens.
fn world() -> (Game, Assets) {
    let mut g = Game::new(7);
    g.player = 1;
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
/// press to be declined or ignored rather than to change the screen.
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

/// **A press on the campaign map's own column reaches the map through the
/// stack**, which is the other end of the same claim: passing is only correct if
/// something underneath answers.
///
/// End Turn under an open county panel is the case that matters, because it is
/// the only way a turn can be ended without closing the panel first — and it is
/// where our own BACK TO MAP button used to sit.
#[test]
fn end_turn_works_through_an_open_county_panel() {
    let (mut g, a) = world();
    let mut m = Machine::new(ScreenId::Campaign);
    {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        m.handle(Event::Click { x: 0, y: 0 }, &mut ctx); // build the map's planes
    }
    m.push(ScreenId::County(1, county::Panel::Tax));
    assert_eq!(m.top_id(), Some(ScreenId::County(1, county::Panel::Tax)));

    {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        m.handle(
            Event::Click {
                x: map::END_TURN_BUTTON.centre_x(),
                y: map::END_TURN_BUTTON.y + map::END_TURN_BUTTON.h / 2,
            },
            &mut ctx,
        );
    }
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the panel did not come down");
    // A turn takes many ticks now — `begin_turn` starts it and `tick_turn`
    // winds it on — so *in flight* is the observable effect of the press, and
    // it is the one that says the press arrived.
    assert!(
        l2_game::turn::turn_in_flight(&g),
        "the press reached the map and started nothing; End Turn was not run",
    );
    for _ in 0..400 {
        if !l2_game::turn::turn_in_flight(&g) {
            break;
        }
        let mut ctx = Ctx { game: &mut g, assets: &a };
        m.update(&mut ctx);
    }
    assert_eq!(g.turns_played, 1, "and the turn did not finish");
}
