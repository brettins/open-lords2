use super::*;

impl MapScreen {
    /// The county at a canvas pixel, or 0.
    pub fn county_at(&self, x: i32, y: i32) -> u8 {
        if !self.map_clip().contains(x, y) {
            return 0;
        }
        self.tags.at(x, y)
    }

    /// `Map_ToggleZoom`: the campaign screen has two zooms and this is the only
    /// way between them.
    pub(super) fn toggle_zoom(&mut self, ctx: &mut Ctx) {
        if self.zoom.id == NEAR.id {
            self.saved = self.view;
            self.set_zoom(ctx, FAR);
            self.view = Viewport::new(0x0C, 0x0E).clamped(&FAR);
            self.status = "ZOOMED OUT".into();
        } else {
            self.set_zoom(ctx, NEAR);
            self.view = self.saved.clamped(&NEAR);
            self.status = "ZOOMED IN".into();
        }
    }

    /// **`Map_SetZoom` — the only place `self.zoom` is written after
    /// construction,
    /// rides on it.**
    ///
    /// `g_mapZoom` is a global in the original and is read from three arms that
    /// are not on this screen: `Map_EdgeScroll`'s far-zoom refusal, which is
    /// what decides whether the information panel closes on an edge hover, and
    /// the `if (g_mapZoom != 2)` at the head of `FUN_00438ACC` and
    /// `FUN_0043893C`. Our overlays cannot reach this screen, so the value has
    /// to be somewhere they can see, and a projection written at the one write
    /// site cannot drift from the thing it projects.
    pub(super) fn set_zoom(&mut self, ctx: &mut Ctx, zoom: Zoom) {
        self.zoom = zoom;
        ctx.game.map_zoom_far = zoom.id == FAR.id;
    }

    /// **`Map_EdgeScroll` — the direction the pointer's position asks for.**
    ///
    /// The original reads the *desktop* cursor (`GetCursorPos` into
    /// `0x004E6594`/`0x004E6598`) and scrolls while it sits on the outermost
    /// pixel of a 640 × 480 screen. It ran full-screen, so "the edge of the
    /// screen" and "the edge of the window" were the same place.
    ///
    /// They are not the same place for us, and that is the whole of the bug a
    /// player reported as *"I can't scroll the map"*. The window is scaled by
    /// an integer factor
    /// of 640 × 480 has black borders — and a cursor pushed into the border is
    /// outside the picture. `main.rs` clamps every pointer position into the
    /// canvas ([`pixels::Pixels::clamp_pixel_pos`]),
    /// the border arrives here **as the edge pixel it is nearest**.
    /// gesture works at the edge of the *window* whatever the letterboxing is
    /// doing. Nothing here needs to know the window exists.
    ///
    /// `None` when the pointer is not at an edge, or has left the window.
    pub(super) fn edge_direction(&self) -> Option<Dir> {
        if !self.pointer_in {
            return None;
        }
        let (x, y) = self.pointer;
        let west = x <= 0;
        let east = x >= CANVAS_W - 1;
        let north = y <= 0;
        let south = y >= CANVAS_H - 1;
        match (north, east, south, west) {
            (true, false, false, false) => Some(Dir::N),
            (true, true, false, false) => Some(Dir::NE),
            (false, true, false, false) => Some(Dir::E),
            (false, true, true, false) => Some(Dir::SE),
            (false, false, true, false) => Some(Dir::S),
            (false, false, true, true) => Some(Dir::SW),
            (false, false, false, true) => Some(Dir::W),
            (true, false, false, true) => Some(Dir::NW),
            _ => None,
        }
    }

    pub(crate) fn scroll(&mut self, dir: Dir) -> bool {
        match self.view.scrolled(dir, &self.zoom) {
            Some(v) => {
                self.view = v;
                self.opened = true;
                true
            }
            None => false,
        }
    }

    /// `Map_CentreOnTile`. Reached from a minimap click, from a click on a
    /// county town, and from `Field_SetType`'s caller — the original centres
    /// the map before it opens anything over it (`docs/decisions.md` C22).
    pub fn centre_on_tile(&mut self, x: usize, y: usize) {
        self.view = Viewport::centred_on_tile(x, y, &self.zoom);
        // Somebody has said where to look, so the opening centre
        // ([`MapScreen::open_on_the_player`]) must not override it on the first
        // paint. It is a *default* for a screen nobody has positioned, not a
        // thing that happens to every campaign screen once.
        self.opened = true;
    }

    pub(super) fn centre_on_county(&mut self, anchor: (usize, usize)) {
        self.centre_on_tile(anchor.0, anchor.1);
    }

    /// **Ours.** Select the player's next unit in ascending slot order and
    /// centre the map on it, so an army the viewport is nowhere near can still
/// be given an order. Ascending slot order.
    /// because a selection that depends on where the viewport happens to be is
    /// a selection two peers could disagree about.
    pub(super) fn cycle_unit(&mut self, ctx: &mut Ctx) {
        let mine = ctx.game.player_units();
        if mine.is_empty() {
            self.cancel_move_selection();
            self.status = "YOU HAVE NOTHING ON THE MAP".into();
            return;
        }
        let next = match self.selected_unit {
            Some(cur) => mine.iter().copied().find(|&id| id > cur).unwrap_or(mine[0]),
            None => mine[0],
        };
        self.begin_move_selection(ctx, next);
        if let Some(u) = ctx.game.kingdom.campaign.units.get(next) {
            let (x, y, men, left, kind) = (u.x, u.y, u.men, u.moves_left(), u.kind);
            self.centre_on_tile(x as usize, y as usize);
            self.status = format!("{} #{next}: {men} MEN, {left} MOVES", kind.name().to_uppercase());
        }
    }

    /// **End the turn — or get as far as the first battle**, and leave for
    /// screen `0x1C` if the turn ended the game.
    ///
    /// `FUN_00476768` is the original's shape for the ending: dismissing the
    /// message that set `DAT_0053F0C4` calls `FUN_00497879` — which advances the
/// campaign counter on a win — and sets `g_screenId = 0x1C`.
    /// message window here yet, so the turn's own end is the dismissal.
    ///
    /// [`turn::begin_turn`] is the interactive door and may come back holding a
    /// question, which is `Push`ed as screen `0x12`. The turn stays suspended on
    /// the [`Game`](crate::game::Game) until the prompt
    /// hand it back; [`Self::resume_turn`] is what picks it up again when they
    /// pop, and it is called from `update` because this screen is underneath
    /// them and gets its tick back the moment they are gone.
    pub(crate) fn end_turn(&mut self, ctx: &mut Ctx) -> Transition {
        if turn::turn_in_flight(ctx.game) || self.fading.is_some() {
            return Transition::Stay;
        }
        // **The gold is snapshotted here and not in `resume_turn`.** A turn
        // takes many frames now, and `resume_turn` runs on all of them; reading
        // the treasury there would compare the end of the turn against the
        // frame before it and report a change of nothing.
        self.gold_at_turn_start = ctx.game.gold();
        // The slider's drag has nowhere to land while the map is not taking
        // clicks. The unit *selection* is kept: an order placed and then
        // silently cancelled by the turn ending would be a surprise.
        self.slider_held = false;
        let step = turn::begin_turn(ctx.game);
        self.status = "ENDING THE TURN...".into();
        self.scrolled = true;
        let before = self.gold_at_turn_start;
        self.settle_turn(ctx, step, before)
    }

    /// Pick a suspended turn back up **and wind it on by one tick**. Called
/// every tick; almost always a no-op.
    /// turn in flight.
    ///
    /// This used to call [`turn::dismiss_report`], which ran the machine to the
    /// end unless a battle stopped it —
    /// before the next frame was drawn. [`turn::tick_turn`] is one
    /// `Turn_Tick` / `Units_Tick` pair, which is what the original's main loop
    /// calls once a frame, and it is what gives a turn its duration. See
    /// [`turn::TurnStep::Running`].
    pub(super) fn resume_turn(&mut self, ctx: &mut Ctx) -> Transition {
        if !turn::turn_in_flight(ctx.game) {
            return Transition::Stay;
        }
        // A turn that is still asking is one whose screen has not been put up
        // yet, or has just been popped without answering; either way the prompt
        // is what has to come back.
        if turn::pending_question(ctx.game).is_some() {
            return Transition::Push(ScreenId::BattlePrompt);
        }
        if turn::pending_report(ctx.game).is_some() {
            return Transition::Push(ScreenId::BattleResult);
        }
        // **A tick of the turn is a repaint**, because the whole point of
        // spreading it over frames is that the units on it are seen to move.
        self.scrolled = true;
        let step = turn::tick_turn(ctx.game);
        let before = self.gold_at_turn_start;
        self.settle_turn(ctx, step, before)
    }

    /// What to do with whatever the turn machine came back with.
    pub(super) fn settle_turn(&mut self, ctx: &mut Ctx, step: turn::TurnStep, before: i32) -> Transition {
        match step {
            turn::TurnStep::Ask(_) => return Transition::Push(ScreenId::BattlePrompt),
            turn::TurnStep::Report(_) => return Transition::Push(ScreenId::BattleResult),
            // One tick down. The next frame brings the next one.
            turn::TurnStep::Running => return Transition::Stay,
            turn::TurnStep::Stuck => {
                self.status = "THE TURN MACHINE DID NOT COME ROUND".into();
                return Transition::Stay;
            }
            turn::TurnStep::Done(outcome) => self.finish_turn(ctx, *outcome, before),
        }
    }

    pub(super) fn finish_turn(
        &mut self,
        ctx: &mut Ctx,
        outcome: turn::TurnOutcome,
        before: i32,
    ) -> Transition {
        let change = ctx.game.gold() - before;
        let fought = outcome.battles.len();
        let status = format!(
            "{} {} {} - {} MSG{}",
            season_name(ctx.game.kingdom.season),
            ctx.game.kingdom.year,
            widget::signed(change),
            outcome.report.messages.len(),
            if fought == 0 { String::new() } else { format!(" - {fought} BATTLE") }
        );
        let over = outcome.outcome.is_over();
        if over {
            ctx.game.campaign.enter_conquest_screen();
        }
        // **`Turn_Tick`'s phase 7 fades the screen the instant `Season_Advance`
        // returns**, and this is that instant. `FUN_004B0CB4(_, 1, _)` down to
        // a quarter, the seasonal art reloaded in the dark, `FUN_0049A3E6`'s
        // `FUN_004B0CB4(_, 0, _)` back up. See [`l2_view::fade`] and
        // [`Fading`].
        //
        // The status line
        // light: the original's `g_screenId = 0x1C` is on the far side of the
        // fade too.
        self.fading = Some(Fading { phase: 0, status, over });
        self.scrolled = true;
        Transition::Stay
    }

    /// One frame of the end-of-turn fade. See [`Fading`].
    pub(super) fn tick_fade(&mut self) -> Transition {
        let Some(mut f) = self.fading.take() else { return Transition::Stay };
        self.scrolled = true;
        f.phase += 1;
        // **`FUN_0049A3E6` — the rolling autosave, and this frame is where the
        // original writes it.** `[V]`:
        //
        // ```c
        // void FUN_0049a3e6(void) {
        //   if (g_screenId == '$') {                      /* 0x24, the bottom  */
        //     DAT_0057c968 = 1; Gfx_LoadCountyMode();     /* the season's art  */
        //     Screen_DrawCampaign(3); FUN_004b14ff(); Gfx_Present(1);
        //     DAT_0056d6a0 = 1; Gfx_MarkAllDirty();
        //     FUN_004b0cb4(0,0,0x5691f0);                 /* and back up       */
        //     Save_RotateAndWrite();
        //   }
        // }
        // ```
        //
        // So it is neither the button nor the far side of the light: it is the
        // one frame the screen is dark
        // what it stores is therefore the **opening of the turn that just
        // began**. [`l2_view::fade::is_darkest`] already names this frame for
        // the art reload, which is the statement two lines above it.
        //
        // Raised, not performed — see [`crate::screen::Screen::take_autosave`].
        if l2_view::fade::is_darkest(f.phase) {
            self.autosave = true;
        }
        if f.phase < l2_view::fade::PHASES {
            self.fading = Some(f);
            return Transition::Stay;
        }
        self.status = f.status;
        if f.over {
            // `Replace`, not `Push`: the campaign map underneath is a map of a
            // game that is over.
            // `0x1C` goes on to `Game_NewGame` or the front end, never back to
            // it.
            return Transition::Replace(ScreenId::Conquest);
        }
        Transition::Stay
    }

    /// Whether the base render is being held back until the fade bottoms out.
    ///
    /// **The seasonal art changes in the dark, and that is the job the fade is
    /// doing.** The original's second `FUN_004B0CB4` call site is
    /// `FUN_0049A3E6`, *after* reloading the seasonal art — so the reload
    /// happens between the two calls, inside the dark window. A player who has
    /// played it says the same thing from the other side: the fade *"hides the
    /// season change visuals just abruptly changing"*. Two unrelated sources
    /// meeting is what turns the note in [`l2_view::fade`] from inferred into
    /// confirmed.
    ///
    /// So a rebuild that falls due while the light is going down waits for the
    /// bottom. Without this the map pops to the new season one frame after the
    /// button and then fades a picture the player has already watched change,
    /// which is the abruptness the effect exists to hide.
    pub(super) fn holding_art_for_the_dark(&self) -> bool {
        matches!(self.fading, Some(Fading { phase, .. }) if phase < l2_view::fade::STEPS)
    }
}
