//! `PUMKIN.WAV` and `PUMKIN2.WAV` are 160 MB each and **the original never
//! plays them**: `FUN_004AEF7E` opens one, calls `__filelength`, compares it
//! against 151,000,000 and closes it. They are a full-install check, and their
//! content is the CD soundtrack at 44.1 kHz — the audio the original played
//! through the drive, left on disk. [`MAX_BYTES`] refuses them by size rather
//! than by name, because a size limit also catches whatever else a modder
//! drops in the folder.

pub const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sound {
    pub channels: u16,
    pub rate: u32,
    pub samples: Vec<i16>,
}

impl Sound {
    pub fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.samples.len() / self.channels as usize
        }
    }

    pub fn frame(&self, i: usize) -> (i16, i16) {
        let c = self.channels as usize;
        match self.samples.get(i * c) {
            None => (0, 0),
            Some(&l) if c == 1 => (l, l),
            Some(&l) => (l, self.samples.get(i * c + 1).copied().unwrap_or(l)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavError {
    NotRiff,
    Truncated,
    NoFormat,
    NoData,
    Unsupported { format: u16, bits: u16 },
    TooLarge(usize),
}

impl core::fmt::Display for WavError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            WavError::NotRiff => write!(f, "not a RIFF/WAVE file"),
            WavError::Truncated => write!(f, "truncated"),
            WavError::NoFormat => write!(f, "no fmt chunk"),
            WavError::NoData => write!(f, "no data chunk"),
            WavError::Unsupported { format, bits } => {
                write!(f, "unsupported: format {format}, {bits}-bit")
            }
            WavError::TooLarge(n) => write!(f, "{n} bytes, over the {MAX_BYTES}-byte limit"),
        }
    }
}

fn u16_at(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]))
}

fn u32_at(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(i)?,
        *b.get(i + 1)?,
        *b.get(i + 2)?,
        *b.get(i + 3)?,
    ]))
}

pub fn decode(bytes: &[u8]) -> Result<Sound, WavError> {
    if bytes.len() > MAX_BYTES {
        return Err(WavError::TooLarge(bytes.len()));
    }
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(WavError::NotRiff);
    }

    let mut format: Option<(u16, u16, u32, u16)> = None; // tag, channels, rate, bits
    let mut data: Option<&[u8]> = None;

    let mut at = 12usize;
    while at + 8 <= bytes.len() {
        let tag = &bytes[at..at + 4];
        let len = u32_at(bytes, at + 4).ok_or(WavError::Truncated)? as usize;
        let body = at + 8;
        let end = body.checked_add(len).ok_or(WavError::Truncated)?;
        if end > bytes.len() {
            if tag == b"data" {
                data = Some(&bytes[body..]);
            }
            break;
        }
        match tag {
            b"fmt " if len >= 16 => {
                format = Some((
                    u16_at(bytes, body).ok_or(WavError::Truncated)?,
                    u16_at(bytes, body + 2).ok_or(WavError::Truncated)?,
                    u32_at(bytes, body + 4).ok_or(WavError::Truncated)?,
                    u16_at(bytes, body + 14).ok_or(WavError::Truncated)?,
                ));
            }
            b"data" => data = Some(&bytes[body..end]),
            _ => {}
        }
        at = end + (end & 1);
    }

    let (tag, channels, rate, bits) = format.ok_or(WavError::NoFormat)?;
    let data = data.ok_or(WavError::NoData)?;
    if tag != 1 {
        return Err(WavError::Unsupported { format: tag, bits });
    }
    let channels = channels.max(1);

    let samples = match bits {
        8 => data.iter().map(|&b| ((b as i16) - 128) << 8).collect(),
        16 => data
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect(),
        _ => return Err(WavError::Unsupported { format: tag, bits }),
    };

    Ok(Sound { channels, rate: rate.max(1), samples })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn riff(channels: u16, rate: u32, bits: u16, data: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36u32 + data.len() as u32).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * channels as u32 * (bits / 8) as u32).to_le_bytes());
        v.extend_from_slice(&(channels * bits / 8).to_le_bytes());
        v.extend_from_slice(&bits.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(data.len() as u32).to_le_bytes());
        v.extend_from_slice(data);
        v
    }

    #[test]
    fn eight_bit_is_unsigned_and_centred_on_128() {
        let s = decode(&riff(1, 11025, 8, &[0x80, 0x00, 0xff])).unwrap();
        assert_eq!(s.rate, 11025);
        assert_eq!(s.channels, 1);
        assert_eq!(s.samples, vec![0, -32768, 32512]);
    }

    #[test]
    fn a_mono_frame_is_delivered_to_both_ears() {
        let s = decode(&riff(1, 11025, 8, &[0xff])).unwrap();
        assert_eq!(s.frame(0), (32512, 32512));
        assert_eq!(s.frames(), 1);
    }

    #[test]
    fn a_stereo_frame_keeps_its_two_sides() {
        let s = decode(&riff(2, 11025, 8, &[0x00, 0xff])).unwrap();
        assert_eq!(s.frames(), 1);
        assert_eq!(s.frame(0), (-32768, 32512));
    }

    #[test]
    fn past_the_end_is_silence_rather_than_a_panic() {
        let s = decode(&riff(1, 11025, 8, &[0xff])).unwrap();
        assert_eq!(s.frame(9), (0, 0));
    }

    #[test]
    fn a_chunk_between_fmt_and_data_is_stepped_over() {
        let mut v = riff(1, 11025, 8, &[0x80]);
        let at = v.len() - 9;
        let mut extra = b"fact".to_vec();
        extra.extend_from_slice(&3u32.to_le_bytes());
        extra.extend_from_slice(&[1, 2, 3, 0]);
        v.splice(at..at, extra);
        v.splice(4..8, ((v.len() - 8) as u32).to_le_bytes());
        assert_eq!(decode(&v).unwrap().samples, vec![0]);
    }

    #[test]
    fn a_data_length_that_overruns_the_file_takes_what_is_there() {
        let mut v = riff(1, 11025, 8, &[0x80, 0x80]);
        let n = v.len();
        v.splice(n - 6..n - 2, 999u32.to_le_bytes());
        assert_eq!(decode(&v).unwrap().samples, vec![0, 0]);
    }

    #[test]
    fn compressed_and_oversized_are_named_rather_than_guessed_at() {
        let mut v = riff(1, 11025, 8, &[0x80]);
        v[20] = 17; // IMA ADPCM
        assert_eq!(decode(&v), Err(WavError::Unsupported { format: 17, bits: 8 }));
        assert_eq!(decode(b"not a wav at all"), Err(WavError::NotRiff));
        assert_eq!(
            decode(&vec![0u8; MAX_BYTES + 1]),
            Err(WavError::TooLarge(MAX_BYTES + 1))
        );
    }
}
