//! The pointer, record and wait steps trace-run.ps1 added (`FORMAT.md`), on our side.

use l2_game::input::Event;
use l2_game::screen::Ctx;
use l2_net::trace;

use crate::script::Step;
use crate::Run;

/// trace-run.ps1 raw event categories (`hooktable.json` `cat`) to the FORMAT.md kind ours emits.
fn kind_of(event: &str) -> Option<&'static str> {
    Some(match event.split(':').nth(1)? {
        "army" | "order" | "panel" => "call",
        "sound" | "voice" | "music" => "sound",
        "msg" => "message",
        "film" => "film",
        "ai" => "ai",
        _ => return None,
    })
}

impl Run {
    pub(crate) fn input(&mut self, step: &Step) -> bool {
        match *step {
            Step::Move(x, y) => self.pointer(x, y),
            Step::Drag(x0, y0, x1, y1) => self.drag(x0, y0, x1, y1),
            Step::Record(n) => self.record(n),
            _ => unreachable!("{step:?}"),
        }
        true
    }

    pub(crate) fn pointer(&mut self, x: i32, y: i32) {
        self.send(Event::Pointer { x, y });
    }

    /// trace.ps1 `Drag`: press at (x0, y0), eight moves with the button held, release; two steps each.
    pub(crate) fn drag(&mut self, x0: i32, y0: i32, x1: i32, y1: i32) {
        self.pointer(x0, y0);
        self.frames(2);
        self.send(Event::Click { x: x0, y: y0 });
        self.frames(2);
        for i in 1..=8 {
            let (x, y) = (x0 + (x1 - x0) * i / 8, y0 + (y1 - y0) * i / 8);
            self.pointer(x, y);
            self.frames(2);
        }
        self.send(Event::Release { x: x1, y: y1 });
    }

    pub(crate) fn frames(&mut self, n: u64) {
        (0..n).for_each(|_| self.frame());
    }

    /// Frames with every painter run, as the original's `record` keeps each frame's draws.
    pub(crate) fn record(&mut self, n: u64) {
        for _ in 0..n {
            self.machine.mark_dirty();
            self.frame();
            self.kept.push(self.frame);
        }
    }

    /// Our save, film and new-game steps are synchronous: nothing to wait for.
    pub(crate) fn wait_event(&mut self, event: &str, max: u64) -> bool {
        let Some(kind) = kind_of(event) else { return true };
        let at = trace::count();
        self.until(max, &format!("wait-event {event}"), |_| trace::kinds_from(at).contains(&kind))
    }

    /// trace-run.ps1 `ok`: one frame, then the open box's OK; fails when none is up.
    pub(crate) fn ok(&mut self) -> bool {
        self.frame();
        let ctx = Ctx { game: &mut self.game, assets: &self.assets };
        let open = ctx.game.messages.open().cloned();
        let at = |r| l2_game::screens::message::window_frame(&ctx, r);
        let Some(f) = open.as_ref().and_then(at) else {
            l2_net::trace!("failed", 0, "Scenario", step = "ok");
            return false;
        };
        let (x, y) = f.ok_button();
        self.click(x + 24, y + 16);
        true
    }

    /// trace-run.ps1 `dismiss`: up to 24 steps for a box (screen 0x27, or ours over the map with
    /// screen 0), one step, OK at Ui_OkButton + (24, 16), 60 steps; at most 20 boxes.
    pub(crate) fn dismiss(&mut self) {
        let up = |r: &Run| r.screen() == 0x27 || r.game.messages.is_open();
        for _ in 0..20 {
            for _ in 0..24 {
                if up(self) {
                    break;
                }
                self.frame();
            }
            if !up(self) {
                break;
            }
            let ctx = Ctx { game: &mut self.game, assets: &self.assets };
            let open = ctx.game.messages.open().cloned();
            let at = |r| l2_game::screens::message::window_frame(&ctx, r);
            let Some(f) = open.as_ref().and_then(at) else { break };
            self.frame();
            let (x, y) = f.ok_button();
            self.click(x + 24, y + 16);
            self.frames(60);
        }
    }

    /// trace.ps1 `Click`: the pointer, two steps, press, two steps, release.
    pub(crate) fn click(&mut self, x: i32, y: i32) {
        self.pointer(x, y);
        self.frames(2);
        self.send(Event::Click { x, y });
        self.frames(2);
        self.send(Event::Release { x, y });
    }

    /// trace.ps1 `RClick`, the same with the right button.
    pub(crate) fn rclick(&mut self, x: i32, y: i32) {
        self.pointer(x, y);
        self.frames(2);
        self.send(Event::RightPress { x, y });
        self.frames(2);
        self.send(Event::RightClick { x, y });
    }
}
