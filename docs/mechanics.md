# Game mechanics — what is mapped, and what is not

> **Looking for how the game works, rather than how much of it we have?**
> That is [`rules.md`](rules.md) — the same mechanics in plain language with the real
> numbers. This document is the *inventory*: what has been looked at, and what has not.

**This document exists to be read by someone who has played the game**, so that they can
point at what is missing. That is a check nothing else here can perform: you cannot grep for
an absence, and every other document in `docs/` describes what we found rather than what we
failed to look for.

Two mechanics were added to this project in a single conversation because a player mentioned
them in passing — cattle needing peasants to tend them, and herd crowding in four bands.
Neither was in any document. Both are real, both are traced to the instruction stream, and
both are now implemented and checked against the shipped save (`docs/kingdom.md` §13).

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
- ✅ **Tax's effect on happiness — fixed.** It is two separate fields: `+0x0F` is
  `5 − rate` and the "Other counties" term `+0x16` is a lookup in `g_taxHappinessOther`,
  now transcribed as `TAX_HAPPINESS_OTHER`. We computed `min(5 − rate, 0)` for both, which
  agrees with the table at six rates out of 51 — one of them rate 0, which is every rate in
  the one save we test.
- ✅ Tax ceiling is **50**, and it lives in `l2-kingdom` beside the table whose length
  fixes it (was wrongly 100, and was in the application crate)
- 📖 Merchant buying and selling, 15 goods with prices
- 📖 Wages: traced, and now joined to the army records that pay them — `docs/armies.md` §6.4
- ❓ Bankruptcy: five stages, traced, never exercised

### Food and farming
- ✅ Grain: sowing (which debits the store for seed), growing, harvest, four-season cycle
- ✅ Ration levels: none / quarter / half / normal / double / triple, and their happiness cost
- ✅ Dairy feeds five people a head; cattle are slaughtered to make up the shortfall
- ✅ Field types: fallow, grain, pasture — and reclamation
- ✅ Fertility, and the *Advanced Farming* option that changes how it works
- ✅ Weather: six kinds, and their effect on sowing, growth, harvest and the herd
- ✅ **Cattle tending.** Three labourers per head is full staffing; below it the shortfall
  is added to the death rate, and a county with no pasture at all loses half its herd. Our
  herds no longer survive unattended, and all fifty-six of the save's own herd forecasts
  reproduce.
- ✅ Herd crowding, `herd / fieldsCattle`, four bands, stored and fed back into births and
  deaths
- 🕳 **Ale.** Brewing and its happiness effect are a rule a mod can set — but ale is *bought
  at the merchant*, and no merchant screen exists.

  **[V] Ale is base game, not the expansion.** A player doubted it was ever in the shipped
  product. `L2.eng` in the stock GOG install settles it: group 68 index 19 is the merchant
  tooltip *"Buy ale for your county, as a gift for its people."*, group 85 index 6 is the
  happiness line *"From ale"*, group 6 index 4 is the goods entry, group 62 has the ration
  readout *"No Ale quaffed"* / *"Barrels swilled."*, and group 295 lists it among what a
  merchant sells. It is easy to miss in play — one line on a breakdown panel — but it ships.
- ❓ **Sheep, and the food priority order** — both found on the ration screen (`L2.eng`
  group 62) while checking the above, and **neither has ever been looked at**:
  - **Sheep and wool look like a cut subsystem.** The evidence, all `[V]`:
    - They have names — `L2.eng` group 6 lists all fourteen goods, and 3 is *"Sheep"*,
      5 is *"Wool"*.
    - They have ration-screen text: *"No sheep eaten."*, *"Sheep feed"*, *"Sheep will
      remain."* — sheep are eaten like cattle.
    - **They are the only two goods priced zero** in the merchant table, and in this
      binary's idiom a zero is *"not offered"* rather than *"free"* — the same convention
      that stops the Baron building a royal castle.
    - **No county can produce either**, and there is no shepherd among the nine peasant jobs.
    - Group 68 gives ale its own merchant tooltip and gives sheep none.
    - Two people who have played the game do not remember them.

    Sheep make wool; both look cut; the names, the goods slots and the ration strings were
    left behind. That is an inference, not a finding — nobody has traced whether a scenario
    can grant sheep.

    **Reproduce them anyway.** The decision is to carry whatever the original carries,
    vestigial or not: keep sheep and wool as goods, keep their price of zero, and keep their
    ration lines. At parity the game itself demonstrates you cannot buy them, which is a
    better answer than our deciding in advance that they do not exist — and it costs two
    rows in a table. **Do not prune content on a judgement that it looks dead.**
  - *"Click on a food to swap its priority."* **The order food is eaten in is a player
    setting**, not the constant our `ration` pass assumes. Our pass hardcodes dairy → grain
    → slaughter; the original lets you reorder five foods.

### People
- ✅ Population, births by happiness, deaths by health and season
- ✅ Health: five bands, and how rations move them
- ✅ Happiness: the sum of tax, health, ration, events, ale and army terms
- ✅ Migration between neighbouring counties
- ✅ Population bands — one icon on screen stands for `ceil(pop/25)` people
- ⚠️ **Peasant jobs: nine of them, and the worker counts now import.** Every county's nine
  records sum to its population exactly, which is what proves the `0x0C` stride. The other
  **two** ints of each record — a wanted floor and a useful ceiling — are not imported, and
  neither is the allocator (`FUN_0044F6E7`) that fills them, which is why the herd's
  pre-season labour cannot be recovered from a save
- 🕳 Moving peasants between jobs (a rubber-band drag on the village screen)
- 📖 Unrest and revolt

### Land and building
- ✅ Castle types, costs, workforce, garrison caps, free archers
- 🕳 **The castle designer.** One of the game's signature features. Untouched.
- 📖 Industry: wood, iron, stone, weapons, with an efficiency ramp that compounds seasonally
- 📖 **A county's resources come from the map**, not from the county record —
  `County_PlaceResourceSites` reads plane 2 on `Town`-bank tiles at load. And an enemy army
  marching over a site disables it for three seasons, which is the writer of
  `disabled_seasons` we could not find — `docs/armies.md` §3.5
- ❓ Field painting — choosing what a field grows, by brush, on the map

### The wider game
- ✅ Random events: a 256-slot deck, 24 distinct, and the bug that exempts even-numbered counties
- 📖 The AI's fourteen-step turn, its four personalities and tax ladders
- 📖 **The four lords are the Knight, the Baron, the Countess and the Bishop** — `L2.eng`
  group 7, and the `Kt`/`Bn`/`Ct`/`Bp` prefixes on 448 shipped voice files. Four
  personality records, not five. `docs/diplomacy.md` §0
- 📖 **The Bishop builds royal castles at 2,000 gold** where the Knight needs 10,000 and the
  Baron and Countess never build one — and he is also the lord handed the most free gold
  every turn. It is the same byte, `lord == 4`, in both tables. §8.1–8.2
- ✅ Scoring
- 📖 **Diplomacy — traced end to end, implemented nowhere.** `docs/diplomacy.md`
  - 📖 **A standing per pair of realms**, −30 … +30, in the realm record at `+0x84 + n*0x10`.
    It heals +1 a turn towards other AIs and **never towards a human player**
  - 📖 **Seven messages you can send**: gift, compliment, insult, offer alliance, terminate
    alliance, ask an ally for help, ask an ally to attack. Five inbox slots per realm; the AI
    answers on its next turn, so a reply lags one turn
  - 📖 **Gold gifts ratchet.** Each gift is judged against the largest you have ever sent, and
    one under half the increment costs 8 standing — more than the best gift gains
  - 📖 **Compliments sour.** +15, +8, then −4 for every one after the third, forever
  - 📖 **Alliances are exclusive** — one ally at a time, one byte. They gate asking for help
    and they gate the offence hook. An AI's grudge against its own ally grows every turn and
    breaks the alliance at a per-lord threshold
  - 📖 **Two warnings then war.** Once an AI's standing bottoms out it sends *"Warning."*
    twice and then *"Notice of revenge."*, after which it will never ally with you again
  - 📖 **A message is an `L2.eng` group id**, and its voice file is
    `<Kt|Bn|Ct|Bp><group>_<1..4>.wav` — all 448 are in the install
  - 🕳 The diplomacy screen — `0x0B`, *"the other lords"*, `faces.pl8`, one of the 29
  - ❓ Whether the Baron favours peasant armies. The per-lord weapon rota is read; what its
    six fields index is **not established**. §8.3
- 📖 **Armies on the campaign map.** The record, the levy, movement, foraging, sieges and the
  battle-result return are all traced in `docs/armies.md`. Still nothing implemented.
- 📖 **Mercenaries.** Twelve fixed bands, one per nationality, walking the map; you hire the
  one standing in your county when you raise an army there. Sizes, prices, troop types and
  the walk come out of six static tables — `docs/armies.md` §5
- 📖 Merchants and transports as things that move — `docs/formats/plane4.md` for the routes,
  `docs/armies.md` §2 for the mover, the cost map and the pathfinder they share with armies

---

## The battle — the real-time half

- ✅ Figures, units, formations, the 80-figure ceiling
- ✅ Movement, elevation, cell swapping
- ✅ Pathfinding, including two reproduced original bugs
- ✅ Melee: attack bands, recovery as the only defence, the heavy blow
- ✅ Missiles: three weapon classes, range, reload, armour
- ✅ Battle AI: the strength advantage, the 200-frame think, 3 of 17 order handlers reachable
- 🕳 **The other 14 handlers are siege**, and there is no castle to besiege. The campaign half
  is traced now: engines are *built on the spot* over several seasons with the army pinned in
  place, and only then does the assault start — `docs/armies.md` §4
- 🕳 **Missiles are computed and never fired** — a hit resolves, nothing flies
- ❓ Siege engines: catapults, towers, rams, boiling oil as things that act. Where they come
  from is traced — 200 / 200 / 400 man-seasons each, and the defender's oil count is a switch
  on castle type — `docs/armies.md` §4
- ❓ Moat filling (traced: figures raise the terrain 15 times)
- ❓ Retreat, capture, what happens after a battle ends
- 📖 How a battle result returns to the campaign — `Battle_ReturnToCampaign`, `docs/armies.md` §7

---

## The interface

- ✅ Campaign map: a scrolling viewport, two zooms, edge-scroll
- ✅ The minimap is a *picture* in `MAPnn.PL8`, tinted per county
- ✅ Four county panels — population, tax, happiness, rations
- 📖 **29 screens exist.** We have three. Merchant, court, armoury, mercenaries,
  send-supplies, castle-building, siege prep and twenty more are enumerated and unbuilt.
  Three of them are now decompiled rather than merely enumerated: the raise-army screen
  (`0x00418653`, and the mercenary offer lives on it — there is no separate mercenaries
  screen), the army-division screen and the siege-preparation screen — `docs/armies.md`.
- 🕳 The original's fonts (`Fntl2_9/14/22.pl8`) — we draw with a hand-made 5×7
- ❓ The village screen, where peasants are moved
- 📖 **Sound: 771 `.wav` files, 396 MB. Nothing plays yet, but the shape is known.**

  | | files | size | |
  |---|---:|---:|---|
  | `PUMKIN.WAV` / `PUMKIN2.WAV` | 2 | **320 MB** | byte-identical; see below |
  | Lord voices | 449 | 32 MB | `Kt`/`Bn`/`Ct`/`Bp` × groups 170–197 × 4 takes |
  | `S`-numbered speech | 197 | 8 MB | |
  | Music | 10 | 27 MB | `Scroll1‑5` (map and county), `Battle1‑5` |
  | Troop and combat effects | 70+ | 1 MB | |

  **[V] The two 160 MB files are never played.** `FUN_004AEF7E` opens `pumkin.wav` — or
  `pumkin2.wav` if it is missing — calls `__filelength`, tests it against **151,000,000**,
  and closes it. It is a full-install check: the file exists to be *measured*. Its content is
  the soundtrack at CD quality, 44.1 kHz 16-bit stereo and 15.9 minutes, against every
  in-game track's 11 kHz 8-bit. That is the CD audio the original played through the drive,
  left on disk where the game only ever weighs it. A player confirmed by ear that `Scroll1`
  begins about 8:37 into it, so the tracks really are concatenated — and there is **no index**
  anywhere: none in the executable, and the WAV carries no `cue ` chunk. None was ever
  needed, because from the CD the game asked for a *track number*.

  **[V] The check is also inert.** `DAT_005C9A74` is set to 1 before the test, set to 1 again
  in both success branches, and never set to any other value anywhere in the binary. Its
  three readers ask `(flag < 1) || (2 < flag)`, which cannot be true. So **81% of this game's
  audio exists to satisfy a test whose answer is already fixed.** The flag is persisted in
  saves, so one could in principle carry a failing value; nothing writes one.
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
