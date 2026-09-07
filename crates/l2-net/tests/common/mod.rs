//! A simulation small enough to read and real enough to break.
//!
//! The lockstep tests need *a* simulation. A trivial one — a counter —
//! would pass every test in this crate while exercising none of the
//! things that actually desync games, so this one deliberately uses
//! every primitive the crate offers and every shape the determinism
//! contract worries about:
//!
//! * fixed-point positions ([`Fixed`], D-2), advanced by multiplication
//!   and division, so a rounding change is visible;
//! * the frozen PRNG ([`Pcg32`], D-3) drawn from inside `step`, so a
//!   stream change is visible;
//! * a `Vec` walked by index (D-4), never a map;
//! * a sort with an id tiebreak (D-7);
//! * commands decoded from their canonical bytes (D-10).
//!
//! It also has a [`ToySim::bias`] knob whose whole purpose is to
//! produce a peer that is *almost* right — one unit's damage off by
//! one, hundreds of ticks in. That is what a real desync looks like,
//! and a detector that only catches a peer running a completely
//! different simulation is not worth having.

#![allow(dead_code)] // each test file uses a different part of this

use l2_net::{Canonical, Command, Decode, Encode, Fixed, Pcg32, Reader, Simulation, Tick};

/// A unit on the field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub id: u32,
    pub owner: u8,
    pub x: Fixed,
    pub y: Fixed,
    pub hp: i32,
    pub target_x: Fixed,
    pub target_y: Fixed,
}

/// An order a player can give.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    /// Send a unit to a point.
    MoveTo { unit: u32, x: Fixed, y: Fixed },
    /// Hurt a unit for a rolled amount.
    Attack { unit: u32 },
}

impl Encode for Order {
    fn encode(&self, out: &mut Canonical) {
        match self {
            Order::MoveTo { unit, x, y } => {
                out.u8(1);
                out.u32(*unit);
                out.fixed(*x);
                out.fixed(*y);
            }
            Order::Attack { unit } => {
                out.u8(2);
                out.u32(*unit);
            }
        }
    }
}

impl Decode for Order {
    fn decode(input: &mut Reader<'_>) -> Result<Self, l2_net::CodecError> {
        let at = input.position();
        match input.u8()? {
            1 => Ok(Order::MoveTo {
                unit: input.u32()?,
                x: input.fixed()?,
                y: input.fixed()?,
            }),
            2 => Ok(Order::Attack { unit: input.u32()? }),
            tag => Err(l2_net::CodecError::BadTag { tag, expected: "order", at }),
        }
    }
}

impl Order {
    pub fn payload(&self) -> Vec<u8> {
        Canonical::bytes_of(self)
    }
}

#[derive(Debug, Clone)]
pub struct ToySim {
    pub rng: Pcg32,
    pub units: Vec<Unit>,
    pub now: Tick,
    /// Added to every damage roll. Zero on an honest peer.
    pub bias: i32,
    /// Set when a command failed to decode. A real simulation would
    /// have to decide a policy; the test only needs to know it did not
    /// happen.
    pub bad_commands: u32,
}

impl ToySim {
    pub fn new(seed: u64, units: usize) -> ToySim {
        let mut rng = Pcg32::from_seed(seed);
        let mut list = Vec::new();
        for id in 0..units as u32 {
            // Positions drawn from the generator, so two peers that
            // seeded differently diverge before tick 0 — which is what
            // the handshake's seed check exists to prevent.
            let x = Fixed::from_int(rng.range(0, 79));
            let y = Fixed::from_int(rng.range(0, 79));
            list.push(Unit {
                id,
                owner: (id % 2) as u8,
                x,
                y,
                hp: 100,
                target_x: x,
                target_y: y,
            });
        }
        ToySim { rng, units: list, now: Tick::ZERO, bias: 0, bad_commands: 0 }
    }

    pub fn unit(&self, id: u32) -> Option<&Unit> {
        self.units.iter().find(|u| u.id == id)
    }

    fn unit_mut(&mut self, id: u32) -> Option<&mut Unit> {
        self.units.iter_mut().find(|u| u.id == id)
    }
}

impl Simulation for ToySim {
    fn step(&mut self, tick: Tick, commands: &[Command]) {
        self.now = tick;

        for command in commands {
            match l2_net::decode_all::<Order>(&command.payload) {
                Ok(Order::MoveTo { unit, x, y }) => {
                    if let Some(unit) = self.unit_mut(unit) {
                        unit.target_x = x;
                        unit.target_y = y;
                    }
                }
                Ok(Order::Attack { unit }) => {
                    // The roll happens whether or not the unit exists,
                    // so that a peer with a different view of which
                    // units are alive still draws the same number of
                    // values. Getting this wrong is the classic
                    // lockstep bug.
                    let roll = self.rng.range(1, 6) + self.bias;
                    if let Some(unit) = self.unit_mut(unit) {
                        unit.hp -= roll;
                    }
                }
                Err(_) => self.bad_commands += 1,
            }
        }

        // Movement: one tenth of the way to the target each tick, in
        // index order (D-4).
        for i in 0..self.units.len() {
            let unit = &mut self.units[i];
            let dx = unit.target_x - unit.x;
            let dy = unit.target_y - unit.y;
            unit.x += dx.mul_ratio(1, 10);
            unit.y += dy.mul_ratio(1, 10);
        }

        // A per-tick drift, so that an idle simulation still advances
        // its checksum and a stuck peer is visible.
        let drift = self.rng.below(3) as i32 - 1;
        if let Some(unit) = self.units.first_mut() {
            unit.hp += drift;
        }

        // Dead units are removed, and the order is fixed by id (D-7).
        self.units.retain(|u| u.hp > 0);
        self.units.sort_by_key(|u| u.id);
    }

    fn encode_state(&self, out: &mut Canonical) {
        out.section("tick");
        out.u32(self.now.0);

        out.section("rng");
        out.encode(&self.rng);

        out.section("units");
        out.seq(&self.units, |c, unit| {
            c.u32(unit.id);
            c.u8(unit.owner);
            c.fixed(unit.x);
            c.fixed(unit.y);
            c.i32(unit.hp);
            c.fixed(unit.target_x);
            c.fixed(unit.target_y);
        });
    }
}
