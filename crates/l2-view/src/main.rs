//! Viewer for Lords of the Realm II assets.
//!
//! ```text
//! l2-view <file.pl8> <palette.256> [frame]      # sprite sheet
//! l2-view --map <game dir> [slot]               # campaign map
//! l2-view --battle <game dir> [map]             # a .skr battle, running
//! ```
//!
//! Sprite and map modes: Left/Right step through frames or slots.
//! Battle mode: Space pauses, Right single-steps while paused, the arrow keys
//! and A/D scroll, F follows the fighting. Escape quits in all of them.
//!
//! The canvas is a plain 640x480 buffer of palette indices - the same model the
//! original engine uses - which is then expanded through the palette to RGBA.
//! Keeping an indexed buffer rather than drawing RGBA directly matters: the
//! endgame is diffing our output against the original's framebuffer, and the
//! original thinks in palette indices.
//!
//! **The window is a convenience, not the verification.** Everything drawn here
//! can be produced and asserted on with no window at all, which is what
//! `tests/install.rs` does.

use l2_formats::maps::{MapSet, Plane, PLANE_DIM};
use l2_formats::{Palette, Pl8, Skr};
use l2_mods::Platform;
use std::{path::PathBuf, sync::Arc, time::{Duration, Instant}};

use l2_view::battle::{self, BattleRunner};
use l2_view::canvas::{Canvas, TRANSPARENT};
use l2_view::figures::Colour;
use l2_view::scene::{self, BattleAssets, Camera};
use l2_view::terrain;

use pixels::{Pixels, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const CANVAS_W: usize = l2_view::canvas::WIDTH;
const CANVAS_H: usize = l2_view::canvas::HEIGHT;

/// The zoom-2 tile sets are 10x6, so a whole 64x64 map spans 630x378 and fits
/// on one screen. Zoom 0 (58x30) would need 3654x1890 and a scrolling viewport.
const TILE_W: usize = 10;
const TILE_H: usize = 6;

/// Map plane 1 selects a bank; the value is the layer index times four.
/// Layer order comes from the resource table at 0x004DA050.
const BANKS: [&str; 5] = ["Base2a", "Mtns2a", "Roads2a", "Town2a", "Castle2a"];

/// Simulation ticks advanced per drawn frame.
///
/// A *fixed* number, deliberately. Deriving it from elapsed wall-clock time
/// would make the state at a given moment depend on how fast the machine
/// happens to be, and `docs/netcode.md` does not allow the simulation to learn
/// anything from the clock. The viewer decides when to draw; it never decides
/// how a tick turns out.
const TICKS_PER_FRAME: u32 = 3;

/// How often the window is repainted while a battle runs.
///
/// This is the only place the clock is consulted, and it decides *when to
/// draw*, never what a tick contains. Without it the loop repaints as fast as
/// the machine can manage - about ten thousand frames a second here - which is
/// more than the surface can present and makes wgpu reject the submission
/// outright. The picture was a blank white window until this was throttled.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

enum Scene {
    Sprite {
        bytes: Vec<u8>,
        frame: usize,
        count: usize,
    },
    Map {
        maps: Vec<u8>,
        banks: Vec<Vec<u8>>,
        slot: usize,
        count: usize,
    },
    Battle(Box<BattleScene>),
}

struct BattleScene {
    runner: BattleRunner,
    assets: BattleAssets,
    camera: Camera,
    follow: bool,
    running: bool,
    map: usize,
}

struct Viewer {
    scene: Scene,
    palette: Palette,
    title: String,
    canvas: Canvas,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
    /// When the next battle frame is due. Paces drawing only.
    next_frame: Instant,
}

/// Build the mod overlay for a game directory. Loading through it rather than
/// straight off disk means a mod that supplies its own `T32_bat1.pl8` is picked
/// up with no change to the rendering path, and asset names resolve
/// case-insensitively - which matters because the shipped install is
/// inconsistent about casing.
fn platform(dir: &PathBuf, mods: Option<&PathBuf>) -> Result<Platform, Box<dyn std::error::Error>> {
    let mut builder = Platform::builder().base(dir);
    if let Some(m) = mods {
        // Discovery only finds candidates; enabling is a separate, deliberate
        // step so an overlay never activates something merely by its presence
        // on disk. A viewer wants everything it was pointed at - a game would
        // let the player choose, and the order they give is the conflict policy.
        let found = l2_mods::discover(m)?;
        let ids: Vec<String> = found.iter().map(|f| f.id.clone()).collect();
        if ids.is_empty() {
            println!("no mods found in {}", m.display());
        }
        builder = builder.mods_dir(m).enable(ids);
    }
    let platform = builder.build()?;
    // Say what a mod changed. Silence here means the base install is what you
    // are looking at.
    for m in &platform.load_order {
        println!("mod: {} {}", m.id, m.version);
    }
    for (file, layers) in &platform.report().shadowed_assets {
        println!("overridden: {file} <- {}", layers.join(" < "));
    }
    Ok(platform)
}

impl Viewer {
    fn sprite(pl8: PathBuf, pal: PathBuf, frame: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(&pl8)?;
        let palette = Palette::from_bytes(&std::fs::read(&pal)?)?;
        let count = Pl8::parse(&bytes)?.frames.len();
        let title = pl8
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Viewer::new(
            Scene::Sprite {
                bytes,
                frame: frame.min(count.saturating_sub(1)),
                count,
            },
            palette,
            title,
        ))
    }

    fn map(
        dir: PathBuf,
        mods: Option<PathBuf>,
        slot: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let platform = platform(&dir, mods.as_ref())?;
        let vfs = &platform.vfs;
        let maps = vfs.read("L2_maps.dat")?;
        let count = MapSet::parse(&maps)?.slot_count();
        let banks = BANKS
            .iter()
            .map(|b| vfs.read(&format!("{b}.pl8")))
            .collect::<Result<Vec<_>, _>>()?;
        // The zoom-2 tile sets ship no palette of their own; they share the
        // zoom-0 one.
        let palette = Palette::from_bytes(&vfs.read("Base1a.256")?)?;
        Ok(Viewer::new(
            Scene::Map {
                maps,
                banks,
                slot: slot.min(count.saturating_sub(1)),
                count,
            },
            palette,
            "L2_maps.dat".into(),
        ))
    }

    /// A running battle on one of a `.skr`'s twenty maps.
    fn battle(
        dir: PathBuf,
        mods: Option<PathBuf>,
        map: usize,
        skr_name: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let platform = platform(&dir, mods.as_ref())?;
        let vfs = &platform.vfs;

        let bytes = vfs.read(skr_name)?;
        let skr = Skr::parse(&bytes)?;
        let map = map.min(skr.map_count() - 1);
        let text = skr.text(map)?;
        let field = terrain::build(skr.terrain(map)?, 1);

        // Sixteen men per figure is what docs/battle.md derives for USER.SKR
        // map 0. The size ladder that would compute it from the two armies is
        // not implemented here.
        let a = battle::army_from_counts(&skr.army(map, l2_formats::Side::Attacker)?.counts, 16);
        let b = battle::army_from_counts(&skr.army(map, l2_formats::Side::Defender)?.counts, 16);
        let runner = BattleRunner::deploy(field, &a, &b);

        let assets = BattleAssets::load(
            |name| vfs.read(name).map_err(|e| format!("{name}: {e}")),
            Colour::Red,
            Colour::Blue,
        )
        .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
        let palette = Palette::from_bytes(&vfs.read(scene::TILE_PALETTE)?)?;

        let camera = scene::follow(&runner);
        let title = if text.title.is_empty() {
            format!("{skr_name} map {}", map + 1)
        } else {
            format!("{skr_name} - {}", text.title)
        };
        println!(
            "{title}: {} figures ({} vs {})",
            runner.fighters.len(),
            runner.living(l2_sim::SIDE_B),
            runner.living(l2_sim::SIDE_A)
        );
        Ok(Viewer::new(
            Scene::Battle(Box::new(BattleScene {
                runner,
                assets,
                camera,
                follow: true,
                running: true,
                map,
            })),
            palette,
            title,
        ))
    }

    fn new(scene: Scene, palette: Palette, title: String) -> Self {
        Viewer {
            scene,
            palette,
            title,
            canvas: Canvas::screen(),
            window: None,
            pixels: None,
            next_frame: Instant::now(),
        }
    }

    /// Blit one decoded frame into the canvas, honouring index-0 transparency -
    /// the same rule the original blitters use.
    fn blit(&mut self, pl8_bytes: &[u8], frame: usize, ox: i32, oy: i32) {
        let Ok(pl8) = Pl8::parse(pl8_bytes) else { return };
        let Ok(d) = pl8.decode(frame) else { return };
        self.canvas.blit(&d, ox, oy);
    }

    fn compose(&mut self) {
        self.canvas.clear(TRANSPARENT);
        let subtitle = match &self.scene {
            Scene::Sprite { .. } => self.compose_sprite(),
            Scene::Map { .. } => self.compose_map(),
            Scene::Battle(_) => self.compose_battle(),
        };
        if let Some(w) = &self.window {
            w.set_title(&format!("{} - {}", self.title, subtitle));
        }
    }

    fn compose_sprite(&mut self) -> String {
        let Scene::Sprite { bytes, frame, count } = &self.scene else {
            return String::new();
        };
        let (bytes, frame, count) = (bytes.clone(), *frame, *count);
        let (fw, fh) = match Pl8::parse(&bytes).and_then(|p| p.decode(frame)) {
            Ok(d) => (d.width as usize, d.height as usize),
            Err(e) => return format!("frame {}: {e}", frame + 1),
        };
        let ox = (CANVAS_W.saturating_sub(fw) / 2) as i32;
        let oy = (CANVAS_H.saturating_sub(fh) / 2) as i32;
        self.blit(&bytes, frame, ox, oy);
        format!("frame {}/{} - {}x{}", frame + 1, count, fw, fh)
    }

    /// Draw a campaign map slot.
    ///
    /// Standard isometric projection: screen x follows `x - y`, screen y follows
    /// `x + y`. Tiles are painted back to front in order of `x + y`, so nearer
    /// tiles overlap further ones - which is what makes the diamonds tessellate
    /// into a continuous landscape.
    fn compose_map(&mut self) -> String {
        let Scene::Map { maps, banks, slot, count } = &self.scene else {
            return String::new();
        };
        let (maps, banks, slot, count) = (maps.clone(), banks.clone(), *slot, *count);

        let Ok(set) = MapSet::parse(&maps) else {
            return "unparseable".into();
        };
        let Ok(map) = set.slot(slot) else {
            return format!("slot {slot} out of range");
        };
        if map.is_empty() {
            return format!("slot {}/{} - empty", slot + 1, count);
        }

        let origin_x = (CANVAS_W / 2) as i32 - (TILE_W / 2) as i32;
        let origin_y = 48i32;

        // Back to front. Within a diagonal the order does not matter, since
        // those tiles never overlap each other.
        let mut drawn = 0usize;
        for sum in 0..(2 * PLANE_DIM - 1) {
            for x in 0..PLANE_DIM {
                if sum < x || sum - x >= PLANE_DIM {
                    continue;
                }
                let y = sum - x;
                let bank = (map.at(Plane::GfxBank, x, y) / 4) as usize;
                let frame = map.at(Plane::GfxIndex, x, y) as usize;
                let Some(bytes) = banks.get(bank) else { continue };
                let sx = origin_x + (x as i32 - y as i32) * (TILE_W / 2) as i32;
                let sy = origin_y + (x as i32 + y as i32) * (TILE_H / 2) as i32;
                self.blit(bytes, frame, sx, sy);
                drawn += 1;
            }
        }
        format!(
            "slot {}/{} - {} counties, {} tiles",
            slot + 1,
            count,
            map.county_count(),
            drawn
        )
    }

    fn compose_battle(&mut self) -> String {
        let Scene::Battle(b) = &mut self.scene else {
            return String::new();
        };
        if b.follow {
            b.camera = scene::follow(&b.runner);
        }
        let drawn = scene::draw(&mut self.canvas, &b.runner, &b.assets, b.camera);
        let alive = (0..b.runner.fighters.len()).filter(|i| b.runner.is_alive(*i)).count();
        format!(
            "map {} - tick {} - {} v {} figures, {alive} alive, {drawn} drawn{}",
            b.map + 1,
            b.runner.tick,
            b.runner.living(l2_sim::SIDE_B),
            b.runner.living(l2_sim::SIDE_A),
            if b.running { "" } else { " [paused]" }
        )
    }

    fn present(&mut self) {
        let Some(pixels) = self.pixels.as_mut() else { return };
        for (px, &idx) in pixels
            .frame_mut()
            .chunks_exact_mut(4)
            .zip(self.canvas.pixels.iter())
        {
            let [r, g, b] = self.palette.rgb(idx);
            px[0] = r;
            px[1] = g;
            px[2] = b;
            px[3] = 0xff;
        }
        if let Err(e) = pixels.render() {
            eprintln!("render failed: {e}");
        }
    }

    fn step(&mut self, delta: i32) {
        let (cur, count) = match &mut self.scene {
            Scene::Sprite { frame, count, .. } => (*frame, *count),
            Scene::Map { slot, count, .. } => (*slot, *count),
            Scene::Battle(b) => {
                // Right single-steps the simulation, which is how a disputed
                // tick gets looked at.
                if delta > 0 {
                    b.runner.step();
                }
                self.compose();
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
                return;
            }
        };
        if count == 0 {
            return;
        }
        let n = count as i32;
        let next = (((cur as i32 + delta) % n + n) % n) as usize;
        match &mut self.scene {
            Scene::Sprite { frame, .. } => *frame = next,
            Scene::Map { slot, .. } => *slot = next,
            Scene::Battle(_) => {}
        }
        self.compose();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn scroll(&mut self, dx: i32, dy: i32) {
        let moved = if let Scene::Battle(b) = &mut self.scene {
            b.follow = false;
            b.camera = Camera::clamped(b.camera.x as i32 + dx, b.camera.y as i32 + dy);
            true
        } else {
            false
        };
        if moved {
            self.compose();
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
    }

    fn is_running_battle(&self) -> bool {
        matches!(&self.scene, Scene::Battle(b) if b.running)
    }
}

impl ApplicationHandler for Viewer {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title(self.title.clone())
            .with_inner_size(winit::dpi::LogicalSize::new(CANVAS_W as f64, CANVAS_H as f64));
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));

        let size = window.inner_size();
        let surface =
            SurfaceTexture::new(size.width.max(1), size.height.max(1), Arc::clone(&window));
        let pixels = Pixels::new(CANVAS_W as u32, CANVAS_H as u32, surface).expect("create pixels");

        self.window = Some(window);
        self.pixels = Some(pixels);
        self.compose();
    }

    /// A running battle advances a fixed number of ticks per frame and asks for
    /// a redraw; everything else waits for input.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.is_running_battle() {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let now = Instant::now();
        if now < self.next_frame {
            event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));
            return;
        }
        self.next_frame = now + FRAME_INTERVAL;
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame));

        if let Scene::Battle(b) = &mut self.scene {
            for _ in 0..TICKS_PER_FRAME {
                if b.runner.is_decided() {
                    b.running = false;
                    break;
                }
                b.runner.step();
            }
        }
        self.compose();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(p) = self.pixels.as_mut() {
                    if let Err(e) = p.resize_surface(size.width.max(1), size.height.max(1)) {
                        eprintln!("resize failed: {e}");
                    }
                }
            }
            WindowEvent::RedrawRequested => self.present(),
            WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                match event.physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                    PhysicalKey::Code(KeyCode::ArrowRight) => self.step(1),
                    PhysicalKey::Code(KeyCode::ArrowLeft) => self.step(-1),
                    PhysicalKey::Code(KeyCode::ArrowUp) => self.scroll(0, -2),
                    PhysicalKey::Code(KeyCode::ArrowDown) => self.scroll(0, 2),
                    PhysicalKey::Code(KeyCode::KeyA) => self.scroll(-2, 0),
                    PhysicalKey::Code(KeyCode::KeyD) => self.scroll(2, 0),
                    PhysicalKey::Code(KeyCode::KeyF) => {
                        if let Scene::Battle(b) = &mut self.scene {
                            b.follow = !b.follow;
                        }
                        self.compose();
                    }
                    PhysicalKey::Code(KeyCode::Space) => {
                        if let Scene::Battle(b) = &mut self.scene {
                            b.running = !b.running;
                        }
                        self.compose();
                        if let Some(w) = &self.window {
                            w.request_redraw();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn usage() -> ! {
    eprintln!("usage: l2-view <file.pl8> <palette.256> [frame]");
    eprintln!("       l2-view --map <game dir> [slot] [--mods <dir>]");
    eprintln!("       l2-view --battle <game dir> [map] [--skr <name>] [--mods <dir>]");
    std::process::exit(2)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // --mods <dir> and --skr <name> may appear anywhere; strip them before
    // positional parsing.
    let mut mods: Option<PathBuf> = None;
    let mut skr_name = "USER.SKR".to_string();
    let mut rest: Vec<String> = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--mods" => match it.next() {
                Some(d) => mods = Some(PathBuf::from(d)),
                None => usage(),
            },
            "--skr" => match it.next() {
                Some(n) => skr_name = n.clone(),
                None => usage(),
            },
            _ => rest.push(a.clone()),
        }
    }

    let mut viewer = match rest.first().map(|s| s.as_str()) {
        Some("--map") => {
            if rest.len() < 2 {
                usage();
            }
            let slot = rest.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            Viewer::map(PathBuf::from(&rest[1]), mods, slot)?
        }
        Some("--battle") => {
            if rest.len() < 2 {
                usage();
            }
            let map = rest.get(2).and_then(|s| s.parse::<usize>().ok()).unwrap_or(1);
            Viewer::battle(PathBuf::from(&rest[1]), mods, map.saturating_sub(1), &skr_name)?
        }
        _ => {
            if rest.len() < 2 {
                usage();
            }
            let frame = rest.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
            Viewer::sprite(PathBuf::from(&rest[0]), PathBuf::from(&rest[1]), frame)?
        }
    };

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut viewer)?;
    Ok(())
}
