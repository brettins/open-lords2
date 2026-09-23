//! **`Options_Save` (`0x004AE15F`) and `Options_Load` (`0x004AE1B8`)** — the
//! preferences file.
//!
//! # `L2.INI` is not it
//!
//! The install ships one, and it is two lines: `[Config]`, `CDDrive=.`,
//! `WindowsDir=.`. Its only reader is `Ini_ReadConfig` (`0x0040ECC4`,
//! `00400000.c:8355-8357`), which reads **`WindowsDir=` and nothing else** out
//! of it — `CDDrive=` has no reader in the image — then follows that directory
//! to `sierra.ini` for `CDRomDrives=`, `CDROM=`, `SoundChannels=`,
//! `PhysicalMem=`, `VideoSpeed=` and CPU. `[V]`. Nothing writes either file:
//! `grep PrivateProfile` over the whole decompilation finds no hit, and the
//! parser is a substring search that never looks at `[Config]` at all.
//!
//! # `lords2.inf` is
//!
//! `Options_Save` does `__write(fd, &g_options, 0x468)` — the whole preference
//! block as bytes, `004a0000.c:5978-5981`. One call site, `FUN_0049767D`
//! (`00490000.c:3084`), itself reached only from `0x0040EBFE` **after the
//! message loop ends**: a crash loses the session's settings. `Options_Load`
//! reads it back in `App_WinMain` before `App_InitWindow`
//! (`00400000.c:8221`), over defaults `Options_SetDefaults` (`0x004AE310`) has
//! just laid down, and `Options_Validate` (`0x004AE2BD`) throws the file away
//! wholesale when `g_optionsMagic` is not `0x7EC`.
//!
//! **So the tip-screens flag does persist.** `g_optTipScreens` (`0x0053F24C`)
//! is inside that block — `0x0053F1E0 + 0x6C`, under `+0x468` — and it has
//! exactly two writers in the image: `Options_SetDefaults`
//! (`004a0000.c:6090`, to 1) and `Opt_ToggleTipScreens` (`0x00434787`,
//! `00430000.c:1716`). Neither `Game_NewGame` nor the save loader touches it,
//! which is why [`crate::game::Prefs`] is carried across a new game and a load
//! rather than defaulted with the world.
//!
//! # Ours
//!
//! We do not write the original's format (`docs/environment.md`, *Where our
//! saves go*), so this is a text file beside our saves with one key per
//! `g_options` field we carry, named after the global. `Magic` is
//! `g_optionsMagic`, and a file without it is discarded the way
//! `Options_Validate` discards one.

use std::path::{Path, PathBuf};

use crate::game::Prefs;
use crate::setup::{option, SetupOptions};
use crate::text::PlayerName;

pub const FILE: &str = "prefs.ini";

/// `g_optionsMagic` (`0x0053F204`) — 0x7EC, checked by `Options_Validate`.
pub const MAGIC: i32 = 0x7EC;

/// `[Options]`, after `g_options` (`0x0053F1E0`). The original's own INI has no
/// reader that looks at a section header, so ours is a label, and [`decode`]
/// skips it the same way.
pub const SECTION: &str = "Options";

pub fn path() -> Option<PathBuf> {
    crate::saves::dir().map(|d| d.join(FILE))
}

/// Key names, each the original's global minus its `g_opt` prefix.
const KEYS: &[(&str, &str)] = &[
    ("Music", "g_optMusic 0x0053F218"),
    ("SoundEffects", "g_optSoundEffects 0x0053F214"),
    ("Speech", "g_optSpeech 0x0053F20C"),
    ("Animations", "g_optAnimations 0x0053F248"),
    ("TipScreens", "g_optTipScreens 0x0053F24C"),
    ("ToolTips", "g_optToolTips 0x0053F250"),
    ("ScrollSpeed", "g_optScrollSpeed 0x0053F234"),
    ("GameSpeed", "g_optGameSpeed 0x0053F230"),
    // [D] ours: `Prefs::debug_overlay` has no original. Nothing in the image
    // draws it, so nothing in the image persists it.
    ("DebugOverlay", "ours"),
    ("FullScreen", "g_optFullScreen 0x0053F228"),
];

/// `g_diploLetterDraft` (`0x0053F2B8`): four buffers of 200 bytes.
pub const LETTER_BYTES: usize = 200;
pub type Letter = [u8; LETTER_BYTES];

/// The rest of `g_options` that has a reader in our game. [V] layout: the
/// shipped `lords2.inf` (0x468 bytes) read against `Options_SetDefaults`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kept {
    /// `g_options+0x00`, 31 bytes, `Mem_CopyBytes(0x4DF17C, 0x53F1E0, 0x1F)`
    /// ("Player1"); page 4 edits it in place, `Edit_Begin(&g_options, 0x10, 0xC0, 0)`.
    pub player_name: PlayerName,
    /// `g_setupOpt*` `+0xA8..+0xD4`. `Setup_DefaultOptions` (`0x004AE539`) has two
    /// callers that matter: `Options_SetDefaults` and *Defaults*, `FUN_004335F0`
    /// hotspot 3. [V]
    pub setup: SetupOptions,
    /// `g_diploLetterDraft` `+0xD8`. `None` is `Options_SetDefaults`' copy of
    /// `L2.eng` group 226, `004a0000.c:6115-6129`.
    pub letters: [Option<Letter>; 4],
}

impl Default for Kept {
    fn default() -> Kept {
        Kept {
            player_name: PlayerName::new(crate::text::DEFAULT_PLAYER_NAME),
            setup: SetupOptions::new(),
            letters: [None; 4],
        }
    }
}

/// `g_setupOpt*` in block order, `0x0053F288` step 4, to the drop-down index.
const SETUP_KEYS: [(&str, usize); 12] = [
    ("SetupOptNobles", option::NOBLES),
    ("SetupOptTimeLimit", option::TIME_LIMIT),
    ("SetupOptDifficulty", option::DIFFICULTY),
    ("SetupOptWeapons", option::WEAPONS),
    ("SetupOptArmySize", option::ARMY_SIZE),
    ("SetupOptAdvancedFarming", option::ADVANCED_FARMING),
    ("SetupOptArmiesEat", option::ARMIES_EAT),
    ("SetupOptExploration", option::EXPLORATION),
    ("SetupOptCrowns", option::CROWNS),
    ("SetupOptCountyStatus", option::COUNTY_STATUS),
    ("SetupOptCastle", option::STARTING_CASTLE),
    ("SetupOptFight", option::FIGHT),
];

pub fn letter(s: &str) -> Letter {
    let mut out = [0u8; LETTER_BYTES];
    let chars = s.chars().filter(|c| (0x20..0x100).contains(&(*c as u32)));
    // 004b0000.c:7255 FUN_0040210c(draft, 199): at most 199, the 200th byte stays NUL.
    // [D] characters below 0x20 are dropped, not cut at (Options_SetDefaults 004a0000.c:6121-6127).
    for (slot, c) in out.iter_mut().take(LETTER_BYTES - 1).zip(chars) {
        *slot = c as u8;
    }
    out
}

pub fn letter_text(l: &Letter) -> String {
    l.iter().take_while(|b| **b != 0).map(|b| *b as char).collect()
}

fn encode_kept(k: &Kept, out: &mut String) {
    for (key, i) in SETUP_KEYS {
        out.push_str(&format!("{key}={}\n", k.setup.get(i)));
    }
    out.push_str(&format!("PlayerName={}\n", k.player_name.as_str()));
    for (n, l) in k.letters.iter().enumerate() {
        if let Some(l) = l {
            out.push_str(&format!("LetterDraft{}={}\n", n + 1, letter_text(l)));
        }
    }
}

/// The text keys take the whole rest of the line: a name or a letter may hold `;`.
fn set_text(k: &mut Kept, key: &str, value: &str) -> bool {
    if key == "PlayerName" {
        k.player_name = PlayerName::new(value);
        return true;
    }
    let Some(n) = key.strip_prefix("LetterDraft").and_then(|n| n.parse::<usize>().ok()) else {
        return false;
    };
    if let Some(slot) = n.checked_sub(1).and_then(|i| k.letters.get_mut(i)) {
        *slot = Some(letter(value));
    }
    true
}

fn value_of(p: &Prefs, key: &str) -> i32 {
    match key {
        "Music" => i32::from(p.music),
        "SoundEffects" => i32::from(p.effects),
        "Speech" => i32::from(p.speech),
        "Animations" => i32::from(p.animations),
        "TipScreens" => i32::from(p.tip_screens),
        "ToolTips" => i32::from(p.tool_tips),
        "ScrollSpeed" => p.scroll_speed,
        "GameSpeed" => p.game_speed,
        "DebugOverlay" => i32::from(p.debug_overlay),
        "FullScreen" => i32::from(p.full_screen),
        _ => 0,
    }
}

fn set(p: &mut Prefs, key: &str, v: i32) {
    match key {
        "Music" => p.music = v != 0,
        "SoundEffects" => p.effects = v != 0,
        "Speech" => p.speech = v != 0,
        "Animations" => p.animations = v != 0,
        "TipScreens" => p.tip_screens = v != 0,
        "ToolTips" => p.tool_tips = v != 0,
        "ScrollSpeed" => p.scroll_speed = clamp_speed(v),
        "GameSpeed" => p.game_speed = clamp_speed(v),
        "DebugOverlay" => p.debug_overlay = v != 0,
        "FullScreen" => p.full_screen = v != 0,
        _ => {
            if let Some((_, i)) = SETUP_KEYS.iter().find(|(k, _)| *k == key) {
                p.kept.setup.set(*i, v.max(0) as usize);
            }
        }
    }
}

/// [D] ours. `Options_Validate` forces two fields after the read and clamps
/// neither speed; a spinner whose value is off its eleven steps is a defect a
/// hand-edited file would introduce and the original could not.
fn clamp_speed(v: i32) -> i32 {
    v.clamp(Prefs::SPEED_MIN, Prefs::SPEED_MAX)
}

pub fn encode(p: &Prefs) -> String {
    let mut out = format!("[{SECTION}]\nMagic={MAGIC}\n");
    for (key, origin) in KEYS {
        out.push_str(&format!("{key}={}\t; {origin}\n", value_of(p, key)));
    }
    encode_kept(&p.kept, &mut out);
    out
}

/// `Options_SetDefaults` then the file over it, then `Options_Validate`:
/// a wrong or missing `Magic` keeps the defaults whole.
pub fn decode(text: &str) -> Prefs {
    let mut p = Prefs::default();
    let mut magic = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else { continue };
        let key = key.trim();
        if key.starts_with(['[', ';']) || set_text(&mut p.kept, key, value) {
            continue;
        }
        let value = value.split(';').next().unwrap_or("").trim();
        let Ok(v) = value.parse::<i32>() else { continue };
        if key == "Magic" {
            magic = Some(v);
        } else {
            set(&mut p, key, v);
        }
    }
    if magic == Some(MAGIC) { p } else { Prefs::default() }
}

/// `Options_Load`, called once at start-up. A missing file leaves the defaults,
/// which is the original's `__open` returning -1.
pub fn load() -> Prefs {
    let Some(path) = path() else { return Prefs::default() };
    match std::fs::read_to_string(&path) {
        Ok(text) => decode(&text),
        Err(_) => Prefs::default(),
    }
}

/// `Options_Save`. [D] ours: called whenever the block changes, not once after
/// the loop (`0x0040EBFE`), so a crash keeps the session's settings. C???.
pub fn save(p: &Prefs) -> Result<PathBuf, crate::saves::Error> {
    write(p, remembered_install().as_deref())
}

/// [D] ours, no `g_options` field: where the install was found. `crate::install`.
pub const INSTALL_KEY: &str = "InstallDir";

pub fn remembered_install() -> Option<PathBuf> {
    install_in(&std::fs::read_to_string(path()?).ok()?)
}

fn install_in(text: &str) -> Option<PathBuf> {
    text.lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == INSTALL_KEY)
        .map(|(_, v)| PathBuf::from(v.trim()))
        .filter(|p| !p.as_os_str().is_empty())
}

pub fn remember_install(p: &Prefs, dir: &Path) -> Result<PathBuf, crate::saves::Error> {
    write(p, Some(dir))
}

fn write(p: &Prefs, install: Option<&Path>) -> Result<PathBuf, crate::saves::Error> {
    let path = path().ok_or(crate::saves::Error::NoDirectory)?;
    let dir = path.parent().expect("path joins a directory").to_path_buf();
    std::fs::create_dir_all(&dir)
        .map_err(|e| crate::saves::Error::Io { path: dir, detail: e.to_string() })?;
    let mut text = encode(p);
    if let Some(d) = install {
        text.push_str(&format!("{INSTALL_KEY}={}\n", d.display()));
    }
    std::fs::write(&path, text)
        .map_err(|e| crate::saves::Error::Io { path: path.clone(), detail: e.to_string() })?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flipped() -> Prefs {
        Prefs {
            tip_screens: false,
            tool_tips: false,
            music: false,
            scroll_speed: 30,
            game_speed: 40,
            debug_overlay: true,
            full_screen: false,
            kept: Kept {
                player_name: PlayerName::new("Ed; the Bold"),
                setup: {
                    let mut s = SetupOptions::new();
                    for (n, (_, i)) in SETUP_KEYS.iter().enumerate() {
                        s.set(*i, n % 2);
                    }
                    s
                },
                letters: [None, Some(letter("Thou art; a knave")), None, Some(letter("x"))],
            },
            ..Prefs::default()
        }
    }

    /// The shipped `lords2.inf`, 0x468 bytes, is the layout `Kept` cites.
    #[test]
    fn the_shipped_block_holds_the_fields_where_kept_says() {
        let Some(dir) = l2_testkit::install_dir() else { l2_testkit::skip!("no game install") };
        let Ok(b) = std::fs::read(dir.join("lords2.inf")) else { l2_testkit::skip!("no lords2.inf") };
        assert_eq!(b.len(), 0x468, "Options_Save's __write length");
        let dword = |o: usize| i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        assert_eq!(dword(0x24), MAGIC, "g_optionsMagic at +0x24");
        for (n, (key, _)) in SETUP_KEYS.iter().enumerate() {
            let at = 0xA8 + 4 * n;
            assert!((0..=6).contains(&dword(at)), "{key} at +{at:#X} is a drop-down index");
        }
        let name = PlayerName::from_bytes(b[..crate::text::PLAYER_NAME_LEN].try_into().unwrap());
        assert!(!name.is_empty(), "the player name at +0x00");
        assert!(b[0xD8..0xD8 + 0x320].iter().any(|c| *c >= 0x20), "the letters at +0xD8");
    }

    #[test]
    fn every_field_we_carry_survives_the_round_trip() {
        let p = flipped();
        assert_eq!(decode(&encode(&p)), p);
        let keys = KEYS.iter().map(|(k, _)| *k).chain(SETUP_KEYS.iter().map(|(k, _)| *k));
        for key in keys.chain(["PlayerName", "LetterDraft2", "LetterDraft4"]) {
            assert!(encode(&p).contains(&format!("\n{key}=")), "{key} is not written");
        }
        assert!(!encode(&p).contains("LetterDraft1="), "an untouched draft stays L2.eng's");
    }

    #[test]
    fn the_tutorial_toggle_comes_back_off() {
        let text = encode(&Prefs { tip_screens: false, ..Prefs::default() });
        assert!(!decode(&text).tip_screens, "the flag the player turned off came back on");
        assert!(Prefs::default().tip_screens, "Options_SetDefaults 004a0000.c:6090 sets it");
    }

    /// `Options_Validate` (`0x004AE2BD`) re-defaults the whole block when
    /// `g_optionsMagic` is not `0x7EC`.
    #[test]
    fn a_file_without_the_magic_is_discarded_whole() {
        let good = encode(&flipped());
        assert_eq!(decode(&good.replace(&format!("Magic={MAGIC}"), "Magic=1")), Prefs::default());
        assert_eq!(decode(&good.replace(&format!("Magic={MAGIC}\n"), "")), Prefs::default());
        assert_eq!(decode(""), Prefs::default());
        assert_eq!(decode("[Options]\nMagic=2028\nTipScreens=junk\n"), Prefs::default());
    }

    #[test]
    fn a_hand_edited_speed_is_clamped_to_the_spinners_range() {
        let text = format!("[{SECTION}]\nMagic={MAGIC}\nScrollSpeed=9999\nGameSpeed=-5\n");
        assert_eq!(decode(&text).scroll_speed, Prefs::SPEED_MAX);
        assert_eq!(decode(&text).game_speed, Prefs::SPEED_MIN);
    }

    #[test]
    fn the_file_lands_beside_our_saves_and_never_inside_an_install() {
        let dir = std::env::temp_dir().join(format!("l2-prefs-{}", std::process::id()));
        let _scope = crate::saves::scoped_dir(&dir);
        let written = save(&flipped()).expect("write the preferences file");
        assert_eq!(written, dir.join(FILE));
        assert_eq!(load(), flipped());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(load(), Prefs::default(), "a missing file leaves the defaults");
    }

    #[test]
    fn full_screen_is_the_default_and_windowed_is_remembered() {
        assert!(Prefs::default().full_screen, "Display_Init 0x0042E5C0 forces g_optFullScreen = 1");
        let text = encode(&Prefs { full_screen: false, ..Prefs::default() });
        assert!(!decode(&text).full_screen, "the windowed choice came back full screen");
    }

    #[test]
    fn the_install_path_survives_a_settings_save() {
        let dir = std::env::temp_dir().join(format!("l2-prefs-install-{}", std::process::id()));
        let _scope = crate::saves::scoped_dir(&dir);
        let game = PathBuf::from(r"D:\Play\Lords; II");
        remember_install(&Prefs::default(), &game).unwrap();
        assert_eq!(remembered_install(), Some(game.clone()));
        save(&flipped()).unwrap();
        assert_eq!(remembered_install(), Some(game), "save dropped the remembered install");
        assert_eq!(load(), flipped(), "the install line disturbed the settings");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The shipped `L2.INI`, which is `Ini_ReadConfig`'s input and not a
    /// preferences file: if it ever grew a player-facing key, this fails.
    #[test]
    fn the_installs_l2_ini_holds_only_the_two_hardware_keys() {
        let Some(dir) = l2_testkit::install_dir() else { l2_testkit::skip!("no game install") };
        let Ok(text) = std::fs::read_to_string(dir.join("L2.INI")) else {
            l2_testkit::skip!("no L2.INI in the install")
        };
        let keys: Vec<String> = text
            .lines()
            .filter_map(|l| l.split_once('='))
            .map(|(k, _)| k.trim().to_string())
            .collect();
        assert_eq!(keys, ["CDDrive", "WindowsDir"], "L2.INI in {}", dir.display());
        assert!(text.contains("[Config]"), "the one section header, which no reader parses");
        for absent in ["TipScreens", "ToolTips", "Music", "Speech", "Animations"] {
            assert!(!text.contains(absent), "{absent} is in lords2.inf, not L2.INI");
        }
    }
}
