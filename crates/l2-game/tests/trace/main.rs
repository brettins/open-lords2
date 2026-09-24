//! Our side of the trace differ: a scenario played headless, every `l2_net::trace!` site
//! written as `<scenario>.ours.jsonl`. C246; `tools/oracle/scenarios/FORMAT.md`.

mod harness;
mod script;
mod shot;
mod sink;
mod steps;
mod symbols;

use std::path::PathBuf;

use l2_game::audio::{Audio, Director};
use l2_game::game::Assets;
use l2_game::input::Event;
use l2_game::screen::{Ctx, Machine, ScreenId};
use l2_game::screens::setup::SetupPage;
use l2_game::Game;
use l2_net::trace;
use l2_view::Canvas;
use script::Step;

pub struct Run {
    game: Game,
    assets: Assets,
    audio: Audio,
    director: Director,
    machine: Machine,
    canvas: Canvas,
    tables: l2_kingdom::tables::Tables,
    frame: u64,
    /// Snap and `record` frames: the draw and text records FORMAT.md keeps (trace-run.ps1 KeepDraws).
    kept: Vec<u64>,
    shots: Option<shot::Shots>,
}

pub fn scenarios_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/oracle/scenarios")
}

impl Run {
    /// `main.rs`'s launch: the world behind the title page, the title page, the intro over it.
    pub fn launch() -> Option<Run> {
        let dir = l2_testkit::install_dir()?;
        let platform = l2_mods::Platform::builder().base(&dir).build().expect("the install mounts");
        let tables = platform.kingdom_tables().expect("tables");
        let assets = Assets::load(&platform.vfs).expect("assets load");
        let game = new_game(&assets, tables.clone());
        let mut machine = Machine::new(ScreenId::Setup(SetupPage::Title));
        l2_game::movie::start_up(&mut machine);
        Some(Run {
            game,
            audio: Audio::headless(&platform.vfs),
            assets,
            director: Director::new(),
            machine,
            canvas: Canvas::screen(),
            tables,
            frame: 0,
            kept: Vec::new(),
            shots: None,
        })
    }

    fn send(&mut self, e: Event) {
        let mut ctx = Ctx { game: &mut self.game, assets: &self.assets };
        self.machine.handle(e, &mut ctx);
    }

    /// One `App::tick` and, when the machine says so, one `App::redraw`.
    fn frame(&mut self) {
        self.frame += 1;
        trace::set_frame(self.frame);
        let mut ctx = Ctx { game: &mut self.game, assets: &self.assets };
        self.machine.update(&mut ctx);
        self.director.listen(&mut self.audio, &self.machine, &self.game);
        harness::end_one_shots(&mut self.audio);
        if self.machine.take_dirty() {
            let ctx = Ctx { game: &mut self.game, assets: &self.assets };
            self.machine.draw(&ctx, &mut self.canvas);
        }
    }

    fn screen(&self) -> i64 {
        self.machine.traced_screen_byte(&self.game).map_or(-1, i64::from)
    }

    fn page(&self) -> i64 {
        match self.machine.top_id() {
            Some(ScreenId::Setup(p)) => i64::from(p.number()),
            _ => -1,
        }
    }

    /// A wait that runs out writes a `failed` record and ends the scenario, as trace-run.ps1 does.
    fn until(&mut self, max: u64, what: &str, done: impl Fn(&Run) -> bool) -> bool {
        for _ in 0..max {
            if done(self) {
                return true;
            }
            self.frame();
        }
        let step = format!("{what} {:#x} {:?}", self.screen(), self.machine.ids());
        l2_net::trace!("failed", 0, "Scenario", step = step);
        false
    }

    /// trace-run.ps1 `$NewGameSteps` + `$NewGameTail`, the same clicks.
    fn new_game_steps(new: bool) -> Vec<Step> {
        let max = script::MAX_FRAMES;
        vec![
            Step::WaitPage(1, max), Step::WaitIdle(5, max), Step::Click(320, 103, false, false),
            Step::WaitPage(2, max), Step::WaitIdle(5, max), Step::Click(320, 103, false, false),
            Step::WaitPage(5, max), Step::Advance(20),
            Step::Click(if new { 448 } else { 192 }, 129, false, false),
            Step::WaitPage(4, max), Step::Advance(20), Step::Click(432, 227, false, false),
            Step::WaitScreenNot(0x1F, max), Step::Advance(300),
        ]
    }

    pub fn play(&mut self, steps: &[Step]) -> bool {
        for step in steps {
            let ok = match step {
                Step::NewGame(new) => self.play(&Run::new_game_steps(*new)),
                Step::Films(play) => {
                    if !play {
                        self.assets.films = l2_game::movie::FilmFiles::default();
                    }
                    true
                }
                Step::WaitPage(p, max) => self.until(*max, "page", |r| r.page() == *p),
                Step::WaitScreenNot(id, max) => self.until(*max, "screen-not", |r| r.screen() != *id),
                Step::Peek => true,
                Step::Ok => self.ok(),
                Step::Move(..) | Step::Drag(..) | Step::Record(_) => self.input(step),
                Step::WaitEvent(event, max) => self.wait_event(event, *max),
                Step::Dismiss => {
                    self.dismiss();
                    true
                }
                _ => {
                    self.step(step);
                    true
                }
            };
            if !ok {
                return false;
            }
        }
        true
    }

    fn step(&mut self, step: &Step) -> bool {
        {
            match step {
                Step::Load(name) => {
                    let save = l2_testkit::fixture_save(name).expect("the save loads");
                    self.game = l2_game::scenario::from_save(&save, self.tables.clone())
                        .expect("the save plays");
                    // Save_Read 0x004ADFE6 -> FUN_004976A1 00490000.c:3114-3118 aiStep = 1 [V].
                    l2_game::save::resume(&mut self.game);
                    self.machine = Machine::new(ScreenId::Campaign);
                    // SaveLoadScreen::landing: FUN_004976A1 0x004976A1, g_battlePhase 2 -> 0x29.
                    if self.game.battle.is_some() {
                        self.machine.push(ScreenId::Battlefield);
                    }
                    // trace-run.ps1 `$LoadSteps` ends `wait-screen-not 0x1F`, `advance 60`; its
                    // first frame is Save_Read's own (raw endturn-england: advance at f196, ret f197).
                    self.frames(59);
                }
                Step::Click(_, _, _, true) => {}
                Step::Click(x, y, right, false) => {
                    if *right {
                        self.rclick(*x, *y);
                    } else {
                        self.click(*x, *y);
                    }
                }
                // trace.ps1 `Key`: down, one step, up.
                Step::Key(k) => {
                    self.send(steps::key(k));
                    self.frame();
                }
                Step::WaitScreen(id, max) => {
                    return self.until(*max, "screen", |r| r.screen() == *id)
                }
                Step::WaitPhase(p, max) => {
                    return self.until(*max, "phase", |r| r.game.kingdom.turn.phase as i64 == *p)
                }
                Step::WaitIdle(quiet, max) => return self.wait_idle(*quiet, *max),
                Step::Advance(n) => (0..*n).for_each(|_| self.frame()),
                // trace-run.ps1 `snap`: one frame drawn, then the hash.
                Step::Snap => {
                    self.machine.mark_dirty();
                    self.frame();
                    self.kept.push(self.frame);
                    let hash = shot::fnv(&self.canvas.pixels);
                    let shot = self.shot();
                    l2_net::trace!("snap", 0, "Canvas", hash = hash as i64, shot = shot);
                }
                Step::Shot => {
                    let hash = shot::fnv(&self.canvas.pixels);
                    let shot = self.shot();
                    l2_net::trace!("shot", 0, "Canvas", hash = hash as i64, shot = shot);
                }
                Step::Anchor(label) => l2_net::trace!("anchor", 0, "Scenario", label = label),
                _ => unreachable!("play handles {step:?}"),
            }
        }
        true
    }

    /// Idle: `quiet` frames in a row with no record but `draw`/`text`/`rand`.
    fn wait_idle(&mut self, quiet: u64, max: u64) -> bool {
        let mut still = 0;
        let mut last = Vec::new();
        for _ in 0..max {
            let at = trace::count();
            self.frame();
            let busy = trace::kinds_from(at).iter().any(|k| !matches!(*k, "draw" | "text" | "rand"));
            still = if busy { 0 } else { still + 1 };
            if busy {
                last = trace::kinds_from(at);
            }
            if still >= quiet {
                return true;
            }
        }
        l2_net::trace!("failed", 0, "Scenario", step = format!("wait-idle {quiet}: {last:?}"));
        false
    }
}

fn new_game(assets: &Assets, tables: l2_kingdom::tables::Tables) -> Game {
    let options = l2_game::setup::SetupOptions::new().commit(1, l2_net::Quirks::default());
    l2_game::scenario::new_game(assets, 0, &options, 1, 1, l2_game::scenario::SEED, tables)
        .expect("a new world")
}

/// Plays `tools/oracle/scenarios/<name>.jsonl`; writes `<name>.ours.jsonl` beside the test binary.
pub fn trace_scenario(name: &str) -> Option<(Vec<trace::Record>, PathBuf)> {
    let text = std::fs::read_to_string(scenarios_dir().join(format!("{name}.jsonl")))
        .expect("the scenario exists");
    let steps = script::parse(&text).expect("the scenario parses");
    let Some(mut run) = Run::launch() else {
        eprintln!("skip: no game install");
        return None;
    };
    let dir = std::env::var_os("L2_TRACE_OUT")
        .map_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from);
    run.shots = Some(shot::Shots { dir: dir.clone(), name: name.into(), next: 0 });
    trace::start();
    run.play(&steps);
    let kept = std::mem::take(&mut run.kept);
    let records: Vec<_> = trace::stop()
        .into_iter()
        .filter(|r| !matches!(r.kind, "draw" | "text") || kept.binary_search(&r.frame).is_ok())
        .collect();
    let out = dir.join(format!("{name}.ours.jsonl"));
    let body: String = records.iter().map(|r| r.to_json() + "\n").collect();
    std::fs::write(&out, body).expect("the trace writes");
    Some((records, out))
}

/// The scenarios `tools/oracle/trace-diff.js` reads: `L2_TRACE_OUT=dir cargo test ... -- --ignored`.
#[test]
#[ignore = "minutes of frames; run for the differ"]
fn the_differs_scenarios_write_traces() {
    let names = std::env::var("L2_TRACE_SCENARIOS")
        .unwrap_or_else(|_| "launch,newgame-original,newgame-new,zoom-map,endturn-england,endturn-year,raise-army".into());
    for name in names.split(',') {
        let Some((records, out)) = trace_scenario(name) else { return };
        let failed = records.iter().find(|r| r.kind == "failed").map(|r| r.to_json());
        eprintln!("{name}: {} records -> {} {failed:?}", records.len(), out.display());
    }
}

#[test]
fn launch_to_menu_reaches_the_title_page_and_writes_a_trace() {
    let Some((records, out)) = trace_scenario("launch-to-menu") else { return };
    eprintln!("{} records -> {}", records.len(), out.display());
    let screens: Vec<_> = records.iter().filter(|r| r.kind == "screen").collect();
    assert!(
        screens.iter().any(|r| r.args.contains(&("id", trace::Arg::I(0x1F)))),
        "the title page, g_screenId 0x1F, is in the trace: {screens:?}"
    );
    assert!(records.iter().any(|r| r.kind == "film"), "Smk_Play for the intro");
    let sound = |name: &str| records.iter().any(|r| r.kind == "sound" && r.name == name);
    assert!(sound("Music_Play") && sound("Sound_PlayFile"), "the title page's music and voice");
    let title = records.iter().position(|r| r.args.contains(&("label", trace::Arg::S("title".into()))));
    let shown = records.iter().position(|r| r.args.contains(&("id", trace::Arg::I(0x1F))));
    assert!(shown < title, "the title page is traced on the frame it opens, before the anchor");
    symbols::every_record_is_named(&records);
}
