#![allow(unused_imports)]
use super::*;
use super::dialog_flow::*;
use super::letters::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, Screen, ScreenId, Transition};
use l2_game::screens::diplomacy::{self, DiplomacyScreen, Menu};
use l2_game::Game;
use l2_kingdom::diplomacy::Kind;
use l2_view::Canvas;

#[test]
fn a_gift_travels_from_a_click_to_the_rivals_inbox() {
    let (mut game, assets) = world();
    let mut machine = Machine::new(ScreenId::Diplomacy);

    {
        let ctx = Ctx { game: &mut game, assets: &assets };
        let screen = DiplomacyScreen::new();
        assert_eq!(screen.target(&ctx), 2);
        assert_eq!(DiplomacyScreen::cards(&ctx), vec![2, 3], "the person has no card of his own");
    }

    // Card 1 is realm 3. `FUN_004369BD` writes `g_diploTarget`.
    send(&mut machine, &mut game, &assets, middle(diplomacy::card_rect(1)));
    assert_eq!(machine.depth(), 1, "picking a rival does not open anything");

    assert_eq!(Menu::NoAlly.rows()[0], 2);
    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(0)));
    assert_eq!(machine.depth(), 2, "the compose dialog is on top");
    assert_eq!(machine.top_id(), Some(ScreenId::DiploCompose(3, Kind::Gift.byte())));

    for _ in 0..12 {
        send(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_MORE));
    }
    send(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_LESS));

    let purse = game.kingdom.realms[1].gold;
    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_SEND));
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

#[test]
fn the_gift_amount_is_clamped_to_the_purse_on_every_click() {
    let (mut game, assets) = world();
    game.kingdom.realms[1].gold = 35;
    let mut machine = Machine::new(ScreenId::Diplomacy);
    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(0)));
    for _ in 0..10 {
        send(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_MORE));
    }
    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::GIFT_SEND));
    assert_eq!(game.kingdom.realms[1].gold, 0, "35, not 100");
    assert_eq!(game.kingdom.diplomacy.pending(2).next().unwrap().gold, 35);
}

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

    let mut machine = Machine::new(ScreenId::Diplomacy);
    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(3)));
    assert_eq!(machine.top_id(), Some(ScreenId::DiploCompose(2, Kind::EndAlliance.byte())));
}

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
    let mut machine = Machine::new(ScreenId::Diplomacy);
    press_and_wait(&mut machine, &mut game, &assets, middle(diplomacy::menu_widget(0)));
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

