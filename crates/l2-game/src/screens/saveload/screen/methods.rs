#![allow(unused_imports)]
use super::*;
use super::screen_impl::*;
use super::*;
use super::helpers::*;
use super::tests::*;
use l2_view::{text, Canvas};
use crate::input::{Event, Key, Rect};
use crate::press::{Press, Widget};
use crate::saves::{self, Entry};
use crate::screen::{Ctx, Screen, ScreenId, Transition};
use crate::shell::{self, font, Pen};

impl SaveLoadScreen {
    /// Reads the save directory once, on open. A screen that re-listed on every
    /// frame would be a screen that hits the disk sixty times a second.
    pub fn new(mode: Mode) -> SaveLoadScreen {
        let entries = saves::list();
        let mut screen = SaveLoadScreen {
            mode,
            entries,
            top: 0,
            selected: None,
            name: begin_name(""),
            status: Status::Idle,
            press: Press::new(),
            working: 0,
        };
        // Loading opens on the first file, because loading *is* choosing one.
        // Saving opens on none, because saving is naming one, and a preselected
        // row would mean the confirm button overwrote a game the player never
        // pointed at.
        if mode == Mode::Load {
            screen.select(0);
        }
        screen
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn status(&self) -> &Status {
        &self.status
    }

    /// What the name field holds — the typed name in save mode, the selected
    /// file's in load mode.
    pub fn name(&self) -> String {
        self.name.text()
    }

    /// The field itself, for a test that wants to look at the caret.
    pub fn name_field(&self) -> &crate::text::TextField {
        &self.name
    }

    /// Where row `i` of the visible page is drawn. The painter fills columns
/// **across** and then steps down, so this is `i % COLS` for the
    /// column and `i / COLS` for the row and not the other way round.
    pub fn row_rect(i: usize) -> Rect {
        let (col, row) = (i % COLS, i / COLS);
        let x = LIST.0 + col as i32 * COL_W;
        // The third column is **narrower than the other two**, and that is the
// painter's geometry: it steps x by 120
        // three times inside an interior that is only 336 wide, so the columns
        // start at 48, 168 and 288 and the box ends at 382. A hit box of a
        // uniform 120 would put the third column's right-hand 18 pixels
        // outside the list it belongs to.
        let right = (INTERIOR.0 + INTERIOR.2).min(x + COL_W - 8);
        Rect::new(x, LIST.1 + row as i32 * ROW_H, right - x, ROW_H)
    }

    /// The highest `top` that still shows a full page, in steps of three.
    pub(crate) fn max_top(&self) -> usize {
        let over = self.entries.len().saturating_sub(PAGE);
        // Round up to a whole scroll step so that the last press lands on a
// reachable value.
        over.div_ceil(SCROLL_STEP) * SCROLL_STEP
    }

    pub(crate) fn scroll(&mut self, by: i32) {
        let max = self.max_top() as i32;
        self.top = (self.top as i32 + by).clamp(0, max) as usize;
    }

    /// Which entry a click landed on, if any.
    pub(crate) fn at(&self, x: i32, y: i32) -> Option<usize> {
        (0..PAGE)
            .find(|&i| Self::row_rect(i).contains(x, y))
            .map(|i| self.top + i)
            .filter(|&i| i < self.entries.len())
    }

    /// Highlight a row and put its name in the field. In save mode that is how
    /// an existing save is overwritten — you pick it, and the name it had is
    /// what the confirm button will write to.
    pub(super) fn select(&mut self, i: usize) {
        let Some(entry) = self.entries.get(i) else { return };
        self.selected = Some(i);
        self.name = begin_name(&entry.name);
    }

    /// **`DAT_005CD41C = 100`**, which is the whole of the thumb up's handler
    /// and of Enter's: arm the latch, and let [`WORK_FRAMES`] run.
    ///
/// **And the line the box speaks**, which is `SaveLoad_Tick`'s.
    /// the handler's: the tick that takes the latch up plays `S040_02.wav` on
    /// `g_screenId == '6'` — the save box — and `S040_01.wav` on anything else,
    /// which here is the load box. `[V]`, two `if`s and not an `if`/`else`.
    /// Ours collapses the arm and the take-up into this one call,
    /// is a frame earlier than the original's and on the same occasion.
    ///
    /// A screen cannot reach the audio layer (`docs/netcode.md` D-3), so the
    /// decision is made here and reported on [`crate::game::Game::spoken`].
    // sfx: FUN_004ad9f0#1,FUN_004ad9f0#2
    pub(super) fn begin(&mut self, ctx: &mut Ctx) {
        self.working = WORK_FRAMES;
        self.status = Status::Working;
        let line = match self.mode {
            Mode::Save => crate::audio::names::speech::SAVE_GAME,
            Mode::Load => crate::audio::names::speech::LOAD_GAME,
        };
        ctx.game.spoken = (ctx.game.spoken.0.wrapping_add(1), line);
    }

    /// Frames left before the load or the save runs; 0 when nothing is armed.
    pub fn working(&self) -> u8 {
        self.working
    }

    /// One `g_saveLoadWidgets` record's handler. The index is [`widgets`]'.
    pub(crate) fn fire(&mut self, ctx: &mut Ctx, widget: usize) -> Transition {
        match widget {
            // `FUN_004342F3`.
            0 => {
                self.begin(ctx);
                Transition::Stay
            }
            // `SaveLoad_Cancel` (`0x00434308`): `g_screenId = g_screenIdSaved`.
            1 => Transition::Pop,
            // `SaveLoad_Scroll`, hotspot id −3 and +3.
            2 => {
                self.scroll(-(SCROLL_STEP as i32));
                Transition::Stay
            }
            _ => {
                self.scroll(SCROLL_STEP as i32);
                Transition::Stay
            }
        }
    }

    /// The load or the save itself — what `SaveLoad_Tick` does when
    /// `DAT_0057D3C4` reaches zero. Everything that can go wrong comes back as
    /// a [`Status`] and the screen stays open; only success closes it.
    pub(super) fn confirm(&mut self, ctx: &mut Ctx) -> Transition {
        match self.mode {
            Mode::Load => {
                // **The path comes from the edit buffer, not from the
                // highlighted row.** `SaveLoad_Tick` (`0x004AD9F0`) is
                // `Str_Copy(0x4EA130, 0x4EAD60, 0xC); Path_AddExtension(…)` —
                // twelve bytes of the *typed* name — and clicking a row is what
                // puts a name into the buffer. So one road, and the mouse joins
                // it upstream. Ours read `selected` and ignored what was typed.
                let name = self.name.text().trim().to_string();
                let Some(i) = self.entries.iter().position(|e| e.name == name) else {
                    self.status = Status::Failed(if name.is_empty() {
                        "NO SAVED GAME IS SELECTED".into()
                    } else {
                        format!("{name:?} IS NOT A SAVED GAME")
                    });
                    return Transition::Stay;
                };
                let tables = ctx.game.kingdom.tables;
                match saves::read_path(&self.entries[i].path, tables) {
                    // **The whole game is replaced or none of it is.** `decode`
                    // builds a complete `Game` before this line runs,
                    // that turns out to be unreadable halfway through cannot
                    // leave the player holding half of one.
                    Ok(game) => {
                        // Nor does a load clear `g_tipShown`. `crate::tip`.
                        let tips = ctx.game.tips;
                        *ctx.game = game;
                        ctx.game.tips = tips;
                        Transition::Pop
                    }
                    Err(e) => {
                        self.status = Status::Failed(e.to_string().to_uppercase());
                        Transition::Stay
                    }
                }
            }
            Mode::Save => {
                // **OURS, and it is a refusal the original does not make.**
                //
                // `Menu_SaveGame` (`0x00433F49`) is reachable from the
                // battlefield — `Screen_FrameInput`'s `0x29` arm opens with
                // `Menu_OpenDropdown` and neither the opener nor the handler
                // tests `g_battlePhase` — and the original's `.sav` is a memory
                // dump, so the battle goes into it. Ours is a versioned format
                // that does not encode `crate::battlefield::LiveBattle`, and
                // `crate::save::decode` puts `battle: None` back.
                //
                // So a mid-battle save would write a file that quietly lost the
                // battle the player was fighting. It is refused instead, through
                // `Status::Failed`, whose first line is the game's **own**
                // sentence — `Eng_DrawString(40, ERROR_INDEX)` —
// sees a refusal.
                //
                // **Measured, and that is why it is still a refusal.** The
                // state a mid-battle save would have to carry is
                // `LiveBattle` -> `BattleRunner` -> `Battle`, `Battlefield`,
                // `Vec<Fighter>`, `Units`, `Ai`, `AiField`, `Missiles`,
                // `SiegeState`: **196 fields over 16 structs, 21 of them
                // private to `l2-sim`**. The kingdom encoder next door spends
                // 1975 lines on 242 stored fields, so this is four figures of
                // encoder, not the ~150 lines the question was worth.
                //
                // arm: ours/save-refuses-mid-battle left-press
                if ctx.game.battle.is_some() {
                    self.status = Status::Failed(BATTLE_REFUSAL.into());
                    return Transition::Stay;
                }
                let name = self.name.text().trim().to_string();
                if !saves::is_valid_name(&name) {
                    self.status = Status::Failed(format!("{name:?} IS NOT A SAVE NAME"));
                    return Transition::Stay;
                }
                match saves::write(&name, ctx.game) {
                    // The box closes, the way `Menu_SaveGame`'s does: it saved
                    // `g_screenId` into `g_screenIdSaved` on the way in and the
                    // screen puts it back on the way out, which is what a `Pop`
                    // over whatever pushed this is.
                    Ok(_) => Transition::Pop,
                    Err(e) => {
                        self.status = Status::Failed(e.to_string().to_uppercase());
                        Transition::Stay
                    }
                }
            }
        }
    }

    /// **One event into the name field**, and it is live on **both** screens.
    ///
    /// `Screen_HandleInput`'s arm is `else if (g_screenId == '5' || g_screenId
    /// == '6')` — one arm for `0x35` and `0x36` together — so the original lets
    /// a person **type the name of the game they want to load**, and
    /// `SaveLoad_Tick` builds the path out of the edit buffer either way rather
    /// than out of the highlighted row. Ours refused every keystroke unless
    /// `mode == Save`, which was a restriction we invented; clicking a row
    /// still fills the field, so the mouse route is unchanged.
    pub(super) fn edit(&mut self, event: Event, ctx: &Ctx) -> bool {
        // arm: 0x004BA9C8/saveload-name key
        let m = crate::text::FontMetrics::of(&ctx.assets.shell);
        if !self.name.event(event, &m) {
            return false;
        }
        self.status = Status::Idle;
        true
    }
}

