//! **The message scroll, played.**
//!
//! ```text
//! cargo test -p l2-game --test messages
//! ```
//!
//! Three questions, and they are the three the message window was blocking:
//!
//! * **can a player dismiss a message?** — `Msg_HandleInput` (`0x0047685D`),
//!   every screen's arm, which nothing in this workspace had;
//! * **can a player answer a lord?** — the two prompts `docs/arms.json` called
//!   *"the only two places a person answers a lord rather than writing to one,
//!   and both are unreachable"*;
//! * **can a player be told he has won?** — `docs/plan.md`'s mainline win, which
//!   ends *"the game is won when **that** is dismissed"* and had nothing to
//!   dismiss.
//!
//! Everything here goes through [`Machine::handle`] and [`Machine::update`] with
//! [`Event`] values. Nothing calls `message::dismiss`, sets `campaign.outcome`,
//! or reaches into the ring except to put a letter in it — which is what a rule
//! does, and the rules are `l2_kingdom`'s.
//!
//! > *"A rule with no way in is not a rule the game has."* — `docs/agents.md`

use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, category, Prompt, Record};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_kingdom::victory::Outcome;

// ---------------------------------------------------------------------- setup

fn send(m: &mut Machine, g: &mut Game, a: &Assets, e: Event) {
    let mut ctx = Ctx { game: g, assets: a };
    m.handle(e, &mut ctx);
}

fn tick(m: &mut Machine, g: &mut Game, a: &Assets) {
    let mut ctx = Ctx { game: g, assets: a };
    m.update(&mut ctx);
}

/// The middle of a widget, so a hit test that is off by a pixel still passes and
/// one that is off by a button does not.
fn on(at: (i32, i32)) -> Event {
    Event::Click { x: at.0 + Prompt::SIDE / 2, y: at.1 + Prompt::SIDE / 2 }
}

/// Five realms on a small map, realm 1 the human, nothing owned yet.
fn world() -> (Game, Assets, Machine) {
    let mut g = Game::new(0xB0A7);
    g.player = 1;
    g.kingdom.set_county_count(6);
    for id in 1..=6usize {
        let c = &mut g.kingdom.counties[id];
        c.population = 500;
        c.happiness = 70;
    }
    for realm in 1..=5usize {
        g.kingdom.realms[realm].in_play = true;
        g.kingdom.realms[realm].strength = 3;
        g.kingdom.realms[realm].gold = 2_000;
        g.kingdom.realms[realm].lord = realm as u8 - 1;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].lord = 1;
    (g, Assets::placeholder(), Machine::new(ScreenId::Campaign))
}

/// One `Msg_Enqueue`. The `to` is the local player or 0, because that is the
/// only kind of record that ever reaches a peer's ring.
fn post(g: &mut Game, r: Record) {
    let player = g.player;
    assert!(g.messages.enqueue(r, player), "the peer filter kept this record out");
}

fn notice(group: u16) -> Record {
    Record { to: 0, group, category: category::NOTICE, ..Record::default() }
}

/// Run the machine until the scroll is up, and say so if it never is.
fn open_the_scroll(m: &mut Machine, g: &mut Game, a: &Assets) {
    for _ in 0..8 {
        tick(m, g, a);
        if m.top_id() == Some(ScreenId::Message) {
            return;
        }
    }
    panic!("Msg_Pump never raised the window; the screen is {:?}", m.top_id());
}

macro_rules! painted {
    () => {{
        let Some(dir) = l2_testkit::install_dir() else {
            l2_testkit::skip!("no game install, so there is no artwork to draw with");
        };
        let platform =
            l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let (mut g, _placeholder, m) = world();
        g.map_slot = 4;
        (g, assets, m)
    }};
}

fn painting(g: &mut Game, a: &Assets, m: &mut Machine) -> l2_view::Canvas {
    let mut canvas = l2_view::Canvas::screen();
    let ctx = Ctx { game: g, assets: a };
    m.draw(&ctx, &mut canvas);
    canvas
}

// ------------------------------------------------------- 1. dismiss a message

/// **The scroll appears on the campaign map and the corner button closes it.**
///
/// `Msg_Pump` pulls, `Msg_DrawWindow` lays the window out, `Ui_OkButton` puts
/// its picture in the bottom-right corner, and `Msg_HandleInput` hit-tests a
/// 48 × 48 box round it.
#[test]
fn a_message_appears_on_the_map_and_the_corner_button_closes_it() {
    let (mut g, a, mut m) = world();
    post(&mut g, notice(0x92));

    open_the_scroll(&mut m, &mut g, &a);
    assert_eq!(g.messages.open().map(|r| r.group), Some(0x92));

    // **Pinned, not computed.** `FUN_004093E0(0x20, 0xA0, 0x1A, 0x0E)` and
    // `Ui_OkButton(x + w - 0x30, y + h - 0x30)` put the corner button at
    // (0x190, 0x150) = (400, 336). Deriving this from `frame_of` would make the
    // test agree with whatever the geometry says, which is the trap
    // `docs/agents.md` records: ablating a constant while computing the probe
    // from that same constant tests nothing at all.
    send(&mut m, &mut g, &a, Event::Click { x: 400, y: 336 });

    assert!(!g.messages.is_open(), "the scroll is gone");
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and the map is back");
}

/// The **right** button closes it too, from anywhere — the arm the original
/// tests before it tests anything else.
#[test]
fn the_right_button_closes_it_from_anywhere() {
    let (mut g, a, mut m) = world();
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);

    send(&mut m, &mut g, &a, Event::RightClick { x: 4, y: 4 });
    assert!(!g.messages.is_open());
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
}

/// **A left click on the map closes a notice and the map does nothing else.**
/// `Map_Click`'s whole body is inside `if (g_messageGroup == 0)`.
#[test]
fn a_left_click_on_the_map_closes_a_notice_and_selects_nothing() {
    let (mut g, a, mut m) = world();
    g.kingdom.counties[3].owner = 1;
    g.selected = 0;
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);

    // The middle of the viewport: somewhere the map would ordinarily act on.
    send(&mut m, &mut g, &a, Event::Click { x: 200, y: 200 });
    assert!(!g.messages.is_open(), "the click closed the scroll");
    assert_eq!(g.selected, 0, "and the map did not act on it");
}

/// …and **a question survives that click**, which is the half that keeps a
/// stray click from answering a lord.
#[test]
fn a_left_click_on_the_map_leaves_a_question_standing() {
    let (mut g, a, mut m) = world();
    post(
        &mut g,
        Record { to: 1, from: 3, group: 180, category: category::ALLIANCE_PROMPT, ..Record::default() },
    );
    open_the_scroll(&mut m, &mut g, &a);

    send(&mut m, &mut g, &a, Event::Click { x: 200, y: 200 });
    assert!(g.messages.is_open(), "an alliance offer is not closed by a click on the map");
    assert_eq!(m.top_id(), Some(ScreenId::Message));
}

/// **The message never times out in single player.** `Msg_Pump` clamps the
/// timer to 1 rather than expiring it, so the scroll waits — two thousand ticks
/// is more than the whole timer and it is still up.
#[test]
fn in_single_player_the_scroll_waits_for_ever() {
    let (mut g, a, mut m) = world();
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);

    for _ in 0..(message::TIMER_START + 500) {
        tick(&mut m, &mut g, &a);
    }
    assert!(g.messages.is_open());
    assert_eq!(m.top_id(), Some(ScreenId::Message));
}

/// **…and in a network game it goes on its own after 399 ticks**, which is the
/// same code with `g_multiplayer` set. Without this the timeout branch is a rule
/// with no way in.
#[test]
fn in_a_network_game_the_scroll_closes_itself() {
    let (mut g, a, mut m) = world();
    g.multiplayer = true;
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);

    let mut ticks = 0;
    while m.top_id() == Some(ScreenId::Message) && ticks < 2_000 {
        tick(&mut m, &mut g, &a);
        ticks += 1;
    }
    // Pinned: 2000 - 0x641 = 399 decrements leave the timer at 0x641, and the
    // 400th puts it below. Written out rather than computed from the two
    // constants the code under test reads.
    assert_eq!(ticks, 400, "2000 down past 0x641, and it is gone");
    assert!(!g.messages.is_open());
}

/// **Opening another screen throws the message away.** `Msg_Pump`'s else
/// branch, which is the arm that makes the scroll part of the map rather than
/// part of the interface.
#[test]
fn walking_into_another_screen_loses_the_message() {
    let (mut g, a, mut m) = world();
    post(&mut g, notice(0x92));
    open_the_scroll(&mut m, &mut g, &a);
    assert!(g.messages.is_open());

    // Pushed from outside, exactly as `Machine::push` is documented for: a
    // screen the interface can only reach through three clicks.
    m.push(ScreenId::Village(3));
    tick(&mut m, &mut g, &a);
    assert!(!g.messages.is_open(), "the village dismissed it unread");
}

// -------------------------------------------------------- 2. answer a lord

fn alliance_offer(from: u8) -> Record {
    Record {
        to: 1,
        from,
        group: 180,
        variant: 0,
        category: category::ALLIANCE_PROMPT,
        ..Record::default()
    }
}

/// **A player accepts an alliance.** `L2.eng` group 180, *"Accept alliance ?"*,
/// message category `0x0B`, widget table `0x004DDAC0`, handler `FUN_00436872` —
/// and the thumb-up calls `Diplo_FormAlliance(g_localPlayer, offerer)`.
#[test]
fn a_player_can_accept_an_alliance() {
    let (mut g, a, mut m) = world();
    g.kingdom.realms[3].offer_pending = true;
    post(&mut g, alliance_offer(3));
    open_the_scroll(&mut m, &mut g, &a);

    let [yes, _no] = Prompt::AcceptAlliance.widgets();
    send(&mut m, &mut g, &a, on(yes));

    assert_eq!(g.kingdom.realms[1].ally, 3, "realm 1 is allied to realm 3");
    assert_eq!(g.kingdom.realms[3].ally, 1, "and realm 3 to realm 1");
    assert!(!g.messages.is_open());
    assert_eq!(m.top_id(), Some(ScreenId::Campaign));
}

/// **Declining an AI's offer in single player does nothing at all** — not a
/// refusal, not a grudge, not a letter. The guard is
/// `(offerer is human) || (hotspot != 0)`, and an AI offer declined fails both.
///
/// Asserted rather than assumed, because a rule that does nothing is
/// indistinguishable from a rule nobody wired up unless something checks the
/// nothing.
#[test]
fn declining_an_ai_offer_runs_nothing_at_all() {
    let (mut g, a, mut m) = world();
    let before = g.kingdom.realms.clone();
    post(&mut g, alliance_offer(3));
    open_the_scroll(&mut m, &mut g, &a);

    let [_yes, no] = Prompt::AcceptAlliance.widgets();
    send(&mut m, &mut g, &a, on(no));

    assert!(!g.messages.is_open(), "the window closed");
    assert_eq!(g.kingdom.realms[1].ally, 0, "and nobody is allied");
    assert_eq!(g.kingdom.realms[3].ally, 0);
    assert_eq!(
        g.kingdom.realms[1].pair(3),
        before[1].pair(3),
        "no grudge, no standing change -- the offer simply lapses",
    );
}

/// **A right click on an alliance offer is neither yes nor no.** The right
/// branch is tested before the widgets and has no category test in front of it.
#[test]
fn a_right_click_closes_an_offer_without_answering_it() {
    let (mut g, a, mut m) = world();
    post(&mut g, alliance_offer(3));
    open_the_scroll(&mut m, &mut g, &a);

    send(&mut m, &mut g, &a, Event::RightClick { x: 300, y: 300 });
    assert!(!g.messages.is_open());
    assert_eq!(g.kingdom.realms[1].ally, 0, "closing is not accepting");
}

/// **An offer that arrives when you already have an ally closes itself.**
/// `Msg_DrawWindow`'s category-`0x0B` arm opens with the guard, so the window is
/// never drawn and the player never sees it.
#[test]
fn an_offer_lapses_unseen_when_you_already_have_an_ally() {
    let (mut g, a, mut m) = world();
    l2_kingdom::diplomacy::form_alliance(&mut g.kingdom.realms, 1, 2);
    post(&mut g, alliance_offer(3));

    for _ in 0..8 {
        tick(&mut m, &mut g, &a);
    }
    assert!(!g.messages.is_open(), "the draw dismissed it");
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "and it was never on screen to click");
    assert_eq!(g.kingdom.realms[1].ally, 2, "the alliance we had is untouched");
}

/// **A player pays his ally for help.** `L2.eng` group 185, *"Pay -"*, message
/// category `0x0A`, widget table `0x004DDA90`, handler `Diplo_PayHelpClicked`
/// (`0x004367FF`) — the accept button calls `Diplo_PayForHelp(myAlly, me,
/// g_diploHelpCounty, g_diploHelpPrice)`.
#[test]
fn a_player_can_pay_his_ally_for_help() {
    let (mut g, a, mut m) = world();
    l2_kingdom::diplomacy::form_alliance(&mut g.kingdom.realms, 1, 2);
    g.kingdom.counties[4].owner = 3;
    g.kingdom.diplomacy.help_county = 4;
    g.kingdom.diplomacy.help_price = 250;
    let purse = g.kingdom.realms[1].gold;
    let multiple = g.kingdom.realms[2].pair(1).help_price_multiple;

    post(
        &mut g,
        Record { to: 1, from: 2, group: 185, category: category::PAY_PROMPT, county: 4, ..Record::default() },
    );
    open_the_scroll(&mut m, &mut g, &a);

    let [yes, _no] = Prompt::PayForHelp.widgets();
    send(&mut m, &mut g, &a, on(yes));

    assert_eq!(g.kingdom.realms[1].gold, purse - 250, "the gold moved");
    assert_eq!(g.kingdom.realms[2].gold, 2_000 + 250, "into the ally's treasury");
    assert_eq!(
        g.kingdom.realms[2].pair(1).help_price_multiple,
        multiple + 1,
        "and the next favour costs more",
    );
    assert!(!g.messages.is_open());
}

/// Declining the price leaves the treasury alone.
#[test]
fn declining_the_price_costs_nothing() {
    let (mut g, a, mut m) = world();
    l2_kingdom::diplomacy::form_alliance(&mut g.kingdom.realms, 1, 2);
    g.kingdom.diplomacy.help_county = 4;
    g.kingdom.diplomacy.help_price = 250;
    let purse = g.kingdom.realms[1].gold;

    post(
        &mut g,
        Record { to: 1, from: 2, group: 185, category: category::PAY_PROMPT, county: 4, ..Record::default() },
    );
    open_the_scroll(&mut m, &mut g, &a);

    let [_yes, no] = Prompt::PayForHelp.widgets();
    send(&mut m, &mut g, &a, on(no));

    assert_eq!(g.kingdom.realms[1].gold, purse, "not a crown");
    assert!(!g.messages.is_open());
}

/// **A click inside a prompt that is on neither button does not fall through.**
/// `Widget_Test` is asked first and the corner button after it, so a miss lands
/// on the corner box or on nothing — but it never reaches the map underneath
/// while a question is up, because `Msg_DismissUnlessQuestion` refuses it.
#[test]
fn a_miss_inside_a_prompt_does_not_answer_it() {
    let (mut g, a, mut m) = world();
    post(&mut g, alliance_offer(3));
    open_the_scroll(&mut m, &mut g, &a);

    let [yes, _no] = Prompt::AcceptAlliance.widgets();
    // One pixel above the thumb-up: inside the window, on neither button.
    send(&mut m, &mut g, &a, Event::Click { x: yes.0, y: yes.1 - 1 });
    assert!(g.messages.is_open(), "still waiting for an answer");
    assert_eq!(g.kingdom.realms[1].ally, 0);
}

// ------------------------------------------------------------ 3. win a game

/// **A player is told he has won, and the telling is what wins it.**
///
/// The chain, end to end and with nothing skipped: realm 3 is recounted at zero
/// strength, `Realm_RecountStrength` enqueues group 194; `Score_RankRealms`
/// leaves `opponents_remaining` at 0; `Msg_Pump` shows 194; `Msg_DrawWindow`'s
/// category-`0x0E` arm sees no opponents left and **enqueues group 225 onto the
/// ring it is being drawn from**; the player dismisses 194; `Msg_Pump` shows
/// 225; `Msg_DrawWindow` sets `DAT_0053F0C4 = 10`; the player dismisses 225 and
/// `Msg_Dismiss` enters screen `0x1C`.
///
/// Every dismissal below is a click.
#[test]
fn a_player_is_told_he_has_won_and_dismissing_it_wins_the_game() {
    let (mut g, a, mut m) = world();
    // The human holds everything; the other four hold nothing and are about to
    // be counted out.
    for id in 1..=6usize {
        g.kingdom.counties[id].owner = 1;
    }
    l2_kingdom::conquest::recount_realm_counties(&g.kingdom.counties, &mut g.kingdom.realms);
    for realm in 2..=5u8 {
        g.recount_realm(realm);
    }
    assert_eq!(g.campaign.ranking.opponents_remaining, 0, "nobody is left to fight");
    assert_eq!(g.outcome(), Outcome::InPlay, "and nothing has been decided yet");

    // Four obituaries and, behind them, a victory that does not exist yet.
    let mut seen: Vec<u16> = Vec::new();
    for _ in 0..12 {
        if m.top_id() != Some(ScreenId::Message) {
            tick(&mut m, &mut g, &a);
            continue;
        }
        let group = g.messages.open().expect("a record").group;
        seen.push(group);
        let frame = message::frame_of(g.messages.open().unwrap()).expect("a frame");
        let (x, y) = frame.ok_button();
        send(&mut m, &mut g, &a, Event::Click { x, y });
        if m.top_id() == Some(ScreenId::Conquest) {
            break;
        }
    }

    assert!(seen.contains(&194), "an AI's obituary was shown: {seen:?}");
    assert_eq!(seen.last(), Some(&225), "and the last thing shown is Victory!: {seen:?}");
    assert_eq!(g.outcome(), Outcome::Won);
    assert_eq!(g.outcome().value(), 10, "DAT_0053F0C4");
    assert_eq!(m.top_id(), Some(ScreenId::Conquest), "screen 0x1C");
    assert_eq!(g.campaign.map, 1, "and the campaign stepped on");
}

/// **`docs/plan.md`'s mainline win, on its own**, with `Score_RankRealms` kept
/// out of it.
///
/// The test above wins a real position, and it turns out **two** paths put group
/// 225 in the ring there: this one, and `rank_and_crown` crowning the sole
/// survivor. Ablating the enqueue inside the draw left that test green — which
/// is exactly `docs/agents.md`'s *"a check that passes for an accidental reason
/// looks exactly like one that passes"*, and it is why this test exists.
///
/// So: no recount, no ranking pass, nothing but a ranking that says there is
/// nobody left and one obituary in the ring. Displaying it has to produce the
/// victory, or the arm is not built.
#[test]
fn displaying_an_ending_with_nobody_left_is_what_enqueues_the_victory() {
    let (mut g, a, mut m) = world();
    g.campaign.ranking =
        l2_kingdom::victory::Ranking { opponents_remaining: 0, ..Default::default() };
    post(
        &mut g,
        Record { to: 0, from: 3, group: 194, category: category::ENDING, ..Record::default() },
    );

    open_the_scroll(&mut m, &mut g, &a);
    assert_eq!(g.messages.open().map(|r| r.group), Some(194));
    assert_eq!(
        g.messages.waiting().iter().map(|r| r.group).collect::<Vec<_>>(),
        vec![225],
        "displaying 194 pushed 225 onto the ring it was drawn from",
    );
    assert_eq!(g.outcome(), Outcome::InPlay, "and 194 itself decided nothing");

    send(&mut m, &mut g, &a, Event::RightClick { x: 4, y: 4 });
    open_the_scroll(&mut m, &mut g, &a);
    assert_eq!(g.messages.open().map(|r| r.group), Some(225), "Victory!");
    assert_eq!(g.outcome(), Outcome::Won, "displayed is what writes 10");

    send(&mut m, &mut g, &a, Event::RightClick { x: 4, y: 4 });
    assert_eq!(m.top_id(), Some(ScreenId::Conquest), "and dismissed is what enters 0x1C");
}

/// **The mirror: the player's own defeat.** Group 224 with `from ==
/// g_localPlayer` is `DAT_0053F0C4 = 11`, and dismissing it enters the same
/// screen with the other branch.
#[test]
fn a_player_is_told_he_has_lost_and_dismissing_it_loses_the_game() {
    let (mut g, a, mut m) = world();
    for id in 1..=6usize {
        g.kingdom.counties[id].owner = 2;
    }
    l2_kingdom::conquest::recount_realm_counties(&g.kingdom.counties, &mut g.kingdom.realms);
    g.recount_realm(1);
    assert!(g.campaign.ranking.opponents_remaining > 0, "or it would be scored a win");

    open_the_scroll(&mut m, &mut g, &a);
    assert_eq!(g.messages.open().map(|r| r.group), Some(224), "Defeat!");
    assert_eq!(g.outcome(), Outcome::Lost, "displayed is what writes the outcome");
    assert_eq!(m.top_id(), Some(ScreenId::Message), "and it is still on screen");

    send(&mut m, &mut g, &a, Event::RightClick { x: 4, y: 4 });
    assert_eq!(m.top_id(), Some(ScreenId::Conquest), "dismissed is what acts on it");
    assert_eq!(g.campaign.map, 0, "a loss replays the map");
}

/// **An AI dying while others live ends nothing**, and the player still gets
/// told. Without this the two tests above would pass on a chain that declared a
/// winner every time anybody died.
#[test]
fn an_ai_dying_while_others_live_is_a_message_and_not_an_ending() {
    let (mut g, a, mut m) = world();
    for id in 1..=6usize {
        g.kingdom.counties[id].owner = if id <= 3 { 1 } else { 2 };
    }
    l2_kingdom::conquest::recount_realm_counties(&g.kingdom.counties, &mut g.kingdom.realms);
    g.recount_realm(3);

    open_the_scroll(&mut m, &mut g, &a);
    assert_eq!(g.messages.open().map(|r| r.group), Some(194), "Foiled again.");
    send(&mut m, &mut g, &a, Event::RightClick { x: 4, y: 4 });

    assert_eq!(g.outcome(), Outcome::InPlay);
    assert_eq!(m.top_id(), Some(ScreenId::Campaign), "the game carries on");
}

/// **The lord-flavoured variant is carried.** `Realm_RecountStrength` enqueues
/// group 194 with `variant = lord * 4 + rotation - 4`, and group 194 holds
/// seventeen strings — a label and sixteen laments, four per lord. A message
/// with no variant would draw the Knight's first line for every lord in the
/// game.
#[test]
fn an_ai_obituary_carries_the_lords_own_line() {
    let (mut g, _a, _m) = world();
    for id in 1..=6usize {
        g.kingdom.counties[id].owner = 1;
    }
    l2_kingdom::conquest::recount_realm_counties(&g.kingdom.counties, &mut g.kingdom.realms);
    g.kingdom.realms[4].lord = 3;
    g.kingdom.realms[4].voice_rotation = 2;
    g.recount_realm(4);

    let r = g.messages.waiting().into_iter().find(|r| r.group == 194).expect("an obituary");
    assert_eq!(r.variant, 3 * 4 + 2 - 4, "lord * 4 + rotation - 4");
    assert_eq!(r.body_index(), 11, "and the body is variant + 1, past the label");
}

// -------------------------------------------------------------- 4. the save

/// **A save taken with messages queued keeps them.**
///
/// This is not tidiness. The endings are shown one at a time now, so a person
/// really can save on the campaign map with three obituaries behind the one on
/// screen — and a save that dropped them would be a save that can never be won.
#[test]
fn a_save_carries_the_ring_and_the_window() {
    let (mut g, a, mut m) = world();
    post(&mut g, notice(0x92));
    post(&mut g, notice(0x93));
    post(&mut g, Record { to: 1, from: 3, group: 180, category: category::ALLIANCE_PROMPT, ..Record::default() });
    open_the_scroll(&mut m, &mut g, &a);

    let bytes = l2_game::save::encode(&g);
    let back = l2_game::save::decode(&bytes, l2_kingdom::tables::Tables::DEFAULT)
        .expect("it reads back");

    assert_eq!(back.messages.open(), g.messages.open(), "the record on screen");
    assert_eq!(back.messages.timer(), g.messages.timer(), "with its timer");
    assert_eq!(back.messages.waiting(), g.messages.waiting(), "and the two behind it");
    assert_eq!(back.messages.waiting().len(), 2);
}

// ------------------------------------------------- 5. against real artwork
//
// Five defects in one week existed only against the real files — a caret that
// drew nothing because it used `_` and `Ui_DrawText` maps that byte to a space,
// and text landing on top of itself because a helper returned an advance with
// the real font and an absolute position without one. Nothing below has a
// threshold in it: each is an equality or an inequality between two canvases,
// which is what `docs/agents.md` asks for instead.

/// **Drawing the scroll twice over itself changes nothing.**
///
/// Every glyph and every frame is an opaque blit, so a second draw is a no-op —
/// *but only if the first one happened*. It is the build stamp's assertion, and
/// it needs no threshold and no knowledge of what else is on the page.
#[test]
fn the_scroll_paints_and_painting_it_again_changes_nothing() {
    let (mut g, a, mut m) = painted!();
    // **The map alone first.** Comparing against a blank canvas would measure
    // the campaign map underneath and pass with the window deleted, which is the
    // build stamp's mistake exactly.
    let bare = painting(&mut g, &a, &mut m);

    post(&mut g, notice(146));
    open_the_scroll(&mut m, &mut g, &a);
    let once = painting(&mut g, &a, &mut m);
    assert!(once.diff_count(&bare) > 0, "the window is on top of the map");

    let mut twice = once.clone();
    {
        let ctx = Ctx { game: &mut g, assets: &a };
        m.draw(&ctx, &mut twice);
    }
    assert_eq!(once.diff_count(&twice), 0, "an opaque blit drawn twice is the same picture");
}

/// **The BODY of the message is drawn, and it is the message's own.**
///
/// Two records of the **same group** and different variants: the heading is the
/// group's own label in both, so the only thing that can differ is the body
/// string `Eng_DrawString(group, variant + 1)` picks. Group 194 holds a label
/// and sixteen laments, which is why it is the one used.
///
/// If the body draw did nothing the two pictures would be identical, and this is
/// the assertion that says so — with no threshold and no knowledge of where the
/// text lands.
#[test]
fn two_variants_of_one_group_draw_two_different_bodies() {
    let first = {
        let (mut g, a, mut m) = painted!();
        post(&mut g, Record { to: 0, group: 194, variant: 0, ..Record::default() });
        open_the_scroll(&mut m, &mut g, &a);
        painting(&mut g, &a, &mut m)
    };
    let second = {
        let (mut g, a, mut m) = painted!();
        post(&mut g, Record { to: 0, group: 194, variant: 5, ..Record::default() });
        open_the_scroll(&mut m, &mut g, &a);
        painting(&mut g, &a, &mut m)
    };
    assert!(
        first.diff_count(&second) > 0,
        "one group, two variants, one picture -- the body was not drawn",
    );
}

/// **The corner button and the answer buttons are really on the canvas.**
///
/// Not a pixel count and not a comparison against a different message: draw the
/// frame, copy it, draw *those two things again* on the copy, and require the
/// two canvases to be identical. A sprite is an opaque blit, so a second draw
/// over itself changes nothing — but only if the first one happened.
///
/// Ablating `pen.ok_button` moves 500-odd pixels here and nothing anywhere else
/// in this file, which is the property the build-stamp test was written to have
/// and the pixel count it replaced did not.
#[test]
fn the_scrolls_clickable_pictures_are_drawn_where_the_hit_tests_look() {
    let (mut g, a, mut m) = painted!();
    post(&mut g, alliance_offer(3));
    open_the_scroll(&mut m, &mut g, &a);

    let once = painting(&mut g, &a, &mut m);
    let mut again = once.clone();
    let record = *g.messages.open().expect("a record");
    {
        let ctx = Ctx { game: &mut g, assets: &a };
        l2_game::screens::message::repaint_clickables(&ctx, &mut again, &record);
    }
    assert_eq!(
        once.diff_count(&again),
        0,
        "the corner button and the two mailed hands were already painted",
    );
}

/// The same for a plain notice, which has the corner button and no widgets.
#[test]
fn a_notices_corner_button_is_drawn_too() {
    let (mut g, a, mut m) = painted!();
    post(&mut g, notice(146));
    open_the_scroll(&mut m, &mut g, &a);

    let once = painting(&mut g, &a, &mut m);
    let mut again = once.clone();
    let record = *g.messages.open().expect("a record");
    {
        let ctx = Ctx { game: &mut g, assets: &a };
        l2_game::screens::message::repaint_clickables(&ctx, &mut again, &record);
    }
    assert_eq!(once.diff_count(&again), 0);
}

/// **The window frame itself is painted**, and not only the words on it.
///
/// The probe is the window's own top edge — inside the border and outside every
/// glyph and every button — so it is about `FUN_004093E0` and nothing else.
/// Ablating the text or either button leaves it alone; ablating the frame lets
/// the campaign map show through and this goes red.
///
/// The coordinates are pinned from the decompilation, not read back from
/// `frame_of`: `Msg_DrawWindow`'s category-`0x0B` arm is
/// `FUN_004093E0(0x10, 0x80, 0x1C, 0x0F)`.
#[test]
fn the_window_frame_covers_the_map_under_it() {
    let (mut g, a, mut m) = painted!();
    let bare = painting(&mut g, &a, &mut m);
    post(&mut g, alliance_offer(3));
    open_the_scroll(&mut m, &mut g, &a);
    let with_window = painting(&mut g, &a, &mut m);

    let (x, y) = (0x10usize, 0x80usize);
    let differs = (0..16)
        .filter(|dx| bare.at(x + dx, y) != with_window.at(x + dx, y))
        .count();
    assert!(
        differs > 0,
        "sixteen pixels of the window's top edge and not one of them changed -- \
         the frame was not drawn, or it was drawn somewhere else",
    );
}
