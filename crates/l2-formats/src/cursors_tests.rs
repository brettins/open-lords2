use super::*;

/// A cursor resource: hotspot, BITMAPINFOHEADER, palette (BGRA), XOR then AND
/// bits, both bottom-up. `index` and `and` are top-down, row-major.
fn encode(bpp: u16, w: usize, h: usize, pal: &[[u8; 4]], used: u32, index: &[u8], and: &[bool]) -> Vec<u8> {
    let mut b = vec![3, 0, 4, 0];
    for v in [40u32, w as u32, (h * 2) as u32] {
        b.extend(v.to_le_bytes());
    }
    b.extend(1u16.to_le_bytes());
    b.extend(bpp.to_le_bytes());
    for v in [0u32, 0, 0, 0, used, 0] {
        b.extend(v.to_le_bytes());
    }
    for c in pal {
        b.extend(c);
    }
    let xor_stride = (w * bpp as usize).div_ceil(32) * 4;
    let and_stride = w.div_ceil(32) * 4;
    for row in (0..h).rev() {
        let mut line = vec![0u8; xor_stride];
        for x in 0..w {
            let bit = x * bpp as usize;
            line[bit / 8] |= index[row * w + x] << (8 - bpp as usize - bit % 8);
        }
        b.extend(line);
    }
    for row in (0..h).rev() {
        let mut line = vec![0u8; and_stride];
        for x in 0..w {
            line[x / 8] |= u8::from(and[row * w + x]) << (7 - x % 8);
        }
        b.extend(line);
    }
    b
}

/// The original's seven cursors, as (bpp, palette, index, and) per pixel.
fn exe_cursors() -> Option<Vec<(u16, u16, Vec<[u8; 4]>, Vec<u8>, Vec<bool>)>> {
    let exe = std::fs::read(l2_testkit::install_dir()?.join("Lords2.exe")).ok()?;
    let (img, rva) = Image::open(&exe).unwrap();
    let root = img.offset(rva).unwrap();
    let group = entries(&exe, root).unwrap().into_iter().find(|e| e.0 == RT_GROUP_CURSOR).unwrap();
    let mut out = Vec::new();
    for (id, _, _) in entries(&exe, root + group.1 as usize).unwrap() {
        let g = resource(&img, root, RT_GROUP_CURSOR, id).unwrap().unwrap();
        let b = resource(&img, root, RT_CURSOR, u16le(g, 18).unwrap() as u32).unwrap().unwrap();
        let bpp = u16le(b, 18).unwrap();
        let pal = (0..2).map(|i| b[44 + i * 4..48 + i * 4].try_into().unwrap()).collect();
        let (xor, and) = (52, 52 + 4 * 32);
        let bit = |base: usize, x: usize, y: usize| b[base + (31 - y) * 4 + x / 8] >> (7 - x % 8) & 1;
        let index = (0..32 * 32).map(|p| bit(xor, p % 32, p / 32)).collect();
        let mask = (0..32 * 32).map(|p| bit(and, p % 32, p / 32) == 1).collect();
        out.push((id as u16, bpp, pal, index, mask));
    }
    Some(out)
}

/// Every cursor `App_InitWindow` (`0x004B2258`) loads is 1bpp; re-encoded at 4
/// and 8 bits with the same palette and mask, each decodes to the same picture.
/// Ablation: the old `colours as u8 - 1` index mask overflows at 8 bits.
#[test]
fn the_exes_cursors_decode_alike_at_four_and_eight_bits() {
    let Some(cursors) = exe_cursors() else {
        l2_testkit::skip!("no Lords2.exe to take cursors from");
    };
    assert_eq!(cursors.len(), 7);
    let mut inverting = 0;
    for (id, bpp, pal, index, and) in &cursors {
        assert_eq!(*bpp, 1, "cursor {id}");
        let one = decode(*id, &encode(1, 32, 32, pal, 0, index, and)).unwrap();
        for depth in [4u16, 8] {
            let mut wide = pal.clone();
            wide.resize(1 << depth, [0, 0, 0, 0]);
            let got = decode(*id, &encode(depth, 32, 32, &wide, 0, index, and)).unwrap();
            assert_eq!(got.rgba, one.rgba, "cursor {id} at {depth} bits");
        }
        inverting += (0..1024).filter(|&p| and[p] && pal[index[p] as usize][..3] != [0, 0, 0]).count();
    }
    eprintln!("{inverting} inverting pixels across the seven cursors");
}

/// Index 255 at 8 bits, biClrUsed shortening the palette, and the AND=1 pixels:
/// black is transparent, any other colour is drawn as itself ([D] in `decode`).
#[test]
fn eight_bit_indices_short_palettes_and_the_inverting_pixel() {
    let mut pal = vec![[0, 0, 0, 0]; 256];
    pal[255] = [30, 20, 10, 0];
    pal[1] = [255, 255, 255, 0];
    let p = decode(1, &encode(8, 4, 1, &pal, 0, &[255, 1, 0, 1], &[false, false, true, true])).unwrap();
    assert_eq!(p.rgba, [10, 20, 30, 255, 255, 255, 255, 255, 0, 0, 0, 0, 255, 255, 255, 255]);

    let short = [[0, 0, 0, 0], [0, 0, 200, 0], [0, 100, 0, 0]];
    let p = decode(1, &encode(4, 2, 1, &short, 3, &[2, 1], &[false, false])).unwrap();
    assert_eq!(p.rgba, [0, 100, 0, 255, 200, 0, 0, 255], "the XOR bits follow the 3-colour palette");
}
