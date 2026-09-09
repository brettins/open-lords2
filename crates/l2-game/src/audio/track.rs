//! **Which track plays, and why.** The one part of the audio layer that is a
//! *rule* rather than plumbing, so it is pure, has no dependencies, and is
//! tested without a device or an install.
//!
//! # The campaign track is how much of the map you hold
//!
//! `FUN_00499ACA` — reached from `Opt_ToggleMusic` when `g_battlePhase` is 0,
//! which is what identifies it as the campaign picker — is a ladder:
//!
//! ```c
//! if      (g_realms[g_localPlayer].countyCount    <  2)     scroll1.wav
//! else if (g_realms[g_localPlayer].shareOfMapPct  <  8)     scroll1.wav
//! else if (g_realms[g_localPlayer].shareOfMapPct  < 15)     scroll2.wav
//! else if (g_realms[g_localPlayer].shareOfMapPct  < 29)     scroll3.wav
//! else if (g_realms[g_localPlayer].shareOfMapPct  < 43)     scroll4.wav
//! else                                                      scroll5.wav
//! ```
//!
//! `shareOfMapPct` is realm `+0x60`, rebuilt every season by `FUN_0049D1E0` as
//! `PctOf(countyCount, g_countyCount)` — **the percentage of the map's
//! counties this realm owns**, and nothing else. `[V]`
//!
//! So the music is a progress bar. A player who has just started hears
//! `Scroll1`; a player who has taken most of England hears `Scroll5`. It is
//! not random and it does not cycle. A person who played this game remembered
//! it as *"music changed based on how far you were in the game, possibly army
//! sizes or number of counties owned"* and as *"scroll1 almost always played
//! in the first map right away"* — both halves are right, and the first clause
//! of the ladder is why the second half is *almost* always: **one county is
//! `Scroll1` whatever the map size**, and only from the second county does the
//! percentage decide.
//!
//! Armies are not in it. `shareOfMapPct` is counties over counties; the
//! realm's `strength` byte (`3 × counties + armies`) is a different quantity
//! and this function does not read it.
//!
//! # The battle track alternates in pairs
//!
//! `FUN_00477B2F`, reached the same way with `g_battlePhase` 2, keeps a 0/1
//! toggle and adds a fixed base:
//!
//! | | counter | tracks |
//! |---|---|---|
//! | a field battle | `DAT_00553D28` | `Battle1` ↔ `Battle2` |
//! | a siege | `DAT_00553540` | `Battle3` ↔ `Battle4` |
//! | `DAT_0057A0F0` set | `DAT_00553540` | `Battle4` ↔ `Battle5` |
//!
//! The counter is incremented *before* use and wraps above 1, so the first
//! battle of a session plays the **second** track of its pair, not the first.
//! `[V]` — it is `n = n + 1; if (1 < n) n = 0;` and the globals start at zero.
//!
//! `DAT_0057A0F0` is not identified. It also picks how `g_castleLevel` is
//! derived a few lines later in `FUN_00477C89`, so it distinguishes a battle
//! fought on the campaign's own map from one that is not; [`BattleKind`]
//! carries the two we can name and leaves the third out rather than guessing
//! at it.
//!
//! # The branch that never runs
//!
//! Both pickers open with `FUN_004AF841("scroll2.wav")` / `("battle2.wav")` —
//! **a file-existence probe**, not a "is it playing" query: it opens the file,
//! closes it, and retries once after a `chdir` to the install. A zero means
//! the file is missing, and only then (and only in multiplayer) does a
//! shortened three-branch ladder run. Both files ship, so **on any complete
//! install those branches are dead**. They are the fallback for a minimal
//! install, and they are not reproduced here. `[V]`

/// A music track: which of the two sets, and which of its five.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Music {
    /// `Scroll1‑5` — the campaign map and the county screens.
    Scroll(u8),
    /// `Battle1‑5`.
    Battle(u8),
}

impl Music {
    /// The file name, lower-cased as the binary's tables spell it.
    pub fn file(self) -> &'static str {
        match self {
            Music::Scroll(n) => super::names::MUSIC_SCROLL[(n.max(1).min(5) - 1) as usize],
            Music::Battle(n) => super::names::MUSIC_BATTLE[(n.max(1).min(5) - 1) as usize],
        }
    }
}

/// What kind of battle is being fought, which is what picks the pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleKind {
    /// `g_battleIsSiege == 0` — `Battle1` ↔ `Battle2`.
    Field,
    /// `g_battleIsSiege != 0` — `Battle3` ↔ `Battle4`.
    Siege,
}

/// `FUN_00499ACA`'s ladder: the campaign track for a realm holding
/// `county_count` counties, which is `share_of_map_pct` of the map.
///
/// The two arguments are the two the original reads, in the order it reads
/// them, so the first clause's precedence is visible rather than folded away.
pub fn campaign(county_count: u8, share_of_map_pct: i32) -> Music {
    // A realm down to its last county gets Scroll1 regardless of how small the
    // map is - on a four-county map one county is 25%, which the percentage
    // ladder would answer with Scroll3.
    if county_count < 2 {
        return Music::Scroll(1);
    }
    Music::Scroll(match share_of_map_pct {
        i32::MIN..=7 => 1,
        8..=14 => 2,
        15..=28 => 3,
        29..=42 => 4,
        _ => 5,
    })
}

/// The pair of toggles `FUN_00477B2F` keeps, and the rule that steps them.
///
/// It is a struct rather than two loose counters because the *state* is the
/// finding: the original does not choose a battle track, it advances a
/// counter, and two consecutive field battles are guaranteed to sound
/// different.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BattleCycle {
    /// `DAT_00553D28`.
    field: u8,
    /// `DAT_00553540`.
    siege: u8,
}

impl BattleCycle {
    /// Advance the right counter and answer the track — `FUN_00477B2F`.
    ///
    /// Called once when a battle begins, exactly as the original calls it once
    /// from `Battle_Begin` and again from `Opt_ToggleMusic` when music is
    /// switched back on mid-battle. Switching music off and on therefore
    /// *changes the track*, which is the original's behaviour and not a
    /// mistake in this port.
    pub fn next(&mut self, kind: BattleKind) -> Music {
        match kind {
            BattleKind::Field => {
                self.field = if self.field >= 1 { 0 } else { self.field + 1 };
                Music::Battle(1 + self.field)
            }
            BattleKind::Siege => {
                self.siege = if self.siege >= 1 { 0 } else { self.siege + 1 };
                Music::Battle(3 + self.siege)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_county_is_scroll1_whatever_share_that_is() {
        // The clause the ladder checks first, and the reason a player
        // remembers Scroll1 opening every game. On a four-county map one
        // county is 25%, which the percentage ladder alone would call Scroll3.
        assert_eq!(campaign(1, 25), Music::Scroll(1));
        assert_eq!(campaign(0, 100), Music::Scroll(1));
    }

    #[test]
    fn england_turn_one_opens_on_scroll1() {
        // Fourteen counties, one owned: PctOf(1, 14) = 7. Both clauses agree
        // here, which is why the fixture cannot tell them apart on its own -
        // hence the test above.
        assert_eq!(campaign(1, 7), Music::Scroll(1));
    }

    #[test]
    fn the_ladder_climbs_at_8_15_29_and_43() {
        // Each threshold checked on both sides. These five numbers are the
        // whole of the rule and a fencepost in any of them is inaudible until
        // somebody plays for an hour.
        assert_eq!(campaign(2, 7), Music::Scroll(1));
        assert_eq!(campaign(2, 8), Music::Scroll(2));
        assert_eq!(campaign(2, 14), Music::Scroll(2));
        assert_eq!(campaign(2, 15), Music::Scroll(3));
        assert_eq!(campaign(2, 28), Music::Scroll(3));
        assert_eq!(campaign(2, 29), Music::Scroll(4));
        assert_eq!(campaign(2, 42), Music::Scroll(4));
        assert_eq!(campaign(2, 43), Music::Scroll(5));
        assert_eq!(campaign(9, 100), Music::Scroll(5));
    }

    #[test]
    fn a_negative_share_cannot_climb_the_ladder() {
        // `shareOfMapPct` is a byte in the original and cannot go negative;
        // ours is the `i32` `pct_of` returns. Pinning the bottom of the match
        // keeps a future signed reader from falling through to Scroll5.
        assert_eq!(campaign(2, -1), Music::Scroll(1));
    }

    #[test]
    fn the_first_battle_of_a_session_plays_the_second_track() {
        // `n = n + 1; if (1 < n) n = 0;` from zero, so the increment lands
        // first. Not a detail worth inventing, and not one worth losing.
        let mut c = BattleCycle::default();
        assert_eq!(c.next(BattleKind::Field), Music::Battle(2));
        assert_eq!(c.next(BattleKind::Field), Music::Battle(1));
        assert_eq!(c.next(BattleKind::Field), Music::Battle(2));
    }

    #[test]
    fn sieges_have_their_own_pair_and_their_own_counter() {
        let mut c = BattleCycle::default();
        assert_eq!(c.next(BattleKind::Siege), Music::Battle(4));
        assert_eq!(c.next(BattleKind::Field), Music::Battle(2));
        // The field battle in between did not disturb the siege counter.
        assert_eq!(c.next(BattleKind::Siege), Music::Battle(3));
    }

    #[test]
    fn every_track_resolves_to_a_shipped_file_name() {
        for n in 1..=5u8 {
            assert_eq!(Music::Scroll(n).file(), format!("scroll{n}.wav"));
            assert_eq!(Music::Battle(n).file(), format!("battle{n}.wav"));
        }
    }
}
