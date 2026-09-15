#![allow(unused_imports)]
use super::*;
use super::tests_part::*;
use l2_view::Canvas;
use crate::input::{Event, Key};

/// **One record of `g_playerNames`** (`0x00553D54`) — a lord's name as the
/// interface shows it.
///
/// Fixed width on purpose
/// 44-byte slots at `0x00553D50` and one save block of its own
/// (`g_saveBlocks[2] = {0x00553D50, 264}`); the name is 31 bytes at `+4`, the
/// banner colour is `+0x25` and `+0x27` is the "a person drives this" byte
/// `Player_SetHuman` sets. `+0x00` — the four bytes the name sits after — is
/// the DirectPlay player id, which is what makes the block's base four lower
/// than `g_playerNames` without anything being misaligned;
/// [`l2_formats::save::Player`] reads the slot and
/// [`crate::scenario::from_save`] fills this array from it. A `String` here
/// would put a length prefix and an allocation into something the original
/// writes as a fixed run of bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PlayerName([u8; PLAYER_NAME_LEN]);

impl PlayerName {
    pub const EMPTY: PlayerName = PlayerName([0; PLAYER_NAME_LEN]);

    pub fn new(s: &str) -> PlayerName {
        let mut out = [0u8; PLAYER_NAME_LEN];
        for (slot, c) in out.iter_mut().zip(s.chars().filter(|c| (*c as u32) < 0x100)) {
            *slot = c as u8;
        }
        PlayerName(out)
    }

    pub fn bytes(&self) -> &[u8; PLAYER_NAME_LEN] {
        &self.0
    }

    pub fn from_bytes(b: [u8; PLAYER_NAME_LEN]) -> PlayerName {
        PlayerName(b)
    }

    pub fn as_str(&self) -> String {
        self.0.iter().take_while(|b| **b != 0).map(|b| *b as char).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.0[0] == 0
    }
}

impl std::fmt::Debug for PlayerName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl Default for PlayerName {
    fn default() -> PlayerName {
        PlayerName::EMPTY
    }
}

