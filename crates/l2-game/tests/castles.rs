//! **The castle route, played rather than tested.**
//!
//! ```text
//! cargo test -p l2-game --test castles
//! ```
//!
//! Every line below goes through [`Machine::handle`] with an [`Event`]. Nothing
//! here sets `castle_degraded`, `garrison_unit` or `besieging_county` by hand,
//! and that is the whole point: those three fields were **read by rules and
//! written by nothing a player could reach**, which is why a county could never
//! build a castle, a castle could never be manned and a siege could only ever
//! be laid by a test that laid it itself.
//!
//! > *"A field is only tested if something a test reads was written by
//! > something the game runs. A test that populates the state it then asserts
//! > on is checking its own fixture."* — `docs/agents.md`
//!
//! So the route is: **order a castle from the sidebar, watch it go up over
//! seasons, march an army into it, have an enemy march up to it, and end the
//! turn into the assault.** The only things placed by hand are the ones a
//! scenario would place — the map, the counties, and armies that already exist.
//!
//! # Why the map has a castle plot in it
//!
//! A county's castle stands on a 2×2 block of plane-0 bit `0x80` tiles whose
//! terrain is `0x14` (bare) or `0x15 … 0x19` (a castle of type 1 … 5).
//! `County_FindCastleTile` finds that block at load and stamps `0x14` on it;
//! [`l2_kingdom::map::stamp_castle_terrain`] is what raises it afterwards.
//! [`plot`] below is that block, and it is the only piece of scenery these
//! tests place.

use l2_game::game::Assets;
use l2_game::input::{Event, Key, Rect};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::{castle, map};
use l2_game::Game;
use l2_kingdom::map::{flags, terrain, CampaignMap, MAP_DIM, MAP_TILES};
use l2_kingdom::unit::{TroopType, Unit, UnitKind};
use l2_kingdom::MercenaryBands;
use l2_view::campaign;

/// The border between county 1 and county 2, chosen the same way
/// `tests/military.rs` chooses it: inside the band the campaign screen opens
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
    for _ in 0..4_000 {
        if done(m, g) {
            return;
        }
        tick(m, g, a);
    }
    panic!("{what} never happened");
}

fn end_turn(m: &mut Machine, g: &mut Game, a: &Assets) {
    let before = g.kingdom.turn_count;
    press(m, g, a, 'e');
    run_until(m, g, a, "the turn", |_, g| g.kingdom.turn_count > before);
    for _ in 0..=l2_view::fade::PHASES {
        tick(m, g, a);
    }
}

/// The sidebar's **CASTLE** button, found by its name rather than by its index
/// so that reordering the strip does not silently point this at COURT.
fn castle_button() -> Rect {
    map::SIDEBAR_BUTTONS
        .iter()
        .find(|b| b.name == "CASTLE")
        .expect("the sidebar has a castle button")
        .rect()
}

/// A county's castle plot: a 2×2 block of `SETTLEMENT` tiles at terrain
/// `0x14`, placed inside the opening viewport so it can be clicked and marched
/// to. Returns its north-west tile.
fn plot(g: &mut Game, county: u8, at: (u8, u8)) -> (u8, u8) {
    for dy in 0..2u8 {
        for dx in 0..2u8 {
            let (x, y) = (at.0 + dx, at.1 + dy);
            g.kingdom.campaign.map.set_flags(x, y, flags::SETTLEMENT);
            g.kingdom.campaign.map.set_terrain(x, y, terrain::CASTLE_PLOT);
            g.kingdom.campaign.map.set_county(x, y, county);
        }
    }
    at
}

/// Two counties, one the player's and one an opponent's, on a map every tile of
/// which is walkable. The opponent holds two counties so that losing one does
/// not eliminate it and end the game — `tests/military.rs` learned that the
/// hard way and it is the same trap here.
fn world() -> (Game, Assets) {
    let mut g = Game::new(11);
    g.kingdom.set_county_count(3);
    for id in 1..=3usize {
        let c = &mut g.kingdom.counties[id];
        c.population = 4_000;
        c.happiness = 90;
        c.grain = 60_000;
        c.herd = 400;
    }
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[2].owner = 2;
    g.kingdom.counties[3].owner = 2;
    for (id, neighbours) in [(1usize, vec![2u8]), (2, vec![1, 3]), (3, vec![2])] {
        let c = &mut g.kingdom.counties[id];
        c.neighbour_count = neighbours.len() as u8;
        for (i, n) in neighbours.into_iter().enumerate() {
            c.neighbours[i] = n;
        }
    }
    for realm in 1..=2usize {
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 5;
        g.kingdom.realms[realm].gold = 20_000;
        // Enough wood and stone in the store that a palisade is paid for on the
        // spot. A castle ordered without them is a separate test.
        g.kingdom.realms[realm].wood = 5_000;
        g.kingdom.realms[realm].stone = 5_000;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].weapons = [200; 6];

    let mut m = CampaignMap::empty();
    for i in 0..MAP_TILES {
        let x = i % MAP_DIM;
        m.county[i] = if x < BORDER_1_2 { 1 } else { 2 };
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

/// A tile whose whole 4×4 neighbourhood — from one west and one north to two
/// east and two south — is inside the opening viewport **and** in the given
/// county, so a 2×2 castle block placed on it has room around it for an army to
/// stand and for a test to click.
///
/// The campaign screen opens at `Map_InitMode`'s own scroll origin and these
/// tests never scroll, so [`pixel`]'s fresh probe is a valid ruler for the
/// machine's screen. Searched in ascending `(y, x)` so the answer is the same
/// every run.
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

// ---------------------------------------------------------------------------
// 1. Building one
// ---------------------------------------------------------------------------

/// **The door.** The sidebar's CASTLE button opens screen `0x1B` for the
/// selected county, and refuses a county that is not yours — `Castle_OpenScreen`
/// (`0x00436A88`), whose else-branch is message `0x70`.
#[test]
fn the_sidebar_castle_button_opens_the_chooser_for_your_own_county() {
    let (mut g, a, mut m) = on_the_map();
    click(&mut m, &mut g, &a, on(castle_button()));
    assert_eq!(m.top_id(), Some(ScreenId::Castle(1)), "the chooser opened on county 1");

    send(&mut m, &mut g, &a, Event::KeyDown(Key::Escape));
    g.selected = 2;
    click(&mut m, &mut g, &a, on(castle_button()));
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "county 2 is not yours");
}

/// **The whole verb by mouse**: open the chooser, pick a castle out of the row
/// of five, press the tick — and the county is building one.
///
/// `castle_degraded` had no writer a player could reach before this. The
/// assertion that matters is not that the field moved but that **nothing in
/// this test wrote it**.
#[test]
fn picking_a_castle_and_pressing_ok_starts_the_work() {
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);
    let (wood, stone) = (g.kingdom.realms[1].wood, g.kingdom.realms[1].stone);

    click(&mut m, &mut g, &a, on(castle_button()));
    // The third strip: a Norman keep. The five are the five castle pictures,
    // side by side, and their rectangles are the original's widget table.
    click(&mut m, &mut g, &a, on(castle::type_rect(2)));
    click(&mut m, &mut g, &a, on(castle::OK));

    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the screen closes on the order");
    let c = &g.kingdom.counties[1];
    assert_eq!(c.castle_type, 3, "castleType moves the moment the work is ordered");
    assert_eq!(c.castle_building, 0, "and nothing stood here before it");
    assert_eq!(c.castle_degraded, 1, "**the field nothing used to write**");
    assert!(c.castle_switch, "and the castle job is switched on");
    assert_eq!(c.castle_work_left, 800, "a Norman keep is 800 man-seasons");
    assert_eq!(c.castle_percent, 0);
    // 200 wood and 1,000 stone, paid out of a store that had both.
    assert_eq!(g.kingdom.realms[1].wood, wood - 200);
    assert_eq!(g.kingdom.realms[1].stone, stone - 1_000);
    assert_eq!((c.castle_wood_owed, c.castle_stone_owed), (0, 0));

    // And the map knows: the plot's terrain is a castle now, not bare ground.
    assert_eq!(
        g.kingdom.campaign.map.terrain_at(here.0, here.1),
        terrain::CASTLE_PLOT + 3,
        "the block is stamped as a Norman keep",
    );
}

/// The OK button's two refusals, driven through the screen.
#[test]
fn the_ok_button_refuses_the_castle_you_have_and_anything_smaller() {
    for (standing, pick, why) in [
        (3u8, 2usize, "already"),
        (3, 1, "smaller"),
        (3, 0, "smaller"),
    ] {
        let (mut g, a, mut m) = on_the_map();
        let here = visible_in(&g, 1);
        plot(&mut g, 1, here);
        g.kingdom.counties[1].castle_type = standing;
        let before = g.kingdom.realms[1].stone;

        click(&mut m, &mut g, &a, on(castle_button()));
        click(&mut m, &mut g, &a, on(castle::type_rect(pick)));
        click(&mut m, &mut g, &a, on(castle::OK));

        assert_eq!(m.top_id(), Some(ScreenId::Campaign), "{why}: the screen closes either way");
        assert_eq!(g.kingdom.counties[1].castle_degraded, 0, "{why}: no work was started");
        assert_eq!(g.kingdom.counties[1].castle_type, standing, "{why}: and nothing changed");
        assert_eq!(g.kingdom.realms[1].stone, before, "{why}: nothing was spent");
    }
}

/// **You have to close the mines to build a castle**, and that is the
/// original's rule rather than this fixture's shape.
///
/// `Labour_Allocate` serves the industry half as a round robin — wood, stone,
/// iron, blacksmith, **and castle building only as the tail**, reached when all
/// four of those are at their ceilings. `Industry_LabourEstimate` gives wood,
/// iron and stone a ceiling of **100,000** in any owned county that has the
/// site, so those four are never full and the tail is never reached: a county
/// with its industries running puts every spare hand down the mine and none on
/// the walls, for ever.
///
/// It is not a defect and it is not ours. It is why `AI_ChooseIndustry`
/// switches iron and the blacksmith off outright the moment a lord orders a
/// castle, and keeps wood and stone only while the build still owes some — a
/// rule that reads as an odd strategic quirk until you see what it is *for*.
/// For a human the switch is a click on the building on the campaign map.
///
/// **The click is not driven here and it is worth saying why.** The industry
/// toggle goes through `MapScreen::county_at`, which reads the *painted* county
/// plane — and the plane comes from the map **file**, which `Assets::placeholder`
/// supplies as 4,096 zero bytes. So no click on any tile of these synthetic
/// worlds ever finds a county. `tests/screens.rs`'s
/// `every_painted_pixel_of_a_mine_reaches_the_industry_toggle` drives that half
/// against the real sheet and the real map, which is where it belongs; what is
/// asserted here is the *rule*, which is the half that was never stated.
#[test]
fn a_county_with_its_mines_running_never_gets_round_to_the_castle() {
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);

    click(&mut m, &mut g, &a, on(castle_button()));
    click(&mut m, &mut g, &a, on(castle::type_rect(0)));
    click(&mut m, &mut g, &a, on(castle::OK));

    for _ in 0..4 {
        end_turn(&mut m, &mut g, &a);
    }
    let c = &g.kingdom.counties[1];
    assert_eq!(c.labour_useful[3], 200, "the castle's ceiling is a real number");
    assert_eq!(c.labour[3], 0, "and not one person is standing at it");
    assert_eq!(c.castle_work_left, 200, "four seasons and no work done");
    assert!(c.labour[6] > 0, "they are all in the forest, which never fills up");

    // Shut the four industries — the state a click on each building produces.
    for slot in 0..4 {
        g.kingdom.counties[1].industry[slot].enabled = false;
    }
    end_turn(&mut m, &mut g, &a);
    assert!(
        g.kingdom.counties[1].labour[3] > 0,
        "with the mines shut the builders finally have somewhere to be",
    );
}

/// **A castle ordered from the map is finished by ending turns**, and it comes
/// with its own garrison of archers — `Castle_BuildTick`'s completion branch,
/// which nothing had ever run.
#[test]
fn ending_turns_finishes_the_castle_and_it_arrives_with_a_garrison() {
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);
    // A county with nothing to mine, so the industry half of its people has
    // nowhere to go but the castle. See the test above for what a county with
    // mines does instead.
    for slot in 0..4 {
        g.kingdom.counties[1].industry[slot].has_resource = false;
    }

    click(&mut m, &mut g, &a, on(castle_button()));
    click(&mut m, &mut g, &a, on(castle::type_rect(0))); // a wooden palisade, 200 man-seasons
    click(&mut m, &mut g, &a, on(castle::OK));
    assert_eq!(g.kingdom.counties[1].castle_degraded, 1);
    assert_eq!(g.kingdom.campaign.units.len(), 0, "nothing on the map yet");

    let mut seasons = 0;
    while g.kingdom.counties[1].castle_degraded != 0 && seasons < 20 {
        end_turn(&mut m, &mut g, &a);
        seasons += 1;
    }
    assert_eq!(
        g.kingdom.counties[1].castle_degraded, 0,
        "the palisade never topped out in {seasons} seasons — percent {}, work left {}",
        g.kingdom.counties[1].castle_percent, g.kingdom.counties[1].castle_work_left,
    );
    assert_eq!(g.kingdom.counties[1].castle_percent, 100);
    assert_eq!(g.kingdom.counties[1].castle_type, 1);
    assert!(!g.kingdom.counties[1].castle_switch, "the builders go back to the fields");

    // `Castle_RaiseFreeGarrison`: a palisade is worth 50 archers, mustered and
    // marched straight inside.
    let garrison = g.kingdom.counties[1].garrison_unit;
    assert_ne!(garrison, 0, "a new castle comes with a garrison");
    let unit = g.kingdom.campaign.units.get(garrison).expect("the garrison");
    assert_eq!(unit.men, 50, "50 archers for a wooden palisade");
    assert_eq!(unit.troops[TroopType::Archer.index()], 50, "and they carry bows");
    assert_eq!(unit.garrison_county, 1, "the unit knows which castle it is in");
    assert_eq!(
        (unit.x, unit.y),
        here,
        "and it was teleported onto the castle block, not left outside",
    );
}

// ---------------------------------------------------------------------------
// 2. Manning one
// ---------------------------------------------------------------------------

/// **Marching your own army onto your own castle garrisons it** —
/// `Unit_ReachCastleBuilding`'s first arm, which had no counterpart here at
/// all: our stepper *trampled* a castle instead, because code 6 is two handlers
/// and only the resource-site one was written.
#[test]
fn marching_an_army_onto_your_own_castle_puts_it_inside() {
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);
    g.kingdom.counties[1].castle_type = 2; // a motte and bailey, 200 men
    l2_kingdom::map::stamp_castle_terrain(&mut g.kingdom.campaign.map, 1, 2);

    // The army starts one tile west of the block.
    let outside = (here.0 - 1, here.1);
    let id = army_at(&mut g, 1, 1, 150, outside);

    click(&mut m, &mut g, &a, pixel(outside.0, outside.1).unwrap());
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    assert!(g.kingdom.campaign.units.get(id).is_some_and(|u| u.moving), "ordered from the map");

    end_turn(&mut m, &mut g, &a);

    assert_eq!(g.kingdom.counties[1].garrison_unit, id, "the county holds the link");
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert_eq!(unit.garrison_county, 1, "and so does the unit");
    assert_eq!((unit.x, unit.y), here, "teleported onto the block");
}

/// **The cap is the castle's**, and over it nothing happens at all: the army
/// stays where it is and the castle stays empty. `Army_GarrisonApply`'s only
/// guard.
#[test]
fn a_castle_refuses_more_men_than_it_can_barrack() {
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);
    g.kingdom.counties[1].castle_type = 1; // a palisade: 150 men
    l2_kingdom::map::stamp_castle_terrain(&mut g.kingdom.campaign.map, 1, 1);

    let outside = (here.0 - 1, here.1);
    let id = army_at(&mut g, 1, 1, 151, outside);
    click(&mut m, &mut g, &a, pixel(outside.0, outside.1).unwrap());
    click(&mut m, &mut g, &a, pixel(here.0, here.1).unwrap());
    end_turn(&mut m, &mut g, &a);

    assert_eq!(g.kingdom.counties[1].garrison_unit, 0, "151 men will not fit in 150");
    let unit = g.kingdom.campaign.units.get(id).expect("still there");
    assert_eq!(unit.garrison_county, 0);
    assert_ne!((unit.x, unit.y), here, "and it did not move onto the block");
}

// ---------------------------------------------------------------------------
// 3. Besieging one, and taking the county
// ---------------------------------------------------------------------------

/// **The whole route, in one game.**
///
/// County 2 is the opponent's. It gets a castle and a garrison the same way the
/// player's would — the AI's own order is out of scope here, so the castle is
/// placed and the garrison is walked in by the same map clicks a player would
/// use for his own. Then the player's army marches up to the castle, which lays
/// the siege, and ending the turn runs phase 2's assault to a verdict.
#[test]
fn an_army_that_marches_onto_an_enemy_castle_besieges_it_and_the_turn_settles_the_assault() {
    let (mut g, a, mut m) = on_the_map();
    // A palisade, so no siege engines are needed and phase 2 assaults on the
    // first turn: `ENGINES_REQUIRED_FROM_LEVEL` is 3.
    let keep = visible_in(&g, 2);
    plot(&mut g, 2, keep);
    g.kingdom.counties[2].castle_type = 1;
    l2_kingdom::map::stamp_castle_terrain(&mut g.kingdom.campaign.map, 2, 1);

    // The garrison walks in, the way a player's would.
    let garrison = army_at(&mut g, 2, 2, 40, (keep.0 - 1, keep.1));
    {
        let map = g.kingdom.campaign.map.clone();
        l2_kingdom::movement::order_move(
            &map,
            &mut g.kingdom.campaign.units,
            garrison,
            keep,
            l2_kingdom::movement::Routing::Direct,
        )
        .expect("one tile");
    }
    end_turn(&mut m, &mut g, &a);
    assert_eq!(
        g.kingdom.counties[2].garrison_unit, garrison,
        "the enemy castle is manned, and by a march rather than by an assignment",
    );

    // Now the county cannot be walked into at all — `conquest.rs:114`, the one
    // `if` this whole subsystem exists to open.
    assert!(
        !l2_kingdom::conquest::can_be_entered(
            &g.kingdom.counties,
            &g.kingdom.campaign.units,
            2,
            1
        ),
        "a castle plus a garrison shuts the county",
    );

    // The player's army marches onto the castle. That is `Army_BeginSiege`.
    let camp = (keep.0 - 1, keep.1 + 1);
    let besieger = army_at(&mut g, 1, 1, 800, camp);
    click(&mut m, &mut g, &a, pixel(camp.0, camp.1).unwrap());
    click(&mut m, &mut g, &a, pixel(keep.0, keep.1).unwrap());
    assert!(g.kingdom.campaign.units.get(besieger).is_some_and(|u| u.moving), "ordered");

    // Ending the turn walks it up, lays the siege, and phase 2 storms the
    // palisade — which stops and asks, because the besieger is the human's.
    press(&mut m, &mut g, &a, 'e');
    run_until(&mut m, &mut g, &a, "the assault prompt", |m, _| {
        m.top_id() == Some(ScreenId::BattlePrompt)
    });
    assert_eq!(
        g.kingdom.campaign.units.get(besieger).map(|u| u.besieging_county),
        Some(2),
        "the march laid the siege",
    );

    // Decline: the autocalc settles it. 800 men against 40 behind a palisade.
    click(
        &mut m,
        &mut g,
        &a,
        on(l2_game::screens::battle::widget_rect(l2_game::screens::battle::DECLINE)),
    );
    assert_eq!(m.top_id(), Some(ScreenId::BattleResult), "then the result");
    click(&mut m, &mut g, &a, on(l2_game::screens::battle::ok_rect()));
    run_until(&mut m, &mut g, &a, "the rest of the turn", |m, _| {
        m.top_id() == Some(ScreenId::Campaign)
    });

    assert!(
        g.kingdom.campaign.units.get(garrison).is_none(),
        "the garrison was destroyed by the assault",
    );
    assert_eq!(g.kingdom.counties[2].garrison_unit, 0, "and the county's link with it is gone");
    assert!(
        g.kingdom
            .campaign
            .units
            .get(besieger)
            .is_none_or(|u| u.besieging_county == 0),
        "the siege is over either way",
    );
    // With the garrison gone the county can be entered, which is the point of
    // the whole exercise.
    assert!(
        l2_kingdom::conquest::can_be_entered(
            &g.kingdom.counties,
            &g.kingdom.campaign.units,
            2,
            1
        ),
        "and the door the siege existed to open is open",
    );
}

// ---------------------------------------------------------------------------
// 4. The picture
// ---------------------------------------------------------------------------

/// **A castle is drawn on the campaign map**, and its picture follows its state.
///
/// Not an assertion about `Assets::placeholder`: it reads the override plane
/// the painter is handed, which is the same plane at any zoom and with any
/// artwork. `docs/agents.md`'s warning about the placeholder applies to the hit
/// test, and this is not one.
///
/// The three appearances are `Castle_StampTile`'s three arms, and driving all
/// three is deliberate — a frame table exercised at one input is C26's shape.
#[test]
fn a_castle_is_stamped_onto_the_map_and_changes_picture_as_it_goes_up() {
    use l2_kingdom::map::{castle_stamp, CASTLE_BANK_BYTE};

    // Finished: `level * 4 + 0x50`.
    let built = castle_stamp(3, 0, 100).expect("a keep");
    assert_eq!(built.bank, CASTLE_BANK_BYTE);
    assert_eq!(built.terrain, 0x17, "a Norman keep's content byte");
    assert_eq!(built.frames, [0x58, 0x5A, 0x59, 0x5B], "0x50 + 2*4, plus [0, 2, 1, 3]");

    // Under way, under half done: `level * 4 + 0x28`.
    let early = castle_stamp(3, 1, 49).expect("a keep");
    assert_eq!(early.frames[0], 0x30);
    // …and at half, `level * 4 + 0x3C`.
    let half = castle_stamp(3, 1, 50).expect("a keep");
    assert_eq!(half.frames[0], 0x44);
    assert_eq!(half.terrain, early.terrain, "the content byte does not move with the work");

    // All three are different pictures, which is what the arms are for.
    assert_ne!(early.frames, half.frames);
    assert_ne!(half.frames, built.frames);
    // A repair (`castleDegraded == 2`) uses the same two work arms.
    assert_eq!(castle_stamp(3, 2, 10).unwrap().frames, early.frames);

    assert!(castle_stamp(0, 0, 0).is_none(), "a bare plot draws what the file holds");

    // And the map screen puts them on the override plane the painter reads.
    let (mut g, a, mut m) = on_the_map();
    let here = visible_in(&g, 1);
    plot(&mut g, 1, here);
    let bare = {
        let ctx = Ctx { game: &mut g, assets: &a };
        map::MapScreen::town_graphics(&ctx).get(here.0 as usize, here.1 as usize)
    };
    assert_eq!(bare, None, "an empty plot overrides nothing");

    click(&mut m, &mut g, &a, on(castle_button()));
    click(&mut m, &mut g, &a, on(castle::type_rect(2)));
    click(&mut m, &mut g, &a, on(castle::OK));
    let scaffold = {
        let ctx = Ctx { game: &mut g, assets: &a };
        map::MapScreen::town_graphics(&ctx).get(here.0 as usize, here.1 as usize)
    };
    assert_eq!(
        scaffold,
        Some((CASTLE_BANK_BYTE, 0x30)),
        "the moment it is ordered there is scaffolding on the map",
    );
    // All four quadrants, and each a different frame.
    let quads: Vec<Option<(u8, u8)>> = {
        let ctx = Ctx { game: &mut g, assets: &a };
        let o = map::MapScreen::town_graphics(&ctx);
        (0..2)
            .flat_map(|dy| (0..2).map(move |dx| (dx, dy)))
            .map(|(dx, dy)| o.get(here.0 as usize + dx, here.1 as usize + dy))
            .collect()
    };
    assert_eq!(
        quads,
        vec![
            Some((CASTLE_BANK_BYTE, 0x30)),
            Some((CASTLE_BANK_BYTE, 0x32)),
            Some((CASTLE_BANK_BYTE, 0x31)),
            Some((CASTLE_BANK_BYTE, 0x33)),
        ],
        "the 2x2 block, with Map_StampBlock's [0, 2, 1, 3] quadrant offsets",
    );
}
