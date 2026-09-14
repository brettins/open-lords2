//! **The shipped films, decoded — and checked against something that is not
//! this decoder.**
//!
//! ```text
//! LORDS2_DIR="F:\games\Lords of the Realm II" cargo test -p l2-smk --test corpus
//! ```
//!
//! Three kinds of evidence, and the file is laid out strongest last:
//!
//! 1. **the container closes** — every byte of every film is accounted for;
//! 2. **every bitstream is consumed to its padding** — a Huffman tree read one
//!    bit wrong desynchronises everything after it and either overruns its
//!    chunk or stops well short of it, so *"fewer than 32 bits left over, in
//!    every video frame and every audio chunk of 45 films"* is a property a
//!    wrong decoder does not have by accident;
//! 3. **an independent decoder produced the same pixels, palettes and samples.**
//!    [`ORACLE`] was printed by a scratch program running the LGPL `smk` crate
//!    as a black box — its public API only, its source never opened, nothing of
//!    it in this tree — and every hash below is a literal out of that run, not
//!    a number this decoder was allowed to produce. `docs/formats/smk.md`
//!    records the run.
//!
//! **Ablated, and what went red.** Swapping the two `Full` codes of a row in
//! `decode_video` leaves every bitstream consumed — the
//! padding test stays green, correctly, because a column swap reads the same
//! bits — and [`every_film_matches_an_independent_decoder`] fails on the first
//! film, `AXMEN.SMK`, at frame 10's pixels. Skipping `Tree16::decode`'s cache
//! update desynchronises the very first film so badly that it overruns a
//! chunk, so every test in this file fails together on the shared decode.

mod corpus_tests;
pub use corpus_tests::*;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use l2_smk::Smk;

/// What one film decoded to.
#[derive(Debug, Clone)]
struct Stats {
    width: u32,
    height: u32,
    frames: usize,
    y_scale: u32,
    period: u32,
    slack: i64,
    signature: [u8; 4],
    tracks: usize,
    rate: u32,
    stereo: bool,
    /// The most bits left unread in any one frame's video bitstream, and in
    /// any one audio chunk.
    video_pad: usize,
    audio_pad: usize,
    /// Every audio chunk produced exactly what its own header promised.
    audio_lengths_agree: bool,
    pcm: usize,
    video: u64,
    palette: u64,
    audio: u64,
    frame10: u64,
}

fn fnv(h: &mut u64, bytes: &[u8]) {
    for &b in bytes {
        *h ^= b as u64;
        *h = h.wrapping_mul(0x100000001b3);
    }
}

const FNV_START: u64 = 0xcbf29ce484222325;

pub(super) fn decode(path: &std::path::Path) -> Stats {
    let smk = Smk::parse(std::fs::read(path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let h = *smk.header();
    let mut d = smk.decoder();
    let (mut video, mut palette, mut audio, mut frame10) = (FNV_START, FNV_START, FNV_START, 0);
    let mut video_pad = 0;
    while d.next_frame(&smk).unwrap_or_else(|e| panic!("{}: {e}", path.display())) {
        let (read, total) = d.video_bits();
        video_pad = video_pad.max(total - read);
        if d.frame() == Some(10) {
            let mut one = FNV_START;
            fnv(&mut one, d.pixels());
            frame10 = one;
        }
        fnv(&mut video, d.pixels());
        for e in d.palette() {
            fnv(&mut palette, e);
        }
    }
    let (mut audio_pad, mut pcm, mut agree) = (0, 0, true);
    for f in 0..smk.frames() {
        if let Some(c) = smk.audio_chunk(f, 0).unwrap_or_else(|e| panic!("{}: {e}", path.display())) {
            audio_pad = audio_pad.max(c.bits.1 - c.bits.0);
            agree &= c.pcm.len() == c.unpacked;
            pcm += c.pcm.len();
            fnv(&mut audio, &c.pcm);
        }
    }
    let track = h.track(0);
    Stats {
        width: h.width,
        height: h.height,
        frames: smk.frames(),
        y_scale: h.y_scale(),
        period: h.period_10us(),
        slack: smk.slack(),
        signature: h.signature,
        tracks: (0..7).filter(|&t| h.track(t).is_some()).count(),
        rate: track.map_or(0, |t| t.rate),
        stereo: track.is_some_and(|t| t.stereo),
        video_pad,
        audio_pad,
        audio_lengths_agree: agree,
        pcm,
        video,
        palette,
        audio,
        frame10,
    }
}

/// Every film in the install, decoded once for all the tests in this file —
/// the corpus is 7,652 frames and a debug build takes about twenty seconds.
fn corpus() -> Option<&'static BTreeMap<String, Stats>> {
    static CORPUS: OnceLock<Option<BTreeMap<String, Stats>>> = OnceLock::new();
    CORPUS
        .get_or_init(|| {
            let dir: PathBuf = l2_testkit::install_dir()?;
            let mut out = BTreeMap::new();
            for e in std::fs::read_dir(&dir).ok()?.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("smk")) {
                    let name = p.file_name().unwrap().to_string_lossy().to_string();
                    out.insert(name, decode(&p));
                }
            }
            (!out.is_empty()).then_some(out)
        })
        .as_ref()
}

macro_rules! films {
    () => {
        match corpus() {
            Some(c) => c,
            None => l2_testkit::skip!("no install with .smk films (the DOS release ships none)"),
        }
    };
}

