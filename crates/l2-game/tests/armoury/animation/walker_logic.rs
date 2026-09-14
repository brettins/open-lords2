#![allow(unused_imports)]
use super::*;
use super::sheet_validation::*;
use super::rendering::*;
use super::*;
use super::hit_map::*;
use super::rack::*;
use super::screenshots::*;
use std::path::PathBuf;
use l2_game::game::Assets;
use l2_game::input::{Event, Rect};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::armoury;
use l2_game::Game;
use l2_kingdom::tables::{Tables, WEAPON_TYPE_COUNT};
use l2_view::Canvas;

// -------------------------------------------------- the room in motion

/// **The soldier stops under the weapon he has come for**, and the two tables
/// that say so were written by two people who never mentioned each other.
///
/// `g_armouryWalkStopX` (`0x004DE6F0`) is where `Armoury_DrawWalker` stops him.
/// `g_armouryWallItems` (`0x004D2D88`) is where `Armoury_DrawWallItems` hangs
/// the weapon. Neither table refers to the other and nothing in the binary
/// derives one from the other — so their agreement is evidence, in the sense
/// `CLAUDE.md` means it.
///
/// **The probe is the wall item's own rectangle out of the sheet**, not
/// anything computed from the stop position, which is the trap
/// `docs/agents.md` records against animation offsets specifically: ablate
/// `WALKER_STOP_X` to zeros and every slot but the crossbow fails, because
/// nothing in the assertion mentions the constant under test.
#[test]
fn the_soldier_stops_under_the_weapon_he_has_come_for() {
    let (_g, assets) = world!();
    let items = assets.shell.sheet(armoury::items_sheet(1)).expect("arm_it_r.pl8");
    for slot in 1..=WEAPON_TYPE_COUNT {
        let (frame, wx, _wy) = armoury::WALL[slot - 1];
        let item = items.frame(frame).expect("a wall item");
        let (item_l, item_r) = (wx, wx + item.width as i32);

        let sheet = assets
            .shell
            .sheet(armoury::walker_sheet(1, slot as u8))
            .expect("the red soldier's sheet");
        let man = sheet.frame(0).expect("his first frame");
        let stop = armoury::WALKER_STOP_X[slot];
        let (man_l, man_r) = (stop, stop + man.width as i32);

        assert!(
            man_l < item_r && item_l < man_r,
            "slot {slot}: he stops at {man_l}..{man_r} and the weapon hangs at \
             {item_l}..{item_r} - he is not standing under it",
        );
    }
}

/// **The whole animation, driven the way the game drives it.**
///
/// Open a rack, assign a man to it, move to another rack — and a soldier of the
/// *first* rack's type walks in from off the left, stops under that weapon,
/// plays the five pickup frames, and carries it off the right-hand side.
///
/// The order is `Armoury_ClickRack`'s and it is the finding: the walk is fired
/// with `g_armourySelectedType` **before** it is overwritten, so the man who
/// comes is the one whose weapon the player has just finished handing out.
#[test]
fn a_soldier_walks_over_and_takes_the_weapon() {
    let (mut g, a) = world!();
    // Continue waits twenty ticks now, and ticks run the tip screens.
    g.prefs.tip_screens = false;
    let county = own_county(&g);
    g.selected = county;
    let realm = g.player as usize;

    let stocked: Vec<u8> = (0..WEAPON_TYPE_COUNT)
        .filter(|&s| g.kingdom.realms[realm].weapons[s] > 0)
        .map(|s| s as u8 + 1)
        .collect();
    assert!(!stocked.is_empty(), "the fixture's realm has an armoury");
    let first = stocked[0];

    let mut m = Machine::new(ScreenId::Campaign);
    let send = |m: &mut Machine, g: &mut Game, e: Event| {
        let mut ctx = Ctx { game: g, assets: &a };
        m.handle(e, &mut ctx);
    };
    let tick = |m: &mut Machine, g: &mut Game| {
        let mut ctx = Ctx { game: g, assets: &a };
        m.update(&mut ctx);
    };

    send(&mut m, &mut g, Event::KeyDown(l2_game::input::Key::letter('r')));
    tick(&mut m, &mut g);
    send(
        &mut m,
        &mut g,
        Event::Click {
            x: l2_game::screens::army::SLIDER_X + 40,
            y: l2_game::screens::army::base(false) + 0x20,
        },
    );
    let cont = l2_game::screens::army::continue_button(false);
    send(&mut m, &mut g, Event::Click { x: cont.centre_x(), y: cont.y + cont.h / 2 });
    // `RaiseArmy_Continue` is `Widget_Test` kind 5: twenty frames after the press.
    for _ in 0..l2_game::press::DELAYED_FRAMES {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        m.update(&mut ctx);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Armoury(county)));

    // Open the first rack. **Nobody walks yet** — the latch was just set, so
    // `FUN_004AABD8`'s `latch < chosen` is false.
    let at = grid_box(&a, first).expect("the weapon has a region");
    send(&mut m, &mut g, Event::Click { x: at.centre_x(), y: at.y + at.h / 2 });
    assert_eq!(m.top_id(), Some(ScreenId::Rack(county, first)));
    assert!(!g.levy.anim.walker.active, "opening a rack sends nobody");

    // Assign one man to it, then leave for another rack — or, if the realm
    // stocks only one weapon, back out and open the same one again, which is
    // the same call with the same guard.
    let plus = armoury::button_box(0);
    send(&mut m, &mut g, Event::Click { x: plus.centre_x(), y: plus.y + plus.h / 2 });
    assert_eq!(g.levy.basket.troops()[first as usize], 1);
    assert!(!g.levy.anim.walker.active, "assigning a man does not send him yet");

    match stocked.get(1) {
        Some(&second) => {
            let at = grid_box(&a, second).expect("the second weapon has a region");
            send(&mut m, &mut g, Event::Click { x: at.centre_x(), y: at.y + at.h / 2 });
            assert_eq!(m.top_id(), Some(ScreenId::Rack(county, second)));
        }
        None => {
            send(
                &mut m,
                &mut g,
                Event::Click { x: armoury::RACK_OK.centre_x(), y: armoury::RACK_OK.y + 12 },
            );
            send(&mut m, &mut g, Event::Click { x: at.centre_x(), y: at.y + at.h / 2 });
        }
    }

    let w = g.levy.anim.walker;
    assert!(w.active, "a soldier set off");
    assert_eq!(w.slot, first, "and he is of the type just equipped");
    assert_eq!(w.x, armoury::WALKER_START_X, "starting off the left edge");

    // Now walk him. Everything below is `Anim::tick`, fixed ticks and
    // no clock at all.
    let stop = armoury::WALKER_STOP_X[first as usize];
    let mut walked_in = false;
    let mut pickup: Vec<usize> = Vec::new();
    let mut carried: Vec<usize> = Vec::new();
    for _ in 0..4000 {
        tick(&mut m, &mut g);
        let w = g.levy.anim.walker;
        if !w.active {
            break;
        }
        if w.x < stop {
            assert!(w.frame < armoury::WALK_PHASES as usize, "walking in, empty-handed");
            walked_in = true;
        } else if w.frame >= armoury::CARRY_FIRST {
            carried.push(w.frame);
        } else {
            pickup.push(w.frame);
        }
    }

    assert!(walked_in, "he walked in");
    assert!(!g.levy.anim.walker.active, "and the walk ended");
    assert!(
        pickup.contains(&armoury::PICKUP_FIRST) && pickup.contains(&armoury::PICKUP_LAST),
        "he played the whole pickup, {:?}",
        pickup.first().zip(pickup.last()),
    );
    let mut phases: Vec<usize> = carried.iter().map(|f| f - armoury::CARRY_FIRST).collect();
    phases.sort_unstable();
    phases.dedup();
    assert_eq!(
        phases.len(),
        armoury::WALK_PHASES as usize,
        "and carried it out through all eight phases",
    );
}

/// **A pulse is two of our ticks, because `Tick_Pulses` resets its stamp.**
///
/// `if (0x13 < now - stamp) { …; stamp = now; }` — the stamp goes to the frame
/// that fired, not twenty milliseconds on, so the remainder is thrown away.
/// On a 16 ms tick the first sum past 19 is 32, and every pulse after it is
/// another 32. `g_pulse80` is every fourth pulse: eight ticks.
///
/// This test used to be called *"eighty milliseconds of our ticks is four
/// pulses"* and asserted five ticks to four pulses — the accumulate-and-carry
/// reading that made the walk 1.6 times the original's speed. Every number
/// below is typed from the gate, not computed from `PULSE_MS`. **Ablation,
/// run:** put the `while acc_ms >= PULSE_MS { acc_ms -= PULSE_MS }` loop back
/// and the steps land on ticks 2, 3, 4, 5, 7, 8.
#[test]
fn a_pulse_is_two_of_our_ticks_because_tick_pulses_resets_its_stamp() {
    let mut anim = armoury::Anim::default();
    anim.walker.latch(1, 0);
    assert!(anim.walker.start(1, 1), "one man was assigned, so he sets off");
    let x0 = anim.walker.x;
    let mut walked = Vec::new();
    for _ in 0..8 {
        anim.tick();
        walked.push(anim.walker.x - x0);
    }
    assert_eq!(walked, vec![0, 4, 4, 8, 8, 12, 12, 16], "four pixels on every second tick");
    assert_eq!(anim.torch, 1, "and one 80 ms pulse in eight ticks");
    assert_eq!(anim.weapon, 1);

    // A hundred ticks: fifty pulses, and the 80 ms pulse on every fourth of
    // them — twelve, so neither counter has wrapped.
    for _ in 0..92 {
        anim.tick();
    }
    assert_eq!(anim.torch, 12);
    assert_eq!(anim.weapon, 12);
}

/// **`FUN_004AABD8`'s three guards, each one ablated by taking it away.**
///
/// `if (0 < type && latch[type] < chosen[type] && walkActive < 1)`.
#[test]
fn nobody_walks_for_the_peasants_for_an_untouched_rack_or_over_another_walk() {
    let mut anim = armoury::Anim::default();

    // Slot 0 is the unequipped peasants and has no weapon to fetch.
    anim.walker.latch(0, 0);
    assert!(!anim.walker.start(0, 5), "slot 0 has nothing on the wall");

    // A rack the player opened and did nothing in: the latch equals `chosen`.
    anim.walker.latch(2, 7);
    assert!(!anim.walker.start(2, 7), "no man was assigned, so nobody walks");
    assert!(anim.walker.start(2, 8), "one was, so he does");

    // And a walk in progress is not restarted under itself.
    anim.walker.latch(3, 0);
    assert!(!anim.walker.start(3, 4), "one soldier at a time");
    assert_eq!(anim.walker.slot, 2, "the first one is still walking");
}

