//! **Which of `batfield.pl8`'s 48 frames a field battle is fought on** — the
//! 48-byte table at `0x0057CAE0` and the cursor at `0x005653F8`.
//!
//! The frame is not drawn per battle. `PlayerStart_Shuffle` (`0x00497E65`)
//! deals the whole order once at new-game time and
//! `Battlefield_BuildRandom` (`0x0047AAA3`) walks it one entry per battle:
//!
//! ```c
//! if (DAT_0057a0f0 == 0) {                                  // not a skirmish
//!   DAT_00553528 = (uint)(byte)(&DAT_0057cae0)[DAT_005653f8];
//!   DAT_005653f8 = DAT_005653f8 + 1;
//!   if (0x2e < DAT_005653f8) { DAT_005653f8 = 0; }
//! }
//! ```
//!
//! Both are net-synced at every battle start — `FUN_00444A2F` writes
//! `Net_WriteField(&DAT_005653F8, 4); Net_WriteField(&DAT_0057CAE0, 0x30);`
//! and `FUN_00444A9B` reads the pair back — which is why the walk has to be
//! state and not a draw. Neither is a save block: `docs/stored-fields.json`
//! has no row for either address, so the original carries the order across a
//! `.sav` load only by leaving the globals alone. Ours is in our own save
//! (`save::codec` entry 31) because a lockstep peer that loads must walk the
//! same order.
//!
//! `Sync_Rollback` (`0x0043F5C5`) steps the cursor the other way —
//! `if (DAT_005653f8 < 1) { DAT_005653f8 = 0x2e; } else { DAT_005653f8 - 1; }`
//! — when it rewinds a battle. We have no rollback path to hang that on, so it
//! is recorded here and not implemented.

use crate::Pcg32;

/// `Net_WriteField(&DAT_0057CAE0, 0x30)` — 48 bytes, one per `batfield.pl8`
/// frame.
pub const PLAYLIST_LEN: usize = 0x30;

/// **The walk stops one short of the table.** `Battlefield_BuildRandom` wraps
/// at `0x2e < DAT_005653f8`, so the cursor runs 0 … 0x2E and entry `0x2F` is
/// dealt, net-synced and never read. [V] — the comparison is `0x2e`, not
/// `0x2f`, in `0x0047AAA3`.
pub const WALK_LEN: usize = 0x2f;

/// The dealt order and the cursor into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldPlaylist {
    frames: [u8; PLAYLIST_LEN],
    cursor: usize,
}

impl Default for FieldPlaylist {
    fn default() -> Self {
        FieldPlaylist::empty()
    }
}

impl FieldPlaylist {
    /// What the globals hold before a new game deals them: `.bss` zeros.
    pub fn empty() -> FieldPlaylist {
        FieldPlaylist { frames: [0; PLAYLIST_LEN], cursor: 0 }
    }

    /// `PlayerStart_Shuffle` (`0x00497E65`), second half. [V] against the
    /// decompilation; the draws are ours, per `docs/decisions.md` C62 — the
    /// original scatters on the shared `Rand_Advance` stream (`g_rand7B`),
    /// which we cannot reproduce, so the table gets its own `Pcg32` off the
    /// game seed the way [`crate::diplomacy::Diplomacy::new`] does.
    ///
    /// ```c
    /// for (i = 0; i < 0x30; i++) { auStack_c4[i] = i; }
    /// for (i = 0; i < 0x30; i++) { (&DAT_0057cae0)[i] = 0; }
    /// for (i = 0; i <= 0x2f; i++) {
    ///   Rand_Advance();
    ///   pos = (g_rand7B & 0x1f) + 1 + i;
    ///   if (0x2f < pos) { pos = pos - 0x30; }
    ///   for (t = 0; t < 0x32; t++) {
    ///     if ((pos != i) && ((&DAT_0057cae0)[pos] == '\0')) {
    ///       (&DAT_0057cae0)[pos] = (char)auStack_c4[i]; break;
    ///     }
    ///     pos = pos + 1; if (0x2f < pos) { pos = 0; }
    ///   }
    /// }
    /// ```
    ///
    /// **It is a scatter and not a permutation, and that is the original's.**
    /// Zero is both frame 0 and the free marker, so writing source 0 into a
    /// slot leaves that slot free for a later source to take; and a source
    /// whose 50 probes all land on taken slots is dropped. Frames 1 … 0x2F
    /// therefore appear at most once each and the rest of the table stays 0,
    /// which is frame 0 again. `docs/bugs.md` shape: not fixed.
    pub fn deal(seed: u64) -> FieldPlaylist {
        let mut rng = Pcg32::from_seed(seed);
        let mut frames = [0u8; PLAYLIST_LEN];
        for src in 0..PLAYLIST_LEN {
            let mut pos = (rng.next_u32() & 0x1f) as usize + 1 + src;
            if pos > 0x2f {
                pos -= 0x30;
            }
            // `while (iVar1 = local_f4 + 1, local_f4 < 0x32)` — 50 probes, the
            // table being 48 long.
            for _ in 0..0x32 {
                if pos != src && frames[pos] == 0 {
                    frames[pos] = src as u8;
                    break;
                }
                pos += 1;
                if pos > 0x2f {
                    pos = 0;
                }
            }
        }
        FieldPlaylist { frames, cursor: 0 }
    }

    /// `Battlefield_BuildRandom` (`0x0047AAA3`) — read `DAT_00553528`, then
    /// bump and wrap. One call per open-field battle; a skirmish takes the
    /// `DAT_0057A0F0` branch instead and never touches this.
    pub fn take_next(&mut self) -> u8 {
        let frame = self.frames[self.cursor];
        self.cursor += 1;
        if self.cursor > WALK_LEN - 1 {
            self.cursor = 0;
        }
        frame
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn frames(&self) -> &[u8; PLAYLIST_LEN] {
        &self.frames
    }

    /// For the save codec, which restores both halves of the state
    /// `Net_WriteField` sends.
    pub fn restore(frames: [u8; PLAYLIST_LEN], cursor: usize) -> FieldPlaylist {
        FieldPlaylist { frames, cursor: cursor.min(WALK_LEN - 1) }
    }
}
