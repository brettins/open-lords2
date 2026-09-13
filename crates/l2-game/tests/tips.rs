//! **The tip screens, played.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-game --test tips
//! ```
//!
//! `Tip_Update` (`0x00476AA7`), `Tip_Show` (`0x00476DA9`) and `FUN_00476E21`
//! (`0x00476E21`), none of which existed here: the advice a new player gets,
//! its words, and the forty narration files that read it out. `crate::tip` has
//! the decompilation.
//!
//! Everything asserted is the subject and nothing is a canvas: **which tip
//! fires for which screen**, the **frame count** of the delay, **where the OK
//! button is** for a given wrap, **the words**, and **the tick each clip starts
//! on**. Every number the oracle gives is typed as a literal rather than read
//! from the constant under test, so ablating the constant cannot move the
//! probe with it — `docs/agents.md`.

use std::collections::BTreeMap;

use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, Frame};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::options::{self, Page, Setting};
use l2_game::tip::{self, Tips, View};
use l2_game::Game;
use l2_kingdom::units_tick::Incursion;

// ---------------------------------------------------------------------- setup

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn world() -> Game {
    let mut g = Game::new(0x7195);
    g.player = 1;
    g.kingdom.set_county_count(6);
    g
}

/// A running game on the campaign map, with nothing posted.
fn campaign() -> (Game, Assets, Machine) {
    (world(), Assets::placeholder(), Machine::new(ScreenId::Campaign))
}

/// Tick until `Tip_Show` posts, and say on which tick it did.
fn ticks_until_posted(m: &mut Machine, g: &mut Game, a: &Assets, limit: usize) -> Option<usize> {
    let before = g.tips.shows();
    for n in 1..=limit {
        tick(m, g, a);
        if g.tips.shows() != before {
            return Some(n);
        }
    }
    None
}

fn open_group(g: &Game) -> Option<u16> {
    g.messages.open().map(|r| r.group)
}

/// `Msg_HandleInput`'s right-button branch: `Msg_Dismiss`, whatever it is.
fn right_click() -> Event {
    Event::RightClick { x: 5, y: 5 }
}

fn view(screen: u8) -> View {
    View { enabled: true, in_play: true, screen: Some(screen), ..View::default() }
}

/// A `Tips` whose start-up twenty frames have already run out.
fn armed() -> Tips {
    let mut t = Tips::new();
    let elsewhere = View { enabled: true, in_play: true, screen: None, ..View::default() };
    for _ in 0..20 {
        assert_eq!(tip::update(&mut t, &elsewhere), None);
    }
    assert_eq!(t.delay(), 0);
    t
}

// ------------------------------------------------------------------ the ladder

/// **Which tip fires for which screen** — `Tip_Update`'s six-arm `else if`
/// chain and the campaign map's block, one screen byte at a time.
///
/// Ablation: swap any two groups in `tip::update`'s chain, or its `job == 8`,
/// and the row naming it goes red.
#[test]
fn each_screen_the_ladder_names_gets_its_own_tip() {
    for (screen, job, want) in [
        (0x17, 0, 209), // raise army: "The Armoury:"
        (0x0F, 8, 208), // the job popup on job 8: "The Blacksmith:"
        (0x02, 0, 207), // the village: "The Town Center:"
        (0x39, 0, 218), // advanced options: "Advanced Game Options:"
        (0x1B, 0, 217), // castle building: "Castle Building:"
        (0x10, 0, 210), // move-order mode: "Army Movement:"
    ] {
        let mut t = armed();
        assert_eq!(
            tip::update(&mut t, &View { job, ..view(screen) }),
            Some(want),
            "screen {screen:#04x}"
        );
    }
    let mut t = armed();
    assert_eq!(tip::update(&mut t, &View { job: 7, ..view(0x0F) }), None, "job 7 is not the smithy");
    for screen in [0x04, 0x0A, 0x14, 0x27, 0x31, 0x42] {
        let mut t = armed();
        assert_eq!(tip::update(&mut t, &view(screen)), None, "screen {screen:#04x} has no tip");
    }

    // The campaign map: the kingdom overview first, and only when zoomed out.
    let mut t = armed();
    assert_eq!(tip::update(&mut t, &View { zoom_far: true, ..view(0x00) }), Some(206));
    let mut t = armed();
    assert_eq!(tip::update(&mut t, &view(0x00)), Some(200));
    // And the switch and the phase gate the whole ladder.
    let mut t = armed();
    assert_eq!(tip::update(&mut t, &View { enabled: false, ..view(0x02) }), None);
    assert_eq!(tip::update(&mut t, &View { in_play: false, ..view(0x02) }), None);
}

/// **Our stack, read as the byte the ladder tests.** Pushed, not driven, because
/// the question is the projection and not the screens.
///
/// Ablation: delete `ScreenId::RaiseArmy(_) => Some(0x17)` in
/// `tip::screen_byte` and the raise-army row goes red.
#[test]
fn our_screens_read_as_the_originals_screen_bytes() {
    let g = world();
    for (id, byte) in [
        (ScreenId::Village(1), 0x02),
        (ScreenId::Job(1, 7), 0x0F),
        (ScreenId::RaiseArmy(1), 0x17),
        (ScreenId::Castle(1), 0x1B),
        (ScreenId::Options(Page::Advanced), 0x39),
    ] {
        let mut m = Machine::new(ScreenId::Campaign);
        m.push(id);
        let v = View::of(&m, &g);
        assert_eq!(v.screen, Some(byte), "{id:?}");
        assert!(v.in_play, "the campaign is on the stack");
    }
    // `g_jobPanelJob` is our slot plus one, so slot 7 is the blacksmith's 8.
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Job(1, 7));
    assert_eq!(View::of(&m, &g).job, 8);
    // The message scroll is not a screen id: a letter open on the campaign map
    // leaves `g_screenId` at 0.
    let mut m = Machine::new(ScreenId::Campaign);
    m.push(ScreenId::Message);
    assert_eq!(View::of(&m, &g).screen, Some(0x00));
    // And the front end is not `g_appPhase == 3`.
    let m = Machine::new(ScreenId::Setup(l2_game::screens::setup::SetupPage::Title));
    assert!(!View::of(&m, &g).in_play);
}

/// **Three tips on the campaign map, twenty frames apart.** Through the real
/// machine: the ladder, `Tip_Show`'s screen `0x27`, the pump pulling on it, a
/// right click, and `FUN_00476E21`'s re-arm.
///
/// The 21 is typed. Ablation: `DELAY` to `0x13` and every `Some(21)` is 20.
#[test]
fn a_new_game_meets_three_tips_twenty_frames_apart() {
    let (mut g, a, mut m) = campaign();
    assert_eq!(
        ticks_until_posted(&mut m, &mut g, &a, 100),
        Some(21),
        "FUN_00476A5D arms twenty frames at start-up, so the first tip is on the twenty-first"
    );
    assert!(m.ids().contains(&ScreenId::Tip), "Tip_Show writes g_screenId = 0x27: {:?}", m.ids());
    assert_eq!(m.top_id(), Some(ScreenId::Message), "Msg_Pump runs on 0x27 in the same frame");
    let r = *g.messages.open().expect("the tip window is open");
    assert_eq!(
        (r.group, r.category, r.to, r.from, r.variant),
        (200, 7, 1, 0, 0),
        "Msg_Enqueue(0, g_localPlayer, 200, 0, g_tipCategory[200], …)"
    );

    for want in [201u16, 202] {
        send(&mut m, &mut g, &a, right_click());
        assert!(!g.tips.hosting(), "Msg_Dismiss reaches FUN_00476E21");
        assert_eq!(
            ticks_until_posted(&mut m, &mut g, &a, 100),
            Some(21),
            "twenty quiet frames after the dismissal, then {want}"
        );
        assert_eq!(open_group(&g), Some(want));
    }

    send(&mut m, &mut g, &a, right_click());
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 200), None, "the campaign map has three");
    assert_eq!(m.ids(), vec![ScreenId::Campaign], "screen 0x27 went with the last of them");
}

/// **A tip takes its screen away and gives it back.** On `0x27` the options
/// page's arms do not run, so a click on its first row does nothing; the OK
/// button closes the window and the page is back.
///
/// Ablation: make `TipScreen::handle` return `Pass` and Advanced Farming flips
/// under the tip. Make `MessageScreen::handle` use `message::frame_of` again
/// and the left button cannot close a tip at all.
#[test]
fn a_tip_holds_its_screens_input_and_gives_it_back_when_dismissed() {
    let (mut g, a, mut m) = campaign();
    m.push(ScreenId::Options(Page::Advanced));
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 100), Some(21));
    assert_eq!(open_group(&g), Some(218), "\"Advanced Game Options:\"");
    assert_eq!(
        m.ids(),
        vec![
            ScreenId::Campaign,
            ScreenId::Options(Page::Advanced),
            ScreenId::Tip,
            ScreenId::Message
        ]
    );

    // `Opt_ToggleAdvancedFarming`'s widget at (280, 156).
    let farming = g.kingdom.options.advanced_farming;
    send(&mut m, &mut g, &a, Event::Click { x: 280 + 8, y: 156 + 8 });
    assert_eq!(g.kingdom.options.advanced_farming, farming, "g_screenId is 0x27, not 0x39");
    assert!(g.messages.is_open());

    let r = *g.messages.open().unwrap();
    let ok = {
        let ctx = Ctx { game: &mut g, assets: &a };
        l2_game::screens::message::window_frame(&ctx, &r).expect("a tip window has a frame").ok_button()
    };
    send(&mut m, &mut g, &a, Event::Click { x: ok.0 + 4, y: ok.1 + 4 });
    assert!(!g.messages.is_open(), "the left button closes a tip window");
    tick(&mut m, &mut g, &a);
    assert_eq!(m.ids(), vec![ScreenId::Campaign, ScreenId::Options(Page::Advanced)]);
}

/// **The minimap is live under a tip, and it drops the byte with no restore.**
///
/// Screen `0x27` has no arm in `Screen_FrameInput` (`0x0042FF10`) and none in
/// `Screen_HandleInput` (`0x004BA9C8`) — both were read, and the second really
/// is a bare `return 0`. But every arm of the first ends `goto LAB_00431F25`,
/// which jumps *over* the epilogue, and `0x27` has no arm to jump: it falls
/// into
///
/// ```text
/// if ((leftPressed || rightPressed) && g_screenId != 0x12 && FUN_004323FE()) {
///     if (g_battlePhase == 0) g_screenId = 0;
/// }
/// ```
///
/// That is a bare assignment, not `Msg_Dismiss`, so `FUN_00476E21` does not run:
/// the screen the tip was shown over is **not** put back and `DAT_004F0358` is
/// **not** re-armed. Both halves are asserted, because the second is the whole
/// difference between this and the dismissal above.
///
/// **Ablation, run:** make `TipScreen::handle` return `Transition::Stay` for
/// everything again and the first assertion goes red — the options page comes
/// back instead of the map, and the tip host is still seated.
#[test]
fn a_minimap_press_under_a_tip_drops_the_screen_without_re_arming_the_delay() {
    let (mut g, a, mut m) = campaign();
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 100), Some(21));
    assert!(g.tips.hosting(), "the tip is hosting 0x27");

    // The tip window is on top and takes the click first; the minimap raster is
    // outside it, so this reaches the tip host underneath.
    let mini = l2_view::chrome::minimap_hit_area();
    send(&mut m, &mut g, &a, Event::Click { x: mini.x0 + 4, y: mini.y0 + 4 });

    assert!(!g.tips.hosting(), "g_screenId = 0, so the byte is not 0x27 any more");
    assert_eq!(
        g.tips.delay(),
        0,
        "the epilogue is an assignment, not FUN_00476E21: DAT_004F0358 is not re-armed",
    );
    assert!(g.messages.is_open(), "and the window stays: Msg_Pump runs on 0x00 too");

    // The seat comes off on the next tick, and with no re-arm the ladder posts
    // the next tip on that same tick — where a dismissal would have bought
    // twenty quiet frames. Asserted as the difference, because that is what the
    // missing re-arm *is*.
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 100), Some(1), "the next tip, immediately");
}

// ----------------------------------------------------------------- the window

/// **Where the OK button is**, for the wraps that exercise every rule in the
/// layout. The numbers are worked by hand from `Msg_DrawWindow`'s arm and typed.
///
/// Ablation: delete `if measured < 0x61 { y += 0x40 }` and the first two rows
/// go red; drop `+ y` from the height and all four do.
#[test]
fn the_ok_button_sits_where_the_wrapped_text_puts_it() {
    // One line: measured 0x40, below 0x61, so the window drops 64 pixels AND
    // grows by them, because its height includes its own top.
    let one = message::paragraph_layout(&[1]);
    assert_eq!(one.frame, Frame { x: 0x10, y: 0x80, w: 0x1C0, h: 0xC0 });
    assert_eq!(one.frame.ok_button(), (416, 272));
    assert_eq!(one.heading, (0x20, 0x94));
    assert_eq!(one.tops, vec![0xC0]);

    // Two paragraphs of one line: 0x60, still below 0x61.
    assert_eq!(message::paragraph_layout(&[1, 1]).frame.ok_button(), (416, 304));
    // One more line and it is 0x70 — not dropped, so MORE text puts the OK
    // button HIGHER.
    assert_eq!(message::paragraph_layout(&[1, 2]).frame.ok_button(), (416, 192));

    // A five-paragraph tip, "Castle Building:"'s shape: 0x20 + 0x30 + 0x40 +
    // 3 × 0x20 = 0xF0 measured, so the box is 0xF0 + 0x40 tall.
    let five = message::paragraph_layout(&[2, 3, 1, 1, 1]);
    assert_eq!(five.frame, Frame { x: 0x10, y: 0x40, w: 0x1C0, h: 0x130 });
    assert_eq!(five.frame.ok_button(), (416, 320));
    assert_eq!(five.tops, vec![0x80, 0xB0, 0xF0, 0x110, 0x130]);
}

/// **`FUN_0040328E`'s line breaks**, with every glyph ten pixels wide so the
/// arithmetic is visible: a space is four pixels whatever the font, it belongs
/// to the word after it, and the fit is strict.
#[test]
fn a_line_breaks_where_the_original_breaks_it() {
    let ten = |_| 10;
    // "aaaa" 40, " bbbb" 44 -> 84, " cccc" 44 -> 128: over 100.
    assert_eq!(message::break_lines("aaaa bbbb cccc", 100, ten), vec!["aaaa bbbb", "cccc"]);
    // Exactly the width is a break, not a fit.
    assert_eq!(message::break_lines("aaaa bbbb", 84, ten), vec!["aaaa", "bbbb"]);
    assert_eq!(message::break_lines("aaaa bbbb", 85, ten), vec!["aaaa bbbb"]);
    // The leading space of a wrapped word is measured and not drawn.
    assert_eq!(message::break_lines("aaaa  bbbb", 50, ten), vec!["aaaa", "bbbb"]);
    // Always a line, because the draw is inside the loop.
    assert_eq!(message::break_lines("", 100, ten), vec![""]);
}

/// **The words fall back to our transcription**, and only where the file is
/// silent — here, a machine with no `L2.eng` at all.
#[test]
fn without_the_players_file_the_tip_speaks_our_transcription() {
    let bare = Assets::placeholder();
    assert_eq!(tip::words(&bare.shell, 207, 0), "The Town Center:");
    assert_eq!(tip::words(&bare.shell, 211, 1), tip::transcribed(211, 1));
    assert!(tip::words(&bare.shell, 211, 1).starts_with("To try to conquer a county"));
}

// ------------------------------------------------------------ the two gates

/// **The switch silences the tips, and every flip re-arms them.**
/// `Opt_ToggleTipScreens` is `g_optTipScreens = !g_optTipScreens;
/// FUN_00476A5D();` — the reset runs on the flip off as well as on.
///
/// Ablation: delete `ctx.game.tips.reset()` in `options::toggle` and the last
/// two assertions go red.
#[test]
fn the_tip_screens_switch_silences_them_and_every_flip_rearms_them() {
    let (mut g, a, mut m) = campaign();
    g.prefs.tip_screens = false;
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 300), None, "Tip screens: No");
    assert_eq!(g.tips.delay(), 20, "and Tip_Update does not even count down");

    let flip = |g: &mut Game| {
        let mut ctx = Ctx { game: g, assets: &a };
        options::toggle(Setting::TipScreens, &mut ctx);
    };
    flip(&mut g);
    assert!(g.prefs.tip_screens);
    assert_eq!(ticks_until_posted(&mut m, &mut g, &a, 100), Some(21));
    assert!(g.tips.shown(200));

    send(&mut m, &mut g, &a, right_click());
    for _ in 0..5 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(g.tips.delay(), 15);
    flip(&mut g);
    assert!(!g.prefs.tip_screens);
    assert!(!g.tips.shown(200), "the flip OFF clears g_tipShown too");
    assert_eq!(g.tips.delay(), 20);
}

/// **The invasion flag**: set for the local player's army only, answered by the
/// ladder's last arm on a screen no other arm names — and cleared before
/// `Tip_Show` is asked, so on screen `0x27` the tip is lost.
/// `docs/bugs.md` B101.
#[test]
fn the_local_players_incursion_raises_the_invasion_tip_and_screen_0x27_swallows_it() {
    let mut t = armed();
    t.note_incursions(&[Incursion { unit: 3, owner: 2, county: 4 }], 1);
    assert!(!t.invaded(), "another realm's army is not g_localPlayer's");
    t.note_incursions(&[Incursion { unit: 3, owner: 1, county: 4 }], 1);
    assert!(t.invaded());
    assert_eq!(tip::update(&mut t, &view(0x04)), Some(211), "\"Invasions:\"");
    assert!(!t.invaded(), "consumed by the arm");

    let mut g = world();
    g.tips = armed();
    assert!(tip::show(&mut g, 200), "a tip is up: g_screenId is 0x27");
    g.tips.note_incursions(&[Incursion { unit: 1, owner: 1, county: 2 }], 1);
    assert_eq!(tip::tick(&mut g, &view(0x27)), None, "Tip_Show refuses on 0x27");
    assert!(!g.tips.invaded(), "and the flag was cleared before it asked");
    assert!(!g.tips.shown(211), "so the tip waits for the next crossing");
}

/// **The flag reaches the tips from a real march.** The simulation reports the
/// crossing; `turn::tick_units_only` is the door the frame loop uses.
///
/// Ablation: delete `game.tips.note_incursions` in `tick_units_only`.
#[test]
fn an_army_marching_out_of_its_own_county_sets_the_flag() {
    use l2_kingdom::map::{flags, CampaignMap, MAP_DIM, MAP_TILES};
    let mut g = world();
    let mut map = CampaignMap::empty();
    for i in 0..MAP_TILES {
        map.county[i] = if i % MAP_DIM < 32 { 1 } else { 2 };
    }
    for x in 0..64u8 {
        map.set_flags(x, 10, flags::ROAD);
    }
    g.kingdom.campaign.map = map;
    g.kingdom.counties[1].owner = 1;
    g.kingdom.realms[1].in_play = true;
    g.kingdom.realms[1].is_human = true;

    let mut u = l2_kingdom::unit::Unit::new(l2_kingdom::UnitKind::Army, 1, 30, 10);
    u.men = 100;
    u.troops[0] = 100;
    u.county = 1;
    u.owner_is_human = true;
    let id = g.kingdom.campaign.units.spawn(u).expect("a free slot");
    l2_kingdom::movement::order_move(
        &g.kingdom.campaign.map,
        &mut g.kingdom.campaign.units,
        id,
        (34, 10),
        l2_kingdom::movement::Routing::Direct,
    )
    .expect("a road the whole way");

    for _ in 0..200 {
        if !g.kingdom.units_moving(l2_kingdom::UnitKind::Army) {
            break;
        }
        l2_game::turn::tick_units_only(&mut g);
    }
    assert_eq!(g.kingdom.campaign.units.get(id).map(|u| u.county), Some(2), "it crossed");
    assert!(g.tips.invaded(), "DAT_00553210: the local player's army entered a county it does not own");
}

// ------------------------------------------------------ with the player's game

/// **The words are the player's own `L2.eng`**, and our transcription agrees
/// with it string for string — which is what makes it a fallback rather than a
/// rewrite. `CLAUDE.md` rule 6.
#[test]
fn every_tip_draws_the_players_own_words_and_our_transcription_is_them() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no L2.eng");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let mut strings = 0;
    for (group, words) in tip::TEXT {
        for (i, want) in words.iter().enumerate() {
            assert_eq!(assets.shell.text(*group as usize, i), *want, "L2.eng {group}/{i}");
            assert_eq!(tip::words(&assets.shell, *group, i), *want);
            strings += 1;
        }
    }
    assert_eq!(strings, 53, "fourteen groups: fourteen headings and thirty-nine paragraphs");
}

/// **The OK corner of a real tip, in the real font.** *"Kingdom overview:"* is
/// one short line, so it takes the one-line layout: dropped 64 pixels, OK at
/// (416, 272).
#[test]
fn the_kingdom_overview_tip_puts_its_ok_button_where_one_line_puts_it() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no body font to measure with");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let mut g = world();
    let mut m = Machine::new(ScreenId::Campaign);
    g.tips = armed();
    assert!(tip::show(&mut g, 206));
    tick(&mut m, &mut g, &assets);
    let r = *g.messages.open().expect("pulled on 0x27");
    let ctx = Ctx { game: &mut g, assets: &assets };
    let frame = l2_game::screens::message::window_frame(&ctx, &r).expect("a tip window");
    assert_eq!(frame, Frame { x: 0x10, y: 0x80, w: 0x1C0, h: 0xC0 });
    assert_eq!(frame.ok_button(), (416, 272));
}

/// **The narrator reads the tip: its first line ten ticks in, then each take
/// once he has been silent for more than a second.**
///
/// `Msg_DrawWindow#24` and `FUN_004B3ACD`, through the real machine, the real
/// director and the real mixer, with no device. The expected ticks are
/// computed from what the mixer reports — when a clip was last sounding — and
/// the rule's own numbers, typed: 81 ticks for `timer < 0x780`, and 64 ticks
/// for *"more than 999 ms"* at 16 ms a tick, counted from the tick the director
/// last saw him busy.
///
/// Ablation: `> 999` to `> 0` and `S200_03`'s tick is wrong; delete the cursor
/// advance and `S200_03` is never heard.
#[test]
fn a_tip_reads_its_first_line_and_then_its_takes_a_second_apart() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no voice clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let (mut g, a, mut m) = campaign();
    let mut sound = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    // Sixteen milliseconds of stereo at the mixer's 44.1 kHz.
    let mut buf = vec![0f32; 706 * 2];

    let names = ["s200_01.wav", "s200_02.wav", "s200_03.wav"];
    let mut first: BTreeMap<&str, usize> = BTreeMap::new();
    let mut last_sounding: BTreeMap<&str, usize> = BTreeMap::new();
    let mut opened_at = None;
    let mut first_line_timer = None;
    for t in 1..=8000 {
        tick(&mut m, &mut g, &a);
        if opened_at.is_none() && g.messages.timer() == 2000 && open_group(&g) == Some(200) {
            opened_at = Some(t);
        }
        director.listen(&mut sound, &m, &g);
        for n in names {
            if !first.contains_key(n) && sound.heard().contains(&n) {
                first.insert(n, t);
                if n == names[0] {
                    first_line_timer = Some(g.messages.timer());
                }
            }
        }
        sound.mix(&mut buf);
        for n in names {
            if sound.is_playing(n) {
                last_sounding.insert(n, t);
            }
        }
        if first.contains_key(names[2]) && last_sounding[names[2]] < t {
            break;
        }
    }
    let opened = opened_at.expect("tip 200 opened");
    assert_eq!(first_line_timer, Some(0x7C6), "the first line on Msg_DrawWindow's tick");

    let t2 = *first.get(names[1]).expect("S200_02.wav was never played");
    let t3 = *first.get(names[2]).expect("S200_03.wav was never played: the cursor did not advance");
    let end1 = last_sounding[names[0]];
    let end2 = last_sounding[names[1]];
    // The takes are first asked for 81 ticks in. If the first line was still
    // sounding then, the stamp is the tick after its last one; if not, nothing
    // was ever stamped and the take starts at once.
    let expect2 = if end1 + 1 >= opened + 81 { end1 + 1 + 63 } else { opened + 81 };
    assert_eq!(t2, expect2, "S200_02: opened {opened}, first line last sounding {end1}");
    assert_eq!(t3, end2 + 1 + 63, "S200_03: S200_02 last sounding {end2}");

    // And nothing else in the pool: the row is 26, 27, 0.
    let pool: Vec<String> =
        l2_game::audio::names::TAKE_POOL.iter().map(|n| n.to_ascii_lowercase()).collect();
    let takes: Vec<&str> = sound.heard().into_iter().filter(|h| pool.iter().any(|p| p == h)).collect();
    assert_eq!(takes, vec!["s200_02.wav", "s200_03.wav"]);
}

/// **A troop cry holds a tip's next take back exactly as the narrator does**,
/// because there is one buffer. `FUN_004B3ACD` asks `Sound_OneShotBusy()`, and
/// `Sound_PlayTroopCry` is `Sound_PlayFile` into the same `DAT_00522AEC` the
/// first line went into — so a cry keeps the next take waiting until a full
/// second after the cry ends. That is what folding the tips' and the battle's
/// two records of the buffer into one field was for (`docs/decisions.md`
/// C166), and until this test nothing had put a cry in front of a take.
///
/// Two runs of one new game. The first finds the tick `S200_02.wav` starts on.
/// The second is the same game with a cry put into the buffer on that tick,
/// before the director listens: the take must not start then, and must start
/// 64 ticks after the last tick the cry was sounding — the rule's own numbers,
/// as in the test above, counted from the cry instead of the narrator.
///
/// Ablation: delete `self.one_shot = Some(key)` from `Audio::play_file` and the
/// take starts on the cry's own tick.
#[test]
fn a_troop_cry_holds_back_a_tips_next_take_as_the_narrator_does() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no voice clips");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    const TAKE: &str = "s200_02.wav";
    // `Sound_PlayTroopCry`'s file for a knight told to attack, take 2.
    const CRY: &str = "knig_e2.wav";

    // (the tick `TAKE` was first heard, the last tick `CRY` was sounding)
    let run = |cry_on: Option<usize>| -> (Option<usize>, Option<usize>) {
        let (mut g, a, mut m) = campaign();
        let mut sound = Audio::headless(&platform.vfs);
        let mut director = audio::Director::new();
        // Sixteen milliseconds of stereo at the mixer's 44.1 kHz.
        let mut buf = vec![0f32; 706 * 2];
        let mut cry_last = None;
        for t in 1..=8000 {
            tick(&mut m, &mut g, &a);
            if cry_on == Some(t) {
                assert!(sound.play_file(CRY, true), "the buffer was busy on the take's own tick");
            }
            director.listen(&mut sound, &m, &g);
            if sound.heard().contains(&TAKE) {
                return (Some(t), cry_last);
            }
            sound.mix(&mut buf);
            if sound.is_playing(CRY) {
                cry_last = Some(t);
            }
        }
        (None, cry_last)
    };

    let on = run(None).0.expect("S200_02.wav was never played");
    let (take, cry_last) = run(Some(on));
    // The claim first: a take that starts on the cry's own tick returns before
    // the cry has been mixed once, so `cry_last` would be empty and an unwrap
    // ahead of this would report the wrong thing.
    assert_ne!(take, Some(on), "the take talked over the cry on tick {on}");
    let take = take.expect("S200_02.wav was never played once the cry had finished");
    let cry_last = cry_last.expect("the cry never sounded");
    assert_eq!(take, cry_last + 1 + 63, "a second of quiet from the cry's last tick, {cry_last}");
}
