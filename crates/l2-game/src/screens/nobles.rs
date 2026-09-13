//! **The standings** — `Screen_GreatestNoble` (`0x0041593B`), `g_screenId`
//! `0x20`.
//!
//! # It is a bar chart of five flagpoles, not a table
//!
//! `docs/screens-county.md` calls it *"the standings"* and `docs/draws.md`
//! filed it as a standings **table**.
//! *"suspiciously few"*. The page is five banners on five
//! poles, each raised to that realm's **percentage of the leader's** score in
//! one category, with the seven categories as tabs along the bottom and one
//! line of text naming the category and whoever leads it.
//!
//! The player's report was *"I can't click the Greatest Nobles button in the
//! treasury view"*, and the button was not the defect: `court.rs` answered it
//! with `Transition::Stay` because there was nothing to go to.
//!
//! # The painter, address by address
//!
//! ```text
//! Screen_GreatestNoble(firstFrame):                             0x0041593B
//!   Restore_WorkingDir(); DAT_0053F050 = 0            campaign sprite mode
//!   if (firstFrame == 1) FUN_004B11CE()
//!   File_ReadChunk("grtnoble.256", palette, 0x300, 0)
//!   FUN_00408FCB("grtnoble.pl8", 0x1E0)     a raw 640 x 480 page, not a sheet
//!   File_ReadChunk("flags.pl8", g_spriteBank, 160000, 0)  the sheet, into RAM
//!   leader = FUN_00415BDC()                        the ranking — see [`rank`]
//!   for slot in 1..6:
//!     realm = g_nobleColumnRealm[slot]                          0x004D2B58
//!     if (g_realms[realm].strength == 0) continue
//!     x   = g_nobleColumnX[slot]                                0x004D2B40
//!     top = (100 - g_nobleBarPct[realm]) * 2                    0x00522C60
//!     Sprite_WGenSprite(realm.shieldIndex - 1, x, top + 0x2D)   flags.pl8
//!     for (i, hue) in [(0x17, 0x10), (0x18, 0x12), (0x19, 0x14), (0x1A, 0x12)]:
//!       FUN_00403A8F(x + i, top + 0x89, x + i, 0x158, hue)      the pole
//!   Sprite_WGenSprite(5, g_nobleTabX[g_nobleCategory], 0x173)   the marker
//!   g_penAdvance = 0
//!   Eng_DrawString(35, g_nobleCategory, 0x148, 0x1BE, body)   "Most counties,"
//!   if (!allLevel && !tiedAtTop)
//!        Ui_DrawText(g_playerNames[leader], pen + 0x14A, 0x1BE, body)
//!   else Eng_DrawString(35, 7, pen + 0x14A, 0x1BE, body)          "undecided."
//!   Ui_OkButton(g_screenStride - 0x1C, g_screenHeight - 0x1C, 1)   (612, 452)
//!   Gfx_MarkAllDirty(); Gfx_Present(1); Palette_Set(grtnoble.256)
//! ```
//!
//! **`Screen_DrawWidgets` has no `0x20` arm** — checked across all of its arms
//! — so this painter is the whole of the screen, and the seven tabs are drawn
//! by `grtnoble.pl8` itself.
//!
//! # `flags.pl8` has **six** frames
//!
//! `docs/draws.md`'s sheet note said frame 5 was *"for the leader"*. It is
//! not: `Sprite_WGenSprite(5, …)` is indexed by `g_nobleCategory` through
//! [`TAB_X`], not by the leading realm, and its x values are the seven tab
//! columns. Frames 0…4 are the five 51 × 92 banners; frame **5 is 23 × 60**,
//! read from the shipped file — the marker that sits under the tab you are
//! looking at. `C194`.
//!
//! # The metric, category by category — `FUN_00415E42` (`0x00415E42`)
//!
//! Undocumented anywhere before this. A realm out of play scores 0 in every
//! category; otherwise, by `g_nobleCategory`:
//!
//! | # | `L2.eng` 35 | realm field | ours |
//! |---|---|---|---|
//! | 0 | *"Most counties,"* | `+0x29` | [`Realm::county_count`] |
//! | 1 | *"Most castles,"* | `+0x4C`, **as a byte** | `score_inputs[`[`SCORE_INPUT_CASTLES`]`]` |
//! | 2 | *"Most troops,"* | `+0x54` | [`Realm::total_men`] |
//! | 3 | *"Most crowns,"* | `+0x118` | [`Realm::gold`] |
//! | 4 | *"Happiest people,"* | `+0x0C` | [`Realm::mean_happiness`] |
//! | 5 | *"Most people,"* | `+0x10` | [`Realm::population_total`] |
//! | 6 | *"Greatest noble,"* | `6 - (+0x2B)`, or **2 flat before 1270** | [`Realm::rank`] |
//!
//! Two of those are worth saying out loud. **Category 6 is the overall
//! standing and it is deliberately dead for the first two years**: `g_year <
//! 0x4F6` returns a flat 2 for every realm, which makes every bar equal, which
//! makes the line read *"Greatest noble, undecided."* until 1270. And
//! **category 1 reads a byte**: `+0x4C` is the finished-castle count
//! `Castle_BuildTick` (`0x004508DE`) writes, and the original truncates it, so
//! a 256th castle would read as none. Reproduced, in [`value`].
//!
//! # The ranking — `FUN_00415BDC` (`0x00415BDC`)
//!
//! ```c
//! leader = 1; best = 0;
//! for (r = 1; r < 6; r++) { v = FUN_00415E42(r); if (best <= v) { leader = r; best = v; } }
//! for (r = 1; r < 6; r++) pct[r] = clamp(PctOf(FUN_00415E42(r), best), 0, 100);
//! allLevel = 1;
//! for (r = 1; r < 6; r++) if (realm r is in play) { first ? remember : differ -> allLevel = 0 }
//! if (!allLevel) { tiedAtTop = any in-play r != leader with pct[r] == pct[leader]; }
//! else           { pct[r] = 50 for every in-play realm; }
//! ```
//!
//! **`best <= v`, not `<`** — so a tie for the lead is won by the **highest
//! realm index**, and with every value zero the leader is realm 5. It never
//! shows, because the same function then calls the category undecided; it is
//! reproduced anyway, because [`Standings::leader`] is what the line would
//! print if either flag were ever cleared under a tie.
//!
//! **The bars are a percentage of the leader, not of a maximum**, so the
//! leader's pole is always full height and a realm with nothing is a bare
//! pole. When every realm is level the bars are set to **50**,
//! which is the one place the original draws a number it did not compute.
//!
//! # The seven tabs — `Hotspot_Test(0, 0, &g_nobleTabs, 7)`
//!
//! `Screen_HandleInput`'s `0x20` arm, and the only widget pass this screen
//! has. The table is at `0x004DC890`, seven 24-byte records read out of
//! `Lords2.exe`:
//!
//! ```text
//! x0 46  y0 428  x1  83  y1 463  handler FUN_0043524E  kind 1  id 0
//! x0 84            x1 121                                      id 1
//! …                                                            …
//! x0 274           x1 312                                      id 6
//! ```
//!
//! `FUN_0043524E` is three statements — `g_nobleCategory = g_uiHotspotId;
//! g_redrawRequest = 2; FUN_004B3994(g_uiHotspotId);` — so a tab changes the
//! category, repaints, and **speaks its own name**: `S035_01.wav` …
//! `S035_07.wav`, the files named after the `L2.eng` group this screen draws.
//!
//! # The way out, and the way in
//!
//! `Screen_FrameInput`'s `0x20` arm is the county panels' shape exactly: a
//! turn ending under it force-closes it, otherwise a right release anywhere or
//! `Ui_OkButtonClicked` in the corner box sets `g_screenId = 0`. So it closes
//! to the **map**, not to the court — but our stack pops, and popping lands on
//! the court, which is a divergence and is recorded as one. The original has
//! one byte; we have a stack, and `docs/decisions.md` C190's `Goto` exists for
//! exactly this. It is deliberately not used: the court is where the button
//! was, and the original's `g_screenId = 0` throws it away only because it has
//! nowhere to keep it.
//!
//! The way in is `FUN_004351C4` (`0x004351C4`), the court's one widget:
//!
//! ```c
//! g_screenId = 0x20; g_redrawRequest = 1;
//! if (g_multiplayer == 0) FUN_00435211();          /* Score_RankAndRefreshAll */
//! else                    Net_SendCommand(0x3B, 0);
//! FUN_004B3994(g_nobleCategory);
//! ```
//!
//! **The recount is the interesting half.** `FUN_00435211`
//! (`Score_RankAndRefreshAll`, `0x00435211`) is `Score_RankRealms()` then
//! `Realm_UpdateTotals(r)` for r in 1..=5 — the totals five of the six score
//! inputs are read from, rebuilt for every realm, before the page is drawn.
//! Without it the page would show the standings as they stood at the last AI
//! turn. See [`recount`].

use l2_view::Canvas;

use l2_kingdom::realm::Realm;
use l2_kingdom::tables::SCORE_INPUT_CASTLES;

use crate::input::{Event, Key, Rect};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{font, Pen};

/// `L2.eng` group 35 — eight strings, which is the whole group, and this
/// screen is its **only consumer in the binary**. `CLAUDE.md` rule 6.
pub const GROUP: usize = 35;

/// The seven categories are group 35 indices 0…6, in this order.
pub const COUNTIES: usize = 0;
pub const CASTLES: usize = 1;
pub const TROOPS: usize = 2;
pub const CROWNS: usize = 3;
pub const HAPPINESS: usize = 4;
pub const PEOPLE: usize = 5;
/// *"Greatest noble,"* — the overall standing, and see [`GREATEST_NOBLE_YEAR`].
pub const GREATEST_NOBLE: usize = 6;
/// Index 7, *"undecided."*, drawn in place of a name when nothing leads.
pub const UNDECIDED: usize = 7;

pub const CATEGORIES: usize = 7;

/// The full-screen page and its palette.
pub const BACKGROUND: &str = "Grtnoble.pl8";
pub const PALETTE: &str = "Grtnoble.256";
/// The banners, and the tab marker. Six frames: 0…4 are 51 × 92 and frame
/// [`MARKER_FRAME`] is 23 × 60.
pub const FLAGS: &str = "Flags.pl8";
pub const MARKER_FRAME: usize = 5;

/// `g_nobleColumnX` (`0x004D2B40`) — entries 1…5, the x of each **column**.
/// Entry 0 is zero and is never read; the loop runs 1…5.
pub const COLUMN_X: [i32; 5] = [39, 167, 294, 424, 552];
/// `g_nobleColumnRealm` (`0x004D2B58`) — which realm stands in each column,
/// and **it is not the realm order**: realm 1 is placed in the middle.
pub const COLUMN_REALM: [u8; 5] = [5, 3, 1, 2, 4];

/// `g_nobleTabX` (`0x004D2B20`) — the marker's x per category. 38 apart, and
/// eight pixels right of each tab's own [`TABS`] left edge.
pub const TAB_X: [i32; CATEGORIES] = [54, 92, 130, 168, 206, 244, 282];
/// `Sprite_WGenSprite(5, …, 0x173)`.
pub const MARKER_Y: i32 = 0x173;

/// `Sprite_WGenSprite(shield - 1, x, (100 - pct) * 2 + 0x2D)`.
pub const FLAG_TOP: i32 = 0x2D;
/// The pole runs from the banner's own bottom edge — `(100 - pct) * 2 + 0x89`,
/// which is [`FLAG_TOP`] plus the banner's 92 rows — down to [`POLE_BOTTOM`].
pub const POLE_TOP: i32 = 0x89;
pub const POLE_BOTTOM: i32 = 0x158;
/// Four one-pixel `FUN_00403A8F` lines: `(dx, colour)`. A pole, lit from the
/// left. **`FUN_00403A8F` is not one of the audit's 26 primitives**, which is
/// why a bar chart of up to twenty lines counted as zero draw calls.
pub const POLE: [(i32, u8); 4] = [(0x17, 0x10), (0x18, 0x12), (0x19, 0x14), (0x1A, 0x12)];
/// A bar is two pixels of height per percent, so 100% is 200 pixels.
pub const PIXELS_PER_PERCENT: i32 = 2;
/// What [`rank`] writes into every in-play realm when the category is level.
pub const LEVEL_BAR_PCT: i32 = 50;

/// `Eng_DrawString(35, category, 0x148, 0x1BE, body)`, and the name two pixels
/// past where it ended — `g_penAdvance + 0x14A` against a label at `0x148`.
pub const LINE_AT: (i32, i32) = (0x148, 0x1BE);
pub const NAME_DX: i32 = 0x14A - 0x148;

/// `Ui_OkButton(g_screenStride - 0x1C, g_screenHeight - 0x1C, 1)` — **mode 1**,
/// `System.pl8` frame `0x10`, computed from the screen size.
pub const OK: Rect = Rect::new(640 - 0x1C, 480 - 0x1C, 24, 24);
pub const OK_MODE: usize = 1;

/// `g_nobleTabs` (`0x004DC890`) — seven `Hotspot_Test` records, `{x0, y0, x1,
/// y1}`, all kind 1, all handler `FUN_0043524E`, ids 0…6. Read out of
/// `Lords2.exe` at a 24-byte stride.
pub const TABS: [Rect; CATEGORIES] = [
    tab(46, 83),
    tab(84, 121),
    tab(122, 159),
    tab(160, 197),
    tab(198, 235),
    tab(236, 273),
    tab(274, 312),
];
pub const TAB_Y0: i32 = 428;
pub const TAB_Y1: i32 = 463;

const fn tab(x0: i32, x1: i32) -> Rect {
    Rect::new(x0, TAB_Y0, x1 - x0, TAB_Y1 - TAB_Y0)
}

/// **`g_year < 0x4F6`** — before 1270, `FUN_00415E42` returns a flat 2 for
/// every realm in the *Greatest noble* category, so the overall standing is
/// *"undecided."* for the first two years of every game.
pub const GREATEST_NOBLE_YEAR: i32 = 0x4F6;
/// The flat score every realm takes before that year.
pub const GREATEST_NOBLE_EARLY: i32 = 2;

/// **`FUN_00415E42`** — one realm's score in one category.
///
/// `year` is `g_year`, read only by [`GREATEST_NOBLE`].
pub fn value(realm: &Realm, category: usize, year: i32) -> i32 {
    if !realm.in_play {
        return 0;
    }
    match category {
        COUNTIES => realm.county_count as i32,
        // `(uint)(byte)g_realms[r].field_0x4c` — **the original truncates it
        // to a byte**, and `Castle_BuildTick` is the only thing that writes it.
        CASTLES => realm.score_inputs[SCORE_INPUT_CASTLES] as u8 as i32,
        TROOPS => realm.total_men,
        CROWNS => realm.gold,
        HAPPINESS => realm.mean_happiness,
        PEOPLE => realm.population_total,
        // `6 - rank`, so rank 1 scores 5. Flat before 1270.
        _ => {
            if year < GREATEST_NOBLE_YEAR {
                GREATEST_NOBLE_EARLY
            } else {
                6 - realm.rank as i32
            }
        }
    }
}

/// What [`rank`] worked out: one bar per realm and who, if anyone, leads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Standings {
    /// `g_nobleBarPct` (`0x00522C60`) — indexed by realm, 0…100. Index 0 is
    /// never written and never read, as the original's is not.
    pub pct: [i32; l2_kingdom::MAX_REALMS],
    /// The realm the last `best <= v` left behind. Meaningful only when
    /// neither flag below is set.
    pub leader: u8,
    /// `DAT_00522C78` — every in-play realm scores the same.
    pub all_level: bool,
    /// `DAT_00522C94` — somebody other than the leader matches its bar.
    pub tied_at_top: bool,
}

impl Standings {
    /// The line prints a name only when neither flag is set — the painter's
    /// own `if (DAT_00522C78 == 0 && DAT_00522C94 == 0)`.
    pub fn decided(&self) -> bool {
        !self.all_level && !self.tied_at_top
    }
}

/// **`FUN_00415BDC`** — score every realm in one category, turn the scores
/// into percentages of the leader's, and decide whether anything leads.
pub fn rank(realms: &[Realm], category: usize, year: i32) -> Standings {
    let at = |r: usize| realms.get(r).map_or(0, |realm| value(realm, category, year));
    let in_play = |r: usize| realms.get(r).is_some_and(|realm| realm.in_play);

    // **`best <= v`**, so a tie for the lead goes to the highest realm index.
    let mut leader = 1u8;
    let mut best = 0i32;
    for r in 1..l2_kingdom::MAX_REALMS {
        let v = at(r);
        if best <= v {
            leader = r as u8;
            best = v;
        }
    }

    let mut pct = [0i32; l2_kingdom::MAX_REALMS];
    for r in 1..l2_kingdom::MAX_REALMS {
        pct[r] = l2_kingdom::math::pct_of(at(r), best).clamp(0, 100);
    }

    // `local_10 == 999` is "nothing seen yet"; the flag falls the first time
    // two in-play realms disagree.
    let mut first: Option<i32> = None;
    let mut all_level = true;
    for r in 1..l2_kingdom::MAX_REALMS {
        if !in_play(r) {
            continue;
        }
        match first {
            None => first = Some(pct[r]),
            Some(f) if pct[r] != f => all_level = false,
            Some(_) => {}
        }
    }

    let mut tied_at_top = false;
    if all_level {
        // The one number the original draws without computing it.
        for r in 1..l2_kingdom::MAX_REALMS {
            if in_play(r) {
                pct[r] = LEVEL_BAR_PCT;
            }
        }
    } else {
        for r in 1..l2_kingdom::MAX_REALMS {
            if in_play(r) && r as u8 != leader && pct[r] == pct[leader as usize] {
                tied_at_top = true;
            }
        }
    }

    Standings { pct, leader, all_level, tied_at_top }
}

/// **`FUN_00435211`, `Score_RankAndRefreshAll` (`0x00435211`)** — the recount
/// the court's button runs before it opens this page.
///
/// `Score_RankRealms()` then `Realm_UpdateTotals(r)` for r in 1..=5. Five of
/// the six score inputs are what `Realm_UpdateTotals` writes and the sixth,
/// the castle count, belongs to `Castle_BuildTick`; without this the page
/// would show whatever the last AI turn left behind. `docs/symbols.md`
/// records the pair as always travelling together, and this is the UI caller.
///
/// **It runs before the ranking, not after**: the original ranks first and
/// then refreshes the totals the *next* ranking will read, which means the
/// ranks this page draws are one refresh behind. Reproduced in that order.
///
/// # This is the one thing on this screen that moves the lockstep digest
///
/// Every field it writes — `+0x2B` the rank, `+0x50` the score, `+0x29`,
/// `+0x10`, `+0x14`, `+0x0C`, `+0x58`, `+0x60`, `+0x2C`, `+0x54` — is on
/// [`l2_kingdom::Realm`], and the digest is `Canonical::hash_of(kingdom)`. So
/// **opening a scoreboard changes the world**, and it changes it only where
/// the totals had gone stale since the last AI turn: run twice over an
/// unchanged kingdom it is idempotent, and run after a county has changed
/// hands mid-turn it is not.
///
/// The original says the same thing in the only way it can: in a network game
/// `Court_OpenGreatestNoble` does **not** call this, it sends
/// `Net_SendCommand(0x3B, 0)`, whose deferred action `NetAct_RankRealms`
/// (`0x00448422`) runs the identical pair on every peer. We have no network
/// game and take the single-player arm; when one exists this call site is one
/// of the places that has to become a command.
/// `docs/netcode.md`.
pub fn recount(game: &mut crate::game::Game) {
    let t = game.kingdom.tables;
    l2_kingdom::ai::rank_realms(&t, &mut game.kingdom.realms);
    for realm in 1..l2_kingdom::MAX_REALMS as u8 {
        let (armies, total_men) = game.kingdom.campaign.units.realm_totals(realm);
        let l2_kingdom::Kingdom { realms, counties, county_count, .. } = &mut game.kingdom;
        l2_kingdom::ai::update_realm_totals(
            &mut realms[realm as usize],
            counties,
            *county_count,
            realm,
            armies,
            total_men,
        );
    }
}

pub struct NoblesScreen;

impl NoblesScreen {
    pub fn new() -> NoblesScreen {
        NoblesScreen
    }
}

impl Default for NoblesScreen {
    fn default() -> NoblesScreen {
        NoblesScreen::new()
    }
}

impl Screen for NoblesScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Nobles
    }

    fn title(&self, _ctx: &Ctx) -> String {
        "The standings — screen 0x20".into()
    }

    fn palette(&self) -> Option<&'static str> {
        Some(PALETTE)
    }

    /// `grtnoble.pl8` is a raw 640 × 480 page read straight into the display
    /// buffer, so this covers whatever opened it.
    fn is_overlay(&self) -> bool {
        false
    }

    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        match event {
            // `Screen_FrameInput`'s epilogue, which runs on every screen but
            // `0x12`: a press inside the minimap raster selects that county,
            // recentres the map and sets `g_screenId = 0`.
            // arm: 0x0042FF10/minimap-under-the-standings left-press
            Event::Click { x, y } if l2_view::chrome::minimap_hit_area().contains(x, y) => {
                Transition::Pass
            }
            // **The seven tabs** — `Hotspot_Test(0, 0, &g_nobleTabs, 7)`, all
            // kind 1, all `FUN_0043524E`, which is
            // `g_nobleCategory = g_uiHotspotId; g_redrawRequest = 2;
            // FUN_004B3994(g_uiHotspotId);`.
            //
            // The category is on the [`crate::game::Game`] and not on this
            // screen for the reason `docs/audio.json` gives for the castle
            // chooser's silence: a selection a screen keeps to itself is
            // invisible to [`crate::audio::Director`], so the spoken name
            // could not be reproduced. The original's is a global too.
            // arm: 0x0043524E/standings-category left-press
            Event::Click { x, y } if TABS.iter().any(|t| t.contains(x, y)) => {
                let hit = TABS.iter().position(|t| t.contains(x, y)).unwrap_or(0);
                ctx.game.nobles_category = hit as u8;
                ctx.game.nobles_spoken = ctx.game.nobles_spoken.wrapping_add(1);
                Transition::Stay
            }
            // `Ui_OkButtonClicked()` — a left release in the 24 x 24 corner
            // box. The original goes to the map; we pop, which lands on the
            // court the button was pressed on. See the module docs.
            // arm: 0x0042FF10/standings-ok left-release
            Event::Click { x, y } if OK.contains(x, y) => Transition::Pop,
            // `g_mouseRightReleased`, anywhere, tested before the OK.
            // arm: 0x0042FF10/standings-right right-release
            Event::RightClick { .. } => Transition::Pop,
            // **Ours**: the arm has no keyboard test.
            // arm: ours/standings-keyboard-close key
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => Transition::Pop,
            _ => Transition::Stay,
        }
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let a = &ctx.assets.shell;
        let ink = &ctx.assets.ink;
        let pen = Pen {
            assets: a,
            ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        if !crate::shell::background(canvas, a, BACKGROUND) {
            canvas.clear(ink.background);
        }

        let category = (ctx.game.nobles_category as usize).min(CATEGORIES - 1);
        let s = rank(&ctx.game.kingdom.realms, category, ctx.game.kingdom.year);

        // The five columns, in the original's own seating order.
        for (slot, &realm) in COLUMN_REALM.iter().enumerate() {
            let Some(r) = ctx.game.kingdom.realms.get(realm as usize) else { continue };
            if !r.in_play {
                continue;
            }
            let x = COLUMN_X[slot];
            let top = (100 - s.pct[realm as usize]) * PIXELS_PER_PERCENT;
            // `Sprite_WGenSprite(shieldIndex - 1, …)` out of `flags.pl8`. A
            // shield of 0 would index -1 in the original; ours skips, and a
            // realm in play always has one.
            let frame = (r.shield_index as usize).wrapping_sub(1);
            if let Some(f) = a.sheet(FLAGS).and_then(|sheet| sheet.frame(frame)) {
                canvas.blit(&f, x, top + FLAG_TOP);
            }
            for (dx, hue) in POLE {
                let y0 = top + POLE_TOP;
                canvas.fill_rect(x + dx, y0, 1, POLE_BOTTOM - y0 + 1, hue);
            }
        }

        // The marker under the tab being looked at — **the category's, not the
        // leader's**. See the module docs.
        if let Some(f) = a.sheet(FLAGS).and_then(|sheet| sheet.frame(MARKER_FRAME)) {
            canvas.blit(&f, TAB_X[category], MARKER_Y);
        }

        // *"Most counties,"* and then either the leader's name or
        // *"undecided."*, two pixels past where the label ended.
        let x = pen.eng(canvas, GROUP, category, LINE_AT.0, LINE_AT.1, font::TEXT);
        if s.decided() {
            let name = super::message::lord_name(ctx, s.leader);
            pen.body(canvas, x + NAME_DX, LINE_AT.1, &name, font::TEXT);
        } else {
            pen.eng(canvas, GROUP, UNDECIDED, x + NAME_DX, LINE_AT.1, font::TEXT);
        }

        pen.ok_button(canvas, OK.x, OK.y, OK_MODE);
    }
}
