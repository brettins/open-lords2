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
