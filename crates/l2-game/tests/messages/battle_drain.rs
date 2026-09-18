#![allow(unused_imports)]
use super::*;
use super::dismissal_and_timing::*;
use super::prompts_and_alliances::*;
use super::outcomes_and_obituaries::*;
use super::posting_and_sync::*;
use super::painting_and_layout::*;
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::message::{self, category, Prompt, Record};
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::Game;
use l2_kingdom::victory::Outcome;

/// **A battle swallows its own letters.** `Msg_Pump` (`0x00472E46`) opens with
#[test]
fn a_battle_drains_the_message_ring_one_frame_at_a_time() {
    use l2_game::battlefield::LiveBattle;
    use l2_sim::runner::{Army, BattleRunner};
    use l2_sim::Troop;
    let (mut g, a, mut m) = world();
    let mut layer = vec![0u8; l2_sim::terrain::CELLS];
    layer[36 * l2_sim::terrain::DIM + 40] = 0x04;
    layer[44 * l2_sim::terrain::DIM + 40] = 0x0F;
    let runner = BattleRunner::deploy_armies(
        l2_sim::terrain::build(&layer, 1),
        0x5EED,
        Army { troops: &[(Troop::Peasants, 40)], owner: 2, human: false },
        Army { troops: &[(Troop::Peasants, 40)], owner: 1, human: true },
    );
    let mut live = LiveBattle::new(runner, 0, 0, 0, None, 1, 1);
    live.paused = true;
    g.battle = Some(Box::new(live));
    post(&mut g, notice(130));
    post(&mut g, notice(131));
    m.push(ScreenId::Battlefield);

    let mut frames_up = 0;
    for _ in 0..12 {
        tick(&mut m, &mut g, &a);
        if m.top_id() == Some(ScreenId::Message) {
            frames_up += 1;
        }
    }
    assert_eq!(m.top_id(), Some(ScreenId::Battlefield), "the battle left the stack");
    assert_eq!(frames_up, 2, "both records are drawn once each and never for a second frame");
    assert!(!g.messages.is_open(), "a message is still open with the battlefield on the stack");

    let (mut g, a, mut m) = world();
    post(&mut g, notice(130));
    open_the_scroll(&mut m, &mut g, &a);
    for _ in 0..4 {
        tick(&mut m, &mut g, &a);
    }
    assert_eq!(m.top_id(), Some(ScreenId::Message), "off the battlefield the scroll stays up");
}

