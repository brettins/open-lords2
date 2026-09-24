
use super::*;
use super::blank_field;

fn fight(seed: u64) -> BattleRunner {
    BattleRunner::deploy_armies(
        blank_field(),
        seed,
        Army { troops: &[(Troop::Swordsmen, 6), (Troop::Archers, 4)], owner: 1, human: false },
        Army { troops: &[(Troop::Pikemen, 6), (Troop::Peasants, 4)], owner: 2, human: true },
    )
}

/// `Motion::Dying` is state 2's pose — `BattleMan_StateDead` (`0x004830E9`),
/// `Anim_CollapseA2` (`0x00487CE4`) then `Anim_DyingA2` (`0x00487908`). Only
/// `BattleMan_StateFillMoat` (`0x00483FE1`) plays it on a living man, and there
/// is no moat on a field battlefield.
#[test]
fn no_living_figure_is_drawn_dying_in_a_field_battle() {
    for seed in [0x5EED, 1, 2, 7, 0xC0FFEE] {
        let mut r = fight(seed);
        let mut corpses = 0;
        for tick in 0..12_000u32 {
            r.step();
            for i in 0..r.fighters.len() {
                assert!(
                    !(r.is_alive(i) && r.fighters[i].anim == Motion::Dying),
                    "seed {seed} tick {tick}: figure {i} alive with the corpse pose"
                );
            }
            corpses = r.fighters.iter().filter(|f| f.anim == Motion::Dying).count();
            if r.is_decided() {
                break;
            }
        }
        assert!(corpses > 0, "seed {seed}: nobody died, the invariant proved nothing");
    }
}

/// **The moat filler is the one living man who plays `Anim_Dying`** —
/// `BattleMan_StateFillMoat` (`0x00483FE1`) is its only caller — and he must
/// wear [`Motion::Shovelling`], not the corpse pose.
///
/// **Ablation**: give him `Motion::Dying`, as this did, and two things follow
/// at once. He is drawn in the frames every dead man wears, and `step`'s corpse
/// arm counts `+0x173` out under him, so [`BattleRunner::corpse_gone`] stops
/// the renderer collecting a man who is alive and working.
#[test]
fn a_man_shovelling_earth_into_the_moat_is_alive_and_not_a_corpse() {
    let mut r = fight(0x5EED);
    let sim = r.fighters[0].sim;
    let cell = r.fighters[0].y as usize * DIM + r.fighters[0].x as usize + 1;
    r.field.cells[cell].surface = crate::siege::SURFACE_WATER;
    r.field.cells[cell].terrain = 11;
    r.sim.figures[sim].state = State::FillingMoat;
    r.fighters[0].moat_cell = Some(cell as u32);
    for _ in 0..200 {
        assert!(r.fill_moat_tick(0), "he stopped shovelling");
        assert!(r.is_alive(0));
        assert_eq!(r.fighters[0].anim, Motion::Shovelling);
        assert!(!r.corpse_gone(0), "a living man was cleared away as a body");
    }
    assert!(r.fighters[0].phase <= 0x5F);
}

/// **`Anim_DyingA2` has no knight arm.** `00480000.c:3004-3006` returns for
/// every `troopType` the band misses — of 0 … 6 that is 6 alone — *before*
/// `animPhase += 1` and *before* `facingDrawn = dirc`, so a shovelling knight
/// holds both.
#[test]
fn a_shovelling_knight_holds_his_phase_and_his_drawn_facing() {
    let (mut r, i) = knight();
    r.fighters[i].phase = 7;
    r.fighters[i].facing = 3;
    r.fighters[i].facing_drawn = 5;
    for _ in 0..200 {
        r.shovel(i);
        assert_eq!(r.fighters[i].anim, Motion::Shovelling);
        assert_eq!(r.fighters[i].phase, 7, "the knight's phase was stepped");
        assert_eq!(r.fighters[i].facing_drawn, 5, "the knight's drawn facing moved");
    }
}

fn knight() -> (BattleRunner, usize) {
    let r = BattleRunner::deploy_armies(
        blank_field(),
        0x5EED,
        Army { troops: &[(Troop::Knights, 6)], owner: 1, human: false },
        Army { troops: &[(Troop::Peasants, 6)], owner: 2, human: true },
    );
    let i = r.fighters.iter().position(|f| f.troop == Troop::Knights).unwrap();
    (r, i)
}

/// **The horse is held too.** `Anim_DyingA2` returns at `00480000.c:3004-3006`
/// before `horseFrame` (`+0x11`) is written, and state 9's latched arm
/// (`00480000.c:1697-1725`) calls no other handler — so the byte
/// `Anim_WalkA2`'s knight arm left (`2766`, `|dirc| * 6 + animPhase >> 2`) is
/// what the painter draws for as long as he digs.
///
/// **Ablation**: drop `f.horse_frame = (f.facing % 8) * 6 + (f.phase >> 2)`
/// from [`BattleRunner::march`] and the held byte is 0, so
/// `render.rs`'s `Anim::Shovelling => f.horse_frame` puts a standing horse
/// under a digging rider — the defect this row names.
#[test]
fn a_shovelling_knight_holds_his_horse_mid_stride() {
    let (mut r, i) = knight();
    r.fighters[i].facing = 3;
    r.fighters[i].phase = 0x0E;
    r.march(i, true);
    let held = r.fighters[i].horse_frame;
    assert_eq!(held, 3 * 6 + (0x0F >> 2), "the walk arm did not write `+0x11`");
    assert_ne!(held % 6, 0, "a horse already at pose 0 would prove nothing");
    for _ in 0..200 {
        r.shovel(i);
        assert_eq!(r.fighters[i].horse_frame, held, "the shovel moved the horse");
    }
}

/// BattleMan_StateDead 0x004830E9, 00480000.c:1337: freed on the 81st run; StateEngineDead :1958 on the 121st.
/// Ablation: `>` to `>=` in `state_dead` frees on the 80th / 120th.
#[test]
fn a_body_is_freed_on_its_eighty_first_dead_run_an_engine_on_its_hundred_and_twenty_first() {
    for (troop, runs) in [(Troop::Swordsmen, 81u32), (Troop::Catapults, 121)] {
        let mut r = fight(0x5EED);
        r.fighters[0].troop = troop;
        let sim = r.fighters[0].sim;
        r.sim.figures[sim].men = 0;
        r.sim.figures[sim].state = State::Dead;
        let mut n = 0;
        while r.sim.figures[sim].owner != 0 {
            r.step();
            n += 1;
        }
        assert_eq!(n, runs, "{troop:?}");
        assert!(r.corpse_gone(0));
    }
}

/// Battle_UpdateAllMen 0x004822ED runs state 2 in the man's own slot (00480000.c:899). Melee_Tick 0x00494908
/// kills g_curBattleMan in his own handler (00490000.c:1803/1824): first dead run next frame, freed 81 on.
/// BattleMan_BurnTick runs before his dispatch (00480000.c:882): the death frame is his first run, 80 on.
/// Ablation: count the dead in the end-of-frame sweep and the melee dead are freed 80 frames on.
#[test]
fn a_man_cut_down_is_freed_eighty_one_frames_on_a_man_burnt_eighty() {
    let mut cut = 0;
    for seed in [0x5EED, 1, 2, 7, 0xC0FFEE] {
        let mut r = fight(seed);
        let mut died = vec![None; r.fighters.len()];
        for tick in 0..12_000u32 {
            r.step();
            for i in 0..r.fighters.len() {
                if !r.is_alive(i) && died[i].is_none() {
                    died[i] = Some(tick);
                }
                let Some(d) = died[i] else { continue };
                if d != u32::MAX && r.sim.figures[r.fighters[i].sim].owner == 0 {
                    assert_eq!(tick - d, 81, "seed {seed}: figure {i}");
                    cut += 1;
                    died[i] = Some(u32::MAX);
                }
            }
            if r.is_decided() {
                break;
            }
        }
    }
    assert!(cut > 0, "nobody was freed");

    let mut r = fight(0x5EED);
    let sim = r.fighters[0].sim;
    let cell = r.fighters[0].y as usize * DIM + r.fighters[0].x as usize;
    r.field.cells[cell].surface = fire::SURFACE_BURNING;
    r.sim.figures[sim].men = 1;
    r.sim.figures[sim].hits = 99;
    let mut d = None;
    for tick in 0..400u32 {
        r.step();
        if !r.is_alive(0) && d.is_none() {
            d = Some(tick);
        }
        if r.sim.figures[sim].owner == 0 {
            assert_eq!(tick - d.unwrap(), 80);
            return;
        }
    }
    panic!("the burnt man was never freed (died {d:?})");
}
