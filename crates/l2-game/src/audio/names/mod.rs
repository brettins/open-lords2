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
//! This is the trap, and it is invisible in the decompiler unless you compare
//! two base addresses. `Sound_LoadBank` **stores** at
//! `&DAT_00522B00 + i * 4` counting `i` from 0. `Sound_PlaySlot` and
//! `Sound_RestartSlot` **read** from `&DAT_00522AFC + slot * 4` — and
//! `0x00522AFC` is four bytes *below* `0x00522B00`. So
//!
//! `[V]`, and confirmed twice over by what the numbers then mean.
//!
//! Reading the slots as 0-based makes `click3.wav` look unplayable, because
//! nothing passes 0. It is slot **1**, and `Widget_Test` (`0x0040DA1E`) plays
//! it every time a widget is pressed. [`slot`] is the one place that
//! conversion happens.
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

mod audio_tables;
pub use audio_tables::*;

/// The five battle tracks, `0x004D9228`, in table order. Index 0 is
/// `battle1.wav`. `[V]`
pub const MUSIC_BATTLE: [&str; 5] = [
    "battle1.wav",
    "battle2.wav",
    "battle3.wav",
    "battle4.wav",
    "battle5.wav",
];

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

pub fn troop_cry(troop: usize, class: usize, take: usize) -> Option<&'static str> {
    match *TROOP_CRIES.get(troop)?.get(class)?.get(take)? {
        "null.wav" => None,
        name => Some(name),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bank {
    /// `FUN_00499D7D`'s twelve, loaded when the campaign comes up.
    Kingdom,
    /// `FUN_00499D97`'s seventeen, loaded when a battle does.
    Battle,
}

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

/// **`TileInfo_Draw` (`0x0041C208`) — the bank slot a resource site plays when
/// the information panel opens on it.**
///
/// The ladder is on `g_pickedTileGraphic` behind `flags & 0x80`, and it is the
/// *same* four ranges [`l2_kingdom::industry::map_toggle_for_graphic`] uses to
/// decide which industry a click on that tile switches — so the two agree by
/// construction. `[V]`
///
/// **The blacksmith plays the quarry's sound**
/// job 8 does. `[V]` at both sites; `[I]` that it is because the kingdom bank
/// has no forge in it.
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

/// **The tip screens' chained takes** — `FUN_004B3ACD`'s five-byte rows at
/// `0x004E1E40 + group * 5`, for the ten groups whose row is not all zero.
///
/// `[V]` read out of the executable; every other row in 200…219 is zero.
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

pub fn tip_take(group: u16, cursor: usize) -> u8 {
    TIP_TAKES
        .iter()
        .find(|(g, _)| *g == group)
        .and_then(|(_, row)| row.get(cursor))
        .copied()
        .unwrap_or(0)
}

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

