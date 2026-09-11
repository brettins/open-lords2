//! **The message scroll** — `Msg_DrawWindow` (`0x0047309E`, 10,915 bytes) and
//! `Msg_HandleInput` (`0x0047685D`), the arm that runs before every other arm
//! in the game.
//!
//! # It is not a screen in the original, and it is one here
//!
//! `g_screenId` does not change when a message opens. The window is painted
//! over whatever was up by `Msg_Pump`, which `Battle_Frame` calls once a frame,
//! and its input is the **first** thing `Screen_FrameInput` does:
//!
//! ```c
//! Screen_HitRegion();
//! iVar2 = Msg_HandleInput();                                    /* 0x0047685D */
//! if ((iVar2 == 0) && (iVar2 = Screen_HandleInput(), iVar2 == 0)) {
//!     … the fifty per-screen arms …
//! }
//! ```
//!
//! Our machine says that with an **overlay** on top of the stack: an overlay
//! draws over what is beneath it, and [`crate::screen::Machine::handle`] offers
//! an event to the top screen first. The one place the analogy has to be exact
//! is the *fall-through*: `Msg_HandleInput` returning zero is a click that
//! reaches the screen underneath, and that is [`Transition::Pass`]. A left click
//! that misses the corner button and misses the answer widgets is **not
//! consumed** — see [`MessageScreen::handle`].
//!
//! # What the window is, arm by arm
//!
//! Twenty categories, enumerated in [`crate::message::category`]. Nine have a
//! layout of their own here and the rest share the plain one; every one of them
//! draws `Ui_OkButton` in the same corner — `(x + w - 0x30, y + h - 0x30)` —
//! except the floating tip, which draws no button at all and cannot be closed
//! by hand.
//!
//! # The three input arms and the five answers
//!
//! `Msg_HandleInput`, in the original's own order, which is not the order a
//! reader expects:
//!
//! 1. **a right release closes it, whatever it is** — tested *before* the
//!    widgets, so right-clicking an alliance offer is neither yes nor no;
//! 2. **five widget tests**, one per prompt, each running its handler;
//! 3. **a left press in the 48 × 48 box** round the corner button.
//!
//! Everything else returns zero and falls through.
//!
//! # What this screen deliberately does not do
//!
//! * **Voice.** `Msg_PlayVoice` (`0x004B35C1`) is a `.wav` lookup and belongs to
//!   the audio branch, and it is built: [`crate::audio::voice_tick`] is the
//!   per-category schedule and [`crate::audio::Director`] fires it. The five
//!   timer values (`0x7C6`, `0x776`, `0x708`, `0x76C`, `0x5A`) were cited here
//!   as *"recorded in `crate::message`"* and **had never been written there** --
//!   a citation that did not resolve, which is a rule with no way in wearing a
//!   doc comment. They live beside `voice_tick` now, with the category each one
//!   belongs to, which was the half nobody had recorded. `docs/decisions.md`
//!   C126.
//! * **The Smacker.** Categories `0x0D` and `0x0E` play `cap_cty<n>.smk` and
//!   `0x004F0340` when `g_optAnimations` is on, and *dismiss themselves from
//!   inside the draw* to do it. We have no video player; the unanimated branch
//!   of both is what is built, which is the branch the original takes with
//!   animations off. `docs/arms.json` records the animated half as `missing`.

use l2_view::Canvas;

use crate::input::{Event, Key, Rect};
use crate::press::{Kind, Press, Widget};
use crate::message::{self, category, Prompt, Record, Shape};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

/// `FUN_004093E0(…, …, …, …)`'s border set. Every one of `Msg_DrawWindow`'s
/// arms passes the four-argument form, which is border set 1.
const BOX_SET: usize = 1;

/// `Faces.pl8` — the portrait beside a lord's letter, blitted at
/// `(x + 0x10, y + 0x12)` into a `Ui_DrawInsetRect(x + 0xF, y + 0x11, 0x52,
/// 0x4E)` well. `FUN_00475D73` picks the frame: `lord * 3 - 3`, **12 for a
/// human**, 16 for realm 0 and 17 for a realm above 5.
const FACES: &str = "Faces.pl8";
const FACE_AT: (i32, i32) = (0x10, 0x12);
const FACE_WELL: (i32, i32, i32, i32) = (0xF, 0x11, 0x52, 0x4E);

/// `Pl8_DrawFrame(g_panelsSheet, shieldIndex + 0xFF, x + w - 0x1E, y + 0x12)`.
const SHIELD_BASE: usize = 0xFF;

/// `FUN_00475D73`'s frame ladder.
pub fn face_frame(lord: u8, is_human: bool, realm: u8) -> usize {
    if realm < 1 {
        return 16;
    }
    if realm > 5 {
        return 17;
    }
    if is_human {
        return 12;
    }
    (lord as usize).saturating_mul(3).saturating_sub(3)
}

/// **The message scroll.**
///
/// It holds no state: the record, the timer and the ring are all on
/// [`crate::Game`], exactly as the original keeps them in its data segment,
/// because the window is opened by the frame driver and not by anything the
/// player did.
pub struct MessageScreen {
    /// **Where the pointer was**, for the one layout that needs it: the
    /// floating tip is placed at `g_mouseX`/`g_mouseY`, which the original reads
    /// straight out of the globals the window procedure writes.
    ///
    /// Tracked here rather than on [`crate::Game`] because nothing else in this
    /// workspace wants it and a cursor position on the world is a cursor
    /// position in the save. It starts at the middle of the screen, which is
    /// where the tip's own clamp puts it anyway if the pointer has not moved.
    pointer: (i32, i32),
    /// The open prompt's press timer.
    ///
    /// **All five prompt tables are `Widget_Test` kind 4**, read out of `+0x0F`
    /// of `0x004DDA90`, `0x004DDAC0`, `0x004DDAF0`, `0x004DDB20` and
    /// `0x004DDB50` — the same pair of mailed hands as the yes/no box and a
    /// *different kind*, which is why the kind has to be read rather than
    /// inferred from the picture. The repeat is unreachable: every one of the
    /// five handlers calls `Msg_Dismiss` first, so the table is gone before a
    /// second fire could come. What kind 4 buys is the pressed picture.
    press: Press,
}

/// The open prompt's two widgets as a table. Index 0 is **yes**, hotspot id 1.
fn prompt_widgets(prompt: Prompt) -> [Widget; 2] {
    let [yes, no] = prompt.widgets();
    let side = Prompt::SIDE;
    [
        Widget::new(Rect::new(yes.0, yes.1, side, side), Kind::Repeat),
        Widget::new(Rect::new(no.0, no.1, side, side), Kind::Repeat),
    ]
}

impl MessageScreen {
    pub fn new() -> MessageScreen {
        MessageScreen { pointer: (320, 240), press: Press::new() }
    }

    /// The record on screen, or `None` for the one frame the machine may still
    /// draw this after the queue closed it.
    fn record(ctx: &Ctx) -> Option<Record> {
        ctx.game.messages.open().copied()
    }
}

impl Default for MessageScreen {
    fn default() -> MessageScreen {
        MessageScreen::new()
    }
}

impl Screen for MessageScreen {
    fn id(&self) -> ScreenId {
        ScreenId::Message
    }

    /// `Widget_Test`'s `Sound_RestartSlot(1)`, carried up to the audio
    /// layer. See [`Screen::take_clicks`].
    fn take_clicks(&mut self) -> u8 {
        self.press.take_clicks()
    }

    fn title(&self, ctx: &Ctx) -> String {
        match MessageScreen::record(ctx) {
            Some(r) => format!("Message {} — category {:#04x}", r.group, r.category),
            None => "Message".into(),
        }
    }

    /// It paints a box over whatever raised it and never clears.
    fn is_overlay(&self) -> bool {
        true
    }

    /// **`Msg_HandleInput` (`0x0047685D`), in its own order.**
    ///
    /// The three things worth noticing, because none of them is what a reader
    /// would guess from the function's shape:
    ///
    /// * the **right** release is tested before the widgets, so it closes an
    ///   unanswered question rather than declining it;
    /// * the corner button's hit box is **48 × 48**, twice the picture, while
    ///   every other screen in the game uses `Ui_OkButtonClicked`'s 24 × 24;
    /// * anything else **falls through** to the screen underneath, which is how
    ///   `Map_Click`'s dismissal and the campaign sidebar both stay live with a
    ///   message up.
    fn handle(&mut self, event: Event, ctx: &mut Ctx) -> Transition {
        let Some(record) = MessageScreen::record(ctx) else { return Transition::Pop };
        match event {
            // `else { Msg_Dismiss(); return 1; }` — the whole of the
            // right-button branch, with no category test in front of it.
            // arm: 0x0047685D/message-scroll-dismiss right-release
            Event::RightClick { .. } => leave(ctx),
            Event::Click { x, y } => {
                // The five `Widget_Test` calls, in the original's order:
                // category 0x11, then 10, then 0x0B, then the three groups of
                // category 0x0C. Each returns 1 whether or not the click was on
                // a button, so a miss inside a prompt does NOT fall through.
                if let Some(prompt) = record.answer_widgets() {
                    if let Some(i) = self.press.event(&prompt_widgets(prompt), event) {
                        return answer(ctx, prompt, i == 0);
                    }
                    // `Widget_Test` returning 0 falls on through to the corner
                    // button below, which is why a prompt can still be closed
                    // without answering it.
                }
                let shape = record.shape();
                if shape.has_ok_button() {
                    // A tip window's corner is wherever its wrapped text put it,
                    // so the hit box is computed from the same text the draw
                    // wraps. Before the tips existed this was `frame_of`, which
                    // has no row for `0x05`…`0x09` — a tip could not be closed
                    // with the left button at all.
                    if let Some(frame) = window_frame(ctx, &record) {
                        if frame.ok_hitbox().contains(x, y) {
                            // `FUN_004B18E3()` consumes the click so the screen
                            // underneath cannot also act on it, then dismisses.
                            // arm: 0x0047685D/message-ok-dismiss left-press
                            return leave(ctx);
                        }
                    }
                }
                // **Zero: not mine.** `Screen_HandleInput` and then the
                // per-screen arms get this click. On the campaign map that is
                // `Map_Click`, whose whole body is skipped and whose `else` is
                // `Msg_DismissUnlessQuestion`.
                Transition::Pass
            }
            // **Ours.** `Msg_HandleInput` tests no key at all, and the window
            // procedure has no arm for one either. A demo that can be driven
            // from the keyboard is worth more than the omission is faithful,
            // and this is counted rather than hidden.
            // arm: ours/message-keyboard-dismiss key
            Event::KeyDown(Key::Escape) | Event::KeyDown(Key::Enter) => leave(ctx),
            Event::Pointer { x, y } => {
                self.pointer = (x, y);
                if let Some(prompt) = record.answer_widgets() {
                    self.press.event(&prompt_widgets(prompt), event);
                }
                Transition::Pass
            }
            Event::Release { .. } => {
                self.press.release();
                Transition::Pass
            }
            _ => Transition::Pass,
        }
    }

    /// **The draw's own side effects** — the tip clamp and the three arms
    /// `Msg_DrawWindow` runs on the frame the window opens.
    ///
    /// The **timer** is not here: it is `Msg_Pump`'s, and it lives in
    /// [`crate::screen::Machine::pump_messages`] beside the pull it is exclusive
    /// with. That split is the original's and it matters — see that function.
    ///
    /// **In the original all of this is in the draw.** It cannot be here:
    /// [`Screen::draw`] takes a `&Ctx`, which is the compiler enforcing that
    /// painting a frame cannot change the world (`crates/l2-game/src/screen.rs`,
    /// *Draw cannot mutate*). The original's draw and input run once each per
    /// frame in a fixed order, so moving these three arms into `update` changes
    /// nothing about when they fire; it is recorded because it is a difference.
    fn update(&mut self, ctx: &mut Ctx) -> Transition {
        // `Widget_Test`'s countdown, which runs the pressed picture down. It
        // cannot fire: the prompts are kind 4, whose fire is on the press, and
        // the handler dismisses before a repeat could arrive.
        self.press.tick();
        if !ctx.game.messages.is_open() {
            return Transition::Pop;
        }
        // `Msg_DrawWindow`'s `g_messageTimer == 2000` arms, one of which can
        // close the window and one of which can end the game.
        if !message::show(ctx.game) {
            // The two arms that reach here are the alliance lapses, which cannot
            // set an outcome — but the test is on the outcome and not on the
            // category, because that is what `Msg_Dismiss` tests.
            if ctx.game.campaign.outcome.is_over() {
                return Transition::Replace(ScreenId::Conquest);
            }
            return Transition::Pop;
        }
        Transition::Stay
    }

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas) {
        let Some(record) = MessageScreen::record(ctx) else { return };
        let pen = Pen {
            assets: &ctx.assets.shell,
            ink: &ctx.assets.ink,
            chrome: ctx.assets.chrome.as_ref(),
            shadow: Some(font::SHADOW),
            caps: None,
        };
        if let Shape::Paragraphs(n) = record.shape() {
            draw_paragraphs(&pen, ctx, canvas, &record, n);
            return;
        }
        let Some(frame) = message::frame_of(&record) else {
            // A category with no constant geometry — the tip, the help window
            // and the two letter categories — or one `Msg_DrawWindow` has no arm
            // for. The tip is the only one of them anything enqueues today.
            draw_tip(&pen, ctx, canvas, &record, self.pointer);
            return;
        };
        pen.window(canvas, frame.x, frame.y, frame.w / 16, frame.h / 16, BOX_SET);

        match record.shape() {
            Shape::Letter | Shape::Prompt => draw_letter(&pen, ctx, canvas, &record, frame),
            Shape::CountyPortrait => draw_county_portrait(&pen, ctx, canvas, &record, frame),
            Shape::Ending => draw_ending(&pen, ctx, canvas, &record, frame),
            Shape::Garrison => draw_garrison(&pen, ctx, canvas, &record, frame),
            Shape::Event => draw_event(&pen, ctx, canvas, &record, frame),
            _ => draw_notice(&pen, ctx, canvas, &record, frame),
        }

        if let Some(prompt) = record.answer_widgets() {
            draw_prompt(&pen, canvas, prompt, self.press.pressed());
        }
        if record.shape().has_ok_button() {
            let (x, y) = frame.ok_button();
            pen.ok_button(canvas, x, y, 0);
        }
    }
}

// ------------------------------------------------------------------ the words

/// The county's name — `Eng_DrawString(100, g_scenarioIndex * 0x14 + county)`.
/// **Twenty per map slot**, not sixteen; the stride is the table's, not the
/// county count's.
fn county_name(ctx: &Ctx, id: u8) -> String {
    let name = ctx.assets.shell.text(message::GROUP_COUNTY, ctx.game.map_slot * 20 + id as usize);
    if name.is_empty() {
        format!("COUNTY {id}")
    } else {
        name.to_string()
    }
}

/// `g_playerNames[realm]`, with `L2.eng` group 7 standing in for a realm whose
/// name was never set — which is what `Game_NewGame` copies in in the first
/// place.
fn lord_name(ctx: &Ctx, realm: u8) -> String {
    let named = ctx.game.player_names.get(realm as usize).map(|n| n.as_str()).unwrap_or_default();
    if !named.is_empty() {
        return named;
    }
    let lord = ctx.game.kingdom.realms.get(realm as usize).map_or(0, |r| r.lord);
    let s = ctx.assets.shell.text(7, lord.min(4) as usize);
    if s.is_empty() {
        format!("REALM {realm}")
    } else {
        s.to_string()
    }
}

/// The group's own label, index 0 — *"Index 0 of a group is a label the game
/// wrote about itself"*, and here it is drawn as the heading of the window.
fn label(ctx: &Ctx, group: u16) -> String {
    let s = ctx.assets.shell.text(group as usize, 0);
    if s.is_empty() {
        format!("MESSAGE {group}")
    } else {
        s.to_string()
    }
}

/// The body — `FUN_0040328E(group, variant + 1, …)`, one wrapped paragraph.
fn body(ctx: &Ctx, record: &Record) -> String {
    ctx.assets.shell.text(record.group as usize, record.body_index()).to_string()
}

// ---------------------------------------------------------------- the layouts

/// **Category `0x00` and its near neighbours.** The heading is a three-way
/// choice and it is the only interesting thing in the arm:
///
/// ```c
/// if (county == 0) {
///     if (spare == 0) Ui_DrawCentred(group, 0, …);              /* the label   */
///     else            Ui_DrawText(g_playerNames + spare * 0x2C, …); /* a lord   */
/// } else              Ui_DrawCentred(100, scenario * 0x14 + county, …); /* a county */
/// ```
///
/// So `+0x13` is not spare: it puts a **second realm's** name where the group's
/// own label would have gone.
fn draw_notice(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, f: message::Frame) {
    let heading = if record.county != 0 {
        county_name(ctx, record.county)
    } else if record.spare != 0 {
        lord_name(ctx, record.spare)
    } else {
        label(ctx, record.group)
    };
    if record.county == 0 && record.spare != 0 {
        pen.heading(canvas, f.x + 0x20, f.y + 0x20, &heading, font::TEXT);
    } else {
        pen.heading_centred(canvas, f.x + 0x10, f.y + 0x20, f.w - 0x20, &heading, font::TEXT);
    }
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x50, f.w - 0x40, &body(ctx, record), font::TEXT);
}

/// **Categories `0x01`, `0x0A` and `0x0B`** — a lord's letter. All three draw
/// the identical head: shield, portrait well, *"From "* plus the sender's name
/// in the heading font, then the group's label, then the wrapped body.
///
/// The pay prompt inserts `Ui_DrawCount(g_diploHelpPrice, 0, …)` immediately
/// after the label, on the same line, which is why the price reads as part of
/// the sentence rather than as a field.
fn draw_letter(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, f: message::Frame) {
    draw_portrait(pen, ctx, canvas, record.from, f);
    let from = ctx.assets.shell.text(message::GROUP_FROM, 0).to_string();
    let mut x = pen.heading(canvas, f.x + 0x6C, f.y + 0x20, &from, font::TEXT);
    pen.heading(canvas, x, f.y + 0x20, &lord_name(ctx, record.from), font::TEXT);

    x = pen.body(canvas, f.x + 0x6C, f.y + 0x40, &label(ctx, record.group), font::TEXT);
    if record.category == category::PAY_PROMPT {
        let price = ctx.game.kingdom.diplomacy.help_price;
        pen.count(canvas, x, f.y + 0x40, price, 0, font::TEXT);
    } else if record.spare != 0 {
        pen.body(canvas, x, f.y + 0x40, &lord_name(ctx, record.spare), font::TEXT);
    } else if record.county != 0 {
        pen.body(canvas, x, f.y + 0x40, &county_name(ctx, record.county), font::TEXT);
    }
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x70, f.w - 0x40, &body(ctx, record), font::TEXT);
}

/// **Category `0x02`** — the portrait panel with `L2.eng` 109/1 over the county
/// name, both centred in 0x140 rather than in the window's own width.
fn draw_county_portrait(
    pen: &Pen,
    ctx: &Ctx,
    canvas: &mut Canvas,
    record: &Record,
    f: message::Frame,
) {
    draw_portrait_well(pen, canvas, f);
    let heading = ctx.assets.shell.text(message::GROUP_FROM, 1).to_string();
    pen.heading_centred(canvas, f.x + 0x6C, f.y + 0x18, 0x140, &heading, font::TEXT);
    let county = county_name(ctx, record.county);
    pen.heading_centred(canvas, f.x + 0x6C, f.y + 0x38, 0x140, &county, font::TEXT);
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x70, f.w - 0x40, &body(ctx, record), font::TEXT);
}

/// **Category `0x0E`, the ending.** The name at the top is the *local player's*
/// for group 225 and the *sender's* for everything else — which is what makes
/// one layout serve *"Victory!"*, *"Defeat!"* and an AI's obituary.
fn draw_ending(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, f: message::Frame) {
    draw_portrait_well(pen, canvas, f);
    let who = if record.group == l2_kingdom::victory::MSG_VICTORY {
        ctx.game.player
    } else {
        record.from
    };
    pen.heading_centred(canvas, f.x + 0x20, f.y + 0x20, 0x1A0, &lord_name(ctx, who), font::TEXT);
    pen.body_centred(
        canvas,
        f.x + 0x20,
        f.y + 0x48,
        f.w - 0x40,
        &label(ctx, record.group),
        font::TEXT,
    );
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x70, f.w - 0x40, &body(ctx, record), font::TEXT);
}

/// **Category `0x11`** — *"Cannot garrison castle."* Two numbers under the
/// body: how much room is left in the castle, and how many men the army has.
///
/// The first is `castleCapacity[castleType] - garrison`, and it is drawn with
/// the `'@'` blank lead so it lines up with the second.
fn draw_garrison(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, f: message::Frame) {
    pen.heading_centred(
        canvas,
        f.x + 0x10,
        f.y + 0x20,
        f.w - 0x20,
        &label(ctx, record.group),
        font::TEXT,
    );
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x40, f.w - 0x40, &body(ctx, record), font::TEXT);

    let county = ctx.game.kingdom.counties.get(record.county as usize);
    let garrison = county
        .map(|c| c.garrison_unit)
        .filter(|u| *u != 0)
        .and_then(|u| ctx.game.kingdom.campaign.units.get(u))
        .map_or(0, |u| u.men);
    // `*(int *)(&DAT_004D8A0C + castleType * 4)` — which is
    // `CASTLE_GARRISON_CAP[castleType - 1]`, because the table at `0x004D8A10`
    // is indexed by castle types **1..=5**. It looks like an off-by-one and is
    // not one; the entry it would read at type 0 is the tail of
    // `CASTLE_WORKFORCE` and a county with no castle cannot be garrisoned.
    let capacity =
        county.map_or(0, |c| l2_kingdom::industry::garrison_cap(&ctx.game.kingdom.tables, c.castle_type));
    let room = capacity - garrison;
    // `Msg_DrawWindow`: `Ui_DrawNumber(room, '@', &DAT_004D700C, x + 0x20,
    // y + 0x70)` and then the noun at `x + g_penAdvance + 0x20`. The suffix is
    // **one space**, so the old no-lead-plus-space had the digits and the noun
    // both four pixels left. **[V]**
    let body = shell::Face::Body;
    let x = pen.number_in(body, canvas, f.x + 0x20, f.y + 0x70, room, '@', " ", font::TEXT);
    pen.eng(canvas, record.group as usize, 2, x, f.y + 0x70, font::TEXT);

    let x = pen.eng(canvas, record.group as usize, 3, f.x + 0x20, f.y + 0x80, font::TEXT);
    let men = ctx
        .game
        .kingdom
        .campaign
        .units
        .get(record.variant as usize)
        .map_or(0, |u| u.men) as i32;
    // `Ui_DrawNumber(menTotal, '@', &DAT_004D7010, …)`, one space. **[V]**
    pen.number_in(body, canvas, x, f.y + 0x80, men, '@', " ", font::TEXT);
    pen.eng(canvas, record.group as usize, 4, f.x + 0x20, f.y + 0xB0, font::TEXT);
}

/// **Category `0x0F`, a county's random event** — `Msg_DrawWindow`'s arm at
/// `00470000.c:2067`, whose record `FUN_00448D7E` posts as
/// `Msg_Enqueue(0, g_localPlayer, county.eventId, 0, 0x0F, county, 0, 0)`.
///
/// ```c
/// Ui_DrawCentred(group, 0, x + 0x10, y + 0x20, w - 0x20, &g_fontHeading, 0x3F);
/// FUN_0040328E(group, 1, x + 0x20, y + 0x40, w - 0x40, 400, 0, 0, &g_fontBody, 0x3F);
/// g_penAdvance = 0;
/// switch (g_counties[county].eventId) {       /* the county's, not the record's */
///   0x87 Rats:          Ui_DrawCount(+0x278, 2, x + 0x20, y + 0x90) + 77/0x19
///   0x8B Grain found:   Ui_DrawCount(+0x278, 2, …)                  + 77/0x1A
///   0x88 Mad cows:      Ui_DrawCount(+0x274, 4, …)                  + 77/0x14
///   0x89 Wolves:        Ui_DrawCount(+0x274, 4, …)                  + 77/0x15
///   0x8C Bad cattle:    Ui_DrawCount(+0x274, 4, …)                  + 77/0x16
///   0x8D Cow bonanza:   Ui_DrawCount(+0x274, 4, …)                  + 77/0x17
///   0x8A Plague:        Ui_DrawNumber(+0x2F8, '@', " ", …)          + 77/0x1D
///   0x8E Wedding fever: Ui_DrawNumber(+0x2F8, '@', " ", …)          + 77/0x1E
/// }
/// ```
///
/// **The heading is the group's own label, never the county's name** —
/// [`draw_notice`] would have put the county there, and this arm has no county
/// name on it at all. The body is index 1 whatever the variant.
///
/// **Six of the eight number lines are drawn and two are not.** Plague and
/// Wedding fever print county `+0x2F8`, `Population_UpdateAll`'s event swing,
/// which no import or rule of ours carries: our population rule computes that
/// swing differently (`docs/decisions.md` C164), so a carried byte would print
/// a number the rule did not apply. Their word starts at the pen after that
/// number, so it cannot be placed without it and is not drawn either.
///
/// **And nothing posts this record yet.** `FUN_00448D7E` runs once a frame
/// from the loop at `0x004B99C0` for `g_selectedCounty`, and posts when the
/// county's `eventFired` is set and it is the local player's; no function of
/// ours does. The painter is here so that the day something does, the letter
/// says what the original's says.
fn draw_event(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, f: message::Frame) {
    pen.heading_centred(
        canvas,
        f.x + 0x10,
        f.y + 0x20,
        f.w - 0x20,
        &label(ctx, record.group),
        font::TEXT,
    );
    let text = ctx.assets.shell.text(record.group as usize, 1).to_string();
    pen.body_wrapped(canvas, f.x + 0x20, f.y + 0x40, f.w - 0x40, &text, font::TEXT);

    let Some(c) = ctx.game.kingdom.counties.get(record.county as usize) else { return };
    let (value, noun, word) = match c.event_id {
        0x87 => (c.grain_event_change, EVENT_NOUN_SACK, 0x19),
        0x8B => (c.grain_event_change, EVENT_NOUN_SACK, 0x1A),
        0x88 => (c.herd_event_change, EVENT_NOUN_ANIMAL, 0x14),
        0x89 => (c.herd_event_change, EVENT_NOUN_ANIMAL, 0x15),
        0x8C => (c.herd_event_change, EVENT_NOUN_ANIMAL, 0x16),
        0x8D => (c.herd_event_change, EVENT_NOUN_ANIMAL, 0x17),
        // 0x8A and 0x8E: `+0x2F8`, not carried — see above.
        _ => return,
    };
    let (x, y) = (f.x + 0x20, f.y + 0x90);
    let next = pen.count(canvas, x, y, value, noun, font::TEXT);
    pen.eng(canvas, EVENT_GROUP, word, next, y, font::TEXT);
}

/// `L2.eng` group 77, whose indices `0x14` … `0x1A` are the event letters'
/// last words: *"died of disease."*, *"eaten by rats."* and the rest.
const EVENT_GROUP: usize = 77;
/// Group 8's *Sack* and *Animal*, the two nouns the event arm counts in.
const EVENT_NOUN_SACK: usize = 2;
const EVENT_NOUN_ANIMAL: usize = 4;

/// **Category `0x04`, the floating tip.** Its box follows the cursor, clamped
/// into `0x50 ..= 0xF0` on both axes — so it never leaves the middle of the
/// screen however far out the pointer is.
///
/// ```c
/// x = (mouseX < 0xF1) ? mouseX + 0x20 : mouseX - 0xA8;   clamp 0x50 .. 0xF0
/// y = (mouseY < 0xF1) ? mouseY + 0x20 : mouseY - 0x50;   clamp 0x50 .. 0xF0
/// ```
///
/// One centred line of the group's label, and **no button**.
fn draw_tip(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, pointer: (i32, i32)) {
    if record.category != category::TIP {
        return;
    }
    let (mx, my) = pointer;
    let x = if mx < 0xF1 { mx + 0x20 } else { mx - 0xA8 }.clamp(0x50, 0xF0);
    let y = if my < 0xF1 { my + 0x20 } else { my - 0x50 }.clamp(0x50, 0xF0);
    pen.window(canvas, x, y, 0xB, 3, BOX_SET);
    pen.body_centred(canvas, x, y + 0x10, 0xB0, &label(ctx, record.group), font::TEXT);
}

// ------------------------------------------------------------------ the parts

/// `Ui_DrawInsetRect(x + 0xF, y + 0x11, 0x52, 0x4E)` — the recess the portrait
/// sits in. Drawn by every layout that has a portrait, including the two that
/// then blit nothing into it.
fn draw_portrait_well(pen: &Pen, canvas: &mut Canvas, f: message::Frame) {
    let (dx, dy, w, h) = FACE_WELL;
    pen.inset(canvas, Rect::new(f.x + dx, f.y + dy, w, h));
}

/// The well, the face and the shield.
fn draw_portrait(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, realm: u8, f: message::Frame) {
    let r = ctx.game.kingdom.realms.get(realm as usize);
    if let (Some(r), Some(chrome)) = (r, pen.chrome) {
        let shield = SHIELD_BASE + r.shield_index.clamp(0, 5) as usize;
        chrome.draw_panel_frame(canvas, shield, f.x + f.w - 0x1E, f.y + 0x12);
    }
    draw_portrait_well(pen, canvas, f);
    let frame = face_frame(
        r.map_or(0, |r| r.lord),
        r.is_some_and(|r| r.is_human),
        realm,
    );
    if let Some(sheet) = pen.assets.sheet(FACES) {
        if let Some(bitmap) = sheet.frame(frame) {
            canvas.blit(&bitmap, f.x + FACE_AT.0, f.y + FACE_AT.1);
        }
    }
}

/// **Draw the scroll's two clickable pictures again, and nothing else.**
///
/// This exists for a test and says so. `docs/agents.md`: *"where an operation is
/// idempotent, assert idempotence rather than an effect"* — a sprite is an
/// opaque blit, so drawing the corner button and the answer buttons a second
/// time over themselves changes no pixel, **but only if they were there the
/// first time**. Deleting either draw call above makes this one *add* them, and
/// the two canvases stop being equal.
///
/// It is the build stamp's assertion, and it is here rather than in the test
/// because the test may not know the geometry — computing the probe from the
/// same constants the code reads is what makes an ablation prove nothing.
pub fn repaint_clickables(ctx: &Ctx, canvas: &mut Canvas, record: &Record) {
    let pen = Pen {
        assets: &ctx.assets.shell,
        ink: &ctx.assets.ink,
        chrome: ctx.assets.chrome.as_ref(),
        shadow: Some(font::SHADOW),
        caps: None,
    };
    // `None`: this repaints what a FRESH frame would draw, and a press timer is
    // per-screen state the caller does not have. The test that uses it compares
    // an untouched draw with a second one, so both sides are up.
    if let Some(prompt) = record.answer_widgets() {
        draw_prompt(&pen, canvas, prompt, None);
    }
    if record.shape().has_ok_button() {
        if let Some(frame) = window_frame(ctx, record) {
            let (x, y) = frame.ok_button();
            pen.ok_button(canvas, x, y, 0);
        }
    }
}

// ------------------------------------------------------------ the tip window

/// The window a record is drawn in: the constant geometry of
/// [`message::frame_of`], or for a tip window the geometry its text computes.
pub fn window_frame(ctx: &Ctx, record: &Record) -> Option<message::Frame> {
    match record.shape() {
        Shape::Paragraphs(n) => Some(tip_layout(ctx, record, n).0.frame),
        _ => message::frame_of(record),
    }
}

/// **The tip window's paragraphs, broken into lines the way `FUN_0040328E`
/// breaks them**, and the layout those line counts give.
///
/// The words are [`crate::tip::words`] — the player's `L2.eng`, and our
/// transcription only where the file is silent — and they are measured in the
/// body font the paragraphs are drawn in, which is `&g_fontBody` in both of
/// `Msg_DrawWindow`'s loops.
pub fn tip_layout(ctx: &Ctx, record: &Record, n: usize) -> (message::Paragraphs, Vec<Vec<String>>) {
    let texts: Vec<Vec<String>> = (1..=n)
        .map(|i| {
            let words = crate::tip::words(&ctx.assets.shell, record.group, i);
            message::break_lines(&words, message::PARAGRAPH_WIDTH, |c| glyph_width(ctx, c))
        })
        .collect();
    let lines: Vec<usize> = texts.iter().map(Vec::len).collect();
    (message::paragraph_layout(&lines), texts)
}

/// `FUN_004015B9(c, &g_fontBody)` — one glyph's advance. `[I]` for a character
/// the font has no frame for: the original answers 0 and
/// [`crate::shell::font::Font::width`] answers a space's advance; no tip string
/// has been found to contain one.
fn glyph_width(ctx: &Ctx, c: char) -> i32 {
    let s = c.to_string();
    match &ctx.assets.shell.body {
        Some(f) => f.width(&s),
        None => l2_view::text::width(&s),
    }
}

/// **Categories `0x05`…`0x09` — the tip window.** `Msg_DrawWindow`'s last
/// arm, drawn to [`message::paragraph_layout`]: the box, the group's label in
/// the heading font, `category − 4` paragraphs in the body font, and the OK
/// button in the corner the text decided.
///
/// The first of the arm's two loops paints every paragraph at one height
/// before the box exists, to measure them; the box then covers it, so it is
/// not reproduced as paint.
fn draw_paragraphs(pen: &Pen, ctx: &Ctx, canvas: &mut Canvas, record: &Record, n: usize) {
    let (layout, texts) = tip_layout(ctx, record, n);
    let f = layout.frame;
    pen.window(canvas, f.x, f.y, f.w / 16, f.h / 16, BOX_SET);
    let heading = crate::tip::words(&ctx.assets.shell, record.group, 0);
    pen.heading(canvas, layout.heading.0, layout.heading.1, &heading, font::TEXT);
    for (lines, top) in texts.iter().zip(&layout.tops) {
        for (k, line) in lines.iter().enumerate() {
            pen.body(canvas, f.x + 0x10, top + 0x10 * k as i32, line, font::TEXT);
        }
    }
    let (x, y) = f.ok_button();
    pen.ok_button(canvas, x, y, 0);
}

/// `Widget_Draw(0, 0, table, 2)` — the two mailed hands, `System.pl8` frames 29
/// and 31.
fn draw_prompt(pen: &Pen, canvas: &mut Canvas, prompt: Prompt, down: Option<usize>) {
    let [yes, no] = prompt.widgets();
    // `Widget_Draw` adds one to the frame while `+0x0D` runs.
    for (i, (at, frame)) in [(yes, Prompt::FRAME_YES), (no, Prompt::FRAME_NO)].into_iter().enumerate() {
        let frame = if down == Some(i) { frame + 1 } else { frame };
        if !pen.system_frame(canvas, frame, at.0, at.1) {
            crate::shell::button_recess(canvas, at.0, at.1, Prompt::SIDE, Prompt::SIDE);
        }
    }
}

// ----------------------------------------------------------------- the answers

/// **`Msg_Dismiss` and what it asks for.**
///
/// Its last three lines are the ending — `if (outcome == 10 || outcome == 11)
/// { Campaign_EnterConquest(); g_screenId = 0x1C; }` — so a dismissal is
/// sometimes a screen change and not a pop. Every arm that closes the window
/// goes through here, which is what stops one of them from forgetting.
fn leave(ctx: &mut Ctx) -> Transition {
    match message::dismiss(ctx.game) {
        message::Dismissal::GameOver(_) => Transition::Replace(ScreenId::Conquest),
        _ => Transition::Pop,
    }
}

/// **The five widget handlers**, which are the only two places a person answers
/// a lord and three more besides.
///
/// Every one of them calls `Msg_Dismiss` **first** and then acts on
/// `g_uiHotspotId`, so the window is gone before the rule runs — which is why
/// none of them has anything to say about a refusal.
fn answer(ctx: &mut Ctx, prompt: Prompt, yes: bool) -> Transition {
    match prompt {
        // `Diplo_PayHelpClicked` (`0x004367FF`):
        //   Msg_Dismiss();
        //   if (hotspot != 0) Diplo_PayForHelp(myAlly, me, g_diploHelpCounty, g_diploHelpPrice);
        // **Declining does nothing at all** — not even a letter back.
        // arm: 0x004367FF/pay-for-help-prompt left-press-repeat
        Prompt::PayForHelp => {
            message::dismiss(ctx.game);
            if yes {
                let me = ctx.game.player;
                let ally = ctx.game.kingdom.realms[me as usize].ally;
                let county = ctx.game.kingdom.diplomacy.help_county;
                let price = ctx.game.kingdom.diplomacy.help_price;
                l2_kingdom::diplomacy::pay_for_help(
                    &mut ctx.game.kingdom.realms,
                    ally,
                    me,
                    county,
                    price,
                );
            }
            Transition::Pop
        }
        // `FUN_00436872` (`0x00436872`):
        //   Msg_Dismiss();
        //   if (realms[offerer].isHuman || hotspot != 0)
        //       if (hotspot == 1) Diplo_FormAlliance(g_localPlayer, offerer);
        //
        // **The guard is the finding.** Declining an AI's offer in single player
        // runs *nothing*: no refusal, no grudge, no letter. The offer lapses
        // when the offering realm clears `offer_pending` on its next turn.
        // arm: 0x00436872/accept-alliance-prompt left-press-repeat
        Prompt::AcceptAlliance => {
            let offerer = ctx.game.messages.open().map_or(0, |r| r.from);
            message::dismiss(ctx.game);
            if yes && offerer != 0 {
                let me = ctx.game.player;
                l2_kingdom::diplomacy::form_alliance(&mut ctx.game.kingdom.realms, me, offerer);
            }
            Transition::Pop
        }
        // `FUN_004368FD` / `FUN_0043695D` — the *ally's* answer to a request the
        // player made. Both set `g_diploKind` to one of two values by hotspot
        // and post it: 7/8 for help, 9/10 for attack. Those four kinds are
        // beyond `l2_kingdom::diplomacy::Kind`'s seven, which stops at 6 — they
        // are network commands and not composer kinds, and `Net_SendCommand`
        // (`0x49`) is their only consumer. In a single-player game the two
        // handlers therefore do nothing but dismiss.
        // arm: 0x004368FD/answer-help-request left-press-repeat
        Prompt::AnswerHelpRequest | Prompt::AnswerAttackRequest => {
            message::dismiss(ctx.game);
            Transition::Pop
        }
        // `FUN_004376BB` (`0x004376BB`) — *"Cannot garrison castle."*
        //
        //   g_screenId = 0; g_redrawRequest = 2;          /* BEFORE the test */
        //   if (hotspot != 0) { … seed the basket …; Msg_Dismiss(); g_screenId = 0x11; }
        //
        // **No is not a dismissal.** The handler returns to the campaign map
        // without closing the window, so the prompt is still up and has to be
        // closed with the corner button or the right button. Reproduced;
        // `docs/bugs.md` B94.
        // arm: 0x004376BB/garrison-split-prompt left-press-repeat
        Prompt::Garrison => {
            if !yes {
                return Transition::Pass;
            }
            let unit = ctx.game.messages.open().map_or(0, |r| r.variant as usize);
            message::dismiss(ctx.game);
            if unit != 0 && ctx.game.kingdom.campaign.units.get(unit).is_some() {
                // `g_screenId = 0x11` — and `DivideScreen` seeds its own basket
                // from the army the first time it is drawn, which is
                // `FUN_004378B3`'s seeding that `FUN_004376BB` duplicates.
                return Transition::Replace(ScreenId::Divide(unit));
            }
            Transition::Pop
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `FUN_00475D73`'s ladder, which is four cases and not one.
    #[test]
    fn a_human_rival_shows_the_twelfth_face_and_a_lord_shows_his_own() {
        assert_eq!(face_frame(1, false, 3), 0);
        assert_eq!(face_frame(2, false, 3), 3);
        assert_eq!(face_frame(4, false, 3), 9);
        assert_eq!(face_frame(2, true, 3), 12, "a human rival");
        assert_eq!(face_frame(2, false, 0), 16, "realm 0 is the game itself");
        assert_eq!(face_frame(2, false, 6), 17, "and 6 is the merchant");
    }
}
