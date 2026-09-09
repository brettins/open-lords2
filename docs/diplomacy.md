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

Two facts shared by all of them. A reply is produced **only if the sender is human**
(`realms[sender].isHuman != 0`), so AI-to-AI diplomacy is entirely silent — which is right,
since nobody would see it. And every reply advances the sender's `voiceRotation`, so the four
recorded takes cycle rather than repeat. **[V]**

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

**[D]**. `personality[+0x30]` is a **treasury floor** — 750 / 800 / 900 / 1000 by lord — below
which an ally will not move at all.

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
matrix from the `ally` bytes and drops any pairing that is one-sided or whose partner has been
eliminated. **[D]**

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

`Diplo_OffendAll` (`0x004A24D1`) lowers **every other realm's** standing towards the offender.
Betraying an ally is the only thing in the game that costs reputation with third parties.
**[D]**

The offences and their sites:

| what | amount | site |
|---|---:|---|
| destroying an enemy supply transport | **5** | `Unit_EnterOccupiedTile` `0x004658C1`; also sends group 161 *"Supplies lost."* to the owner and 162 *"Supplies destroyed."* to the attacker |
| a unit destroying something in another realm's county | **10** | `FUN_0046673C`, only when the victim is human |
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
| 3 | this realm has already written to me this turn | index 24 alone — *"A message has been dispatched, my Lord."* |

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
* **The six untraced personality fields.** §8.4. Five more were closed by `Ai_TradeForCounty`.
* **`g_realmsActive` (`0x00554004`) has two writers with two meanings**, and the diplomacy code
  reads whichever wrote last. §3.4.
* **`pair +0x06` and `+0x07`** are neither initialised nor read anywhere. Probably padding, not
  proven.
* **The message-record fields `+0x12`, `+0x13` and `+0x14`** are named from what the diplomacy
  callers put in them; the other ~130 `Msg_Enqueue` call sites use them differently and those
  uses were not read.
* **`Msg_DrawWindow` is 10,915 bytes and was read only for its text and voice lookups.** The
  per-category window layouts — where the portrait sits, which buttons exist, how the alliance
  and pay prompts are answered — are not written down.
* **The AI-to-AI half is unobservable and untested.** Both silent paths (AI allies with AI, AI
  gifts an AI) are read off the code and produce no message anyone could ever have seen.
* **Nothing here has been run.** `docs/method.md` §3.
