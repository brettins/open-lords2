#![allow(unused_imports)]
use super::*;
use super::dismissal_and_timing::*;
use super::painting_and_layout::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, category, help, Record, Shape};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;

/// One of `Menu_HelpHowDoI`'s (`0x0043480C`) five records.
fn topic(group: u16) -> Record {
    Record { to: 0, group, category: category::HELP, ..Record::default() }
}

fn band_diff(a: &l2_view::Canvas, b: &l2_view::Canvas, top: i32, bottom: i32) -> usize {
    let mut n = 0;
    for y in top..bottom {
        for x in 0..640i32 {
            if a.at(x as usize, y as usize) != b.at(x as usize, y as usize) {
                n += 1;
            }
        }
    }
    n
}

fn paint(group: u16) -> l2_view::Canvas {
    let (mut g, a, mut m) = world();
    post(&mut g, topic(group));
    open_the_scroll(&mut m, &mut g, &a);
    painting(&mut g, &a, &mut m)
}

/// **`g_helpWindowGeom` (`0x004D6EB8`), all six records.** `[V]` read out of
/// `Lords2.exe`; the stored `w`/`h` are cells, so the frame is sixteen times
/// them.
#[test]
fn the_help_geometry_is_the_table_in_the_exe() {
    let f = |group: u16| {
        message::frame_of(&topic(group)).map(|f| (f.x, f.y, f.w, f.h))
    };
    assert_eq!(f(291), Some((32, 176, 26 * 16, 10 * 16)), "the FAQ index page's short box");
    for group in 292..=295 {
        assert_eq!(f(group), Some((16, 32, 28 * 16, 27 * 16)), "group {group}");
    }
    assert_eq!(f(296), Some((32, 160, 26 * 16, 12 * 16)), "the dead CD check");
    assert_eq!(f(290), None);
    assert_eq!(f(297), None);
}

/// **`DAT_004D6A8C + group * 4`**, whose group-291 entry is `0x004D6F18`.
///
/// `[V]` `1, 5, 5, 5, 10, 1`, which is `strings − 1` for all six
/// (`docs/formats/eng.md` §5).
#[test]
fn the_paragraph_counts_are_the_table_in_the_exe() {
    assert_eq!(
        (291..=296).map(help::paragraphs).collect::<Vec<_>>(),
        vec![1, 5, 5, 5, 10, 1]
    );
    assert_eq!(help::paragraphs(290), 0);
    for (group, strings) in help::TEXT {
        assert_eq!(strings.len(), help::paragraphs(*group) + 1, "group {group}");
        assert!(!strings[0].is_empty(), "group {group} has a heading");
    }
}

/// **This is the ablation.** Delete the `Shape::Help` arm from
/// `MessageScreen::draw` and both records fall to `draw_notice`, which paints
/// one string at `y + 0x50` and nothing below it — the band is identical in
/// both pictures and this fails.
#[test]
fn the_help_window_draws_every_paragraph_of_its_group() {
    let five = paint(292);
    let ten = paint(295);
    let (top, bottom) = (32 + 0x150, 32 + 0x170);
    assert!(
        band_diff(&five, &ten, top, bottom) > 0,
        "group 295's late paragraphs paint where group 292's window is bare"
    );
}

#[test]
fn two_topics_draw_their_own_words() {
    let grain = paint(292);
    let castle = paint(293);
    let (top, bottom) = (32 + 0x10, 32 + 0x60);
    assert!(
        band_diff(&grain, &castle, top, bottom) > 0,
        "*How do I grow grain?* and *How do I build a castle?* are two pictures"
    );
}

#[test]
fn a_click_on_the_corner_button_closes_a_help_topic() {
    let (mut g, a, mut m) = world();
    post(&mut g, topic(295));
    open_the_scroll(&mut m, &mut g, &a);
    let f = message::frame_of(&topic(295)).expect("the help window has geometry");
    let (x, y) = f.ok_button();
    send(&mut m, &mut g, &a, Event::Click { x: x + 12, y: y + 12 });
    tick(&mut m, &mut g, &a);
    assert_ne!(m.top_id(), Some(ScreenId::Message), "the scroll closed");
}

#[test]
fn the_transcription_matches_the_players_eng_file() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no install, so there is no L2.eng to hold it against");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    for (group, strings) in help::TEXT {
        for (i, ours) in strings.iter().enumerate() {
            assert_eq!(
                &assets.shell.text(*group as usize, i),
                ours,
                "group {group} string {i}"
            );
        }
    }
}
