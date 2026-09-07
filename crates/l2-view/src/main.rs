//! Windowed viewer for Lords of the Realm II sprites and screens.
//!
//! ```text
//! l2-view <file.pl8> <palette.256> [frame]
//! ```
//!
//! Left/Right step through frames, Escape quits.
//!
//! The canvas is a plain 640x480 buffer of palette indices - the same model the
//! original engine uses - which is then expanded through the palette to RGBA.
//! Keeping an indexed buffer rather than drawing RGBA directly matters: the
//! endgame is diffing our output against the original's framebuffer, and the
//! original thinks in palette indices.

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

struct Viewer {
    bytes: Vec<u8>,
    palette: Palette,
    title: String,
    frame: usize,
    frame_count: usize,
    /// Indexed canvas, exactly as the original engine would hold it.
    canvas: Vec<u8>,
    window: Option<Arc<Window>>,
    pixels: Option<Pixels<'static>>,
}

impl Viewer {
    fn new(pl8: PathBuf, pal: PathBuf, frame: usize) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(&pl8)?;
        let palette = Palette::from_bytes(&std::fs::read(&pal)?)?;
        let frame_count = Pl8::parse(&bytes)?.frames.len();
        let title = pl8
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Viewer {
            bytes,
            palette,
            title,
            frame: frame.min(frame_count.saturating_sub(1)),
            frame_count,
            canvas: vec![CLEAR_INDEX; CANVAS_W * CANVAS_H],
            window: None,
            pixels: None,
        })
    }

    /// Decode the current frame and blit it into the indexed canvas, centred.
    ///
    /// This mirrors the original's blitter: a pixel is written only where the
    /// decoded frame says it is opaque, so palette index 0 leaves the canvas
    /// showing through rather than painting black.
    fn compose(&mut self) {
        self.canvas.fill(CLEAR_INDEX);

        let pl8 = match Pl8::parse(&self.bytes) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("parse failed: {e}");
                return;
            }
        };
        let decoded = match pl8.decode(self.frame) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("frame {}: {e}", self.frame);
                return;
            }
        };

        let (fw, fh) = (decoded.width as usize, decoded.height as usize);
        let ox = CANVAS_W.saturating_sub(fw) / 2;
        let oy = CANVAS_H.saturating_sub(fh) / 2;

        for y in 0..fh.min(CANVAS_H.saturating_sub(oy)) {
            for x in 0..fw.min(CANVAS_W.saturating_sub(ox)) {
                let src = y * fw + x;
                if decoded.opaque[src] {
                    self.canvas[(oy + y) * CANVAS_W + ox + x] = decoded.indices[src];
                }
            }
        }

        if let Some(w) = &self.window {
            w.set_title(&format!(
                "{} - frame {}/{} - {}x{}",
                self.title,
                self.frame + 1,
                self.frame_count,
                fw,
                fh
            ));
        }
    }

    /// Expand the indexed canvas through the palette into the RGBA framebuffer.
    fn present(&mut self) {
        let Some(pixels) = self.pixels.as_mut() else {
            return;
        };
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
        if self.frame_count == 0 {
            return;
        }
        let n = self.frame_count as isize;
        self.frame = (((self.frame as isize + delta) % n + n) % n) as usize;
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
    if args.len() < 2 {
        eprintln!("usage: l2-view <file.pl8> <palette.256> [frame]");
        std::process::exit(2);
    }
    let frame = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let mut viewer = Viewer::new(PathBuf::from(&args[0]), PathBuf::from(&args[1]), frame)?;
    println!("{} - {} frames", viewer.title, viewer.frame_count);

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.run_app(&mut viewer)?;
    Ok(())
}
