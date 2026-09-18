//! `Lords2.exe` plays 25 of its 143 sounds from *inside* the per-man state
//! machine: `Melee_Tick` (`0x00494908`) plays a sword when a man falls and a
//! death cry when the last of a figure does, `BattleMan_FireMissile`
//! (`0x00483337`) plays the bow as the arrow leaves, `Missile_Step`
//! (`0x00492C8B`) plays the hit, `FUN_0049694F` plays the wall coming down. Each
//! is an *event inside a tick*. The state after the tick says where a man
//! stands; it does not say that he struck, and `crate::runner` had no
//! event stream at all — it resolved unit and figure state and nothing else.
//!
//! Every one of those call sites goes through `Sound_PlaySlot` (`0x00426120`)
//! or `Sound_PlayFile` (`0x00427990`), and **both drop the request while the
//! buffer they would use is still sounding** — `GetStatus` against
//! `DSBSTATUS_PLAYING` for a bank slot, `Sound_OneShotBusy` for the one-shot
//! buffer. `[V]` from both bodies. Ten men falling to swords in one frame is
//! therefore one `sword2.wav` and nine dropped requests, in either order.

use crate::figure::{Side, SIDE_A};
use crate::missile::WeaponClass;
use crate::troop::Troop;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cues {
    melee_casualties: [u32; 11],
    melee_deaths: [u32; 2],
    missile_hits: [u32; 2],
    missile_casualties: [u32; 2],
    missile_deaths: u32,
    loosed: [u32; 3],
    /// **State 17 loosed its own missile**, `[bow, crossbow]` —
    /// `BattleMan_StateCloseToAttack` (`0x00484BF9`), a call site apart from
    /// `BattleMan_FireMissile`'s.
    closing_loosed: [u32; 2],
    walls_struck: u32,
    walls_missed: u32,
    /// `Wall_Smash` (`FUN_0049694F`) opened a stretch of wall.
    walls_smashed: u32,
    /// A pot of boiling oil was poured — `FUN_0047A814`, from either of its
    /// two callers, `Melee_Tick` and `BattleUnit_Order`.
    oil_poured: u32,
    /// A siege tower docked with a wall — `FUN_00491492`'s success arm.
    towers_docked: u32,
    /// A bridge caught fire — `FUN_0048551D`, from `Missile_Step` or from
    /// `Cell_TryEnter`. Once per call, as the original plays it once per call.
    bridges_fired: u32,
    burn_deaths: [u32; 2],
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
    /// State 17's own loose. Zero for [`WeaponClass::Catapult`]: the handler
    /// only sounds for class 2 and class 1.
    pub fn closing_loosed(&self, class: WeaponClass) -> u32 {
        match class {
            WeaponClass::Bow => self.closing_loosed[0],
            WeaponClass::Crossbow => self.closing_loosed[1],
            WeaponClass::Catapult => 0,
        }
    }
    pub fn walls_struck(&self) -> u32 {
        self.walls_struck
    }
    pub fn walls_smashed(&self) -> u32 {
        self.walls_smashed
    }
    pub fn walls_missed(&self) -> u32 {
        self.walls_missed
    }
    pub fn oil_poured(&self) -> u32 {
        self.oil_poured
    }
    pub fn towers_docked(&self) -> u32 {
        self.towers_docked
    }
    pub fn bridges_fired(&self) -> u32 {
        self.bridges_fired
    }
    pub fn burn_deaths(&self, side: Side) -> u32 {
        self.burn_deaths[side_index(side)]
    }

    pub fn of_every_occasion() -> Cues {
        Cues {
            melee_casualties: [1; 11],
            melee_deaths: [1; 2],
            missile_hits: [1; 2],
            missile_casualties: [1; 2],
            missile_deaths: 1,
            loosed: [1; 3],
            closing_loosed: [1; 2],
            walls_struck: 1,
            walls_missed: 1,
            walls_smashed: 1,
            oil_poured: 1,
            towers_docked: 1,
            bridges_fired: 1,
            burn_deaths: [1; 2],
        }
    }

    pub fn is_behind(&self, earlier: &Cues) -> bool {
        let pairs = self
            .melee_casualties
            .iter()
            .zip(&earlier.melee_casualties)
            .chain(self.melee_deaths.iter().zip(&earlier.melee_deaths))
            .chain(self.missile_hits.iter().zip(&earlier.missile_hits))
            .chain(self.missile_casualties.iter().zip(&earlier.missile_casualties))
            .chain(self.loosed.iter().zip(&earlier.loosed))
            .chain(self.closing_loosed.iter().zip(&earlier.closing_loosed))
            .chain(self.burn_deaths.iter().zip(&earlier.burn_deaths))
            .chain([
                (&self.missile_deaths, &earlier.missile_deaths),
                (&self.walls_struck, &earlier.walls_struck),
                (&self.walls_missed, &earlier.walls_missed),
                (&self.walls_smashed, &earlier.walls_smashed),
                (&self.oil_poured, &earlier.oil_poured),
                (&self.towers_docked, &earlier.towers_docked),
                (&self.bridges_fired, &earlier.bridges_fired),
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
    pub(crate) fn closing_loose(&mut self, class: WeaponClass) {
        match class {
            WeaponClass::Bow => bump(&mut self.closing_loosed[0]),
            WeaponClass::Crossbow => bump(&mut self.closing_loosed[1]),
            WeaponClass::Catapult => {}
        }
    }
    pub(crate) fn wall_struck(&mut self) {
        bump(&mut self.walls_struck);
    }
    pub(crate) fn wall_smashed(&mut self) {
        bump(&mut self.walls_smashed);
    }
    pub(crate) fn wall_missed(&mut self) {
        bump(&mut self.walls_missed);
    }
    pub(crate) fn oil_pour(&mut self) {
        bump(&mut self.oil_poured);
    }
    pub(crate) fn tower_dock(&mut self) {
        bump(&mut self.towers_docked);
    }
    pub(crate) fn bridge_fire(&mut self) {
        bump(&mut self.bridges_fired);
    }
    pub(crate) fn burn_death(&mut self, side: Side) {
        bump(&mut self.burn_deaths[side_index(side)]);
    }
}

impl l2_net::Encode for Cues {
    fn encode(&self, out: &mut l2_net::Canonical) {
        let mut put = |vs: &[u32]| {
            for v in vs {
                out.u32(*v);
            }
        };
        put(&self.melee_casualties);
        put(&self.melee_deaths);
        put(&self.missile_hits);
        put(&self.missile_casualties);
        put(&[self.missile_deaths]);
        put(&self.loosed);
        put(&[self.walls_struck, self.walls_missed, self.walls_smashed]);
        put(&[self.oil_poured, self.towers_docked, self.bridges_fired]);
        put(&self.burn_deaths);
    }
}

impl l2_net::Decode for Cues {
    fn decode(input: &mut l2_net::Reader<'_>) -> Result<Cues, l2_net::CodecError> {
        let mut c = Cues::default();
        for v in c
            .melee_casualties
            .iter_mut()
            .chain(&mut c.melee_deaths)
            .chain(&mut c.missile_hits)
            .chain(&mut c.missile_casualties)
            .chain(core::iter::once(&mut c.missile_deaths))
            .chain(&mut c.loosed)
            .chain([
                &mut c.walls_struck,
                &mut c.walls_missed,
                &mut c.walls_smashed,
                &mut c.oil_poured,
                &mut c.towers_docked,
                &mut c.bridges_fired,
            ])
            .chain(&mut c.burn_deaths)
        {
            *v = input.u32()?;
        }
        Ok(c)
    }
}
