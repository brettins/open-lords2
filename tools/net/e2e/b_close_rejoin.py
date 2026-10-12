from e2e import *
h, a, b = three("47301")
in_sync([h, a, b], "B start")
a.close()
for s in (h, b):
    s.wait(lambda st: "Matilda" in st["waiting"] and st["screens"].endswith("NetWait]"), f"{s.name} waits for Matilda", 30)
    note(f"{s.name}: {s.status()['screens']} waiting={s.status()['waiting']} frozen={frozen(s)}")
h.shot("B1-host-waiting"); b.shot("B1-joiner-waiting")
a.launch("--join", "127.0.0.1:47301")
for s in (h, a, b):
    s.wait(lambda st: st["live"] == "true" and st["waiting"] == "[]" and not st["screens"].endswith("NetWait]"), f"{s.name} resumed", 60)
for s in (h, a, b): advancing(s)
note("B: after rejoin " + ", ".join(f"{s.name} tick {s.tick()} notice={s.status()['notice']}" for s in (h, a, b)))
a.shot("B2-rejoined"); h.shot("B2-host-after")
time.sleep(4)
in_sync([h, a, b], "B after rejoin")
for s in (h, a, b): s.kill()
