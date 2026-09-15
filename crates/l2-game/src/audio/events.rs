#![allow(unused_imports)]
use super::*;
use super::engine::*;
use super::director::*;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use mixer::Mixer;
use track::{BattleCycle, BattleKind, Music};
use wav::Sound;

/// **When the narrator speaks, by message category** — `Msg_DrawWindow`'s
/// (`0x0047309E`) voice schedule.
///
/// `Msg_DrawWindow` is 10,915 bytes: it dismisses,
/// enqueues, sets its own timer and plays its own sound, all from inside the
/// draw. Every one of its
/// sixteen `Msg_PlayVoice` calls is guarded by `g_messageTimer == <constant>`,
/// and `g_messageTimer` counts **down** from [`crate::message::TIMER_START`]
/// (2000), one per tick. `[V]`.
///
/// `Msg_DrawWindow` delegates them to `Msg_DrawDiplomacy` (`0x00475E07`) and
/// `Msg_DrawBeyondLetter` (`0x00476488`), which carry a voice call of their own
/// on their own schedule. Reading those two is a separate job and inventing a
/// tick for them would be worse than the silence. `docs/audio-triggers.md`
/// records them as unread.
pub fn voice_tick(category: u8) -> Option<i32> {
    use crate::message::category as c;
    Some(match category {
        c::NOTICE | c::CAPTURE => 0x776,
        c::LETTER | c::PAY_PROMPT | c::ALLIANCE_PROMPT => 0x708,
        c::ENDING => 0x76C,
        c::TIP => 0x5A,
        c::COUNTY_PORTRAIT
        | c::COUNTY_NOTICE
        | c::EVENT
        | c::COUNTY_TALL
        | c::GARRISON_PROMPT
        | c::CASTLE => 0x7C6,
        n if (c::PARAGRAPHS_FIRST..=c::PARAGRAPHS_LAST).contains(&n) => 0x7C6,
        _ => return None,
    })
}

/// **The fanfare a message window opens with**, on the frame the timer is still
/// [`crate::message::TIMER_START`] — `Msg_DrawWindow`'s five `Sound_PlayFile`
/// calls, which is all of them. `[V]`.
///
/// category: `0x71 < group && group < 0x7F`, which is `L2.eng` 114…126. That is
/// the one place in the audio layer where a group decides a sound, and it is
/// why [`names::fanfare::CAPTURED`] had no caller until now.
pub(super) fn open_fanfare(category: u8, group: u16) -> Option<&'static str> {
    use crate::message::category as c;
    match category {
        c::NOTICE if (0x72..=0x7E).contains(&group) => Some(names::fanfare::CAPTURED),
        c::LETTER | c::PAY_PROMPT | c::ALLIANCE_PROMPT | c::CAPTURE => {
            Some(names::fanfare::MESSAGE)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// `Sound_PlaySlot(n)`, or its 28-byte thunk `FUN_004262CF(n)` — a
    /// **1-based** slot of [`names::BATTLE_BANK`], dropped if that buffer is
    /// still sounding. [`Audio::play_effect_if_idle`].
    Slot(usize),
    File(&'static str),
}

/// Three of the twenty-five sites are not here, and each is `blocked` in
/// `docs/audio.json` on a mechanic this engine does not model: state 17's own
/// loose (`BattleMan_StateCloseToAttack` ×2) and a realm eliminated
/// mid-battle (`FUN_0047FE0B`). Fire, boiling oil, a tower docking and the
/// rampart too high to shoot down were the other six, and `l2_sim::fire` is
/// where they went.
pub fn battle_requests(was: &l2_sim::Cues, now: &l2_sim::Cues) -> Vec<Request> {
    use l2_sim::{Troop, WeaponClass, ALL_TROOPS, SIDE_A, SIDE_B};
    let struck = |t: Troop| now.melee_casualties(t) != was.melee_casualties(t);
    let hit = |w: WeaponClass| now.missile_hits(w) != was.missile_hits(w);
    let felled = |w: WeaponClass| now.missile_casualties(w) != was.missile_casualties(w);
    let loosed = |w: WeaponClass| now.loosed(w) != was.loosed(w);
    let mut out = Vec::new();
    let mut ask = |moved: bool, request: Request| {
        if moved {
            out.push(request);
        }
    };

    // **`Melee_Tick` (`0x00494908`), a man falling to a blow.** The sword is
    // chosen by the troop that **struck** him: `other.troopType == 2 ? 4 : == 3
    // ? 5 : == 6 ? 5 : 6`. `[V]`
    // sfx: Melee_Tick#1
    ask(struck(Troop::Macemen), Request::Slot(4));
    // sfx: Melee_Tick#2
    ask(struck(Troop::Swordsmen), Request::Slot(5));
    // sfx: Melee_Tick#3
    ask(struck(Troop::Knights), Request::Slot(5));
    // sfx: Melee_Tick#4
    ask(
        ALL_TROOPS
            .iter()
            .filter(|t| !matches!(t, Troop::Macemen | Troop::Swordsmen | Troop::Knights))
            .any(|&t| struck(t)),
        Request::Slot(6),
    );
    // `me.side == 0 ? 0xB : me.side == 4 ? 0xC`. `[V]`
    // sfx: Melee_Tick#5
    ask(now.melee_deaths(SIDE_A) != was.melee_deaths(SIDE_A), Request::Slot(0xb));
    // sfx: Melee_Tick#6
    ask(now.melee_deaths(SIDE_B) != was.melee_deaths(SIDE_B), Request::Slot(0xc));

    // **`BattleMan_BurnTick` (`0x0049459A`), a figure's last man dying in
    // fire** — the same ladder as the melee death, on the same two slots:
    //
    // `me.side == 0 ? 0xB : me.side == 4 ? 0xC`. `[V]`
    // sfx: BattleMan_BurnTick#1
    ask(now.burn_deaths(SIDE_A) != was.burn_deaths(SIDE_A), Request::Slot(0xb));
    // sfx: BattleMan_BurnTick#2
    ask(now.burn_deaths(SIDE_B) != was.burn_deaths(SIDE_B), Request::Slot(0xc));

    // **`Missile_Step` (`0x00492C8B`).** A catapult shot counted against a
    // wall, and one that reached a wall four or more high and was not; a shot
    // striking a man, crossbow 10 and bow 8; the same slot again on a
    // casualty, which is always dropped because that buffer started a
    // statement earlier; and `0xD` for the last man. `[V]`
    // sfx: Missile_Step#1
    ask(now.walls_struck() != was.walls_struck(), Request::Slot(0xf));
    // sfx: Missile_Step#2
    ask(now.walls_missed() != was.walls_missed(), Request::Slot(0x10));
    // sfx: Missile_Step#3
    ask(hit(WeaponClass::Crossbow), Request::Slot(10));
    // sfx: Missile_Step#4
    ask(hit(WeaponClass::Bow), Request::Slot(8));
    // sfx: Missile_Step#5
    ask(felled(WeaponClass::Crossbow), Request::Slot(10));
    // sfx: Missile_Step#6
    ask(felled(WeaponClass::Bow), Request::Slot(8));
    // sfx: Missile_Step#7
    ask(now.missile_deaths() != was.missile_deaths(), Request::Slot(0xd));

    // **The shot leaving.** `BattleMan_FireMissile` (`0x00483337`): crossbow 9,
    // bow 7. `BattleMan_StateEngineFire` (`0x004843BC`): the catapult, `0xE`.
    //
    // `[V]` for the slots; `[D]` that our catapult's loose is the same occasion,
    // because ours fires through the shared reload path.
    //
    // sfx: BattleMan_FireMissile#1
    ask(loosed(WeaponClass::Crossbow), Request::Slot(9));
    // sfx: BattleMan_FireMissile#2
    ask(loosed(WeaponClass::Bow), Request::Slot(7));
    // sfx: BattleMan_StateEngineFire#1
    ask(loosed(WeaponClass::Catapult), Request::Slot(0xe));

    // **`Wall_Smash` (`FUN_0049694F`)**, whose first statement is
    // `Sound_PlayFile("bathit2.wav", 0, 0)`. `[V]`
    // sfx: FUN_0049694f#1
    ask(now.walls_smashed() != was.walls_smashed(), Request::File(names::battle::WALL_SMASH));

    // **`FUN_0047A814`, a pot of oil poured** — its last statement,
    // `FUN_004262CF(3)`, `pouroil.wav`. `[V]`
    // sfx: FUN_0047a814#1
    ask(now.oil_poured() != was.oil_poured(), Request::Slot(3));
    // **`FUN_00491492`, a siege tower docking** — `FUN_004262CF(0x11)`,
    // `siegedoc.wav`, the last of the seventeen. `[V]`
    // sfx: FUN_00491492#1
    ask(now.towers_docked() != was.towers_docked(), Request::Slot(0x11));
    // **`FUN_0048551D`, a bridge catching fire** — its first statement,
    // `Sound_PlayFile("dest_ind.wav", 0, 0)`, the one-shot buffer. `[V]`
    // sfx: FUN_0048551d#1
    ask(now.bridges_fired() != was.bridges_fired(), Request::File(names::battle::BRIDGE_FIRE));

    out
}

/// **`Sound_PlayTroopCry` (`0x00499CB1`)'s body, and `g_troopCryCounter`
/// (`0x0053EF60`) with it.**
///
/// `[V]`, and **no random number anywhere in it**: the take is a round robin
/// per (troop, class), stepped *before* it is read, so the first cry of each
/// pair is take **1**, not take 0.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TroopCries {
    counter: [[u8; 4]; 11],
}

impl TroopCries {
    pub fn cry(&mut self, troop: u8, class: u8) -> Option<&'static str> {
        let (t, c) = (troop as usize, class as usize);
        let n = self.counter.get_mut(t)?.get_mut(c)?;
        *n += 1;
        if *n > 3 {
            *n = 0;
        }
        let take = if c == 3 { 0 } else { *n as usize };
        names::troop_cry(t, c, take)
    }
}

