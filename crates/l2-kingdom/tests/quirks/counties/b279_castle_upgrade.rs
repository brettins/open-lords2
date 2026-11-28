use super::*;
use l2_kingdom::county::County;
use l2_kingdom::industry::order_castle;
use l2_kingdom::realm::Realm;

fn owned() -> County {
    let mut c = County::new();
    c.owner = 1;
    c.population = 2_000;
    c
}

/// Orders each type in turn on empty stores; returns (owed wood, owed stone, realm wood).
fn ordered_in_a_row(types: &[u8], quirks: Quirks) -> (i32, i32, i32) {
    let (mut c, mut r) = (owned(), Realm::new());
    for &ty in types {
        assert!(order_castle(T, &mut c, &mut r, ty, quirks));
    }
    (c.castle_wood_owed, c.castle_stone_owed, r.wood)
}

/// Issue #21: stone then royal at once. Ablation: `castle_bill` not adding `+0x1D0`/`+0x1D4`.
#[test]
fn b279_an_unpaid_stone_castle_counts_towards_a_royal_one_only_unless_fixed() {
    let (faithful, fixed) = pair(Quirk::UpgradeForgivesUnpaidCastle);
    assert_eq!(ordered_in_a_row(&[4, 5], faithful), (400, 1_000, 0), "reproduced");
    assert_eq!(ordered_in_a_row(&[4, 5], fixed), (800, 3_000, 0), "fixed: the royal bill");
}

/// Issue #21: palisade, motte, keep, stone castle in a row on empty stores.
#[test]
fn b279_the_ladder_refunds_wood_never_paid_only_unless_fixed() {
    let (faithful, fixed) = pair(Quirk::UpgradeForgivesUnpaidCastle);
    assert_eq!(ordered_in_a_row(&[1, 2, 3, 4], faithful), (0, 1_000, 400), "reproduced");
    assert_eq!(ordered_in_a_row(&[1, 2, 3, 4], fixed), (400, 2_000, 0), "fixed: the stone castle bill");
}

/// A finished motte still pays 600 wood back towards a keep when fixed.
/// Ablation: `castle_bill`'s `castle_degraded != 0` gate.
#[test]
fn b279_fixed_still_refunds_a_finished_castle() {
    let (_, fixed) = pair(Quirk::UpgradeForgivesUnpaidCastle);
    let (mut c, mut r) = (owned(), Realm::new());
    c.castle_type = 2;
    (c.castle_wood_owed, c.castle_stone_owed) = (5, 5);
    r.stone = 10_000;
    assert!(order_castle(T, &mut c, &mut r, 3, fixed));
    assert_eq!((r.wood, r.stone), (600, 10_000 - 920));
}

/// A besieged upgrade still owes its unpaid bill when fixed (00470000.c:3550).
/// Ablation: `castle_bill` testing `castle_degraded == 1`.
#[test]
fn b279_fixed_carries_a_besieged_builds_debt() {
    let (_, fixed) = pair(Quirk::UpgradeForgivesUnpaidCastle);
    let (mut c, mut r) = (owned(), Realm::new());
    assert!(order_castle(T, &mut c, &mut r, 4, fixed));
    c.castle_degraded = l2_kingdom::siege::CASTLE_DEGRADED_DAMAGED;
    assert!(order_castle(T, &mut c, &mut r, 5, fixed));
    assert_eq!((c.castle_wood_owed, c.castle_stone_owed), (800, 3_000), "the royal bill");
}

#[test]
fn b279_is_always_fixed_online() {
    assert!(Quirk::UpgradeForgivesUnpaidCastle.fixed_online());
    assert!(Quirks::FAITHFUL.online().is_fixed(Quirk::UpgradeForgivesUnpaidCastle));
}
