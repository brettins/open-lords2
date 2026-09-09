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
both are now implemented and checked against the England turn-one fixture (`docs/kingdom.md` §13).

## The game states its own rules in three places, and we had read one

Before adding to this inventory, check whether the game already answers the question. Three
shipped files are the game speaking English about itself, and they are **oracles in the same
sense `Lords2.exe` is** — evidence, not prior art.

| | what it is | why it matters |
|---|---|---|
| `L2.eng` | every string the game draws, in 318 groups | a function that draws group *N* is a function about whatever group *N* is about — `docs/method.md` §7.2 |
| **`Readme.txt`** | **the v1.03 patch's rules errata, with manual page references** | **the game correcting its own manual, and it post-dates the manual** |
| the printed manual | the rules as intended | wrong twice, and superseded by `Readme.txt` wherever they disagree — C8, C10 |

**`Readme.txt` had not been read by anyone on this project until today**, and it is 192
lines. It confirms or extends, in the game's own words: herd overcrowding's **four** states
and their names; the ale **1:10** ratio and the +1…+5 happiness ladder; that **multiple
fields reclaim at once**, a field taking at least four seasons and taking its full worker
requirement before any labour spills to the next; that **blacksmiths share one global
resource pool** divided by how many are switched on; that castle construction **cannot be
given labour until every material has been delivered**, and that turning it off is how you
choose which castle gets materials first; that castle garrisons **forage for themselves** and
so do not eat county stores; and that armies may be **split three different ways**, including
below 50 men when splitting *into* a castle.

Several of those are mechanics this tree has traced from the instruction stream and several
are not implemented at all. Where it and the manual disagree, `Readme.txt` is later and
wins. Treat a claim it makes as **[V]**-adjacent: it is a statement by the authors about the
shipped build, not a recollection.

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
- 📖 **Merchant buying and selling, 15 goods with prices — now traced to one function.**
  `Merchant_Trade` (`0x004284CE`) is the only thing that moves a good. Positive quantity
  buys, negative sells, no partial fills, and an unowned county trades out of a purse of its
  own at `+0x1F4`. Which good goes where, and what that says about the armoury and about
  sheep, is `docs/kingdom.md` §7.6. Still implemented nowhere.
- 📖 Wages: traced, and now joined to the army records that pay them — `docs/armies.md` §6.4
- 📖 **Bankruptcy: six seasons, not five stages.** Named end to end and pinned to the
  messages that announce each rung — `docs/kingdom.md` §7.7. Still never exercised.

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

  **[V] There is no brewing, and no ale to store.** `Merchant_Trade` hands `Ale_Apply`
  (`0x00428C42`) the **crowns spent** and the happiness lands immediately; ale has no county
  field and no production. The ladder is one point per tenth of the population's worth of
  crowns, capped at five, then capped again at `5 − aleHappinessGiven` — and nothing resets
  that byte, so five points is a county's lifetime allowance. `Ale_PreviewGain`
  (`0x00435673`) is the same ladder copied out for the panel, which matters for modding:
  **two places, not one.** `docs/kingdom.md` §7.6.

  **[V] Ale is base game, not the expansion.** A player doubted it was ever in the shipped
  product. `L2.eng` in the stock GOG install settles it: group 68 index 19 is the merchant
  tooltip *"Buy ale for your county, as a gift for its people."*, group 85 index 6 is the
  happiness line *"From ale"*, group 6 index 4 is the goods entry, group 62 has the ration
  readout *"No Ale quaffed"* / *"Barrels swilled."*, and group 295 lists it among what a
  merchant sells. It is easy to miss in play — one line on a breakdown panel — but it ships.
- 📖 **Sheep, and the food priority order** — both found on the ration screen (`L2.eng`
  group 62) while checking the above. **Both are now traced, and group 62 is the answer to
  both:**

  > **[V] The shipped ration panel is group 87, and it has three foods.** `Panel_Ration`
  > (`0x00411B72`) draws dairy, grain and cattle out of county `+0x16C`, `+0x170` and
  > `+0x174`. There is no county field for sheep, none for ale, and `Ration_Apply` has no
  > priority array in it — dairy is unconditional and the rest is one percentage. **That is
  > the evidence**, and it is about code and record layout rather than about text.
  >
  > **[I] Group 62 is corroboration, and weaker than it first looked.** Every string quoted
  > below — *"swap its priority"*, *"Sheep feed"*, *"Barrels swilled."*, *"Dairy produce
  > feeds"* — occurs in group 62 and nowhere else in `L2.eng`, and no literal anywhere in
  > the corpus passes 62 to any of the six primitives that take a group. But **135 of the
  > 317 groups are unreached by a literal**, and some of those are certainly live: 94…98 are
  > the army names, and the event and help texts are indexed by a computed id through the
  > 70 call sites whose group argument is a variable. So "nothing references group 62" is
  > one fact in a set of 135, not a proof of death. It is consistent with a cut screen; it
  > does not establish one on its own.
  >
  > *This paragraph was first written the other way round, claiming the silence as the
  > finding. Re-deriving it without `anchor.js` — after the `litNum` escape bug — produced
  > the denominator that made the claim ordinary. The conclusion survives because it never
  > actually rested on the strings.*

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
    - **`Merchant_Trade` has no branch for good 3 or good 5, in either direction.** This is
      the one that upgrades the case from strong to closed, because it answers the obvious
      objection to the price of zero: *a zero price would still let you sell them for
      nothing, if the code could move them.* It cannot. Twelve goods have a branch; these
      two have none, buying or selling. `docs/kingdom.md` §7.6.
    - **The game's own tutorial text lists the goods and omits them.** Group 295 index 4:
      *"Visit a merchant … and buy cows, grain, weapons, ale, wood, iron, or stone."*

    Sheep make wool; both are cut; the names, the goods slots and the ration strings were
    left behind. **Whether a scenario can grant sheep is still untraced** — but even if one
    could, nothing would eat them and nothing could sell them.

    **Reproduce them anyway.** The decision is to carry whatever the original carries,
    vestigial or not: keep sheep and wool as goods, keep their price of zero, and keep their
    ration lines. At parity the game itself demonstrates you cannot buy them, which is a
    better answer than our deciding in advance that they do not exist — and it costs two
    rows in a table. **Do not prune content on a judgement that it looks dead.**
  - ✅ *"Click on a food to swap its priority."* — **and it is not a shipped setting.** The
    string is group 62's, and nothing reads group 62. What ships is `Ration_Apply`: dairy is
    subtracted first and free, and the remainder is divided between slaughter and grain by
    **one percentage**, county `+0x15F`, which the player moves with `Ration_SetSplit`. A
    dial, not a queue, and there is no third food in it to reorder.

    So the earlier note here was wrong in both halves: the original does *not* let you
    reorder five foods, and our pass's *"dairy → grain → slaughter"* is not the shipped rule
    either — grain and slaughter are simultaneous, weighted, and per county. `l2-kingdom`
    already carries the split (`docs/kingdom.md` §4.3 reproduces all fourteen counties with
    it); it was this inventory and `docs/rules.md` §4 that had the sequential story.

### People
- ✅ Population, births by happiness, deaths by health and season
- ✅ Health: five bands, and how rations move them
- ✅ Happiness: the sum of tax, health, ration, events, ale and army terms
- ✅ Migration between neighbouring counties
- ✅ Population bands — one icon on screen stands for `ceil(pop/25)` people
- ✅ **Peasant jobs: nine of them, and all three ints of every record import.** The worker
  count is word 0; word 1 is a **wanted floor** and word 2 a **useful ceiling**, and the
  `0x0C` stride is three `i32`s rather than one and eight bytes of slack.

  Their writers are traced and named: `Grain_LabourEstimate` (`0x0044D374`) and
  `Herd_LabourEstimate` (`0x0044DD4D`) each walk `workers = 0 … population` and store the
  first count that stops the job going backwards; every other job writes −1. The ceiling is
  the count past which more workers do no good — **100,000 for iron, stone and wood**,
  because more miners always help, and **0 for a resource the county has not got**.

  The shipped save checks it: seven of the nine floors are −1 in all fourteen counties, and
  wood's ceiling is exactly 100,000 in every owned county and exactly 0 in every unowned
  one. That one byte is the whole explanation of the save's labour split.
- ✅ **The labour allocator** — `Labour_Allocate` (`0x0044F6E7`), 2,147 bytes, the only
  writer of the nine records. It splits the population into a farm half and an industry half
  by county `+0x08`, gives each job `Pct(half, share)` people or as many as its ceiling
  allows, walks the leftovers round the jobs with room, and drops the remainder into *Idle
  townsfolk*. `l2_kingdom::labour` reproduces it and **all fourteen of the save's counties
  come back exactly**.

  On the way it named three fields `docs/screens-county.md` §9 had given up on: `+0x130`,
  `+0x134` and `+0x138` are the first three of **eight job percentages** at
  `+0x130 + job*4`, in two groups that each sum to 100.

  ✅ **It runs where a player's click reaches it** — `Field_SetType` and
  `Industry_ToggleFromMap` both allocate straight afterwards, and both do here.

  ✅ **And it is in the season pipeline now, twice.** It was not for a long time, and the
  obstruction was worth naming: `Season_Advance` (`0x00448440`, *not* `0x0044C1EE` — that
  address is inside `Field_ReclaimTick`) never calls `County_RefreshEstimates` before either
  `Labour_AllocateAll`, because **each estimate is the tail call of the pass that invalidates
  it**. So wiring the allocator was six tail calls rather than one function, and the missing
  seventh piece was that `County_RefreshEstimates` *does* run every season — as the middle
  statement of `Panels_RefreshAll`, `Season_Advance`'s last call.

  All nine ceilings are computed now. The three that could not be written before took real
  work: `Grain_Grow` and `Grain_Harvest` are now `land::grow_step` and `land::harvest_step`,
  which cap the crop at `labour * multiplier` and apply fertility (`docs/kingdom.md` §7.1);
  the four industry ceilings read the owning realm and `Industry_WeaponShares`
  (`0x0044F15B`); and `Field_ReclaimTick` now spends `labour[2]` as a budget, which is what
  kept wiring it from being a *silent* no-op. **The invariant closes** — the nine records sum
  to the population in every county in every season, for ten seasons of the England
  position. `crates/l2-kingdom/tests/labour_gap.rs`.

  🕳 One piece is still inferred: **the castle ceiling is 0 until a build's materials are
  delivered**, and this tree debits them up front, so the gate is permanently open. The
  arithmetic given a complete delivery is reproduced; the six delivery words are not
  modelled.

  ✅ And one worry retired: **`Labour_Allocate` never reads `labour_wanted`** — verified by
  exhaustion over its 2,147 bytes, which read `+0xCC + slot*0x0C` eight times and
  `+0xC8 + slot*0x0C` not once. The floors are a display value.
- ✅ Moving peasants between jobs: a rubber-band drag on the village screen — see below
- 📖 **Unrest and revolt — traced to the end now.** The ladder was known; what it *does* was
  not. `County_RaiseRevolt` (`0x004AC185`) puts 30% of the population on a free road tile as
  a kind-2 mob at morale 50, all of them in `troops[0]` — unarmed peasants — takes them out
  of the population, and hands the county to `County_MakeIndependent`. If there is no free
  tile it returns 0, the counter stays at 4, and the county never revolts. There is also a
  **dead branch**: the "happiness 0 revolts instantly" arm is guarded by a test identical to
  its own parent and can never run. `docs/kingdom.md` §6.
- ✅ **Territorial contiguity — a mechanic that was in none of these documents.** Every
  season, a realm keeps only its **most populous connected block of counties** and everything
  cut off from it declares independence. `Realm_SecedeIsolatedCounties` (`0x0044AE3C`), and
  the game names it itself: `L2.eng` 127 *"Deeming itself too far from the heart of your
  empire …"* and 128 *"Your lands divide."* It changes what conquest is worth in both
  directions: taking the county that *bridges* an enemy's territory costs him the far half
  for free, and a county taken behind his lines cannot be held. Implemented in
  `crates/l2-kingdom/src/territory.rs` as `Pass::SecedeIsolatedCounties`;
  `docs/kingdom.md` §6.1 and `docs/rules.md` §5a.

  **It still has no data-side oracle.** Every realm in every fixture holds exactly one
  county, so the invariant the pass maintains is trivially true in all six saves. What moved
  it from unanchored to corroborated is **a player's recollection that cut-off counties do
  secede in play**, and that plus two strings and the code is the whole of the anchor — it is
  the weakest-anchored thing this crate implements, and worth a real test the day a
  multi-county save exists.

### Land and building
- ✅ Castle types, costs, workforce, garrison caps, free archers
- 🕳 **The castle designer.** One of the game's signature features. Untouched.
- 📖 Industry: wood, iron, stone, weapons, with an efficiency ramp that compounds seasonally
- 📖 **A county's resources come from the map**, not from the county record —
  `County_PlaceResourceSites` reads plane 2 on `Town`-bank tiles at load. And an enemy army
  marching over a site disables it for three seasons, which is the writer of
  `disabled_seasons` we could not find — `docs/armies.md` §3.5
- ✅ **Field painting — choosing what a field grows, by brush, on the map.** This was the
  largest gameplay hole in the project: every county of the England position starts with
  `fieldsGrain = 0`, the counts had exactly one production writer (the scenario import), and
  so **the whole grain economy was finished, tested and unreachable in play** — for the
  player *and* for the AI. `docs/decisions.md` C27.

  The five brushes are read out of the executable's own hotspot tables, and the counts turn
  out to be a *cache* over the twenty map tiles' terrain bytes. `docs/kingdom.md` §7.2.
- ✅ **Switching an industry on and off**, which is also only reachable by clicking its
  building on the map — `Industry_ToggleFromMap` (`0x0043D309`) and the enable byte at
  `+0x297 + industry*0x18`. **Found in the binary and then confirmed by a player the same
  day, unprompted**: *"you can click on the forest or mine on the main map to turn them off
  for that county. 'Forestry off'. 'Forestry On'."* Those are the game's own strings —
  `L2.eng` groups 228–237, ten one-string groups the function picks by
  `230 + industry*2 + on`, which also confirms the industry numbering. `docs/kingdom.md`
  §7.4.1.

### The wider game
- ✅ Random events: a 256-slot deck, 24 distinct, and the bug that exempts even-numbered counties
- 📖 The AI's fourteen-step turn, its four personalities and tax ladders
- 📖 **The four lords are the Knight, the Baron, the Countess and the Bishop** — `L2.eng`
  group 7, and the `Kt`/`Bn`/`Ct`/`Bp` prefixes on 448 shipped voice files. Four
  personality records, not five. `docs/diplomacy.md` §0
- 📖 **The Bishop builds royal castles at 2,000 gold** where the Knight needs 10,000 and the
  Baron and Countess never build one — and he is also the lord handed the most free gold
  every turn. It is the same byte, `lord == 4`, in both tables. §8.1–8.2
- ✅ Scoring — and the treasury bonus is **50 flat above 2,000 gold**, not the three-rung
  ladder the table describes: `Score_RankRealms` tests its smallest threshold first, so the
  +100 and +200 arms are dead code. `docs/decisions.md` C33
- ✅ **Winning and losing.** `strength = 3 × counties + armies`, recounted at the top of every
  realm's turn including the human's; the last opponent's death notice is what raises
  *"Victory!"*; the outcome byte is 10 won / 11 lost and screen `0x1C` reads it.
  `docs/kingdom.md` §8.4, `crates/l2-kingdom/src/victory.rs`
- ✅ **The campaign** — eight maps, both tables read out of the executable, and **nothing
  carries between them**: the next map is a whole new game. `docs/kingdom.md` §8.5
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
- ✅ **Armies on the campaign map.** The unit record, the levy and the armoury, the cost map,
  the flood fill and the path extractor, the stepper with its trampling and field-crossing,
  and the season hooks — wages, foraging, the starvation ladder, the move reset. In
  `crates/l2-kingdom`: `unit`, `map`, `movement`, `levy`, `conquest`. `docs/armies.md`
- ✅ **Taking a county.** Stepping onto a county's castle tile is the attack; a castle *and* a
  garrison that is not yours is the siege gate; a neutral county below happiness 11 surrenders
  and anything else raises a defence. `docs/armies.md` §8 — a section that did not exist until
  it was implemented
- ✅ **Mercenaries.** Twelve fixed bands, one per nationality, walking the map; you hire the
  one standing in your county when you raise an army there. Sizes, prices, troop types and
  the walk come out of six static tables — `docs/armies.md` §5
- ❓ **Whether any of it matches the original in play.** There is **no army in
  `lastturn.sav`** — six units, all merchants — so nothing above has a data-side oracle, and
  the whole layer is code-only. `docs/armies.md` §9 lists the falsifiable predictions.
- 📖 Merchants and transports as things that move — `docs/formats/plane4.md` for the routes,
  `docs/armies.md` §2 for the mover, the cost map and the pathfinder they share with armies

---

## The battle — the real-time half

- ✅ Figures, units, formations, the 80-figure ceiling
- ✅ Movement, elevation, cell swapping
- ✅ Pathfinding, including two reproduced original bugs
- ✅ Melee: attack bands, recovery as the only defence, the heavy blow
- ✅ Missiles: three weapon classes, range, reload, armour
- ✅ Battle AI: the strength advantage, the 200-frame think, **all 17 order handlers
  reachable**. The fourteen siege ones had never been dispatched once, because nothing could
  produce a siege; `crates/l2-sim/tests/siege.rs` runs one and enumerates every handler it
  reaches. `docs/battle-ai.md` §6
- ✅ **Sieges, both halves.** Campaign: laying one, the engine order and its ceilings, the
  build over seasons, breaking it, the assault and its gate — `crates/l2-kingdom/src/siege.rs`,
  checked against **five snapshots of a real siege** (`crates/l2-kingdom/tests/siege.rs`).
  Battle: the two damage accumulators, the three siege end conditions, the wall, the gate and
  the way in — `crates/l2-sim/src/siege.rs`.
- 🕳 **Missiles are computed and never fired** — a hit resolves, nothing flies
- ⚠ **The castle's layout on the battlefield is ours, not the original's.**
  `Battlefield_BuildCastle`'s cell *translation* is read (`docs/battle.md` §3.0.1); the
  layout **rasters** it translates are not. `l2_sim::siege::our_castle` is a plain concentric
  keep with one of everything the rules need, and says so in its name and everywhere it draws.
- ✅ Siege engines and boiling oil as things that act: they are raised into the battle with
  the right counts (`Army_PrepareForBattle`, verified against `siege-sieging.sav`), the ram
  is the only figure that can reach the wall-breaking state, and the oil unit gets the
  defender-only handler. What they *look like* doing it is the renderer's, and is not done.
- ❓ Moat filling (traced: figures raise the terrain 15 times)
- ✅ **When a battle ends, and what happens after** — `Battle_CheckOutcome` (`0x00477DFC`),
  which was in no document at all until the seam was wired. A field battle ends **two** ways:
  one side's living men reaching zero, or a withdrawal, which is tested first.
  **No morale break, no rout threshold, no clock.** Seven `L2.eng` group 82 outcome banners,
  four of them sieges. `docs/armies.md` §7.3
- ✅ **The autocalc** — `Battle_AutoResolve` (`0x004AAD07`), the other of the two ways a
  battle can end, and the one every AI-versus-AI battle takes. A strength ratio into a
  ten-rung ladder; an evenly matched fight leaves the winner **a tenth of its army**.
  `docs/armies.md` §7.2
- ✅ **The campaign–battle seam** — an army that reaches an enemy county now fights and hands
  the result back: `crates/l2-kingdom/src/battle.rs` and `crates/l2-game/src/engagement.rs`,
  checked end to end against the battle fixture triple in `crates/l2-game/tests/seam.rs`.
  **`g_battleLoser` holds the winner**; four sites say so and `docs/armies.md` §7 lists them.
  Implementing it on the name destroys the winner and hands the county to the corpse
- 🕳 **A battle the player watches is not one the player can steer.** Orders go in at
  deployment and nothing withdraws, surrenders or re-tasks a unit mid-battle; the lever
  exists (`BattleRunner::withdraw`) and no rule pulls it
- ❓ Capture — prisoners and ransom, which nothing here has looked at

---

## The interface

- ✅ Campaign map: a scrolling viewport, two zooms, edge-scroll
- ✅ The minimap is a *picture* in `MAPnn.PL8`, tinted per county
- ✅ Four county panels — population, tax, happiness, rations
- ✅ **The village** (screen `0x02`) — the county's picture, its eight peasant clusters, and
  the rubber-band drag that moves people between jobs.

  ⚠️ **This entry said "a full screen" and was wrong.** The village is an **inset over the
  campaign map** — a 363 × 320 picture at (64, 64), with the menu bar, the county sidebar
  and a band of map showing around it. A player opened the game, clicked the town square and
  said he saw a dialogue with the map still around it; he was right and the decompiled
  reasoning was not. `docs/decisions.md` C22, and `docs/screens-county.md` §6.4.4.

  The drag is three screen ids in the original — `0x02` idle, `0x05` while the band is
  drawn, `0x06` while the selection is carried — so the gesture is **press, drag nine
  pixels, release, then a second click**, not drag-and-drop. Where a drop lands is a
  *painted file*: `vill_gd8.pl8` is a 45 × 40 grid of 8-pixel cells naming the cluster under
  every part of the picture.
- ✅ **Clicking the campaign map** — `Map_Click` (`0x0043CE1A`) dispatches every left click
  on it: your army, your merchant, the town square (the village), and **an industry
  building, which toggles that industry on or off**. That last one is the writer of the
  enable byte the labour allocator gates each mining job on, and nothing on any county panel
  does it — **player-confirmed**, `docs/kingdom.md` §7.4.1.
  `docs/screens-county.md` §6.4.5
- ✅ **The job popup** (screen `0x0F`) — the window and the head of it: the job's name, its
  worker count, and the three-colour rule that reads the record's other two words. Its nine
  per-job bodies are not drawn and say so.
- 🕳 The village's animations — `Village_Animate` steps six counters over
  `villani1`/`villani2`. The scene here is still.
- 📖 **`Screen_Draw` has 39 arms**, 35 of them with a named painter. Drawn with real
  contents: the front end and its setup pages, the campaign map, the county panels, the
  village, the job popup, the conquest screen, **and the siege-preparation screen**, which
  graduated out of the shell table: the three engine rows with their counts and percent
  bars, the seasons countdown, the six order buttons at the widget table's own coordinates,
  and *Lift siege* / *Proceed* both doing what the original's do. Merchant, court, armoury,
  send-supplies, castle-building and twelve more remain shells — right artwork and hotspots,
  contents unbuilt.
  Three of them are now decompiled rather than merely enumerated: the raise-army screen
  (`0x00418653`, and the mercenary offer lives on it — there is no separate mercenaries
  screen), the army-division screen and the siege-preparation screen — `docs/armies.md`.
- 🕳 The original's fonts (`Fntl2_9/14/22.pl8`) — we draw with a hand-made 5×7
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

## ❓ `mapl2.exe` — a second executable nobody has analysed

The install ships **two** PE binaries. `mapl2.exe` is 253,440 bytes, dated 16 July 1997, and
its own error strings call it the **"L2 Battlemap editor"**. Nothing in this project has
looked at it.

It is worth a task on its own for one reason: **it is an independent implementation of the
battlefield format by the original authors**, so it is a potential second oracle in the same
way `Lords2.exe` is the first — and a disagreement between the two would be far more
informative than either alone.

It is also a **debug build**, which is unusual and useful:

* the CodeView record still carries the PDB path `C:\tools\map_l2\Debug\mapl2.pdb`;
* it links the *debug* CRT — `dbgheap.c`, `dbgrpt.c` and friends are named in it.

Its error strings name six data files, and which of them ship is itself informative:

| file | ships? |
|---|---|
| `l2.sg2` | **yes**, 74,480 bytes, 14 Jul 1997 |
| `l2map.inf` | **yes**, 268 bytes, 28 May 1997 |
| `status.txt` | present, but the game **writes** it at runtime — not an input |
| `battles.txt` | no |
| `troops.txt` | no |
| `batfiel2.lbm` | no |

It also reads `l2.eng`, `*.skr` and `my_maps1.skr`. `tools/audit/mapl2.js` is a scratch
probe of its PE structure and a couple of tables; it is not analysis and nothing is written
down from it. **`l2.sg2` in particular is a shipped 74 KB file this project has never
opened.**

## What to look for when reading this

The useful gaps are the ones that are **not on the list at all**. A mechanic you remember
that has no row here has never been looked at — and two of those turned up in twenty
minutes the first time anyone asked.

Particularly worth doubting: anything involving **things that move on the campaign map**
(armies, merchants, transports), anything about **what happens between turns**, and any
rule that only shows up at values the England turn-one fixture never reaches — that last category has
already produced two wrong rules today, both invisible to **932 tests** — that count is
frozen at the day it happened (`docs/decisions.md` C26); the suite is far larger now and
that is exactly the point.
