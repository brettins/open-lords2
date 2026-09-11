//! **The tool tips, played.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test tooltips
//! ```
//!
//! `FUN_00476E95` (`0x00476E95`) and the six functions under it, which were not
//! built: the Help Options panel's *"Tool tips"* row flipped a flag nothing
//! read. `crate::tooltip` has the decompilation.
//!
//! What is asserted is the subject: **which tip** a pointer position gets on
//! which screen, **the tick** it appears on, **what takes it away**, **where the
//! box is**, and **the words in the box** — never a whole canvas. Every number
//! the oracle gives is typed as a literal rather than read from the constant
//! under test (`docs/agents.md`, *ablating a constant while computing your probe
//! from that same constant tests nothing at all*).
//!
//! Tip screens are off in every world here, because a tip screen is `g_screenId
//! 0x27` and `DAT_004D6FB8[0x27]` is 0 — the one test that is about that turns
//! one on.

use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::county::Panel;
use l2_game::screens::options::{self, Setting};
use l2_game::shell::Pen;
use l2_game::tooltip::{self, Shown, Sidebar};
use l2_game::Game;
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_view::Canvas;

// ---------------------------------------------------------------------- setup

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn ticks(m: &mut Machine, g: &mut Game, a: &Assets, n: usize) {
    for _ in 0..n {
        tick(m, g, a);
    }
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn point(m: &mut Machine, g: &mut Game, a: &Assets, x: i32, y: i32) {
    send(m, g, a, Event::Pointer { x, y });
}

fn world() -> Game {
    let mut g = Game::new(0x7195);
    g.player = 1;
    // **Tip screens: No.** See the module header.
    g.prefs.tip_screens = false;
    g.kingdom.set_county_count(6);
    g.kingdom.counties[1].owner = 1;
    g.selected = 1;
    g
}

fn campaign() -> (Game, Assets, Machine) {
    (world(), Assets::placeholder(), Machine::new(ScreenId::Campaign))
}

/// The End Turn strip, `y 460 …`: `FUN_00477320`'s last arm, id 14.
const END_TURN: (i32, i32) = (560, 470);

fn shown(m: &Machine) -> Option<Shown> {
    m.tooltips().shown()
}

// ------------------------------------------------------------------ the rest

/// **A second of rest, which is sixty-three of our ticks, and not sixty-two.**
///
/// `FUN_00477131`: `999 < (int)(timeGetTime() - stamp)`, the stamp written on
/// the frame the mouse last changed. At 16 ms a tick, 62 ticks is 992 ms and 63
/// is 1,008. The tick that *sees* the move writes the stamp and is not counted.
///
/// Ablations: `REST_MS` 999 → 991 shows on tick 62 and goes red; dropping the
/// `if changed { stamp = now }` arm shows on the first still tick (the stamp is
/// *long ago* when the campaign arrives) and goes red.
#[test]
fn end_turn_names_itself_on_the_sixty_third_still_tick_and_not_the_sixty_second() {
    let (mut g, a, mut m) = campaign();
    point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
    tick(&mut m, &mut g, &a); // the tick that saw the move
    for n in 1..=62 {
        tick(&mut m, &mut g, &a);
        assert_eq!(shown(&m), None, "still tick {n} is too early");
    }
    tick(&mut m, &mut g, &a);
    let s = shown(&m).expect("the sixty-third still tick shows it");
    assert_eq!(s.id, 14, "End Turn's id in FUN_00477320");
    assert_eq!(tooltip::words(&a.shell, s.id), "End your turn");
    // 560 >= 321, so 220 to the left; 470 >= 241, so 30 above.
    assert_eq!((s.x, s.y), (340, 440));
}

/// **Moving resets the rest; moving while a tip is up takes it away — and the
/// frame that takes it away does not restart the second.**
///
/// `FUN_004770B8` hides without touching the stamp, and `FUN_00477131` writes
/// the stamp only on a frame the mouse changed *while no tip is up*. So a
/// one-tick nudge of a tip leaves the stamp where the tip's own resolve put
/// it: the tip comes back 63 ticks after *that*, not 63 after the nudge — and a
/// tip that had been up a whole second comes back on the very next still tick.
///
/// Ablations: stamp the hide in `frame`'s shown branch and the return moves a
/// tick later and goes red; drop `pointer_changed` for a `Pointer` event and
/// the reset goes red.
#[test]
fn moving_resets_the_rest_and_moving_under_a_tip_takes_it_away() {
    let (mut g, a, mut m) = campaign();
    point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
    tick(&mut m, &mut g, &a);
    ticks(&mut m, &mut g, &a, 40);
    // A move forty ticks in: the whole second starts again.
    point(&mut m, &mut g, &a, END_TURN.0 + 1, END_TURN.1);
    tick(&mut m, &mut g, &a);
    ticks(&mut m, &mut g, &a, 62);
    assert_eq!(shown(&m), None, "the rest restarted at the move");
    tick(&mut m, &mut g, &a); // tick S: shown, and stamped
    assert_eq!(shown(&m).map(|s| s.id), Some(14));

    // A move under the tip, on tick S + 1: gone.
    point(&mut m, &mut g, &a, END_TURN.0 + 2, END_TURN.1);
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m), None, "FUN_004770B8 hides it");
    // Still again. The stamp is still S's, so ticks S + 2 … S + 62 are early…
    for n in 2..=62 {
        tick(&mut m, &mut g, &a);
        assert_eq!(shown(&m), None, "tick S + {n}");
    }
    // …and S + 63 is a second after the tip's own resolve, not after the nudge.
    tick(&mut m, &mut g, &a);
    let s = shown(&m).expect("the hide wrote no stamp");
    assert_eq!((s.id, s.x), (14, END_TURN.0 + 2 - 220), "at the new place");

    // Up a whole second, then nudged once: back on the next still tick.
    ticks(&mut m, &mut g, &a, 63);
    point(&mut m, &mut g, &a, END_TURN.0 + 3, END_TURN.1);
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m), None);
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m).map(|s| s.x), Some(END_TURN.0 + 3 - 220), "no rest after a one-tick nudge");

    // A button is a mouse change too.
    send(&mut m, &mut g, &a, Event::RightClick { x: END_TURN.0 + 3, y: END_TURN.1 });
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m), None, "g_mouseInputChanged is set by a button");
}

/// **The option off shows nothing, and the flip back on re-arms the rest.**
///
/// `FUN_00476E95` is `if (g_optToolTips != 0) { … }` round everything, and
/// `Opt_ToggleToolTips` writes `_DAT_004EA830 = 0` — so the tick after the
/// option comes back on shows the tip with no second of rest.
///
/// Ablations: remove the `enabled` guard in `Tooltips::frame` and the first
/// assertion goes red; remove the `tool_tips_seen` rearm in `Machine` and the
/// last one does (the tip then waits until 63 ticks after the stamp).
#[test]
fn the_option_off_shows_nothing_and_turning_it_on_shows_the_tip_at_once() {
    let (mut g, a, mut m) = campaign();
    point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
    tick(&mut m, &mut g, &a); // stamp
    {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        options::toggle(Setting::ToolTips, &mut ctx);
    }
    assert!(!g.prefs.tool_tips);
    ticks(&mut m, &mut g, &a, 200);
    assert_eq!(shown(&m), None, "g_optToolTips is 0");

    // Back on, and the pointer moves: that frame stamps and shows nothing.
    let toggle = |g: &mut Game| {
        let mut ctx = Ctx { game: g, assets: &a };
        options::toggle(Setting::ToolTips, &mut ctx);
    };
    toggle(&mut g);
    assert!(g.prefs.tool_tips);
    point(&mut m, &mut g, &a, END_TURN.0 + 1, END_TURN.1);
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m), None, "a changed frame stamps");

    // Off and on again, two ticks after that stamp: nowhere near a second. The
    // tip shows anyway, because the flip zeroed the stamp.
    //
    // **This test was green with the rearm deleted** when it flipped back on
    // after the 200 ticks above: by then the stamp was old whatever the flip
    // did. Ablation is what found that.
    toggle(&mut g);
    tick(&mut m, &mut g, &a);
    toggle(&mut g);
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m).map(|s| s.id), Some(14), "Opt_ToggleToolTips zeroed the stamp");
}

/// **And off means nothing is drawn**, even with a tip that was already up.
///
/// Ablation: remove the `prefs.tool_tips` test in `tooltip::draw` and the box
/// region comes out identical to the one drawn with the option on.
#[test]
fn the_option_off_draws_no_box() {
    let (mut g, a, mut m) = campaign();
    point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
    ticks(&mut m, &mut g, &a, 64);
    let s = shown(&m).expect("a tip is up");
    let rect = {
        let ctx = Ctx { game: &mut g, assets: &a };
        tooltip::layout(&ctx, s).0
    };
    let on = region(&draw(&mut m, &mut g, &a), rect);
    g.prefs.tool_tips = false;
    let off = region(&draw(&mut m, &mut g, &a), rect);
    assert_ne!(on, off, "the box is drawn with the option on and not with it off");
    assert!(off.iter().any(|&p| p != tooltip::FILL), "no fill under the option off");
}

// ------------------------------------------------------------------ the box

fn draw(m: &mut Machine, g: &mut Game, a: &Assets) -> Canvas {
    let mut canvas = Canvas::screen();
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, &mut canvas);
    canvas
}

fn region(c: &Canvas, r: Rect) -> Vec<u8> {
    let mut out = Vec::new();
    for y in r.y..r.y + r.h {
        for x in r.x..r.x + r.w {
            out.push(c.at(x as usize, y as usize));
        }
    }
    out
}

/// **The words, in their own box** — compared region for region with a box
/// the test draws itself from the literal string, not with a canvas.
///
/// With the placeholder font a glyph advances 6 pixels, so *"End your turn"*
/// ends 78 pixels on plus `Ui_DrawText`'s trailing 4: `g_penAdvance` 82, and
/// `0xC - (0xB0 - 82) / 16` is 7 units — a 112 × 22 box at (340, 440), one line
/// tall.
///
/// Ablations: delete the second pass's text and the region loses its words;
/// delete the fill and the first pass shows through on the map; swap `FILL`
/// for `INK` — each goes red.
#[test]
fn the_tip_is_its_string_in_a_filled_outlined_box() {
    let (mut g, a, mut m) = campaign();
    point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
    ticks(&mut m, &mut g, &a, 64);
    let s = shown(&m).expect("a tip is up");
    let (rect, lines) = {
        let ctx = Ctx { game: &mut g, assets: &a };
        tooltip::layout(&ctx, s)
    };
    assert_eq!(rect, Rect::new(340, 440, 112, 22));
    assert_eq!(lines, vec!["End your turn".to_string()]);

    let got = region(&draw(&mut m, &mut g, &a), rect);

    let mut want = Canvas::screen();
    want.fill_rect(340, 440, 112, 22, 0x20);
    let pen = Pen { assets: &a.shell, ink: &a.ink, chrome: a.chrome.as_ref(), shadow: None, caps: None };
    pen.body(&mut want, 344, 444, "End your turn", 0x3F);
    want.fill_rect(340, 440, 112, 1, 0x3F);
    want.fill_rect(340, 461, 112, 1, 0x3F);
    want.fill_rect(340, 440, 1, 22, 0x3F);
    want.fill_rect(451, 440, 1, 22, 0x3F);
    assert_eq!(got, region(&want, rect), "the box is not End Turn's words in a 0x20 box outlined 0x3F");
}

/// **The pure geometry, from literals** — `FUN_00477131`'s two offsets,
/// `FUN_00477249`'s clamp, and `FUN_00476E95`'s size rule.
///
/// Ablations: remove the `y > 0x1B7` clamp and the bottom-edge row goes red;
/// change `0x141` to `0x140` and the middle row does.
#[test]
fn the_box_is_placed_and_sized_the_way_the_original_places_it() {
    for (pointer, want) in [
        ((100, 100), (130, 130)), // right of and below the pointer
        ((320, 240), (350, 270)), // the last column and row that are
        ((321, 241), (101, 211)), // the first that flip
        ((620, 478), (400, 440)), // bottom right: 448 clamped to 440
        ((639, 479), (419, 440)),
        ((0, 0), (30, 30)),
    ] {
        assert_eq!(tooltip::place(pointer.0, pointer.1), want, "pointer {pointer:?}");
    }
    // One line is 22 tall and two are 40; a line of 176 is 12 units, and C
    // division rounds (176 - 178) / 16 to zero, not to minus one.
    assert_eq!(tooltip::box_size(1, 82), (112, 22));
    assert_eq!(tooltip::box_size(2, 178), (192, 40));
    assert_eq!(tooltip::box_size(3, 176), (192, 40));
    // (176 - 20) / 16 is 9, so 3 units.
    assert_eq!(tooltip::box_size(1, 20), (48, 22));
}

/// A battle between two armies, on the field and paused, over the campaign.
fn battlefield() -> (Game, Assets, Machine) {
    let mut g = world();
    g.kingdom.counties[2].owner = 2;
    for realm in 1..=2usize {
        g.kingdom.realms[realm].in_play = true;
    }
    let spawn = |g: &mut Game, owner: u8, x: u8| {
        let mut u = Unit::new(UnitKind::Army, owner, x, 20);
        u.men = 100;
        u.troops[TroopType::Peasant.index()] = 100;
        u.county = owner;
        u.home_county = owner;
        u.owner_is_human = owner == 1;
        g.kingdom.campaign.units.spawn(u).expect("a free slot")
    };
    let attacker = spawn(&mut g, 1, 30);
    let defender = spawn(&mut g, 2, 31);
    let runner = l2_game::engagement::begin_fight(&mut g.kingdom, attacker, defender, None, 7)
        .expect("two armies");
    g.battle =
        Some(Box::new(l2_game::battlefield::LiveBattle::new(runner, attacker, defender, 2, None, 1, 1)));
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Battlefield);
    (g, Assets::placeholder(), m)
}

/// **A two-line tip in the bottom-right corner stays on the screen** — the
/// battlefield's Autocalc button, `FUN_004777AA`'s id 30.
///
/// In the placeholder font *"Autocalculate battle or siege results"* breaks
/// after *"siege"* at both wraps — 142 + 39 is 181 — so the box is 40 tall;
/// `FUN_00477249` pulls it up from 448 to 440, and it ends on the screen's last
/// row. Every pixel the box writes must be inside the canvas: *"is it drawn"*
/// and *"can it be seen"* are different claims (`docs/agents.md`).
///
/// Ablation: remove the `y > 0x1B7` clamp and the box ends at 488, off the
/// screen; the containment assertion goes red.
#[test]
fn a_two_line_tip_at_the_bottom_right_corner_stays_on_the_screen() {
    let (mut g, a, mut m) = battlefield();
    tick(&mut m, &mut g, &a);
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "the field is up");
    point(&mut m, &mut g, &a, 620, 478);
    ticks(&mut m, &mut g, &a, 64);
    let s = shown(&m).expect("the battlefield's ladder answers");
    assert_eq!(s.id, 30);
    assert_eq!(tooltip::words(&a.shell, 30), "Autocalculate battle or siege results");
    let (rect, lines) = {
        let ctx = Ctx { game: &mut g, assets: &a };
        tooltip::layout(&ctx, s)
    };
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(rect, Rect::new(400, 440, 192, 40));
    assert!(
        rect.x >= 0 && rect.y >= 0 && rect.x + rect.w <= 640 && rect.y + rect.h <= 480,
        "the box {rect:?} leaves the 640 x 480 screen"
    );
    // And the outline's bottom row really is on it.
    let c = draw(&mut m, &mut g, &a);
    assert_eq!(c.at(400, 479), tooltip::INK, "the box's last row is drawn");
}

// ------------------------------------------------------------ which screens

/// **`DAT_004D6FB8`: the sidebar's tips follow it onto the screens that sit on
/// it, and nowhere else.**
///
/// A county panel (`0x15`) is a campaign-ladder screen; the other lords
/// (`0x0B`) and a tip screen (`0x27`) are not.
///
/// Ablation: make `ladder_of` answer the campaign ladder for every byte and the
/// two `None` rows go red.
#[test]
fn a_county_panel_keeps_the_sidebar_tips_and_the_other_lords_and_a_tip_screen_do_not() {
    for (over, want) in [
        (Some(ScreenId::County(1, Panel::Tax)), Some(14)),
        (Some(ScreenId::Diplomacy), None),
        (None, None), // a tip screen, below
    ] {
        let (mut g, a, mut m) = campaign();
        match over {
            Some(id) => m.push(id),
            None => {
                g.prefs.tip_screens = true;
                assert!(l2_game::tip::show(&mut g, 200), "a tip is posted");
            }
        }
        point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
        ticks(&mut m, &mut g, &a, 70);
        if over.is_none() {
            assert!(m.ids().contains(&ScreenId::Tip), "screen 0x27 is up");
        }
        assert_eq!(shown(&m).map(|s| s.id), want, "over {over:?}");
    }
}

/// **A repaint takes the tip away and the ordinary rule brings it back** —
/// `Screen_Draw`'s `FUN_0047703A`, which keeps the stamp.
///
/// The county panel is a campaign-ladder screen, so the only thing that can
/// take End Turn's tip away when it opens is the repaint. Ten ticks after the
/// tip went up its stamp is ten ticks old, so it stays away until the second is
/// up — sixty-three ticks after the stamp — and a repaint a second later
/// brings it straight back.
///
/// Ablations: remove the `drop_tip` call in `Machine::run_tooltips` and the
/// first `None` goes red; stamp the drop in `Tooltips::drop_tip` and the last
/// assertion does.
#[test]
fn opening_a_screen_drops_the_tip_and_its_own_stamp_brings_it_back() {
    let (mut g, a, mut m) = campaign();
    point(&mut m, &mut g, &a, END_TURN.0, END_TURN.1);
    tick(&mut m, &mut g, &a); // the stamp, at tick 1
    ticks(&mut m, &mut g, &a, 63); // the tip, and the stamp again, at tick 64
    assert_eq!(shown(&m).map(|s| s.id), Some(14));
    ticks(&mut m, &mut g, &a, 10);
    m.push(ScreenId::County(1, Panel::Tax));
    tick(&mut m, &mut g, &a); // tick 75
    assert_eq!(shown(&m), None, "the panel's repaint dropped it");
    ticks(&mut m, &mut g, &a, 51); // tick 126: 62 after the stamp
    assert_eq!(shown(&m), None, "the stamp is not yet a second old");
    tick(&mut m, &mut g, &a); // tick 127: 63 after it
    assert_eq!(shown(&m).map(|s| s.id), Some(14), "back, over the panel");

    ticks(&mut m, &mut g, &a, 100);
    m.push(ScreenId::Diplomacy);
    tick(&mut m, &mut g, &a);
    assert_eq!(shown(&m), None, "0x0B has no tips at all");
}

// ------------------------------------------------------------ the ladders

/// **`FUN_00477320`, position by position**, against literals out of the
/// decompilation.
///
/// Ablations: swap the `0x223` and `0x23B` splits and the heart rows go red;
/// read the farm list's `1` as wheat and the cattle row does.
#[test]
fn the_campaign_ladder_names_every_part_of_the_sidebar() {
    let mine = Sidebar { minimap_mode: 0, owned: true, farm: vec![1, 0, 2], industry: vec![6, 4, 5, 7, 3] };
    for ((x, y), want) in [
        ((477, 300), 0),  // left of the sidebar
        ((560, 23), 0),   // the menu bar
        ((500, 50), 1),   // the minimap itself
        ((620, 50), 2),   // mode buttons, owner mode: labour
        ((620, 80), 3),   // food
        ((620, 110), 4),  // happiness
        ((620, 140), 5),  // the overview button
        ((500, 200), 6),  // population
        ((560, 200), 34), // the heart between
        ((600, 200), 7),  // happiness report
        ((500, 230), 8),  // tax
        ((600, 230), 9),  // rations
        ((500, 280), 10), // the labour slider
        ((500, 304), 15), // farm row 0: cattle (pitch 45, three rows)
        ((500, 349), 16), // row 1: wheat
        ((500, 394), 17), // row 2: reclaiming
        ((600, 304), 18), // industry row 0: wood (pitch 30, five rows)
        ((600, 334), 20), // row 1 is slot 4: iron
        ((600, 364), 19), // row 2 is slot 5: stone
        ((600, 394), 21), // weapons
        ((600, 424), 22), // the castle
        ((500, 440), 11), // army
        ((520, 440), 12), // treasury
        ((560, 440), 13), // supplies
        ((600, 440), 32), // castle
        ((620, 440), 33), // diplomacy
        ((560, 470), 14), // End Turn
    ] {
        assert_eq!(tooltip::campaign_tip(&mine, x, y), want, "({x}, {y})");
    }
    // An overlay up: the three statistic buttons name the mode that is on, and
    // the fourth returns to the owners' map.
    let food = Sidebar { minimap_mode: 2, ..mine.clone() };
    assert_eq!(tooltip::campaign_tip(&food, 620, 50), 3);
    assert_eq!(tooltip::campaign_tip(&food, 620, 120), 3);
    assert_eq!(tooltip::campaign_tip(&food, 620, 140), 31);
    // Somebody else's county: the strip, the slider and the rows say nothing.
    let theirs = Sidebar { owned: false, ..mine.clone() };
    for (x, y) in [(500, 200), (500, 280), (500, 304), (600, 424)] {
        assert_eq!(tooltip::campaign_tip(&theirs, x, y), 0, "({x}, {y})");
    }
    // A row past the list is nothing, and so is a list value with no tip.
    let short = Sidebar { farm: vec![1], ..mine };
    assert_eq!(tooltip::campaign_tip(&short, 500, 380), 0);
}

/// **`FUN_004777AA`** — the battlefield's eight.
#[test]
fn the_battle_ladder_names_the_overview_the_troops_and_the_five_buttons() {
    for ((x, y), want) in [
        ((477, 100), 0),
        ((500, 23), 0),
        ((500, 100), 23),
        ((500, 300), 24),
        ((500, 430), 25),
        ((500, 460), 26),
        ((520, 460), 27),
        ((560, 460), 28),
        ((600, 460), 29),
        ((630, 460), 30),
    ] {
        assert_eq!(tooltip::battle_tip(x, y), want, "({x}, {y})");
    }
}

// ------------------------------------------------------------ the install

/// **`DAT_004D6FB8`, byte for byte, out of the player's own `Lords2.exe`.**
///
/// The only check of [`tooltip::SCREENS`] that cannot be typed into agreement:
/// every other test here reads the constant.
#[test]
fn the_screen_table_is_the_one_in_the_players_executable() {
    let Some(exe) = l2_testkit::executable() else {
        l2_testkit::skip!("no Lords2.exe");
    };
    let table = l2_testkit::pe::Table::at(&exe, 0x004D_6FB8);
    let read: Vec<u8> = (0..tooltip::SCREENS.len()).map(|i| table.u8_at(i)).collect();
    assert_eq!(read, tooltip::SCREENS.to_vec());
}

/// **Group 220 is our transcription, string for string**, and in the player's
/// own body font the corner tip really is two lines — so the clamp above is a
/// case a player meets and not one only the placeholder font makes.
#[test]
fn group_220_is_the_players_own_words_and_autocalc_wraps_in_the_real_font() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no L2.eng");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    for (i, want) in tooltip::TEXT.iter().enumerate() {
        assert_eq!(assets.shell.text(tooltip::GROUP, i), *want, "group 220 index {i}");
    }
    let mut g = world();
    let s = Shown { id: 30, x: 400, y: 440 };
    let ctx = Ctx { game: &mut g, assets: &assets };
    let (rect, lines) = tooltip::layout(&ctx, s);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(rect.h, 40);
}
