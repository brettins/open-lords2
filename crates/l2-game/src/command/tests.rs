use super::*;
use l2_kingdom::divide::{SplitBasket, SplitInto};
use l2_kingdom::field::FieldType;
use l2_kingdom::industry::MapToggle;
use l2_kingdom::levy::LevyBasket;
use l2_kingdom::siege::Engine;
use l2_kingdom::tables::Commodity;
use l2_kingdom::trade::Good;
use l2_kingdom::unit::{Mercenaries, TroopType};
use l2_net::{decode_all, Canonical, CodecError, Quirk};

fn samples() -> Vec<GameCommand> {
    let mut levy = LevyBasket::default();
    levy.slots[3].chosen = 40;
    levy.slots[7].available = -2;
    let mut split = SplitBasket::default();
    split.parent[6] = 12;
    split.daughter[0] = 300;
    split.mercenaries = Some(Mercenaries { band: 9, troop: TroopType::Knight, men: 200 });
    vec![
        OrderCastle { county: 7, castle: 3 }.into(),
        CombineArmies { occupant: 70_000, mover: 2 }.into(),
        ValidateSiegeLink { unit: 5 }.into(),
        Trade { qty: -40, good: Good::Mail, buy_price: 9, sell_price: 6, county: 12 }.into(),
        PayForHelp {}.into(),
        FormAlliance { offerer: 4 }.into(),
        OrderEngine { unit: 3, engine: Engine::BatteringRam, delta: -1 }.into(),
        LiftSiege { unit: 8 }.into(),
        SendSupplies { from: 2, to: 30, grain: 1000, cattle: -1 }.into(),
        ToggleOption { option: GameOption::FightHumansOnly }.into(),
        SetQuirk { quirk: *Quirk::ALL.last().unwrap(), reproduced: true }.into(),
        RankRealms {}.into(),
        SetTaxRate { county: 1, rate: 5 }.into(),
        SetRation { county: 2, level: 4 }.into(),
        SetRationSplit { county: 3, split: 77, sweep: true }.into(),
        RaiseArmy { county: 4, basket: levy, happiness_cost: -3, hire: Some(11) }.into(),
        SplitArmy { army: 9, basket: split, into: SplitInto::Castle { county: 6, tile: (1, 250) } }
            .into(),
        LeaveCastle { unit: 10 }.into(),
        DisbandArmy { army: 11 }.into(),
        MoveUnit { unit: 12, dest: (64, 3) }.into(),
        MoveLabour { county: 1, from: 2, to: 9, icons: -4 }.into(),
        BalanceLabour { county: 1, cluster: 6, fill: false }.into(),
        BalanceAllLabour { county: 33 }.into(),
        PaintField { county: 5, tile: 9000, kind: FieldType::Reclaiming }.into(),
        ToggleIndustry { county: 5, what: MapToggle::Industry(Commodity::Stone) }.into(),
        EndTurn {}.into(),
        AnswerBattle { answer: Answer::Decline }.into(),
        TakeTheField {}.into(),
        FinishBattle {}.into(),
        DismissReport {}.into(),
    ]
}

#[test]
fn every_variant_round_trips_byte_identically() {
    let all = samples();
    for (i, cmd) in all.iter().enumerate() {
        assert_eq!(cmd.tag() as usize, i, "one sample per tag, in tag order");
        let bytes = Canonical::bytes_of(cmd);
        let back: GameCommand = decode_all(&bytes).expect("decodes");
        assert_eq!(&back, cmd);
        assert_eq!(Canonical::bytes_of(&back), bytes);
    }
    let past = [all.len() as u8];
    assert!(matches!(decode_all::<GameCommand>(&past), Err(CodecError::BadTag { .. })));
}

#[test]
fn every_value_of_every_enum_field_round_trips() {
    let mut all: Vec<GameCommand> = Vec::new();
    all.extend(GAME_OPTIONS.map(|option| ToggleOption { option }.into()));
    all.extend(Quirk::ALL.iter().map(|&quirk| SetQuirk { quirk, reproduced: false }.into()));
    all.extend(l2_kingdom::trade::ALL_GOODS.map(|good| {
        Trade { qty: 1, good, buy_price: 0, sell_price: 0, county: 1 }.into()
    }));
    all.extend(l2_kingdom::siege::ENGINES.map(|engine| {
        OrderEngine { unit: 1, engine, delta: 1 }.into()
    }));
    all.push(ToggleIndustry { county: 1, what: MapToggle::Castle }.into());
    all.push(AnswerBattle { answer: Answer::TakeTheField }.into());
    all.push(SplitArmy { army: 1, basket: SplitBasket::default(), into: SplitInto::Field }.into());
    for cmd in all {
        let back: GameCommand = decode_all(&Canonical::bytes_of(&cmd)).expect("decodes");
        assert_eq!(back, cmd);
    }
}

#[test]
fn a_truncated_command_is_refused() {
    let bytes = Canonical::bytes_of(&GameCommand::from(SetTaxRate { county: 1, rate: 5 }));
    assert!(decode_all::<GameCommand>(&bytes[..bytes.len() - 1]).is_err());
}

fn owned_county() -> Game {
    let mut g = Game::new(7);
    g.kingdom.set_county_count(2);
    g.kingdom.counties[1].owner = 1;
    g.kingdom.counties[1].tax_rate = 1;
    g
}

#[test]
fn apply_from_the_local_slot_is_issue() {
    let (mut a, mut b) = (owned_county(), owned_county());
    let order = SetTaxRate { county: 1, rate: 4 };
    assert!(a.issue(order.clone()));
    b.apply(local(), &order.into());
    assert_eq!(a.kingdom.counties[1].tax_rate, 4);
    assert_eq!(b.kingdom.counties[1].tax_rate, 4);
    assert_eq!(b.player, 1);
}

#[test]
fn a_slot_without_a_seat_changes_nothing() {
    let mut g = owned_county();
    g.apply(PlayerSlot::new(1), &SetTaxRate { county: 1, rate: 4 }.into());
    assert_eq!(g.kingdom.counties[1].tax_rate, 1);
}

#[test]
fn rank_realms_is_the_courts_recount() {
    let mut g = owned_county();
    g.kingdom.realms[1].gold = 5000;
    let t = g.kingdom.tables;
    let want = g.kingdom.realms[1].compute_score(&t);
    assert_ne!(want, g.kingdom.realms[1].score);
    crate::screens::nobles::recount(&mut g);
    assert_eq!(g.kingdom.realms[1].score, want);
}

/// Sim-state mutation a screen must not do itself. `setup/` is the lobby.
const FORBIDDEN: &[&str] = &[
    "&mut ctx.game.kingdom",
    "&mut game.kingdom",
    "ctx.game.set_tax_rate(",
    "ctx.game.set_ration(",
    "ctx.game.set_ration_split(",
    "ctx.game.raise_army(",
    "ctx.game.split_army(",
    "ctx.game.leave_castle(",
    "ctx.game.disband_army(",
    "ctx.game.order_unit_move(",
    "ctx.game.move_labour(",
    "ctx.game.balance_labour(",
    "ctx.game.balance_all_labour(",
    "ctx.game.rank_realms(",
    "kingdom.paint_field(",
    "kingdom.toggle_industry(",
    "kingdom.toggle_army_foraging(",
    "trade::trade(",
    "form_alliance(",
    "pay_for_help(",
    "order_engine(",
    "break_siege(",
    "validate_link(",
    "supply::spawn(",
    "unit::combine(",
    "ai::rank_realms(",
    "order_castle(",
    "stamp_castle_terrain(",
    "quirks.set_reproduced(",
    "turn::end_players_turn(",
    "turn::begin_turn(",
    "turn::end_turn(",
    "turn::answer_battle(",
    "turn::take_the_field(",
    "turn::finish_battle(",
    "turn::dismiss_report(",
];

fn assigns_kingdom(line: &str) -> bool {
    let Some(at) = line.find("ctx.game.kingdom.") else { return false };
    let rest = &line[at..];
    let end = rest.find(';').unwrap_or(rest.len());
    let stmt = &rest[..end];
    [" = ", " += ", " -= ", " |= ", " &= "].iter().any(|op| stmt.contains(op))
        && !line.trim_start().starts_with("let ")
}

fn scan(dir: &std::path::Path, out: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).expect("screens dir") {
        let path = entry.expect("entry").path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n != "setup") {
                scan(&path, out);
            }
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("source");
        for (n, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if FORBIDDEN.iter().any(|p| code.contains(p)) || assigns_kingdom(code) {
                out.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
            }
        }
    }
}

#[test]
fn screens_change_the_world_only_through_commands() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/screens");
    let mut hits = Vec::new();
    scan(&dir, &mut hits);
    assert!(hits.is_empty(), "issue a crate::command order instead:\n{}", hits.join("\n"));
}
