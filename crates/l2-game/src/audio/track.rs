//! `FUN_00499ACA` — reached from `Opt_ToggleMusic` when `g_battlePhase` is 0,
//! which is what identifies it as the campaign picker — is a ladder:
//!
//! `shareOfMapPct` is realm `+0x60`, rebuilt every season by `FUN_0049D1E0` as
//! `PctOf(countyCount, g_countyCount)` — **the percentage of the map's
//! counties this realm owns**, and nothing else. `[V]`
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
//! `[V]` — it is `n = n + 1; if (1 < n) n = 0;` and the globals start at zero.
//!
//! `DAT_0057A0F0` is not identified. It also picks how `g_castleLevel` is
//! derived a few lines later in `FUN_00477C89`, so it distinguishes a battle
//! fought on the campaign's own map from one that is not; [`BattleKind`]
//! carries the two we can name and leaves the third out
//! at it.
//!
//! Both pickers open with `FUN_004AF841("scroll2.wav")` / `("battle2.wav")` —
//! **a file-existence probe**, not a "is it playing" query: it opens the file,
//! closes it, and retries once after a `chdir` to the install. A zero means
//! the file is missing, and only then (and only in multiplayer) does a
//! shortened three-branch ladder run. Both files ship, so **on any complete
//! install those branches are dead**. They are the fallback for a minimal
//! install, and they are not reproduced here. `[V]`

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Music {
    Scroll(u8),
    Battle(u8),
    /// `Music_Play` (`0x004263AD`) is a **ninth** way to start a sound and
    /// `docs/audio-triggers.md`'s enumeration had eight. Nine call sites use it
    /// and every one of them is the front end — `App_WinMain` (`0x0040E9AB`)
    /// at start-up, `FUN_00497A34` on the way back to the title,
    /// `Screen_DrawConquest` over the interstitial, `Smk_OnFinished` when the
    /// intro films end, and two screens of `Screen_FrameInput`'s own ladder.
    Setup,
    /// **`setup2.wav`, played once** — `Screen_DrawConquest` (`0x0041E1DD`)'s
    /// other arm: `Music_Play(g_campaignMap < 8 ? "setup.wav" : "setup2.wav",
    /// 0, g_campaignMap < 8)`. The finished campaign's interstitial is the
    /// only screen in the game whose bed does not loop.
    Setup2,
}

impl Music {
    pub fn file(self) -> &'static str {
        match self {
            Music::Scroll(n) => super::names::MUSIC_SCROLL[(n.max(1).min(5) - 1) as usize],
            Music::Battle(n) => super::names::MUSIC_BATTLE[(n.max(1).min(5) - 1) as usize],
            Music::Setup => super::names::MUSIC_SETUP,
            Music::Setup2 => super::names::MUSIC_SETUP2,
        }
    }

    pub fn loops(self) -> bool {
        !matches!(self, Music::Setup2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleKind {
    Field,
    Siege,
}

/// `FUN_00499ACA`'s ladder: the campaign track for a realm holding
/// `county_count` counties, which is `share_of_map_pct` of the map.
pub fn campaign(county_count: u8, share_of_map_pct: i32) -> Music {
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BattleCycle {
    /// `DAT_00553D28`.
    field: u8,
    /// `DAT_00553540`.
    pub(super) siege: u8,
}

impl BattleCycle {
    /// Advance the right counter and answer the track — `FUN_00477B2F`.
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
        assert_eq!(campaign(1, 25), Music::Scroll(1));
        assert_eq!(campaign(0, 100), Music::Scroll(1));
    }

    #[test]
    fn england_turn_one_opens_on_scroll1() {
        assert_eq!(campaign(1, 7), Music::Scroll(1));
    }

    #[test]
    fn the_ladder_climbs_at_8_15_29_and_43() {
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
        assert_eq!(campaign(2, -1), Music::Scroll(1));
    }

    #[test]
    fn the_first_battle_of_a_session_plays_the_second_track() {
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
