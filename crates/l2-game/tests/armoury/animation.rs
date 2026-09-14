#![allow(unused_imports)]
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

/// The five item sheets are five different pictures, all of them present, and
/// all of them carrying the twenty-one frames the two screens index — six
/// weapons, eight portraits, and the six little icons the levy screen prints
/// its stocks beside.
#[test]
fn all_five_armoury_sheets_carry_the_frames_both_screens_ask_for() {
    let (_g, assets) = world!();
    let mut sizes: Vec<usize> = Vec::new();
    for name in armoury::ITEM_SHEETS {
        let sheet = assets.shell.sheet(name).unwrap_or_else(|| panic!("{name} is in the install"));
        for &(frame, ..) in &armoury::WALL {
            assert!(sheet.frame(frame).is_some(), "{name} has no wall frame {frame}");
        }
        for slot in 0..armoury::RACKS_DRAWN {
            let frame = armoury::RACKS[slot].0;
            assert!(sheet.frame(frame).is_some(), "{name} has no rack frame {frame}");
        }
        for i in 0..WEAPON_TYPE_COUNT {
            let frame = l2_game::screens::army::ICON_FRAME_BASE + i;
            assert!(sheet.frame(frame).is_some(), "{name} has no levy icon {frame}");
        }
        sizes.push(sheet.frame(0).map(|f| f.indices.len()).unwrap_or(0));
    }
    assert!(sizes.iter().all(|&n| n > 0), "a sheet decoded to nothing");

    // And one per weapon type for the rack panel.
    for name in armoury::WEAPON_SHEETS {
        let sheet = assets.shell.sheet(name).unwrap_or_else(|| panic!("{name} is in the install"));
        let f = sheet.frame(0).expect("frame 0");
        assert_eq!((f.width, f.height), (100, 100), "{name}'s picture is not 100 x 100");
    }
}

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

/// **The artwork counts the animation for us.** `Armoury_DrawWalker` reaches
/// frame `cycle + 0` walking in (eight), `pickup / 3 + 8` taking the weapon
/// down (five, `8 … 0x0C`) and `cycle + 0x0D` carrying it out (eight). That is
/// 21, and every one of the thirty `Trp_*.pl8` sheets holds exactly 21 frames.
///
/// The height is the second coincidence and it is a better one: the frames are
/// 158 tall and `Screen_Armoury` saves its four strips at height `0x9E`, which
/// **is** 158. A strip is exactly one soldier.
#[test]
fn every_walker_sheet_holds_exactly_the_frames_the_walk_reaches() {
    let (_g, assets) = world!();
    let last = armoury::CARRY_FIRST + armoury::WALK_PHASES as usize - 1;
    assert_eq!(last, 20, "the walk's highest frame index");

    let mut seen = 0;
    for colour in 0..armoury::WALKER_SHEETS.len() as u8 {
        for slot in 1..=WEAPON_TYPE_COUNT as u8 {
            let name = armoury::walker_sheet(colour, slot);
            let sheet = assets.shell.sheet(name).unwrap_or_else(|| panic!("{name} is installed"));
            let f = sheet.frame(last).unwrap_or_else(|| panic!("{name} has no frame {last}"));
            assert_eq!(f.height as i32, armoury::STRIP_H, "{name} is not one strip tall");
            assert!(
                sheet.frame(last + 1).is_none(),
                "{name} has a frame past the end of the walk",
            );
            seen += 1;
        }
    }
    assert_eq!(seen, 36, "six colours by six weapons, red twice");
}

/// **`Armtorch.pl8` is two torches and says so**: thirteen frames, then
/// thirteen more, which is `Armoury_DrawTorches`' `+ 0x0D` measured off the
/// file. Each `Arm_<weapon>.pl8` is 24, which is
/// `DAT_005AEA48`'s wrap.
#[test]
fn the_two_animation_counters_wrap_where_their_sheets_end() {
    let (_g, assets) = world!();
    let torch = assets.shell.sheet(armoury::TORCH_SHEET).expect("Armtorch.pl8");
    let n = armoury::TORCH_FRAMES as usize;
    assert_eq!(armoury::TORCH_SECOND, n, "the second torch begins where the first ends");
    let first = torch.frame(0).expect("frame 0");
    let second = torch.frame(n).expect("the second torch");
    assert_ne!(
        (first.width, first.height),
        (second.width, second.height),
        "the two blocks are 77 x 49 and 77 x 48 - a boundary an index off by one breaks",
    );
    assert!(torch.frame(n * 2).is_none(), "there is no third torch");

    for name in armoury::WEAPON_SHEETS {
        let sheet = assets.shell.sheet(name).expect("a weapon sheet");
        let last = armoury::WEAPON_FRAMES as usize - 1;
        assert!(sheet.frame(last).is_some(), "{name} has no frame {last}");
        assert!(sheet.frame(last + 1).is_none(), "{name} has a 25th frame");
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

/// **`Armoury_RestoreWalkerStrip`'s four rectangles**, and the one place the
/// original's own erase does not quite reach.
///
/// The strip chosen for a given x is 240 pixels wide (160 for the last) and the
/// soldier is 89 wide, so at the top of each band his right-hand columns stand
/// **five pixels** outside the rectangle that gets restored. Measured off the
/// sheet: the assertion is on the frames' own opaque
/// bounding box, so it is about what is painted and not about the record's
/// declared width.
#[test]
fn the_erase_strip_is_the_original_s_four_and_the_soldier_overhangs_it() {
    let (_g, assets) = world!();

    for &(x, w) in &armoury::STRIPS {
        let r = armoury::walker_strip(x);
        assert_eq!((r.x, r.w, r.y, r.h), (x, w, armoury::WALKER_Y, armoury::STRIP_H));
    }
    assert_eq!(armoury::walker_strip(armoury::WALKER_START_X).x, 0, "he starts in the first");
    assert_eq!(armoury::walker_strip(0x9F).x, 0);
    assert_eq!(armoury::walker_strip(0xA0).x, 0xA0);
    assert_eq!(armoury::walker_strip(0x13F).x, 0xA0);
    assert_eq!(armoury::walker_strip(0x140).x, 0x140);
    assert_eq!(armoury::walker_strip(0x1DF).x, 0x140);
    assert_eq!(armoury::walker_strip(0x1E0).x, 0x1E0);
    assert_eq!(armoury::walker_strip(0x27F).x, 0x1E0);

// The widest opaque column any of his frames paints.
    let sheet = assets.shell.sheet(armoury::walker_sheet(1, 1)).expect("Trp_xb_r.pl8");
    let mut widest = 0;
    for f in 0..=armoury::CARRY_FIRST + armoury::WALK_PHASES as usize - 1 {
        let frame = sheet.frame(f).expect("a walk frame");
        for (i, &o) in frame.opaque.iter().enumerate() {
            if o {
                widest = widest.max(i % frame.width as usize + 1);
            }
        }
    }
    assert!(widest > 0, "the sheet decoded to nothing");

    // The last x that still chooses the first strip, and how far past its right
    // edge he reaches there. `x` steps by four from -0x50, so it is even.
    let last = 0xA0 - armoury::WALKER_STEP;
    let strip = armoury::walker_strip(last);
    let over = (last + widest as i32) - (strip.x + strip.w);
    assert!(
        over > 0,
        "he was expected to overhang the strip he is erased with; widest opaque \
         column {widest}, overhang {over}",
    );
    assert!(over < 16, "and only by a sliver, not by half a man: {over}");
}

/// **The soldier and the torches are on the screen**, matched against the
/// sheets they come out of.
///
/// `docs/agents.md`: *"is it drawn" and "can it be seen" are different claims.*
/// Every other test above this one is about the walker's *state*, and all of
/// them pass with the blit deleted. This one takes the frame the animation says
/// is current, walks its opaque pixels, and requires the canvas to hold that
/// exact palette index at that exact screen position — so a garbage sprite, the
/// wrong frame of the right sheet and a draw that lands off the bottom of the
/// page all fail, and so does no draw at all.
///
/// The rack panel is the case worth having: it is an *overlay*, so the room and
/// everything moving in it is painted by the armoury underneath it. Both are
/// checked, because that is the sharing `Screen_DrawWidgets` does with two
/// nearly identical arms.
#[test]
fn the_soldier_and_the_torches_are_painted_where_the_animation_says() {
    let (mut g, a) = world!();
    let county = own_county(&g);
    g.selected = county;
    g.open_levy(county);

    let realm = g.player as usize;
    let shield = g.kingdom.realms[realm].shield_index;
    let slot = (0..WEAPON_TYPE_COUNT)
        .find(|&s| g.kingdom.realms[realm].weapons[s] > 0)
        .map(|s| s as u8 + 1)
        .expect("the fixture's realm has an armoury");

    // Put him mid-stride, at a torch phase that is not zero so a counter stuck
// at its initial value is a failure.
    g.levy.anim.walker.latch(slot, 0);
    assert!(g.levy.anim.walker.start(slot, 1));
    for _ in 0..200 {
        g.levy.anim.tick();
    }
    let w = g.levy.anim.walker;
    let torch_phase = g.levy.anim.torch;
    assert!(w.active && w.x > 0, "he is on the floor at x {}", w.x);
    assert!(torch_phase > 0, "and the torches have moved");

    let expect = |c: &Canvas, sheet: &str, index: usize, ox: i32, oy: i32, what: &str| {
        let f = a.shell.sheet(sheet).expect("the sheet").frame(index).expect("the frame");
        let (mut hits, mut misses) = (0usize, 0usize);
        for y in 0..f.height as i32 {
            for x in 0..f.width as i32 {
                let i = y as usize * f.width as usize + x as usize;
                if !f.opaque[i] {
                    continue;
                }
                let (px, py) = (ox + x, oy + y);
                if px < 0 || py < 0 || px >= 640 || py >= 480 {
                    continue;
                }
                if c.at(px as usize, py as usize) == f.indices[i] {
                    hits += 1;
                } else {
                    misses += 1;
                }
            }
        }
        assert!(hits > 200, "{what}: only {hits} of its pixels are on the canvas");
        assert_eq!(misses, 0, "{what}: {misses} pixels differ from {sheet} frame {index}");
    };

    for (screen, what) in [
        (ScreenId::Armoury(county), "the armoury"),
        (ScreenId::Rack(county, slot), "the rack panel over it"),
    ] {
        let mut m = Machine::new(ScreenId::Armoury(county));
        if screen != ScreenId::Armoury(county) {
            m.push(screen);
        }
        assert_eq!(m.top_id(), Some(screen));
        let c = frame(&mut m, &mut g, &a);
        expect(
            &c,
            armoury::walker_sheet(shield, w.slot),
            w.frame,
            w.x,
            armoury::WALKER_Y,
            &format!("{what}: the walking soldier"),
        );
        for (i, &(tx, ty)) in armoury::TORCH_AT.iter().enumerate() {
            expect(
                &c,
                armoury::TORCH_SHEET,
                torch_phase as usize + i * armoury::TORCH_SECOND,
                tx,
                ty,
                &format!("{what}: torch {i}"),
            );
        }

        // And the weapon turning in the panel's own well, third
        // animation and the one that was drawing frame 0 for ever. **This was
        // added because ablating it found nothing**: every assertion above is
        // about the room, and the well is on the panel.
        if screen != ScreenId::Armoury(county) {
            expect(
                &c,
                armoury::WEAPON_SHEETS[slot as usize - 1],
                g.levy.anim.weapon as usize,
                armoury::WEAPON_AT.0,
                armoury::WEAPON_AT.1,
                "the rack panel: the weapon in the well",
            );
        }
    }
}

/// **A hundred ticks of the armoury leave the kingdom byte-identical.**
///
/// The room moves and the world does not. `Anim` is on
/// [`l2_game::game::LevyOrder`], lockstep digest
/// never sees — `docs/netcode.md` — and this is the assertion that keeps it
/// there. Ablated by writing anything at all into `ctx.game.kingdom` from
/// `Anim::tick`'s callers.
#[test]
fn a_hundred_ticks_of_the_armoury_leave_the_kingdom_byte_identical() {
    let (mut g, a) = world!();
    let county = own_county(&g);
    g.selected = county;
    g.open_levy(county);

    let mut m = Machine::new(ScreenId::Armoury(county));
    // Send a soldier off, so the tick has the most to do that it ever has.
    g.levy.anim.walker.latch(1, 0);
    g.levy.anim.walker.start(1, 1);

    let before = l2_kingdom::save::encode(&g.kingdom);
    for _ in 0..100 {
        let mut ctx = Ctx { game: &mut g, assets: &a };
        m.update(&mut ctx);
    }
    assert!(g.levy.anim.walker.x > armoury::WALKER_START_X, "the soldier moved");
    assert_eq!(before, l2_kingdom::save::encode(&g.kingdom), "and the world did not");
}

// ---------------------------------------------------------------------------

