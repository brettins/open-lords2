#![allow(unused_imports)]
use super::*;
use super::realms_and_counties::*;
use super::units_and_merchants::*;
use l2_formats::save::{Save, SaveError, COUNTY_RECORDS, NEIGHBOUR_SLOTS, REALM_RECORDS};
use l2_testkit::{executable, saves, skip, SaveFile};

#[test]
fn the_block_table_accounts_for_a_save_exactly() {
    let exe = l2_testkit::executable!();
    let layout = l2_formats::save::Layout::from_executable(&exe).expect("block table");
    let blocks: u32 = layout.blocks().iter().map(|b| b.len).sum();

    assert_eq!(blocks as usize, 267_028, "the saved regions");
    assert_eq!(layout.expected_len(), 471_828, "plus castles.dat as 16 x 12,800");
    assert_eq!(267_028 + 16 * 12_800, 471_828, "and the arithmetic itself");

    let mut at = 0usize;
    for b in layout.blocks() {
        assert_eq!(b.offset, at, "block at {:#010x} does not follow the previous one", b.va);
        at += b.len as usize;
    }
    assert_eq!(at, 267_028);
}

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

/// `g_saveBlocks[2] = {0x00553D50, 264}` reads as six `0x2C` names starting a
/// dword before `g_playerNames` (`0x00553D54`), which would mean a block
/// misaligned by four bytes with realm 5's record running past its end. It is
/// a **player table** instead: `0x00553D50` is slot 0, the name is `+0x04`, and
/// the four bytes at `+0x00` are the DirectPlay player id
/// (`FUN_0043E9E2` writes `g_dpPlayerId` there; `Mp_DropDepartedPlayers`
/// eliminates a human realm whose copy has gone to zero).
///
/// * `+0x00` is **zero** in every save — a single-player game has no
///   connection. Read four bytes later and it is the first four characters of
///   a name, which is never zero for a realm in play.
///
/// * the local player's `+0x25` is the shield `g_realms[p].shieldIndex` holds,
///   two fields at two different strides agreeing. `Player_SetHuman` writes
///   both.
///
///   `FUN_00401136` copies a terminated string.
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

