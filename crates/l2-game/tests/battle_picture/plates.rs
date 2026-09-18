//! **The column's numbers and the banner plates** — `FUN_00423530`'s two
//! `Ui_DrawNumberRight` counts (`0x00423530`) and `FUN_004238B8` /
//! `FUN_004239D5` (`0x004238B8`, `0x004239D5`), the plate and the count a held
//! figure gets in the right column.

use super::*;
use l2_game::shell::{font, Pen};

fn pen<'a>(a: &'a Assets) -> Pen<'a> {
    Pen {
        assets: &a.shell,
        ink: &a.ink,
        chrome: a.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    }
}

fn stamped(draw: impl FnOnce(&mut Canvas)) -> Vec<(usize, usize, u8)> {
    let mut want = Canvas::screen();
    draw(&mut want);
    let mut out = Vec::new();
    for y in 0..480 {
        for x in 0..640 {
            let c = want.at(x, y);
            if c != 0 {
                out.push((x, y, c));
            }
        }
    }
    out
}

fn found(canvas: &Canvas, want: &[(usize, usize, u8)]) -> usize {
    want.iter().filter(|&&(x, y, c)| canvas.at(x, y) == c).count()
}

fn counted() -> (Game, Canvas, Assets) {
    let (assets, _platform) = install().expect("checked by the caller");
    let (mut g, mut m) =
        staged(30, &[(Troop::Peasants, 8)], &[(Troop::Peasants, 3)], |(x, y)| {
            (x as i32 - 4, y as i32 - 6)
        });
    let mut canvas = Canvas::screen();
    paint(&mut m, &mut g, &assets, &mut canvas);
    (g, canvas, assets)
}

#[test]
fn the_two_living_men_counts_are_centred_on_the_plate_the_shields_sit_on() {
    if install().is_none() {
        l2_testkit::skip!("no game install, so no body font to draw the counts with");
    }
    let (g, canvas, a) = counted();
    let p = pen(&a);
    let men = (live(&g).runner.men(l2_sim::SIDE_B), live(&g).runner.men(l2_sim::SIDE_A));
    assert_ne!(men.0, men.1, "the two sides must differ for the swap to be visible");

    for (x, n, side) in [(0x1FA, men.0, "left"), (0x24A, men.1, "right")] {
        let want = stamped(|c| {
            p.body_centred(c, x, 0x1A6, 0x38, &n.to_string(), 0x20);
        });
        assert!(!want.is_empty(), "the {side} count renders nothing");
        assert_eq!(found(&canvas, &want), want.len(), "the {side} count is not at ({x}, 0x1A6)");
    }
    let swapped = stamped(|c| {
        p.body_centred(c, 0x1FA, 0x1A6, 0x38, &men.1.to_string(), 0x20);
    });
    assert!(found(&canvas, &swapped) < swapped.len(), "the left box carries side 0's count");
}

fn picked() -> (Game, Canvas, Assets, l2_mods::Platform) {
    let (assets, platform) = install().expect("checked by the caller");
    let (mut g, mut m) =
        staged(30, &[(Troop::Peasants, 8)], &[(Troop::Peasants, 8)], |(x, y)| {
            (x as i32 - 4, y as i32 - 6)
        });
    let cells: Vec<(u8, u8)> = live(&g)
        .runner
        .fighters
        .iter()
        .enumerate()
        .filter(|(i, f)| f.side == SIDE_A && live(&g).runner.is_alive(*i))
        .map(|(_, f)| (f.x, f.y))
        .collect();
    let lo = (cells.iter().map(|c| c.0).min().unwrap(), cells.iter().map(|c| c.1).min().unwrap());
    let hi = (cells.iter().map(|c| c.0).max().unwrap(), cells.iter().map(|c| c.1).max().unwrap());
    let (lx, ly) = pixel(live(&g), lo);
    let (hx, hy) = pixel(live(&g), hi);
    let from = (lx - bf::TILE / 2 + 2, ly - bf::TILE / 2 + 2);
    let to = (hx + bf::TILE / 2 - 2, hy + bf::TILE / 2 - 2);
    send(&mut m, &mut g, &assets, Event::Pointer { x: from.0, y: from.1 });
    send(&mut m, &mut g, &assets, Event::Click { x: from.0, y: from.1 });
    send(&mut m, &mut g, &assets, Event::Pointer { x: to.0, y: to.1 });
    send(&mut m, &mut g, &assets, Event::Release { x: to.0, y: to.1 });
    send(&mut m, &mut g, &assets, Event::Pointer { x: 240, y: 240 });
    assert!(live(&g).runner.selected_count(1) > 0, "the box picked nobody");
    let mut canvas = Canvas::screen();
    paint(&mut m, &mut g, &assets, &mut canvas);
    (g, canvas, assets, platform)
}

/// **A held figure's banner is `Misc_bat` frame `layout.frame + troopType` at
/// the slot's own `(x, y)`** — `FUN_004238B8`, over frame 0.
///
/// The bases are read out of `Lords2.exe` at `DAT_004D31F0` (file offset
/// `0xD13F0`): 13 for the twelve-slot layout, 24 for eighteen, 35 for fifty,
/// eleven troops apart. Under thirteen figures the first band applies, and
/// peasants are troop type 0, so slot `n` must hold frame 13.
#[test]
fn each_held_figures_banner_is_its_troops_plate_from_the_twelve_slot_band() {
    if install().is_none() {
        l2_testkit::skip!("no game install, so no Misc_bat.PL8 to draw the banners from");
    }
    let (g, canvas, _a, platform) = picked();
    let picked = live(&g).runner.selected_fighters(1);
    let layout = bf::BannerLayout::for_count(picked.len());
    assert_eq!(layout.slots, 12, "this staging is meant to take the twelve-slot band");
    let sheet = crate::column::sheet(&platform);

    for (slot, &f) in picked.iter().enumerate() {
        let troop = live(&g).runner.fighters[f].troop;
        let frame = sheet.frame(layout.frame + troop.index()).expect("the plate decodes");
        let r = layout.rect(slot);
        // Not all of it: `FUN_004239D5` writes the count over the plate, which
        // costs a few per cent of its pixels.
        let got = crate::column::column_matched(&canvas, &frame, (r.x, r.y));
        assert!(got >= 0.9, "only {:.0}% of slot {slot}'s plate is at {r:?}", got * 100.0);
        let other = sheet.frame(24 + troop.index()).expect("the middle band decodes");
        let wrong = crate::column::column_matched(&canvas, &other, (r.x, r.y));
        assert!(wrong < 0.9, "slot {slot} carries the eighteen-slot band's plate");
    }
}

/// **The men on a banner are drawn `0x14` right of the plate and `2` down**,
/// and `0xC` right once the count needs three digits — `FUN_004239D5`'s
/// `man.men < 0x65` fork, which is room for the extra digit.
#[test]
fn a_banners_men_are_drawn_inside_its_plate_at_the_forks_own_offset() {
    if install().is_none() {
        l2_testkit::skip!("no game install, so no body font to draw the banner counts with");
    }
    let (g, canvas, a, _platform) = picked();
    let p = pen(&a);
    let picked = live(&g).runner.selected_fighters(1);
    let layout = bf::BannerLayout::for_count(picked.len());

    for (slot, &f) in picked.iter().enumerate() {
        let sim = live(&g).runner.fighters[f].sim;
        let men = live(&g).runner.sim.figures[sim].men;
        assert!(men > 0, "a held figure with no men would take no slot");
        let r = layout.rect(slot);
        let dx = if men < 0x65 { 0x14 } else { 0xC };
        let want = stamped(|c| {
            p.body(c, r.x + dx, r.y + 2, &men.to_string(), font::TEXT);
        });
        assert!(!want.is_empty(), "slot {slot}'s count renders nothing");
        assert_eq!(found(&canvas, &want), want.len(), "slot {slot}'s count is not on its plate");
    }
}
