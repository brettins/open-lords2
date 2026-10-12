from e2e import *
import sys
fails = 0
N = int(sys.argv[1]); n = int(sys.argv[2])
for i in range(N):
    port = str(47200 + i)
    seats = [Seat("Richard")] + [Seat(nm) for nm in ["Matilda", "John"][:n-1]]
    seats[0].launch("--host", port, "--players", str(n))
    for s in seats[1:]: s.launch("--join", f"127.0.0.1:{port}", "--players", str(n))
    try:
        for s in seats: s.wait(lambda st: st["live"] == "true" and int(st["tick"]) > 60, "playing", 60)
        note(f"run {i}: ok")
    except AssertionError as e:
        fails += 1
        note(f"run {i}: FAIL {e}")
        for s in seats: note(f"   {s.name}: {s.status().get('screens')} tick={s.status().get('tick')} waiting={s.status().get('waiting')} halted={s.status().get('halted')}")
    for s in seats: s.kill()
note(f"{fails}/{N} failed")
