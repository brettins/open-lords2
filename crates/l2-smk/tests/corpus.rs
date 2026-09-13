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

fn decode(path: &std::path::Path) -> Stats {
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

#[test]
fn every_shipped_film_closes_its_container() {
    let _ = l2_testkit::install!();
    let c = films!();
    assert_eq!(c.len(), 45, "45 files, of which AXMEN.SMK and Axemen.smk are one film twice");
    for (name, s) in c {
        assert_eq!(s.slack, 0, "{name}: {} bytes unaccounted for", s.slack);
        assert_eq!(&s.signature, b"SMK2", "{name}");
        assert_eq!((s.tracks, s.rate), (1, 11025), "{name}: one 11,025 Hz track");
    }
    assert_eq!(c.values().map(|s| s.frames).sum::<usize>(), 7652);
    // Eight stereo tracks and 37 mono, which is docs/formats/smk.md's census.
    assert_eq!(c.values().filter(|s| s.stereo).count(), 8);
}

#[test]
fn every_bitstream_is_read_to_its_padding_and_no_further() {
    let _ = l2_testkit::install!();
    for (name, s) in films!() {
        assert!(s.video_pad < 32, "{name}: a video frame left {} bits unread", s.video_pad);
        assert!(s.audio_pad < 32, "{name}: an audio chunk left {} bits unread", s.audio_pad);
        assert!(s.audio_lengths_agree, "{name}: a chunk decoded to a length its header did not give");
    }
}

/// **A film's sound track is **
/// what makes the two candidate clocks one clock.
///
/// `Smk_PlayLoop` (`0x0042DBC7`) advances a film only when `SmackWait` answers
/// 0, and `_SmackWait@4` is 320 bytes at RVA `0x3170` of `Smackw32.dll` whose
/// only import call is `WINMM.dll!timeGetTime` at `+0xB0` — `[V]`, by scanning
/// the DLL's `FF 15` sites against its import table. So a film is paced against
/// real milliseconds, and its track is played out by the device in real
/// milliseconds too. Whether `smackw32` slews that deadline to the sound
/// buffer — the DirectSound path installs a `timeSetEvent` callback,
/// `_TimerFunc@20`, which reads `timeGetTime` as well — cannot be told apart
/// here and **does not matter**, because:
///
/// every one of the 45 films carries `frames × period` of audio to within a
/// millisecond, over films as long as 131 seconds. Ours therefore paces the
/// header's rate against a real clock ([`l2_game`'s `clock::Ticker`]) and gets
/// the audio's answer. `docs/decisions.md` C193.
///
/// Ablation: tighten the tolerance to 0.1 ms and the first film goes red —
/// *"AXMEN.SMK: 11665 ms of sound over 11666 ms of picture"*.
#[test]
fn every_track_is_as_long_as_its_picture() {
    let _ = l2_testkit::install!();
    let mut worst = (0i64, String::new());
    for (name, s) in films!() {
        let channels = if s.stereo { 2 } else { 1 };
        let samples = (s.pcm / channels) as i64;
        // Nanoseconds, both sides, integer throughout.
        let sound = samples * 1_000_000_000 / s.rate as i64;
        let picture = s.frames as i64 * s.period as i64 * 10_000;
        let gap = sound - picture;
        if gap.abs() > worst.0 {
            worst = (gap.abs(), name.clone());
        }
        assert!(
            gap.abs() <= 1_000_000,
            "{name}: {} ms of sound over {} ms of picture",
            sound / 1_000_000,
            picture / 1_000_000
        );
    }
    assert!(worst.0 > 0, "a corpus with no gap at all would mean nothing was measured");
}

/// The film the game opens with, field by field.
#[test]
fn the_intro_is_560_by_144_doubled_at_twelve_frames_a_second() {
    let _ = l2_testkit::install!();
    let intro = &films!()["Intro.smk"];
    assert_eq!((intro.width, intro.height, intro.y_scale), (560, 144, 2));
    assert_eq!(intro.frames, 1578);
    assert_eq!(intro.period, 8333, "-8333 tens of microseconds, 12.0005 frames a second");
    assert_eq!(intro.pcm, 1_449_728, "131 seconds of 11,025 Hz mono");
    assert_eq!(intro.frame10, 0x73685a67f9360b25, "frame 10, hashed by the other decoder");
}

/// `(file, frames, video, palette, audio, frame 10)` — FNV-1a over every
/// frame's stored pixels, every frame's 768 palette bytes, every track-0
/// sample, and frame 10's pixels alone. **Printed by the other decoder.**
///
/// `Pill_brn.smk` is the one row it could not finish: it stops at frame 104 on
/// a palette copy whose source overlaps its destination, which
/// `docs/formats/smk.md` diagnosed as a guard of that implementation's rather
/// than a fault in the file. So that row pins only what the other decoder
/// reached — frame 10 and the audio, which it decodes separately.
const ORACLE: &[(&str, usize, u64, u64, u64, u64)] = &[
    ("AXMEN.SMK", 140, 0x3a2ab973bae3d751, 0xbccdaa1c710d55f5, 0x2e522dc1788ec51d, 0x853837d41d47687b),
    ("Axemen.smk", 140, 0x3a2ab973bae3d751, 0xbccdaa1c710d55f5, 0x2e522dc1788ec51d, 0x853837d41d47687b),
    ("BAT_LOS5.SMK", 70, 0x5aa719f463e089fa, 0x48c6dce01573e125, 0x2eb44bd15dc2ae82, 0x9cabd40bd65b539e),
    ("BAT_LOS6.SMK", 91, 0x6cf2717ab602f75a, 0x8750ad81e2f73704, 0xd77e3ef8dd150e7e, 0x4ac0c33f1fe532d5),
    ("BAT_WIN5.SMK", 51, 0x8c4519782760e51b, 0xb9a8bb0dcb906dec, 0xb2c623f7301493f7, 0x08ecf717f06025dd),
    ("BAT_WIN6.SMK", 61, 0x84f36c1667025d68, 0x0c36324a0f8d421d, 0xb963fbdc48b86a1f, 0x1051340786c144a6),
    ("Bat_los1.smk", 48, 0x984070cee19dd4ed, 0xf395fc2147ef5285, 0xde51110c817bf857, 0xb16ab83d10f1a357),
    ("Bat_los2.smk", 48, 0x37d8c918b1300f67, 0x9d6a1ca5492135e5, 0x80033483f1c311b9, 0x0c2f8773b366fbed),
    ("Bat_los3.smk", 48, 0x0bd3827cee777916, 0x2888ed872214a225, 0xbc0ae141941eee71, 0x4ecbc033b4672dab),
    ("Bat_los4.smk", 55, 0xd3eea54e007b95c8, 0x5f21cf08f5b9a2c7, 0x822332d5c4c949a3, 0xf5fa1d86bdb263a4),
    ("Bat_win1.smk", 66, 0x894f3aef03eed953, 0x3538152a80b9bb1d, 0xd17e0c25b1446c07, 0x9629007bac4fc2bb),
    ("Bat_win2.smk", 48, 0xf4448e43fc3e92da, 0xff232757d8a84625, 0xb2be5e0f3aa3d6a4, 0xe28d5bcad8f7a395),
    ("Bat_win3.smk", 80, 0xf060ecfaa4802305, 0xb5c72125ed0672a5, 0x9fcec1a70175b382, 0x6f2fd4e119f812f9),
    ("Bat_win4.smk", 48, 0x81662f530ab5c82f, 0x07613832ef8b2da5, 0xbf88c3adef15fa0b, 0xcb4eefe5be4e3bbe),
    ("CAS_LOS3.SMK", 62, 0xd50e244069193834, 0x6659855880abf611, 0x2301454f2f8f392f, 0x4dbe2fda9d632ddb),
    ("CAS_WIN3.SMK", 56, 0xe022b9828d7958fe, 0x8ad5c097b6f43185, 0x637b70cfaafa0e5b, 0x783dc5f5a42694e4),
    ("Cap_cty1.smk", 81, 0xc295bed858d4eecc, 0xd0680d5cb713dfce, 0x2ac743569ce4c26a, 0x15056efe08c8a0e9),
    ("Cap_cty2.smk", 82, 0xcf26dc30eea788f3, 0x14f102c1ed0a11f5, 0xd91dc2ae6d319980, 0x7687606ecba2b493),
    ("Cap_cty3.smk", 101, 0x177c9347c5594625, 0xcbae5030140b0d16, 0xb84b44148920bfea, 0xc0195ed73973742d),
    ("Cart_brn.smk", 102, 0x71e167b0ce921a7d, 0x5a84aac1a8c07424, 0x987d977f7b32b3ea, 0x7d713bf505ae0d37),
    ("Cart_bsp.smk", 102, 0xa1851678bdf04d4e, 0x36af2bd6bcd279d5, 0xeac9ec1519c9d1c2, 0x1f9999506d7685df),
    ("Cart_kgt.smk", 102, 0x3d5b1dff64111647, 0x0c88067cc808d25e, 0x987d977f7b32b3ea, 0x13d1d56ef99ad635),
    ("Cas_los1.smk", 111, 0x1ed3321dd1b1126f, 0xd9d29610274f699c, 0xfc4a1224d76ab238, 0xe0bfdb4486a933e1),
    ("Cas_los2.smk", 70, 0x5c6a4bcb885258b9, 0xac655c8d1794c0f5, 0x630be10314b9abe1, 0xd6aab36e5740eee7),
    ("Cas_win1.smk", 69, 0xd2dc3b3d63ebe46f, 0xaf98f8d2f065d268, 0x18639f824d22d8b1, 0x24af167c3e499995),
    ("Cas_win2.smk", 104, 0x65d4a3c061bf6def, 0x82e1275685f90da5, 0x8407ed5436aa7b38, 0x51e2e382b767d591),
    ("Castle1.smk", 101, 0x03c4c40153e5008d, 0x3672ba844215b74b, 0x0ac102e673772676, 0xe7bb1149643450e7),
    ("Castle2.smk", 133, 0x432bd95ce9afff5c, 0x6ee0728b505ba08b, 0x07f6c84fb01f6cc3, 0xb74316783ba61df3),
    ("Castle3.smk", 126, 0xa26d7c06d61938ca, 0xf2897aca46772439, 0xb1569670458daa99, 0xf81371b2259a33b6),
    ("Castle4.smk", 165, 0xa1556bada1843a6d, 0x0f038f3fd7daa9cb, 0xb18b0aa29b5b49fe, 0x88c12a7ac756b108),
    ("Castle5.smk", 219, 0x33f54df3eb2088f2, 0x3d90cba8c4e6ed57, 0x53fb39295eb941c1, 0x88a0cde4f486f59f),
    ("Credits.smk", 537, 0x59b89a2722dc813d, 0x34d3950af24c77e6, 0x64821aa515b75f5e, 0xd14a42b51997706a),
    ("Hang.smk", 125, 0xf8d367f739fa2f71, 0x0007d36f23dca39e, 0x37422b21cc1c2e01, 0xc768c20c0348165a),
    ("Imptitle.smk", 118, 0x955c9ace3628169b, 0xa9ee028bee0d1b65, 0x3fae7fe8d771d764, 0x6a6656ce77653645),
    ("Intro.smk", 1578, 0x184ae9e4e56ea072, 0x821b7e66d97b9095, 0xbb6dac023a8521bb, 0x73685a67f9360b25),
    ("Jail.smk", 178, 0x06bc11b3aed84b74, 0x352bd96567942519, 0x8f8d1b7d939620a8, 0xc0ec1df1b87a1ee5),
    ("LOM.SMK", 1449, 0x870937eacb776164, 0x3434d82263fab9ab, 0xdb37783428a24803, 0x1464f261c6a3b75a),
    ("Pill_brn.smk", 109, 0, 0, 0x550a827a94e19ad5, 0x7fad5d990023cb58),
    ("Pill_cts.smk", 109, 0x582640f1df6f0d8a, 0x9de3450828bceffa, 0xdd1bfaa6d6a80c1f, 0x38b3cdb00cd7b682),
    ("Pill_kgt.smk", 109, 0x93bcf71fec6733d4, 0x1740df2516633769, 0x93f2e23c38e9653a, 0x918aa1df6f949bad),
    ("Sge_los1.smk", 70, 0xe7a9c4568560c92b, 0x9ea40fd873c166bd, 0x0c495c87a8f63dc9, 0xf080eba825548697),
    ("Sge_los2.smk", 70, 0x4cac15925162d85a, 0x99f4323883bb8905, 0x62a95d6dd3df4a3b, 0x181853e3dbe0b081),
    ("Sge_win1.smk", 60, 0xb47181d7c8eddd1d, 0xb978c4cd1be9e055, 0x4424d31b1544b8f8, 0xa291cbc90f5ee15b),
    ("Sge_win2.smk", 60, 0xf58adf0089dc813e, 0x4d8ce8313ad470fd, 0xdd08e92b91c69aaf, 0x946a3f73447d8006),
    ("Win_game.smk", 300, 0xcecd46284ab3ecaa, 0x3007492d9495bd64, 0xcc7b3e6d2354a3dd, 0x53bfd5e808926529),
];

#[test]
fn every_film_matches_an_independent_decoder() {
    let _ = l2_testkit::install!();
    let c = films!();
    assert_eq!(ORACLE.len(), c.len(), "a film this table does not cover, or one it covers twice");
    let mut pcm = 0;
    for &(name, frames, video, palette, audio, frame10) in ORACLE {
        let s = c.get(name).unwrap_or_else(|| panic!("{name} is not in this install"));
        assert_eq!(s.frames, frames, "{name}: frame count");
        assert_eq!(s.frame10, frame10, "{name}: frame 10's pixels");
        assert_eq!(s.audio, audio, "{name}: the audio track");
        if video != 0 {
            assert_eq!(s.video, video, "{name}: every frame's pixels");
            assert_eq!(s.palette, palette, "{name}: every frame's palette");
        }
        pcm += s.pcm;
    }
    // The other decoder's total, recorded in docs/formats/smk.md before this
    // decoder existed.
    assert_eq!(pcm, 8_798_274);
}
