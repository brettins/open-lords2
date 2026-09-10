//! **The auto-repeat ramp, against the player's own `Lords2.exe`.**
//!
//! `crate::press::REPEAT_GATE` is forty-eight bytes copied out of the game at
//! `0x004D2748`, and the reason it is checked here rather than merely commented
//! is `docs/agents.md`: *ablating a constant while computing your probe from
//! that same constant tests nothing at all.* Every unit test beside the
//! constant — the fire schedule, the acceleration, the first-repeat delay — is
//! computed **from** the table, so all of them agree with any table whatever.
//! The exe is the only thing in the world that can disagree.

use l2_game::press;

/// The table is the game's, byte for byte.
///
/// Ablated, and this is what it printed: changing entry 14 from 1 to 0 fails
/// with *"byte 14: ours 0, the game's 1"*, and the whole-array assertion under
/// it fails too. Nothing else in the suite moves, which is the point — the
/// schedule tests are derived from the constant and stay green.
#[test]
fn the_repeat_ramp_is_the_exes_own_forty_eight_bytes() {
    let exe = l2_testkit::executable!();
    let t = l2_testkit::pe::Table::at(&exe, press::REPEAT_GATE_ADDR);
    let theirs: Vec<u8> = (0..press::REPEAT_GATE.len()).map(|i| t.u8_at(i)).collect();

    for (i, (&ours, &game)) in press::REPEAT_GATE.iter().zip(theirs.iter()).enumerate() {
        assert_eq!(
            ours, game,
            "byte {i}: ours {ours}, the game's {game}. \
             The ramp at {:#010X} is hand-authored, not a formula — copy it, do not derive it.",
            press::REPEAT_GATE_ADDR,
        );
    }
    assert_eq!(press::REPEAT_GATE.to_vec(), theirs);

    // **And the two indices the game never reads are still checked above**,
    // deliberately: entries 0..8 and entry 47 are dead, and the constant is a
    // copy of the bytes rather than an interpretation of them. If a future
    // reader trims them, this goes red and the trim has to be argued for.
    assert_eq!(&theirs[..8], &[8, 8, 8, 8, 8, 8, 8, 8], "the eight dead leading bytes");
    assert_eq!(theirs[47], 0, "entry 47 is dead because the clamp branch skips the table");
}
