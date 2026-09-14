#![allow(unused_imports)]
use super::*;
use super::fight::*;
use super::siege::*;
use super::tests::*;
use l2_kingdom::battle::{self, Aftermath, Settlement, Verdict};
use l2_kingdom::conquest::Attack;
use l2_kingdom::kingdom::Kingdom;
use l2_kingdom::unit::TROOP_TYPES;
use l2_sim::runner::{blank_field, BattleRunner, Muster};
use l2_sim::{End, Troop, SIDE_A, SIDE_B};

/// One army's seven campaign troop counts — peasant, crossbowman, maceman,
/// swordsman, pikeman, archer, knight, in the order every table in the game
/// agrees on.
pub type Roster = [i32; TROOP_TYPES];

/// **One unit's seven counts with its mercenary band folded into its own troop
/// type**, which is how the original's roster painter reads them.
///
/// `FUN_004224E7` does `if (unit.mercTroop == row) count += unit.mercMen` on
/// every row of *both* its modes, and mode 0 stashes the folded figure into
/// `DAT_00568420` / `DAT_0056843C` for mode 1 to print in parentheses. It has
/// to: `Mercenary_Hire` (`0x004AC7F3`) adds the band to `+0x168` and leaves
/// `+0x16C` alone, so a band is in the total and in no row until the painter
/// puts it in one. A mercenary company shows up *inside* the swordsmen rather
/// than beside them. **[V]**
pub fn roster_of(u: &l2_kingdom::unit::Unit) -> Roster {
    let mut rows = u.troops;
    if let Some(band) = u.mercenaries {
        rows[band.troop.index()] += band.men();
    }
    rows
}

impl BattleReport {
    /// The realm that held the field.
    pub fn winner_owner(&self) -> u8 {
        if self.verdict.attacker_won {
            self.attacker_owner
        } else {
            self.defender_owner
        }
    }

    /// The realm that did not.
    pub fn loser_owner(&self) -> u8 {
        if self.verdict.attacker_won {
            self.defender_owner
        } else {
            self.attacker_owner
        }
    }

    /// **Which of `L2.eng` group 82's seven heading/body pairs this battle
    /// draws** for `local_player` — [`l2_kingdom::battle::outcome`], which is
    /// `FUN_00478419`.
    ///
/// It is a method because the answer depends on who is
    /// looking: the same battle is *won* to one peer, *lost* to the other and
    /// [`Outcome::Bystander`](l2_kingdom::battle::Outcome::Bystander) to a
    /// third. A field on the report would have to pick one of them, and in a
    /// lockstep game every peer holds the same report.
    pub fn outcome(&self, local_player: u8) -> l2_kingdom::battle::Outcome {
        battle::outcome(
            self.verdict,
            self.is_siege,
            local_player,
            self.winner_owner(),
            self.loser_owner(),
        )
    }
}

