//! The one command list every state-changing player action goes through.
//! `docs/netcode.md` §2, §5 step 1; the codec is D-10.

mod codec;
mod orders;
mod run;
#[cfg(test)]
mod tests;

pub use orders::*;
pub use run::Castled;

use crate::game::Game;
use crate::screen::Ctx;
use l2_net::PlayerSlot;

pub trait Order: Clone + Into<GameCommand> {
    type Reply;
    fn run(self, game: &mut Game) -> Self::Reply;
}

macro_rules! commands {
    ($($tag:literal $name:ident),* $(,)?) => {
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub enum GameCommand { $($name($name)),* }

        $(impl From<$name> for GameCommand {
            fn from(o: $name) -> Self { GameCommand::$name(o) }
        })*

        impl GameCommand {
            pub fn tag(&self) -> u8 {
                match self { $(GameCommand::$name(_) => $tag),* }
            }

            fn run(self, game: &mut Game) {
                match self { $(GameCommand::$name(o) => { let _ = o.run(game); }),* }
            }

            fn decode_tagged(
                tag: u8,
                at: usize,
                input: &mut l2_net::Reader<'_>,
            ) -> Result<GameCommand, l2_net::CodecError> {
                use codec::Wire;
                match tag {
                    $($tag => Ok(GameCommand::$name($name::get(input)?)),)*
                    tag => Err(l2_net::CodecError::BadTag { tag, expected: "game command", at }),
                }
            }

            fn encode_body(&self, out: &mut l2_net::Canonical) {
                use codec::Wire;
                match self { $(GameCommand::$name(o) => o.put(out)),* }
            }
        }
    };
}

commands! {
    0 OrderCastle,
    1 CombineArmies,
    2 ValidateSiegeLink,
    3 Trade,
    4 PayForHelp,
    5 FormAlliance,
    6 OrderEngine,
    7 LiftSiege,
    8 SendSupplies,
    9 ToggleOption,
    10 SetQuirk,
    11 RankRealms,
    12 SetTaxRate,
    13 SetRation,
    14 SetRationSplit,
    15 RaiseArmy,
    16 SplitArmy,
    17 LeaveCastle,
    18 DisbandArmy,
    19 MoveUnit,
    20 MoveLabour,
    21 BalanceLabour,
    22 BalanceAllLabour,
    23 PaintField,
    24 ToggleIndustry,
}

/// The slot this machine issues from. `[I]` one seat until the lobby's
/// `Start` assigns seats (row net-kingdom-lockstep).
pub fn local() -> PlayerSlot {
    PlayerSlot::new(0)
}

impl Game {
    /// `[I]` only the local seat maps to a realm until seats exist.
    pub fn realm_of(&self, slot: PlayerSlot) -> Option<u8> {
        (slot == local()).then_some(self.player)
    }

    /// Commands from an unseated slot are dropped.
    pub fn apply(&mut self, slot: PlayerSlot, command: &GameCommand) {
        let Some(realm) = self.realm_of(slot) else { return };
        let me = std::mem::replace(&mut self.player, realm);
        command.clone().run(self);
        self.player = me;
    }

    /// Single player applies at once; the reply is what the screen reads.
    pub fn issue<O: Order>(&mut self, order: O) -> O::Reply {
        order.run(self)
    }
}

impl Ctx<'_> {
    pub fn issue<O: Order>(&mut self, order: O) -> O::Reply {
        self.game.issue(order)
    }
}
