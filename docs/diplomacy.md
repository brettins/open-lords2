# Diplomacy, messages and the four lords

The subsystem a player named in one sentence and no document here had a row for:

> *"You can message nobles and form alliances, send them gold and they respond with various
> messages (which have voice acting sound files). The bishop always tries to build really big
> castles, I think the baron makes peasant armies."*

All of it is real, all of it is in the binary, the bishop half of the prediction is
**confirmed at the instruction stream**, and the baron half is now **contradicted** by it —
his weapon rota contains no crossbows at all (§8.3). `docs/mechanics.md` had diplomacy as a
single `🕳 gap` line; this is what was behind it.

Evidence legend, as everywhere here:

| | meaning |
|---|---|
| **[V]** | verified — read out of the binary *and* closed by an independent check (arithmetic, a shipped file, a second source) |
| **[D]** | decompiled — read off the decompiled C, one source, no second reading |
| **[I]** | inferred — consistent with everything measured, not proven |

Nothing in this document was obtained by running the game. Every claim is static: the file,
the instruction stream, and the shipped `.eng` and `.wav` inventories.

> **§10 is what happened when it was implemented**, and it corrects nine things in the
> sections below. Read it before trusting a `[D]` here: implementing a document is the only
> way to find out whether it is true, and this one was 90 % right and wrong in nine places
> that each change behaviour. Every correction is marked at the section it belongs to as
> well, so a reader who never reaches §10 is not misled.

---

## 0. The headline

**There are exactly four AI lords, and the game says so in four independent places.**

| lord byte (realm `+0x07`) | `L2.eng` group 7 | voice prefix | message variants | personality record |
|---:|---|---|---:|---|
| 0 | *No player* — the human | — | — | — |
| 1 | **The Knight** | `Kt` | 0–3 | `0x004D8A58` |
| 2 | **The Baron** | `Bn` | 4–7 | `0x004D8B48` |
| 3 | **The Countess** | `Ct` | 8–11 | `0x004D8C38` |
| 4 | **The Bishop** | `Bp` | 12–15 | `0x004D8D28` |

**[V]**, and this is the one claim here with four independent confirmations, because
everything else hangs off it:

1. **Arithmetic.** Every diplomatic message is enqueued with the variant index
   `lord × 4 + rot − 4`, where `rot` is that realm's own 0…3 rotation counter (realm
   `+0x159`). For lords 1…4 that is exactly 0…15, in four contiguous blocks of four.
2. **The shipped filenames.** `Msg_PlayVoice` (`0x004B35C1`) indexes 16-byte filenames at
   `g_msgVoiceLord` (`0x004E0458`) by `(group − 170) × 0x100 + variant × 0x10`. Variants
   0–3 read `Kt<group>_1..4.wav`, 4–7 `Bn…`, 8–11 `Ct…`, 12–15 `Bp…`.
3. **The install.** 448 of those files exist — `Kt` 112, `Bn` 112, `Ct` 112, `Bp` 112 — which
   is 28 groups × 4 variants each, exactly the range the code guards (`170 ≤ group ≤ 197`).
4. **The prose.** Of 48 lines in groups 170–197 mentioning the Church, the Almighty, *"my
   son"*, the Council or the elders, **46 sit at string indices 13–16** — the Bishop's block.
   *Queen* and *my Lady* appear only at 9–11 (the Countess); jousting, flails and *"the
   strongest man I have ever seen"* only at 1–4 (the Knight).

Point 4 alone would be the C3 error — four tones matched to four remembered archetypes. It is
here only as a check on points 1–3, which are arithmetic and filenames.

A fifth confirmation turned up while checking something else, and it is the strongest of all:
**new-game setup names the lords itself.** It calls `Eng_Seek(7, realm[+0x07])` and copies 16
bytes into `g_playerNames`, so the string the player sees beside a rival's portrait *is* group
7 indexed by the lord byte. **[V]**

### 0.1 The lord is not the realm, and it is not the colour

This is the trap, and it is worth stating before anything is built on the tables.

**`g_aiPersonality` and `g_aiGoldGrant` are indexed by the lord byte, not by the realm
index.** The expressions are literal:

```c
g_aiGoldGrant + g_optDifficulty*4 + realm[+0x07] * 0x10          /* AI_SetTaxRates */
g_aiPersonality + (realm[+0x07] * 3 - 3) * 0x50                  /* everywhere else */
```

`realm[+0x07]` runs 0 (the human), 1…4 (the lords), 6 (eliminated). So **row 0 of both gold
tables is lord 0 — the human — and it is all zeros**, and row 4 is the Bishop. Five rows and
five realms is precisely the coincidence `decisions.md` C3 warns about; the tables have five
rows because the *lord byte* has five values, and the fifth AI row people keep looking for does
not exist because the lord byte does not have a fifth AI value.

**Which lord a realm gets is drawn at new game and is not fixed.** Setup hands each AI realm a
**colour slot** (`realm[+0x0A]`, 1…5, in realm order), then picks its lord from
**`g_lordChoice` (`0x004DC17C`)** — four scenario groups × five colour slots × four candidate
lord ids — taking the first candidate not already used. Every row but one is a permutation of
{1,2,3,4}, and 4 × 0x14 = 80 bytes ends at `0x004DC1CC` where the colour pairs begin. **[V]**

So a realm's **colour** comes from `+0x0A` and its **lord** from `+0x07`, and they are related
only through that candidate table. *"The bishop is purple"* can be a real tendency in a given
scenario — the Bishop heads the candidate list for slot 4 in scenario group 0 and for slot 2 in
group 2 — but it is a bias, not an identity, and **nothing may be indexed by colour or by realm
index that the code indexes by lord.**

**`g_aiPersonality` therefore has four records, and `docs/kingdom.md` §8.2's open question is
closed.** The records are 240 bytes (`0x50 × 3`, only the first row used) from `0x004D8A58`,
so 4 × 240 lands on `0x004D8E18` — and what begins there is a **campaign-progression table
with a `0x20` stride**: `FUN_00499E5D` reads `g_scenarioIndex` from its `+0x00`,
`g_optDifficulty` from its `+0x04`, and further fields from `+0x14`. It is not a fifth
personality record. **[V]** — the arithmetic closes on a differently-strided table that has an
identified reader, which is stronger than §8.2's *"fits no pattern"*.

---

## 1. Where the standing lives

**There is a per-pair standing, and it is inside the realm record.**

`g_realms` is `0x0057BF00`, stride `0x160` (`docs/kingdom.md` §2). Bytes `+0x84 … +0xE3` of
each record are **six 16-byte sub-records, one per other realm**, addressed as
`realm[me] + 0x84 + them × 0x10`. Slot 0 is never used, so five are live.
`0x84 + 6 × 0x10 = 0xE4`, and `+0xE4` is referenced nowhere in the binary while `+0xE5` is the
AI's chosen muster county — so **the block closes exactly**. **[V]**

`Diplo_Init` (`0x004A1C53`) writes every field of it once, which is what makes the map below
a reading rather than a guess.

| off (from `+0x84`) | type | name | Ev | meaning |
|---|---|---|---|---|
| `+0x00` | i8 | **standing** | [V] | how `me` feels about `them`. Initialised to **5** for an in-play AI realm and **0** for a human or a dead one. Clamped to **[−30, +30]** at every write site. |
| `+0x01` | u8 | allied | [V] | 1 while the two are allied; set and cleared in pairs. |
| `+0x02` | u8 | grudge | [V] | accumulates while allied; past the lord's threshold the alliance breaks. §4.1. |
| `+0x03` | u8 | warningsSent | [V] | 0 → 1 → 2 → 3, the *Warning / Warning / Notice of revenge* ladder. §5. |
| `+0x04` | u8 | **atWar** | [V] | set when the ladder tops out, or when an act breaks an alliance. Permanently blocks alliance offers. |
| `+0x05` | u8 | complimentsFrom | [V] | how many compliments `them` has sent `me`. Never reset. §3.2. |
| `+0x06` `+0x07` | u8 | — | [D] | **never initialised, never read.** Padding. |
| `+0x08` | i32 | **bestGift** | [V] | the largest single gift `me` has received from `them`. Ratchets up, never down. §3.1; `L2.eng` group 72 index 23 is *"Last gift was"*. |
| `+0x0C` | u8 | hasMail | [V] | a message from `them` is waiting in my inbox; drawn as an icon and cleared when the inbox is answered. |
| `+0x0D` | u8 | **helpPriceMultiple** | [V] | initialised to **1**, incremented every time `me` is paid to help `them`. The price of help is `personality[+0x0C] × this`. §3.5. |
| `+0x0E` | u16 | — | [D] | zeroed at init, read nowhere. |

**The standing is asymmetric.** `realm[a]` holds a's view of b, `realm[b]` holds b's view of a,
and nearly every rule moves only one of them. **[V]**

### 1.1 What the player sees of it

`Diplo_DrawLordCard` (`0x004171EE`) draws a **10 × 63 vertical thermometer** per rival at
x = `0x88`, filled from +30 down to the standing, in one of three colours:

| standing | palette index |
|---|---|
| ≥ **+11** | `0xFA` |
| −10 … +10 | `0xFC` |
| ≤ **−11** | `0xF9` |

**Those two break points are exactly the AI's two alliance decisions.**
`Diplo_ReplyAllianceOffer` accepts outright at ≥ +11, refuses outright below −10, and rolls
dice in between. The UI's three bands and the AI's three branches are the same numbers written
by different code. **[V]** — and it is the best corroboration in this document that `+0x84` is
what it looks like.

### 1.2 Realm-level diplomacy state

| off | type | name | Ev | meaning |
|---|---|---|---|---|
| `+0x07` | u8 | lord | [V] | 0 human, 1…4 the four lords, 6 eliminated. §0. |
| `+0x0A` | u8 | shieldIndex | [D] | clamped 1…5; selects `Misc_cty.pl8` frame `0x55 + n` and a 16-entry palette row. The realm's banner colour, used on the map too. Listed because the lord card draws it. |
| `+0x1C` | u8 | offerPending | [D] | an alliance offer to a human is outstanding; cleared at the top of that realm's turn. |
| `+0x1E` | u8 | — | [D] | cleared by both alliance functions and **read nowhere**. Dead. |
| `+0x80` | u8 | allyCandidate | [D] | who `AI_Diplomacy` has decided to court. |
| `+0x81` | u8 | **ally** | [V] | the realm's ally, 0 for none. **One byte, so one ally at a time.** |
| `+0xE9` | u8 | tauntTimer | [D] | counts to 8 before a taunt is sent. §6. |
| `+0xEA` | u8 | tauntStage | [D] | 0 → *"How are you doing?"*, 1 → *"Helpful advice."* |
| `+0xEB` | u8 | warTarget | [D] | the realm the AI has resolved to attack. |
| `+0xEC` | i8 | offerTimer | [D] | counts to `personality[+0x14]` between alliance offers. |
| `+0xED` | u8 | crownedOnce | [D] | one-shot guard on the *"Just call me king."* message. |
| `+0x118` | i32 | gold | [V] | the treasury; gifts move it directly. |
| `+0x159` | u8 | **voiceRotation** | [V] | 0…3, incremented and wrapped after every message this realm sends. Picks which of the lord's four recorded takes plays. |

---

## 2. The machinery: an inbox, a queue and a ring of voices

Three separate stores, and conflating them is the easy mistake.

```
  player's UI ──Diplo_Post──► g_diploInbox[to][0..4]        one turn of latency
                              0x0053F0F0, 5 slots x 8 bytes per realm
                                     |
                                     |  AI turn step 1, next turn
                                     v
                              Diplo_AnswerInbox  ──►  a reply group is chosen
                                     |
                                     v
                              Msg_Enqueue  ──►  g_messageQueue[head++]
                                                0x00568480, 50 x 0x18, wraps at 50
                                     |
                                     v
                              Msg_Pump  ──►  Msg_DrawWindow + Msg_PlayVoice
```

### 2.1 The inbox — `g_diploInbox`, `0x0053F0F0`

Six realms × five slots × 8 bytes = 240 bytes. **[V]**: the realm stride `0x28` and the slot
stride 8 are literal in all three functions that touch it, and all three walk `0..4`.

| off | type | meaning |
|---|---|---|
| `+0x00` | u8 | **sender realm.** 0 terminates the walk; slots are filled at the first free index, so the array stays dense. |
| `+0x01` | u8 | **message kind**, 0…6. §3. |
| `+0x02` | u8 | a county id, for kinds 5 and 6. |
| `+0x04` | i32 | gold, for kind 0. |

`Diplo_Post` (`0x004A2621`) is the only writer. It finds the first free slot, stores the four
fields, bumps `pair[to][from].complimentsFrom` when the kind is 1, sets
`pair[to][from].hasMail`, and — if the gold field is non-zero — **moves the gold immediately**,
clamped to what the sender actually holds. **A gift is spent when it is posted, not when it is
answered.** **[V]**

`Diplo_AnswerInbox` (`0x004A277D`) is **AI turn step 1** (`docs/kingdom.md` §3.2). It walks the
five slots, dispatches on the kind byte, then zeroes all five slots *and* the six `hasMail`
bytes. **A realm's inbox is emptied every turn whether or not it was full.** Because the human
realm's AI turn is skipped entirely (realm `+0x05`), **a message posted to a human realm's
inbox would never be answered** — in single player nothing posts one, and in multiplayer the
send goes through the netcode instead (§7). **[D]** on that consequence.

### 2.2 The queue — `g_messageQueue`, `0x00568480`

`Msg_Enqueue` (`0x00472BC5`) is the **general** notification path — 144 call sites, most of
them nothing to do with diplomacy. It assembles a 24-byte record at `g_messageCompose`
(`0x00553510`) and copies it into a 50-slot ring.

| off | type | meaning |
|---|---|---|
| `+0x00` | i32 | recipient realm |
| `+0x04` | i32 | sender realm (0 = the game itself) |
| `+0x08` | i32 | **`L2.eng` group id** — a message *is* its group number |
| `+0x0C` | i32 | **variant**, `lord × 4 + rot − 4` for a lord's letter; 0 for system messages |
| `+0x11` | u8 | **category** — selects the window layout. 1 for a plain lord letter, `0x0B` for the alliance prompt, 10 for the pay-for-help prompt; 0, 3, 4, `0x0C`, `0x0E`, `0x0F`, `0x13` for non-diplomatic kinds |
| `+0x12` | u8 | a county id, where the text needs one |
| `+0x13` | u8 | spare |
| `+0x14` | i32 | a numeric payload |

**A message is enqueued only when its recipient is the local player** (or realm 0): the guard
resolves to `to == 0 || to == g_localPlayer` down all three of its branches, so the sender
tests inside it are vestigial. **[V]**

`Msg_Pump` (`0x00472E46`), called from the frame loop, pulls from the tail
(`g_messageQueueTail`, `0x00553EC4`), sets `g_messageGroup` and `g_messageVariant`, starts a
**2000-tick display timer** (`g_messageTimer`, `0x0057C954`) and calls `Msg_DrawWindow`
(`0x0047309E`, 10,915 bytes). It only runs on screens `0x00` (the campaign map), `0x27`,
`0x29`, and `0x0F` when the job panel is on job 8. **[D]**

The window draws the heading with `Eng_DrawString(group, 0, …)` and the body with
`Eng_DrawString(group, variant + 1, …)` — **which is why the Bishop's variants 12…15 are
strings 13…16, and why the Church histogram in §0 lands where it does.** **[V]**

### 2.3 The voices — `Msg_PlayVoice`, `0x004B35C1`

Four filename tables, and their extents close by arithmetic.

| groups | table | index | first name |
|---|---|---|---|
| 100 … 169 | `g_msgVoice100` `0x004DF9B8` | `(g − 100) × 0x10` | `S100_01.wav` |
| 200 … 284 | `g_msgVoice200` `0x004DFE18` | `(g − 200) × 0x10` | `S200_01.wav` |
| **170 … 197** | **`g_msgVoiceLord` `0x004E0458`** | `(g − 170) × 0x100 + variant × 0x10` | `Kt170_1.wav` |
| — | `g_msgVoiceS010` `0x004DF7B8` | `n × 0x10`, `n` 0…15 | `S010_01.wav` |

`0x004DF9B8 + 70 × 0x10 = 0x004DFE18`: the first table ends exactly where the second begins.
`0x004E0458 + 28 × 0x100 = 0x004E2058`, and a different filename table (`S020_01.wav`) begins
exactly there. **[V]**

**So a diplomatic voice line is one line of arithmetic:**

```
wav = "<Kt|Bn|Ct|Bp><group>_<rot+1>.wav"        group 170..197, rot 0..3
```

All 448 files are present in the install, and the three system tables hold one file per group
with no lord in them at all. **[V]**

**Only groups 170…197 are spoken by a lord.** That range is also exactly the set of `L2.eng`
groups with **17 strings — a label at index 0 and sixteen lines** — and groups 166–169 and 200
either side of it have 5, 2, 2, 2 and 4. Three unrelated sources agree on the same 28 groups.
**[V]**

---

## 3. The seven things you can send

`L2.eng` **group 72 is the diplomacy screen**, and its menu items are the message kinds in
order: the kind byte is `menuIndex − 2`. **[V]**

| kind | menu label (group 72) | inbox payload | handler | reply groups |
|---:|---|---|---|---|
| 0 | *Dispatch a gift.* | gold at `+0x04` | `0x004A29B2` | 171 / 172 / 173 |
| 1 | *Send a compliment.* | — | `0x004A2CD6` | 174 / 175 / 176 |
| 2 | *Send an insult.* | — | `0x004A2F61` | 177 |
| 3 | *Offer an alliance.* | — | `0x004A30F6` | 178 / 179 / 196 / 197 |
| 4 | *Terminate alliance.* | — | `0x004A3475` | **none** |
| 5 | *Ask ally for help.* | county at `+0x02` | `0x004A3551` | 183 / 184 / 185 |
| 6 | *Ask ally to attack.* | county at `+0x02` | `0x004A38DB` | 186 / 187 / 188 |

Each handler's *identity* is **[V]**, because the group it sends carries a label at index 0
that names it — *"Reply to gift."*, *"Reply to compliment."*, *"Reply to Insult."*, *"Reply to
alliance offer."*, *"Help in"*, *"Pay -"*, *"Attack of"*. Each handler's *arithmetic* is
**[D]**.

> **Correction (§10.1): only three of the seven have the `isHuman` guard.** This paragraph
> used to say *"a reply is produced **only if the sender is human**, so AI-to-AI diplomacy is
> entirely silent"*. `Diplo_ReplyGift`, `Diplo_ReplyCompliment` and `Diplo_ReplyInsult` open
> with `if (g_realms[them].isHuman != 0)`; **`Diplo_ReplyAllianceOffer`,
> `Diplo_ReplyAllianceEnd`, `Diplo_ReplyHelpRequest` and `Diplo_ReplyAttackRequest` have no
> such test**, so all four would act on an AI's letter — form the alliance, break it, march.
> Nothing posts one in single player, so the *consequence* stands; the *reason* was wrong,
> and it is the reason a netcode seam would have relied on.

One fact shared by all of them: every reply advances the sender's `voiceRotation`, so the
four recorded takes cycle rather than repeat. **[V]**

### 3.1 Gold — and the ratchet nobody would guess

Let `T = personality[+0x08]` and `best = pair[me][them].bestGift`:

| gift | tier | reply group | **standing** |
|---|---|---|---:|
| `< best + T/2` | contemptuous | 173 | **−8** |
| `< best + T` | grudging | 172 | **+5** |
| `≥ best + T` | pleased | 171 | **+10** |

then `best = max(best, gift)`. **[D]**

`T` by lord: Knight 100, Baron 100, Countess 200, **Bishop 50**.

Three consequences, none of them in the manual. **The bar ratchets** — every gift is judged
against the largest you have ever sent, so the second must beat the first. **A gift can lose
you standing**: anything under half the increment costs 8, which is more than the best gift
gains. And the Bishop is the cheapest lord to please (`T = 50`) *and* the one with the fattest
cheat income (§8.2) — he is easy to buy and does not need buying.

### 3.2 Compliments — three and you have overdone it

`n = pair[me][them].complimentsFrom`, incremented by `Diplo_Post` before the handler runs, and
**never reset**:

| n | reply | standing |
|---:|---|---:|
| 1 | 174 | **+15** |
| 2 | 175 | **+8** |
| ≥ 3 | 176 | **−4** |

**[D]**, and group 176 says the same in words: *"You are boring me now with your groveling
letters. Do not send me any more."* So compliments are worth +23 in total, once, and cost 4
apiece forever after.

### 3.3 Insults, and ending an alliance

Kind 2 breaks any alliance between the two, costs **−20**, and replies with group 177.
Kind 4 breaks the alliance, costs **−15**, and **sends nothing at all** — terminating an
alliance from the diplomacy screen is silent. **[D]**

The *"Broken alliance."* text, group 182, is not this. It comes from `Diplo_Offend` when an
*act* breaks an alliance — §5.

### 3.4 Alliance offers

```c
if (pair.atWar)                       reply 196;             standing -1
else if (g_realmsActive < 3)          reply 178;             standing -2
else if (realm[me].ally)              reply 197;             standing -1
else if (realm[them].ally)            reply 178;             standing -2
else if (standing >= 11)              reply 179;  ALLY;      standing +4
else if (standing < -10)              reply 178;             standing -2
else if (rand7B < standing*3 + 45)    reply 179;  ALLY;      standing +4
else                                  reply 178;             standing -2
```

**[D]**. `rand7B` (`g_rand7B`, `0x0058FD60`) is `randStateB & 0x7F`, so 0…127: the middle band
runs from about **12 %** at standing −10 to about **59 %** at +10, hinging on 45/128 ≈ 35 % at
0. Group 196 is labelled *"Retort to alliance offer."* and 197 *"Reply to alliance offer."* —
the two "I cannot, and here is why" refusals.

`g_realmsActive` (`0x00554004`) is **[D]** with a caveat: two functions write it, one counting
realms still in play and one counting realms that have not yet finished their turn, and the
second runs every frame of phase 4. Whichever wrote last is what `< 3` reads. Both readings
mean *"the game is nearly over"* and the branch refuses either way, so nothing here depends on
resolving it — but it is not resolved.

### 3.5 Asking an ally for help, or for an attack

Both have the same shape; only the constants differ.

```c
if (realm[me].ally != them)                    reply refuse
else if (realm[me].gold < personality[+0x30])  reply refuse
else if (standing < 10)                        reply refuse
else if (rand7B < standing * K)                reply accept
else                                           reply "pay me"
```

| | K | refuse | accept | pay | refusal grudge |
|---|---:|---|---|---|---:|
| help (kind 5) | **4** | 183 | 184 | 185 | +1 |
| attack (kind 6) | **2** | 186 | 187 | 188 | +2 |

**[D]**. `personality[+0x30]` is 750 / 800 / 900 / 1000 by lord, below which an ally will not
move at all.

> **Correction (§10.2): it is a *population* floor, not a treasury one.** The comparison is
> `g_realms[me].populationMean < personality[+0x30]` — realm `+0x14`, the mean population of
> the realm's counties. §8.4's own note that the same field is *"also a county-population
> floor at step 9"* is the corroboration: it is a population floor in both places. So the
> pseudo-code's second line reads `realm[me].populationMean`, not `realm[me].gold`, and an
> ally with a full treasury and empty villages refuses.

*Accept* calls `Diplo_PayForHelp` with a price of **zero**: the ally's `warTarget` becomes your
county and it marches. *Pay me* puts `personality[+0x0C] × pair.helpPriceMultiple` into
`g_diploHelpPrice` (`0x00567958`) and shows a prompt (category 10); clicking it runs
`Diplo_PayForHelp` for real — the gold moves, the multiple increments, and **your standing with
them drops 4** for having made them do it.

Base help price by lord: Knight 500, Baron 1000, Countess 1600, Bishop 1500. It **doubles,
trebles, quadruples** with each purchase, because the multiple starts at 1 and never falls.
**[D]**

---

## 4. What an alliance actually is

`Diplo_FormAlliance` (`0x004A1774`) writes six bytes and nothing else: `pair.allied = 1` and
`pair.grudge = 0` in **both** directions, and `realm.ally` on both sides.
`Diplo_BreakAlliance` (`0x004A1A54`) undoes it. **[V]**

So mechanically an alliance is:

* **exclusive** — `realm[+0x81]` is one byte, so a realm has at most one ally, and both
  alliance paths refuse when either side already has one;
* **the gate on kinds 5 and 6** — only an ally can be asked for help or for an attack;
* **a shield against your own actions counting** — `Diplo_ActionAllowed` (`0x004A16F7`)
  returns 0 when the target is your ally, which suppresses the offence hook (§5) and adds 1 to
  their grudge instead;
* **shown on the map and the lord card** — allied and at-war icons, and the county strip's
  owner colour.

It is **not** shared vision, shared victory, or an automatic call to arms. Help is a request
that can be refused or invoiced.

### 4.1 An alliance decays on its own

`AI_Diplomacy` (`0x004A0C1D`) is **AI turn step 2** and it is the real diplomacy driver.
`docs/kingdom.md` §3.2 summarised it as *"age each rival's grudge counter"*; here is what it
does.

**First, standing heals — but not towards you.**

```c
for (other = 1; other < 6; other++)
    if (!realms[other].isHuman)
        pair[me][other].standing = min(pair[me][other].standing + 1, +30);
```

The guard is on the **other** realm being non-human. **An AI's standing towards a human player
never drifts back up.** Damage you do is permanent; damage the AIs do to each other heals at a
point a turn. **[V]** — one loop, one guard, and no second reading is available.

**Then, if allied, the grudge accumulates:**

| condition | grudge |
|---|---:|
| standing towards the ally < −10 | **+10** |
| `g_realmsActive < 3` | **+25** |
| the ally is the top-ranked realm (`g_rankLeader`) and its `+0x60` ≥ 21 | **+1** |

and when the grudge exceeds `personality[+0x10]` — Knight **5**, Baron **10**, Countess **15**,
Bishop **20** — the alliance breaks, `Diplo_Offend(me, ally, 5)` fires, and group **181**
(*"End of alliance."*) is sent. **[D]**

> **A bug: two of the three envy tiers are unreachable.** The ally-is-winning test reads
> ```c
> if (v < 0x15) { if (v < 0x22) { if (0x32 < v) grudge += 4; } else grudge += 2; }
> else grudge += 1;
> ```
> `v < 0x15` implies `v < 0x22`, which makes `grudge += 2` dead, and implies `!(0x32 < v)`,
> which makes `grudge += 4` dead. Only `v ≥ 0x15 → +1` can fire. The ladder was written with
> its comparisons the wrong way round. **[V]** — arithmetic over three constants, not a
> reading.

**And if not allied, the AI goes courting.** From year 1269, if the realm is not ranked 1st,
`Diplo_PickAllyCandidate` (`0x004A1241`) picks the **best-ranked** realm that is in play, not
ranked 1st, unallied, not already being courted, not at war with it, and whose standing is
above −10. A counter must then reach `personality[+0x14]` — Knight **12**, Baron **10**,
Countess **8**, Bishop **4** turns — before it acts. Against another AI it simply forms the
alliance with no message; against a human it sets `offerPending` and sends group **180**,
*"Accept alliance ?"*, with category `0x0B`, the prompt layout. **[D]**

`Diplo_ReconcileAlliances` (`0x004A1847`), called from `Turn_Tick`, rebuilds the `allied`
matrix from the `ally` bytes. **[D]**

> **Correction (§10.3): it *repairs* a one-sided pairing rather than dropping it.** This
> paragraph used to say it *"drops any pairing that is one-sided or whose partner has been
> eliminated"*. The test is
> `if (handled[partner] || (ally[partner] != 0 && ally[partner] != me))` — so a partner that
> points at **nobody** falls into the `else`, where the function **writes `ally` back onto
> the partner** and sets `allied` in both directions. Only a partner already pointing at a
> *third* realm, or one already marked dead by this pass, drops it. And "already marked dead
> by this pass" is an ascending walk over an array it is filling, so it can only see realms
> *below* the one being examined: **an alliance with a higher-indexed eliminated realm
> survives reconciliation.** `docs/bugs.md`.

---

## 5. What makes an AI hate you

`Diplo_Offend(offended, offender, amount)` (`0x004A1EE1`) is the single hook. Everything that
damages a relationship goes through it. **[D]**

```
if the offender was my ally:
      break the alliance
      set pair.atWar = 1                  -- UNLESS I am the Bishop (lord == 4)
      set my warTarget, if not already set
      Diplo_OffendAll(offender, 15)       -- everyone else's opinion of them drops 15
      send group 182 "Broken alliance."
      amount += 15

standing -= amount, clamped to [-30, +30]

if not already at war, standing has bottomed out at -30, and the offender is HUMAN:
      warningsSent 0 -> 1:  group 189  "Warning."
      warningsSent 1 -> 2:  group 190  "Warning."
      warningsSent 2 -> 3:  group 191  "Notice of revenge."   and pair.atWar = 1
```

**Two warnings, then war.** The war flag is what `Diplo_ReplyAllianceOffer` reads to send group
196, so once an AI is at war with you it will never accept an alliance again, at any standing.

> **Correction (§10.4): the whole function is a no-op when the offended realm is human.** The
> entry guard is `0 < offended < 6 && 0 < offender < 6 && offender != offended &&
> realms[offended].strength != 0 && **realms[offended].isHuman == 0**`. A person's realm
> therefore keeps **no standing towards anybody**, which is why `Diplo_Init` opens a human's
> row at 0 and why nothing ever moves it. Everything in this document that reads a standing
> is reading an AI's. The one exception is `Diplo_OffendAll`, which walks 1..5 with no
> `isHuman` test at all — so a person's row *can* move, downward only, by 15 a betrayal, and
> by nothing else in the game.
>
> **Correction (§10.5): the Bishop's guard covers `warTarget` as well as `atWar`.** §5's note
> below says the test *"guards only the `atWar` write"*. The `atWar` store is a
> comma-expression **inside the same `&&` chain** as the `warTarget == 0` test:
> `if ((lord != 4) && (pair.atWar = 1, warTarget == 0)) warTarget = offender;` — so a
> betrayed Bishop sets neither. He does not even name the realm he was betrayed by.

`Diplo_OffendAll` (`0x004A24D1`) lowers **every other realm's** standing towards the offender.
Betraying an ally is the only thing in the game that costs reputation with third parties.
**[D]**

The offences and their sites:

| what | amount | site |
|---|---:|---|
| destroying an enemy supply transport | **5** | `Unit_EnterOccupiedTile` `0x004658C1`; also sends group 161 *"Supplies lost."* to the owner and 162 *"Supplies destroyed."* to the attacker |
| trampling a field in another realm's county | **10** | `Unit_CrossField` `0x0046673C` — **only when the *trampling realm* is human**, not the victim; see §10.6 |
| burning a dwelling | **20** | `Unit_BurnDwelling` `0x00468AE2`; also costs the county 25 % of its population |
| winning a battle | **20** | `FUN_004AB383` — the loser's realm is offended by the winner's |

And crossing into a county an army has *declared* as its target sends group **170**,
*"Invasion of"*, from the invader to the owner. **[D]**

> **The Bishop does not declare war when his alliance is broken.** The test
> `realms[me].lord != 4` guards only the `atWar` write. Every other lord flags a war; the
> Bishop breaks the alliance, takes the standing hit, sends group 182, and stays technically at
> peace — so he can be allied with again later. **[D]**, one guard and one reading. Whether it
> is a character trait or a slip, nothing in the code says.

---

## 6. Unprompted messages

Not everything is a reply. **[D]** throughout.

| group | label | when |
|---:|---|---|
| 170 | *Invasion of* | an army enters its declared target county, owned by someone else |
| 180 | *Accept alliance ?* | AI step 2 courts a human |
| 181 | *End of alliance.* | AI step 2's grudge passes the lord's threshold |
| 182 | *Broken alliance.* | an *act* breaks an alliance (§5) |
| 189, 190 | *Warning.* | standing bottomed out, first and second time |
| 191 | *Notice of revenge.* | third time — war |
| 192 | *Helpful advice.* | **AI step 13** — an AI holding > 27 % of the map patronises the **last-placed** realm, if that realm is human and not its ally |
| 193 | *How are you doing?* | **AI step 13** — an AI holding > 39 % of the map taunts **every** human realm |
| 194 | *Foiled again.* | `Realm_RecountStrength`, AI step 0's strength recount — the realm has just been **eliminated**. The **local player** in the same position gets group 224; a human who is *not* the local player gets nothing at all. `docs/kingdom.md` §8.4 |
| 195 | *Just call me king.* | `Score_RankRealms`, when the leader and the trailer are the same realm — i.e. the last one standing, and only if that realm is an AI that has not been crowned before. Guarded once by realm `+0xED`; the second time round the guard sends the local player group 225 instead. |

**`docs/kingdom.md` §3.2 has step 13 wrong.** It says *"offer an alliance, or break one"*.
`AI_Taunt` (`0x004A13A6`) does neither: it is the taunt timer, gated on the realm's rank being
better than 2nd and counting to 8 before it speaks. Alliances are entirely step 2's business.

`g_rankLeader` (`0x00553D24`) and `g_rankTrailer` (`0x00522D90`) are the first and last
non-zero entries of the rank table `Score_RankRealms` sorts. **[V]** — and because they hold
*realm indices*, the two being equal means one realm is left in play, which is the game's
victory condition. `docs/kingdom.md` §8.4.

---

## 7. What the player sees — screen `0x0B`

`docs/screens-county.md` §1 lists screen `0x0B` as *"the other lords, `faces.pl8`"*, painter
`0x00416CF3`. That is the diplomacy screen, and here is the rest of it. **[D]** throughout
except where marked.

`Diplo_DrawScreen` (`0x00416CF3`) draws, top to bottom:

* one **lord card** per rival realm still in play, stacked at `y = 0x31 + i × 100`: an 82 × 78
  inset with the portrait from `faces.pl8` frame `lord × 3 − 3` (frame 12 for a human rival),
  the realm's shield, its name from `g_playerNames` (`0x00553D54`, stride `0x2C`), a selection
  outline when it is the current target, the three status icons — **allied**, **at war**,
  **mail waiting** — and the standing thermometer of §1.1;
* the selected rival's name in the heading font at (0xD0, 0x3D);
* the **action menu**, `L2.eng` group 72, in one of four layouts chosen by `g_diploMenuState`
  (`0x00553F38`):

| state | condition | menu items |
|---:|---|---|
| 0 | I have no ally | 2 3 4 5 — gift, compliment, insult, **offer alliance** |
| 1 | this realm is my ally | 2 3 4 6 7 8 — gift, compliment, insult, **terminate**, **ask help**, **ask attack** |
| 2 | I am allied to someone else | 2 3 4 only |
| 3 | **I have already written to *them*** this turn | index 24 alone — *"A message has been dispatched, my Lord."* |

> **Correction (§10.7): state 3's condition is the other way round, and so is the mail
> icon.** This table used to read *"this realm has already written to me"*. The test is
> `pair[target][localPlayer].hasMail` — the **target's** record, indexed by **me** — and
> `Diplo_Post` sets `pair[to][from].hasMail`, so the flag means *my* letter is sitting in
> *their* inbox. Group 72 index 24 says the same in words. The lord card's mail icon reads
> the same byte the same way, so it marks a rival you have written to, **not** one who has
> written to you. A person's own inbox is never read by anything at all: `Diplo_AnswerInbox`
> is AI turn step 1, and a human realm's AI turn is skipped.
>
> One consequence worth stating because it *is* the rule: **one letter per rival per turn.**
> The menu is gone until they answer.

Picking an item sets `g_diploKind` (`0x005651CC`) and opens screen `0x1A`, the compose page:
kind 0 takes a gold amount into `g_diploGold` (`0x0057A0F8`); kinds 1–4 open a **199-character
free-text letter** in one of four buffers at `g_diploLetterDraft` (`0x0053F2B8`, stride 200);
kinds 5 and 6 open a county picker (group 72 indices 19 and 20, *"Choose the county you want
help in."* / *"…attacked."*).

`Diplo_SendClicked` (`0x00436408`) validates, copies the drafted letter into the player's
per-realm slot at `g_diploLetter` (`0x00567D10`, stride `0xCA` — saved and loaded with the game
and drawn by `Msg_DrawWindow` for the recipient), and calls `Diplo_Post`. Its refusals are
their own `L2.eng` groups: **240** *"You did not select a county"*, **241** *"does not belong
to anyone"*, **242** *"does not belong to us"*, **243** *"does not have an enemy in it"*,
**244** *"is part of our alliance"*, **219** *"You cannot ally with this player until they end
their current treaty"*.

**In multiplayer it does not call `Diplo_Post` at all.** When `g_multiplayer` is set it
issues net command `0x48` or `0x49`, and `FUN_00448308` — a handler with no callers in the
decompilation, i.e. dispatched from a command table — calls `Diplo_Post` on every peer.
`FUN_00448339` is the gold half. That is the seam `docs/netcode.md` will need. **[D]**

---

## 8. The bishop, the baron, and whether the player was right

### 8.1 The bishop builds royal castles. Confirmed.

**AI turn step 6**, `AI_BuildCastles` (`0x0049EDC7`), for each county the realm owns:

```c
if (county.owner == me
 && county.population >= P[+0xC8]
 && county.castleType == 0
 && realm.castlesUnderConstruction < P[+0x90]) {
        type = 5 if gold >= P[+0xDC]      /* royal castle   */
             : 4 if gold >= P[+0xD8]      /* stone castle   */
             : 3 if gold >= P[+0xD4]      /* Norman keep    */
             : 2 if gold >= P[+0xD0]      /* motte & bailey */
             : 1 if gold >= P[+0xCC]      /* palisade       */
             : none;
        /* a zero threshold means that type is not offered to this lord */
        if (type) Castle_Order(county, type - 1);
}
```

**[D]** on the control flow; **[V]** on the constants, read from the file:

| lord | palisade | motte & bailey | Norman keep | stone castle | **royal castle** | min pop | concurrent builds |
|---|---:|---:|---:|---:|---:|---:|---:|
| Knight | 200 | — | 1,000 | — | **10,000** | 700 | 4 |
| Baron | — | 500 | — | 4,000 | **never** | 650 | 3 |
| Countess | — | 300 | — | 2,000 | **never** | 600 | 2 |
| **Bishop** | — | — | 100 | — | **2,000** | 600 | **1** |

**The prediction holds, and it is sharper than "big".** The Bishop is the only lord who reaches
the royal castle cheaply: 2,000 crowns against the Knight's 10,000, while the Baron and
Countess never build one at any treasury. He also builds **one castle at a time** — his
`+0x90` is 1 — so the money does not spread: any county he starts on gets a Norman keep (from
100 crowns) or, from 2,000, the royal castle.

`Castle_Order` (`0x00436D02`) takes a 0-based type and stores `type + 1` into county `+0x1C0`,
and `docs/kingdom.md` §7.5 already has castle type 5 = royal castle from `L2.eng` group 71.
**[V]** on the type numbering.

### 8.2 The bishop is also the one who cheats hardest

`AI_SetTaxRates` (`0x0049D638`) hands every AI realm free gold each turn, indexed
`lord × 0x10 + difficulty × 4` — the index expression is literal in the code. Both tables are
exactly five rows: `0x004DC1E0 + 5 × 0x10 = 0x004DC230`, where the small table begins, and
`0x004DC230 + 5 × 0x10 = 0x004DC280`, where an unrelated table begins. **[V]**

| lord | `g_aiGoldGrant` (≥ 3 counties) | `g_aiGoldGrantSmall` (< 3) |
|---|---|---|
| 0 — the human | 0, 0, 0, 0 | 0, 0, 0, 0 |
| 1 — Knight | 0, 400, 700, 1200 | 0, 160, 250, 400 |
| 2 — Baron | 100, 500, 800, 1400 | 40, 180, 300, 500 |
| 3 — Countess | 0, 400, 700, 1200 | 0, 160, 250, 400 |
| **4 — Bishop** | **250, 600, 1100, 1800** | **100, 240, 400, 600** |

**Row 0 is the human and it is all zeros**, so there is no fifth AI row here either: the gold
table and the personality table agree on four lords and are indexed by the same byte. §0.1 —
**these rows are lords, not realms.** Reading them as realms 1…5 shifts every lord by one and
makes the Bishop's grant look middling; the index expression settles it.

**The Bishop's row is the largest at every difficulty**, 1.29× to 1.5× the others. So the
player's two remarks are **one mechanism**: the lord handed the most free gold is the lord who
spends it on the game's most expensive castle at the lowest threshold, and builds only one at a
time so it all lands in one place. That is not two tables coinciding; it is `lord == 4` in both.

**Gold is not the only handout.**

* **People, cattle and grain**, tiered by county count: 1–2 counties get `d × 20` people,
  `d × 5` head and `d × 40` sacks per county; 3–4 counties get half; 5 or more get nothing.
  Already in `docs/kingdom.md` §8.2. **[V]**
* **Free weapons in an emergency.** `FUN_0049FCC5`, reached from AI step 9: a realm down to
  **one county** before year **1273** is handed **100 each of weapon types 3 and 4 if it is the
  Bishop**, or **100 of weapon type 0 if it is the Countess** — and nothing at all if it is the
  Knight or the Baron. **[D]**
* **A muster the player has no equivalent of.** `FUN_004A50AE(county, pct, 100, 1)` conscripts
  `pct` of a county's population in one call, `pct = personality[+0x40]`: Knight 30, Baron 30,
  Countess 40, **Bishop 50**. **[D]**

### 8.3 The baron and peasant armies: **not established**

The claim is that the Baron favours peasant armies. Here is what is traceable, and it does not
decide it.

Every AI muster goes through one path: `FUN_004A50AE` conscripts a percentage of a county's
population as raw men, then a loop hands out the realm's stockpiled weapons **ten at a time,
cycling weapon slots 1…6**, until either the peasants or the stock run out. The composition
that results is a function of what the realm's blacksmiths have made, not of a per-lord
preference for peasants. **[D]**

What *is* per-lord is the **weapon rota**: AI step 12 (`0x0049E77D`) sets each county's weapon
type from a **ten-step cycle** over six personality fields, advancing one step per county per
turn. **[V]** on the dispatch and the values:

| step | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
|---|---|---|---|---|---|---|---|---|---|---|
| field | `+0x50` | `+0x54` | `+0x58` | `+0x5C` | `+0x50` | `+0x54` | `+0x58` | `+0x5C` | `+0x60` | `+0x64` |
| Knight | 0 | 1 | 4 | 2 | 0 | 1 | 4 | 2 | 5 | 2 |
| Baron | 3 | 5 | 4 | 4 | 3 | 5 | 4 | 4 | 4 | 5 |
| Countess | 0 | 1 | 1 | 2 | 0 | 1 | 1 | 2 | 4 | 0 |
| Bishop | 4 | 4 | 3 | 4 | 4 | 4 | 3 | 4 | 4 | 3 |

**They are `g_weaponCost` indices — `[V]`, and this closes what the paragraph below used to
leave open.** It said *"nothing here ties `+0x50` to `g_weaponCost` beyond both being small
integers under 6"*. What ties them is the call the rota loop makes **immediately after**
assigning one: `FUN_0049ED13` (`0x0049ED13`) looks the value it just wrote into county
`weaponType` up in `g_weaponCost`, and sets the blacksmith's *has resource* flag from whether
the realm's wood and iron cover that row. The field is a `g_weaponCost` index because a
`g_weaponCost` lookup is the next thing done with it.

So, with crossbow 0, mace 1, sword 2, pike 3, bow 4, armour 5 (`docs/kingdom.md` §7.4): the
Knight's rota contains **0** three times in ten and the Countess's four; the **Baron's contains
no 0 at all**, and neither does the Bishop's. The Baron makes pikes, bows and armour where the
Knight makes crossbows and bows — the **opposite** of "the Baron makes peasant armies".

**So the bishop half of the prediction is confirmed and the baron half is contradicted.** What
*can* be said
is that **peasant armies are not a lord trait**: every AI raises peasants and arms whatever it
can, and the only per-lord number in the muster is *how many*, where the Bishop takes the
largest share and the Knight and Baron the smallest.

### 8.4 The personality record, as far as it is traced

`g_aiPersonality`, `0x004D8A58`, four records of 240 bytes. **[V]** on the values (read from
the file), **[D]** on each field's meaning, from its single reader.

| off | Knight | Baron | Countess | Bishop | what reads it |
|---|---:|---:|---:|---:|---|
| `+0x00` | 1 | 1 | 0 | 9 | farming style, dispatched by AI step 5 |
| `+0x04` | 2 | 2 | 2 | 1 | tax ladder, `AI_SetTaxRates` |
| `+0x08` | 100 | 100 | 200 | **50** | gift increment (§3.1) |
| `+0x0C` | 500 | 1000 | 1600 | 1500 | base price of military help (§3.5) |
| `+0x10` | **5** | 10 | 15 | 20 | grudge tolerated before breaking an alliance |
| `+0x14` | 12 | 10 | 8 | **4** | turns between alliance offers |
| `+0x28` | 3 | 4 | 4 | 2 | turns between musters — `FUN_0049F977` counts realm `+0x45` up to it |
| `+0x30` | 750 | 800 | 900 | 1000 | treasury floor for helping; also a county-population floor at step 9 |
| `+0x40` | 30 | 30 | 40 | **50** | percent of a county conscripted |
| `+0x50`…`+0x64` | §8.3 | | | | the ten-step weapon rota |
| `+0x68` | 100 | 120 | 200 | 250 | the **weapon stock** a lord wants before mustering — a threshold on realm `+0x138` |
| `+0x70` | 300 | 300 | 250 | **150** | the county population needed before a castle garrison is raised (`FUN_0049F12F`) |
| `+0x74` | 6 | 10 | **5** | 10 | turns between raids; step 10 loads realm `+0x15A` from it |
| `+0x9C` | 32 | 28 | 23 | 35 | the tax rate put on a county the lord has written off (`FUN_0049F431`) |
| `+0x90` | 4 | 3 | 2 | **1** | concurrent castle projects |
| `+0xC8` | 700 | 650 | 600 | 600 | county population needed to start a castle |
| `+0xCC`…`+0xDC` | §8.1 | | | | the five castle gold thresholds |

**`+0x68` used to read *"a threshold on realm `+0x38`"*, and that is a missing digit.** It is
realm **`+0x138`**, the maintained sum of the six weapon counters (`Realm_RecountWeapons`,
`0x004487A9`) — the number the panel draws as *Arms*. `+0x38` sits inside the twenty-four
army-name counters at `+0x2D`, which is not a number anything would threshold. Found by trying
to use the field; see `crates/l2-kingdom/src/ai_army.rs`.

**Three more of the untraced set have readers now**, all in the table above and all `[D]` from
a single reader each: `+0x70`, `+0x74` and `+0x9C`. Two of them are worth a sentence. `+0x70`
runs the **opposite** way to the castle-building floor at `+0xC8` — the Bishop needs the
largest county before he will *build* a castle (600) and the smallest before he will
*garrison* one (150). And every value of `+0x9C` is far above anything a lord's own tax ladder
would charge a county he meant to keep (§8.2's ladders top out at 15), so it is a lord
stripping a county on the way out rather than a tax policy.

Fields at `+0x2C` (a flat 100 in all four records) and `+0x6C` (2, 3, 4, 5) hold plausible
per-lord values and **are still not traced**. Five more are traced by `Ai_TradeForCounty`
(`0x0049E39B`), which the three AI-realm farming styles run before they farm: `+0x84`, `+0x88`
and `+0x8C` are the **wood, stone and iron the lord keeps back** — everything above them is
sold to the county merchant — and `+0x78`/`+0x7C` are the treasury floor and the quantity for
buying weapons of the county's current type. All three reserves hold the same number within a
lord (250 / 300 / 500 / 1000), which is consistent with a single "keep this much of everything"
figure written into three slots. **[D]**, from a single reader each. Note the *wants* that
switch each of the three between selling and buying are **realm** fields at `+0x70 … +0x7C`,
not personality ones sharing the offsets — the two records are easy to confuse here and
`FUN_0049E1BF` (AI step 4) is what fills the realm's. The record is 240 bytes and a little over
half of it is accounted for.

---

## 9. What is not established

Stated plainly, because a wrong map is worse than a small one.

* ~~**The baron / peasant-army claim.**~~ **Settled against it** — §8.3. The rota fields are `g_weaponCost` indices, pinned by `FUN_0049ED13`, and the Baron makes no crossbows at all.
* ~~**The six untraced personality fields.**~~ **Closed, and the last two are dead.** Five were closed by `Ai_TradeForCounty` and three more by `crates/l2-kingdom`'s army and industry passes. The two that were left, **`+0x2C` (a flat 100 in all four records) and `+0x6C` (2, 3, 4, 5)**, have **no reader anywhere in `Lords2.exe`** — §10.9.
* **`g_realmsActive` (`0x00554004`) has two writers with two meanings**, and the diplomacy code
  reads whichever wrote last. §3.4.
* **`pair +0x06` and `+0x07`** are neither initialised nor read anywhere. Probably padding, not
  proven.
* **The message-record fields `+0x12`, `+0x13` and `+0x14`** are named from what the diplomacy
  callers put in them; the other ~130 `Msg_Enqueue` call sites use them differently and those
  uses were not read.
* ~~**`Msg_DrawWindow` is 10,915 bytes and was read only for its text and voice lookups.**~~
  **Closed** — §11. Twenty category arms, the five widget tables that answer them, and three
  arms that are not drawing at all. The two prompts a person answers a lord with are reachable
  now; `crates/l2-game/tests/messages.rs` accepts an alliance and pays for help by clicking.
* ~~**The AI-to-AI half is unobservable and untested.**~~ It is observable now, in *our*
  engine — §10.8 is forty turns of it — but that is our arithmetic agreeing with itself and
  not evidence about the original. It remains unobserved **in the game**.
* ~~**Nothing here has been run.**~~ Nothing here has been run *in the original*, which is
  still true and still the limit on everything above. `crates/l2-kingdom/src/diplomacy.rs`
  runs all of it in ours; §10 is what that produced.

---

## 10. What happened when it was implemented

`crates/l2-kingdom/src/diplomacy.rs`, `crates/l2-game/src/screens/diplomacy.rs`. Implementing
a document is the only way to find out whether it is true, and this one was right about
almost everything and wrong in nine places that each change behaviour. Seven are corrections
to sections above and are marked there; two are new.

Every finding below is `[D]` from the same corpus this document was written from — a second
reading of the same functions, asking a different question — except §10.8, which is `[V]` on
our own engine's output and says nothing about the original's.

| | correction | where |
|---|---|---|
| 10.1 | only **three** of the seven replies have the `isHuman` guard, not all seven | §3 |
| 10.2 | `personality[+0x30]` is a **population** floor, not a treasury one | §3.5 |
| 10.3 | `Diplo_ReconcileAlliances` **repairs** a one-sided alliance rather than dropping it | §4.1 |
| 10.4 | `Diplo_Offend` is a **no-op when the offended realm is human** | §5 |
| 10.5 | the Bishop's guard covers **`warTarget` as well as `atWar`** | §5 |
| 10.6 | the field trample fires when the **trampler** is human, not the victim | §5 |
| 10.7 | menu state 3 and the mail icon mean *"I have written to them"* | §7 |
| 10.8 | what forty turns of it actually produces | new |
| 10.9 | the last two personality fields are **dead**, not untraced | §8.4, §9 |

### 10.8 Forty turns of England, with diplomacy running

The only oracle this subsystem can have. `docs/decisions.md` C26: **every shipped fixture is
turn one with every realm holding exactly one county**, so every diplomatic state above
*"everyone is neutral and equal"* has no oracle at all — playing the position out is the only
way to look at one, and what follows is *our* engine, not the game's.

The first run was the interesting one, because it produced **nothing**:

```
realm 1 (human)  ally=0 warTarget=0 |   0    0    0    0    0
realm 2 (Knight) ally=0 warTarget=0 |   5   30   30   30   30
realm 3 (Baron)  ally=4 warTarget=0 |   5   30   30   30A  30
realm 4 (Bishop) ally=3 warTarget=0 |   5   30   30A  30   30
realm 5 (Countess) ally=0 warTarget=0 | 5   30   30   30   30
```

Every AI-to-AI pair saturated at **+30** — `AI_Diplomacy`'s heal of one a turn, from
`Diplo_Init`'s opening 5, reaching the ceiling on turn 25 and staying there. Two alliances
formed; **no realm came to think ill of anybody, and none picked a war target.** The raid
rule wants a rival below −10 and there was not one on the map.

The reason is the sentence at the top of §5 read forwards: `Diplo_Offend`'s **four call sites
are not in the diplomacy code**. They are in the mover and in the battle return, and until
those three lines were wired the heal had nothing to push against. With them:

```
after 40 turns: 21 non-zero standings, 4 below −10, 1 at war, 1 war target, 2 realms allied
realm 4 ally=5 warTarget=5 |   5   30    1   30  -26 A W
```

Two things in that line are the subsystem showing its own shape.

**Realm 4 is allied to realm 5 and at war with it at the same time**, and that is faithful.
`Diplo_PickAllyCandidate` refuses a candidate the *courter* is at war with —
`pair[me][cand].atWar` — and says nothing about `pair[cand][me]`. Realm 5's own war flag was
never set, so realm 5 courted realm 4 and `Diplo_FormAlliance` wrote both bytes. The war is
one-directional because the *standing* is one-directional (§1), and the alliance is not.

**And the human's row moved by exactly −15 in one place**, which is §10.4's exception doing
the only thing it can: somebody betrayed an ally, `Diplo_OffendAll` walked realms 1..5 with no
`isHuman` test, and the person's opinion of the betrayer dropped 15 and will never recover,
because nothing that could raise it will ever run for a human realm.

**The asymmetry that follows is worth stating as a rule for a player**: an AI's opinion of
*you* only ever goes down, and its opinion of *another AI* heals a point a turn. So the raid
rule fires against a person far more readily than against a rival — on the England position
in forty turns, **only** against a person. That is not a balance decision anybody made; it is
one `isHuman` in a loop guard.

### 10.9 The last two personality fields have no reader at all

§8.4 left `+0x2C` (a flat 100 in all four records) and `+0x6C` (2, 3, 4, 5) as *"plausible
per-lord values and still not traced"*. They are **dead**.

The method is an exhaustive scan rather than a failure to find, which is the standard
`docs/arms.json` sets for a `dead` verdict: `tools/oracle/decomp` holds every one of the
2,452 functions in `Lords2.exe`'s text section, decompiled with the `AiPersonality` struct
from `docs/records.json` applied, so every access to an offset the struct does not name
renders as `field_0xNN`. Across the whole corpus the personality struct is touched at
`+0x00`, `+0x04`, `+0x08`, `+0x0C`, `+0x10`, `+0x14`, `+0x28`, `+0x30`, `+0x40`, `+0x50`,
`+0x54`, `+0x58`, `+0x5C`, `+0x60`, `+0x64`, `+0x68`, `+0x70`, `+0x74`, `+0x78`, `+0x7C`,
`+0x84`, `+0x88`, `+0x8C`, `+0x90`, `+0x9C`, `+0xA0`, `+0xC8` and `+0xCC…+0xDC`.
**`field_0x2c` does not occur anywhere in the corpus**, and `field_0x6c` occurs fourteen
times and every one is a different struct.

So the 240-byte record has two four-byte slots the shipped game never reads. Nothing may be
built on them, and — the reason this is worth a paragraph rather than a line — nothing should
be *inferred* from them either: a per-lord value with no reader is exactly the shape of a
finding that is not one.

### 10.10 What is still missing on the player's side

Two arms, and they are the same blocker: **`Msg_DrawWindow` (`0x0047309E`, 10,915 bytes) has
only ever been read for its text and voice lookups**, so its per-category window layouts do
not exist here.

* **`Diplo_PayHelpClicked` (`0x004367FF`)** — the accept button on the category-10 *"Pay -"*
  prompt. The rule behind it is built; there is no window to click.
* **`FUN_00436872`** — the *"Accept alliance ?"* prompt's two buttons, category `0x0B`.
  Its guard is `(realms[offerer].isHuman != 0) || (hotspot != 0)`, so in single player
  **declining an AI's offer runs nothing at all** — not even a refusal message. The offer
  simply lapses when the offering realm clears `offerPending`.

Those two are **the only places a person answers a lord rather than writing to one**, and
both are unreachable. Everything a person can *initiate* is built: `docs/arms.json`'s
`diplomacy` group is nine reproduced arms and three missing ones, the third being the
199-character free-text letter, which is another branch's.

**And one thing §7 implies that is not true: the four letter buffers are not four fields.**
`FUN_0040210C(g_diploLetterDraft + (kind − 1) × 200, 199)` is a bounded copy *out of*
`DAT_005CD550`, the game's **one shared text-edit buffer** — `FUN_00401D26(ch)` inserts a
typed character into it at cursor `DAT_005BB4A8`. So there is a single editor and the four
200-byte slots are snapshots harvested from it once a frame, with `FUN_00402009` copying the
other way when a dialog opens. A letter half-written to the Knight and abandoned is still in
the editor when you open the Baron's.

> This paragraph is here because the first reading of it was wrong in the way `CLAUDE.md`
> rule 4 is about: `FUN_0040210C` was called *"the keyboard entry field"* on the strength of
> where it is called from, and it is the *harvest*. It was caught by re-reading after the
> decompilation corpus was rebuilt with 211 more function names — not because any of the new
> names is in this function, but because re-reading is the only thing that has ever caught
> this class of error. `docs/agents.md`.

---

## 11. Answering a lord — the message window, and where the answer lives

§2's diagram ends at `Msg_Pump ──► Msg_DrawWindow`, and §9 recorded that the 10,915 bytes on
the far side of that arrow had never been read for anything but their strings. They have been
now. This section is the half of diplomacy that is not *writing* to a lord.

### 11.1 Two of the seven kinds arrive as a question, and only two

Of everything in this document, exactly **two** things put a decision in front of the player
rather than a notice:

| the message | `L2.eng` | category | widget table | handler |
|---|---|---|---|---|
| *"Accept alliance ?"* | 180 | `0x0B` | `0x004DDAC0` | `Diplo_AcceptAllianceClicked` (`0x00436872`) |
| *"Pay -"* | 185 (help), 188 (attack) | `0x0A` | `0x004DDA90` | `Diplo_PayHelpClicked` (`0x004367FF`) |

Three more prompts exist on the **diplomatic letter** (category `0x0C`, `Msg_DrawDiplomacy`),
and two of them are the *ally's* answer to a request the player made — groups `0xFA` and
`0xFB`, handlers `Diplo_ReplyHelpClicked` and `Diplo_ReplyAttackClicked`. Both set
`g_diploKind` to a value **beyond the composer's seven** (7 and 8 for help, 9 and 10 for
attack) and post `Net_SendCommand(0x49)`, and nothing else consumes those numbers: **in a
single-player game they dismiss the letter and do nothing.** That is not a gap in the
reading; it is the branch the original takes with `g_multiplayer` clear.

The third is group `0xF8`, an alliance offer arriving inside a letter, and it re-uses the
category-`0x0B` table — one table, two categories, one handler.

### 11.2 Declining an AI's offer runs nothing at all

`Diplo_AcceptAllianceClicked`'s guard is worth quoting because it is easy to read past:

```c
Msg_Dismiss();
DAT_0056D678 = g_uiHotspotId;
if ((g_realms[DAT_0057C8B8].isHuman != 0) || (g_uiHotspotId != 0)) {
    if (g_multiplayer == 0) { if (g_uiHotspotId == 1) Diplo_FormAlliance(g_localPlayer, DAT_0057C8B8); }
    else Net_SendCommand(0x47, 0);
}
```

An **AI** offer **declined** fails both halves of the outer test, so nothing runs: no refusal
letter, no grudge, no standing change, no `offer_pending` clear. The offer simply lapses when
the offering realm reaches §4's courtship step on its next turn. §5's grudge table is not
involved, and a player who says no pays nothing for it.

`DAT_0057C8B8` is **not** a diplomacy global. `Msg_DrawWindow` writes it, from the
category-`0x0B` arm, to whichever realm's letter is on screen — beside the shield and the
portrait. So the *drawing* code is what tells the *answering* code who is asking.

### 11.3 An offer that arrives too late is never seen

`Msg_DrawWindow`'s category-`0x0B` arm opens with

```c
if (g_realms[g_localPlayer].ally != 0) { Msg_Dismiss(); return; }
```

and `Msg_DrawDiplomacy` carries the same guard for group `0xF8`. So a second lord's offer,
queued behind the first, **closes itself the frame it would have been drawn** — the player
never sees it and never declines it. Two AI realms courting in the same season is not rare
(§4 has no exclusion between them), so this is the ordinary case rather than a corner.

It is also the clearest instance of a thing worth knowing about this function generally:
**three of its arms are not drawing at all**, and none of them is visible from the category
switch. The other two are the ending's outcome ladder and the floating tip's timer clamp.

### 11.4 The price is read at the click, not at the offer

`Diplo_PayHelpClicked` passes `g_diploHelpCounty` and `g_diploHelpPrice` — the two globals
`Diplo_ReplyHelpRequest` set when it *composed* the message (§3). They are single globals, not
fields of the message, so **a second pay-prompt queued behind the first quotes the second
one's price on both windows.** Reproduced; `Kingdom::diplomacy` holds the same two values in
the same way.

### 11.5 Where it is

`crates/l2-game/src/message.rs` is the ring and the rules; `crates/l2-game/src/screens/message.rs`
is the window and the arms; `crates/l2-game/tests/messages.rs` plays every route above with
`Event` values. `docs/arms.json`'s `messages` group has one record per arm, including the four
that are not built and why.
