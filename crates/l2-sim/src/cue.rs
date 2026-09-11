//! **What happened inside a tick that a listener could hear** — a record the
//! simulation writes and never reads.
//!
//! # Why this exists
//!
//! `Lords2.exe` plays 25 of its 143 sounds from *inside* the per-man state
//! machine: `Melee_Tick` (`0x00494908`) plays a sword when a man falls and a
//! death cry when the last of a figure does, `BattleMan_FireMissile`
//! (`0x00483337`) plays the bow as the arrow leaves, `Missile_Step`
//! (`0x00492C8B`) plays the hit, `FUN_0049694F` plays the wall coming down. Each
//! is an *event inside a tick*. The state after the tick says where a man
//! stands; it does not say that he struck, and `crate::runner` had no
//! event stream at all — it resolved unit and figure state and nothing else.
//! That, and not the sounds, was the gap.
//!
//! # Why counters, and why that is exact rather than a shortcut
//!
//! Every one of those call sites goes through `Sound_PlaySlot` (`0x00426120`)
//! or `Sound_PlayFile` (`0x00427990`), and **both drop the request while the
//! buffer they would use is still sounding** — `GetStatus` against
//! `DSBSTATUS_PLAYING` for a bank slot, `Sound_OneShotBusy` for the one-shot
//! buffer. `[V]` from both bodies. Ten men falling to swords in one frame is
//! therefore one `sword2.wav` and nine dropped requests, in either order.
//!
//! So what a listener needs from a tick is **whether each kind of event
//! happened at least once**, not how many times or in what order — and a
//! monotone counter per kind answers exactly that when it is compared with its
//! value at the previous listen. It is a record of the simulation's own
//! occasions, keyed by what the original's call site branches on (the killer's
//! troop type, the dying figure's side, the weapon class) and **not** by sound:
//! which file those keys mean is `l2-game`'s business, and nothing here knows a
//! sound exists.
//!
//! # Why it cannot feed back
//!
//! `docs/netcode.md` D-3. The cue is written by the simulation and read by
//! nothing in it; the listener is `l2_game::audio::Director`, which holds a
//! shared reference to the whole game and cannot write. Two properties keep
//! that true and both are tested rather than asserted:
//!
//! * **the simulation never reads its cues** — `crate::runner`'s
//!   `a_battle_whose_cues_are_wiped_every_tick_is_the_same_battle` runs two
//!   copies, zeroes one's record every tick, and requires every other field to
//!   agree;
//! * **they are not in the lockstep checksum**, on purpose. They carry nothing
//!   two peers could disagree about that the figures do not already carry, and
//!   putting a presentation record into the digest would make "which sounds we
//!   reproduce" a network-protocol version. `tests/lockstep.rs`' census walks
//!   `Missile`, `Fighter` and `SiegeState`, and this is none of them.

use crate::figure::{Side, SIDE_A};
use crate::missile::WeaponClass;
use crate::troop::Troop;

/// Monotone counts of the per-man events the original asks a sound for.
///
/// Fields are private so that nothing outside this crate can *write* one — a
/// screen that could bump a counter could fake a battle's sound — and every
/// writer inside it is `pub(crate)`. Reading is free.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cues {
    /// A man fell to a melee blow, by the **striker's** troop index —
    /// `Melee_Tick`'s `hits > 99` arm, which picks its sword by
    /// `g_battleMen[other].troopType`.
    melee_casualties: [u32; 11],
    /// A figure lost its last man in melee, by **its own** side:
    /// `[side 0, side 4]` — `Melee_Tick`'s `men < 1` arm.
    melee_deaths: [u32; 2],
    /// A missile struck a man, `[bow, crossbow]` — `Missile_Step`, before the
    /// casualty test.
    missile_hits: [u32; 2],
    /// …and that hit crossed the casualty threshold, `[bow, crossbow]`.
    missile_casualties: [u32; 2],
    /// …and it was the figure's last man.
    missile_deaths: u32,
    /// A missile was loosed, `[bow, crossbow, catapult]`.
    loosed: [u32; 3],
    /// A catapult shot counted against a wall cell.
    walls_struck: u32,
    /// `Wall_Smash` (`FUN_0049694F`) opened a stretch of wall.
    walls_smashed: u32,
}

fn side_index(side: Side) -> usize {
    if side == SIDE_A {
        0
    } else {
        1
    }
}

fn bump(n: &mut u32) {
    *n = n.wrapping_add(1);
}

impl Cues {
    pub fn melee_casualties(&self, striker: Troop) -> u32 {
        self.melee_casualties[striker.index()]
    }
    pub fn melee_deaths(&self, side: Side) -> u32 {
        self.melee_deaths[side_index(side)]
    }
    /// Zero for [`WeaponClass::Catapult`], which cannot hit a man.
    pub fn missile_hits(&self, class: WeaponClass) -> u32 {
        match class {
            WeaponClass::Bow => self.missile_hits[0],
            WeaponClass::Crossbow => self.missile_hits[1],
            WeaponClass::Catapult => 0,
        }
    }
    pub fn missile_casualties(&self, class: WeaponClass) -> u32 {
        match class {
            WeaponClass::Bow => self.missile_casualties[0],
            WeaponClass::Crossbow => self.missile_casualties[1],
            WeaponClass::Catapult => 0,
        }
    }
    pub fn missile_deaths(&self) -> u32 {
        self.missile_deaths
    }
    pub fn loosed(&self, class: WeaponClass) -> u32 {
        self.loosed[class.index() as usize - 1]
    }
    pub fn walls_struck(&self) -> u32 {
        self.walls_struck
    }
    pub fn walls_smashed(&self) -> u32 {
        self.walls_smashed
    }

    /// **One of every occasion the record can hold** — for a census of what a
    /// listener can be asked to play, not for a battle.
    ///
    /// An exhaustive literal with no `..`, so a counter added to this struct
    /// does not compile until it is given a value here, and a census built on
    /// this cannot silently miss it. `docs/agents.md`: prefer a shape that
    /// cannot be wrong.
    pub fn of_every_occasion() -> Cues {
        Cues {
            melee_casualties: [1; 11],
            melee_deaths: [1; 2],
            missile_hits: [1; 2],
            missile_casualties: [1; 2],
            missile_deaths: 1,
            loosed: [1; 3],
            walls_struck: 1,
            walls_smashed: 1,
        }
    }

    /// Whether any count is **behind** `earlier`, which no single battle can
    /// produce — so a listener holding a previous battle's record knows it is
    /// looking at a new one.
    pub fn is_behind(&self, earlier: &Cues) -> bool {
        let pairs = self
            .melee_casualties
            .iter()
            .zip(&earlier.melee_casualties)
            .chain(self.melee_deaths.iter().zip(&earlier.melee_deaths))
            .chain(self.missile_hits.iter().zip(&earlier.missile_hits))
            .chain(self.missile_casualties.iter().zip(&earlier.missile_casualties))
            .chain(self.loosed.iter().zip(&earlier.loosed))
            .chain([
                (&self.missile_deaths, &earlier.missile_deaths),
                (&self.walls_struck, &earlier.walls_struck),
                (&self.walls_smashed, &earlier.walls_smashed),
            ]);
        for (now, was) in pairs {
            if now < was {
                return true;
            }
        }
        false
    }

    pub(crate) fn melee_casualty(&mut self, striker: Troop) {
        bump(&mut self.melee_casualties[striker.index()]);
    }
    pub(crate) fn melee_death(&mut self, side: Side) {
        bump(&mut self.melee_deaths[side_index(side)]);
    }
    pub(crate) fn missile_hit(&mut self, class: WeaponClass) {
        match class {
            WeaponClass::Bow => bump(&mut self.missile_hits[0]),
            WeaponClass::Crossbow => bump(&mut self.missile_hits[1]),
            WeaponClass::Catapult => {}
        }
    }
    pub(crate) fn missile_casualty(&mut self, class: WeaponClass) {
        match class {
            WeaponClass::Bow => bump(&mut self.missile_casualties[0]),
            WeaponClass::Crossbow => bump(&mut self.missile_casualties[1]),
            WeaponClass::Catapult => {}
        }
    }
    pub(crate) fn missile_death(&mut self) {
        bump(&mut self.missile_deaths);
    }
    pub(crate) fn loose(&mut self, class: WeaponClass) {
        bump(&mut self.loosed[class.index() as usize - 1]);
    }
    pub(crate) fn wall_struck(&mut self) {
        bump(&mut self.walls_struck);
    }
    pub(crate) fn wall_smashed(&mut self) {
        bump(&mut self.walls_smashed);
    }
}
