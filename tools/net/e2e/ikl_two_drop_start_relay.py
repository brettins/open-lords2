from e2e import *
import sys
RELAY_BIN = os.path.expanduser("~/.cache/lords2-target/debug/l2-relay")
which = sys.argv[1:] or ["K", "L", "I", "Ilive"]

if "K" in which:   # both joiners crash at once; both come back
    h, a, b = three("47361")
    a.kill(); b.kill()
    h.wait(lambda st: "Matilda" in st["waiting"] and "John" in st["waiting"], "host waits for both", 30)
    note(f"K: host waiting={h.status()['waiting']}"); h.shot("K0-host-waits-two")
    a.launch("--join", "127.0.0.1:47361")
    a.wait(lambda st: st["live"] == "true", "Matilda back in", 60)
    time.sleep(2)
    note(f"K: with John still gone: host waiting={h.status()['waiting']} Matilda waiting={a.status()['waiting']} screens={a.status()['screens']}")
    a.shot("K1-matilda-waits-john")
    b.launch("--join", "127.0.0.1:47361")
    for s in (h, a, b): s.wait(lambda st: st["live"] == "true" and st["waiting"] == "[]" and not st["screens"].endswith("NetWait]"), f"{s.name} resumed", 60)
    for s in (h, a, b): advancing(s)
    time.sleep(3); in_sync([h, a, b], "K both back")
    for s in (h, a, b): s.kill()

if "L" in which:   # a joiner dies the moment the game starts
    seats = [Seat(n) for n in ("Richard", "Matilda", "John")]
    h, a, b = seats
    h.launch("--host", "47362", "--players", "3"); a.launch("--join", "127.0.0.1:47362", "--players", "3"); b.launch("--join", "127.0.0.1:47362", "--players", "3")
    b.wait(lambda st: st["phase"] == "Started", "John's game starts", 60)
    b.kill()
    note(f"L: John killed at host tick {h.tick()}")
    h.wait(lambda st: "John" in st["waiting"] or st["stalled"] == "true", "host notices", 30)
    time.sleep(3)
    note(f"L: host waiting={h.status()['waiting']} stalled={h.status()['stalled']} tick={h.tick()} screens={h.status()['screens']}")
    note(f"L: Matilda waiting={a.status()['waiting']} tick={a.tick()} screens={a.status()['screens']}")
    b.launch("--join", "127.0.0.1:47362")
    for s in (h, a, b): s.wait(lambda st: st["live"] == "true" and st["waiting"] == "[]" and int(st["tick"]) > 30 and not st["screens"].endswith("NetWait]"), f"{s.name} playing", 90)
    for s in (h, a, b): advancing(s)
    time.sleep(3); in_sync([h, a, b], "L John back after dying at the start")
    for s in (h, a, b): s.kill()

def by_code(relay, tag):
    h, a, b = Seat("Richard"), Seat("Matilda"), Seat("John")
    rl = ["--relay", relay] if relay else []
    h.launch(*rl, "--host", "47363", "--players", "3")
    h.wait(lambda st: st["code"] != "-", "a join code", 30)
    code = h.status()["code"]; note(f"{tag}: code {code}")
    a.launch(*rl, "--join", code, "--players", "3"); b.launch(*rl, "--join", code, "--players", "3")
    for s in (h, a, b): s.wait(lambda st: st["live"] == "true" and int(st["tick"]) > 60, f"{s.name} playing", 90)
    a.kill()
    h.wait(lambda st: "Matilda" in st["waiting"], "host waits for Matilda", 30)
    a.launch(*rl, "--join", code)
    for s in (h, a, b): s.wait(lambda st: st["live"] == "true" and st["waiting"] == "[]" and not st["screens"].endswith("NetWait]"), f"{s.name} resumed", 60)
    for s in (h, a, b): advancing(s)
    time.sleep(3); in_sync([h, a, b], f"{tag}: Matilda back by code")
    b.kill(); h.wait(lambda st: "John" in st["waiting"], "host waits for John", 30)
    h.click(*WAIT_HOST_COMPUTER)
    for s in (h, a): s.wait(lambda st: st["waiting"] == "[]" and not st["screens"].endswith("NetWait]"), f"{s.name} plays on", 20)
    advancing(h); advancing(a); time.sleep(3); in_sync([h, a], f"{tag}: after computer took John")
    for s in (h, a, b): s.kill()

if "I" in which:
    r = subprocess.Popen([RELAY_BIN, "--listen", "127.0.0.1:47400"], stdout=open(os.path.join(RUNS, "relay.log"), "w"), stderr=subprocess.STDOUT)
    time.sleep(1)
    try: by_code("127.0.0.1:47400", "I-local")
    finally: r.kill()
if "Ilive" in which:
    by_code(None, "I-live")
