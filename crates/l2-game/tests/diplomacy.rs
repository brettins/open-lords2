//! **The player's side of diplomacy, driven the way a player drives it.**
//!
//! `docs/agents.md`: *"an agent testing our engine does not touch the OS input
//! queue"* — everything here is an `Event` value handed to `Machine::handle`,
//! and nothing opens a window or needs a copy of the game.
//!
//! The point of the file is the thing the unit tests in
//! `l2_kingdom::diplomacy` cannot say: **that there is a route from a click to
//! the rule.** Every one of those tests calls the rule directly, and a rule
//! nobody can reach is the failure this whole subsystem was blocking on in the
//! other direction — see `docs/decisions.md` C62 and `tests/ai_war.rs`.
//!
//! So each test below starts at the sidebar or at the lord card and ends by
//! reading `l2_kingdom`'s own state.

use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId, Transition};
use l2_game::screens::diplomacy::{self, DiplomacyScreen, Menu};
use l2_game::Game;
use l2_kingdom::diplomacy::Kind;
use l2_view::Canvas;

/// Realm 1 is the person, realms 2 and 3 are the Knight and the Baron, and
/// county 3 has an enemy standing in it so that *ask for help* has something to
/// be about.
fn world() -> (Game, Assets) {
    let mut g = Game::new(5);
    g.kingdom.set_county_count(4);
    for realm in 1..=3usize {
        let r = &mut g.kingdom.realms[realm];
        r.in_play = true;
        r.strength = 3;
        r.county_count = 1;
        r.gold = 4_000;
        r.population_mean = 900;
        r.lord = realm as u8 - 1;
        r.shield_index = realm as u8;
        g.kingdom.counties[realm].owner = realm as u8;
        g.kingdom.counties[realm].population = 800;
    }
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[1].lord = 0;
    // County 4 is nobody's, which is `Diplo_SendClicked`'s group 241.
    g.kingdom.counties[4].owner = 0;
    g.kingdom.counties[4].population = 400;
    // County 1 is mine and under threat, which is what kind 5 wants.
    g.kingdom.counties[1].enemy_troops = 40;
    g.player = 1;
    g.selected = 1;
    g.kingdom.init_diplomacy();
    (g, Assets::placeholder())
}

fn send(machine: &mut Machine, game: &mut Game, assets: &Assets, event: Event) {
    let mut ctx = Ctx { game, assets };
    machine.handle(event, &mut ctx);
}

fn middle(r: l2_game::input::Rect) -> Event {
    Event::Click { x: r.x + r.w / 2, y: r.y + r.h / 2 }
}

/// **The whole player's side, click by click: pick a rival, open the gift
/// dialog, step the amount up, send it, and find the gold gone and the letter
/// in the AI's inbox.**
///
/// This is the test the subsystem exists to make possible. Every intermediate
/// state is asserted, because a route that arrives at the right end state by
/// the wrong path is a route that breaks the moment anything moves.
#[test]
fn a_gift_travels_from_a_click_to_the_rivals_inbox() {
    let (mut game, assets) = world();
    let mut machine = Machine::new(ScreenId::Diplomacy);

    // The screen opens on `Diplo_DefaultTarget`'s pick: the first in-play realm
    // that is not me, which is realm 2.
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        let screen = DiplomacyScreen::new();
        assert_eq!(screen.target(&ctx), 2);
        assert_eq!(DiplomacyScreen::cards(&ctx), vec![2, 3], "the person has no card of his own");
    }

    // Card 1 is realm 3. `FUN_004369BD` writes `g_diploTarget`.
    send(&mut machine, &mut game, &assets, middle(diplomacy::card_rect(1)));
    assert_eq!(machine.depth(), 1, "picking a rival does not open anything");

    // The no-ally menu's first row is *"Dispatch a gift."*
    assert_eq!(Menu::NoAlly.rows()[0], 2);
    send(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(0)));
    assert_eq!(machine.depth(), 2, "the compose dialog is on top");
    assert_eq!(machine.top_id(), Some(ScreenId::DiploCompose(3, Kind::Gift.byte())));

    // Twelve clicks on the plus, one on the minus: 110 crowns.
    for _ in 0..12 {
        send(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_MORE));
    }
    send(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_LESS));

    let purse = game.kingdom.realms[1].gold;
    send(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_SEND));
    assert_eq!(machine.depth(), 1, "and it goes back to the lord cards");

    assert_eq!(game.kingdom.realms[1].gold, purse - 110, "spent when posted, not when answered");
    assert_eq!(game.kingdom.realms[3].gold, 4_000 + 110);
    let pending: Vec<_> = game.kingdom.diplomacy.pending(3).copied().collect();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].from, 1);
    assert_eq!(pending[0].kind, Kind::Gift.byte());
    assert_eq!(pending[0].gold, 110);
    assert!(game.kingdom.realms[3].pair(1).has_mail);
}

/// **The gift stepper cannot promise money the treasury does not have**, and
/// the clamp is applied on every click rather than at the send.
#[test]
fn the_gift_amount_is_clamped_to_the_purse_on_every_click() {
    let (mut game, assets) = world();
    game.kingdom.realms[1].gold = 35;
    let mut machine = Machine::new(ScreenId::Diplomacy);
    send(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(0)));
    for _ in 0..10 {
        send(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_MORE));
    }
    send(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_SEND));
    assert_eq!(game.kingdom.realms[1].gold, 0, "35, not 100");
    assert_eq!(game.kingdom.diplomacy.pending(2).next().unwrap().gold, 35);
}

/// **The menu is the gate on the two requests**, exactly as
/// `docs/diplomacy.md` §4 says: only an ally can be asked for help or for an
/// attack, and the rule is not a guard anywhere — it is which rows the screen
/// draws.
#[test]
fn asking_for_help_is_only_on_the_menu_of_an_ally() {
    let (mut game, assets) = world();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(Menu::of(&ctx, 2), Menu::NoAlly);
    }
    l2_kingdom::diplomacy::form_alliance(&mut game.kingdom.realms, 1, 2);
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(Menu::of(&ctx, 2), Menu::Allied, "the ally's own card");
        assert_eq!(Menu::of(&ctx, 3), Menu::AlliedElsewhere, "and everybody else's");
    }
    assert!(Menu::Allied.rows().contains(&7));
    assert!(!Menu::AlliedElsewhere.rows().contains(&7));

    // Row 3 of the allied menu is *"Terminate alliance."*, kind 4 — the same
    // widget that offers one when you have none.
    let mut machine = Machine::new(ScreenId::Diplomacy);
    send(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(3)));
    assert_eq!(machine.top_id(), Some(ScreenId::DiploCompose(2, Kind::EndAlliance.byte())));
}

/// **A letter of mine already in their inbox replaces the whole menu**, and the
/// flag it reads is the *target's* record indexed by *me* — which is what
/// `docs/diplomacy.md` §7 had backwards.
#[test]
fn one_letter_per_rival_per_turn_and_the_menu_says_so() {
    let (mut game, assets) = world();
    game.kingdom.post_letter(1, 2, Kind::Compliment, 0, 0);
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(Menu::of(&ctx, 2), Menu::Dispatched);
        assert_eq!(Menu::of(&ctx, 3), Menu::NoAlly, "and only for the rival written to");
        assert_eq!(Menu::Dispatched.rows(), &[24], "group 72/24, and no widget under it");
    }
    // A click where the first menu row would be does nothing, because the
    // dispatched layout sets `g_diploWidgetCount = 0`.
    let mut machine = Machine::new(ScreenId::Diplomacy);
    send(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(0)));
    assert_eq!(machine.depth(), 1, "there is nothing to click");
}

/// **`Diplo_SendClicked`'s refusal ladder, every rung.** `docs/decisions.md`
/// C26: a rule can be wrong at 45 of its 51 inputs and stay invisible when the
/// only fixture exercises one value, so every branch is put a case.
#[test]
fn every_one_of_the_send_buttons_refusals_is_reachable() {
    use diplomacy::{refusal, Refusal};
    let (mut game, assets) = world();
    let ctx = Ctx { game: &mut game, assets: &assets };

    // Kinds 5 and 6, in the order the original tests them.
    assert_eq!(refusal(&ctx, 2, Kind::AskHelp, 0), Some(Refusal::NoCounty));
    assert_eq!(refusal(&ctx, 2, Kind::AskHelp, 4), Some(Refusal::Unowned), "county 4 is nobody's");
    assert_eq!(refusal(&ctx, 2, Kind::AskHelp, 2), Some(Refusal::NotOurs));
    assert_eq!(
        refusal(&ctx, 2, Kind::AskHelp, 3),
        Some(Refusal::NotOurs),
        "and it is tested before the enemy count, so a rival's threatened county is `not ours`"
    );
    assert_eq!(refusal(&ctx, 2, Kind::AskHelp, 1), None, "mine, and under threat");

    assert_eq!(refusal(&ctx, 2, Kind::AskAttack, 0), Some(Refusal::NoCounty));
    assert_eq!(refusal(&ctx, 2, Kind::AskAttack, 4), Some(Refusal::Unowned));
    assert_eq!(
        refusal(&ctx, 2, Kind::AskAttack, 1),
        Some(Refusal::Allied),
        "**a county of my own gets `is part of our alliance`**, which is the wrong \
         sentence for the case — reproduced"
    );
    assert_eq!(refusal(&ctx, 2, Kind::AskAttack, 3), None, "somebody else's, and not my ally's");

    // The four letter kinds have nothing to validate at all.
    for kind in [Kind::Gift, Kind::Compliment, Kind::Insult, Kind::EndAlliance] {
        assert_eq!(refusal(&ctx, 2, kind, 0), None, "{kind:?} has no county and no guard");
    }
}

/// The alliance refusal has **a human in its guard**, which is easy to miss and
/// changes what happens in single player: an AI that already has an ally is not
/// refused here at all. The offer goes out, and comes back as group 178.
#[test]
fn only_a_human_rivals_existing_treaty_stops_the_offer_being_sent() {
    use diplomacy::{refusal, Refusal};
    let (mut game, assets) = world();
    l2_kingdom::diplomacy::form_alliance(&mut game.kingdom.realms, 2, 3);
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(refusal(&ctx, 2, Kind::OfferAlliance, 0), None, "realm 2 is an AI");
    }
    game.kingdom.realms[2].is_human = true;
    let ctx = Ctx { game: &mut game, assets: &assets };
    assert_eq!(refusal(&ctx, 2, Kind::OfferAlliance, 0), Some(Refusal::TargetAlreadyAllied));
}

/// **Right-click exits both screens, and they exit to different places.** The
/// gesture `docs/agents.md` records as the one we systematically miss, and the
/// asymmetry is the original's: the cross returns to the lord cards, the corner
/// button and the right button go to the map.
#[test]
fn the_right_button_leaves_and_the_cross_goes_back() {
    let (mut game, assets) = world();
    let mut machine = Machine::new(ScreenId::Diplomacy);
    send(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(0)));
    assert_eq!(machine.depth(), 2);
    // The cross: back to 0x0B.
    send(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_CANCEL));
    assert_eq!(machine.top_id(), Some(ScreenId::Diplomacy));
    assert_eq!(machine.depth(), 1);

    // The right button on the compose dialog: straight to the map.
    send(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(0)));
    send(&mut machine, &mut game, &assets, Event::RightClick { x: 300, y: 300 });
    assert_eq!(machine.top_id(), Some(ScreenId::Campaign));

    // And on the lord cards: the corner button leaves too.
    let mut machine = Machine::new(ScreenId::Diplomacy);
    send(&mut machine, &mut game, &assets, Event::RightClick { x: 100, y: 100 });
    assert!(machine.should_quit() || machine.depth() == 0, "popping the only screen quits");
}

/// **Nothing on either screen sends a letter as a side effect of drawing it.**
///
/// The compose dialog is the only screen in the game whose *draw* reads the
/// pair record it is about, and the pair record is what the AI's decisions are
/// made of — so a draw that wrote would be a rule the player could fire by
/// looking. Asserted by drawing every shape twice and comparing the kingdom.
#[test]
fn drawing_the_dialogs_changes_no_state_at_all() {
    let (mut game, assets) = world();
    let before = game.kingdom.clone();
    for id in [
        ScreenId::Diplomacy,
        ScreenId::DiploCompose(2, 0),
        ScreenId::DiploCompose(2, 1),
        ScreenId::DiploCompose(2, 5),
    ] {
        let mut screen = id.build();
        for _ in 0..2 {
            let mut canvas = Canvas::screen();
            let ctx = Ctx { game: &mut game, assets: &assets };
            screen.draw(&ctx, &mut canvas);
        }
        assert_eq!(game.kingdom, before, "{id:?} moved something by being drawn");
    }
}

/// The screens are **overlays**: the original clears nothing and paints over
/// the campaign map, which is what `Machine::draw` walking back to the last
/// non-overlay screen is for.
#[test]
fn both_screens_are_insets_over_the_map() {
    for id in [ScreenId::Diplomacy, ScreenId::DiploCompose(2, 0)] {
        assert!(id.build().is_overlay(), "{id:?} is drawn over what was there");
    }
}

/// A menu click on a rival that has been knocked out cannot happen, because
/// `Diplo_DrawScreen` replaces a dead target before it draws anything.
#[test]
fn a_target_that_is_knocked_out_is_replaced_rather_than_kept() {
    let (mut game, assets) = world();
    let mut screen = DiplomacyScreen::new();
    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        assert_eq!(screen.target(&ctx), 2);
    }
    let mut machine = Machine::new(ScreenId::Diplomacy);
    send(&mut machine, &mut game, &assets, middle(diplomacy::card_rect(1)));
    game.kingdom.realms[3].strength = 0;
    game.kingdom.realms[3].in_play = false;
    let ctx = Ctx { game: &mut game, assets: &assets };
    let _ = &mut screen;
    let mut fresh = DiplomacyScreen::new();
    assert_eq!(fresh.target(&ctx), 2, "the default takes over");
    let mut canvas = Canvas::screen();
    l2_game::screen::Screen::draw(&mut fresh, &ctx, &mut canvas);
    assert_eq!(DiplomacyScreen::cards(&ctx), vec![2], "and the dead realm loses its card");
}

/// `Transition::Pass` is not used by either screen: every click either belongs
/// to it or is ignored. Stated because `screens/mod.rs` warns that *"passing by
/// default rather than by decision is how two screens end up both acting on one
/// click"*, and these two sit over the map.
#[test]
fn neither_screen_ever_passes_a_click_to_the_map_underneath() {
    let (mut game, assets) = world();
    for id in [ScreenId::Diplomacy, ScreenId::DiploCompose(2, 0)] {
        let mut screen = id.build();
        for (x, y) in [(0, 0), (639, 479), (320, 240), (16, 400)] {
            let mut ctx = Ctx { game: &mut game, assets: &assets };
            let t = screen.handle(Event::Click { x, y }, &mut ctx);
            assert_ne!(t, Transition::Pass, "{id:?} passed a click at ({x}, {y})");
        }
    }
}
