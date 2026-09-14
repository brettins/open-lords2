#![allow(unused_imports)]
use super::*;
use crate::screens::setup::skirmish::muster;
use l2_sim::runner::{BattleRunner, Muster};

impl SetupScreen {
    /// Page 12's state, for a test that wants to see what an arm did.
    pub fn skirmish(&self) -> &crate::screens::setup::skirmish::Skirmish {
        &self.skirmish
    }

    /// Stand in for `Troops_Load` (`0x0042AC0C`): the parse of `TROOPS2.ENG`
    /// is not built, so the table is handed in.
    pub fn set_troops(&mut self, t: crate::screens::setup::skirmish::TroopsTable) {
        self.troops = t;
    }

    /// The names page 13 lists — `DAT_004E8790`, which a directory scan fills.
    pub fn set_skirmish_files(&mut self, files: Vec<String>) {
        self.skirmish_files = files;
    }

    /// ***Go*** — `FUN_0043D5B7`, whose body is `FUN_0043F304`, `Msg_Reset`,
    /// `FUN_0042BA40`, `FUN_0042C5AD`, `Battle_Start` (`0x004778A0`) and
    /// `g_battleState = 3`.
    ///
    /// What arrives at `Battle_Start` was put there on the way **in**:
    /// `Skirmish_Setup` (`0x0042B7F7`) runs when page 12 opens, not when this
    /// button is pressed — it pairs `g_localPlayer` against `DAT_0056D5CC`,
    /// sets `g_battleArmyA = 1` and `g_battleArmyB = 2`, loads `BATTLES.ENG`
    /// and the troops table, raises `DAT_0057A0F0` and calls
    /// `Skirmish_FillArmies`. Every arm on the page then re-runs the fill, so
    /// the two armies are always the ones the page is showing. Ours is a pure
    /// function of the page's state, so it runs here, once.
    ///
    /// The two slots are `g_battleArmyA` and `g_battleArmyB` themselves, and
    /// no campaign unit stands behind either: the skirmish flag is what stops
    /// the end of the battle writing casualties back to units 1 and 2 of a
    /// kingdom that is not playing.
    pub(crate) fn go_skirmish(&mut self, ctx: &mut Ctx) -> Transition {
        let (mine, theirs) = self.skirmish.fill_armies(&self.troops);
        if mine.men == 0 || theirs.men == 0 {
            // No troops table, no armies — `Troops_Load` returns 0 on an
            // install with no `TROOPS2.ENG` and the original raises the
            // battlefield anyway, with two empty musters. `BattleRunner`
            // refuses an empty side, so the button says nothing instead.
            return Transition::Stay;
        }
        let (my_slot, their_slot) = self.skirmish.slots();
        let seed = self.skirmish.seed();
        let (mine_t, theirs_t) = (muster(&mine.counts), muster(&theirs.counts));
        let local = ctx.game.player;
        let foe = crate::screens::ratings::skirmish_opponent(local);
        let a = Muster { troops: &mine_t, owner: local, human: true };
        let b = Muster { troops: &theirs_t, owner: foe, human: false };
        // `FUN_0042BA40` puts whoever attacks in `g_battleArmyA`, which is the
        // side `Battle_Start` deploys first.
        let runner = if self.skirmish.local_attacks {
            BattleRunner::deploy_muster(crate::batfield::field(seed), seed, a, b)
        } else {
            BattleRunner::deploy_muster(crate::batfield::field(seed), seed, b, a)
        };
        let mut battle =
            crate::battlefield::LiveBattle::new(runner, my_slot, their_slot, 0, None, local, 1);
        battle.skirmish = true;
        ctx.game.battle = Some(Box::new(battle));
        Transition::Push(ScreenId::Battlefield)
    }
}
