#![allow(unused_imports)]
use super::*;
use crate::screens::setup::skirmish::muster;
use l2_sim::runner::{BattleRunner, Muster};

impl SetupScreen {
    pub fn skirmish(&self) -> &crate::screens::setup::skirmish::Skirmish {
        &self.skirmish
    }

    /// For a test that wants a page state without a click path to it —
    /// every arm on the page goes through [`SetupScreen::handle`].
    pub fn skirmish_mut(&mut self) -> &mut crate::screens::setup::skirmish::Skirmish {
        &mut self.skirmish
    }

    pub fn troops(&self) -> &crate::screens::setup::skirmish::TroopsTable {
        &self.troops
    }

    /// A table in place of the one `Troops_Load` (`0x0042AC0C`) reads, so a
    /// test can name the counts it asserts on. Pinning `troops_file` is what
    /// stops [`SetupScreen::update`]'s page-entry load replacing it.
    pub fn set_troops(&mut self, t: crate::screens::setup::skirmish::TroopsTable) {
        self.troops = t;
        self.troops_file = Some("(supplied)");
    }

    /// Which of `TROOPS.ENG` / `TROOPS2.ENG` / `TROOPS3.ENG` the table came
    /// from — `l2_formats::troops::file_for`'s answer at the last load.
    pub fn troops_file(&self) -> Option<&'static str> {
        self.troops_file
    }

    pub fn banner_frame(&self) -> usize {
        self.banner
    }

    /// The names page 13 lists — `DAT_004E8790`, which `Files_Enumerate`
    /// fills and [`crate::shell::ShellAssets::skirmish_files`] holds.
    pub fn set_skirmish_files(&mut self, files: Vec<String>) {
        self.skirmish_files = files;
    }

    /// **`SaveLoad_Scroll` (`0x00434346`) under list 2** — the skirmish box's
    /// arrows, hotspot id −1 and +1: `g_fileListTop += delta`, floored at 0,
    /// then walked back one row when it passes `DAT_004EB25C - 5` and put
    /// back to 0 outright when the list is shorter than ten names. The
    /// half-page clamp is the original's, `docs/screens-county.md`.
    ///
    /// The record is `g_skirmishFileWidgets` (`0x004DDDD8`), two kind-4
    /// widgets at (480, 256) and (480, 288) — see
    /// [`super::skirmish_file::FILE_SCROLL`], which reaches this.
    /// `DAT_004EA1A0` — the row `FUN_00414E06` and `FUN_00434174` both count
    /// from.
    pub fn skirmish_file_top(&self) -> usize {
        self.skirmish_file_top
    }

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
    /// `Skirmish_Setup` (`0x0042B7F7`) runs when page 12 opens, not when this
    /// button is pressed — it pairs `g_localPlayer` against `DAT_0056D5CC`,
    /// sets `g_battleArmyA = 1` and `g_battleArmyB = 2`, loads `BATTLES.ENG`
    /// and the troops table, raises `DAT_0057A0F0` and calls
    /// `Skirmish_FillArmies`. Every arm on the page then re-runs the fill, so
    /// the two armies are always the ones the page is showing. Ours is a pure
    /// function of the page's state, so it runs here, once.
    ///
    // arm: 0x0043D5B7/skirmish-go left-press
    pub(crate) fn go_skirmish(&mut self, ctx: &mut Ctx) -> Transition {
        let (mine, theirs) = self.skirmish.fill_armies(&self.troops);
        if mine.men == 0 || theirs.men == 0 {
            // **[D]** `FUN_0043D5B7` tests only the multiplayer/master
            // condition (`00430000.c:7777-7788`) and raises the battlefield
            // with whatever the fill left — two empty musters on an install
            // with no `TROOPS2.ENG`, because `Troops_Load` returned 0.
            return Transition::Stay;
        }
        let (my_slot, their_slot) = self.skirmish.slots();
        let seed = self.skirmish.seed();
        let (mine_t, theirs_t) = (muster(&mine.counts), muster(&theirs.counts));
        let local = ctx.game.player;
        let foe = crate::screens::ratings::skirmish_opponent(local);
        let a = Muster { troops: &mine_t, owner: local, human: true, mercenaries: None };
        let b = Muster { troops: &theirs_t, owner: foe, human: false, mercenaries: None };
        // `Skirmish_Setup` (`00420000.c:4564-4565`): record 1 is
        // `g_battleArmyA`, raised on side 4; record 2 `g_battleArmyB`, side 0
        // and in a siege the garrison. `FUN_0042BA40` (`:4673-4691`) puts the
        // local player in record 2 when `DAT_0053EF5C == g_localPlayer` — the
        // *defender*: `FUN_004209C1` `:245-247` captions realm 1 11/0x17
        // "Defender" then, and `FUN_0042C4E2` `:4826` lets him add only oil.
        let (att, def) = if my_slot == 1 { (a, b) } else { (b, a) };
        let (att_slot, def_slot) = (1, 2);
        debug_assert_eq!(their_slot, 3 - my_slot);
        // **`FUN_0042B9C4` (`0x0042B9C4`, `00420000.c:4608-4619`)** — which
        // builder raises the field, by category: 0 and 1
        // `Battlefield_BuildRandom`, 2 `Battlefield_BuildCastle(DAT_0056D590)`
        // with `g_battleIsSiege = 1` (`00430000.c:8090`), 3
        // `Battlefield_BuildFromSkr`.
        //
        // The list row **is** the castle level: `Battlefield_BuildCastle(DAT_0056D590)`
        // (`00420000.c:4615`) sets `g_castleLevel = DAT_0056D590` (`00470000.c:5060`);
        // `Siege_LowerDrawbridge` (`0x00496B9F`) reads `DAT_004D4B18[row]` under the
        // skirmish flag, and nothing in the siege reads `DAT_0057C910`.
        let level = self.skirmish.castle_level();
        let mut runner = match level {
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
            // Category 3 is `Battlefield_BuildFromSkr` (`0x0047B8B2`), which
            // `l2_sim::terrain::build` is; categories 0 and 1 are
            // `Battlefield_BuildRandom`'s skirmish branch, `batfiel2.pl8`
            // frame `DAT_0056D590` (+10 for category 1).
            _ => BattleRunner::deploy_muster(
                self.skirmish_field(ctx, seed),
                seed,
                att,
                def,
            ),
        };
        // `DAT_0057A0F0`: Battle_CheckOutcome's failed-assault arm reads `0x004D4A58`.
        runner.siege.skirmish = true;
        // `FUN_0042C5AD` (`0x0042C5AD`), the statement before `Battle_Start`
        // in `FUN_0043D5B7`: the two musters are copied into the records the
        // ratings sheet reads as *Before*.
        let local_side =
            if self.skirmish.local_defends { l2_sim::SIDE_A } else { l2_sim::SIDE_B };
        ctx.game.battle_master.seed_defaults(&ctx.assets.shell);
        ctx.game.battle_master.begin(
            (local, foe),
            local_side,
            mine.counts,
            theirs.counts,
        );
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
