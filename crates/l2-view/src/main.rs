//! Viewer for Lords of the Realm II assets.
//!
//! ```text
//! l2-view <file.pl8> <palette.256> [frame]   # sprite sheet
//! l2-view --map <game dir> [slot]            # campaign map
//! ```
//!
//! Left/Right step through frames or map slots, Escape quits.
//!
//! The canvas is a plain 640x480 buffer of palette indices - the same model the
//! original engine uses - which is then expanded through the palette to RGBA.
//! Keeping an indexed buffer rather than drawing RGBA directly matters: the
//! endgame is diffing our output against the original's framebuffer, and the
//! original thinks in palette indices.

use l2_formats::maps::{Plane, MapSet, PLANE_DIM};
use l2_formats::{Palette, Pl8};
use std::{path::PathBuf, sync::Arc};

use pixels::{Pixels, SurfaceTexture};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

const CANVAS_W: usize = 640;
const CANVAS_H: usize = 480;

/// Palette index the canvas is cleared to. 0 is the game's transparent index,
/// so an unpainted canvas reads as "nothing drawn here" rather than a colour.
const CLEAR_INDEX: u8 = 0;

/// The zoom-2 tile sets are 10x6, so a whole 64x64 map spans 630x378 and fits
/// on one screen. Zoom 0 (58x30) would need 3654x1890 and a scrolling viewport.
const TILE_W: usize = 10;
const TILE_H: usize = 6;

/// Map plane 1 selects a bank; the value is the layer index times four.
/// Layer order comes from the resource table at 0x004DA050.
const BANKS: [&str; 5] = ["Base2a", "Mtns2a", "Roads2a", "Town2a", "Castle2a"];

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
}

struct Viewer {
    scene: Scene,
    palette: Palette,
    title: String,
    canvas: Vec<u8>,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
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

    fn map(dir: PathBuf, slot: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let maps = std::fs::read(dir.join("L2_maps.dat"))?;
        let count = MapSet::parse(&maps)?.slot_count();
        let banks = BANKS
            .iter()
            .map(|b| std::fs::read(dir.join(format!("{b}.pl8"))))
            .collect::<Result<Vec<_>, _>>()?;
        // The zoom-2 tile sets ship no palette of their own; they share the
        // zoom-0 one.
        let palette = Palette::from_bytes(&std::fs::read(dir.join("Base1a.256"))?)?;
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

    fn new(scene: Scene, palette: Palette, title: String) -> Self {
        Viewer {
            scene,
            palette,
            title,
            canvas: vec![CLEAR_INDEX; CANVAS_W * CANVAS_H],
            window: None,
            pixels: None,
        }
    }

    /// Blit one decoded frame into the canvas at (ox, oy), honouring the
    /// decoded opacity mask so index 0 leaves the canvas showing through - the
    /// same rule the original blitters use.
    fn blit(&mut self, pl8_bytes: &[u8], frame: usize, ox: isize, oy: isize) {
        let Ok(pl8) = Pl8::parse(pl8_bytes) else { return };
        let Ok(d) = pl8.decode(frame) else { return };
        let (fw, fh) = (d.width as usize, d.height as usize);
        for y in 0..fh {
            let cy = oy + y as isize;
            if cy < 0 || cy >= CANVAS_H as isize {
                continue;
            }
            for x in 0..fw {
                let cx = ox + x as isize;
                if cx < 0 || cx >= CANVAS_W as isize {
                    continue;
                }
                let src = y * fw + x;
                if d.opaque[src] {
                    self.canvas[cy as usize * CANVAS_W + cx as usize] = d.indices[src];
                }
            }
        }
    }

    fn compose(&mut self) {
        self.canvas.fill(CLEAR_INDEX);
        let subtitle = match &self.scene {
            Scene::Sprite { .. } => self.compose_sprite(),
            Scene::Map { .. } => self.compose_map(),
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
        let ox = (CANVAS_W.saturating_sub(fw) / 2) as isize;
        let oy = (CANVAS_H.saturating_sub(fh) / 2) as isize;
        self.blit(&bytes, frame, ox, oy);
        format!("frame {}/{} - {}x{}", frame + 1, count, fw, fh)
    }

    /// Draw a campaign map slot.
    ///
    /// Standard isometric projection: screen x follows `x - y`, screen y follows
    /// `x + y`. Tiles are painted back to front in order of `x + y`, so nearer
    /// tiles overlap further ones — which is what makes the diamonds tessellate
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

        let origin_x = (CANVAS_W / 2) as isize - (TILE_W / 2) as isize;
        let origin_y = 48isize;

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
                let sx = origin_x + (x as isize - y as isize) * (TILE_W / 2) as isize;
                let sy = origin_y + (x as isize + y as isize) * (TILE_H / 2) as isize;
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

    fn present(&mut self) {
        let Some(pixels) = self.pixels.as_mut() else { return };
        for (px, &idx) in pixels.frame_mut().chunks_exact_mut(4).zip(self.canvas.iter()) {
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

    fn step(&mut self, delta: isize) {
        let (cur, count) = match &self.scene {
            Scene::Sprite { frame, count, .. } => (*frame, *count),
            Scene::Map { slot, count, .. } => (*slot, *count),
        };
        if count == 0 {
            return;
        }
        let n = count as isize;
        let next = (((cur as isize + delta) % n + n) % n) as usize;
        match &mut self.scene {
            Scene::Sprite { frame, .. } => *frame = next,
            Scene::Map { slot, .. } => *slot = next,
        }
        self.compose();
        if let Some(w) = &self.window {
            w.request_redraw();
        }
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
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut viewer = if args.first().map(|s| s.as_str()) == Some("--map") {
        if args.len() < 2 {
            eprintln!("usage: l2-view --map <game dir> [slot]");
            std::process::exit(2);
        }
        let slot = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        Viewer::map(PathBuf::from(&args[1]), slot)?
    } else {
        if args.len() < 2 {
            eprintln!("usage: l2-view <file.pl8> <palette.256> [frame]");
            eprintln!("       l2-view --map <game dir> [slot]");
            std::process::exit(2);
        }
        let frame = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
        Viewer::sprite(PathBuf::from(&args[0]), PathBuf::from(&args[1]), frame)?
    };

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut viewer)?;
    Ok(())
}
