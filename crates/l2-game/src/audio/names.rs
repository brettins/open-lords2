//! **Which file is which sound**, read out of `Lords2.exe`'s data section.
//!
//! Nothing here is a guess. The original addresses a sound by *index into a
//! table of fixed-width filenames*, and the tables are laid out in `.data` as
//! `char[N][16]` — so recovering them is reading bytes, not inferring
//! intent. Every address below is an image address with the standard
//! `0x400000` base, and every list is in table order.
//!
//! ```text
//! 0x004D9228  char[5][16]    battle1.wav … battle5.wav        MUSIC_BATTLE
//! 0x004DC034  char[5][12]    scroll1.wav … scroll5.wav        MUSIC_SCROLL
//! 0x004DAF00  char[12][16]   click3 … army                    KINGDOM_BANK
//! 0x004DAFC0  char[17][16]   click3 … siegedoc                BATTLE_BANK
//! 0x004DB0D0  char[11][4][4][16]                              TROOP_VOICES
//! 0x004E0258  char[28][4][4][16]  kt/bn/ct/bp 170…197         (a convention, not a table)
//! ```
//!
//! # The two banks are *preloaded*, and that is why they are two
//!
//! `Sound_LoadBank(names, count)` (`0x00425E4B`) opens `count` files through
//! `mmioOpenA` and keeps each in a DirectSound buffer, so a click costs no
//! I/O. The game fills the cache twice with two different banks —
//! `FUN_00499D7D` loads [`KINGDOM_BANK`]'s twelve when the campaign starts,
//! `FUN_00499D97` loads [`BATTLE_BANK`]'s seventeen when a battle does — and
//! the *same slot number* therefore means a different sound depending on which
//! screen you are on. Both banks begin with `click3.wav` and `null.wav`, which
//! is what made the two-bank arrangement visible: it is not one table with a
//! gap.
//!
//! `null.wav` is not in either install. Entry 1 of both banks is
//! a deliberate hole.
//!
//! # **Slot numbers are 1-based, and the arrays here are not**
//!
//! This is the trap, and it is invisible in the decompiler unless you compare
//! two base addresses. `Sound_LoadBank` **stores** at
//! `&DAT_00522B00 + i * 4` counting `i` from 0. `Sound_PlaySlot` and
//! `Sound_RestartSlot` **read** from `&DAT_00522AFC + slot * 4` — and
//! `0x00522AFC` is four bytes *below* `0x00522B00`. So
//!
//! ```text
//! slot n  ==  BANK[n - 1]
//! ```
//!
//! `[V]`, and confirmed twice over by what the numbers then mean.
//! `Unit_MoveInFacing` plays slot 12 for an army, 11 for a merchant or
//! transport, 5 for a peasant mob: 1-based those are `army.wav`,
//! `merchant.wav` and `rioters.wav`, and 0-based they are nothing, `army.wav`
//! and `fallow.wav`. `g_jobSound` maps the village's jobs to slots 7, 4, 6,
//! 10, 8, 9: 1-based that is wheat, cattle, fallow, iron, stone, wood — six
//! for six — and 0-based it is stonecut for grain and rioters for cattle.
//!
//! Reading the slots as 0-based makes `click3.wav` look unplayable, because
//! nothing passes 0. It is slot **1**, and `Widget_Test` (`0x0040DA1E`) plays
//! it every time a widget is pressed. [`slot`] is the one place that
//! conversion happens.
//!
//! # What ships and is never asked for
//!
//! **753 of the install's 771 `.wav` files are named somewhere in
//! `Lords2.exe`; 18 are not.** `[V]`, by scanning the executable for each
//! shipped filename. The unreferenced ones are `Ff_win.wav`, `Ff_capt2.wav`,
//! `Ff_msg1.wav`, `Supply.wav`, `Dest_fld.wav`, `Boilguy1/2.wav`,
//! `Boilwood.wav`, `Bathit1.wav`, `Arch_e5.wav`, `Knig_e3.wav`,
//! `Bp100_3.wav`, and six `S###_##` clips.
//!
//! **`Ff_win.wav` is the interesting one.** `Battle_ReturnToCampaign`
//! (`0x004AB383`) plays a fanfare at two sites and *both* of them name
//! `ff_lose.wav` — two separate string literals, at `0x004DE8F8` and
//! `0x004DE904`, holding the same text. A victory fanfare shipped and the
//! function that would play it plays the defeat one twice. `[V]` that the
//! binary contains no reference to `ff_win.wav`; `[I]` that this is a bug

/// The five battle tracks, `0x004D9228`, in table order. Index 0 is
/// `battle1.wav`. `[V]`
pub const MUSIC_BATTLE: [&str; 5] = [
    "battle1.wav",
    "battle2.wav",
    "battle3.wav",
    "battle4.wav",
    "battle5.wav",
];

/// **The front end's bed** — `Music_Play("setup.wav", 0, 1)`, a literal rather
/// than a table entry.
///
/// Two siblings ship and are not reached from our engine: `setup2.wav`, which
/// `Screen_DrawConquest` (`0x0041E1DD`) plays **unlooped** once the campaign is
/// past its eighth map, and `SETUP3.WAV`, which four sites play over the
/// credits and the ending. See [`super::track::Music::Setup`].
pub const MUSIC_SETUP: &str = "setup.wav";
pub const MUSIC_SETUP2: &str = "setup2.wav";
pub const MUSIC_SETUP3: &str = "setup3.wav";

/// The five campaign tracks, `0x004DC034`, in table order. `[V]`
pub const MUSIC_SCROLL: [&str; 5] = [
    "scroll1.wav",
    "scroll2.wav",
    "scroll3.wav",
    "scroll4.wav",
    "scroll5.wav",
];

/// **`FUN_00499D7D`'s twelve**, `FUN_00425E4B(0x004DAF00, 12)` — the bank the
/// campaign and county screens play from. `[V]`
///
/// Slots 3…11 are the village's work: cattle, unrest, fallow land, wheat, the
/// quarry, the wood, the iron, the merchant, the army. That is the shape of
/// the village's nine job slots, so this table is worth writing down
/// even before anything plays it.
pub const KINGDOM_BANK: [&str; 12] = [
    "click3.wav",   // 0
    "null.wav",     // 1 — not in the install; a hole in both banks
    "dest_ind.wav", // 2
    "moo_2.wav",    // 3
    "rioters.wav",  // 4
    "fallow.wav",   // 5
    "wheat.wav",    // 6
    "stonecut.wav", // 7
    "woodcut.wav",  // 8
    "iron.wav",     // 9
    "merchant.wav", // 10
    "army.wav",     // 11
];

/// **`FUN_00499D97`'s seventeen**, `FUN_00425E4B(0x004DAFC0, 17)` — the bank a
/// battle plays from. Slot 0 is `click3.wav` again: the two banks overlap by
/// two entries and are otherwise disjoint. `[V]`
pub const BATTLE_BANK: [&str; 17] = [
    "click3.wav",   // 0
    "null.wav",     // 1
    "pouroil.wav",  // 2
    "sword5.wav",   // 3
    "sword2.wav",   // 4
    "sword3.wav",   // 5
    "bowmen1.wav",  // 6
    "bow_hit.wav",  // 7
    "crossbow.wav", // 8
    "cros_hit.wav", // 9
    "deadguy2.wav", // 10
    "deadguy3.wav", // 11
    "deadguy4.wav", // 12
    "catfire.wav",  // 13
    "cathit.wav",   // 14
    "catmiss.wav",  // 15
    "siegedoc.wav", // 16
];

/// **`g_troopSounds` (`0x004DB0D0`)** — `char[11][4][4][16]`, the troop cries,
/// indexed `[troop][class][take]`. `[V]`: transcribed from the executable's
/// bytes, with the binary's own casing (`knig_M1.wav`), and asserted against
/// them by `tests/audio_battle.rs`.
///
/// `Sound_PlayTroopCry(class)` (`0x00499CB1`) indexes it as
/// `unit * 0x100 + class * 0x40 + take * 0x10`, so the stride is troop, then
/// event class, then take. The four classes are the four things a player tells
/// his men, and each is named by the letter its files carry:
///
/// | class | files | asked for by |
/// |---:|---|---|
/// | 0 | `_U` | a selection committed — `Battle_DragSelect`, both arms |
/// | 1 | `_P` | an order to go somewhere, and `H`/`V` |
/// | 2 | `_E` | an order onto an enemy |
/// | 3 | `_M` | an order onto surface 2, the moat |
///
/// **Class 3 is always take 0**, which is `docs/bugs.md` D34: the other three
/// cells of every `_M` row can never be chosen, and that is where the seven
/// `_F1` names sit — none of which ships — and `Swor_U3.wav` and `Arch_U3.wav`,
/// which do not ship either. Two `_M` rows borrow another troop's voice:
/// crossbowmen and swordsmen say `Pike_M1`, macemen and archers `Peas_M1`.
///
/// The four siege engines have one row each, class 1 — the engine being told
/// to move — and `null.wav` everywhere else.
pub const TROOP_CRIES: [[[&str; 4]; 4]; 11] = [
    [
        ["Peas_U1.wav", "Peas_U2.wav", "Peas_U3.wav", "Peas_U4.wav"],
        ["Peas_P1.wav", "Peas_P2.wav", "Peas_P3.wav", "Peas_P4.wav"],
        ["Peas_E2.wav", "Peas_E1.wav", "Peas_E2.wav", "Peas_E3.wav"],
        ["Peas_M1.wav", "Peas_U5.wav", "Peas_E1.wav", "Peas_F1.wav"],
    ],
    [
        ["Cros_U1.wav", "Cros_U2.wav", "Cros_U1.wav", "Cros_U2.wav"],
        ["Cros_P1.wav", "Cros_P2.wav", "Cros_P3.wav", "Cros_P2.wav"],
        ["Cros_E1.wav", "Cros_E3.wav", "Cros_E2.wav", "Cros_E3.wav"],
        ["Pike_M1.wav", "Cros_E3.wav", "Cros_U2.wav", "Cros_F1.wav"],
    ],
    [
        ["Mace_U1.wav", "Mace_U2.wav", "Mace_U1.wav", "Mace_U2.wav"],
        ["Mace_P1.wav", "Mace_P2.wav", "Mace_P1.wav", "Mace_P3.wav"],
        ["Mace_E1.wav", "Mace_E2.wav", "Mace_E1.wav", "Mace_E2.wav"],
        ["Peas_M1.wav", "Mace_E2.wav", "Mace_U1.wav", "Mace_F1.wav"],
    ],
    [
        ["Swor_U1.wav", "Swor_U2.wav", "Swor_U1.wav", "Swor_U2.wav"],
        ["Swor_P1.wav", "Swor_P2.wav", "Swor_P1.wav", "Swor_P3.wav"],
        ["Swor_E1.wav", "Swor_E2.wav", "Swor_E3.wav", "Swor_E4.wav"],
        ["Pike_M1.wav", "Swor_E2.wav", "Swor_U3.wav", "Swor_F1.wav"],
    ],
    [
        ["Pike_U1.wav", "Pike_U2.wav", "Pike_U3.wav", "Pike_U4.wav"],
        ["Pike_P1.wav", "Pike_P3.wav", "Pike_P2.wav", "Pike_P3.wav"],
        ["Pike_E1.wav", "Pike_E2.wav", "Pike_E1.wav", "Pike_E2.wav"],
        ["Pike_M2.wav", "Pike_E2.wav", "Pike_U1.wav", "Pike_F1.wav"],
    ],
    [
        ["Arch_U1.wav", "Arch_U2.wav", "Arch_U1.wav", "Arch_U2.wav"],
        ["Arch_P1.wav", "Arch_P2.wav", "Arch_P1.wav", "Arch_P2.wav"],
        ["Arch_E1.wav", "Arch_E2.wav", "Arch_E3.wav", "Arch_E4.wav"],
        ["Peas_M1.wav", "Arch_E2.wav", "Arch_U3.wav", "Arch_F1.wav"],
    ],
    [
        ["Knig_U1.wav", "Knig_U2.wav", "Knig_U1.wav", "Knig_U2.wav"],
        ["Knig_P1.wav", "Knig_P2.wav", "Knig_P1.wav", "Knig_P2.wav"],
        ["Knig_E1.wav", "Knig_E2.wav", "Knig_E1.wav", "Knig_E2.wav"],
        ["knig_M1.wav", "Knig_E2.wav", "Knig_U2.wav", "Knig_F1.wav"],
    ],
    [["null.wav"; 4], ["movcat.wav"; 4], ["null.wav"; 4], ["null.wav"; 4]],
    [["null.wav"; 4], ["movsiege.wav"; 4], ["null.wav"; 4], ["null.wav"; 4]],
    [["null.wav"; 4], ["movbat.wav"; 4], ["null.wav"; 4], ["null.wav"; 4]],
    [["null.wav"; 4], ["movoil.wav"; 4], ["null.wav"; 4], ["null.wav"; 4]],
];

/// The cry a troop, class and take name, or `None` for a cell holding
/// `null.wav` and for anything out of the table's range. Which take is asked
/// for is `super::TroopCries`'s business, not this table's.
pub fn troop_cry(troop: usize, class: usize, take: usize) -> Option<&'static str> {
    match *TROOP_CRIES.get(troop)?.get(class)?.get(take)? {
        "null.wav" => None,
        name => Some(name),
    }
}

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

/// Which bank a slot number is being read against — the two are not
/// interchangeable and a bare number does not say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bank {
    /// `FUN_00499D7D`'s twelve, loaded when the campaign comes up.
    Kingdom,
    /// `FUN_00499D97`'s seventeen, loaded when a battle does.
    Battle,
}

/// **The file a 1-based slot number names**, which is the form every call site
/// in the original uses. See the module note: `slot n` is `BANK[n - 1]`.
///
/// `None` for slot 0, for a slot past the bank, and for
/// the `null.wav` hole — all three of which mean "no sound" and none of which
/// is an error.
pub fn slot(bank: Bank, slot: usize) -> Option<&'static str> {
    let table: &[&'static str] = match bank {
        Bank::Kingdom => &KINGDOM_BANK,
        Bank::Battle => &BATTLE_BANK,
    };
    match table.get(slot.checked_sub(1)?) {
        Some(&"null.wav") | None => None,
        Some(name) => Some(name),
    }
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

/// **`TileInfo_Draw` (`0x0041C208`) — the bank slot a resource site plays when
/// the information panel opens on it.**
///
/// The ladder is on `g_pickedTileGraphic` behind `flags & 0x80`, and it is the
/// *same* four ranges [`l2_kingdom::industry::map_toggle_for_graphic`] uses to
/// decide which industry a click on that tile switches — so the two agree by
/// construction. `[V]`
///
/// | graphic | site | slot | file |
/// |---|---|---:|---|
/// | 0…3 | mine | 10 | `iron.wav` |
/// | 4…6 | quarry | 8 | `stonecut.wav` |
/// | 7…9 | **blacksmith** | 8 | `stonecut.wav` |
/// | 10…12 | lumber mill | 9 | `woodcut.wav` |
///
/// **The blacksmith plays the quarry's sound**
/// job 8 does. `[V]` at both sites; `[I]` that it is because the kingdom bank
/// has no forge in it.
///
/// 13 and above is the castle, which is silent.
pub fn resource_site_slot(graphic: u8) -> Option<usize> {
    Some(match graphic {
        0..=3 => 10,
        4..=6 => 8,
        7..=9 => 8,
        10..=12 => 9,
        _ => return None,
    })
}

/// **`FUN_00438B02` (`0x00438B02`) — the bank slot a field brush button
/// plays.**
///
/// The original's ladder is on `g_uiHotspotId`, the raw terrain the button
/// paints, and it is five arms over three slots. `[V]`:
///
/// ```c
/// if      (id == 0x13) Sound_RestartSlot(4);   /* moo_2.wav  — pasture */
/// else if (id == 2)    Sound_RestartSlot(7);   /* wheat.wav  — grain   */
/// else if (id == 1)    Sound_RestartSlot(6);   /* fallow.wav — fallow  */
/// else if (id == 0)    Sound_RestartSlot(6);   /* …abandon             */
/// else if (id == 0x19) Sound_RestartSlot(6);   /* …start reclaiming    */
/// ```
///
/// Taken here over the *ranges* `County_RecountFields` counts.
/// five brush values, because the brush value is not what is on the ground a
/// statement later: `Field_SetType` runs `Herd_UpdateCrowding`, which repaints
/// a fresh pasture `0x13` to its grazing grade `0x14 … 0x16`
/// (`l2_kingdom::field::herd_update_crowding`). The five brush values are each
/// inside the range that answers with their slot, so on the brush's own arms
/// the two readings agree. `[D]` on the widening.
pub fn field_brush_slot(terrain: u8) -> Option<usize> {
    use l2_kingdom::field::terrain as t;
    Some(match terrain {
        t::PASTURE_FIRST..=t::PASTURE_LAST => 4,
        t::GRAIN..=t::GRAIN_LAST => 7,
        t::WASTE | t::FALLOW => 6,
        t::RECLAIM_FIRST..=t::RECLAIM_LAST => 6,
        _ => return None,
    })
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
    /// `crates/l2-game/src/screens/nobles.rs` is the screen.
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
    ///    `flags & 0x20` arm). It tests kinds 1, 4 and 2 and the castle branch
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

/// **The tip screens' chained takes** — `FUN_004B3ACD`'s five-byte rows at
/// `0x004E1E40 + group * 5`, for the ten groups whose row is not all zero.
/// `[V]` read out of the executable; every other row in 200…219 is zero.
///
/// A byte is a 1-based index into [`TAKE_POOL`], and the first zero ends the
/// group. Rows below 200 are the ASCII of the name pool and are never reached,
/// because only a tip window calls the function.
pub const TIP_TAKES: [(u16, [u8; 5]); 10] = [
    (200, [26, 27, 0, 0, 0]),
    (201, [1, 2, 3, 0, 0]),
    (202, [4, 5, 6, 0, 0]),
    (207, [7, 8, 9, 0, 0]),
    (209, [10, 11, 12, 13, 0]),
    (210, [14, 15, 16, 0, 0]),
    (212, [17, 0, 0, 0, 0]),
    (214, [18, 19, 0, 0, 0]),
    (217, [20, 21, 22, 23, 0]),
    (218, [24, 25, 0, 0, 0]),
];

/// `s_S201_02_wav_004E2290`, sixteen bytes a name: the pool the takes index.
/// `[V]` from the executable; entries 28…30 are the sentinel `S000_00.wav`
/// and the `n < 0x1F` guard lets no row reach them.
pub const TAKE_POOL: [&str; 27] = [
    "S201_02.wav",
    "S201_03.wav",
    "S201_04.wav",
    "S202_02.wav",
    "S202_03.wav",
    "S202_04.wav",
    "S207_02.wav",
    "S207_03.wav",
    "S207_04.wav",
    "S209_02.wav",
    "S209_03.wav",
    "S209_04.wav",
    "S209_05.wav",
    "S210_02.wav",
    "S210_03.wav",
    "S210_04.wav",
    "S212_02.wav",
    "S214_02.wav",
    "S214_03.wav",
    "S217_02.wav",
    "S217_03.wav",
    "S217_04.wav",
    "S217_05.wav",
    "S218_02.wav",
    "S218_03.wav",
    "S200_02.wav",
    "S200_03.wav",
];

/// `table[group * 5 + cursor]`, or 0 — the end of the group.
pub fn tip_take(group: u16, cursor: usize) -> u8 {
    TIP_TAKES
        .iter()
        .find(|(g, _)| *g == group)
        .and_then(|(_, row)| row.get(cursor))
        .copied()
        .unwrap_or(0)
}

/// `"S201_02.wav" + (n - 1) * 0x10`, behind the `n < 0x1F` guard.
pub fn take_name(n: u8) -> Option<&'static str> {
    if !(1..0x1F).contains(&n) {
        return None;
    }
    TAKE_POOL.get(n as usize - 1).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lord_voice_convention_reproduces_the_table() {
        // The sixteen cells of group 170's block at 0x004E0258, as the bytes
        // have them: four takes of the Knight, then the Baron, the Countess
        // and the Bishop.
        let block: Vec<String> = (0..16).map(|v| lord_voice(170, v).unwrap()).collect();
        assert_eq!(block[0], "kt170_1.wav");
        assert_eq!(block[3], "kt170_4.wav");
        assert_eq!(block[4], "bn170_1.wav");
        assert_eq!(block[8], "ct170_1.wav");
        assert_eq!(block[12], "bp170_1.wav");
        assert_eq!(block[15], "bp170_4.wav");
        // And the last cell of the whole table, 28 groups on.
        assert_eq!(lord_voice(197, 15).unwrap(), "bp197_4.wav");
    }

    #[test]
    fn groups_outside_the_diplomatic_band_have_no_lord() {
        assert_eq!(lord_voice(169, 0), None);
        assert_eq!(lord_voice(198, 0), None);
        assert_eq!(lord_voice(170, 16), None);
    }

    #[test]
    fn a_system_voice_is_the_group_padded_to_three_digits() {
        assert_eq!(system_voice(100).unwrap(), "S100_01.wav");
        assert_eq!(system_voice(246).unwrap(), "S246_01.wav");
        assert_eq!(system_voice(99), None);
        assert_eq!(system_voice(170), None);
    }

    #[test]
    fn a_slot_number_is_one_based_and_the_movement_sounds_prove_it() {
        // `Unit_MoveInFacing` (0x00466D84): army 12, merchant/transport 11,
        // peasant mob 5. These three lines are the check that fixed the whole
        // indexing - read 0-based they are nothing, army and fallow.
        assert_eq!(slot(Bank::Kingdom, 12), Some("army.wav"));
        assert_eq!(slot(Bank::Kingdom, 11), Some("merchant.wav"));
        assert_eq!(slot(Bank::Kingdom, 5), Some("rioters.wav"));
    }

    #[test]
    fn the_village_job_map_lands_on_the_six_sounds_it_should() {
        // `g_jobSound` (0x004D2950): jobs 1, 2, 3, 5, 6, 7 -> slots 7, 4, 6,
        // 10, 8, 9. The second, independent confirmation of the 1-based read,
        // and the more convincing one, because six unrelated numbers all land
        // on the right work.
        let jobs = [7, 4, 6, 10, 8, 9];
        let want = ["wheat.wav", "moo_2.wav", "fallow.wav", "iron.wav", "stonecut.wav", "woodcut.wav"];
        for (s, w) in jobs.into_iter().zip(want) {
            assert_eq!(slot(Bank::Kingdom, s), Some(w), "slot {s}");
        }
    }

    #[test]
    fn slot_one_is_the_click_and_slot_zero_does_not_exist() {
        assert_eq!(slot(Bank::Kingdom, 1), Some("click3.wav"));
        assert_eq!(slot(Bank::Battle, 1), Some("click3.wav"));
        assert_eq!(slot(Bank::Kingdom, 0), None, "there is no slot 0");
        assert_eq!(slot(Bank::Kingdom, 2), None, "the null.wav hole");
        assert_eq!(slot(Bank::Kingdom, 13), None, "past the twelve");
        assert_eq!(slot(Bank::Battle, 17), Some("siegedoc.wav"), "the last of seventeen");
        assert_eq!(slot(Bank::Battle, 18), None);
    }

    #[test]
    fn the_same_slot_means_two_things_in_the_two_banks() {
        // The trap the two-bank arrangement sets. Slot 5 is the peasant mob on
        // the campaign and a sword swing in a battle.
        assert_eq!(slot(Bank::Kingdom, 5), Some("rioters.wav"));
        assert_eq!(slot(Bank::Battle, 5), Some("sword2.wav"));
    }

    #[test]
    fn the_two_banks_overlap_only_where_the_binary_says_they_do() {
        assert_eq!(KINGDOM_BANK[0], BATTLE_BANK[0], "both banks start on the click");
        assert_eq!(KINGDOM_BANK[1], BATTLE_BANK[1], "and both have the same hole");
        let shared = KINGDOM_BANK
            .iter()
            .filter(|n| BATTLE_BANK.contains(n))
            .count();
        assert_eq!(shared, 2, "otherwise disjoint - they are two banks, not one");
    }
}
