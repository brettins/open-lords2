#![allow(unused_imports)]
use super::*;
use super::queue::*;
use super::tests::*;
use l2_kingdom::diplomacy::Letter;
use l2_kingdom::victory::{self, Ending, Outcome, OutcomeStep};
use crate::game::Game;

/// One 24-byte ring record — `Msg_Enqueue`'s eight parameters, in memory order.
///
/// | offset | field | |
/// |---|---|---|
/// | `+0x00` | [`Record::to`] | recipient realm; **0 is everybody** |
/// | `+0x04` | [`Record::from`] | sender realm; 0 is the game itself |
/// | `+0x08` | [`Record::group`] | the `L2.eng` group, and **0 means empty slot** |
/// | `+0x0C` | [`Record::variant`] | which string of the group, drawn as `variant + 1` |
/// | `+0x11` | [`Record::category`] | which of [`category`]'s twenty layouts |
/// | `+0x12` | [`Record::county`] | a county id where the heading needs one |
/// | `+0x13` | [`Record::spare`] | a **second realm**, whose name replaces the heading |
/// | `+0x14` | [`Record::payload`] | a number the layout may print |
///
/// `+0x10` is a hole; the record is 24 bytes and eight fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Record {
    pub to: u8,
    pub from: u8,
    pub group: u16,
    pub variant: u8,
    pub category: u8,
    pub county: u8,
    /// `+0x13`. **Not spare at all** in categories `0x00` and `0x01`: a non-zero
    /// value there is a realm index and `Ui_DrawText(g_playerNames + spare *
    /// 0x2C, …)` puts that lord's name where the county name or the group's own
    /// label would have gone. `County_ChangeOwner`'s group `0x72` sets it to the
    /// *previous* owner, which is what makes *"X has taken Y from Z"* one
    /// string.
    pub spare: u8,
    pub payload: i32,
}

impl Record {
    /// An empty slot. `Msg_Pump` tests `group == 0` and nothing else.
    pub fn is_empty(&self) -> bool {
        self.group == 0
    }

    /// The `L2.eng` index of the body string: `variant + 1`, past the group's
    /// own label at index 0.
    pub fn body_index(&self) -> usize {
        self.variant as usize + 1
    }

    /// Which layout `Msg_DrawWindow` picks for this record.
    pub fn shape(&self) -> Shape {
        Shape::of(self.category)
    }

    /// Whether this record carries a question — the four categories
    /// `Msg_DismissUnlessQuestion` (`0x00476710`) refuses to close, and the four
    /// `Msg_HandleInput` runs a `Widget_Test` for.
    ///
    /// **The two lists are the same four and that is checkable
    /// assumed**: `0x11`, `0x0A`, `0x0B`, `0x0C`. See
    /// [`MessageQueue::dismiss_unless_question`] and
    /// [`Record::answer_widgets`].
    pub fn is_question(&self) -> bool {
        matches!(
            self.category,
            category::GARRISON_PROMPT
                | category::PAY_PROMPT
                | category::ALLIANCE_PROMPT
                | category::DIPLOMACY
        )
    }

    /// The yes/no pair this record draws, or `None`.
    ///
    /// Category `0x0C` is the awkward one and it is awkward in the original
    /// too: `Msg_DrawDiplomacy` draws a widget for **three** of its eleven
    /// groups and `Msg_HandleInput` tests exactly those three, so
    /// [`Record::is_question`] is true for the whole category and this is false
    /// for eight of its groups. A right-click still closes those eight; there is
/// nothing to click.
    pub fn answer_widgets(&self) -> Option<Prompt> {
        match self.category {
            category::GARRISON_PROMPT => Some(Prompt::Garrison),
            category::PAY_PROMPT => Some(Prompt::PayForHelp),
            category::ALLIANCE_PROMPT => Some(Prompt::AcceptAlliance),
            category::DIPLOMACY => match self.group {
                0xF8 => Some(Prompt::AcceptAlliance),
                0xFA => Some(Prompt::AnswerHelpRequest),
                0xFB => Some(Prompt::AnswerAttackRequest),
                _ => None,
            },
            _ => None,
        }
    }
}

impl From<Letter> for Record {
    fn from(l: Letter) -> Record {
        Record {
            to: l.to,
            from: l.from,
            group: l.group,
            variant: l.variant,
            category: l.category,
            county: l.county,
            spare: 0,
            payload: l.payload,
        }
    }
}

impl From<Ending> for Record {
    fn from(e: Ending) -> Record {
        Record {
            to: e.to,
            from: e.from,
            group: e.group,
            // **Not zero.** `Ending::variant` was added for this: groups 194
            // and 195 hold sixteen lord-flavoured lines apiece and the ending
            // chain picks one. Dropping it here is how every lord in the game
            // would come to say the Knight's first line.
            variant: e.variant,
            category: e.category,
            county: 0,
            spare: 0,
            payload: 0,
        }
    }
}

impl Record {
    /// Back to the ending the victory rules read. They consult `group` and
    /// `from` and nothing else — see [`l2_kingdom::victory::outcome_of`].
    pub fn as_ending(&self) -> Ending {
        Ending {
            group: self.group,
            from: self.from,
            to: self.to,
            category: self.category,
            variant: self.variant,
        }
    }
}

/// Where each category's window goes, read off `Msg_DrawWindow`'s
/// `FUN_004093E0` calls one arm at a time.
///
/// Three categories are not here
/// [`category::TIP`] follows the cursor, [`category::HELP`] and the two
/// letter categories index tables in `.rdata`, and the paragraph stack computes
/// its height from how many paragraphs it drew.
pub fn frame_of(record: &Record) -> Option<Frame> {
    let f = match record.category {
        category::NOTICE => Frame::new(0x20, 0xA0, 0x1A0, 0xE0),
        category::COUNTY_TALL => Frame::new(0x20, 0xA0, 0x1A0, 0xF0),
        category::GARRISON_PROMPT | category::CASTLE => Frame::new(0x20, 0x90, 0x1A0, 0xF0),
        category::LETTER => Frame::new(0x10, 0x80, 0x1C0, 0x100),
        category::COUNTY_PORTRAIT => Frame::new(0x10, 0x90, 0x1C0, 0x100),
        category::COUNTY_NOTICE => Frame::new(0x10, 0xA0, 0x1C0, 0xF0),
        category::PAY_PROMPT | category::ALLIANCE_PROMPT => Frame::new(0x10, 0x80, 0x1C0, 0xF0),
        category::CAPTURE => Frame::new(0x20, 0xA0, 0x1A0, 0xC0),
        category::ENDING => Frame::new(0x10, 0x80, 0x1C0, 0xE0),
// **The one arm whose height is a rule**, and it
        // was written down here and then not applied — every event drew in the
        // short box, so the sixteen high-numbered events lost 0x20 of window and
        // had their corner button, and its 48 × 48 hit box, 0x20 too high.
        //
        // `[V]`, the arm's own first statement:
        // `DAT_00552ff8 = (short)eventId < 0x12E ? 0xC0: 0xE0;` — and the
        // *taller* box is the sixteen with **no** number line, which is the
        // opposite of what the note here claimed. The eight short ones
        // (`0x87`…`0x8E`) draw a count at `y + 0x90`, 0x30 clear of the bottom;
        // the tall ones spend the extra on body text.
        //
        // The id is the group, so the record carries it.
        category::EVENT => Frame::new(0x20, 0xA0, 0x1A0, event_height(record.group)),
        _ => return None,
    };
    Some(f)
}

/// `Msg_DrawWindow`'s category-`0x0F` height: `(short)eventId < 0x12E ? 0xC0 :
/// 0xE0`. The comparison is **signed 16-bit** on the county's stored id, so an
/// id that is not an event's at all — 0, the id a county that never drew one
/// carries — takes the short box.
pub fn event_height(group: u16) -> i32 {
    if (group as i16) < 0x12E {
        0xC0
    } else {
        0xE0
    }
}

/// The wrap width of a tip paragraph: `DAT_00553024 - 0x20`, with the window
/// `0x1C0` wide.
pub const PARAGRAPH_WIDTH: i32 = 0x1A0;

/// **Where a tip window's parts go** — `Msg_DrawWindow`'s categories
/// `0x05`…`0x09`, the one layout whose size is computed from its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paragraphs {
    /// The box, and so the OK button: [`Frame::ok_button`].
    pub frame: Frame,
    /// `Eng_DrawString(group, 0, x + 0x10, y + 0x14, heading)`.
    pub heading: (i32, i32),
    /// The top of each paragraph, drawn at `x + 0x10`.
    pub tops: Vec<i32>,
}

/// **The layout, from how many lines each paragraph wrapped to.** `[V]`:
///
/// ```c
/// x = 0x10; y = 0x40; w = 0x1C0; h = 0x20;            /* DAT_005CD4F8 */
/// for (i = 0; i <= category - 5; i++) {
///     FUN_0040328E(group, i + 1, x + 0x10, y + 0x40, w - 0x20, …);  /* h += 0x10 a line */
///     h += 0x10;
/// }
/// if (h < 0x61) y += 0x40;
/// height = h + y;                                     /* DAT_00552FF8 */
/// FUN_004093E0(x, y, w / 16, height / 16);
/// Eng_DrawString(group, 0, x + 0x10, y + 0x14, heading);
/// h = 0x40;
/// for (…) { FUN_0040328E(group, i + 1, x + 0x10, y + h, …); h += 0x10; }
/// Ui_OkButton(x + w - 0x30, height + y - 0x30, 0);
/// ```
///
/// Three things in it are not what a reader would write:
///
/// * **the text is measured by drawing it.** The first loop paints every
///   paragraph at one fixed height before the box exists, and only the running
///   total survives; the box then paints over the lot. It is not reproduced as
///   paint, because nothing of it is visible.
/// * **the box's height includes its own top.** `height = h + y`, so a window
///   moved down by the short-text rule also grows by the same 64 pixels.
/// * **a short tip is pushed down 64 pixels** when the measured height is below
/// `0x61`, and the
///   one-sentence tips.
pub fn paragraph_layout(lines: &[usize]) -> Paragraphs {
    const X: i32 = 0x10;
    const W: i32 = 0x1C0;
    let mut measured = 0x20;
    for &n in lines {
        measured += 0x10 * n as i32 + 0x10;
    }
    let mut y = 0x40;
    if measured < 0x61 {
        y += 0x40;
    }
    let mut tops = Vec::with_capacity(lines.len());
    let mut at = 0x40;
    for &n in lines {
        tops.push(y + at);
        at += 0x10 * n as i32 + 0x10;
    }
    Paragraphs { frame: Frame { x: X, y, w: W, h: measured + y }, heading: (X + 0x10, y + 0x14), tops }
}

/// **`FUN_0040328E`'s line breaking** (`0x0040328E`), with `FUN_004036F9`
/// (`0x004036F9`) as the word measure. `glyph` is `FUN_004015B9`'s width of one
/// non-space character in the font the text is drawn in.
///
/// Not [`crate::shell::Pen::wrap`], and the difference is the OK button:
///
/// * **a space is four pixels, whatever the font**, and it is measured as part
/// of the word *after* it — so a word fits only if it fits with its leading
///   space, even at the start of a line where that space is then not drawn;
/// * **the test is strict**: a line that would come out exactly `width` wide
///   breaks;
/// * **`$` separates words and has no width**; a character below `0x20` has no
///   width and does not separate;
/// * a word wider than a whole line is never placed, and the function draws
///   empty lines until its own guard of 99 gives out.
///
/// Always at least one line, because the draw is inside the loop.
pub fn break_lines(text: &str, width: i32, glyph: impl Fn(char) -> i32) -> Vec<String> {
    let chars: Vec<char> = text.chars().skip_while(|c| (*c as u32) < 0x20).collect();
    let mut at = 0;
    let mut lines = Vec::new();
    let mut more = true;
    let mut drawn = 0;
    while more {
        drawn += 1;
        if drawn >= 100 {
            break;
        }
        let mut used = 0;
        let mut line = String::new();
        let mut line_start = true;
        while more && used < width {
            let (w, n) = measure_word(&chars[at..], &glyph);
            used += w;
            if used < width {
                for _ in 0..n {
                    let c = chars[at];
                    at += 1;
                    if !line_start || c != ' ' {
                        line.push(c);
                        line_start = false;
                    }
                }
                if at >= chars.len() {
                    more = false;
                }
            } else if used == 0 {
                more = false;
            }
        }
        lines.push(line);
    }
    lines
}

/// `FUN_004036F9`: the width of the next word *including the spaces before it*,
/// and how many characters that is.
fn measure_word(rest: &[char], glyph: &impl Fn(char) -> i32) -> (i32, usize) {
    let (mut w, mut n, mut in_word) = (0, 0, false);
    for &c in rest.iter().take(1999) {
        match c {
            ' ' if in_word => return (w, n),
            ' ' => w += 4,
            '$' if in_word => return (w, n),
            '$' => {}
            c if (c as u32) > 0x1F => {
                w += glyph(c);
                in_word = true;
            }
            _ => {}
        }
        n += 1;
    }
    (w, n)
}

