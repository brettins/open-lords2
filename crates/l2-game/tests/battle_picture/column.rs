//! **The right column's chrome, `Misc_bat.PL8`** — the five blits
//! `Screen_DrawBattlefield` (`0x004233F7`) and `FUN_00423530` (`0x00423530`)
//! make and this engine did not.
//!
//! Every probe is read out of the **file's own frame headers**, never out of
//! `l2_view::chrome::misc_bat` or the call in `draw_column_chrome`: a `Pl8`
//! frame carries the position it was authored at, and each of the five agrees
//! with its call site, so the header is an independent witness to the
//! coordinate. `docs/agents.md`, *compute the probe from the constant you are
//! ablating*.

use super::*;

pub(crate) fn column_matched(canvas: &Canvas, frame: &DecodedFrame, (x, y): (i32, i32)) -> f64 {
    let w = frame.width as usize;
    let (mut hit, mut seen) = (0usize, 0usize);
    for i in 0..frame.indices.len() {
        if !frame.opaque[i] {
            continue;
        }
        let (px, py) = (x + (i % w) as i32, y + (i / w) as i32);
        if px < 0 || px >= 640 || py < 0 || py >= 480 {
            continue;
        }
        seen += 1;
        hit += usize::from(canvas.at(px as usize, py as usize) == frame.indices[i]);
    }
    if seen == 0 { 0.0 } else { hit as f64 / seen as f64 }
}

pub(crate) fn bytes(platform: &l2_mods::Platform) -> Vec<u8> {
    platform.vfs.read("Misc_bat.PL8").expect("Misc_bat.PL8")
}

pub(crate) fn sheet(platform: &l2_mods::Platform) -> Sheet {
    Sheet::new(bytes(platform)).expect("a PL8")
}

fn header(raw: &[u8], index: usize) -> (i32, i32) {
    let pl8 = l2_formats::Pl8::parse(raw).expect("a PL8");
    let f = &pl8.frames[index];
    (f.x as i32, f.y as i32)
}

fn column(paused: bool, shields: (u8, u8)) -> (Game, Machine, Canvas, Assets, l2_mods::Platform) {
    let (assets, platform) = install().expect("checked by the caller");
    let (mut g, mut m) =
        staged(30, &[(Troop::Peasants, 8)], &[(Troop::Peasants, 8)], |(x, y)| {
            (x as i32 - 4, y as i32 - 6)
        });
    g.kingdom.realms[1].shield_index = shields.0;
    g.kingdom.realms[2].shield_index = shields.1;
    g.battle.as_deref_mut().expect("a live battle").paused = paused;
    let mut canvas = Canvas::screen();
    paint(&mut m, &mut g, &assets, &mut canvas);
    (g, m, canvas, assets, platform)
}

#[test]
fn the_right_columns_three_plates_are_misc_bats_own_frames_at_their_own_positions() {
    if install().is_none() {
        l2_testkit::skip!("no game install, so no Misc_bat.PL8 to draw the column from");
    }
    let (_g, _m, canvas, _a, platform) = column(false, (1, 2));
    let raw = bytes(&platform);
    let sheet = sheet(&platform);

    for (index, want, floor) in
        [(0usize, (0x1E0, 0xB8), 1.0), (2, (0x1E0, 0x19C), 0.6), (1, (0x1E0, 0x1C0), 0.6)]
    {
        let f = sheet.frame(index).expect("the frame decodes");
        assert_eq!(header(&raw, index), want, "frame {index}'s header moved");
        let got = column_matched(&canvas, &f, want);
        assert!(got >= floor, "only {:.0}% of frame {index} is at {want:?}", got * 100.0);
    }
}

/// `Screen_DrawBattlefield` draws `DAT_00568934 + 6` at `(0x1E2, 0x19D)` and
/// `DAT_00568938 + 6` at `(0x230, 0x19D)`, and `g_battleArmyB` — the
/// right-hand one — is the **side-0** army (`l2_view::scene`'s
/// `BattleBanner_Draw` note). So a battle whose sides hold different realms
/// puts the *attacker's* plate on the left, which is the way round the names
/// argue against.
#[test]
fn each_sides_shield_plate_is_its_realms_and_the_left_plate_is_side_four() {
    if install().is_none() {
        l2_testkit::skip!("no game install, so no Misc_bat.PL8 to draw the plates from");
    }
    let (_g, _m, canvas, _a, platform) = column(false, (3, 5));
    let sheet = sheet(&platform);

    let left = sheet.frame(6 + 5).expect("side 4's plate decodes");
    let right = sheet.frame(6 + 3).expect("side 0's plate decodes");
    assert!(
        column_matched(&canvas, &left, (0x1E2, 0x19D)) == 1.0,
        "realm 5's plate is not at the left recess"
    );
    assert!(
        column_matched(&canvas, &right, (0x230, 0x19D)) == 1.0,
        "realm 3's plate is not at the right recess"
    );
    assert!(
        column_matched(&canvas, &left, (0x230, 0x19D)) < 1.0,
        "both recesses carry side 4's plate"
    );
}

/// **An ownerless side's plate is frame 12, not frame 6** — the seeder's
/// `if (DAT_00568934 == 0) DAT_00568934 = 6;`, and frame 6 is the lit retreat
/// button, so without the clamp the recess would hold a button.
#[test]
fn a_shieldless_realms_plate_is_clamped_to_the_sixth_and_not_to_a_button() {
    if install().is_none() {
        l2_testkit::skip!("no game install, so no Misc_bat.PL8 to draw the plates from");
    }
    let (_g, _m, canvas, _a, platform) = column(false, (1, 0));
    let sheet = sheet(&platform);

    let clamped = sheet.frame(6 + 6).expect("the sixth plate decodes");
    let button = sheet.frame(6).expect("the retreat button decodes");
    assert!(
        column_matched(&canvas, &clamped, (0x1E2, 0x19D)) == 1.0,
        "realm 0's plate is not the sixth"
    );
    assert!(
        column_matched(&canvas, &button, (0x1E2, 0x19D)) < 1.0,
        "the left recess holds the retreat button"
    );
}

/// `FUN_00423530` puts frame 5 at `(0x1E1, 0x1C1)` under
/// `if (DAT_0053F238 != 0)` and frame 6 at `(0x201, 0x1C1)` under
/// `if (DAT_00568964 == 1)` — input armed, which `Battle_Start` sets before the
/// screen is raised and nothing on it clears.
#[test]
fn the_pause_lamp_is_drawn_only_while_the_battle_is_paused() {
    if install().is_none() {
        l2_testkit::skip!("no game install, so no Misc_bat.PL8 to draw the lamps from");
    }
    let (_g, _m, paused, _a, platform) = column(true, (1, 2));
    let (_g2, _m2, running, _a2, _p2) = column(false, (1, 2));
    let sheet = sheet(&platform);

    let lamp = sheet.frame(5).expect("the pause lamp decodes");
    let retreat = sheet.frame(6).expect("the retreat lamp decodes");
    assert!(column_matched(&paused, &lamp, (0x1E1, 0x1C1)) == 1.0, "no lamp on a paused battle");
    assert!(
        column_matched(&running, &lamp, (0x1E1, 0x1C1)) < 1.0,
        "the lamp is lit on a running battle"
    );
    for (canvas, what) in [(&paused, "paused"), (&running, "running")] {
        assert!(
            column_matched(canvas, &retreat, (0x201, 0x1C1)) == 1.0,
            "no retreat lamp on a {what} battle"
        );
    }
}

#[test]
fn our_placeholder_buttons_give_way_to_the_strips_own_pictures() {
    if install().is_none() {
        l2_testkit::skip!("no game install, so no Misc_bat.PL8 to draw the strip from");
    }
    let (_g, _m, canvas, _a, platform) = column(false, (1, 2));
    let f = sheet(&platform).frame(1).expect("the strip decodes");

    // Slot 2 of `Hotspot_Test(0x1E0, 0x1C0, &DAT_004DC710, 5)`, which no lamp
    // is drawn over, compared column by column inside the strip's own frame.
    let (x0, y0) = (0x1E0 + 2 * 32, 0x1C0);
    let w = f.width as usize;
    let (mut hit, mut seen) = (0usize, 0usize);
    for i in 0..f.indices.len() {
        let (dx, dy) = ((i % w) as i32, (i / w) as i32);
        if !f.opaque[i] || !(2 * 32..3 * 32).contains(&dx) {
            continue;
        }
        seen += 1;
        hit += usize::from(canvas.at((0x1E0 + dx) as usize, (y0 + dy) as usize) == f.indices[i]);
    }
    assert!(seen > 0, "slot 2 is outside the strip");
    assert_eq!(hit, seen, "{} of {seen} pixels of slot 2 are ours, not the strip's", seen - hit);
    let _ = x0;
}
