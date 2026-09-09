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
//! `null.wav` is not in either install and never was. Entry 1 of both banks is
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
//! rather than a decision.

/// The five battle tracks, `0x004D9228`, in table order. Index 0 is
/// `battle1.wav`. `[V]`
pub const MUSIC_BATTLE: [&str; 5] = [
    "battle1.wav",
    "battle2.wav",
    "battle3.wav",
    "battle4.wav",
    "battle5.wav",
];

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
/// the village's nine job slots, which is why this table is worth writing down
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
/// `None` for slot 0 (there is no slot 0), for a slot past the bank, and for
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

/// The fanfares, which are played by name rather than out of a bank —
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
    pub const AFTER_BATTLE: &str = "ff_lose.wav";
}

/// The lord voices: `<kt|bn|ct|bp><group>_<1..=4>.wav`, `0x004E0258`.
///
/// A table in the binary — 28 groups × 16 × 16 bytes — but a pure naming
/// convention on disk, so it is generated rather than transcribed.
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
/// are `S<group padded to 3>_01.wav`, so again a convention rather than a
/// transcription.
pub fn system_voice(group: u16) -> Option<String> {
    if (100..=169).contains(&group) || (200..=299).contains(&group) {
        Some(format!("S{group:03}_01.wav"))
    } else {
        None
    }
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
