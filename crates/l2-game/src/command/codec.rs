//! D-10: hand-written, fixed-width, little-endian. `usize` goes as `u32`.

use super::GameCommand;
use l2_kingdom::divide::{SplitBasket, SplitInto};
use l2_kingdom::field::FieldType;
use l2_kingdom::industry::MapToggle;
use l2_kingdom::levy::{BasketSlot, LevyBasket};
use l2_kingdom::siege::{Engine, ENGINES};
use l2_kingdom::tables::Commodity;
use l2_kingdom::trade::Good;
use l2_kingdom::unit::{Mercenaries, ALL_TROOP_TYPES};
use l2_net::{Canonical, CodecError, Decode, Encode, Quirk, Reader};

pub trait Wire: Sized {
    fn put(&self, out: &mut Canonical);
    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError>;
}

impl Encode for GameCommand {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.tag());
        self.encode_body(out);
    }
}

impl Decode for GameCommand {
    fn decode(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let at = input.position();
        let tag = input.u8()?;
        GameCommand::decode_tagged(tag, at, input)
    }
}

macro_rules! prim {
    ($($t:ty: $f:ident),*) => {$(
        impl Wire for $t {
            fn put(&self, out: &mut Canonical) { out.$f(*self) }
            fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> { input.$f() }
        }
    )*};
}
prim!(u8: u8, i16: i16, i32: i32, bool: bool);

impl Wire for usize {
    fn put(&self, out: &mut Canonical) {
        out.u32(u32::try_from(*self).expect("command index fits u32"));
    }
    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(input.u32()? as usize)
    }
}

impl<A: Wire, B: Wire> Wire for (A, B) {
    fn put(&self, out: &mut Canonical) {
        self.0.put(out);
        self.1.put(out);
    }
    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok((A::get(input)?, B::get(input)?))
    }
}

impl<T: Wire> Wire for Option<T> {
    fn put(&self, out: &mut Canonical) {
        out.option(self.as_ref(), |out, v| v.put(out));
    }
    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        input.option(T::get)
    }
}

impl<T: Wire + Copy + Default, const N: usize> Wire for [T; N] {
    fn put(&self, out: &mut Canonical) {
        self.iter().for_each(|v| v.put(out));
    }
    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let mut a = [T::default(); N];
        for v in a.iter_mut() {
            *v = T::get(input)?;
        }
        Ok(a)
    }
}

/// C244: a letter's text, length-prefixed UTF-8.
impl Wire for String {
    fn put(&self, out: &mut Canonical) {
        out.str(self);
    }
    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(input.str()?.to_string())
    }
}

/// An enum as its index in a fixed table.
fn indexed<T: Copy + PartialEq>(
    all: &[T],
    what: &'static str,
    input: &mut Reader<'_>,
) -> Result<T, CodecError> {
    let at = input.position();
    let tag = input.u8()?;
    all.get(tag as usize).copied().ok_or(CodecError::BadTag { tag, expected: what, at })
}

fn index_of<T: PartialEq>(all: &[T], v: &T) -> u8 {
    all.iter().position(|x| x == v).expect("value is in its table") as u8
}

macro_rules! table {
    ($($t:ty => $all:expr, $what:literal);* $(;)?) => {$(
        impl Wire for $t {
            fn put(&self, out: &mut Canonical) { out.u8(index_of(&$all, self)) }
            fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
                indexed(&$all, $what, input)
            }
        }
    )*};
}

const FIELD_TYPES: [FieldType; 5] = [
    FieldType::Waste,
    FieldType::Fallow,
    FieldType::Grain,
    FieldType::Pasture,
    FieldType::Reclaiming,
];
const ANSWERS: [super::Answer; 2] = [super::Answer::TakeTheField, super::Answer::Decline];
const MAP_TOGGLES: [MapToggle; 5] = [
    MapToggle::Industry(Commodity::Wood),
    MapToggle::Industry(Commodity::Iron),
    MapToggle::Industry(Commodity::Weapons),
    MapToggle::Industry(Commodity::Stone),
    MapToggle::Castle,
];

table! {
    Engine => ENGINES, "siege engine";
    FieldType => FIELD_TYPES, "field type";
    MapToggle => MAP_TOGGLES, "map toggle";
    Good => l2_kingdom::trade::ALL_GOODS, "good";
    l2_kingdom::unit::TroopType => ALL_TROOP_TYPES, "troop type";
    Quirk => *Quirk::ALL, "quirk";
    super::GameOption => super::GAME_OPTIONS, "game option";
    super::Answer => ANSWERS, "battle answer";
}

impl Wire for BasketSlot {
    fn put(&self, out: &mut Canonical) {
        (self.chosen, (self.available, self.remaining)).put(out);
    }
    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let (chosen, (available, remaining)) = Wire::get(input)?;
        Ok(BasketSlot { chosen, available, remaining })
    }
}

impl Wire for LevyBasket {
    fn put(&self, out: &mut Canonical) {
        self.slots.put(out);
    }
    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        Ok(LevyBasket { slots: Wire::get(input)? })
    }
}

impl Wire for Mercenaries {
    fn put(&self, out: &mut Canonical) {
        (self.band, (self.troop, self.men)).put(out);
    }
    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let (band, (troop, men)) = Wire::get(input)?;
        Ok(Mercenaries { band, troop, men })
    }
}

impl Wire for SplitBasket {
    fn put(&self, out: &mut Canonical) {
        (self.parent, self.daughter).put(out);
        (self.mercenaries, self.mercenaries_leave).put(out);
    }
    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let (parent, daughter) = Wire::get(input)?;
        let (mercenaries, mercenaries_leave) = Wire::get(input)?;
        Ok(SplitBasket { parent, daughter, mercenaries, mercenaries_leave })
    }
}

impl Wire for SplitInto {
    fn put(&self, out: &mut Canonical) {
        match *self {
            SplitInto::Field => out.u8(0),
            SplitInto::Castle { county, tile } => {
                out.u8(1);
                (county, tile).put(out);
            }
        }
    }
    fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
        let at = input.position();
        match input.u8()? {
            0 => Ok(SplitInto::Field),
            1 => {
                let (county, tile) = Wire::get(input)?;
                Ok(SplitInto::Castle { county, tile })
            }
            tag => Err(CodecError::BadTag { tag, expected: "split target", at }),
        }
    }
}

/// A struct as its fields in declaration order.
macro_rules! wire_struct {
    ($($name:ident { $($f:ident),* })*) => {$(
        impl Wire for $name {
            #[allow(unused_variables)]
            fn put(&self, out: &mut Canonical) { $(self.$f.put(out);)* }
            #[allow(unused_variables)]
            fn get(input: &mut Reader<'_>) -> Result<Self, CodecError> {
                Ok($name { $($f: Wire::get(input)?),* })
            }
        }
    )*};
}
pub(super) use wire_struct;
