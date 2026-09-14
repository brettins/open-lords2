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
/// timer.
///
/// # The draw is the behaviour, so the trigger is a countdown
///
/// `Msg_DrawWindow` is 10,915 bytes: it dismisses,
/// enqueues, sets its own timer and plays its own sound, all from inside the
/// draw. Every one of its
/// sixteen `Msg_PlayVoice` calls is guarded by `g_messageTimer == <constant>`,
/// and `g_messageTimer` counts **down** from [`crate::message::TIMER_START`]
/// (2000), one per tick. `[V]`.
///
/// | category | ticks after opening | constant |
/// |---|---:|---|
/// | `0x02`, `0x03`, `0x05`…`0x09`, `0x0F`, `0x10`, `0x11`, `0x12` | 10 | `0x7C6` |
/// | `0x00` notice, `0x0D` capture (unanimated) | 90 | `0x776` |
/// | `0x0E` ending (unanimated) | 100 | `0x76C` |
/// | `0x01` letter, `0x0A` pay prompt, `0x0B` alliance prompt | 200 | `0x708` |
/// | `0x04` tip | 10 | `0x5A`, against a timer clamped to 100 |
/// | `0x13` help | — | silent |
///
/// **The five constants are one rule and a delay.** `0x7C6` is 1990 against a
/// start of 2000 and `0x5A` is 90 against the tip's clamped 100: *both are ten
/// ticks after the window opened, which the tip needs a constant for.
/// its own. The three larger delays are the
/// categories that play a **fanfare** on the opening frame — the voice waits
/// for the trumpet instead of talking over it.
/// is the one that also plays a lord's sting at 90.
///
/// `docs/audio-triggers.md` has the enumeration this came out of. The five
/// values were already on file — `crates/l2-game/src/screens/message.rs` lists
/// them and says they are *"recorded in `crate::message`"*, where they had
/// never been written — but **which category takes which** was not, and that is
/// the half a caller needs.
///
/// # Categories `0x0C` and `0x14` are absent on purpose
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
        // The tip's timer is clamped to `TIP_TIMER` on the frame it opens, so
        // ten ticks in is 90. Same rule, different start.
        c::TIP => 0x5A,
        c::COUNTY_PORTRAIT
        | c::COUNTY_NOTICE
        | c::EVENT
        | c::COUNTY_TALL
        | c::GARRISON_PROMPT
        | c::CASTLE => 0x7C6,
        n if (c::PARAGRAPHS_FIRST..=c::PARAGRAPHS_LAST).contains(&n) => 0x7C6,
        // `HELP`.
        _ => return None,
    })
}

/// **The fanfare a message window opens with**, on the frame the timer is still
/// [`crate::message::TIMER_START`] — `Msg_DrawWindow`'s five `Sound_PlayFile`
/// calls, which is all of them. `[V]`.
///
/// `ff_capt.wav` is the conquest band and the guard is on the *group*.
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

/// One call the original makes from inside the battlefield's per-man state
/// machine, in the form the audio layer can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// `Sound_PlaySlot(n)`, or its 28-byte thunk `FUN_004262CF(n)` — a
    /// **1-based** slot of [`names::BATTLE_BANK`], dropped if that buffer is
    /// still sounding. [`Audio::play_effect_if_idle`].
    Slot(usize),
    /// `Sound_PlayFile(name, 0, 0)` — the one-shot buffer, dropped if it is
    /// still sounding. [`Audio::play_file`].
    File(&'static str),
}

/// **The battlefield's sounding call sites, as a function of what happened.**
///
/// `was` and `now` are one battle's [`l2_sim::Cues`] at two ticks; the answer
/// is every call the original would have made in between, **once per kind**.
/// Every call below is drop-if-busy.
/// its own buffer.
/// dropped by the original too. `l2-sim`'s `crate::cue` has the argument.
///
/// Each arm is the original's branch, beside the id of the site it reproduces.
/// Which *event* each counter records is decided where it is written, in
/// `l2-sim`; which *slot* that event plays is decided here, because it is the
/// original's ladder and not a rule of the battle.
///
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
    // **`Melee_Tick`, the last man of a figure**, by the dying figure's side:
    // `me.side == 0 ? 0xB : me.side == 4 ? 0xC`. `[V]`
    // sfx: Melee_Tick#5
    ask(now.melee_deaths(SIDE_A) != was.melee_deaths(SIDE_A), Request::Slot(0xb));
    // sfx: Melee_Tick#6
    ask(now.melee_deaths(SIDE_B) != was.melee_deaths(SIDE_B), Request::Slot(0xc));

    // **`BattleMan_BurnTick` (`0x0049459A`), a figure's last man dying in
    // fire** — the same ladder as the melee death, on the same two slots:
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
    // `[V]` for the slots; `[D]` that our catapult's loose is the same occasion,
    // because ours fires through the shared reload path.
    // engine's own 100-of-180 cadence.
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
/// ```c
/// counter[unit][class] += 1;
/// if (3 < counter[unit][class]) counter[unit][class] = 0;
/// take = counter[unit][class];
/// if (class == 3) take = 0;
/// Sound_PlayFile(g_troopSounds + class*0x40 + take*0x10 + unit*0x100, 1, 0);
/// ```
///
/// `[V]`, and **no random number anywhere in it**: the take is a round robin
/// per (troop, class), stepped *before* it is read, so the first cry of each
/// pair is take **1**, not take 0.
/// in `.bss` and this function is its only writer — an exhaustive reference
/// search — so it starts at zero with the process and is never reset between
/// battles. Ours lives on the [`Director`], which lives as long as the process.
///
/// That settles the determinism question the brief raised before it could
/// arise: **a cry draws on no generator at all**, so there is nothing to keep
/// away from the simulation's `Pcg32`. If a future site does need presentation
/// randomness, it needs a generator of its own on the audio side, never the
/// battle's.
///
/// The counter steps **even when the cry is dropped**, because the drop is
/// inside `Sound_PlayFile`. So two orders in quick succession sound take 1 and
/// then, when the next is heard, take 3 — the dropped take 2 was spent.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TroopCries {
    /// `[troop][class]`, 0…3.
    counter: [[u8; 4]; 11],
}

impl TroopCries {
    /// Step the counter for one cry and name the file it plays — `None` for a
    /// cell holding `null.wav` (a siege engine told anything but to move) and
    /// for anything out of range. The counter steps in the `None` case too, as
    /// the original's does.
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

