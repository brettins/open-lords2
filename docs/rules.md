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
population growth reads the happiness this turn already wrote. Twenty-five passes, in this
sequence:

| # | pass | what it does |
|---:|---|---|
| 1 | Clock | advance season; roll the year if Winter is beginning |
| 2 | Events | deal one random event per eligible county |
| 3 | Weather | roll each county's weather for the season |
| 4 | **Tax** | collect; write the tax happiness terms |
| 5 | Wages | pay the army |
| 6 | **Rations** | feed everyone; this is where food is actually spent |
| 7 | Health | move each county's health meter by how well it ate |
| 8 | **Happiness** | sum the terms into one number |
| 9 | Unrest | move each county towards or away from revolt |
| 10 | Fertility | age the soil |
| 11 | Field reclamation | turn wasteland into usable fields |
| 12 | **Grain** | sow, grow or harvest, depending on the season |
| 13 | **Herd** | cattle births and deaths |
| 14–17 | Industry | weapons, then iron, then stone, then wood — **in that order** |
| 18 | Castle building | advance anything under construction |
| 19 | **Labour** | reassign every county's peasants to jobs, from scratch |
| 20 | Migration | move people between neighbouring counties |
| 21 | **Population** | births and deaths |
| 22 | Score | rank the realms |
| 23 | **Labour, again** | and once more, now that the newborns and the levies are counted |
| 24 | History | write this season's line into the 400-season ring |
| 25 | Ration preview | recompute each job's thresholds and what next season *would* cost |

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

Feeding a county happens in one pass, and it works from the top down:

1. Work out what the county needs: `population ÷ divisor × multiplier`, where the divisor and
   multiplier come from the ration level. Normal rations need one sack per ten people.
2. **Dairy first.** Every head of cattle feeds **5 people**, free, without being killed.
3. **Then grain**, from the store.
4. **Then slaughter.** Any shortfall is made up by killing cattle, at 10 people per head.
5. If it still cannot be fed, the ration level drops and everyone is unhappier.

**The player can reorder that list.** The ration screen says *"Click on a food to swap its
priority"* — the eating order is a setting, not a constant. We do not model this yet.

The same screen reports each food in turn, and it names **five**, not three: *"Dairy produce
feeds …"*, *"Grain feeds …"*, *"Sheep feed …"*, *"Cows feed …"*, and *"Barrels swilled."*
So ale is consumed alongside the food rather than merely bought.

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
| 1–10 | *Low herd crowding* | 14% | 0.01% |
| 11–20 | *Average herd crowding* | 9% | 0.03% |
| 21–30 | *Herd overcrowded* | 5% | 0.05% |
| 31+, or no pasture | *Massive overcrowding!!* | 2% | 0.07% |

That is the whole point of the table: **an overcrowded herd dies seven times as fast and
breeds a seventh as often.** Small herds that are fully staffed also get a birth bonus on
top, so a handful of well-tended cattle recovers much faster than the percentage suggests.

Crowding is also drawn on the map, so you can see it without opening a panel.

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

The same click switches **industries** on and off: clicking a mine, quarry, forest or smithy
on the map toggles it, and there is no other way to do it either.

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

Then the treasury is added as a **bracket, not a rate** — four steps, and nothing above them:

| gold | bonus |
|---|---:|
| 2,001 – 5,000 | +50 |
| 5,001 – 10,000 | +100 |
| over 10,000 | +200 |

So hoarding past 10,000 crowns adds **nothing at all**, and the whole treasury is worth at
most four castles.

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

## 9. Where this is *not* the whole story

Things the engine does not yet do, so this document describes the original rather than us:
the castle designer, sieges, most of the interface, merchants and transports as things that
move, and the fourteen siege battle orders. `docs/mechanics.md` tracks what is implemented
against what is merely known.

And a standing caution, learned twice the hard way: **the England turn-one fixture exercises one narrow
slice of these rules.** Every county in it sits at tax rate 0 with a well-staffed herd, which
is exactly why two wrong rules survived 932 passing tests. A rule that looks right in the
save may still be wrong everywhere else.
