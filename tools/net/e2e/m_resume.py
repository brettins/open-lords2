from e2e import *
# M: a three-seat game saved by its host, continued from the lobby: Matilda (same
# install) is seated on her realm, Eleanor (new) is given John's by a click.
h, a, b = three("47341")
def season(seats, past):
    for _ in range(12):
        for s in seats: settle(s); s.click(*END_TURN)
        try: return seats[0].wait(lambda st: int(st["turns"]) > past, "a season passes", 20)
        except AssertionError: pass
    raise AssertionError("no season passed")
season((h, a, b), 0)
for s in (h, a, b): settle(s)
time.sleep(2)
realms = {s.name: int(s.status()["player"]) for s in (h, a, b)}
note(f"M: realms {realms}")
settle(h); h.click(*FILE); time.sleep(0.5); h.click(*HOST_END_ITEM); time.sleep(0.6); h.click(*LEAVE_BOX["save_end"])
h.wait(lambda st: st["screens"] == "[Setup(Title)]", "host at title", 20)
for s in (a, b): s.kill()
h.click(320, 139); time.sleep(1)
h.shot("M1-entry"); h.click(320, 260); time.sleep(1)
h.shot("M2-picker"); h.key("enter")
h.wait(lambda st: st["hosting"] == "true" and st["phase"] == "Lobby", "host's room is up", 30)
a = Seat("Matilda", fresh=False); a.launch("--join", "127.0.0.1:4712", "--players", "3")
e = Seat("Eleanor"); e.launch("--join", "127.0.0.1:4712", "--players", "3")
h.wait(lambda st: st["roster"].count("+") == 3, "both joiners ready", 30)
note(f"M: room {h.status()['roster']} code={h.status()['code']}")
h.shot("M3-room-before")
top_y = 196 if h.status()["code"] not in ("", "None") else int(os.environ.get("ROSTER_TOP", "196"))
john = realms["John"]
h.click(0x90, top_y + 22 * (john - 1) + 6); time.sleep(1)
note(f"M: after clicking John's realm {h.status()['roster']}")
h.shot("M4-room-after")
h.click(224, 328)
for s in (h, a, e): s.wait(lambda st: st["live"] == "true" and int(st["tick"]) > 60, f"{s.name} playing", 90)
got = {s.name: int(s.status()["player"]) for s in (h, a, e)}
note(f"M: resumed realms {got}")
assert got == {"Richard": realms["Richard"], "Matilda": realms["Matilda"], "Eleanor": john}, got
t = int(h.status()["turns"])
for s in (h, a, e): settle(s)
h.shot("M5-resumed")
season((h, a, e), t)
for s in (h, a, e): advancing(s)
in_sync((h, a, e), "M: resumed game")
