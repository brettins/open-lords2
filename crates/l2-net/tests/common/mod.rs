
#![allow(dead_code)] // each test file uses a different part of this

use l2_net::{Canonical, Command, Decode, Encode, Fixed, Pcg32, Reader, Simulation, Tick};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Order {
    MoveTo { unit: u32, x: Fixed, y: Fixed },
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
    pub bias: i32,
    pub bad_commands: u32,
}

impl ToySim {
    pub fn new(seed: u64, units: usize) -> ToySim {
        let mut rng = Pcg32::from_seed(seed);
        let mut list = Vec::new();
        for id in 0..units as u32 {
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
                    let roll = self.rng.range(1, 6) + self.bias;
                    if let Some(unit) = self.unit_mut(unit) {
                        unit.hp -= roll;
                    }
                }
                Err(_) => self.bad_commands += 1,
            }
        }

        for i in 0..self.units.len() {
            let unit = &mut self.units[i];
            let dx = unit.target_x - unit.x;
            let dy = unit.target_y - unit.y;
            unit.x += dx.mul_ratio(1, 10);
            unit.y += dy.mul_ratio(1, 10);
        }

        let drift = self.rng.below(3) as i32 - 1;
        if let Some(unit) = self.units.first_mut() {
            unit.hp += drift;
        }

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
