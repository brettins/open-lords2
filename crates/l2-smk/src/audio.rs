#![allow(unused_imports)]
use super::*;
use super::decode::*;
use super::bitstream::*;
use std::fmt;


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioChunk {
    pub pcm: Vec<u8>,
    pub unpacked: usize,
    pub bits: (usize, usize),
}

pub(super) fn decode_audio(raw: &[u8], desc: Track) -> Result<AudioChunk> {
    if !desc.packed {
        return Ok(AudioChunk { pcm: raw.to_vec(), unpacked: raw.len(), bits: (0, 0) });
    }
    let unpacked = u32_at(raw, 0).ok_or(Error::Truncated("an audio chunk's length"))? as usize;
    let mut bits = Bits::new(&raw[4..], "an audio bitstream");
    let mut pcm = Vec::with_capacity(unpacked);
    if bits.bit()? == 0 {
        return Ok(AudioChunk { pcm, unpacked, bits: (bits.pos, bits.total()) });
    }
    let stereo = bits.bit()? == 1;
    let bits16 = bits.bit()? == 1;
    if stereo != desc.stereo || bits16 != desc.bits16 {
        return Err(Error::BadTree("an audio chunk disagrees with its track's descriptor"));
    }
    if bits16 {
        return Err(Error::Unsupported("16-bit audio"));
    }
    let channels = if stereo { 2 } else { 1 };
    let trees: Vec<Tree8> =
        (0..channels).map(|_| Tree8::read(&mut bits)).collect::<Result<Vec<_>>>()?;
    let mut last = [0u8; 2];
    for c in (0..channels).rev() {
        last[c] = bits.bits(8)? as u8;
    }
    for &v in last.iter().take(channels) {
        if pcm.len() < unpacked {
            pcm.push(v);
        }
    }
    while pcm.len() < unpacked {
        let c = pcm.len() % channels;
        let delta = trees[c].decode(&mut bits)?;
        last[c] = last[c].wrapping_add(delta);
        pcm.push(last[c]);
    }
    Ok(AudioChunk { pcm, unpacked, bits: (bits.pos, bits.total()) })
}

