#![allow(unused_imports)]
use super::*;
use super::title_and_speech::*;
use super::*;
use super::routing::*;
use super::audio_controls_and_feedback::*;
use l2_game::audio::{self, Audio, Scene};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::message;
use l2_game::screens::setup::SetupPage;
use l2_game::Game;

/// **A letter's fanfare takes the one-shot buffer, and the lord cuts it off.**
///
/// `Msg_DrawWindow` opens a letter with `Sound_PlayFile("ff_msg.wav", 1, 0)` —
/// nothing in front of it, and the *speech* flag — and speaks 200 ticks later
/// through `Msg_PlayVoice`, which puts `Sound_StopOneShot()` in front of its own
/// `Sound_PlayFile`. So the fanfare is the buffer's occupant, the lord talks
/// over a trumpet that is still going by stopping it, and the switch that
/// silences the trumpet is Speech, not Sound Effects. `[V]`
///
/// Nothing is mixed, so the fanfare is still sounding when the lord is asked
/// for — the case that tells the two verbs apart.
///
/// Ablations: speak through `play_file` and
/// the lord is dropped; play the fanfare with `false` and Speech: Off no
/// longer silences it.
#[test]
fn a_letters_fanfare_holds_the_buffer_and_the_lord_cuts_it_off() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let assets = Assets::placeholder();
    let fanfare = l2_game::audio::names::fanfare::MESSAGE;
    let lord = l2_game::audio::names::message_voice(170, 0).expect("group 170 has a voice");
    let letter = |game: &Game| {
        let mut rec = message::Record::default();
        rec.group = 170;
        rec.category = message::category::LETTER;
        rec.to = game.player;
        rec
    };

    let mut game = world();
    game.prefs.tip_screens = false;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    assert!(game.messages.enqueue(letter(&game), game.player), "the letter was accepted");
    let mut held = None;
    for _ in 0..240 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        director.listen(&mut audio, &machine, &game);
        if held.is_none() && audio.heard().contains(&fanfare) {
            held = Some(audio.is_playing(fanfare) && audio.one_shot_busy());
        }
        if audio.is_playing(&lord) {
            break;
        }
    }
    assert_eq!(held, Some(true), "the fanfare did not take the buffer - heard {:?}", audio.heard());
    assert!(audio.is_playing(&lord), "the lord was dropped over his fanfare - heard {:?}", audio.heard());
    assert!(!audio.is_playing(fanfare), "and the fanfare was cut off rather than left under him");

    // Speech: Off, Sound Effects: On — and the trumpet is silent.
    let mut game = world();
    game.prefs.tip_screens = false;
    game.prefs.speech = false;
    let mut machine = Machine::new(APP_ROOT);
    machine.push(ScreenId::Campaign);
    let mut audio = Audio::headless(&platform.vfs);
    let mut director = audio::Director::new();
    game.messages.enqueue(letter(&game), game.player);
    for _ in 0..20 {
        let mut ctx = Ctx { game: &mut game, assets: &assets };
        machine.update(&mut ctx);
        director.listen(&mut audio, &machine, &game);
    }
    assert!(machine.ids().contains(&ScreenId::Message), "the letter never opened");
    assert!(!audio.heard().contains(&fanfare), "Speech: Off left the letter's fanfare - heard {:?}", audio.heard());
}

/// **The ten industry lines a player asked for by name.**
///
/// `Industry_ToggleFromMap` (`0x0043D309`) ends with
/// `Msg_Enqueue(0, g_localPlayer, local_10 + 0xE6, …)`, `local_10` being
/// `industry * 2 + on` for the four industries and `-1`/`-2` for the castle
/// switch. **We supply the voice; the enqueue is the industry branch's.** So
/// this asserts the half that is ours — that every one of those ten groups
/// resolves to a clip that ships — and it will start speaking the moment the
/// message arrives, with no further change here.
#[test]
fn the_industry_toggle_groups_all_have_a_voice_that_ships() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut audio = Audio::headless(&platform.vfs);
    // 228/229 are the castle switch, 230..237 the four industries off and on.
    let labels = [
        "Building off", "Building on", "Forestry off", "Forestry on",
        "Mining off", "Mining on", "Blacksmith off", "Blacksmith on",
        "Quarrying off", "Quarrying on",
    ];
    for (i, label) in labels.iter().enumerate() {
        let group = 228 + i as u16;
        let name = l2_game::audio::names::message_voice(group, 0)
            .unwrap_or_else(|| panic!("group {group} ({label}) has no voice"));
        assert_eq!(name, format!("S{group}_01.wav"));
        // `Msg_PlayVoice`'s own verb, which stops the buffer first and so never
        // drops the next group's clip over this one's.
        audio.stop_and_play_file(&name, true);
        assert!(
            audio.heard().contains(&name.to_ascii_lowercase().as_str()),
            "group {group} ({label}) -> {name} did not decode"
        );
    }
}

/// **How much of the game's audio the engine can play, measured.**
///
/// 771 files ship. The number that matters is how many of them any code path
/// can reach, and it was **0** until the fix in `docs/decisions.md`
/// C116.
/// — in three documents, by typing — it was wrong: `battle5.wav` is in the
/// table, ships, decodes, and **cannot be reached**, because the counter that
/// selects it is `DAT_0057A0F0`, the third battle mode `BattleKind` does not
/// name.
///
/// So the set is collected by *driving every scene the policy can produce*
/// through the real `follow` and reading `Audio::heard` — what travelled the
/// road, which is `docs/agents.md`'s standard for a field being tested at all.
/// A new call site makes this go red with the name it added, which is the only
/// way this count stays true.
#[test]
fn the_voice_class_is_84_percent_of_the_games_audio() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut audio = Audio::headless(&platform.vfs);

    // Every name `Msg_PlayVoice` can produce, asked for through its own verb,
    // `stop_and_play_file` — `Sound_StopOneShot(); Sound_PlayFile(name, 1, 0)`,
    // which never drops, so nothing here needs mixing. The three bands are the original's tables: 170..=197 have
    // a lord and sixteen takes, 100..=169 and 200..=284 have one clip each.
    //
    // **Except the tip groups, 200..=218**, which this count used to include
    // and which no player of this engine can hear. Their windows are
    // categories 0x05..=0x09, and `Tip_Show` (`0x00476DA9`) is the only
    // function in the original that posts one; nothing here posts a tip. A
    // name-driven loop proves the *name* resolves, not that the game can ask
    // for it, and thirteen tip clips were sitting in the 543 on that basis.
    // `docs/audio.json` `Msg_DrawWindow#24` and `FUN_004b3acd#1`.
    //
    // **Not 219**, though `FUN_00476A5D` clears twenty tip flags from 200:
    // `L2.eng` 219 is *"Already in alliance."*, a refusal `Diplo_SendClicked`
    // posts, and its clip ships. The first draft of this range swallowed it and
    // the count came out one short — which is the check working.
    //
    // What this does not establish: that every *other* group in the bands is
    // posted by this engine. Only the tip band has been checked.
    const TIP_GROUPS: std::ops::RangeInclusive<u16> = 200..=218;
    for group in (100..=300u16).filter(|g| !TIP_GROUPS.contains(g)) {
        for variant in 0..16u8 {
            if let Some(name) = l2_game::audio::names::message_voice(group, variant) {
                audio.stop_and_play_file(&name, true);
            }
        }
    }
    let spoken = audio.heard().len();
    assert_eq!(
        spoken, 530,
        "the narrator's reachable performance moved: {spoken} clips"
    );
    // 448 lord takes and 82 system clips. The system bands outside the tips
    // cover 135 groups and only 82 of them ship an `_01`.
    // the tables - a group with no clip is a message the narrator does not read.
    assert_eq!(audio.heard().iter().filter(|n| n.starts_with('s')).count(), 82);
    assert_eq!(spoken - 82, 448);

    // **What is still out of reach in this class**, so the number is not read
    // as "the voice is done": the tip screens, first line and chain alike.
    // `FUN_004B3ACD(group)` walks a five-wide table at `0x004E1E40` whose only
    // live rows are groups 200..=218, playing `S201_02.wav + (n - 1) * 0x10`
    // one clip at a time as each finishes. This used to name `S010_13.wav` as
    // the chain's evidence, and that clip is not in the chain at all: it is
    // `g_msgVoiceS010`'s, a sibling table with a different caller.
    for clip in ["s200_01.wav", "s201_01.wav", "s201_02.wav", "s218_03.wav"] {
        assert!(!audio.heard().contains(&clip), "{clip} is a tip clip and no tip is posted");
    }
}

#[test]
fn the_music_fanfares_screens_and_the_click_are_fifty_five_more() {
    let Some(dir) = l2_testkit::install_dir() else {
        l2_testkit::skip!("no game install, so nothing to open");
    };
    let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
    let mut audio = Audio::headless(&platform.vfs);
    assert_eq!(audio.file_count(), 771, "the install's sound count moved");

    // The front end's own bed, which is `Music_Play`'s and not either picker's.
    audio.follow(Scene::FrontEnd);
    // Every rung of `Music_StartCampaign`'s ladder, both clauses.
    for (counties, share) in [(1, 7), (2, 7), (2, 8), (2, 15), (2, 29), (2, 43)] {
        audio.follow(Scene::Campaign { county_count: counties, share_of_map_pct: share });
    }
    // Both battle pairs, twice each, so both sides of both toggles are taken.
    for _ in 0..2 {
        for kind in [
            l2_game::audio::track::BattleKind::Field,
            l2_game::audio::track::BattleKind::Siege,
        ] {
            audio.follow(Scene::Campaign { county_count: 1, share_of_map_pct: 7 });
            audio.follow(Scene::Battle(kind));
        }
    }
    // And the two fanfares `Director::listen` fires, by the same names it uses.
    audio.play_effect(l2_game::audio::names::fanfare::MESSAGE);
    audio.play_effect(l2_game::audio::names::fanfare::BATTLE);
    // `ff_capt.wav`, which `Msg_DrawWindow` plays for the conquest band and
    // which had no caller until the message window arrived.
    audio.play_effect(l2_game::audio::names::fanfare::CAPTURED);

    // **The screen class**, which `tests/audio_screens.rs` drives through the
    // machine and which is asked for here by name, so that this one assertion
    // stays the whole count of what the engine can reach outside the narrator.
    // Six bank slots from `g_jobSound`, four from `TileInfo_Draw`'s ladder —
    // which overlap, because a mine sounds the same in the village and on the
    // map — the forge, and five spoken lines.
    for job in 1..=9usize {
        if let Some(n) = l2_game::audio::names::slot(
            l2_game::audio::names::Bank::Kingdom,
            l2_game::audio::names::JOB_SOUND[job],
        ) {
            audio.play_effect(n);
        }
    }
    audio.play_effect(l2_game::audio::names::blacksmith::FIRE);
    for graphic in 0..13u8 {
        if let Some(n) = l2_game::audio::names::resource_site_slot(graphic)
            .and_then(|s| l2_game::audio::names::slot(l2_game::audio::names::Bank::Kingdom, s))
        {
            audio.play_effect(n);
        }
    }
    for line in [
        l2_game::audio::names::speech::RATION_NOT_MET,
        l2_game::audio::names::speech::RATION_ON_DAIRY,
        l2_game::audio::names::speech::SUPPLIES,
        l2_game::audio::names::speech::ZOOM_OUT,
        l2_game::audio::names::speech::SPLIT_ARMY,
        l2_game::audio::names::speech::CHOOSE_YOUR_SHIELD,
    ] {
        // `play_effect`, not the director's `play_file`: nothing here mixes, so
        // the dropping verb would open the first line and drop the other five.
        audio.play_effect(line);
    }
    // **The four screen-voice tables**, which are the sibling-thunk band:
    // `FUN_004B3714`'s mercenary offer, `FUN_004B3768`'s health line and
    // `FUN_004B37BC`'s picked unit or castle.
    //
    // **Twenty-five distinct files, and the health line contributes four.**
    // `healthBand` is `0 ..= 4`, entries 3 and 4 of the table are both
    // `S020_04.wav`, and so **`S020_05.wav` ships, is named in the binary, and
    // nothing in a running game can ask for it** — `battle5.wav`'s situation
// exactly, and found the same way: by counting distinct files
    // table entries.
    for line in l2_game::audio::names::speech::MERCENARY_OFFER
        .iter()
        .take(l2_kingdom::mercenary::ROSTER.len() - 1)
        .chain(l2_game::audio::names::speech::POPULATION_HEALTH.iter().take(5))
        .chain(l2_game::audio::names::speech::PICKED_UNIT.iter())
        .chain(l2_game::audio::names::speech::PICKED_CASTLE.iter())
    {
        audio.play_effect(line);
    }
    // The four movement sounds `Director::hear_the_march` asks for.
    for slot in [5usize, 11, 12] {
        if let Some(n) =
            l2_game::audio::names::slot(l2_game::audio::names::Bank::Kingdom, slot)
        {
            audio.play_effect_if_idle(n);
        }
    }
    // **The pointer click** — `Widget_Test`'s `Sound_RestartSlot(1)`, the slot
    // `Director::hear_the_click` asks for. `tests/click.rs` is when it sounds.
    if let Some(n) = l2_game::audio::names::slot(l2_game::audio::names::Bank::Kingdom, 1) {
        audio.play_effect(n);
    }

    assert_eq!(
        audio.heard(),
        [
            "army.wav",
            "battle1.wav",
            "battle2.wav",
            "battle3.wav",
            "battle4.wav",
            "click3.wav",
            "fallow.wav",
            "ff_batl.wav",
            "ff_capt.wav",
            "ff_msg.wav",
            "fire.wav",
            "iron.wav",
            "merchant.wav",
            "moo_2.wav",
            "rioters.wav",
            "s011_02.wav",
            "s016_01.wav",
            "s016_02.wav",
            "s016_03.wav",
            "s016_04.wav",
            "s016_05.wav",
            "s016_06.wav",
            "s016_07.wav",
            "s016_08.wav",
            "s016_09.wav",
            "s016_10.wav",
            "s016_11.wav",
            "s016_12.wav",
            "s017_01.wav",
            "s020_01.wav",
            "s020_02.wav",
            "s020_03.wav",
            "s020_04.wav",
            "s021_01.wav",
            "s021_02.wav",
            "s031_01.wav",
            "s031_02.wav",
            "s031_03.wav",
            "s031_04.wav",
            "s033_01.wav",
            "s033_02.wav",
            "s071_02.wav",
            "s071_03.wav",
            "s071_04.wav",
            "s071_05.wav",
            "s071_06.wav",
            "scroll1.wav",
            "scroll2.wav",
            "scroll3.wav",
            "scroll4.wav",
            "scroll5.wav",
            "setup.wav",
            "stonecut.wav",
            "wheat.wav",
            "woodcut.wav",
        ],
        "the set of sounds this engine can reach has changed. If a call site was \
         ADDED this is good news and the number in crates/l2-game/src/audio/mod.rs, \
         docs/mechanics.md and docs/decisions.md C116 moves with it."
    );
    assert_eq!(audio.heard().len(), 55, "55 of 771 outside the voice class");

    // `battle5.wav` ships and decodes; nothing can ask for it.
    // gap in the wiring, it is `DAT_0057A0F0` being unidentified, and
    // `BattleKind` refusing to guess at a third mode is why.
    assert!(!audio.heard().contains(&"battle5.wav"));
}


