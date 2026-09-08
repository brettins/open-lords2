# Game mechanics — what is mapped, and what is not

**This document exists to be read by someone who has played the game**, so that they can
point at what is missing. That is a check nothing else here can perform: you cannot grep for
an absence, and every other document in `docs/` describes what we found rather than what we
failed to look for.

Two mechanics were added to this project in a single conversation because a player mentioned
them in passing — cattle needing peasants to tend them, and herd crowding in four bands.
Neither was in any document. Both are real, both are now traced to the instruction stream,
and one of them (`docs/kingdom.md` §13) is a **rule our simulation still gets wrong**.

Legend:

| | meaning |
|---|---|
| **✅ done** | traced in the binary *and* implemented, with tests |
| **📖 traced** | understood and written down, not implemented |
| **⚠️ wrong** | implemented, and known to disagree with the original |
| **🕳 gap** | we know it exists and have not done it |
| **❓ unknown** | nobody has looked |

---

## The kingdom — the turn-based half

### Money and taxes
- ✅ Tax rate per county, collection, treasury
- ⚠️ **Tax's effect on happiness.** It is a table lookup (`g_taxHappinessOther`), and we
  compute `min(5 − rate, 0)`. Agrees at rate 0, diverges from rate 6. Every test passes
  because the one save we test is all rate 0.
- ✅ Tax ceiling is **50** (was wrongly 100 until today)
- 📖 Merchant buying and selling, 15 goods with prices
- 🕳 Wages: traced, and no army exists to pay
- ❓ Bankruptcy: five stages, traced, never exercised

### Food and farming
- ✅ Grain: sowing (which debits the store for seed), growing, harvest, four-season cycle
- ✅ Ration levels: none / quarter / half / normal / double / triple, and their happiness cost
- ✅ Dairy feeds five people a head; cattle are slaughtered to make up the shortfall
- ✅ Field types: fallow, grain, pasture — and reclamation
- ✅ Fertility, and the *Advanced Farming* option that changes how it works
- ✅ Weather: six kinds, and their effect on sowing, growth, harvest and the herd
- ⚠️ **Cattle tending.** Three labourers per head is full staffing; below it cattle die. We
  model none of it, so our herds survive unattended.
- ✅ Herd crowding, `herd / fieldsCattle`, four bands
- 🕳 **Ale.** Brewing and its happiness effect are a rule a mod can set — but ale is *bought
  at the merchant*, and no merchant screen exists.

### People
- ✅ Population, births by happiness, deaths by health and season
- ✅ Health: five bands, and how rations move them
- ✅ Happiness: the sum of tax, health, ration, events, ale and army terms
- ✅ Migration between neighbouring counties
- ✅ Population bands — one icon on screen stands for `ceil(pop/25)` people
- 📖 Peasant jobs: nine or ten of them. The labour record is **three** ints a job — a wanted
  floor and a useful ceiling — and we import none of it, so industry and sowing produce
  nothing
- 🕳 Moving peasants between jobs (a rubber-band drag on the village screen)
- 📖 Unrest and revolt

### Land and building
- ✅ Castle types, costs, workforce, garrison caps, free archers
- 🕳 **The castle designer.** One of the game's signature features. Untouched.
- 📖 Industry: wood, iron, stone, weapons, with an efficiency ramp that compounds seasonally
- ❓ Field painting — choosing what a field grows, by brush, on the map

### The wider game
- ✅ Random events: a 256-slot deck, 24 distinct, and the bug that exempts even-numbered counties
- 📖 The AI's fourteen-step turn, its four personalities and tax ladders
- ✅ Scoring
- 🕳 Diplomacy
- ❓ Armies on the campaign map: raising, moving, supplying. **Nothing exists.**
- ❓ Merchants and transports as things that move

---

## The battle — the real-time half

- ✅ Figures, units, formations, the 80-figure ceiling
- ✅ Movement, elevation, cell swapping
- ✅ Pathfinding, including two reproduced original bugs
- ✅ Melee: attack bands, recovery as the only defence, the heavy blow
- ✅ Missiles: three weapon classes, range, reload, armour
- ✅ Battle AI: the strength advantage, the 200-frame think, 3 of 17 order handlers reachable
- 🕳 **The other 14 handlers are siege**, and there is no castle to besiege
- 🕳 **Missiles are computed and never fired** — a hit resolves, nothing flies
- ❓ Siege engines: catapults, towers, rams, boiling oil as things that act
- ❓ Moat filling (traced: figures raise the terrain 15 times)
- ❓ Retreat, capture, what happens after a battle ends
- ❓ How a battle result returns to the campaign

---

## The interface

- ✅ Campaign map: a scrolling viewport, two zooms, edge-scroll
- ✅ The minimap is a *picture* in `MAPnn.PL8`, tinted per county
- ✅ Four county panels — population, tax, happiness, rations
- 📖 **29 screens exist.** We have three. Merchant, court, armoury, mercenaries,
  send-supplies, castle-building, siege prep and twenty more are enumerated and unbuilt.
- 🕳 The original's fonts (`Fntl2_9/14/22.pl8`) — we draw with a hand-made 5×7
- ❓ The village screen, where peasants are moved
- ❓ Sound: 771 `.wav` files, nothing plays
- ❓ Video: 45 `.smk` files, no decoder, blocked on a licence decision (D5a)

---

## What to look for when reading this

The useful gaps are the ones that are **not on the list at all**. A mechanic you remember
that has no row here has never been looked at — and two of those turned up in twenty
minutes the first time anyone asked.

Particularly worth doubting: anything involving **things that move on the campaign map**
(armies, merchants, transports), anything about **what happens between turns**, and any
rule that only shows up at values the shipped save never reaches — that last category has
already produced two wrong rules today, both invisible to 932 tests.
