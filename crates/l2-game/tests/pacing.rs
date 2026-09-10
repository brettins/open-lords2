//! **How fast the campaign map moves**, measured through the real screen
//! machine rather than through the turn machine on its own.
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" LORDS2_FIXTURES="E:\dev\lords2-fixtures" \
//!     cargo test -p l2-game --test pacing
//! ```
//!
//! # Why this file exists beside `tests/scenario.rs`'s merchant walk
//!
//! `a_merchant_walks_its_route_over_the_frames_of_a_turn_rather_than_teleporting`
//! asserts three things: a turn takes many ticks, a merchant stands on several
//! distinct tiles across them, and it never enters more than one tile in a
//! tick. All three are true of a merchant that stands perfectly still for
//! thirty-four frames and then crosses eleven tiles in eleven — which is
//! exactly what a player reported:
//!
//! > *"The merchants don't move right when you click End Turn, and then… move
//! > insanely fast. That's a bit odd."*
//!
//! It is `docs/agents.md`'s *test that passes for an accidental reason*, and
//! the accident is that it is written in **tiles per tick**. Tiles per tick was
//! never wrong: `Unit_Step` enters at most one tile per call and always has.
//! What was missing is the other half of `Unit_StepOnce` (`0x0046634D`) — the
//! sub-tile counter that decides *how many ticks a tile takes* — so the unit
//! of measure that catches it is **ticks per tile**, and the unit that catches
//! the frame driver getting it wrong instead is **ticks per frame**.
//!
//! Both are here, and neither can be satisfied by the other.
//!
//! # The numbers are typed, not computed
//!
//! `SUBTILE_SPAN / SUBTILE_STEP_SOLO = 8` admissions a tile, and a road admits
//! one tick in one. A test that wrote that expression would pass with every
//! constant in it ablated to 1 — `docs/agents.md`, *ablating a constant while
//! computing your probe from that same constant tests nothing at all* — so the
//! 8 below is a literal, and this is where it comes from:
//!
//! ```c
//! /* Unit_StepOnce, 0x0046634D, the arm that does not enter a tile */
//! cVar1 = onRoad ? 0 : 3;
//! if (cVar1 < ++field_0x14a) {
//!     field_0x14a = 0;
//!     field_0x149 += (g_multiplayer == 0) ? 2 : 4;
//!     if (field_0x149 >= 0x10) { field_0x14b |= 1; field_0x149 = 0; return 2; }
//! }
//! return 1;
//! ```
//!
//! Sixteen in twos is eight admissions; a road admits every tick and open
//! ground one in four. **8 ticks a road tile, 32 an open one.** `[V]`

use l2_game::audio::{self, Audio};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::scenario;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_kingdom::tables::Tables;
use l2_kingdom::UnitKind;

/// The fewest ticks any unit can take to enter a tile: eight admissions of the
/// sub-tile counter, one admitted per tick because the unit is on a road.
/// Typed; see the module note.
const FEWEST_TICKS_PER_TILE: u32 = 8;

/// How long a turn is allowed to run before this file gives up on it. Well
/// under `turn::MAX_TICKS` and well over the England turn's real length.
const GIVE_UP: u32 = 1_500;

macro_rules! england {
    () => {{
        let save = l2_testkit::england!();
        scenario::from_save(&save, Tables::DEFAULT).expect("the fixture loads")
    }};
}

fn merchant_slots(game: &l2_game::Game) -> Vec<usize> {
    game.kingdom
        .campaign
        .units
        .iter()
        .filter(|(_, u)| u.kind == UnitKind::Merchant)
        .map(|(id, _)| id)
        .collect()
}

/// Where every merchant stood at the end of each **frame** of a turn started by
/// pressing End Turn on the campaign map, plus how many frames it took.
///
/// The machine is driven exactly as `main.rs` drives it: one
/// [`Machine::update`] a frame, and no other door into the simulation.
fn watch_a_turn(game: &mut l2_game::Game, machine: &mut Machine) -> Vec<Vec<(u8, u8)>> {
    let assets = Assets::placeholder();
    let slots = merchant_slots(game);
    let before = game.kingdom.turn_count;

    {
        let mut ctx = Ctx { game, assets: &assets };
        machine.handle(Event::KeyDown(Key::letter('e')), &mut ctx);
    }
    assert!(l2_game::turn::turn_in_flight(game), "End Turn started a turn");

    let mut trail: Vec<Vec<(u8, u8)>> = vec![Vec::new(); slots.len()];
    for _ in 0..GIVE_UP {
        {
            let mut ctx = Ctx { game, assets: &assets };
            machine.update(&mut ctx);
        }
        assert_eq!(
            machine.top_id(),
            Some(ScreenId::Campaign),
            "nobody is at war on turn one, so nothing may come up over the map",
        );
        for (i, &slot) in slots.iter().enumerate() {
            let u = game.kingdom.campaign.units.get(slot).expect("a merchant is never lost");
            trail[i].push(u.tile());
        }
        if game.kingdom.turn_count > before {
            return trail;
        }
    }
    panic!("the turn never came round in {GIVE_UP} frames");
}

/// The frames on which this unit entered a tile.
fn entries(path: &[(u8, u8)]) -> Vec<u32> {
    path.windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] != w[1])
        .map(|(i, _)| i as u32 + 1)
        .collect()
}

/// **A merchant crosses a tile in at least eight frames, not one.**
///
/// This is the assertion the player's report makes, and the one the existing
/// merchant-walk test cannot make: a stall followed by a sprint satisfies
/// *distinct tiles* and *at most one tile a tick* perfectly.
///
/// **Ablated**: making `l2_kingdom::units_tick`'s `cross_sub_tile` return
/// `true` unconditionally — which is the behaviour every build before it had —
/// turns every gap here into 1 and this red on the first merchant.
#[test]
fn a_merchant_takes_at_least_eight_frames_to_cross_one_tile() {
    let mut game = england!();
    let mut machine = Machine::new(ScreenId::Campaign);
    let slots = merchant_slots(&game);
    let trail = watch_a_turn(&mut game, &mut machine);

    let mut measured = 0;
    for (i, &slot) in slots.iter().enumerate() {
        let on = entries(&trail[i]);
        for pair in on.windows(2) {
            let gap = pair[1] - pair[0];
            assert!(
                gap >= FEWEST_TICKS_PER_TILE,
                "merchant {slot} entered a tile on frame {} and the next on frame {} — \
                 {gap} frames apart, and the fewest `Unit_StepOnce` allows is \
                 {FEWEST_TICKS_PER_TILE}",
                pair[0],
                pair[1],
            );
            measured += 1;
        }
    }
    assert!(
        measured >= 6,
        "only {measured} tile-to-tile gaps were measured across six merchants, \
         which is too few to have tested anything",
    );
}

/// **A turn cannot be shorter than the slowest march inside it.**
///
/// The other half of the report — *"they don't move right when you click End
/// Turn"* — and the half that is easy to blame on the wrong thing. Merchants
/// are ordered by phase 6 (`Merchant_AdvanceAll`) and nothing before it moves
/// them, so a run of frames in which no merchant walks is **the original's
/// design and not the defect**. What made it read as a stall is that the
/// walking which followed was over in eleven frames, so the still part was
/// three quarters of the whole turn.
///
/// So the property to hold is the one that was actually violated: a phase that
/// waits for a class of unit to stop walking cannot finish before that class
/// has walked, so **the turn is at least as long as its longest single leg**.
/// A merchant on the England position walks ten tiles, which is eighty ticks;
/// the turn was forty-seven.
///
/// It says nothing about *where* in the turn the walking happens, on purpose:
/// that is phase order, it is the original's, and asserting it here would be
/// asserting a preference.
///
/// **Ablated**: making `cross_sub_tile` return `true` unconditionally gives a
/// 47-frame turn containing an 11-tile leg and this goes red.
#[test]
fn a_turn_lasts_at_least_as_long_as_the_longest_march_in_it() {
    let mut game = england!();
    let mut machine = Machine::new(ScreenId::Campaign);
    let slots = merchant_slots(&game);
    let trail = watch_a_turn(&mut game, &mut machine);
    let frames = trail[0].len() as u32;

    let (longest, walker) = slots
        .iter()
        .enumerate()
        .map(|(i, &slot)| (entries(&trail[i]).len() as u32, slot))
        .max()
        .expect("six merchants");
    assert!(longest > 2, "no merchant walked far enough to time: {longest} tiles");

    let floor = FEWEST_TICKS_PER_TILE * (longest - 1);
    eprintln!("the turn took {frames} frames; merchant {walker} walked {longest} tiles");
    assert!(
        frames >= floor,
        "the turn was {frames} frames and merchant {walker} crossed {longest} tiles \
         inside it — {floor} frames is the least that can take, so the tiles were \
         being entered faster than `Unit_StepOnce` enters them",
    );
}

/// **One `Turn_Tick` a frame, and never two.**
///
/// The unit of measure that would catch the *other* explanation of a stall
/// followed by a sprint: a turn whose ticks are deferred while something else
/// holds the frame and then flushed several to a frame to catch up. It is not
/// what is happening — this passes today and passed before the sub-tile
/// counter landed — and it is here so that it keeps being not what is
/// happening.
///
/// The count comes from the invariant `tests/scenario.rs` already rests on:
/// the same turn run stepped and run all at once takes the same number of
/// `Turn_Tick`s. So the headless door's [`l2_game::turn::TurnOutcome::ticks`]
/// is the tick count, the frames are counted here, and the two must differ by
/// exactly one — [`l2_game::turn::begin_turn`] runs the first tick inside the
/// keystroke, before any frame.
///
/// **Ablated**: returning `TurnStep::Running` from `advance` only every other
/// tick — two ticks to a frame — halves the frame count and this goes red.
#[test]
fn a_frame_of_a_turn_is_exactly_one_turn_tick() {
    let mut stepped = england!();
    let mut machine = Machine::new(ScreenId::Campaign);
    let trail = watch_a_turn(&mut stepped, &mut machine);
    let frames = trail[0].len() as u32;

    let mut at_once = england!();
    let outcome = l2_game::turn::end_turn(&mut at_once).expect("the machine comes round");

    assert_eq!(
        frames,
        outcome.ticks - 1,
        "the turn took {} ticks and {frames} frames; a frame runs one tick, and \
         the odd one out is the tick `begin_turn` runs inside the keystroke",
        outcome.ticks,
    );

    // And the load-bearing half of C59/C60 restated at this door rather than at
    // the turn machine's: spreading a turn over frames is a display change.
    for (id, u) in stepped.kingdom.campaign.units.iter() {
        let other = at_once.kingdom.campaign.units.get(id).map(|o| o.tile());
        assert_eq!(Some(u.tile()), other, "unit {id} ended the turn somewhere else");
    }
    assert_eq!(stepped.gold(), at_once.gold());
    assert_eq!(stepped.kingdom.season, at_once.kingdom.season);
}

/// **The pacing, heard.** One hoofbeat per tile per unit —
/// `Unit_MoveInFacing` (`0x00466D84`), which plays a sound as its first
/// statement after unlinking the unit from the tile it is leaving.
///
/// It is the same number as the two tests above, which is the point of putting
/// it in this file: a player judging the tempo by ear and this file measuring
/// it by frame have to be measuring one thing. Before the sub-tile counter
/// landed, six merchants crossing eleven tiles in eleven frames would have
/// been a 180 ms gallop at the end of every turn.
///
/// What is asserted is that the *call site exists and is reached from a played
/// turn* — `merchant.wav` opened by the audio layer during an End Turn that
/// nothing but merchants walked in. How loud it is and how often the mixer
/// drops it are `Mixer`'s own tests.
///
/// **Ablated**: removing the `hear_the_march` call from `Director::listen`
/// leaves `merchant.wav` unheard and this goes red.
#[test]
fn a_merchant_crossing_a_tile_is_audible() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so no .wav files to open");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::load(&platform.vfs).expect("assets load");
    let mut game = england!();
    let mut machine = Machine::new(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();

    {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.handle(Event::KeyDown(Key::letter('e')), &mut ctx);
    }
    let before = game.kingdom.turn_count;
    for _ in 0..GIVE_UP {
        {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            machine.update(&mut ctx);
        }
        // `App::tick`'s own order: the simulation tick, then the listen.
        director.listen(&mut audio, &machine, &game);
        if game.kingdom.turn_count > before {
            break;
        }
    }

    let heard = audio.heard();
    assert!(
        heard.contains(&"merchant.wav"),
        "a turn in which six merchants walked ten tiles each opened {heard:?} and \
         not merchant.wav — `Unit_MoveInFacing`'s call site is not reached",
    );
}
