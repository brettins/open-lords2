#![allow(unused_imports)]
use super::*;
use super::realms_and_counties::*;
use super::units_and_merchants::*;
use l2_formats::save::{Save, SaveError, COUNTY_RECORDS, NEIGHBOUR_SLOTS, REALM_RECORDS};
use l2_testkit::{executable, saves, skip, SaveFile};

/// The arithmetic that validates the whole schema, and it is a property of the
/// executable: `Save_Write`'s table accounts for
/// 267,028 bytes of live memory, then sixteen 12,800-byte castle blocks, and
/// 267,028 + 16 × 12,800 = 471,828.
///
/// If any block length were misread this would not close, so it is the reason to
/// trust every other number read out of a save.
#[test]
fn the_block_table_accounts_for_a_save_exactly() {
    let exe = l2_testkit::executable!();
    let layout = l2_formats::save::Layout::from_executable(&exe).expect("block table");
    let blocks: u32 = layout.blocks().iter().map(|b| b.len).sum();

    assert_eq!(blocks as usize, 267_028, "the saved regions");
    assert_eq!(layout.expected_len(), 471_828, "plus castles.dat as 16 x 12,800");
    assert_eq!(267_028 + 16 * 12_800, 471_828, "and the arithmetic itself");

    // Blocks are contiguous in the file, in table order. A gap or an overlap
    // would put every addressed read at the wrong offset while still summing
    // to the right total.
    let mut at = 0usize;
    for b in layout.blocks() {
        assert_eq!(b.offset, at, "block at {:#010x} does not follow the previous one", b.va);
        at += b.len as usize;
    }
    assert_eq!(at, 267_028);
}

/// Every save that exists is the size the schema says a save is. Nothing else
/// in this file means anything if this does not hold.
#[test]
fn every_save_is_the_length_the_schema_predicts() {
    let saves = saves!();
    for s in &saves {
        let bytes = std::fs::read(&s.path).expect("re-read");
        assert_eq!(bytes.len(), 471_828, "{}", s.label());
        assert_eq!(s.save.layout().expected_len(), bytes.len(), "{}", s.label());
    }
    eprintln!("save length: {} files agree", saves.len());
}

/// A save whose size does not match the schema is refused
/// the wrong offsets. The failure mode this guards against is silent: wrong
/// offsets still yield numbers.
#[test]
fn a_save_of_the_wrong_size_is_refused() {
    let exe = l2_testkit::executable!();
    let saves = saves!();
    for s in &saves {
        let mut bytes = std::fs::read(&s.path).expect("re-read");
        bytes.push(0);
        match Save::open(&exe, &bytes) {
            Err(SaveError::SizeMismatch { expected, actual }) => {
                assert_eq!(expected, 471_828, "{}", s.label());
                assert_eq!(actual, 471_829, "{}", s.label());
            }
            other => panic!("{}: expected a size mismatch, got {other:?}", s.label()),
        }
        bytes.truncate(100);
        assert!(matches!(Save::open(&exe, &bytes), Err(SaveError::SizeMismatch { .. })));
    }
}

/// The array shapes: seventeen county records, six realm records, and record 0
/// of each is a slot. True of every save because it is the
/// shape of the game's `.data`, not of a scenario.
#[test]
fn the_arrays_are_the_shape_the_executable_gives_them() {
    let saves = saves!();
    for s in &saves {
        let counties = s.save.counties().expect("counties");
        let realms = s.save.realms().expect("realms");
        assert_eq!(counties.len(), COUNTY_RECORDS, "{}", s.label());
        assert_eq!(counties.len(), 17, "{}", s.label());
        assert_eq!(realms.len(), REALM_RECORDS, "{}", s.label());
        assert_eq!(realms.len(), 6, "{}", s.label());
        assert!(!realms[0].in_play(), "{}: realm 0 is an array slot", s.label());
        assert!(!counties[0].is_county(), "{}: record 0 is an array slot", s.label());
    }
}

/// **The six 44-byte player slots, and the four bytes that are not a name.**
///
/// `g_saveBlocks[2] = {0x00553D50, 264}` reads as six `0x2C` names starting a
/// dword before `g_playerNames` (`0x00553D54`), which would mean a block
/// misaligned by four bytes with realm 5's record running past its end. It is
/// a **player table** instead: `0x00553D50` is slot 0, the name is `+0x04`, and
/// the four bytes at `+0x00` are the DirectPlay player id
/// (`FUN_0043E9E2` writes `g_dpPlayerId` there; `Mp_DropDepartedPlayers`
/// eliminates a human realm whose copy has gone to zero).
///
/// Three things say the base is right, and each
/// fails loudly under the wrong reading:
///
/// * `+0x00` is **zero** in every save — a single-player game has no
///   connection. Read four bytes later and it is the first four characters of
///   a name, which is never zero for a realm in play.
/// * the local player's `+0x25` is the shield `g_realms[p].shieldIndex` holds,
///   two fields at two different strides agreeing. `Player_SetHuman` writes
///   both.
/// * every realm in play has a name, and none of them fills all 31 bytes:
///   `FUN_00401136` copies a terminated string.
///
/// **A multiplayer save would legitimately carry a non-zero id**, and this is
/// the assertion that would tell us we finally had one. Every save this
/// machine can reach is somebody's solo game.
#[test]
fn the_player_table_names_every_realm_in_play_and_holds_no_connection() {
    use l2_formats::save::{PLAYER_NAME_LEN, PLAYER_RECORDS};
    let saves = saves!();
    for s in &saves {
        let players = s.save.players().expect("players");
        let realms = s.save.realms().expect("realms");
        let local = s.save.globals().expect("globals").local_player as usize;
        assert_eq!(players.len(), PLAYER_RECORDS, "{}", s.label());
        assert_eq!(players.len(), 6, "{}", s.label());

        assert!(!players[0].is_named(), "{}: slot 0 is an array slot", s.label());

        for p in players.iter() {
            assert_eq!(
                p.dp_player_id,
                0,
                "{}: slot {} carries a DirectPlay id, so either this is the first \
                 multiplayer save we have seen or +0x00 is not where we think",
                s.label(),
                p.index,
            );
        }

        for r in realms.iter().filter(|r| r.in_play()) {
            let p = &players[r.index];
            assert!(p.is_named(), "{}: realm {} is in play and unnamed", s.label(), r.index);
            assert!(
                p.name.contains(&0),
                "{}: realm {}'s name fills all {PLAYER_NAME_LEN} bytes and is unterminated",
                s.label(),
                r.index,
            );
        }

        assert!((1..PLAYER_RECORDS).contains(&local), "{}: g_localPlayer {local}", s.label());
        assert_eq!(
            players[local].shield, realms[local].shield_index,
            "{}: the local player's +0x25 and g_realms[{local}].shieldIndex disagree",
            s.label(),
        );
        eprintln!(
            "{}: realm {local} is {:?}, flying shield {}",
            s.label(),
            players[local].name(),
            players[local].shield,
        );
    }
}

