import os, subprocess, time, shutil, signal
ROOT = os.path.dirname(os.path.abspath(__file__))
RUNS = os.environ.get("LORDS2_E2E_OUT", os.path.join(ROOT, "..", "..", "..", "out", "e2e"))
BIN = os.path.expanduser("~/.cache/lords2-target/debug/l2-game")
GAME = os.environ.get("LORDS2_DIR", "/mnt/bigfast/games/Lords of the Realm II")
PREFS = os.path.expanduser("~/.local/share/open-lords2/saves/prefs.ini")
LOG = []

def note(s):
    line = f"[{time.strftime('%H:%M:%S')}] {s}"
    print(line, flush=True); LOG.append(line)

class Seat:
    def __init__(self, name, fresh=True):
        self.name = name
        self.home = os.path.join(RUNS, name)
        self.saves = os.path.join(self.home, "saves")
        self.remote = os.path.join(self.home, "remote")
        if fresh:
            shutil.rmtree(self.home, ignore_errors=True)
            os.makedirs(self.saves)
            with open(PREFS) as f:
                lines = [l for l in f if not l.startswith(("InstallId", "FullScreen", "PlayerName"))]
            lines += ["FullScreen=0\n", f"PlayerName={name}\n"]
            open(os.path.join(self.saves, "prefs.ini"), "w").writelines(lines)
        self.p = None
        self.launches = 0

    def launch(self, *args):
        self.launches += 1
        env = dict(os.environ, LORDS2_SAVES=self.saves, LORDS2_REMOTE=self.remote,
                   WAYLAND_DISPLAY="lords2-e2e", RUST_BACKTRACE="1")
        env.pop("DISPLAY", None)
        self.out = open(os.path.join(self.home, f"stderr-{self.launches}.log"), "w")
        for f in ("status",):
            try: os.remove(os.path.join(self.remote, f))
            except FileNotFoundError: pass
        self.p = subprocess.Popen([BIN, "--no-sound", "--name", self.name, *args, GAME], env=env,
                                  stdout=self.out, stderr=subprocess.STDOUT)
        self.wait(lambda s: s, f"{self.name} up", 60)
        note(f"{self.name}: launched {' '.join(args)}")

    def send(self, line):
        with open(os.path.join(self.remote, "in"), "a") as f:
            f.write(line + "\n")
        time.sleep(0.25)

    def click(self, x, y): self.send(f"click {x} {y}")
    def key(self, k): self.send(f"key {k}")

    def status(self):
        try: txt = open(os.path.join(self.remote, "status")).read()
        except FileNotFoundError: return {}
        return dict(l.split(" ", 1) for l in txt.splitlines() if " " in l)

    def alive(self): return self.p is not None and self.p.poll() is None

    def wait(self, pred, what, timeout=45):
        t0 = time.time()
        while time.time() - t0 < timeout:
            s = self.status()
            try:
                if pred(s): return s
            except (KeyError, ValueError): pass
            if self.p and self.p.poll() is not None and what != "exit":
                raise AssertionError(f"{self.name} exited ({self.p.returncode}) waiting for: {what}")
            time.sleep(0.2)
        raise AssertionError(f"{self.name} never: {what}; status={self.status()}")

    def tick(self): return int(self.status().get("tick", "-1"))

    def shot(self, name):
        self.send(f"shot {name}")
        ppm = os.path.join(self.remote, f"{name}.ppm")
        for _ in range(40):
            if os.path.exists(ppm): break
            time.sleep(0.1)
        time.sleep(0.2)
        png = os.path.join(RUNS, "shots", f"{name}.png")
        os.makedirs(os.path.dirname(png), exist_ok=True)
        subprocess.run(["magick", ppm, "-scale", "200%", png], check=False)
        os.remove(ppm)
        return png

    def kill(self):
        self.p.send_signal(signal.SIGKILL); self.p.wait()
        note(f"{self.name}: killed (crash / pulled plug)")

    def close(self):
        self.send("close")
        self.p.wait(timeout=20)
        note(f"{self.name}: window closed, exit {self.p.returncode}")

    def hashes(self):
        try: return dict(l.split() for l in open(os.path.join(self.remote, "hashes")))
        except FileNotFoundError: return {}

def in_sync(seats, label):
    hs = [s.hashes() for s in seats]
    common = set(hs[0]).intersection(*hs[1:])
    bad = [t for t in common if len({h[t] for h in hs}) > 1]
    note(f"{label}: {len(common)} common hash checkpoints, {len(bad)} differ" + (f" at {sorted(bad, key=int)[:5]}" if bad else ""))
    assert not bad, f"desync at {bad}"
    return len(common)

def top(s):
    return s.status().get("screens", "[]").strip("[]").split(", ")[-1]

def settle(s, n=12):
    """Dismiss messages and tips with the right button until the map is on top."""
    for _ in range(n):
        t = top(s)
        if t == "Campaign": return True
        if t.startswith(("Message", "Tip", "Info")):
            s.send("rclick 300 300"); time.sleep(0.5)
        else:
            time.sleep(0.5)
    return top(s) == "Campaign"

def three(port, names=("Richard", "Matilda", "John")):
    seats = [Seat(n) for n in names]
    seats[0].launch("--host", port, "--players", str(len(seats)))
    for s in seats[1:]: s.launch("--join", f"127.0.0.1:{port}", "--players", str(len(seats)))
    for s in seats: s.wait(lambda st: st["live"] == "true" and int(st["tick"]) > 60, f"{s.name} playing", 90)
    for s in seats: settle(s)
    return seats

def advancing(s, by=40, timeout=45):
    t0 = s.tick()
    s.wait(lambda st: int(st["tick"]) >= t0 + by, f"{s.name} advances past {t0 + by}", timeout)
    return s.tick()

def frozen(s, secs=3):
    t0 = s.tick(); time.sleep(secs); t1 = s.tick()
    return t1 == t0

FILE = (30, 12)
JOINER_LEAVE_ITEM = (75, 45)
HOST_SAVE_ITEM, HOST_END_ITEM = (47, 45), (92, 64)
LEAVE_BOX = {"leave": (272, 256), "back": (272, 284), "save_end": (272, 228), "end_nosave": (272, 256)}
WAIT_HOST_COMPUTER, WAIT_HOST_SAVE_END, WAIT_JOINER_LEAVE = (272, 304), (272, 331), (272, 300)

def menu_leave(s):
    settle(s); s.click(*FILE); time.sleep(0.5); s.click(*JOINER_LEAVE_ITEM); time.sleep(0.6)
    note(f"{s.name}: File > Leave game -> {top(s)}")
HALT_ALONE, HALT_LEAVE = (244, 307), (396, 307)
END_TURN = (557, 467)

import atexit
def _reap():
    subprocess.run(["pkill", "-9", "-f", "[l]ords2-target/debug/l2-game"])
    time.sleep(0.5)
_reap()
atexit.register(_reap)

if not os.path.exists(os.path.join(os.environ.get("XDG_RUNTIME_DIR", "/tmp"), "lords2-e2e")):
    subprocess.Popen(["kwin_wayland", "--virtual", "--socket=lords2-e2e", "--width", "1920", "--height", "1080",
                      "--no-lockscreen", "--no-global-shortcuts"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(3)
