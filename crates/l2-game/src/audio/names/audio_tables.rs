#![allow(unused_imports)]
use super::*;



/// Files the battlefield plays **by name**.
pub mod battle {
    /// `FUN_0049694F` — `Wall_Smash` — opens with
    /// `Sound_PlayFile("bathit2.wav", 0, 0)`: the effects flag, the one-shot
    /// buffer. `[V]`
    pub const WALL_SMASH: &str = "bathit2.wav";
    /// `FUN_0048551D` — a bridge catching fire — opens with
    /// `Sound_PlayFile("dest_ind.wav", 0, 0)`, the one-shot buffer: the same
    /// file the campaign's destroyed-industry sites ask for by name.
    pub const BRIDGE_FIRE: &str = "dest_ind.wav";
}

/// **`g_jobSound` (`0x004D2950`)** — the bank slot the village's job popup
/// plays as it opens, indexed by the original's **1-based** job number.
///
/// `Panel_JobDetail` (`0x00412B33`) is
/// `if (g_jobSound[job] != 0) Sound_RestartSlot(g_jobSound[job])`, so a zero is
/// a job with no sound — jobs **4** (building) and **9**
/// (idle) are silent, and job **8** never reaches the lookup at all because the
/// blacksmith takes a branch of its own above it.
///
/// `[V]`, the twelve `i32` at `0x004D2950` read out of the executable:
/// `0 7 4 6 0 10 8 9 0 0`. Those six live slots are the second, independent
/// confirmation that slots are 1-based — see the module note — because
/// 1-based they land on wheat, cattle, fallow, iron, stone and wood for the six
/// jobs that do exactly that work, and 0-based they land on nothing that fits.
pub const JOB_SOUND: [usize; 10] = [0, 7, 4, 6, 0, 10, 8, 9, 0, 0];

/// **`Panel_JobDetail`'s blacksmith branch**, `job == 8`, which is the one job
/// that does not go through [`JOB_SOUND`]:
///
/// ```c
/// Sound_PlayFile("fire.wav", 0, 0);
/// Sound_RestartSlot(8);
/// ```
///
/// Two sounds at once — the forge and the quarry's hammering — and it is the
/// only place in the game that plays a bank slot and a file together. `[V]`
pub mod blacksmith {
    pub const FIRE: &str = "fire.wav";
    /// Slot 8 is `stonecut.wav`. The bank has no smithy sound, so the original
    /// borrows the quarry's.
    pub const SLOT: usize = 8;
}

/// **The narrator's interface commentary** — `Sound_PlayFile("S0nn_mm.wav", 1,
/// 0)`, the *speech* flag, played by name from a screen's own handler.
///
/// A band of the voice class that `Msg_PlayVoice` never reaches: these are not
/// indexed by an `L2.eng` group, they are literals in the function that opens
/// the screen. That is why [`super::names::message_voice`] cannot produce one
/// and why they were missing while 543 files were reachable.
///
/// **`RATION_ON_DAIRY` is the line a player asked for.** He remembered *"All
/// your people are fed by dairy"* and `docs/decisions.md` C133 established that
/// **no such string exists** in any of `L2.eng`'s 317 groups — which was right,
/// and looked for it in the wrong medium. `Panel_OpenRation` (`0x0043A846`):
///
/// ```c
/// g_screenId = 0x19; Panel_Ration();
/// if (rationAchieved == 0)                              Sound_PlayFile("S021_02.wav", 1, 0);
/// else if (herd != 0 && herdEaten == 0 && grainEaten == 0) Sound_PlayFile("S021_01.wav", 1, 0);
/// ```
///
/// The second condition **is** *"all your people are fed by dairy"*: the county
/// has a standing herd, and opening the larder took neither a cow nor a sack.
/// The game says it by **speaking**, on the frame the panel opens, and not in
/// text at all. `[V]` on the condition and the file; `[I]` that the words are
/// the ones he remembered, which needs somebody to listen —
/// `docs/oracle-requests.md`.
pub mod speech {
    /// `Panel_OpenRation` (`0x0043A846`), `rationAchieved == 0` — the county is
    /// not fed at all.
    pub const RATION_NOT_MET: &str = "S021_02.wav";
    /// `Panel_OpenRation`, a standing herd and nothing eaten.
    pub const RATION_ON_DAIRY: &str = "S021_01.wav";
    /// `Sidebar_Button` (`0x0043AE30`) hotspot 3 — send supplies, `g_screenId`
    /// `0x18`. Played only when the county is the local player's; the other
    /// branch raises message `0x70` instead.
    pub const SUPPLIES: &str = "S033_01.wav";
    /// `Map_ZoomOut` (`0x00434FD5`) — the whole-kingdom zoom.
    pub const ZOOM_OUT: &str = "S033_02.wav";
    /// Setup page 4 — *"Choose your title and your shield."* Four handlers
    /// reach that page and all four play this: `FUN_00432CC8` twice,
    /// `Setup_ChooseCampaign` (`0x00433461`), and `FUN_00432B05` after a
    /// successful `Net_JoinGame`.
    pub const CHOOSE_YOUR_SHIELD: &str = "S011_02.wav";
    /// `Panel_SplitButton` (`0x004378B3`), on the branch that opens the
    /// division screen. The refusal branch — an army that has already moved —
    /// is silent and raises message `0x95` instead.
    pub const SPLIT_ARMY: &str = "S017_01.wav";

    /// **The battle prompt's spoken question**, indexed by
    /// `g_battleChoiceOwner` — *not* by the `L2.eng` group 80 index the same
    /// three cases pick, and the two orders differ.
    ///
    /// Three call sites carry the identical ladder and all three are the
    /// statement after `g_screenId = 0x12`: `Battle_BeginFromCampaign`
    /// (`0x004A7158`), `FUN_004A6C68` and `Siege_LaunchAssault`
    /// (`0x004A8AAB`). `[V]`, all three:
    ///
    /// ```c
    /// if (g_battleChoiceOwner == 1)      Sound_PlayFile("S080_03.wav", 1, 0);
    /// else if (g_battleChoiceOwner == 2) Sound_PlayFile("S080_01.wav", 1, 0);
    /// else                               Sound_PlayFile("S080_02.wav", 1, 0);
    /// ```
    ///
    /// **The take is decided and then usually dropped.** `ff_batl.wav` is
    /// played by `Battle_ChooseSettlement` (`0x004A6A30`) on the branch that
    /// returns 1, and all three of these sites run on that return — so the
    /// fanfare is in the one-shot buffer when the line is asked for and bare
    /// `Sound_PlayFile` drops it. Ours makes the same two calls in the same
    /// order through the same verb, so the drop is the buffer's here too
    /// `docs/audio.json` recorded these nine
    /// sites as *"which of the three a given call plays is unread"*; it is
    /// read, and it is this.
    pub const BATTLE_PROMPT: [&str; 3] = ["S080_02.wav", "S080_03.wav", "S080_01.wav"];

    /// **`SaveLoad_Tick` (`0x004AD9F0`)** — the line the box speaks when the
    /// thumb up's latch is taken up, on the frame the 0x96-frame wait starts.
    /// `[V]`:
    ///
    /// ```c
    /// if (g_screenId == '6') { … Sound_PlayFile("S040_02.wav", 1, 0); }
    /// if (g_screenId != '6') { … Sound_PlayFile("S040_01.wav", 1, 0); }
    /// ```
    ///
    /// `0x36` is the save box and `0x35` the load box, so the save speaks
/// `_02` and the load `_01`. Two `if`s, and
    /// the second's body also arms the failure latch when the file cannot be
/// opened — so it is written that way.
    pub const SAVE_GAME: &str = "S040_02.wav";
    /// `SaveLoad_Tick`'s other arm — every screen that is not `0x36`.
    pub const LOAD_GAME: &str = "S040_01.wav";

    /// **The trade spinner crossing into buying.** The merchant's four
    /// quantity handlers each end in the same guard on the quantity before and
    /// after the step — `if (0 < qty && oldQty < 1)` — so the line is said
    /// once, on the move that turns a sale or a standstill into a purchase,
    /// and never again while the player ramps the number up.
    ///
    /// `FUN_00435339` (the up arrow) and `FUN_004355DB` (the ceiling button)
    /// say `S068_01.wav`; `FUN_0043543D` (down) and `FUN_00435541` (the floor
    /// button) say `S068_02.wav`. `[V]` at each of the four.
    pub const TRADE_BUYING_UP: &str = "S068_01.wav";
    /// The down arrow's and the floor button's take of the same moment.
    pub const TRADE_BUYING_DOWN: &str = "S068_02.wav";

    /// **The mercenary offer, read aloud** — `0x004DF8B8`, `char[16][16]`,
    /// indexed by `mercenaryOffer − 1`.
    ///
    /// `Sidebar_Button` (`0x0043AE30`) hotspot 1's tail, the statement after
    /// the one that opens the raise-army screen:
    ///
    /// ```c
    /// g_screenId = 0x17; … ;
    /// if (g_counties[g_selectedCounty].mercenaryOffer != 0)
    ///     FUN_004B3714(g_counties[g_selectedCounty].mercenaryOffer - 1);
    /// ```
    ///
    /// and `FUN_004B3714` is `if (-1 < n && n < 0x10) Sound_PlayFile(table + n
    /// * 0x10, 1, 0)`. `[V]` at both. **This is the line a player reported
    /// missing** — *"A band of Scottish pikemen are available for hire, my
    /// lord"* — and it is not text: the raise-army screen draws the band's
    /// nationality out of `L2.eng` group 16 and says nothing about hiring, so
    /// the offer is *announced* only here.
    ///
    /// Sixteen names for **twelve** nationalities: `l2_kingdom::mercenary::
    /// ROSTER` is twelve long and only `S016_01` … `S016_12` ship, so the last
    /// four entries of the table name files that do not exist. That is the
    /// original's own over-allocation and not a gap of ours; a band outside
    /// 1…12 is silent because [`super::super::Audio::load`] cannot find the
    /// file, which is the same answer `mmioOpenA` gives.
    pub const MERCENARY_OFFER: [&str; 16] = [
        "S016_01.wav",
        "S016_02.wav",
        "S016_03.wav",
        "S016_04.wav",
        "S016_05.wav",
        "S016_06.wav",
        "S016_07.wav",
        "S016_08.wav",
        "S016_09.wav",
        "S016_10.wav",
        "S016_11.wav",
        "S016_12.wav",
        "S016_13.wav",
        "S016_14.wav",
        "S016_15.wav",
        "S016_16.wav",
    ];

    /// **The population panel's health line** — `0x004E2058`, `char[8][16]`,
    /// indexed by `county.healthBand` straight.
    ///
    /// `Panel_OpenPopulation` (`0x0043A8F2`) is three statements and the middle
    /// one is this, which makes it the exact twin of [`RATION_NOT_MET`]'s site:
    ///
    /// ```c
    /// g_screenId = 0x14;
    /// FUN_004B3768((int)(char)g_counties[g_selectedCounty].healthBand);
    /// Panel_Population();
    /// ```
    ///
    /// **Entries 3 and 4 are the same file, and that is why this is a table
    /// The bytes at `0x004E2058` read `S020_01`
    /// `_02`, `_03`, `_04`, `_04`, `_05`, `_06`, `_07` — so the two healthiest
    /// of the five bands `l2_kingdom::tables::health_band` produces share one
    /// clip, and a generated name would have spoken `S020_05.wav` (a real file,
    /// and the wrong line) for band 4. `[V]` from the executable.
    ///
    /// `healthBand` is `0 ..= 4`, so the last three entries are unreachable in
    /// a running game and `S020_06` / `S020_07` do not ship.
    pub const POPULATION_HEALTH: [&str; 8] = [
        "S020_01.wav",
        "S020_02.wav",
        "S020_03.wav",
        "S020_04.wav",
        "S020_04.wav",
        "S020_05.wav",
        "S020_06.wav",
        "S020_07.wav",
    ];

    /// **The standings page saying which category you are looking at** —
    /// `0x004E2168`, `char[][16]`, indexed by `DAT_0055CE7C` straight.
    ///
    /// `FUN_004B3994` is `if (-1 < n && n < 9) Sound_PlayFile(table + n * 0x10,
    /// 1, 0)`, and it has exactly **two callers**, both passing the category:
    /// `FUN_004351C4`, the court's *Greatest nobles* button, and
    /// `FUN_0043524E`, one of the page's seven tabs. So this is the spoken
    /// half of `L2.eng` group 35, file named after group and index, and
    /// `crates/l2-game/src/screens/nobles/mod.rs` is the screen.
    ///
    /// **Two entries past the end of what can be reached, and they are not the
    /// same mistake.** There are seven categories, so `S035_08.wav` — the
    /// voice line for index 7, *"undecided."* — **ships and is played by
    /// nothing**. And the guard admits **nine** where the table holds eight:
    /// index 8 reads the next table along, whose first entry is
    /// `S075_01.wav`. Both are read out of the executable; neither is
    /// reachable from either caller, so this array stops at the seven that
    /// are, and the eighth is carried only so that the shape of the original's
    /// table is visible beside it.
    pub const STANDINGS_CATEGORY: [&str; 8] = [
        "S035_01.wav",
        "S035_02.wav",
        "S035_03.wav",
        "S035_04.wav",
        "S035_05.wav",
        "S035_06.wav",
        "S035_07.wav",
        "S035_08.wav",
    ];

    /// **What the map information panel says about a *unit*** — `0x004E20D8`,
    /// `char[4][16]`. `FUN_004B37BC` picks by `g_units[picked].kind` and, for
    /// an army, by its owner:
    ///
    /// ```c
    /// kind 1 && owner == g_localPlayer  ->  S031_04.wav
    /// kind 1                            ->  S031_03.wav
    /// kind 4  (a transport)             ->  S031_02.wav
    /// kind 2  (a peasant mob)           ->  S031_01.wav
    /// ```
    ///
    /// **Kind 3, the merchant, is not in the ladder and is silent** — and it is
/// silent for a reason: `Map_Click` sends a click on
    /// a merchant to screen `0x08`, the stall, so the information panel never
    /// opens on one from the left button. `[V]`
    ///
    /// # Three independent readings, because "we found nothing" is a claim
    ///
    /// A report — *"picking a merchant says nothing, where a unit or a castle
    /// speaks"* — sent somebody looking for the missing arm. **There is none,
    /// `CLAUDE.md` rule 5: the finding is that the
    /// original does not do it.
    ///
    /// 1. **The ladder.** `FUN_004B37BC` is the last statement of *both*
    ///    functions that open screen `0x04` (`FUN_0043893C` and `Map_Click`'s
    /// `flags & 0x20` arm). It tests kinds 1, 4 and 2 and the castle branch
    ///    and `return`s for everything else; kind 3 falls out of the bottom.
/// 2. **The files.** Four `S031_*.wav` ship.
    /// 3. **The group, which is the reading that settles it.** `L2.eng` group
    ///    31 holds five unit descriptions and four of them are these four, in
    ///    this order: 13 *"These starving revolutionaries…"*, 14 *"This
    ///    transport is moving goods…"*, 15 *"This is an enemy army."*, 16
    ///    *"This is one of your armies."* The fifth is index **12**,
    ///    *"Merchants allow a county to buy needed supplies and raise revenue
    ///    by selling goods."* — the one member of the run with prose and no
    /// voice file. The words exist. `[V]`
    pub const PICKED_UNIT: [&str; 4] = [
        "S031_01.wav",
        "S031_02.wav",
        "S031_03.wav",
        "S031_04.wav",
    ];

    /// **What it says about a *castle*** — `0x004E2118`, `char[5][16]`, indexed
    /// by `county.castleType − 1`, so a wooden palisade and a fortress get
    /// different sentences.
    ///
    /// `FUN_004B37BC`'s tile branch, with all three of its guards: the tile
    /// carries plane-0 bit `0x80` (a settlement), its graphic is `0x15` or
    /// above (the castle end of `Industry_ToggleFromMap`'s ladder, which is how
    /// the original tells a keep from a mine without a second plane)
    /// county's `castleType` is 1…5. `[V]`
    ///
    /// The table starts at `S071_02` — `S071_01.wav` does not ship and is named
    /// nowhere.
    pub const PICKED_CASTLE: [&str; 5] = [
        "S071_02.wav",
        "S071_03.wav",
        "S071_04.wav",
        "S071_05.wav",
        "S071_06.wav",
    ];
}

/// The fanfares, which are played by name —
/// `FUN_00427990(name, 0 or 1, 0)` at a handful of sites.
pub mod fanfare {
    /// A message window opening, category 1 — a letter from another lord.
    /// `Msg_DrawWindow` (`0x0047309E`), on the frame `g_messageTimer` is 2000.
    /// `[V]`
    pub const MESSAGE: &str = "ff_msg.wav";
    /// Category 0 with the message group in `0x72 ..= 0x7E` — the conquest
    /// band. Same function, same frame. `[V]`
    pub const CAPTURED: &str = "ff_capt.wav";
    /// `Battle_ChooseSettlement` (`0x004A6A30`) — a battle is about to be
    /// fought. `[V]`
    pub const BATTLE: &str = "ff_batl.wav";
    /// `Battle_ReturnToCampaign` (`0x004AB383`), **both** of its two sites.
    /// See the module note on `Ff_win.wav`. `[V]`
    ///
    /// **And both sites play it to the *winner*.** `[V]`, read from the
    /// decompilation while wiring this up, because it decides the gate a call
    /// site here would need:
    ///
    /// ```c
    /// if (g_battleLoser == g_battleArmyA) {
    ///   if (g_units[g_battleArmyB].owner == g_localPlayer) Sound_PlayFile("ff_lose.wav", 0, 0);
    /// } else if (g_battleLoser == g_battleArmyB) {
    ///   if (g_units[g_battleArmyA].owner == g_localPlayer) Sound_PlayFile("ff_lose.wav", 0, 0);
    /// }
    /// ```
    ///
    /// Each arm names the army that did **not** lose, so the local player hears
    /// a fanfare exactly when he wins and hears nothing at all when he loses —
    /// and the fanfare he hears is the one called *lose*, while `Ff_win.wav`
    /// ships unreferenced. That makes the defect sharper than "the wrong file
    /// is played at both sites"
    /// one is misnamed.
    ///
    /// **Not wired**, and deliberately: `main.rs`'s `listen` can see the
    /// [`crate::screen::ScreenId::BattleResult`] screen arrive but not who won,
    /// because the [`crate::engagement::BattleReport`] travels inside
    /// `turn::TurnStep::Report` and is never parked on the [`crate::Game`].
    /// Playing it on every battle would add a sound the original never makes,
    /// on the one outcome it is silent for. It needs the verdict, not a call.
    pub const AFTER_BATTLE: &str = "ff_lose.wav";
}

/// The lord voices: `<kt|bn|ct|bp><group>_<1..=4>.wav`, `0x004E0258`.
///
/// A table in the binary — 28 groups × 16 × 16 bytes — but a pure naming
/// convention on disk, so it is generated.
///
/// **`variant` runs 0…15, not 0…3.** `Msg_PlayVoice(group, variant)` indexes
/// the table as `(group - 170) * 0x100 + variant * 0x10`, and `0x100` is
/// sixteen entries, laid out `kt_1 kt_2 kt_3 kt_4 bn_1 … bp_4`. So the high
/// two bits of the variant pick the speaker — Knight, Baron, Countess, Bishop
/// — and the low two pick the take. `[V]` from the table bytes and the index
/// arithmetic.
///
/// That the *same* `variant` also selects the message text
/// (`Eng_DrawString(group, variant + 1)`) means the four personas have four
/// wordings each of every diplomatic letter. `[I]` — the arithmetic says the
/// number is shared, not what the sixteen strings say.
///
/// Groups outside `170 ..= 197` have no lord voice. All 448 files exist in the
/// install; `Bp100_3.wav` is a stray outside the range and is never named.
pub fn lord_voice(group: u16, variant: u8) -> Option<String> {
    if !(170..=197).contains(&group) || variant > 15 {
        return None;
    }
    let who = ["kt", "bn", "ct", "bp"][(variant / 4) as usize];
    Some(format!("{who}{group}_{}.wav", variant % 4 + 1))
}

/// The system voice for an `L2.eng` group that has no lord — `S###_01.wav`.
///
/// `Msg_PlayVoice` has two more tables for these: groups `100 ..= 169` at
/// `g_msgVoice100` and `200 ..= 284` at `g_msgVoice200`, one file each. Both
/// are `S<group padded to 3>_01.wav`, so again a convention
/// transcription.
///
/// **The upper bound is 284 and it used to be 299 here.** `g_msgVoice200` is
/// 85 entries and `docs/symbols.md` says so; this file said `200 ..= 299`,
/// which invents fifteen groups. The install settles it independently: the
/// highest `S2xx` file that ships is `S284_02.wav`, and nothing above it
/// exists. Asserted in `tests/audio_install.rs`.
pub fn system_voice(group: u16) -> Option<String> {
    if (100..=169).contains(&group) || (200..=284).contains(&group) {
        Some(format!("S{group:03}_01.wav"))
    } else {
        None
    }
}

/// **`Msg_PlayVoice` (`0x004B35C1`) itself** — the group and variant of the
/// message on screen to the file that speaks it.
///
/// Three of its four tables, in the order the function tests them: the
/// diplomatic band has a lord and a take, everything in the two system bands
/// has one clip, and everything else is silent. (The fourth, `g_msgVoiceS010`,
/// is reached by `FUN_004B36C0` — see the module
/// note.)
///
/// **Silence is the common case.** 109 groups of the
/// hundreds `L2.eng` holds have a system clip; a group outside all three bands
/// is a message the narrator does not read, and `None` is that.
pub fn message_voice(group: u16, variant: u8) -> Option<String> {
    lord_voice(group, variant).or_else(|| system_voice(group))
}

