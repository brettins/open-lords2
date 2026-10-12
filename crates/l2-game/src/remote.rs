//! `[D]` ours, C261: `LORDS2_REMOTE=<dir>` lets a script play a real process. It reads
//! lines appended to `<dir>/in` (`click x y`, `rclick x y`, `key escape`, `text abc`,
//! `shot name`, `close`), writes `<dir>/status` each frame and appends `<dir>/hashes`
//! every 25 ticks. Unset, nothing here runs.

use std::io::Write;
use std::path::PathBuf;

use l2_game::input::{Event as GameEvent, Key};
use l2_game::screen::Ctx;
use l2_view::Canvas;

use super::App;

pub(crate) struct Remote {
    dir: PathBuf,
    read: usize,
    hashed: u32,
    last: String,
}

impl Remote {
    pub(crate) fn from_env() -> Option<Remote> {
        let dir = PathBuf::from(std::env::var_os("LORDS2_REMOTE")?);
        std::fs::create_dir_all(&dir).ok()?;
        let _ = std::fs::write(dir.join("in"), "");
        let _ = std::fs::remove_file(dir.join("hashes"));
        Some(Remote { dir, read: 0, hashed: 0, last: String::new() })
    }

    fn take_lines(&mut self) -> Vec<String> {
        let Ok(all) = std::fs::read_to_string(self.dir.join("in")) else { return vec![] };
        let Some(end) = all.rfind('\n').map(|i| i + 1) else { return vec![] };
        if end <= self.read {
            return vec![];
        }
        let lines = all[self.read..end].lines().map(str::to_string).collect();
        self.read = end;
        lines
    }
}

fn key(name: &str) -> Option<Key> {
    Some(match name {
        "escape" => Key::Escape,
        "enter" => Key::Enter,
        "space" => Key::Space,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "backspace" => Key::Backspace,
        _ => return None,
    })
}

impl App {
    pub(crate) fn remote_frame(&mut self) -> bool {
        let Some(mut r) = self.remote.take() else { return false };
        let mut close = false;
        for line in r.take_lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            let xy = || Some((w.get(1)?.parse::<i32>().ok()?, w.get(2)?.parse::<i32>().ok()?));
            match (w.first().copied(), xy()) {
                (Some("click"), Some((x, y))) => {
                    self.last_cursor = (x, y);
                    self.deliver(GameEvent::Pointer { x, y });
                    let e = self.left.pressed(x, y);
                    self.deliver(e);
                    if let Some(e) = self.left.released(x, y) {
                        self.deliver(e);
                    }
                }
                (Some("rclick"), Some((x, y))) => {
                    self.deliver(GameEvent::RightPress { x, y });
                    self.deliver(GameEvent::RightClick { x, y });
                }
                (Some("key"), _) => {
                    if let Some(k) = w.get(1).and_then(|n| key(n)) {
                        self.deliver(GameEvent::KeyDown(k));
                    }
                }
                (Some("text"), _) => line[5..].chars().for_each(|c| self.deliver(GameEvent::Text(c))),
                (Some("shot"), _) => self.remote_shot(&r.dir, w.get(1).copied().unwrap_or("shot")),
                (Some("close"), _) => close = true,
                _ => eprintln!("remote: ? {line}"),
            }
            self.machine.mark_dirty();
        }
        self.remote_status(&mut r);
        self.remote = Some(r);
        close
    }

    fn remote_shot(&mut self, dir: &std::path::Path, name: &str) {
        let mut canvas = Canvas::screen();
        let App { game, assets, machine, .. } = self;
        machine.draw(&Ctx { game, assets }, &mut canvas);
        let mut rgba = vec![0u8; 640 * 480 * 4];
        self.machine.present(&self.assets, &canvas, &mut rgba);
        let mut out = b"P6 640 480 255\n".to_vec();
        out.extend(rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]));
        let _ = std::fs::write(dir.join(format!("{name}.ppm")), out);
    }

    fn remote_status(&mut self, r: &mut Remote) {
        let n = &self.game.net;
        let tick = self.live.as_ref().map(|l| l.session.tick().0);
        let s = format!(
            "screens {:?}\nphase {:?}\nhosting {}\nme {:?}\nplayer {}\nlive {}\ntick {}\nhalted {:?}\nstalled {}\nwaiting {:?}\nquiet {:?}\nafter_halt {:?}\nnotice {:?}\nroster {:?}\ncomputer_plays {}\nleaving {:?}\nturns {}\ncode {}\n",
            self.machine.ids(),
            n.phase,
            n.hosting,
            n.me,
            self.game.player,
            self.live.is_some(),
            tick.map_or(-1, |t| t as i64),
            n.halted,
            n.stalled,
            n.waiting,
            n.quiet,
            n.after_halt,
            n.notice.as_ref().map(|(s, _)| s),
            n.roster.players.iter().map(|p| format!("{}{}@{}", p.name, if p.ready { "+" } else { "" }, p.realm)).collect::<Vec<_>>(),
            n.computer_plays,
            self.game.leaving,
            self.game.turns_played,
            n.code.as_deref().unwrap_or("-"),
        );
        if s != r.last {
            let tmp = r.dir.join("status.tmp");
            if std::fs::write(&tmp, &s).is_ok() {
                let _ = std::fs::rename(&tmp, r.dir.join("status"));
            }
            r.last = s;
        }
        let Some(live) = self.live.as_ref() else {
            // C263: the next game in this process counts from tick 0 again.
            if core::mem::take(&mut r.hashed) > 0 {
                let _ = std::fs::remove_file(r.dir.join("hashes"));
            }
            return;
        };
        let t = live.session.tick().0.saturating_sub(2) / 25 * 25;
        if t > r.hashed {
            if let Some(h) = live.session.hash_at(l2_net::Tick(t)) {
                r.hashed = t;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(r.dir.join("hashes")) {
                    let _ = writeln!(f, "{t} {h:016x}");
                }
            }
        }
    }
}
