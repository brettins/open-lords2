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

    fn title(&self, ctx: &Ctx) -> String;

    fn handle(&mut self, _event: Event, _ctx: &mut Ctx) -> Transition {
        Transition::Stay
    }

    fn update(&mut self, _ctx: &mut Ctx) -> Transition {
        Transition::Stay
    }

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
    fn wind_turn(&mut self, _ctx: &mut Ctx) -> Transition {
        Transition::Stay
    }

    fn take_redraw(&mut self) -> bool {
        false
    }

    /// **How many widget clicks this screen owes the audio layer**, taken and
    /// forgotten — `Widget_Test`'s (`0x0040DA1E`) `Sound_RestartSlot(1)`.
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
    ///
    /// * `Game_NewGame` (`0x00497CED`), after `Season_Advance` and
    ///   `Move_BuildCostMap`. [`crate::screens::setup::SetupScreen`] raises it
/// there,
    fn take_autosave(&mut self) -> bool {
        false
    }

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

    fn palette(&self) -> Option<&'static str> {
        None
    }

    fn live_palette(&self) -> Option<l2_formats::Palette> {
        None
    }

    /// **The canvas is not involved.** `FUN_004B0CB4` is entirely a palette
    /// effect — no dither table, no half-brightness blit —
    /// fading draws exactly what it always draws and answers this instead. The
    /// presenter turns the number into colour, which is the same division
    /// [`Screen::palette`] already makes and for the same reason: a [`Canvas`]
    /// is a plane of indices and only one place in the application knows what
    /// they mean.
    fn fade(&self) -> Option<u8> {
        None
    }

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

    fn draw(&mut self, ctx: &Ctx, canvas: &mut Canvas);
}

