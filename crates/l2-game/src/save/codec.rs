#![allow(unused_imports)]
use super::*;

use l2_kingdom::county::MAX_COUNTIES;
use l2_kingdom::industry::BankruptcyAction;
use l2_kingdom::realm::MAX_REALMS;
use l2_kingdom::report::{Message, SeasonReport};
use l2_kingdom::tables::Tables;
use l2_kingdom::{EventKind, Kingdom, Pass, SEASON_PIPELINE};
use l2_net::canonical::{Canonical, CodecError, Reader};
use crate::game::Game;
use crate::screens::setup::MAP_COUNT;

pub(super) fn encode_prefix(game: &Game, out: &mut Canonical) {
    out.section("player");
    out.u8(game.player);
    out.u32(game.map_slot as u32);
    out.u8(game.selected);
    out.u32(game.turns_played);

    out.section("realms");
    out.u32(MAX_REALMS as u32);
    out.raw(&game.realm_colour);
    // `g_playerNames`, fixed width and no length prefix, because that is what
    // the original's own save block is: six 44-byte records of which 31 bytes
    // are the name. See `Game::player_names`.
    for name in &game.player_names {
        out.raw(name.bytes());
    }
    for gold in &game.gold_last {
        out.i32(*gold);
    }

    out.section("anchors");
    out.u32(MAX_COUNTIES as u32);
    out.raw(&game.anchor_x);
    out.raw(&game.anchor_y);
    // `Event_Post`'s `eventFired = 0` lives on the `Game` now and not in the
    // kingdom (`Game::event_posted`), so it has to be saved here or a reload
    // would re-post a letter the player has already read — the original's own
    // save keeps that clearing, in the county record it is allowed to write to.
    for posted in &game.event_posted {
        out.bool(*posted);
    }

    out.section("report");
    out.option(game.last_report.as_ref(), encode_report);

    // The campaign: `DAT_0053F640`, `DAT_0053F258` and `DAT_0053F0C4`.
    //
    // **The ending messages are no longer here; the whole ring is, below.** They
    // used to be a `Vec<Ending>` of their own with the note that the queue is
    // empty at every point a person can save. That stopped being true the moment
// the messages were *shown*: a person can now
    // save on the campaign map with three obituaries still queued behind the one
    // on screen, and a save that dropped them would be a save that can never be
    // won. `docs/decisions.md` C113.
    out.section("campaign");
    let c = &game.campaign;
    out.u8(match c.track {
        crate::victory::Track::First => 0,
        crate::victory::Track::Second => 1,
    });
    out.u32(c.map as u32);
    out.u8(c.outcome.value());
    out.u8(c.ranking.leader);
    out.u8(c.ranking.trailer);
    out.u8(c.ranking.opponents_remaining);
    out.u8(c.ranking.realms_in_play);

    encode_messages(out, game);
}

/// **The message ring**, `g_messageQueue` and the window over it.
///
/// Written as a flat list of the records still waiting plus the one on screen,
/// the cursors are an implementation
/// of a queue and the queue is what has to survive. Reloading rebuilds the ring
/// from index 0, which is where `Msg_Reset` puts it.
fn encode_messages(out: &mut Canonical, game: &Game) {
    out.section("messages");
    let q = &game.messages;
    let open = q.open().copied();
    out.u8(u8::from(open.is_some()));
    if let Some(r) = open {
        encode_record(out, &r);
        out.u32(q.timer() as u32);
    }
    let waiting = q.waiting();
    out.u32(waiting.len() as u32);
    for r in &waiting {
        encode_record(out, r);
    }
}

fn encode_record(out: &mut Canonical, r: &crate::message::Record) {
    out.u8(r.to);
    out.u8(r.from);
    out.u32(r.group as u32);
    out.u8(r.variant);
    out.u8(r.category);
    out.u8(r.county);
    out.u8(r.spare);
    out.u32(r.payload as u32);
}

fn decode_record(input: &mut Reader<'_>) -> Result<crate::message::Record, LoadError> {
    Ok(crate::message::Record {
        to: input.u8()?,
        from: input.u8()?,
        group: input.u32()? as u16,
        variant: input.u8()?,
        category: input.u8()?,
        county: input.u8()?,
        spare: input.u8()?,
        payload: input.u32()? as i32,
    })
}

fn decode_messages(input: &mut Reader<'_>) -> Result<crate::message::MessageQueue, LoadError> {
    let mut q = crate::message::MessageQueue::new();
    if input.u8()? != 0 {
        let r = decode_record(input)?;
        let timer = input.u32()? as i32;
        q.reopen(r, timer);
    }
    let count = input.u32()? as usize;
    if count > crate::message::RING {
        return Err(bad_count(input, count, "queued messages"));
    }
    for _ in 0..count {
        let r = decode_record(input)?;
        // Straight into the ring: the peer filter already ran when the record
        // was first enqueued, and re-running it against a game loaded by a
        // different local player would silently drop messages the file holds.
        q.restore(r);
    }
    Ok(q)
}

/// The campaign section, read back. Every field is range-checked, because a
/// campaign counter past the table is an out-of-bounds map lookup.
fn decode_campaign(input: &mut Reader<'_>) -> Result<crate::victory::Campaign, LoadError> {
    use crate::victory::{Campaign, Track, CAMPAIGN_LENGTH};
    use l2_kingdom::victory::Outcome;

    let track = match input.u8()? {
        0 => Track::First,
        1 => Track::Second,
        other => return Err(bad_count(input, other as usize, "campaign track")),
    };
    let map = input.u32()? as usize;
    if map > CAMPAIGN_LENGTH {
        return Err(bad_count(input, map, "campaign map"));
    }
    let outcome_byte = input.u8()?;
    let outcome = Outcome::from_value(outcome_byte)
        .ok_or_else(|| bad_count(input, outcome_byte as usize, "outcome"))?;
    let ranking = l2_kingdom::victory::Ranking {
        leader: input.u8()?,
        trailer: input.u8()?,
        opponents_remaining: input.u8()?,
        realms_in_play: input.u8()?,
    };
    Ok(Campaign { track, map, outcome, ranking })
}

pub(super) fn decode_prefix(input: &mut Reader<'_>, kingdom: Kingdom) -> Result<Game, LoadError> {
    let player = input.u8()?;
    if player as usize >= MAX_REALMS {
        return Err(LoadError::Player(player));
    }
    let map_slot = input.u32()?;
    if map_slot as usize >= MAP_COUNT {
        return Err(LoadError::MapSlot(map_slot));
    }
    let selected = input.u8()?;
    if selected != 0 && selected as usize > kingdom.county_count {
        return Err(LoadError::Selected(selected));
    }
    let turns_played = input.u32()?;

    let realms = input.u32()? as usize;
    if realms != MAX_REALMS {
        return Err(bad_count(input, realms, "realm count"));
    }
    let mut realm_colour = [0u8; MAX_REALMS];
    realm_colour.copy_from_slice(input.raw(MAX_REALMS)?);
    let mut player_names = [crate::text::PlayerName::EMPTY; MAX_REALMS];
    for slot in player_names.iter_mut() {
        let mut bytes = [0u8; crate::text::PLAYER_NAME_LEN];
        bytes.copy_from_slice(input.raw(crate::text::PLAYER_NAME_LEN)?);
        *slot = crate::text::PlayerName::from_bytes(bytes);
    }
    let mut gold_last = [0i32; MAX_REALMS];
    for slot in gold_last.iter_mut() {
        *slot = input.i32()?;
    }

    let counties = input.u32()? as usize;
    if counties != MAX_COUNTIES {
        return Err(bad_count(input, counties, "county count"));
    }
    let mut anchor_x = [0u8; MAX_COUNTIES];
    anchor_x.copy_from_slice(input.raw(MAX_COUNTIES)?);
    let mut anchor_y = [0u8; MAX_COUNTIES];
    anchor_y.copy_from_slice(input.raw(MAX_COUNTIES)?);
    let mut event_posted = [false; MAX_COUNTIES];
    for slot in event_posted.iter_mut() {
        *slot = input.bool()?;
    }

    let last_report = match input.option(decode_report)? {
        None => None,
        Some(report) => Some(report?),
    };
    let campaign = decode_campaign(input)?;
    let messages = decode_messages(input)?;

    Ok(Game {
        kingdom,
        messages,
        // `g_multiplayer` — session, not world. A save carries no session.
        multiplayer: false,
        player,
        // **A save of ours is between turns, always** — and that is a limit of
        // ours, not a fact about the original.
        //
        // This comment used to read *"The original saves from the campaign map
        // and nowhere else"*, which is **false**. `Screen_FrameInput`'s `0x29`
        // arm opens with `Menu_OpenDropdown(&g_menuBarItems, 3)` and
        // `Menu_SaveGame` (`0x00433F49`) does not test `g_battlePhase`, so the
        // original saves from the **battlefield** as readily as from the map —
        // its `.sav` is a memory dump and the battle goes into it with
        // everything else. `Screen_FrameInput` even *exempts* screens `0x35` and
        // `0x36` from its multiplayer force-close while `g_battlePhase != 0`,
        // which is the save box being deliberately kept alive in a battle.
        // **[V]**
        //
        // Ours cannot, and the cost of making it: `crate::battlefield::LiveBattle`
        // wraps an `l2_sim::runner::BattleRunner`, and that graph —
        // `Battle`, `Battlefield`, `Vec<Fighter>`, `Units`, `Ai`, `AiField`,
        // `Missiles`, `SiegeState` — is **196 fields over 16 structs, 21 of
        // them private to `l2-sim`**, against 242 stored fields for the whole
        // kingdom. Measured; none of it is encoded here. So the
// save screen **refuses** while a battle is live
        // file that silently loses it — `crate::screens::saveload`, and
        // `docs/arms.json`'s `ours/save-refuses-mid-battle`.
        //
        // The two fields either side of `battle` keep the original argument: a
        // loaded game has no half-run turn and no half-made levy.
        //
        // The levy is the clearest case of the three: `g_levyBasket` is scratch
        // that `Army_Create` spends and abandons
        // it — the realm's weapon stocks it was seeded from — is in the kingdom
        // already, encoded by `l2_kingdom::save` and hashed into the lockstep
        // digest with everything else.
        field_policy: crate::engagement::Answer::Decline,
        turn: None,
        levy: crate::game::LevyOrder::default(),
        battle: None,
        // The same argument again, and a fifth and sixth field it covers: a
        // move order asked for from the information panel cannot outlive the
        // frame that asked
        // presentation. A loaded game opens at the near zoom because
        // `MapScreen::new` does, and this is that projection's starting value.
        begin_move_order: None,
        combine_ask: None,
        map_zoom_far: false,
        // And a seventh: `Setup_StartGame` sets `DAT_005440C8 = g_optTimeLimit`
        // for a loaded game as for a new one, so the turn timer starts from the
        // full limit on its first tick. See `crate::turn_clock`.
        turn_clock: crate::turn_clock::TurnClock::default(),
        // And an eighth: the frame each unit's tick handler last wrote is one
        // tick of presentation
        unit_frames: crate::game::UnitFrames::default(),
        map_slot: map_slot as usize,
        realm_colour,
        player_names,
        selected,
        event_posted,
        anchor_x,
        anchor_y,
        gold_last,
        last_report,
        turns_played,
        campaign,
// **Deliberately not in the file.**
        // `Prefs` is what this machine is like — sound, animations, scroll
        // speed — and the presentation quirks are what this reader wants to
        // look at. Neither is a property of the *game*: recording them in a
        // save would put one person's preferences into a world another person
        // then loads, and `VERSION` would move every time somebody added a
        // volume control. The behavioural quirks, which really are the world's,
        // are in `l2_kingdom::save` where they belong.
        prefs: crate::game::Prefs::default(),
        presentation_quirks: crate::game::Quirks::default(),
        // Per run, not per game, and so not in the file: `screens::saveload`
        // puts the running session's tips back over this. `crate::tip`.
        tips: crate::tip::Tips::new(),
        // Which capture and battle film come next. Presentation, and not in
        // any of the original's save blocks either; a loaded game starts both
        // rotations over, as the original's start-up does.
        films: crate::movie::Reel::default(),
        // And a ninth and tenth: which page of the standings somebody had
        // open
        // in none of the original's save blocks either — `Game_NewGame` is its
        // only writer outside the page itself.
        nobles_category: 0,
        nobles_spoken: 0,
        // And an eleventh: the line a screen last asked the narrator for.
        spoken: (0, ""),
    })
}

fn bad_count(input: &Reader<'_>, found: usize, expected: &'static str) -> LoadError {
    LoadError::Malformed(CodecError::BadTag {
        tag: found.min(255) as u8,
        expected,
        at: input.position(),
    })
}

// ---------------------------------------------------------------------------
// The season report
// ---------------------------------------------------------------------------
//
// This is the only part of the file that is a **tagged union**, and every tag
// is decoded through a total function that can fail: an unknown pass index, an
// unknown message tag, an event id the deck does not have and an unknown
// bankruptcy action are four separate refusals.
//
// The tags are written here on purpose. A wire tag
// is a promise about a *file*, and `l2-kingdom` has no files; giving `Message`
// a tag byte there would put a save-format constant in the crate that is
// supposed to be able to change its enums freely. The cost is that adding a
// variant breaks the two matches below at compile time, which is the behaviour
// we want.

/// `Pass` goes out as its position in `SEASON_PIPELINE`
/// its own: the pipeline is the ordering every test already compares against,
/// and `Pass::Industry(Commodity)` is four of its entries, so this needs no
/// second enum for the commodity.
fn encode_report(out: &mut Canonical, report: &SeasonReport) {
    out.seq(&report.passes, |o, pass| o.u32(pass.order() as u32));
    out.seq(&report.messages, encode_message);
    out.bytes(&report.revolts);
}

fn decode_report(input: &mut Reader<'_>) -> Result<Result<SeasonReport, LoadError>, CodecError> {
    // `Reader::seq` hands the element decoder a `CodecError` channel and
    // nothing wider, so the richer refusals — an unknown pass, an unknown
    // event — come back through the *value* as an inner `Result` and are
// unwrapped here.
    let passes = input.seq(|r| {
        let index = r.u32()?;
        Ok(SEASON_PIPELINE.get(index as usize).copied().ok_or(LoadError::BadPass(index)))
    })?;
    let mut collected: Vec<Pass> = Vec::with_capacity(passes.len());
    for pass in passes {
        match pass {
            Ok(p) => collected.push(p),
            Err(e) => return Ok(Err(e)),
        }
    }
    let messages = input.seq(decode_message)?;
    let mut kept: Vec<Message> = Vec::with_capacity(messages.len());
    for message in messages {
        match message {
            Ok(m) => kept.push(m),
            Err(e) => return Ok(Err(e)),
        }
    }
    let revolts = input.bytes()?.to_vec();
    Ok(Ok(SeasonReport { passes: collected, messages: kept, revolts }))
}

fn encode_message(out: &mut Canonical, message: &Message) {
    match *message {
        Message::UnrestWarning { county } => {
            out.u8(1);
            out.u8(county);
        }
        Message::UnrestRising { county, level } => {
            out.u8(2);
            out.u8(county);
            out.u8(level);
        }
        Message::Revolt { county } => {
            out.u8(3);
            out.u8(county);
        }
        Message::Bankrupt { realm, stage, action } => {
            out.u8(4);
            out.u8(realm);
            out.u8(stage);
            out.u8(bankruptcy_tag(action));
        }
        Message::Event { county, kind } => {
            out.u8(5);
            out.u8(county);
            out.u16(kind.id());
        }
        Message::CastleBuilt { county, castle_type } => {
            out.u8(6);
            out.u8(county);
            out.u8(castle_type);
        }
        Message::ArmyStarving { realm, unit, county, stage } => {
            out.u8(7);
            out.u8(realm);
            out.u32(unit as u32);
            out.u8(county);
            out.i32(stage);
        }
        Message::CountySeceded { realm, county } => {
            out.u8(8);
            out.u8(realm);
            out.u8(county);
        }
        Message::LandsDivide { realm, counties } => {
            out.u8(9);
            out.u8(realm);
            out.u8(counties);
        }
        // `Weather_UpdateAll`'s two, `L2.eng` groups 143 and 144.
        Message::Drought { county } => {
            out.u8(10);
            out.u8(county);
        }
        Message::Flooding { county } => {
            out.u8(11);
            out.u8(county);
        }
    }
}

fn decode_message(input: &mut Reader<'_>) -> Result<Result<Message, LoadError>, CodecError> {
    let tag = input.u8()?;
    Ok(match tag {
        1 => Ok(Message::UnrestWarning { county: input.u8()? }),
        2 => Ok(Message::UnrestRising { county: input.u8()?, level: input.u8()? }),
        3 => Ok(Message::Revolt { county: input.u8()? }),
        10 => Ok(Message::Drought { county: input.u8()? }),
        11 => Ok(Message::Flooding { county: input.u8()? }),
        4 => {
            let realm = input.u8()?;
            let stage = input.u8()?;
            let action = input.u8()?;
            match bankruptcy_from_tag(action) {
                Some(action) => Ok(Message::Bankrupt { realm, stage, action }),
                None => Err(LoadError::BadBankruptcy(action)),
            }
        }
        5 => {
            let county = input.u8()?;
            let id = input.u16()?;
            match EventKind::from_id(id) {
                Some(kind) => Ok(Message::Event { county, kind }),
                None => Err(LoadError::BadEvent(id)),
            }
        }
        6 => Ok(Message::CastleBuilt { county: input.u8()?, castle_type: input.u8()? }),
        7 => Ok(Message::ArmyStarving {
            realm: input.u8()?,
            unit: input.u32()? as usize,
            county: input.u8()?,
            stage: input.i32()?,
        }),
        8 => Ok(Message::CountySeceded { realm: input.u8()?, county: input.u8()? }),
        9 => Ok(Message::LandsDivide { realm: input.u8()?, counties: input.u8()? }),
        other => Err(LoadError::BadMessage(other)),
    })
}

/// Exhaustive both ways, so adding a stage to the escalation is a compile
/// error here.
fn bankruptcy_tag(action: BankruptcyAction) -> u8 {
    match action {
        BankruptcyAction::None => 0,
        BankruptcyAction::MercenariesDesert => 1,
        BankruptcyAction::Warned => 2,
        BankruptcyAction::Desertion => 3,
        BankruptcyAction::LastWarning => 4,
        BankruptcyAction::Mutiny => 5,
    }
}

fn bankruptcy_from_tag(tag: u8) -> Option<BankruptcyAction> {
    Some(match tag {
        0 => BankruptcyAction::None,
        1 => BankruptcyAction::MercenariesDesert,
        2 => BankruptcyAction::Warned,
        3 => BankruptcyAction::Desertion,
        4 => BankruptcyAction::LastWarning,
        5 => BankruptcyAction::Mutiny,
        _ => return None,
    })
}

