//! `g_battlefield` (`0x005440E0`) is save block 1 of `Save_Write`'s table —
//! 51,200 bytes, `80 × 80 × 8`. **[V]**

use l2_net::{Canonical, CodecError, Decode, Encode, Reader};

use super::{Battlefield, Cell, DIM};

impl Encode for Cell {
    fn encode(&self, out: &mut Canonical) {
        out.u8(self.terrain);
        out.u8(self.flags);
        out.u8(self.flags2);
        out.u8(self.gfx);
        out.u8(self.elevation);
        out.u8(self.surface);
    }
}

impl Decode for Cell {
    fn decode(input: &mut Reader<'_>) -> Result<Cell, CodecError> {
        Ok(Cell {
            terrain: input.u8()?,
            flags: input.u8()?,
            flags2: input.u8()?,
            gfx: input.u8()?,
            elevation: input.u8()?,
            surface: input.u8()?,
        })
    }
}

fn put_pairs(out: &mut Canonical, ps: &[(u8, u8)]) {
    for (x, y) in ps {
        out.u8(*x);
        out.u8(*y);
    }
}

fn get_pairs<const N: usize>(input: &mut Reader<'_>) -> Result<[(u8, u8); N], CodecError> {
    let mut out = [(0u8, 0u8); N];
    for p in &mut out {
        *p = (input.u8()?, input.u8()?);
    }
    Ok(out)
}

impl Encode for Battlefield {
    fn encode(&self, out: &mut Canonical) {
        out.section("field");
        out.seq(&self.cells, |c, cell| cell.encode(c));
        put_pairs(out, &self.deploy_side0);
        put_pairs(out, &self.deploy_side4);
        put_pairs(out, &[self.home_side0, self.home_side4]);
        for side in &self.enemy_end {
            put_pairs(out, side);
        }
        for group in self.rally.iter().flatten() {
            put_pairs(out, group);
        }
        out.end_section();
    }
}

impl Decode for Battlefield {
    fn decode(input: &mut Reader<'_>) -> Result<Battlefield, CodecError> {
        let at = input.position();
        let cells = input.seq(Cell::decode)?;
        if cells.len() != DIM * DIM {
            return Err(CodecError::LengthOverrun {
                declared: cells.len(),
                remaining: DIM * DIM,
                at,
            });
        }
        let deploy_side0 = get_pairs::<12>(input)?;
        let deploy_side4 = get_pairs::<12>(input)?;
        let homes = get_pairs::<2>(input)?;
        let enemy_end = [get_pairs::<4>(input)?, get_pairs::<4>(input)?];
        let mut rally = [[[(0, 0); 3]; 2]; 2];
        for group in rally.iter_mut().flatten() {
            *group = get_pairs::<3>(input)?;
        }
        Ok(Battlefield {
            cells,
            deploy_side0,
            deploy_side4,
            home_side0: homes[0],
            home_side4: homes[1],
            enemy_end,
            rally,
        })
    }
}
