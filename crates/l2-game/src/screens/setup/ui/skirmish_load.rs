#![allow(unused_imports)]
use super::*;
use l2_formats::battles::BattleText;
use l2_formats::skr::{Side, MAP_COUNT};

/// What `Skirmish_Setup` (`0x0042B7F7`, `00420000.c:4557-4558`) reads when
/// page 12 opens — `FUN_0042AA90` then `Troops_Load` — and what
/// `FUN_0042D4AA` reads when page 13 picks a name.
impl SetupScreen {
    /// **`Troops_Load` (`0x0042AC0C`)**, called on the way in and again by
    /// `FUN_0043DD83` (`00430000.c:8126`) after the sides swap, because which
    /// side the local player is on picks the file.
    ///
    /// The `.skr` rows 35…54 are re-laid afterwards, since `Troops_Load`
    /// clears all 35 of its own rows and this table holds 55.
    pub(crate) fn load_troops(&mut self, ctx: &Ctx) {
        let attacks = self.skirmish.local_defends;
        // `g_multiplayer == 0` — page 12 is the single-player skirmish; page
        // 11 is the multiplayer one and takes `TROOPS.ENG`.
        let multiplayer = self.page == SetupPage::SkirmishMulti;
        self.troops = match ctx.assets.shell.troops(multiplayer, attacks) {
            Some(t) => skirmish::TroopsTable::from_file(t),
            None => Default::default(),
        };
        self.troops_file = Some(l2_formats::troops::file_for(multiplayer, attacks));
        if let Some(name) = self.skirmish.file.clone() {
            self.load_skr(ctx, &name);
        }
    }

    /// **`FUN_0042D4AA` (`0x0042D4AA`, `00420000.c:5257-5287`)** — the army
    /// table at file offset 0 and the text table at `0x6E0`, then twenty rows
    /// written at `(map + 0x23) * 0xDC`.
    ///
    /// The five difficulty columns are the file's counts at 5/3, 4/3, 1, 2/3
    /// and 1/3, each term divided on its own: `a + (a * 2) / 3`, not
    /// `a * 5 / 3`.
    pub(crate) fn load_skr(&mut self, ctx: &Ctx, name: &str) {
        let Some(skr) = ctx.assets.shell.skr(name) else { return };
        self.skr_text = (0..MAP_COUNT).filter_map(|m| skr.text(m).ok()).collect();
        let base = skirmish::ROW_BASE[3];
        if self.troops.rows.len() < base + MAP_COUNT {
            self.troops.rows.resize(base + MAP_COUNT, Default::default());
        }
        for map in 0..MAP_COUNT {
            let sides = [skr.army(map, Side::Attacker), skr.army(map, Side::Defender)];
            for (side, army) in sides.iter().enumerate() {
                let Ok(army) = army else { continue };
                for slot in 0..11 {
                    let a = army.counts[slot] as i32;
                    let row = &mut self.troops.rows[base + map][side];
                    row[0][slot] = (a + (a * 2) / 3) as i16;
                    row[1][slot] = (a + a / 3) as i16;
                    row[2][slot] = a as i16;
                    row[3][slot] = ((a * 2) / 3) as i16;
                    row[4][slot] = (a / 3) as i16;
                }
            }
        }
    }

    /// The three strings of the lit battle — `FUN_0042AB15` on
    /// `(DAT_0053E91C * 10 + row) * 3` for categories 0…2, and `FUN_0042AB82`
    /// (`0x0042AB82`) on the `.skr` text table for category 3, where the same
    /// triple is one 0xB7-byte record at `+0`, `+0x0D` and `+0x2A`.
    pub fn battle_text(&self, ctx: &Ctx, row: usize) -> BattleText {
        if self.skirmish.kind == 3 {
            return match self.skr_text.get(row) {
                Some(t) => BattleText {
                    name: t.name.clone(),
                    title: t.title.clone(),
                    description: t.description.clone(),
                },
                None => BattleText::default(),
            };
        }
        ctx.assets.shell.battles.at(self.skirmish.kind, row)
    }

    /// **`FUN_0042B9C4`'s two non-castle arms** (`00420000.c:4608-4619`):
    /// category 3 raises `Battlefield_BuildFromSkr` (`0x0047B8B2`) on the
    /// chosen file's layer `DAT_0056D590`, and 0 and 1 raise
    /// `Battlefield_BuildRandom` on `batfiel2.pl8`.
    ///
    /// A `.skr` that will not open falls back to the random field, as the
    /// original's `File_ReadChunk` leaves the previous layer standing.
    pub fn skirmish_field(&self, ctx: &Ctx, seed: u64) -> l2_sim::Battlefield {
        let s = &self.skirmish;
        if s.kind == 3 {
            let layer = s
                .file
                .as_deref()
                .and_then(|n| ctx.assets.shell.skr(n).and_then(|f| f.terrain(s.row).ok().map(<[u8]>::to_vec)));
            if let Some(layer) = layer {
                return l2_sim::terrain::build(&layer, seed as u32);
            }
        }
        crate::batfield::skirmish_field(s.kind, s.row, seed)
    }

    /// **`FUN_00420D40` (`0x00420D40`, `00420000.c:296-325`)** — twelve
    /// frames of `Misc_sel.pl8` from `0x21`, one step per 100 ms, and
    /// `Gfx_MarkAllDirty` on each step.
    ///
    /// **[D]** the original's clock is `timeGetTime()` and it stores the
    /// reading rather than adding 100, so its steps drift with the frame. Ours
    /// counts `press::TICK_MS` ticks, which is deterministic — `docs/netcode.md`
    /// — and lands on the first tick past 100 ms. Presentation only: the frame
    /// number reaches no digest.
    pub(crate) fn tick_banner(&mut self) {
        self.banner_ms += crate::press::TICK_MS;
        if self.banner_ms <= BANNER_MS {
            return;
        }
        self.banner_ms = 0;
        self.banner = (self.banner + 1) % BANNER_FRAMES;
        self.clock_redraw = true;
    }
}

/// `if (100 < (int)(DVar1 - _DAT_004D31EC))` — strictly greater, and the
/// timestamp is replaced rather than advanced by 100.
pub const BANNER_MS: u32 = 100;

/// `if (0xB < DAT_004D31E8) DAT_004D31E8 = 0;` — twelve frames, `0x21`…`0x2C`.
pub const BANNER_FRAMES: usize = 12;

/// `Pl8_DrawFrame(g_miscCtySheet, DAT_004D31E8 + 0x21, …)`.
pub const BANNER_FRAME_BASE: usize = 0x21;

/// The banner's two anchors: `(0xB2, 0xF4)` with the musters normal and
/// `(0x10E, 0xCE)` with `DAT_0056899C` up.
pub const BANNER_AT: [(i32, i32); 2] = [(0xB2, 0xF4), (0x10E, 0xCE)];
