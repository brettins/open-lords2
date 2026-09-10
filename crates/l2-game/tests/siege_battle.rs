//! **The siege battle, played.**
//!
//! ```text
//! cargo test -p l2-game --test siege_battle
//! ```
//!
//! `docs/plan.md` §2.3 warned about a one-way door — a battle the campaign can
//! reach and the player cannot watch — and it arrived in the place nobody was
//! looking. The field battle got a screen; **an assault landed in
//! `engagement::fight`, resolved headless, and handed the player a report.** So
//! did every siege in which the player was the *defender*, and that one was
//! worse than a missing screen: `question_for` gave a besieged human
//! `choice_owner = 2`, the prompt draws no widgets under 2, and the turn was
//! suspended with two armies on one tile and no way to answer. **A player
//! besieged by an AI could not end his turn.**
//!
//! Everything below goes through [`Machine::handle`] with an [`Event`]. Nothing
//! here calls `LiveBattle` directly, sets `castle_degraded` or
//! `besieging_county` by hand, or needs a copy of the game.
//!
//! > *"A rule with no way in is not a rule the game has."* — `docs/agents.md`
//!
//! The two things a test may place are the two a scenario places: the map, and
//! armies that already exist. One thing is placed that a scenario would not,
//! and it is called out where it happens — the besieger here is **staged**
//! rather than marched in, so nothing has run `Siege_Prepare` on it and its
//! catapult has to be given through `siege::order_engine`, which is the same
//! call the player's own siege screen makes.
//!
//! > **This paragraph used to say the AI never orders siege engines anywhere in
//! > the workspace.** That was a conclusion drawn from `order_engine`'s three
//! > callers all being the player's, and `order_engine` is the siege screen's
//! > `+` and `−` buttons — the original's AI does not press those either. The
//! > AI's path is `Siege_Prepare` (`0x004A7EB5`), reached from
//! > `Unit_ReachCastleBuilding` when an army *walks onto* the castle, and it
//! > has been implemented all along. `crates/l2-kingdom/tests/ai_siege.rs`
//! > travels that road; what was missing was a test that did, not the code.

use l2_game::game::Assets;
use l2_game::input::{Event, Key, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{battle, map};
use l2_game::Game;
use l2_kingdom::map::{flags, terrain, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

use l2_game::battlefield as bf;

/// As `tests/castles.rs` chooses it: inside the band the campaign screen opens
/// on, so a test can click a tile without scrolling.
const BORDER_1_2: usize = 32;

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

fn click(m: &mut Machine, g: &mut Game, a: &Assets, at: (i32, i32)) {
    send(m, g, a, Event::Click { x: at.0, y: at.1 });
}

fn press(m: &mut Machine, g: &mut Game, a: &Assets, c: char) {
    send(m, g, a, Event::KeyDown(Key::letter(c)));
}

fn on(r: Rect) -> (i32, i32) {
    (r.centre_x(), r.y + r.h / 2)
}

fn pixel(x: u8, y: u8) -> Option<(i32, i32)> {
    let probe = map::MapScreen::new();
    campaign::tile_centre(probe.viewport(), probe.zoom(), x as usize, y as usize)
}

fn run_until(
    m: &mut Machine,
    g: &mut Game,
    a: &Assets,
    what: &str,
    done: impl Fn(&Machine, &Game) -> bool,
) {
    for _ in 0..8_000 {
        if done(m, g) {
            return;
        }
        tick(m, g, a);
    }
    panic!("{what} never happened; the screen is {:?}", m.top_id());
}

/// Two counties, one each. **Realm 1 is the human and realm 2 the AI**, and
/// which of them is besieged is what each test below chooses.
fn world() -> (Game, Assets) {
    let mut g = Game::new(11);
    g.kingdom.set_county_count(2);
    for id in 1..=2usize {
        let c = &mut g.kingdom.counties[id];
        c.population = 4_000;
        c.happiness = 90;
        c.grain = 60_000;
        c.herd = 400;
        c.neighbour_count = 1;
        c.neighbours[0] = if id == 1 { 2 } else { 1 };
    }
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[2].owner = 2;
    for realm in 1..=2usize {
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 5;
        g.kingdom.realms[realm].wood = 5_000;
        g.kingdom.realms[realm].stone = 5_000;
    }
    g.kingdom.realms[1].is_human = true;
    // **Both lords are solvent, and the AI's has to be.** Starving the AI of
    // gold to stop it raising troops of its own is the obvious way to keep
    // these games quiet, and it does not work: a realm that cannot pay its
    // wages runs `Army_Desert` and then destroys armies outright
    // (`docs/kingdom.md` §7.4), so it also kills the besieger this file puts on
    // the map, two turns in, silently. Measured the hard way.
    for realm in 1..=2usize {
        g.kingdom.realms[realm].gold = 20_000;
    }

    let mut m = CampaignMap::empty();
    for i in 0..MAP_TILES {
        m.county[i] = if i % MAP_DIM < BORDER_1_2 { 1 } else { 2 };
    }
    g.kingdom.campaign.map = m;
    g.kingdom.campaign.mercenaries = MercenaryBands::init(2);
    g.player = 1;
    g.selected = 1;
    (g, Assets::placeholder())
}

fn on_the_map() -> (Game, Assets, Machine) {
    let (g, a) = world();
    (g, a, Machine::new(ScreenId::Campaign))
}

fn army_at(g: &mut Game, owner: u8, county: u8, men: i32, at: (u8, u8)) -> usize {
    let mut u = Unit::new(UnitKind::Army, owner, at.0, at.1);
    u.men = men;
    u.troops[TroopType::Peasant.index()] = men;
    u.county = county;
    u.home_county = county;
    u.owner_is_human = owner == 1;
    g.kingdom.campaign.units.spawn(u).expect("a free slot")
}

/// A tile whose whole 4×4 neighbourhood is inside the opening viewport and in
/// the given county — `tests/castles.rs`'s `visible_in`, which these tests need
/// for the same reason: the castle is a 2×2 block and an army has to stand
/// beside it.
fn visible_in(g: &Game, county: u8) -> (u8, u8) {
    for y in 2..60u8 {
        for x in 2..60u8 {
            let ok = (-1i32..=2).all(|dy| {
                (-1i32..=2).all(|dx| {
                    let (px, py) = ((x as i32 + dx) as u8, (y as i32 + dy) as u8);
                    g.kingdom.campaign.map.county_at(px, py) == county && pixel(px, py).is_some()
                })
            });
            if ok {
                return (x, y);
            }
        }
    }
    panic!("no visible 4x4 block in county {county}");
}

/// The 2×2 castle plot `County_FindCastleTile` stamps at load.
fn plot(g: &mut Game, county: u8, at: (u8, u8)) {
    for dy in 0..2u8 {
        for dx in 0..2u8 {
            let (x, y) = (at.0 + dx, at.1 + dy);
            g.kingdom.campaign.map.set_flags(x, y, flags::SETTLEMENT);
            g.kingdom.campaign.map.set_terrain(x, y, terrain::CASTLE_PLOT);
            g.kingdom.campaign.map.set_county(x, y, county);
        }
    }
}

/// March a besieger onto a castle and **watch it walk there**, without ending
/// the turn.
///
/// `Units_Tick` runs on every tick the game is up rather than inside a phase —
/// `docs/decisions.md` C35, and the campaign screen's `update` is what calls it
/// — so an ordered army walks while the player sits on the map. That matters
/// here for a reason beyond fidelity: **ending a turn to move an army also runs
/// phase 2**, so a level-2 castle would be assaulted on the same turn the siege
/// was laid, and the setup would resolve the very battle the test is about to
/// watch.
///
/// `Unit_ReachCastleBuilding` (`0x004686A0`) is what lays the siege — the code-6
/// branch of the stepper, on terrain `0x15 … 0x19`.
fn lay_siege(m: &mut Machine, g: &mut Game, a: &Assets, besieger: usize, keep: (u8, u8), county: u8) {
    march(g, besieger, keep);
    run_until(m, g, a, "the siege", |_, g| {
        g.kingdom.campaign.units.get(besieger).is_some_and(|u| u.besieging_county == county)
    });
}

/// March a unit one tile, through `l2-kingdom` rather than through two clicks.
///
/// `tests/military.rs` sets the precedent and gives the reason: the map's
/// second click on an *enemy* army is a selection rather than an attack order,
/// and what is under test here is the assault rather than the route into it.
/// The player's own march in [`the_player_besieges_and_watches_the_assault`] is
/// still two clicks, because there it *is* the route.
fn march(g: &mut Game, unit: usize, to: (u8, u8)) {
    let map = g.kingdom.campaign.map.clone();
    l2_kingdom::movement::order_move(
        &map,
        &mut g.kingdom.campaign.units,
        unit,
        to,
        l2_kingdom::movement::Routing::Direct,
    )
    .expect("a path one tile long");
}

/// A castle of `castle_type` in `county`, manned by an army that marched in.
/// Returns the plot's north-west tile and the garrison.
fn castle_with_garrison(
    g: &mut Game,
    m: &mut Machine,
    a: &Assets,
    county: u8,
    castle_type: u8,
    men: i32,
) -> ((u8, u8), usize) {
    let keep = visible_in(g, county);
    plot(g, county, keep);
    g.kingdom.counties[county as usize].castle_type = castle_type;
    l2_kingdom::map::stamp_castle_terrain(&mut g.kingdom.campaign.map, county, castle_type);

    let owner = g.kingdom.counties[county as usize].owner;
    let garrison = army_at(g, owner, county, men, (keep.0 - 1, keep.1));
    march(g, garrison, keep);
    // Walked in on plain frames rather than by ending a turn — see
    // [`lay_siege`]. **The men have to fit**: `CASTLE_GARRISON_CAP` is
    // `[150, 200, 200, 400, 600]` indexed by `castle_type - 1`, and over it
    // `Army_GarrisonApply` does nothing at all — the army simply stops outside
    // and the castle stays empty, with no message a test would notice.
    run_until(m, g, a, "the garrison walking in", |_, g| {
        g.kingdom.counties[county as usize].garrison_unit == garrison
    });
    (keep, garrison)
}

/// End turns until phase 2 launches the assault and screen `0x12` is up.
///
/// **A siege is not assaulted the turn it is laid**, and for an AI besieger it
/// is not assaulted the turn after either. `Army_BeginSiege` runs
/// `Siege_Prepare`, which hands an **AI** army the siege-engine defaults its
/// lord's `siege_doctrine` calls for — a human's is left empty for him to fill
/// in on the siege screen — and `Siege_RecomputeBuildTime` then charges
/// `ceil(work / men)` seasons for them. So an AI army of 300 waits two seasons
/// where one of 600 waits one, and the player's own besieger, which ordered
/// nothing, storms on the first phase 2 it reaches.
fn end_turns_until_the_assault(m: &mut Machine, g: &mut Game, a: &Assets) {
    for _ in 0..12 {
        let before = g.kingdom.turn_count;
        press(m, g, a, 'e');
        for _ in 0..8_000 {
            if m.top_id() == Some(ScreenId::BattlePrompt) {
                return;
            }
            if g.kingdom.turn_count > before {
                break;
            }
            tick(m, g, a);
        }
        for _ in 0..=l2_view::fade::PHASES {
            tick(m, g, a);
        }
    }
    panic!("the assault prompt never came; the screen is {:?}", m.top_id());
}

/// **Take the field and unpause.** The two gestures every battle below opens
/// with: the thumb on screen `0x12`, and the pause button — *"the first thing a
/// player does in every battle is press pause, to unpause"*.
fn take_the_field(m: &mut Machine, g: &mut Game, a: &Assets) {
    assert_eq!(m.top_id(), Some(ScreenId::BattlePrompt), "a prompt should be up");
    click(m, g, a, on(battle::widget_rect(battle::TAKE_THE_FIELD)));
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "the field, not the result");
    assert!(g.battle.as_ref().expect("a live battle").paused, "a battle starts paused");
    click(m, g, a, on(bf::Button::Pause.rect()));
    assert!(!g.battle.as_ref().expect("a live battle").paused);
}

/// Run the battle out and skip the banner, which is a right release on `0x2B`.
fn watch_to_the_end(m: &mut Machine, g: &mut Game, a: &Assets, ticks: u32) {
    let mut n = 0;
    while n < ticks && g.battle.as_ref().is_some_and(|b| b.conclusion.is_none()) {
        tick(m, g, a);
        n += 1;
        // A siege that will not end is the failure worth diagnosing, so the
        // state that decides `Battle_CheckOutcome`'s five arms is printed
        // rather than left to be guessed at from a bare timeout.
        if n % 50_000 == 0 {
            if let Some(b) = g.battle.as_ref() {
                eprintln!(
                    "  t={n} besieger={} garrison={} approach={} breach={} ramparts={} \
                     gate={} ditch={}",
                    b.runner.men_of_side(l2_sim::SIDE_B),
                    b.runner.men_of_side(l2_sim::SIDE_A),
                    b.runner.ai.approach_score,
                    b.runner.ai.breach_score,
                    b.runner.siege.ramparts_breached,
                    b.runner.siege.gate_breached,
                    b.runner.siege.moat_filled,
                );
            }
        }
    }
    assert!(
        g.battle.as_ref().is_some_and(|b| b.conclusion.is_some()),
        "the siege did not reach a conclusion in {ticks} frames",
    );
    eprintln!("the siege was decided after {n} frames");
    assert_eq!(g.battle.as_ref().map(|b| b.screen_id()), Some(0x2B), "the banner");
    send(m, g, a, Event::RightClick { x: 100, y: 300 });
    tick(m, g, a);
    assert_eq!(m.top_id(), Some(ScreenId::BattleResult), "the result screen follows");
}

/// The pixel of a battlefield cell in the **overview panel**, which is 160 × 160
/// at (480, 24) and two pixels a cell — `BattleMap_Click` (`0x00432443`).
///
/// A left click there orders the selection to that cell at battlefield scale; a
/// right click looks there. Both are how a player reaches a corner of an 80 × 80
/// field from a 15 × 14 viewport without scrolling to it.
fn overview_px(cell: (u8, u8)) -> (i32, i32) {
    (bf::OVERVIEW.x + cell.0 as i32 * 2, bf::OVERVIEW.y + cell.1 as i32 * 2)
}

/// **Wait for the gate, then send everybody in and charge.**
///
/// Two played gestures — a left click on the overview panel, which is
/// `BattleMap_Click`'s order arm at battlefield scale, and the fourth button —
/// and a besieging player makes both. A formation ordered at the wall stops at
/// the wall; somebody has to tell it to go through the hole it has just made,
/// and `FUN_0047A76D` is what turns a formation into a mob hunting the last of
/// a garrison down.
fn press_the_assault_home(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut n = 0;
    while n < 200_000 {
        let through = g
            .battle
            .as_ref()
            .is_none_or(|b| b.conclusion.is_some() || b.runner.siege.gate_breached);
        if through {
            break;
        }
        tick(m, g, a);
        n += 1;
    }
    eprintln!("the wall was open after {n} frames");
    let target = overview_px(keep_centre(g));
    click(m, g, a, target);
    click(m, g, a, on(bf::Button::Charge.rect()));
    assert!(g.battle.as_ref().is_none_or(|b| b.charged), "DAT_0055322C, the charge latch");
}

/// The cell the way in stands on — flag `0x08`, which is the middle of the
/// bailey in [`l2_sim::siege::our_castle`]. Read off the field rather than
/// written down, because the layout is ours and may change.
fn keep_centre(g: &Game) -> (u8, u8) {
    let live = g.battle.as_ref().expect("a live battle");
    let dim = l2_sim::terrain::DIM;
    live.runner
        .field
        .cells
        .iter()
        .position(|c| c.flags & l2_sim::siege::FLAG_KEEP != 0)
        .map(|i| ((i % dim) as u8, (i / dim) as u8))
        .expect("every castle has one way in")
}

/// **Select every one of your men on screen**, with the gesture that does it: a
/// press on the field, a pointer motion past the 25-pixel slop, and a release.
///
/// `FUN_0043BF07` in three events — `press_field`, `drag_to`, `release_field` —
/// and the box is the whole viewport, so what it picks is whatever the camera is
/// looking at.
fn box_select_the_viewport(m: &mut Machine, g: &mut Game, a: &Assets) {
    let v = bf::VIEW;
    click(m, g, a, (v.x + 2, v.y + 2));
    send(m, g, a, Event::Pointer { x: v.x + v.w - 2, y: v.y + v.h - 2 });
    send(m, g, a, Event::Release { x: v.x + v.w - 2, y: v.y + v.h - 2 });
}


// 1. Being besieged — the prompt that had no way out
// ---------------------------------------------------------------------------

/// **A player besieged by an AI can answer the prompt.**
///
/// `Battle_ChooseSettlement` (`0x004A6A30`) is two `if`s and the second
/// overrides the first:
///
/// ```c
/// if (units[armyA].owner == localPlayer) choiceOwner = 1;
/// if (units[armyB].owner == localPlayer) choiceOwner = units[armyA].ownerIsHuman ? 2 : 1;
/// ```
///
/// so a human defender attacked by an AI holds the choice himself, and only two
/// humans put it in the other man's hands. Ours tested the *defender's*
/// `ownerIsHuman` where the original tests the attacker's, handed the defender
/// a 2, and `BattlePromptScreen` draws no widgets under 2 — because
/// `Battle_ChooseSettlement` writes `DAT_00554408 = 2` only under 1. In the
/// original a bystander's prompt waits for the multiplayer answer timeout;
/// single player has no such timeout.
///
/// So the assertion that matters is the last one: **the turn finishes.**
#[test]
fn a_human_besieged_by_an_ai_holds_the_choice_and_the_turn_can_end() {
    let (mut g, a, mut m) = on_the_map();
    let (keep, _garrison) = castle_with_garrison(&mut g, &mut m, &a, 1, 2, 200);

    let camp = (keep.0 - 1, keep.1 + 1);
    let besieger = army_at(&mut g, 2, 2, 600, camp);
    lay_siege(&mut m, &mut g, &a, besieger, keep, 1);
    end_turns_until_the_assault(&mut m, &mut g, &a);

    let q = l2_game::turn::pending_question(&g).expect("a question");
    assert!(q.is_siege);
    assert_eq!(
        q.choice_owner, 1,
        "an AI attacker leaves the choice with the human defender -- and without \
         this the prompt has no widgets and the turn never ends",
    );

    take_the_field(&mut m, &mut g, &a);
    assert_eq!(g.battle.as_ref().map(|b| b.owner), Some(1));
    // The battle really is up, really is a siege, and really is being stepped.
    for _ in 0..500 {
        tick(&mut m, &mut g, &a);
    }
    {
        let live = g.battle.as_ref().expect("a live battle");
        assert!(live.is_siege());
        assert!(live.runner.tick > 0, "the frames are reaching the simulation");
    }

    // **Then the garrison gives the castle up**, which is the second button:
    // `FUN_0043BA29` opens `Ui_OpenConfirm(11)` — *"Surrender castle?"* —
    // rather than the field battle's *"Retreat from field?"*, and both
    // callbacks reach `FUN_0043BE65`.
    assert_eq!(
        g.battle.as_mut().expect("a live battle").press_retreat(),
        Some(11),
        "a garrison surrenders its castle; it does not retreat from a field",
    );
    click(&mut m, &mut g, &a, on(bf::Button::Retreat.rect()));
    click(&mut m, &mut g, &a, on(l2_game::screens::battlefield::CONFIRM_YES));
    run_until(&mut m, &mut g, &a, "the result screen", |m, _| {
        m.top_id() == Some(ScreenId::BattleResult)
    });
    click(&mut m, &mut g, &a, on(battle::ok_rect()));
    run_until(&mut m, &mut g, &a, "the rest of the turn", |_, g| {
        !l2_game::turn::turn_in_flight(g)
    });
    assert!(
        matches!(m.top_id(), Some(ScreenId::Campaign) | Some(ScreenId::Conquest)),
        "and the campaign is back: {:?}",
        m.top_id(),
    );
}

// ---------------------------------------------------------------------------
// 2. The drawbridge
// ---------------------------------------------------------------------------

/// **The garrison lowers its drawbridge**, from the battlefield's third button.
///
/// `FUN_0043BBE7` → `FUN_00496B9F`. The four guards are the button's and were
/// already built; the routine underneath was a once-per-battle latch that did
/// nothing at all. Now it lays the 7 × 4 patch, opens the gate, moves both siege
/// scores and rebuilds the pathfinding, and the way out of the castle it leaves
/// behind is a thing men can walk through.
///
/// A **stone** castle, because the shipped `Readme.txt` says *"only the Stone
/// and Royal castles have drawbridges"* and the button's third guard is
/// `g_castleLevel < 3`.
///
/// The battle is ended with the **autocalc button** rather than fought out, and
/// that is a second assertion rather than a shortcut: `FUN_0043BE65` leaves for
/// the report without passing through the outcome banner's frame counter, which
/// is `Siege_RecordCastleDamage`'s only caller — so giving up on a battle
/// un-does the damage exactly as it un-does the casualties.
#[test]
fn the_garrison_lowers_the_drawbridge_and_the_besieger_sees_the_gate_open() {
    let (mut g, a, mut m) = on_the_map();
    let (keep, _garrison) = castle_with_garrison(&mut g, &mut m, &a, 1, 4, 400);

    let camp = (keep.0 - 1, keep.1 + 1);
    let besieger = army_at(&mut g, 2, 2, 600, camp);
    lay_siege(&mut m, &mut g, &a, besieger, keep, 1);
    end_turns_until_the_assault(&mut m, &mut g, &a);
    assert_eq!(
        l2_game::turn::pending_question(&g).map(|q| q.castle_level),
        Some(Some(3)),
        "a stone castle",
    );

    take_the_field(&mut m, &mut g, &a);
    {
        let live = g.battle.as_ref().expect("a live battle");
        assert!(live.runner.has_drawbridge(), "a stone castle has one");
        assert!(!live.runner.siege.gate_breached, "and it is shut");
        assert!(!live.sallied);
    }

    // Button 2. `press_sally`'s guards want the local player to be the
    // garrison, which he is: army B is the garrison.
    click(&mut m, &mut g, &a, on(bf::Button::Sally.rect()));
    {
        let live = g.battle.as_ref().expect("a live battle");
        assert!(live.sallied, "the latch, DAT_0052AF9C");
        assert!(live.runner.siege.drawbridge_down);
        assert!(
            live.runner.siege.gate_breached,
            "_DAT_00569588 -- the besieger AI now reads an open gate, which is \
             what lowering your own drawbridge costs you",
        );
        assert!(
            !live.runner.has_drawbridge(),
            "every cell of the patch had its 0x40 cleared, so the way is open",
        );
        let walkable = live
            .runner
            .field
            .cells
            .iter()
            .filter(|c| c.surface == l2_sim::siege::SURFACE_GROUND && !c.impassable())
            .count();
        assert!(walkable > 0, "and what is left is ground men can stand on");
    }
    // A second press is refused with `L2.eng` 157, *"Drawbridge is down."*
    assert_eq!(g.battle.as_mut().expect("a live battle").press_sally(true), Err(0x9D));

    // Give up on it: the fifth button, then Yes.
    click(&mut m, &mut g, &a, on(bf::Button::Autocalc.rect()));
    click(&mut m, &mut g, &a, on(l2_game::screens::battlefield::CONFIRM_YES));
    run_until(&mut m, &mut g, &a, "the result screen", |m, _| {
        m.top_id() == Some(ScreenId::BattleResult)
    });
    let report = l2_game::turn::pending_report(&g).expect("a settled battle");
    assert_eq!(report.resolution, l2_game::engagement::Resolution::Autocalc);
    assert_eq!(
        g.kingdom.counties[1].castle_degraded, 0,
        "a battle nobody watched to its end bills no repair, however much of it \
         was fought first -- Siege_RecordCastleDamage has one caller and it is \
         the outcome banner frame counter",
    );
}

// ---------------------------------------------------------------------------
// 3. Besieging one: the ditch, the breach and the bill
// ---------------------------------------------------------------------------

/// **The whole route, played: besiege, watch, dig, break in, and be billed.**
///
/// This is the one-way door, closed. Before it, `Answer::TakeTheField` on an
/// assault ran the whole battle between two statements and handed back a
/// report; the thumb now raises the battlefield, the battle is *stepped by
/// frames*, and the banner comes up at the end of it.
///
/// `Siege_RecordCastleDamage` (`0x004784CA`) is the only writer of
/// [`l2_kingdom::siege::CASTLE_DEGRADED_DAMAGED`] in the binary, and its three
/// readers were reachable only from their own tests until this route existed.
/// The damage is the **ditch**, filled in by hand, in three played gestures a
/// player would use to reach a corner of an 80 × 80 field from a 15 × 14
/// viewport:
///
/// 1. a **right click on the overview panel** to look at his own army;
/// 2. a **box drag** across the viewport to pick it up;
/// 3. a **left click on the overview panel** on a cell of the ditch, which is
///    `BattleMap_Click`'s order arm at battlefield scale.
///
/// `Formation_RectIsClear` then caches *"this unit was ordered onto water"*,
/// every figure enters state 9, and `FUN_0047DD86` counts the cells they fill.
/// A Norman keep — type 3, level 2 — because that is the smallest castle
/// [`l2_sim::siege::our_castle`] gives a ditch, and it still needs no engines.
#[test]
fn the_player_besieges_watches_fills_the_ditch_and_the_castle_is_billed_for_it() {
    let (mut g, a, mut m) = on_the_map();
    let (keep, garrison) = castle_with_garrison(&mut g, &mut m, &a, 2, 3, 200);

    // The player's army marches onto the castle. Two clicks, because here the
    // march *is* the route: it is `Army_BeginSiege`.
    let camp = (keep.0 - 1, keep.1 + 1);
    let besieger = army_at(&mut g, 1, 1, 120, camp);
    click(&mut m, &mut g, &a, pixel(camp.0, camp.1).unwrap());
    click(&mut m, &mut g, &a, pixel(keep.0, keep.1).unwrap());
    run_until(&mut m, &mut g, &a, "the siege", |_, g| {
        g.kingdom.campaign.units.get(besieger).is_some_and(|u| u.besieging_county == 2)
    });

    end_turns_until_the_assault(&mut m, &mut g, &a);
    let q = l2_game::turn::pending_question(&g).expect("a question");
    assert!(q.is_siege, "the prompt knows it is a siege");
    assert_eq!(q.castle_level, Some(2), "and which castle it is");
    assert_eq!(q.choice_owner, 1, "the besieger is the human, so he chooses");

    take_the_field(&mut m, &mut g, &a);
    // The castle really is on the field, and the siege tables are what is being
    // dispatched.
    {
        let live = g.battle.as_ref().expect("a live battle");
        assert!(live.is_siege());
        assert_eq!(live.castle_level, Some(2));
        assert!(live.runner.siege.is_siege);
        assert!(
            live.runner.field.cells.iter().any(|c| c.flags & l2_sim::siege::FLAG_WALL != 0),
            "there is a wall to fight at",
        );
    }

    // Where the besieger men are, and where the nearest ditch cell is. Both
    // read off the field the battle is on rather than assumed, because the
    // castle layout is ours and may change.
    let (home, ditch) = {
        let live = g.battle.as_ref().expect("a live battle");
        let home = live.runner.home(l2_sim::SIDE_B);
        let dim = l2_sim::terrain::DIM;
        let ditch = live
            .runner
            .field
            .cells
            .iter()
            .enumerate()
            .filter(|(_, c)| c.surface == l2_sim::siege::SURFACE_WATER)
            .map(|(i, _)| ((i % dim) as u8, (i / dim) as u8))
            .min_by_key(|&(x, y)| {
                (x as i32 - home.0 as i32).abs() + (y as i32 - home.1 as i32).abs()
            })
            .expect("a Norman keep has a ditch");
        (home, ditch)
    };

    // 1. Look at the army. 2. Box it. 3. Order it into the ditch.
    let look = overview_px(home);
    send(&mut m, &mut g, &a, Event::RightClick { x: look.0, y: look.1 });
    box_select_the_viewport(&mut m, &mut g, &a);
    assert!(
        g.battle.as_ref().is_some_and(|b| b.runner.selected_count(1) > 0),
        "the box picked nobody up",
    );
    click(&mut m, &mut g, &a, overview_px(ditch));

    let mut n = 0;
    while n < 40_000 {
        let done = g
            .battle
            .as_ref()
            .is_none_or(|b| b.conclusion.is_some() || b.runner.siege.moat_filled > 0);
        if done {
            break;
        }
        tick(&mut m, &mut g, &a);
        n += 1;
    }
    let filled = g.battle.as_ref().map(|b| b.runner.siege.moat_filled).unwrap_or(0);
    assert!(filled > 0, "the ditch was never filled in {n} frames");
    eprintln!("the ditch started going in after {n} frames");

    // Then the assault: wait for the wall to open, send them through it, and
    // charge.
    press_the_assault_home(&mut m, &mut g, &a);
    watch_to_the_end(&mut m, &mut g, &a, 400_000);

    let report = l2_game::turn::pending_report(&g).expect("a settled battle").clone();
    assert!(report.is_siege);
    assert!(
        matches!(report.resolution, l2_game::engagement::Resolution::Fought { .. }),
        "it was fought rather than calculated: {:?}",
        report.resolution,
    );
    assert!(report.castle_damage.any(), "the accumulators reached the report");
    let moat = report.castle_damage.moat_filled as i32;
    let wall = report.castle_damage.wall_damage as i32;
    assert!(moat > 0);

    // **The bill.** Work at five man-seasons a ditch cell and fifteen a wall
    // cell; wood or stone from the *wall* damage alone, in the material the
    // castle is made of — `docs/bugs.md` B69.
    let c = &g.kingdom.counties[2];
    assert_eq!(
        c.castle_degraded,
        l2_kingdom::siege::CASTLE_DEGRADED_DAMAGED,
        "the value three readers could not be reached through",
    );
    assert_eq!(c.castle_level_left, 2, "a Norman keep is what is left standing");
    assert_eq!(c.castle_percent, 0, "and the map tile goes back to scaffolding");
    assert_eq!(
        c.castle_work_left,
        moat * l2_kingdom::siege::REPAIR_WORK_PER_MOAT
            + wall * l2_kingdom::siege::REPAIR_WORK_PER_WALL,
    );
    assert_eq!(c.castle_work_total, c.castle_work_left);
    assert_eq!(
        c.castle_stone_owed,
        wall * l2_kingdom::siege::REPAIR_STONE_PER_WALL,
        "a Norman keep is level 2, so it is repaired in stone",
    );
    assert_eq!(c.castle_wood_owed, 0, "and never in wood");
    // The scars are stored, so the next assault on the same castle picks them
    // up — `FUN_004787A4`, which is what makes them state rather than a report.
    assert_eq!(c.siege_scars.moat_filled as i32, moat);
    assert_eq!(c.siege_scars.wall_damage as i32, wall);

    // And the campaign takes it from there: the siege link is gone on both
    // sides whichever way the assault went.
    click(&mut m, &mut g, &a, on(battle::ok_rect()));
    run_until(&mut m, &mut g, &a, "the rest of the turn", |_, g| {
        !l2_game::turn::turn_in_flight(g)
    });
    for id in [besieger, garrison] {
        if let Some(u) = g.kingdom.campaign.units.get(id) {
            assert_eq!(u.besieging_county, 0);
            assert_eq!(u.besieged_by, 0);
        }
    }
}
