# How the game works

Plain language, with the real numbers. Every other document here is written to help you
*find* something in the binary — offsets, function names, addresses. This one is written to
explain what the game **does**, so a person who has played it can read a paragraph and say
"no, that's wrong", and so an agent can understand a rule without first decoding a struct.

Where a number appears, it is verified against `Lords2.exe` unless marked otherwise. The
reference documents are `docs/kingdom.md`, `docs/battle.md`, `docs/armies.md` and
`docs/diplomacy.md`; this is the readable summary of all four.

---

## 1. The shape of a turn

A turn is one **season**. Four seasons make a year, and the year rolls over when Winter
*begins* — so the label runs Winter, Spring, Summer, Autumn.

Each turn has seven phases in a fixed order:

1. **Unowned counties** get their tax rates and fields set.
2. **Armies move**, and any battles are fought.
3. **Supply transports** walk toward their destinations.
4. **The players' turn** — you give orders; the AI lords run their fourteen-step program.
5. **Revolting peasants** move.
6. **Merchants** walk their routes.
7. **End of season** — everything below happens here, then the turn wraps to phase 1.

---

## 2. What happens between seasons

**The order is the rule.** Taxation reads the happiness that migration has not yet changed;
population growth reads the happiness this turn already wrote. Thirty passes, in this
sequence — and one of them, between the merchants and the muster, is an empty function:

| # | pass | what it does |
|---:|---|---|
| 1 | Clock | advance season; roll the year if Winter is beginning |
| 1a | Ledger | photograph every realm's gold, ore, timber and weapons, so the panels can show a change |
| 2 | Events | deal one random event per eligible county |
| 3 | Weather | roll each county's weather for the season |
| 4 | **Tax** | collect; write the tax happiness terms |
| 5 | Wages | pay the army |
| 6 | **Rations** | feed everyone; this is where food is actually spent |
| 7 | Health | move each county's health meter by how well it ate |
| 8 | **Happiness** | sum the terms into one number |
| 9 | Unrest | move each county towards or away from revolt |
| 9a | **Secession** | any county cut off from your main block declares independence |
| 9b | Field census | recount each county's fallow, grain and pasture strips off the map |
| 10 | Fertility | age the soil |
| 11 | Field reclamation | turn wasteland into usable fields |
| 12 | **Grain** | sow, grow or harvest, depending on the season |
| 13 | **Herd** | cattle births and deaths |
| 14–17 | Industry | weapons, then iron, then stone, then wood — **in that order** |
| 18 | Castle building | advance anything under construction |
| 19 | **Labour** | reassign every county's peasants to jobs, from scratch |
| 20 | Migration | move people between neighbouring counties |
| 21 | **Population** | births and deaths |
| 22 | Merchants | note which merchant is standing in which county |
| 23 | Muster | recount the men under arms in each county |
| 24 | Events expire | clear this season's event swings, so they last exactly one season |
| 25 | **Labour, again** | and once more, now that the newborns and the levies are counted |
| 26 | History | write this season's line into the 400-season ring |
| 27 | Ration preview | recompute each job's thresholds and what next season *would* cost |

**Scoring is not one of these passes.** It used to be listed here. `Score_RankRealms` is
called when the turn phase advances and when a realm's strength is recounted, not at the end
of the season — `docs/kingdom.md` §3.4.

Weapons are made **first**, before the ore is mined — so the blacksmith always spends last
season's iron.

**The labour pass runs twice**, and both times immediately after something changed how many
people there are. That is what keeps a county's nine job records summing to its population
exactly, every season, which is the invariant the whole record layout was proved from.

---

## 3. Happiness — the centre of everything

Happiness drives births, and births drive everything else. It is a running total, not
recomputed from scratch: each season a set of **deltas** are added to last season's value,
and the result is clamped 0–100.

The terms:

| term | how it works |
|---|---|
| **Tax** | `5 − rate`. A rate of 5 is free; below that you *gain* happiness, above it you lose |
| **Other counties' tax** | a table, not a formula. **Zero until rate 19**, then a slow slide to −15 at the maximum rate of 50 |
| **Health** | −10 diseased, −5 sick, 0 average, +1 good, +2 perfect |
| **Rations** | `3 × level − 8`: −8 at starvation, −5 quarter, −2 half, +1 normal, +4 double, +7 triple |
| **Ale** | +1 for every **10% of the population** you spend crowns on, cap +5 |
| **Army** | raising men costs happiness, scaled by what fraction of the county you took |
| **Events** | whatever the random event did |

An **unowned** county gets a flat bonus that an owned one does not, which is why the shipped
save has owned counties at 72 and unowned at 77.

Two things worth knowing because they surprise people:

- **Taxing at 19% costs your other counties nothing at all.** The empire-wide penalty is
  genuinely zero until rate 20, and only reaches −15 at rate 50.
- **The tax ceiling is 50**, not 100.
- **The ale cap is for the entire game, not per season.** The county remembers the total
  happiness ale has ever given it and the bonus is clamped to `5 − that`. Nothing anywhere
  in the binary resets the counter, so once a county has had its five points, ale is worth
  nothing there again, forever.

---

## 4. Food, and why counties starve

Feeding a county happens in one pass:

1. Work out what the county needs: `population ÷ divisor × multiplier`, where the divisor and
   multiplier come from the ration level. Normal rations need one sack per ten people.
2. **Dairy first, and free.** Every head of cattle feeds **5 people** without being killed.
   This is not a priority you can change; it is subtracted before anything else happens.
3. **The rest is split, not ordered.** A single percentage — the county's own setting — says
   how much of the remaining requirement comes from slaughtered cattle; the balance comes
   from grain. One head feeds 10 people, one sack feeds 6.
4. Each side is capped at what is actually in store, and if the total still will not fit the
   ration level drops and everyone is unhappier.

So it is a dial, not a queue, and it really is per county: in the shipped England save four
counties sit at 0% livestock and ten at 100%.

**A correction, because this section used to say otherwise.** It said the player could
reorder five foods, on the strength of the ration-screen text *"Click on a food to swap its
priority"* and the five foods beside it — *"Dairy produce feeds"*, *"Grain feeds"*, *"Sheep
feed"*, *"Cows feed"*, *"Barrels swilled."*

**Those strings are not the ration panel the game draws.** They are `L2.eng` group 62; the
panel is `Panel_Ration`, it reads group **87**, and it shows three foods. The code agrees:
the food pass has no priority list in it and the county record has no field for a food order,
for sheep, or for ale in store. Group 62 looks like a ration screen that was cut — though
that is an inference rather than a finding, and `docs/mechanics.md` says why it is weaker
evidence than it appears.

Armies standing in a county are **extra mouths at the county's ration level** — so they make
the bill bigger rather than eating a fixed amount, and an unpayable bill drops *everyone's*
rations, soldiers and peasants alike.

Separately, an army whose men outnumber the county's entire larder starts starving: a
warning, then 10% desertion a season, then it dissolves.

### Cattle need tending

**Three labourers per head is full staffing.** Below that, the shortfall is added to the herd's
death rate; above it, the benefit caps at twice. A county with cattle and **no pasture at
all** loses half its herd — or all of it, if there are fewer than six head.

Crowding matters too, and it is `herd ÷ pasture fields` in four bands:

| head per field | the game says | births | deaths |
|---:|---|---:|---:|
| 1–10 | *Low herd crowding* | 14% | **1%** |
| 11–20 | *Average herd crowding* | 9% | **3%** |
| 21–30 | *Herd overcrowded* | 5% | **5%** |
| 31+, or no pasture | *Massive overcrowding!!* | 2% | **7%** |

*(The deaths column read 0.01% … 0.07% until it was checked against
`Herd_BirthsAndDeaths`. The rate is per **ten thousand** and it is applied to `herd × 100`,
not to `herd`, so it comes out as whole percent. A hundredfold error, and it made the whole
table look like a births table with a rounding error attached.)*

That is the whole point of the table: **an overcrowded herd dies seven times as fast and
breeds a seventh as often** — and at the worst band births and deaths are within a whisker of
each other, so an overcrowded herd barely grows at all. Small herds that are fully staffed
also get a birth bonus on top, so a handful of well-tended cattle recovers much faster than
the percentage suggests.

**Births and deaths are also seasonal, which nothing here used to say.** *"Do births depend on
the season?"* — yes, directly: **spring gives half again as many calves** and **winter takes
half again as many cows**, applied on top of everything above. Nothing happens in summer or
autumn beyond the ordinary rates.

**Crowding and staffing are separate axes, and this is the thing that surprises people.**
Crowding is `herd ÷ pasture fields`; staffing is `labourers ÷ (herd × 3)`. **A large pasture
with few milkmaids reads *Low herd crowding* and is badly understaffed at the same time** — and
because the shortfall is added to the *death* rate, that county can be losing double digits a
season while the panel shows the best crowding line there is. At zero labourers the death rate
reaches 34%.

Crowding is also drawn on the map, so you can see it without opening a panel.

**What the sidebar's cattle figure includes.** The campaign sidebar's cattle row is `L2.eng`
group 220's *"Cattle, and change next season"*, and it is the **whole** change: births, minus
deaths, minus the animals your people are about to eat. The cattle job popup splits the same
arithmetic into the three lines it is made of — *Change due to farming* (births − deaths),
*Change due to eating*, and *Overall change* — and **the sidebar shows the third.** So a
negative number there with a healthy herd usually means your people are eating well, not that
your cattle are dying.

### Fields are painted on the map, and you start with none sown

A county owns up to twenty fields, and **what each one is being used for is a property of
the map tile, not of the county**. The county's "6 grain, 8 pasture, 3 fallow" is recounted
from those twenty tiles every time one of them changes.

**You paint them by clicking them on the campaign map.** There is no field control on any
county panel. Clicking one of your own fields opens a small popup of three buttons — fallow,
grain, pasture — and clicking a tile that is waste or already being reclaimed offers two
instead: begin reclaiming, or abandon it. A field ruined by this season's drought or flood
offers nothing until the weather moves on.

Every county begins with **no grain fields at all**. That is what the first winter of a game
is for: a county that is not painted grows nothing, because sowing multiplies the number of
grain fields by the sacks it can afford, and nought times anything is nought.

Three consequences worth knowing:

- **Painting reassigns the county's labour immediately.** The game recounts the fields,
  works out how many farmers the new arrangement can use, and reallocates. Paint a field to
  grain and farmers appear on it in the same click.
- **An empty granary means no farmers.** The ceiling on grain farmers is worked out by
  asking how many people would improve the sowing, and sowing is limited by the seed in the
  store — so a county with no grain is told it has no use for a farmer. Buy grain *before*
  you paint.
- **A drought or a flood turns a field to waste, not back to fallow**, and waste has to be
  reclaimed — 800 units of work, at most a quarter of a field a season, so four seasons.

**Reclamation is paid for by the hour.** One unit of that 800 costs one worker-season, so
200 people is a quarter of a field and 800 is a whole one — and **a county with nobody on
reclamation reclaims nothing at all**, however many fields it has started. The gang works
the nearest-to-finished field first and moves on to the next when it is done, so putting a
handful of people on it finishes one field slowly rather than four fields never.

### The crop has to be tended all year, not just sown

Sowing is only the first of three demands the year makes on your grain farmers, and the
other two are just as capable of losing you the harvest:

- **In summer and autumn the crop is capped at what the farmhands can tend** — ten sacks a
  worker with *Advanced Farming*, two without. Sow a full crop, then move everyone off the
  fields, and you reap nothing.
- **At harvest it is capped again, and much harder**: with *Advanced Farming* only half your
  grain farmers count as reapers and each brings in three sacks, so **one and a half sacks a
  head**. A crop that took a hundred people to grow takes far more than a hundred to get in.
- **Fertility multiplies the crop, twice.** The soil rating runs −100 to +100 and is worth
  half of itself as a percentage at *each* of the two growing steps, so perfect soil reaps
  **two and a quarter times** what neutral soil does and ruined soil about a quarter. One
  fallow field per two grain fields is exactly break-even, and cattle fields do not count.
- **Ploughing a wheat field under in midsummer costs you a share of that year's crop**, in
  proportion to the fields you have left against the fields you sowed. Painting *more* grain
  mid-year does nothing at all until the next sowing.

None of this shows up if you leave the labour dials alone, because the game recomputes how
many farmers the fields can use and reassigns people to them every single season. It shows
up the moment you move people off the land by hand.

The same click switches **industries** on and off: clicking a mine, quarry, forest or smithy
on the map toggles it, and there is no other way to do it either. **[V]**, and confirmed by a
player from play before we had asked him: *"you can click on the forest or mine on the main
map to turn them off for that county. 'Forestry off'. 'Forestry On'."* Those are the game's
own words — `L2.eng` groups 228–237, ten one-string groups the toggle picks by arithmetic:

| industry | off | on |
|---|---|---|
| castle building | 228 *"Building off"* | 229 *"Building on"* |
| 0, wood — forestry | 230 *"Forestry off"* | 231 *"Forestry on"* |
| 1, iron — mining | 232 *"Mining off"* | 233 *"Mining on"* |
| 2, weapons — the blacksmith | 234 *"Blacksmith off"* | 235 *"Blacksmith on"* |
| 3, stone — quarrying | 236 *"Quarrying off"* | 237 *"Quarrying on"* |

The shipped `Readme.txt` describes the same mechanic in English a third time, from the other
end: *"turning a blacksmith on will reduce the resources available to other blacksmiths"*, and
castle construction *"'off'"* as the way to choose which castle gets materials first.

---

## 5. People

**Births are two numbers multiplied.** A base rate off a **population** ladder, scaled by a
**happiness** factor:

- The ladder falls as the county fills: 100% up to 40 people, 50% at 100, 20% at 500, 11% at
  1,100, and 2% by 2,000.
- The happiness factor is five coarse bands — **25%** below 26, **50%** below 51, **75%**
  below 76, **100%** below 100, and **120%** at exactly 100.

That ladder is the soft population cap players talk about: **at 2,000 people a county births
2% and Winter kills 8%**, so it cannot grow past it no matter how happy it is.

**Deaths** come from the health band plus the season, added together. A Diseased county in
Winter loses **43%** of its people in a single season.

**Migration** moves people between neighbouring counties, driven by the happiness gap. The
formula makes small gaps produce *nothing*: a county at 72 next to one at 77 moves nobody at
all, which is why the England turn-one fixture's numbers work out with no migration in them.

Peasants are assigned to **nine jobs**: grain farming, cattle farming, field reclamation,
castle building, iron mining, stone quarrying, wood cutting, blacksmith, and **Idle
Townsfolk**. A county without a mine still shows the slot; it just produces nothing.

The village draws them as **eight** clusters of people standing around the picture, not
nine, because **iron and stone share one** — a county's mine and its quarry are painted at
the same spot, and which one you see is which one the county has. Idle Townsfolk is the
cluster in the middle.

One icon on screen stands for `ceil(population ÷ 25)` people, which is why a cluster has
twenty-five slots.

### Nobody chooses their own job

You *can* move people, by dragging a box round some icons and clicking another cluster. But
between seasons the game **reassigns everybody from scratch**, twice, and your drag only
lasts as long as the numbers it was based on:

1. Each county keeps a percentage split — three numbers across the farm jobs and five across
   the industry jobs, each set summing to 100 — and a single percentage saying how much of
   the county is industry at all. Dragging peasants rewrites all of them from where people
   actually ended up.
2. Each job also carries **two thresholds** the game works out for itself: how many workers
   it *wants* before it stops going backwards, and how many it can *usefully* take. Grain
   and cattle get real numbers by trying every possible staffing and seeing which pays;
   mines, quarries and forests are told "as many as you like"; a job whose resource the
   county has not got is told **nought**.
3. Then everyone is dealt out: each job gets its percentage of its half, or as much of it as
   its ceiling allows, and the leftovers are walked round the jobs that still have room —
   grain and cattle get five turns of the wheel to reclamation's one.
4. Whoever nobody can use becomes an **idle townsman**.

That last rule is the whole difference between a county you own and one you do not. An owned
county's forestry has no ceiling, so its spare people cut wood; an unowned county has no
forestry at all, so the same people stand in the square doing nothing. In the shipped save
that is 217 foresters and nobody idle, against nought foresters and 133 idle, from
identical populations.

**A job that is short is drawn short.** The workers it wants and has not got appear as extra
figures in the cluster that cannot be picked up, and the job's own panel prints its worker
count in red. Past the useful ceiling the surplus is drawn in the idle figure instead. Both
are how the interface says "you have this wrong" without a word of text.

---

## 5a. Two ways to lose a county without a battle

**Your empire must be in one piece.** At the end of every season the game works out which of
your counties are joined to which, and you keep only your **largest connected block, by
population**. Anything cut off from it declares independence that same season:

> *"Deeming itself too far from the heart of your empire, this county has declared
> independence and thrown out your officials."*

Two counties count as joined if they are on each other's **neighbour list** — the adjacency
the map was authored with — not merely because their tiles touch. If two blocks are equally
populous the game keeps the later one it found, which in practice means you cannot rely on
keeping the half you expect.

**This is the cheapest attack in the game and it is easy to miss.** Taking the one county
that *bridges* an enemy's territory does not just take that county: everything on the far
side of it goes free at the end of the same season, without a siege, without a battle, and
without costing you a man. A lord holding a long thin realm can be halved by one well-chosen
assault, and the counties that fall off do not become yours — they become neutral, so you
have to go back for them, but so does he. The same rule runs the other way: **do not let
anyone take a county in the middle of yours**, and think twice before pushing a lone army
deep behind someone's lines, because a county you capture out there is cut off from *your*
empire and secedes at the end of the season.

You get told — *"Your lands divide."* when it is more than one — but the AI lords lose theirs
in silence. Nothing else about the rule treats you differently from them.

> **How well is this known?** Less well than everything around it. Every saved game we can
> test has each lord holding exactly one county, so there is no position in which the rule
> could be observed doing anything. What it rests on is the code, the two messages above,
> and one player's memory of counties seceding in play. `docs/kingdom.md` §6.1.

**Or the peasants take it.** A human-owned county whose happiness stays under 25 climbs an
unrest counter one step a season, with a warning at each step — *"Murmurs of unrest."*,
*"Trouble in the county."*, *"Uproar in the shire."*, *"Revolution in your lands."* At the
fourth step **30% of the population walks out as an armed mob** and the county goes neutral.
The mob is unarmed peasants and it wanders the map like any other army.

An AI-owned county is judged on a different ladder — it only climbs below happiness 1, and
recovers between 11 and 40 — so the peasants rise against you far more readily than against
them.

**And your army can leave you.** Miss the wages and your mercenaries go at once; miss them
again and your men start deserting; miss them for six seasons and *"Furious at their ill
treatment, all your troops have deserted. All your armies are disbanded."* Paying in full
once resets the whole ladder.

---

## 6. How the score is calculated

Six weighted numbers, plus a bonus for gold. The weights are wildly uneven and that is the
point:

| what | weight |
|---|---:|
| **Castles held** | **×50** |
| Share of the map (percent of all counties) | ×10 |
| Mean county happiness | ×2 |
| Mean county health | ×2 |
| Total men under arms | ÷5 |
| Total population | ÷10 |

Then the treasury is added as a **bracket, not a rate**. The table clearly means three steps
— +50 over 2,000, +100 over 5,000, +200 over 10,000 — and **the shipped executable pays 50
for all three**:

| gold | bonus |
|---|---:|
| 0 – 2,000 | 0 |
| 2,001 and up | **+50** |

`Score_RankRealms` tests its *smallest* threshold first (`cmp gold, 2000; jle …`), adds the
50 and jumps straight to the next realm, so the 5,000 and 10,000 arms are reached only by a
treasury that has already failed `> 2000` — which is impossible. They are dead code. Verified
from the instruction bytes at `0x0049AED1`, not from the decompiler alone, and reproduced:
`l2_kingdom::tables::score_gold_bracket` pays 50 flat and carries the disassembly.

So hoarding past **2,000** crowns adds nothing at all, and the whole treasury is worth **one
castle**, not four.

**Castles outweigh everything else combined.** The standings screen shows the categories
separately — *Most counties, Most castles, Most troops, Most crowns, Happiest people, Most
people* — and then *Greatest noble* for the overall winner, or *undecided* for a tie.

---

## 7. The four AI lords

There are five realms and **four AI personalities**, because one realm is you. They are the
**Knight**, the **Baron**, the **Countess** and the **Bishop**, and they differ concretely:

| | best castle, and its price | builds at once | conscripts | offers alliance every |
|---|---|---:|---:|---:|
| Knight | royal, **10,000** | 4 | 30% | 12 turns |
| Baron | stone, 4,000 — never royal | 3 | 30% | 10 turns |
| Countess | stone, 2,000 — never royal | 2 | 40% | 8 turns |
| **Bishop** | **royal, 2,000** | **1** | **50%** | **4 turns** |

Each lord is offered only *some* castle types, and a type he is not offered is simply absent
from his ladder rather than priced at zero. The Knight builds palisades (200), Norman keeps
(1,000) and royal castles (10,000) and never a motte or a stone castle; the Bishop builds
Norman keeps at **100** and royal castles at 2,000 and nothing else.

**They are handed free gold every turn, and you are not.** Per turn, by difficulty:

```
Knight     0,  400,  700, 1200
Baron    100,  500,  800, 1400
Countess   0,  400,  700, 1200
Bishop   250,  600, 1100, 1800
You        0,    0,    0,    0
```

So the Bishop gets the most money *and* buys the most expensive castle at the lowest
threshold *and* builds only one at a time, concentrating it.

**The grants reward whoever is already winning.** This reads backwards to anyone expecting
rubber-banding, and it is verified in two separate tables:

- A realm holding **fewer than three counties** draws from a *smaller* gold table — the
  Bishop's 1,800 becomes 600. Being cornered gets you less, not more.
- The free people, cattle and grain go `d × 20 / 5 / 40` per county at one or two counties,
  halve at three or four, and stop **entirely** at five. A realm with no counties at all is
  gated out one level up and gets nothing, gold included.

### Diplomacy

Each pair of realms has a **standing**, −30 to +30. You can send seven things: a gift,
a compliment, an insult, an alliance offer, a termination, a request for help, or a request
to attack someone. Replies arrive on the recipient's next turn.

Three rules that catch people out:

- **Gold gifts ratchet.** Each gift is judged against the *largest you have ever sent*. A
  small follow-up gift costs you 8 standing — more than the best gift ever gains.
- **Compliments sour.** +15, then +8, then **−4 for every one after that, forever**.
- **An AI's opinion of you never heals.** It recovers +1 a turn towards other AIs and
  **never** towards a human player.

Alliances are exclusive — one at a time — and decay on their own through a grudge counter.
Two warnings, and then war permanently.

---

## 8. Battle

Battles are real-time. Your men are drawn as **figures**, each standing for several real
soldiers, grouped into **units**.

- **Recovery is the only melee defence.** There is no defence stat: a figure can only be hit
  once its recovery counter runs down, so slow-recovering troops are simply hit less often.
  Pikemen recover slowest and are the hardest to kill.
- **The heavy blow lands once per figure, for the whole battle** — 300 for macemen, 200 for
  knights, 100 for swordsmen, nothing for anyone else. At 100 hits per casualty, a maceman's
  opening swing kills three men outright.
- **Macemen and swordsmen have identical base damage.** They differ only in that opening blow
  (300 vs 100) and armour (12 vs 35) — glass cannon against durable.
- **Armour only stops missiles.** It is never read during a melee exchange.
- Siege engines take 160 hits per casualty where everyone else takes 100.

The AI runs on **one number**: its total strength as a percentage of yours, minus 100,
recomputed every 101 frames with a random jitter. Field units attack when it is above 5;
castle garrisons sortie only above 260, which is close to never.

---

## 8a. How a game ends, and what a campaign is

**A game ends one way: somebody runs out of everything.** There is no turn limit, no score
target and no date. A realm's *strength* is

> **strength = 3 × counties held + 1 × armies**

recounted at the top of **every** realm's turn — the human's included — and a realm whose
strength comes out zero is out of the game. Armies only: merchants, transports and peasant
mobs are in the same array and do not count.

The messages are asymmetric, and the split is on **"is this me"** rather than on "is this a
human":

| who died | what is raised |
|---|---|
| you | group 224, *"Defeat!"* — *"You have fallen from your once-mighty position…"* |
| an AI | group 194, *"Foiled again."* |
| another human, in a network game | **nothing at all** |

**You win when the last opponent's obituary is shown.** That is worth reading twice, because
it is not where you would look for it. The victory is not decided by a rule at the end of the
turn; it happens in the *message window*. When any elimination notice is displayed and no
realms but yours are left in play, the window enqueues group 225, *"Victory!"* — and when
*that* message is dismissed, the game is over. There is a second path in `Score_RankRealms`,
which crowns whoever is left when the ranking table has one live entry; if that realm is an
AI it gets group 195, *"Just call me king."*, once, and the *next* time the function runs it
sends you the Victory message instead — in a game you have already lost.

The outcome is one byte: **10 won, 11 lost**, and screen `0x1C` reads it.

### A campaign is eight maps and nothing carries between them

Pressing OK on the between-maps screen calls **`Game_NewGame`** — the same function the front
end calls. No gold, no armies, no counties, no diplomacy survives a map boundary. Three
numbers do: which of the two campaigns, how many of its maps you have won, and how the last
one ended.

The first campaign is **Quaintville, Rose, Ireland, Italy, England, France, Crusades,
Germany**; the second is **Australia, Central Am., S. America, U.S.A., Imperium, The World**
and is six maps because its counter starts at 2 and the same "counter < 8" test ends it. The
difficulty climbs 0, 0, 1, 1, 2, 2, 2, 2 and the starting purse is 5,000, 2,500 or 1,000 —
resetting to 5,000 at the top of each difficulty tier rather than falling all the way. A loss
does not advance the counter, so you fight the same country again.

---

## 8b. Sieges

**A county holding both a castle and a garrison cannot be walked into at all.** That single
rule is why sieges exist: the game's own errata say it in as many words — *"If a garrisoned
castle is present in the county, it must be attacked instead of the county town to gain
control of the county."* Run out of undefended counties and you have run out of ways to
win.

**Siege engines are built on the spot, not carried.** You march up, and the whole army
spends its seasons building:

| | man-seasons each | most you may order |
|---|---:|---:|
| catapult | 200 | 4 |
| siege tower | 200 | 4 |
| battering ram | 400 | 2 |

A season of work is **one point per man**, so the wait is `ceil(work ÷ men)`. A 43-man army
building one catapult waits five seasons; a 400-man army building two towers is ready next
season. Ordering more of one kind is not free and ordering a cheaper kind is not weaker —
each row maxes out at the same 800 man-seasons.

**The army is pinned while it builds**, in the sense that matters: *any* successful move
order lifts the siege. The *"Lift the siege?"* prompt is a warning, not a veto.

**Castles below a Norman keep can be stormed bare-handed; nothing above one can.** Order no
engines against a stone or royal castle and your captains refuse outright — *"you must
build some siege engines to besiege this castle"* — and the siege is lifted, not stalled.

**The castle is worth 160 %, 200 %, 250 %, 320 % or 400 %** of the garrison's strength, by
castle size, when a siege is settled by arithmetic rather than fought. That multiplier is
the whole of what a castle is worth on the strategic layer, and it is steep: 43 men against
a 149-man garrison in the *smallest* castle in the game lose without killing 20 of them.

**The defenders get boiling oil** — 1, 2, 3, 4 or 6 pots by castle size — and only the
defenders. The attacker's engines and the defender's oil exist for the length of the battle
and no longer; nothing survives it.

**In the battle itself there are three ways to take a castle.** Break the gate (20,000 hits,
one-shot — a battering ram is worth 20 men a frame at it, so one ram opens a gate in a
thousand frames where a lone swordsman needs twenty thousand); chew through the rampart
(5,000 hits a patch, and the counter resets, so a wall can be breached repeatedly); or
simply **get one man to the keep's door**, which ends the siege with the garrison
untouched. And if the attacker runs out of engines with no breach, a small castle sends him
back to try again and a large one has beaten him.

**Losing an assault does not destroy the besieging army.** It loses its siege and keeps its
men — which is why sieges are attritional rather than all-or-nothing.

**The AI lords each besiege differently**, and it is fixed per lord rather than judged:

| | orders | man-seasons |
|---|---|---:|
| the Knight | four siege towers | 800 |
| the Baron | one ram, two towers | 800 |
| the Countess | three catapults, two towers | 1,000 |
| the Bishop | the same | 1,000 |

The Countess and the Bishop add a ram against a stone or royal castle late in the year,
taking them to 1,400. **The Knight is the only lord who brings no artillery.**

---

## 9. Where this is *not* the whole story

Things the engine does not yet do, so this document describes the original rather than us:
the castle designer, the castle's own layout on the battlefield (ours is a plain concentric
keep and is marked as ours wherever it is drawn), most of the interface, and merchants and
transports as things that move. `docs/mechanics.md` tracks what is implemented against what
is merely known.

And a standing caution, learned twice the hard way: **the England turn-one fixture exercises one narrow
slice of these rules.** Every county in it sits at tax rate 0 with a well-staffed herd, which
is exactly why two wrong rules survived **932 passing tests** — a frozen figure, the size of
the suite on the day it happened (`docs/decisions.md` C26), not a current count. A rule that looks right in the
save may still be wrong everywhere else.
