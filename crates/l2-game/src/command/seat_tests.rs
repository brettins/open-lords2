use super::tests::seated_question;

/// Ablation: the `(Some(_), 1) => 2` arm of `Question::choice_for` (the
/// defender's seat draws the thumbs and *"Will you take the field?"*).
#[test]
fn the_defending_seat_sees_that_its_opponent_has_the_choice() {
    let g = seated_question(1, 2);
    let q = crate::turn::pending_question(&g).expect("asked");
    assert_eq!(q.choice_for(&g, 1), 1, "the attacker's seat chooses");
    assert_eq!(q.choice_for(&g, 2), 2, "group 80 index 2 on the defender's seat");
    assert_eq!(q.choice_for(&g, 3), 0, "a bystander");
    let mut g = g;
    g.seats.truncate(1);
    assert_eq!(q.choice_for(&g, 2), q.choice_owner, "single player reads the world's value");
}

/// C260. Ablations: the slot-0 refusal in `ToComputer::run`; `game.seats.retain`.
#[test]
fn only_the_host_hands_a_seat_to_the_computer_and_the_seat_is_dropped() {
    use super::{GameCommand, ToComputer};
    use l2_net::PlayerSlot;
    let mut g = seated_question(1, 2);
    g.kingdom.realms[1].is_human = true;
    g.kingdom.realms[2].is_human = true;
    let order: GameCommand = ToComputer { realm: 1 }.into();
    g.apply(PlayerSlot::new(1), &order);
    assert!(g.kingdom.realms[1].is_human, "a joiner gave the host's realm away");
    g.apply(PlayerSlot::new(0), &ToComputer { realm: 2 }.into());
    assert!(!g.kingdom.realms[2].is_human, "the computer plays realm 2");
    assert_eq!(g.seats, vec![(PlayerSlot::new(0), 1)], "its slot's orders are dropped from now");
}
