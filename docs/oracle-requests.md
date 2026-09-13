# What we need from somebody who has the game — saves, and answers

**This is a list of things to do in the original game and then save.** It is written for
somebody who knows how to play Lords of the Realm II, in plain language, with no reference
to our code. Each item says what to do, when to press save, and — so you can judge whether
it is worth your time — **what it would settle**.

`docs/plan.md` §5 is the older, shorter version of this list, ordered by yield. This file is
the practical one: it exists because the hundred-turn game (`crates/l2-game/tests/long_game.rs`)
measured which rules we can reach on our own and which we cannot, and **eleven of them we
cannot reach at all.** Every request below is one of those.

## Ask for observations too, not only saves

This file was written asking for **files**, and that was too narrow. A save is the richest
thing somebody can send, but it is also the most work, and it is not always the right
instrument.

**The evidence for that is a question we could not settle by reading.** Whether the quarry
turns the stone industry *on* when it is dropped on a county had been read out of the
decompilation twice and left at `[D]` after about an hour of work. A player dropped one in
a live game and watched. **Thirty seconds, and it settled the byte from a direction the
decompiler cannot reach** — and it reversed him in his own favour, because his *first*
recollection had been right and his correction wrong.

So a question of the form *"play this for ten seconds and tell me what you saw"* is a
first-class request here, and often a better one than a save:

* it costs the person seconds rather than twenty minutes, so it can be asked often;
* it reaches **behaviour over time** — an animation, a sound, a number that moves while you
  watch — which a save cannot carry at all, because a save is one instant;
* it answers *"does this happen?"*, which is exactly the shape of most of what is left at
  `[D]`, and needs no tooling on our side to read;
* and the person answering **already knows the game**, so a recollection is evidence even
  before it is checked. §10 is entirely questions of this kind and it is the cheapest
  section in the file.

Two cautions, both from things that happened. A recollection can be **a real memory of the
wrong screen** — a player described a lord-to-colour preference table exactly, and it was
genuine, and it belonged to the custom battle screen rather than to new-game setup. Ask
*where* he saw it. And a description of what a control *does* is worth more than a
description of what it *means*: *"I dropped it and the little chimney started smoking"* is
a fact, and *"it turns the industry on"* is an interpretation of one.

When an answer settles something, record it like any other oracle result — with what was
asked, what was seen, and which reading it ruled out.

---

## Why this is worth twenty minutes

Every saved game this project has is **turn one**, and in every one of them **each lord holds
exactly one county**. So every rule that needs a bigger position than that has been written
from a decompiler and checked against nothing:

* the empire-wide tax penalty, which is *flat zero* until you tax above 19% — and no save
  this project has has ever carried a county above 8%. **The "no fixture has ever had a
  county above 0%" that stood here was measured on England turn one alone**: the turn pair
  and the six siege saves carry 2, 3, 6 and 8, which turned out to be a working oracle for
  the *"People pay"* preview and was never looked at. So the ask below is specifically for a
  rate of **20 or more**, which is where `g_taxHappinessOther` first bites, and not for a
  non-zero rate. `docs/decisions.md` C142;
* secession, which needs a lord holding lands in two pieces;
* revolt, bankruptcy, alliances, starving armies;
* the weather accumulator, which drifts for a hundred seasons and which we have only ever
  seen on its first.

`docs/decisions.md` C26 is the measured version of the danger: **a rule was wrong at 45 of
its 51 inputs and nothing noticed, because the only save we had exercised one value of its
input.** That was found twice in one afternoon behind a green test suite.

**A save is worth more than any amount of our own reasoning**, because two of our own
implementations agreeing proves only that we ported the same misunderstanding twice.

## Ask for observations too, and know which kind to act on

A save is not the only thing a person with the game running can give us, and it is not the
cheapest. **An observation costs thirty seconds and can settle what an hour of reading leaves
at `[D]`.** *"When I click 'mining off' and then right-click the mine it says it is not
operational — but if I add people back in, it says it's operational"* confirmed, from a
direction no decompiler reaches, a claim that had been settled at the byte an hour earlier and
was still one reading. Two independent sources agreeing is the standard `CLAUDE.md` sets and
the one we can rarely meet; this is a way to meet it for a pound.

So the list below asks for saves, and this section asks for **observations** — *what does it
do*, *what does it say*, *what happens if you*. Both are welcome and the second is nearly free.

**And a distinction that has now been measured over one long evening, because it says which
reports to act on and which to check first:**

> **Reports of what the game *did* have been reliable without exception. Recollections of
> specific *strings and numbers* were reversed three times, each cheaply.**

The behaviour reports were all right: the ration slider was inert, the tax panel was frozen,
the peasants did read as idle, the mine did have a real off state. Every one was a genuine
defect or a genuine mechanic, and every one repaid immediate work.

The recollections were not, and the pattern is not carelessness — twice the correction was
*more* confident than the original and *wronger*, because a correction sounds like the end of
a process:

| recalled | what the binary says |
|---|---|
| *"it does let you drop peasants off in a turned-off industry, IIRC"* | right — but he then corrected himself to *"it boots those people and reassigns them"*, which is wrong; the drop is accepted and **switches the industry on** |
| *"'All your people are fed by dairy.'"* | no such string in any of `L2.eng`'s 317 groups — **and the readout exists anyway, as a voice line.** `Panel_OpenRation` (`0x0043A846`) speaks `S021_01.wav` when the county has a standing herd and ate neither a cow nor a sack. §11 |
| the tower and ram costs | reversed, and the arithmetic closed either way |

The practical rule, and it costs nothing: **act on the behaviour, check the string.** A
sentence he remembers is a *lead to the mechanic*, which is what it was all three times —
every one of those recollections pointed at something real that we had got wrong. It is the
quotation that must not be implemented. **A memory that fits is not evidence, and a
calculation that closes confirms the formula rather than the labels on its inputs.**

## How to save one

1. When the list says *save*, quit to the menu and **Save Game** under a name from the list
   below — the names matter, because our tests look them up by name.
2. Copy the file out of the game folder into **`E:\dev\lords2-fixtures\`**. Do not leave it
   in the install: `lastturn.sav` is a rolling autosave and ten minutes of further play
   destroys it.
3. **You get a free before-and-after.** The game rotates `lastturn.sav` → `old_turn.sav` →
   `safeturn.sav` at every turn boundary, so after any single End Turn those three files are
   this turn, last turn and the turn before. Where a request asks for "before and after",
   copying all three out at once is enough.
4. Say what you did, in one line, with the file. *"Taxed county 3 at 40 for six seasons"* is
   worth as much as the file.

Nothing below needs skill, a particular map, or a good position. Several of them are easier
from a **bad** one.

---

## 1. An empire, with more than one county — the averages, not the tax table

**Retired, the tax half.** `[V]` The *"other counties' tax"* penalty is one unconditional
lookup: `Tax_RecomputePreview` (`0x0044B80B`) is the **only** writer of county `+0x16` and it
reads `g_taxHappinessOther[rate]` with no branch. The table is the 51 `i32` at `0x004D63D8` —
nineteen zeros, `-1` from 20%, `-2` from 24%, steepening to `-15` at 50% — and
`tools/oracle/kingdom.ps1` already reads all 51 entries out of the running binary. A save with
counties at 20/35/50 would confirm what the array says, entry for entry. Not worth twenty
minutes of anybody's time.

**Still open:** the **realm-wide averages**. A save is the only way to read a realm's
empire-wide happiness figure and its population, happiness and health averages, none of which
can be non-trivial with one county, and none of which is a table we can look up.

**Do:** play until **you hold four or more counties**, with their tax rates set apart (one at
0%, one at 20%, one at 35%, one at 50% will do). End the turn once. Save.

**Save as:** `empire-taxed.sav`, and the two rotated files with it.

---

## 2. A county in revolt — three saves, and the most discriminating ask on the list

**Do:** pick one of your counties and make its people miserable — tax it at 50%, set
rations to Starvation, and leave it. Watch the messages. Save **each** of these seasons
as it happens:

| when | save as |
|---|---|
| the season you first see *"Murmurs of unrest."* | `revolt-1-murmurs.sav` |
| the season the peasants actually rise — *"Revolution in your lands."* and a mob on the map | `revolt-2-rising.sav` |
| one End Turn later | `revolt-3-after.sav` |

Then, separately and just as valuable: bring a county's happiness below 25 for **two or
three seasons**, then put the tax back to 0 so it recovers above 25 for **one** season, then
push it back down. Save. Call it `revolt-4-recovered.sav`, and say in a line whether the
county revolted sooner than a fresh one would have.

**Settles four rules we changed this week on the strength of a decompilation and nothing
else:**

1. **How many seasons below 25 it takes.** The manual says *"more than four seasons"*. We
   now read the code as **five** — the first season below 30 spends itself on the warning
   message and the counter does not start until the second. We had it at four until today.
2. **Whether recovering wipes the counter.** We now read it as **yes, completely**: one
   season at 25 or above puts the counter back to zero. We had it as sticky until today.
   Request 4's second half is the whole experiment.
3. **What a revolt does.** We believe 30% of the county's people walk out as an armed mob
   standing on a free road tile near the county's centre, that they carry no weapons at all,
   and that **the county immediately goes neutral** — you lose it there and then. Until this
   week our engine printed the message and did nothing else.
4. **Whether an AI lord's counties revolt too.** We now read the code as yes. If you have
   ever seen an AI lord lose a county to peasants — or are sure you have not — **that
   sentence alone is worth a save.**

---

## 3. A bankrupt realm — six End Turns of not paying

**Do:** raise **several armies**, and hire **mercenaries** if you can. Spend your treasury
down to near nothing — buy grain, build a castle, whatever empties it. Then press End Turn
**six times in a row** without paying the wage bill, and save after **each** one.

**Save as:** `broke-1.sav` … `broke-6.sav`.

**Settles:** the whole bankruptcy ladder, which **has never once fired in any test this
project has run** — not in a hundred turns of England, not in a hundred turns of a
fourteen-county empire, not on any of the forty-four shipped maps. The AI lords are handed
free gold every turn and never overspend it, and the unplayed human has no army, so nothing
we can drive ourselves ever misses a wage.

What we think happens and cannot check: mercenaries leave the moment you miss a payment;
then a warning; then a tenth of every army deserts, each season, for three seasons; then a
last warning; then *"Furious at their ill treatment, all your troops have deserted."* and
every army is gone. **Paying in full once is supposed to reset the whole ladder** — if you
can spare a seventh save, pay up on season four and save that too, as `broke-recovered.sav`.

---

## 4. An army in the field, before and after one End Turn

**Do:** raise an army, march it out of its home county into a county of yours, and End Turn
once. Save. Then do it again with an army standing in a county whose granary is **smaller
than the army** — a starving one — and End Turn three times, saving each.

**Save as:** `army-fed.sav`, then `army-hungry-1.sav` … `army-hungry-3.sav`.

**Settles:** wages, foraging, the army's move allowance resetting, and the starvation ladder
— a warning, then 10% desertion a season, then the army dissolves. None of these has ever
met a file, and the hungry half has never fired in any of our runs either.

---

## 5. An empire cut in two

**Do:** get your lands into a **long thin shape** — a chain of counties — and then let the
middle one go: give it away by losing it in battle, or let it revolt (request 2 gets you
there). End Turn. **Save the turn before and the turn after.**

**Save as:** `divided-before.sav` and `divided-after.sav`.

**Settles:** the secession rule, which we believe works like this and which **no save can
currently contradict**: at the end of every season the game works out which of your counties
are joined to which, keeps only your **most populous** connected block, and everything else
declares independence that same season. If more than one county goes at once you are told
*"Your lands divide."*

Three things we would especially like the file to answer: whether the counties that fall off
become **neutral** rather than the enemy's; whether it is really the most *populous* block
that survives rather than the biggest; and whether an AI lord loses his in silence.

---

## 6. Several seasons on England with the AI running, and an unowned county on a merchant route

**This one has been asked for the longest and never fulfilled. It needs no skill and no
particular action — only that the game be left to run.**

**Do:** start England, do nothing in particular, and press End Turn six or eight times. Make
sure at least one county that **nobody owns** has a merchant walking through it. Save.

**Save as:** `merchants-running.sav`.

**Settles:** an unowned county keeps a small purse of its own, which should come out as
*tax banked + 100 a season − purchases*. **In every game we can drive ourselves that number
is zero**, and it has been zero for a year, so we have no idea whether we compute it at all.
The same file gives the first non-zero trade figures from the original, which is the only
way to know whether our arithmetic writes the same numbers rather than merely consistent
ones.

---

## 7. A drought and a flood

**Do:** play with **Advanced Farming on** and keep going until the county panels show
**Drought** somewhere and, in a later winter or spring, **Flooding** somewhere. Save the
season each one appears, and the season after.

**Save as:** `drought.sav`, `drought-after.sav`, `flood.sav`, `flood-after.sav`.

**Settles:** the weather accumulator. Every county carries a hidden "dryness" number that
drifts season by season, and the visible weather word is just a band of it. We believe the
season pushes every county the same way (spring +8, summer +24, autumn +12, winter −12,
minus a random 0–15), that **one** county and its neighbours get an extra swing, and that a
flood snaps the number back to 30 and a drought back to 70.

**The part with no evidence at all** is a per-county term we found only this week: each
county carries a fixed "climate band" from 0 to 4, cut out of its position in the scenario's
own county list, and in summer and winter that band shifts the local swing by up to 12. Two
saves a season apart, with several counties' weather visible, would settle the whole thing —
and it matters more than it sounds, because weather drives sowing, growth, harvest and the
herd **every season for a hundred seasons**.

If a field of yours is ever ruined to wasteland by the weather, that save is worth having on
its own (`wasteland.sav`) — reclamation is 800 units of work and we have never seen it run
in a real position.

---

## 8. An AI lord's counties, twenty seasons in

**Do:** play any map, leave the AI lords alone for twenty-five or thirty turns, then save.
Ideally take one of their counties on the last turn, so its fields are yours to look at.

**Save as:** `ai-farms.sav`.

**Settles:** three fields nobody has ever seen filled in — how much wasteland an AI county
carries, how much of it is being reclaimed, and which farming style byte it holds.

**And one thing you can answer without saving anything.** In our hundred-turn runs **two of
the four lords never lay a single grain field**: the Knight and the Baron feed enormous
populations entirely on dairy cattle, one county reaching sixteen thousand people on three
thousand head and no grain at all. That looked like a bug and it is not — the lords' style
table really is *graze, graze, plough, mix* — so the question is only whether it looks like
that in play. **Have you ever seen an AI lord's county with no wheat in it?**

---

## 9. A finished game

**Do:** win one, or lose one. Save the last turn before it ends, and — if the game lets you
— the screen after.

**Save as:** `endgame-won.sav` or `endgame-lost.sav`.

**Settles:** what the original actually does at the end. We can drive our own engine to a
win and to a loss, and we have found something odd doing it: after the last human realm is
eliminated our engine reports the game **lost** for one turn and then reports it **won**,
every turn thereafter, for ever. We think that is faithful — there is a second, sloppier
victory path in the original that crowns whoever is left standing — but *"we think the
original is also wrong here"* is not something to build on without a file.

---

## 10. One question that needs no save at all

**Is there a castle designer in your copy of the game?** The wall-drawing designer people
remember — laying out towers and gatehouses yourself — is **not in the executable we have
decompiled**. What is there is a picture-and-stats browser over five fixed castle types.
Our best guess is that the designer belongs to the *Siege Pack* edition rather than to
retail, and that guess is unverified.

**One sentence from you settles it and saves a large phase of work.** Same for: have you
ever seen an AI lord's county taken by peasants, and have you ever seen *"Your lands
divide."*?

## 11. Six sounds to listen to, which needs no save either

We now play these and **we do not know what any of them says.** Each is a `.wav` played by
name from one function, so the *occasion* is `[V]` from the decompilation and the *words* are
`[I]` from the occasion. Open the game, do the thing in the middle column, and write down
what the narrator actually says.

| file | do this | what we think it says |
|---|---|---|
| `S021_01.wav` | open the ration panel of a county with cattle that ate nothing this season | **"All your people are fed by dairy"** — the line you remembered. `Panel_OpenRation` plays it on exactly that condition |
| `S021_02.wav` | open the ration panel of a starving county | the complaint, whatever it is |
| `S033_01.wav` | click **Send supplies** on the sidebar | something about supply |
| `S033_02.wav` | zoom the map all the way out | something about the kingdom view |
| `S017_01.wav` | click **Split** on an army that has not moved | something about dividing an army |
| `S011_02.wav` | reach *"Choose your title and your shield"* in setup | the page's own prompt |

**`S021_01.wav` is the one that matters**, because it closes a thread three sessions old:
`docs/decisions.md` C133 searched all 317 `L2.eng` groups for *"all your people are fed by
dairy"*, correctly found nothing, and concluded the readout did not exist. It exists, and it
is spoken. If the clip says something else entirely, that is a finding too — and a cheap one.

## 12. The turn timer — **retired, answered by reading**

**`[V]`** The **only** per-turn reset of the countdown is `Turn_Tick`'s (`0x0049A010`) third
clause, `DAT_005440C8 = g_optTimeLimit; _DAT_00568D9C = timeGetTime();`. It runs **after** the
count, and it is gated on `aiStep != 999` — which `Turn_End` (`0x0043AC23`) has just set, and
which `Turn_BeginPlayersTurn` (`0x0049B6D3`) clears when the turn comes round. So a turn ended
early carries its own running time into the next one, the next one's first frame can compute a
negative count and call `Turn_End` on a counter that was just cleared, and the restart clause
below it then sees 999 and does nothing.

That is the whole of the question this section used to ask a person with a stopwatch: the
order of the three clauses settles it, and the same order is now `TurnClock::tick`'s. What a
stopwatch would still add is only **how often it bites on real hardware**, which is a matter of
how long a turn takes to run, not of what the code does. `docs/bugs.md` B99.
