#![allow(unused_imports)]
use super::*;
use super::types::*;
use super::machine_struct::*;
use machine::*;
use l2_view::Canvas;
use crate::game::{Assets, Game};
use crate::input::Event;

pub trait Screen {
    fn id(&self) -> ScreenId;

    /// What the window is called while this screen is on top.
    fn title(&self, ctx: &Ctx) -> String;

    /// One input event. The default ignores everything,
/// down what it responds to.
    fn handle(&mut self, _event: Event, _ctx: &mut Ctx) -> Transition {
        Transition::Stay
    }

    /// One fixed simulation tick.
    ///
    /// **Not one frame.** Nothing here is told how much time passed, because
    /// nothing below this crate may learn anything from a clock. The renderer
    /// draws when it can; this steps at a fixed rate and never asks what the
    /// rate was.
    fn update(&mut self, _ctx: &mut Ctx) -> Transition {
        Transition::Stay
    }

/// **`Turn_Tick(); Units_Tick();` — the half of a frame
    /// screen's.**
    ///
    /// `Battle_Frame` (`0x004B99C0`) ends its inner loop with
    ///
    /// ```c
    /// if ((g_battlePhase == 0) && (ticksDue != 0)) { FUN_0040490d(); Turn_Tick(); Units_Tick(); }
    /// ```
    ///
    /// and — `[V]`, read whole. Only
    /// `Screen_Draw` and `Screen_FrameInput`, in the tail below it, dispatch on
    /// the screen. So the campaign winds on under a county panel, a village, an
    /// open menu and the message scroll alike, and the one thing that stops it
    /// is a battle.
    ///
    /// [`Screen::update`] is `Screen_FrameInput`'s half and this is the loop's,
/// so the campaign map has both: [`Machine::wind_turn`] calls this
    /// one every frame whatever is on top.
    fn wind_turn(&mut self, _ctx: &mut Ctx) -> Transition {
        Transition::Stay
    }

    /// Whether the last [`Screen::update`] changed what is on screen.
    ///
    /// Input already forces a repaint — [`Machine::handle`] marks the machine
    /// dirty for every event — so this exists for the one thing that changes
    /// without an event arriving: **edge scrolling**, where the pointer is held
    /// still against the edge of the window and the map moves under it. Taking
/// the flag keeps a still screen costing nothing,
    /// which is the property [`Machine::update`] was written to preserve.
    fn take_redraw(&mut self) -> bool {
        false
    }

    /// **How many widget clicks this screen owes the audio layer**, taken and
    /// forgotten — `Widget_Test`'s (`0x0040DA1E`) `Sound_RestartSlot(1)`.
    ///
    /// The original plays `click3.wav` *inside* the hit test, so the sound is
    /// not a decision any caller makes: pressing a kind-4 or kind-5 widget
    /// sounds, and nothing else in the interface does — not a hotspot, not the
    /// OK button, not the auto-repeat's later pulses. Ours hit-test through
    /// [`crate::press::Press`], which is the same function in the same place,
    /// and this is the one wire out of it.
    ///
    /// **It goes up, never down.** `docs/netcode.md` D-3 keeps
    /// [`crate::audio::Audio`] out of [`Ctx`] so that a screen cannot branch on
    /// a sound; a screen that can only *report* a press it has already acted on
    /// keeps that property exactly. Nothing here is on [`crate::Game`], so it
    /// is not in the save and not in the lockstep digest.
    ///
    /// A screen with no [`crate::press::Press`] answers zero, which is not an
    /// approximation: the original's other tester plays no sound.
    fn take_clicks(&mut self) -> u8 {
        0
    }

    /// **Whether this screen has just reached a `Save_RotateAndWrite`
    /// (`0x0049A453`)**, taken and forgotten.
    ///
    /// The original calls it from exactly two places, `[V]` — both of them
    /// screens here:
    ///
    /// * `FUN_0049A3E6`, which fires on `g_screenId == 0x24` and is the bottom
    ///   of the end-of-turn fade: reload the seasonal art, fade back up,
    ///   autosave. [`crate::screens::map::MapScreen`] raises it there.
    /// * `Game_NewGame` (`0x00497CED`), after `Season_Advance` and
    ///   `Move_BuildCostMap`. [`crate::screens::setup::SetupScreen`] raises it
/// there,
    ///
/// **It goes up**, as [`Screen::take_clicks`] does and
    /// for the same reason: a screen may report that a turn came round, and may
    /// not learn whether a file was written or where it went. Nothing here is on
    /// [`Game`], so it is not in the save and not in the lockstep digest.
    /// [`crate::saves::run_pending`] is the one place it becomes a file.
    fn take_autosave(&mut self) -> bool {
        false
    }

    /// **The original's `g_screenId`
    /// [`Screen::id`].**
    ///
    /// Almost every screen's byte follows from its [`ScreenId`], and
    /// [`crate::tip::screen_byte`] writes those down. One does not: the
    /// campaign map is `0x00`, and *in move-order mode* it is `0x10` —
    /// `Map_BeginMoveSelection` writes it and is the only writer of that value —
    /// while ours keeps the mode inside `MapScreen`. `Tip_Update`'s *"Army
    /// Movement:"* arm tests exactly that byte, so the mode has to be askable.
    fn mode_screen_id(&self) -> Option<u8> {
        None
    }

    /// **`g_minimapMode` (`0x0057A0C4`)**, from the one screen that keeps it.
    ///
    /// A global in the original and a field of the campaign map here, and the
    /// tool-tip ladder (`FUN_00477320`) reads it on every screen that sits on
    /// the sidebar — so the machine asks the stack. See [`crate::tooltip`].
    fn minimap_mode(&self) -> Option<u8> {
        None
    }

    /// The `.256` this screen runs under, if it is not the campaign palette.
    ///
    /// A [`Canvas`] is a plane of palette *indices* and means nothing without
    /// one. Most screens use the campaign palette and answer `None`; the front
    /// end, the merchant, the armoury, castle building and the ratings each
    /// read a palette of their own (`File_ReadChunk("gateway.256", …)` then
/// `Palette_Set`), and the presenter asks the top screen
    /// assuming there is only one.
    fn palette(&self) -> Option<&'static str> {
        None
    }

/// **A palette** — a film's, which changes as it plays.
    ///
    /// `Smk_PlayLoop` copies the film's 768 bytes into `g_paletteRgb` and
    /// uploads them whenever a frame carries a palette, so while a film is up
    /// *the whole screen* runs under it, the window it was raised over
    /// included. When this answers `Some`, the presenter uses it in place of
    /// [`Screen::palette`].
    fn live_palette(&self) -> Option<l2_formats::Palette> {
        None
    }

    /// How far into the end-of-turn screen fade this screen is, or `None` for
    /// the ordinary full-brightness palette.
    ///
    /// **The canvas is not involved.** `FUN_004B0CB4` is entirely a palette
    /// effect — no dither table, no half-brightness blit —
    /// fading draws exactly what it always draws and answers this instead. The
    /// presenter turns the number into colour, which is the same division
    /// [`Screen::palette`] already makes and for the same reason: a [`Canvas`]
    /// is a plane of indices and only one place in the application knows what
    /// they mean.
    ///
    /// The value is a phase in `0 ..= l2_view::fade::PHASES`.
    fn fade(&self) -> Option<u8> {
        None
    }

/// Whether this screen is an **inset over what was underneath**
    /// a page of its own.
    ///
    /// `docs/screens-county.md` §1: *"The game's management surface is a
    /// campaign map plus insets, not a set of full-screen pages."* An overlay
    /// does not clear the canvas, and [`Machine::draw`] paints the screens
    /// beneath it first, back to the last one that is not an overlay.
    ///
    /// It changes nothing about input: only the top screen is ever offered an
    /// event, which is what makes a popup modal.
    ///
    /// **There are two kinds of inset and both answer true**, which matters
    /// because only one of them looks like a window:
    ///
    /// * a framed `Ui_DrawBox` window — the four county panels, the job popup;
    /// * a raw blit with no frame and no clear — **the village**, which is
    ///   `vill.pl8` frame 0, 363 × 320, dropped at (64, `g_villageTopY`) over a
    ///   campaign map it repaints itself (`FUN_004050C0` → `FUN_004CFB08` →
    ///   `Map_DrawFrame`).
    ///
    /// The village was modelled as a page until a player opened one and said it
    /// was a dialogue with the map still showing round it. He was right;
    /// `docs/decisions.md` C22 records why the decompiled reasoning was not.
    fn is_overlay(&self) -> bool {
        false
    }

    pub(crate) fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas);
}

