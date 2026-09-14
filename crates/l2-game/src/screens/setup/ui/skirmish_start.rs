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

    /// **`SaveLoad_Scroll` (`0x00434346`) under list 2** — the skirmish box's
    /// arrows, hotspot id −1 and +1: `g_fileListTop += delta`, floored at 0,
    /// then walked back one row when it passes `DAT_004EB25C - 5` and put
    /// back to 0 outright when the list is shorter than ten names. The
    /// half-page clamp is the original's, `docs/screens-county.md`.
    ///
    /// Public because page 13's arrow widget record is not in the table we
    /// have read: `node tools/oracle/widgets.js` names the two arrows of the
    /// *save/load* box (`0x004DDDA8` and `0x004DDDC0`, list 1) and we could
    /// not find the skirmish box's pair, so nothing on the page reaches this
    /// yet. Finding the record is the open part.
    pub fn scroll_skirmish_files(&mut self, delta: i32) {
        let count = self.skirmish_files.len() as i32;
        let mut top = (self.skirmish_file_top as i32 + delta).max(0);
        if count - 5 < top {
            top -= 1;
        }
        if count < 10 {
            top = 0;
        }
        self.skirmish_file_top = top.max(0) as usize;
    }

    /// ***Go*** — `FUN_0043D5B7` (`00430000.c:7781-7788`), whose body is
    /// `FUN_0043F304`, `Msg_Reset`, `FUN_0042BA40`, `FUN_0042C5AD`,
    /// `g_battleChoiceOwner = 1`, `DAT_0053F018 = 0`, `Battle_Start`
    /// (`0x004778A0`) and `g_appPhase = 3` — the phase word, not
    /// `g_battleState`. The choice owner is the `1` this function hands
    /// `LiveBattle::new`; `DAT_0053F018` is unnamed and nothing of ours reads
    /// it, so the zeroing has nowhere to land.
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
    // arm: 0x0043D5B7/skirmish-go left-press
    pub(crate) fn go_skirmish(&mut self, ctx: &mut Ctx) -> Transition {
        let (mine, theirs) = self.skirmish.fill_armies(&self.troops);
        if mine.men == 0 || theirs.men == 0 {
            // **[D]** `FUN_0043D5B7` tests only the multiplayer/master
            // condition (`00430000.c:7777-7788`) and raises the battlefield
            // with whatever the fill left — two empty musters on an install
            // with no `TROOPS2.ENG`, because `Troops_Load` returned 0.
            // `BattleRunner` refuses an empty side, so the button says
            // nothing instead of panicking.
            return Transition::Stay;
        }
        let (my_slot, their_slot) = self.skirmish.slots();
        let seed = self.skirmish.seed();
        let (mine_t, theirs_t) = (muster(&mine.counts), muster(&theirs.counts));
        let local = ctx.game.player;
        let foe = crate::screens::ratings::skirmish_opponent(local);
        let a = Muster { troops: &mine_t, owner: local, human: true };
        let b = Muster { troops: &theirs_t, owner: foe, human: false };
        // `FUN_0042BA40` (`00420000.c:4665-4673`) puts the local player in
        // `g_battleArmyB` when he attacks, so the attacking muster is his
        // whenever `DAT_0053EF5C == g_localPlayer`.
        let (att, def, att_slot, def_slot) = if self.skirmish.local_attacks {
            (a, b, my_slot, their_slot)
        } else {
            (b, a, their_slot, my_slot)
        };
        // **`FUN_0042B9C4` (`0x0042B9C4`, `00420000.c:4608-4619`)** — which
        // builder raises the field, by category: 0 and 1
        // `Battlefield_BuildRandom`, 2 `Battlefield_BuildCastle(DAT_0056D590)`
        // with `g_battleIsSiege = 1` (`00430000.c:8090`), 3
        // `Battlefield_BuildFromSkr`.
        // The list row **is** the castle level: `Battlefield_BuildCastle(DAT_0056D590)`
        // (`00420000.c:4615`) sets `g_castleLevel = DAT_0056D590` (`00470000.c:5060`);
        // `Siege_LowerDrawbridge` (`0x00496B9F`) reads `DAT_004D4B18[row]` under the
        // skirmish flag, and nothing in the siege reads `DAT_0057C910`.
        let level = self.skirmish.castle_level();
        let runner = match level {
            // `Battlefield_BuildCastle`, through the same two paths
            // `crate::engagement` takes: the install's layout, or the
            // stand-in ring when there is none.
            Some(level) => match crate::castle::sheet(level) {
                Some(sheet) => BattleRunner::deploy_siege_on_sheet(
                    l2_sim::castle::build(level, sheet),
                    &l2_sim::castle::tables(sheet),
                    seed,
                    att,
                    def,
                    level,
                ),
                None => BattleRunner::deploy_siege(
                    l2_sim::siege::our_castle(level),
                    seed,
                    att,
                    def,
                    level,
                ),
            },
            // Category 3's `Battlefield_BuildFromSkr` reads the chosen `.skr`,
            // and nothing parses that file yet — the open field stands in.
            _ => BattleRunner::deploy_muster(
                crate::batfield::field(seed),
                seed,
                att,
                def,
            ),
        };
        let mut battle = crate::battlefield::LiveBattle::new(
            runner,
            att_slot,
            def_slot,
            0,
            level,
            local,
            1,
        );
        battle.skirmish = true;
        ctx.game.battle = Some(Box::new(battle));
        Transition::Push(ScreenId::Battlefield)
    }
}
