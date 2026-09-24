//! Our side of the trace differ: a scenario played headless, every `l2_net::trace!` site
//! written as `<scenario>.ours.jsonl`. CNEW-trace-sink; `tools/oracle/scenarios/FORMAT.md`.

mod script;
mod symbols;

use std::path::PathBuf;

use l2_game::audio::{Audio, Director};
use l2_game::game::Assets;
use l2_game::input::{Event, Key};
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
        if self.machine.take_dirty() {
            let ctx = Ctx { game: &mut self.game, assets: &self.assets };
            self.machine.draw(&ctx, &mut self.canvas);
        }
    }

    fn screen(&self) -> i64 {
        self.machine.traced_screen_byte(&self.game).map_or(-1, i64::from)
    }

    fn until(&mut self, max: u64, what: &str, done: impl Fn(&Run) -> bool) {
        for _ in 0..max {
            if done(self) {
                return;
            }
            self.frame();
        }
        panic!("{what} not reached in {max} frames; {:#x} {:?}", self.screen(), self.machine.ids());
    }

    pub fn play(&mut self, steps: &[Step]) {
        for step in steps {
            match step {
                Step::Load(name) => {
                    let save = l2_testkit::fixture_save(name).expect("the save loads");
                    self.game = l2_game::scenario::from_save(&save, self.tables.clone())
                        .expect("the save plays");
                    self.machine = Machine::new(ScreenId::Campaign);
                }
                Step::NewGame => self.game = new_game(&self.assets, self.tables.clone()),
                Step::Click(x, y, right) => {
                    let (x, y) = (*x, *y);
                    self.send(Event::Pointer { x, y });
                    if *right {
                        self.send(Event::RightPress { x, y });
                        self.frame();
                        self.send(Event::RightClick { x, y });
                    } else {
                        self.send(Event::Click { x, y });
                        self.frame();
                        self.send(Event::Release { x, y });
                    }
                }
                Step::Key(k) => self.send(key(k)),
                Step::WaitScreen(id, max) => self.until(*max, "screen", |r| r.screen() == *id),
                Step::WaitPhase(p, max) => {
                    self.until(*max, "phase", |r| r.game.kingdom.turn.phase as i64 == *p)
                }
                Step::WaitIdle(quiet, max) => self.wait_idle(*quiet, *max),
                Step::Advance(n) => (0..*n).for_each(|_| self.frame()),
                Step::Snap => {
                    let hash = fnv(&self.canvas.pixels);
                    l2_net::trace!("snap", 0, "Canvas", hash = hash as i64);
                }
                Step::Anchor(label) => l2_net::trace!("anchor", 0, "Scenario", label = label),
            }
        }
    }

    /// Idle: `quiet` frames in a row with no record but `draw`/`text`/`rand`.
    fn wait_idle(&mut self, quiet: u64, max: u64) {
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
                return;
            }
        }
        panic!("not idle for {quiet} frames in {max}: {last:?}");
    }
}

fn new_game(assets: &Assets, tables: l2_kingdom::tables::Tables) -> Game {
    let options = l2_game::setup::SetupOptions::new().commit(1, l2_net::Quirks::default());
    l2_game::scenario::new_game(assets, 0, &options, 1, 1, l2_game::scenario::SEED, tables)
        .expect("a new world")
}

fn key(k: &str) -> Event {
    Event::KeyDown(match k {
        "Escape" => Key::Escape,
        "Enter" => Key::Enter,
        "Space" => Key::Space,
        "Backspace" => Key::Backspace,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Left" => Key::Left,
        "Right" => Key::Right,
        c if c.chars().count() == 1 => return Event::Text(c.chars().next().unwrap()),
        other => panic!("no key {other}"),
    })
}

fn fnv(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811C_9DC5u32, |h, &b| (h ^ b as u32).wrapping_mul(0x0100_0193))
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
    trace::start();
    run.play(&steps);
    let records = trace::stop();
    let dir = std::env::var_os("L2_TRACE_OUT")
        .map_or_else(|| PathBuf::from(env!("CARGO_TARGET_TMPDIR")), PathBuf::from);
    let out = dir.join(format!("{name}.ours.jsonl"));
    let body: String = records.iter().map(|r| r.to_json() + "\n").collect();
    std::fs::write(&out, body).expect("the trace writes");
    Some((records, out))
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
    symbols::every_record_is_named(&records);
}
