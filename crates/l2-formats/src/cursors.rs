//! `App_InitWindow` (`0x004B2258`) loads eight `HCURSOR`s with `LoadCursorA`
//! from the executable's own resources — `RT_GROUP_CURSOR` 102, 103, 104, 105
//! (twice), 110, 111 and 113 — and registers the window class with `hCursor`
//! NULL.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    pub id: u16,
    pub width: u16,
    pub height: u16,
    pub hot_x: u16,
    pub hot_y: u16,
    pub rgba: Vec<u8>,
}

impl Picture {
    /// The same picture with every pixel `n` times as wide and as tall, which
    /// is how a 1996 32×32 cursor keeps its size against a canvas the shell
    /// [D] The original never scales a pointer (640x480 fullscreen, one HCURSOR per
    /// kind); ours scales the bitmap and hotspot with the canvas.
    pub fn scaled(&self, n: u32) -> Picture {
        let n = n.max(1);
        let (w, h) = (self.width as u32 * n, self.height as u32 * n);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let src = ((y / n) * self.width as u32 + x / n) as usize * 4;
                let dst = (y * w + x) as usize * 4;
                rgba[dst..dst + 4].copy_from_slice(&self.rgba[src..src + 4]);
            }
        }
        Picture {
            id: self.id,
            width: w as u16,
            height: h as u16,
            hot_x: self.hot_x * n as u16,
            hot_y: self.hot_y * n as u16,
            rgba,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorError {
    NotAnImage(&'static str),
    Truncated,
    Depth(u16),
}

impl core::fmt::Display for CursorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CursorError::NotAnImage(w) => write!(f, "not a PE32 image: {w}"),
            CursorError::Truncated => write!(f, "the resource directory runs past the file"),
            CursorError::Depth(b) => write!(f, "a {b}-bit cursor"),
        }
    }
}

impl std::error::Error for CursorError {}

const RT_CURSOR: u32 = 1;
const RT_GROUP_CURSOR: u32 = 12;

fn u16le(b: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(at..at + 2)?.try_into().ok()?))
}
fn u32le(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

struct Image<'a> {
    bytes: &'a [u8],
    sections: Vec<(u32, u32, u32)>,
}

impl<'a> Image<'a> {
    fn open(bytes: &'a [u8]) -> Result<(Image<'a>, u32), CursorError> {
        let pe = u32le(bytes, 0x3C).ok_or(CursorError::Truncated)? as usize;
        if bytes.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err(CursorError::NotAnImage("no PE signature"));
        }
        let sections_n = u16le(bytes, pe + 6).ok_or(CursorError::Truncated)? as usize;
        let opt_size = u16le(bytes, pe + 20).ok_or(CursorError::Truncated)? as usize;
        let opt = pe + 24;
        if u16le(bytes, opt) != Some(0x010B) {
            return Err(CursorError::NotAnImage("not PE32"));
        }
        let rsrc = u32le(bytes, opt + 96 + 2 * 8).ok_or(CursorError::Truncated)?;
        let mut sections = Vec::with_capacity(sections_n);
        for i in 0..sections_n {
            let s = opt + opt_size + i * 40;
            sections.push((
                u32le(bytes, s + 12).ok_or(CursorError::Truncated)?,
                u32le(bytes, s + 16).ok_or(CursorError::Truncated)?,
                u32le(bytes, s + 20).ok_or(CursorError::Truncated)?,
            ));
        }
        Ok((Image { bytes, sections }, rsrc))
    }

    fn offset(&self, rva: u32) -> Option<usize> {
        self.sections
            .iter()
            .find(|&&(va, size, _)| rva >= va && rva < va + size)
            .map(|&(va, _, raw)| (rva - va + raw) as usize)
    }
}

fn entries(b: &[u8], at: usize) -> Result<Vec<(u32, u32, bool)>, CursorError> {
    let named = u16le(b, at + 12).ok_or(CursorError::Truncated)? as usize;
    let ids = u16le(b, at + 14).ok_or(CursorError::Truncated)? as usize;
    let mut out = Vec::with_capacity(ids);
    for i in named..named + ids {
        let e = at + 16 + i * 8;
        let id = u32le(b, e).ok_or(CursorError::Truncated)?;
        let off = u32le(b, e + 4).ok_or(CursorError::Truncated)?;
        out.push((id, off & 0x7FFF_FFFF, off & 0x8000_0000 != 0));
    }
    Ok(out)
}

fn resource<'a>(
    img: &Image<'a>,
    root: usize,
    kind: u32,
    id: u32,
) -> Result<Option<&'a [u8]>, CursorError> {
    let b = img.bytes;
    let Some(&(_, ty_off, _)) = entries(b, root)?.iter().find(|e| e.0 == kind && e.2) else {
        return Ok(None);
    };
    let Some(&(_, id_off, dir)) = entries(b, root + ty_off as usize)?.iter().find(|e| e.0 == id)
    else {
        return Ok(None);
    };
    let leaf = if dir {
        match entries(b, root + id_off as usize)?.first() {
            Some(&(_, off, _)) => root + off as usize,
            None => return Ok(None),
        }
    } else {
        root + id_off as usize
    };
    let rva = u32le(b, leaf).ok_or(CursorError::Truncated)?;
    let size = u32le(b, leaf + 4).ok_or(CursorError::Truncated)? as usize;
    let at = img.offset(rva).ok_or(CursorError::Truncated)?;
    Ok(Some(b.get(at..at + size).ok_or(CursorError::Truncated)?))
}

pub fn read(exe: &[u8]) -> Result<Vec<Picture>, CursorError> {
    let (img, rsrc_rva) = Image::open(exe)?;
    let root = img.offset(rsrc_rva).ok_or(CursorError::NotAnImage("no resources"))?;
    let mut groups: Vec<u32> = entries(exe, root)?
        .iter()
        .find(|e| e.0 == RT_GROUP_CURSOR && e.2)
        .map(|&(_, off, _)| entries(exe, root + off as usize))
        .transpose()?
        .unwrap_or_default()
        .iter()
        .map(|e| e.0)
        .collect();
    groups.sort_unstable();

    let mut out = Vec::with_capacity(groups.len());
    for id in groups {
        let Some(group) = resource(&img, root, RT_GROUP_CURSOR, id)? else { continue };
        let member = u16le(group, 6 + 12).ok_or(CursorError::Truncated)?;
        let Some(bits) = resource(&img, root, RT_CURSOR, member as u32)? else { continue };
        out.push(decode(id as u16, bits)?);
    }
    Ok(out)
}

fn decode(id: u16, b: &[u8]) -> Result<Picture, CursorError> {
    let hot_x = u16le(b, 0).ok_or(CursorError::Truncated)?;
    let hot_y = u16le(b, 2).ok_or(CursorError::Truncated)?;
    let head = 4;
    let header = u32le(b, head).ok_or(CursorError::Truncated)? as usize;
    let w = u32le(b, head + 4).ok_or(CursorError::Truncated)? as usize;
    let h = u32le(b, head + 8).ok_or(CursorError::Truncated)? as usize / 2;
    let bpp = u16le(b, head + 14).ok_or(CursorError::Truncated)?;
    if !matches!(bpp, 1 | 4 | 8) {
        return Err(CursorError::Depth(bpp));
    }
    let palette = head + header;
    let colours = 1usize << bpp;
    let xor = palette + colours * 4;
    let xor_stride = (w * bpp as usize).div_ceil(32) * 4;
    let and_stride = w.div_ceil(32) * 4;
    let and = xor + xor_stride * h;

    let mut rgba = vec![0u8; w * h * 4];
    for y in 0..h {
        let row = h - 1 - y;
        for x in 0..w {
            let opaque = {
                let byte = *b.get(and + row * and_stride + x / 8).ok_or(CursorError::Truncated)?;
                byte >> (7 - x % 8) & 1 == 0
            };
            let index = {
                let bit = x * bpp as usize;
                let byte = *b.get(xor + row * xor_stride + bit / 8).ok_or(CursorError::Truncated)?;
                (byte >> (8 - bpp as usize - bit % 8) & (colours as u8 - 1)) as usize
            };
            let c = palette + index * 4;
            let px = (y * w + x) * 4;
            // [D] AND 1 with XOR 1 is the inverting pixel, which no display server
            // outside Windows can draw: we make it the ink colour, black.
            if !opaque {
                if index != 0 {
                    rgba[px..px + 4].copy_from_slice(&[0, 0, 0, 255]);
                }
                continue;
            }
            rgba[px] = *b.get(c + 2).ok_or(CursorError::Truncated)?;
            rgba[px + 1] = *b.get(c + 1).ok_or(CursorError::Truncated)?;
            rgba[px + 2] = *b.get(c).ok_or(CursorError::Truncated)?;
            rgba[px + 3] = 255;
        }
    }
    Ok(Picture { id, width: w as u16, height: h as u16, hot_x, hot_y, rgba })
}
