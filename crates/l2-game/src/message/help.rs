//! **The help window** — `Msg_DrawWindow`'s category `0x13` arm
//! (`0x0047309E`), the only arm whose geometry is a table in `.data` rather
//! than constants in the code.
//!
//! The five topics are posted by `Menu_HelpHowDoI` (`0x0043480C`) and its four
//! siblings as groups `0x123`…`0x127`; the sixth record belongs to group 296,
//! the CD check no path in this build reaches.
//!
//! `[V]` the arm, read out of `00470000.c`:
//!
//! ```c
//! x = *(int *)(&g_helpWindowGeom + (group - 0x123) * 0x10);   /* 0x004D6EB8 */
//! y = *(int *)(&DAT_004d6ebc     + (group - 0x123) * 0x10);
//! w = *(int *)(&DAT_004d6ec0     + (group - 0x123) * 0x10);   /* in cells */
//! h = *(int *)(&DAT_004d6ec4     + (group - 0x123) * 0x10);   /* in cells */
//! FUN_004093e0(x, y, w, h);
//! Ui_OkButton(w * 0x10 + x - 0x30, h * 0x10 + y - 0x30, 0);
//! Ui_DrawCentred(group, 0, x + 0x10, y + 0x16, w * 0x10 - 0x20, &g_fontHeading, 0x3f);
//! DAT_005cd4f8 = 0;
//! for (i = 1; i <= *(int *)(&DAT_004d6a8c + group * 4); i++) {
//!     FUN_0040328e(group, i, x + 0x20, DAT_005cd4f8 + y + 0x32, w * 0x10 - 0x40,
//!                  400, 0, 0, &g_fontBody, 0x3f);
//!     DAT_005cd4f8 = DAT_005cd4f8 + 4;
//! }
//! ```
//!
//! * **the stored `w` and `h` are cells, not pixels.** Every other arm stores
//!   pixels and divides by sixteen at the `FUN_004093E0` call; this one stores
//! the divided value and multiplies it back for the button and the text
//!   width. [`frame`] returns pixels, like every other [`Frame`].
//!
//! * **the count table is indexed by the whole group id.** `DAT_004D6A8C + group
//!   * 4`, whose group-291 entry is the `0x004D6F18` the symbol table names.
//!
//!   `FUN_0040328E` itself adds `0x10` per line it wrapped to `DAT_005CD4F8`
//!   (`00400000.c`: `else { local_20 += 0x10; DAT_005cd4f8 += 0x10; }`), so the
//! `+ 4` here is the gap *between* paragraphs and the lines account for
//!   themselves. [`PARAGRAPH_GAP`].

use super::Frame;

/// `g_helpWindowGeom` (`0x004D6EB8`) — six `{x, y, w, h}` records indexed
/// `group - 291`, `w` and `h` in 16-pixel cells.
///
/// `[V]` read out of `Lords2.exe` at the address, all six:
///
/// group 291 is the FAQ index page and gets the short box, 292…295 the four
/// answers and all four get the same tall one, 296 the dead CD check.
const GEOM: [(i32, i32, i32, i32); 6] = [
    (32, 176, 26, 10),
    (16, 32, 28, 27),
    (16, 32, 28, 27),
    (16, 32, 28, 27),
    (16, 32, 28, 27),
    (32, 160, 26, 12),
];

/// `DAT_004D6A8C + group * 4` for the six groups the arm can be entered with.
///
/// `[V]` read out of the exe: `1, 5, 5, 5, 10, 1` — `strings − 1` for all six,
/// which is the rule `docs/formats/eng.md` §5 records. It is a table and not
/// that subtraction because the arm reads the table, and a group whose
/// `L2.eng` has been edited draws what the *exe* says.
const PARAGRAPHS: [usize; 6] = [1, 5, 5, 5, 10, 1];

pub const FIRST: u16 = 291;

/// **The gap between two paragraphs**, `DAT_005CD4F8 = DAT_005CD4F8 + 4`. The
/// lines inside a paragraph step themselves, sixteen pixels each, from inside
/// `FUN_0040328E`.
pub const PARAGRAPH_GAP: i32 = 4;

pub fn frame(group: u16) -> Option<Frame> {
    let (x, y, w, h) = *GEOM.get(group.checked_sub(FIRST)? as usize)?;
    Some(Frame { x, y, w: w * 16, h: h * 16 })
}

pub fn paragraphs(group: u16) -> usize {
    group
        .checked_sub(FIRST)
        .and_then(|i| PARAGRAPHS.get(i as usize))
        .copied()
        .unwrap_or(0)
}

pub fn heading(f: Frame) -> (i32, i32, i32) {
    (f.x + 0x10, f.y + 0x16, f.w - 0x20)
}

/// The first paragraph's top and the wrap width — `FUN_0040328E(group, i, x +
/// 0x20, y + 0x32, w - 0x40, …)`.
pub fn body(f: Frame) -> (i32, i32, i32) {
    (f.x + 0x20, f.y + 0x32, f.w - 0x40)
}

/// One of the window's strings, `CLAUDE.md` rule 6: the player's own `L2.eng`
/// group, and [`TEXT`] only where the file is silent.
pub fn words(shell: &crate::shell::ShellAssets, group: u16, index: usize) -> String {
    let s = shell.text(group as usize, index);
    if s.is_empty() {
        transcribed(group, index).to_string()
    } else {
        s.to_string()
    }
}

fn transcribed(group: u16, index: usize) -> &'static str {
    TEXT.iter()
        .find(|(g, _)| *g == group)
        .and_then(|(_, strings)| strings.get(index))
        .copied()
        .unwrap_or("")
}

/// **Our transcription of groups 291…295**, for an install whose `L2.eng`
/// cannot be read — the same fallback shape as [`crate::tip::words`]'s, and
/// held against the file by `tests/messages`.
pub const TEXT: &[(u16, &[&str])] = &[
    (
        291,
        &[
            "Frequently asked questions.",
            "Below are a few answers to some of the more commonly asked questions about Lords of the Realm. These replies should allow you to get started in the game",
        ],
    ),
    (
        292,
        &[
            "How do I grow grain?",
            "To grow grain, you need 3 things: grain fields, the grain itself, and grain laborers.",
            "To allocate a field to grain growing, click on any non-barren, non-damaged field, and select the grain symbol near the bottom of the panel that appears.",
            "If you have no grain, you must buy some from a merchant when one is in your county.",
            "The computer will automatically plant up to 5 sacks of grain in each field in Winter and allocate the labor needed; less will be planted if you do not have enough grain or labor. Each sack planted will grow into 12 sacks so long as there is enough labor to tend and harvest it during the year.",
            "Grain fields will change in appearance as the seasonal grain cycle progresses. Grain is only planted at the end of the winter turn, and is harvested at the end of the Autumn. You will see the extra grain in your county at the start of each Winter turn.",
        ],
    ),
    (
        293,
        &[
            "How do I build a castle?",
            "You may only have one castle in each county.",
            "To build a castle in a county without one, click on the Build Castle button on the control panel. The castle screen displays five castle types. As you click on each castle, the display in the upper right tells you the materials required for that design and how long it will take workers to build it.",
            "In order to build any castle, you must have the stone and wood required for its construction. Click on the thumbs up gauntlet to begin the project. No work will begin until you have all the needed materials. As soon as you have them, the computer will allocate some labor to construct your castle.",
            "To speed up the construction of a castle, assign more workers to the job.",
            "You may follow the same process to upgrade an existing castle.",
        ],
    ),
    (
        294,
        &[
            "How do I make weapons?",
            "To make weapons, you need an operational blacksmith, some laborers working there, and the required materials for the weapon you want to make.",
            "To activate a blacksmith, click on it. A weapon icon will appear on your control panel showing the weapon currently being produced. Click on this icon to access the blacksmith shop, where various weapons hang on the walls. To change the weapon being produced, select a hanging weapon.",
            "The panel will show how much wood and iron is needed to produce one unit of the selected weapon. All weapons require some wood, and all but the longbow also require iron.",
            "To get iron or wood, buy them from a merchant or produce them yourself. (Iron production requires an iron mine, and wood production requires a lumber mill.)",
            "To increase production of weapons, allocate more laborers to the blacksmith shop. Make sure you have plenty of materials !.",
        ],
    ),
    (
        295,
        &[
            "What should I do each turn?",
            "There is much to be done each turn, depending on your particular strategy. Here are some of your options:",
            "\u{b7} Adjust your labor allocation to maximize food production, industrial output, and castle construction.",
            "\u{b7} Check for overcrowding on your cattle fields, and establish more cattle fields (or get rid of cows) if you have overcrowding.",
            "\u{b7} Visit a merchant, if one is present, and buy cows, grain, weapons, ale, wood, iron, or stone if you need it. Sell extra materials, if you have them, to make some money.",
            "\u{b7} Adjust your tax rate if it needs adjusting.",
            "\u{b7} Create an army if one is needed. Move existing armies, and fight battles.",
            "\u{b7} Send messages to your opponents.",
            "\u{b7} Turn industries on and off.",
            "\u{b7} Adjust rations.",
            "\u{b7} Transport grain or cows between counties.",
        ],
    ),
];
