#![allow(unused_imports)]
use super::*;
use super::corpus::*;
use super::render::*;
use std::{fs, path::{Path, PathBuf}};
use l2_sim::runner::{self as battle, BattleRunner};
use l2_sim::terrain;
use l2_sim::{Troop, SIDE_A, SIDE_B};
use l2_view::campaign;
use l2_view::canvas::Canvas;
use l2_view::chrome;
use l2_view::figures::{self, Anim, Colour};
use l2_view::scene::{self, BattleAssets, Camera};
use l2_view::sheet::Sheet;

/// Map a virtual address to a file offset through the PE section headers.
/// `Lords2.exe` has no ASLR and a fixed image base of `0x400000`, so a virtual
/// address is a constant.
fn va_to_offset(exe: &[u8], va: u32) -> Option<usize> {
    let pe = u32::from_le_bytes(exe[0x3C..0x40].try_into().ok()?) as usize;
    let sections = u16::from_le_bytes(exe[pe + 6..pe + 8].try_into().ok()?) as usize;
    let opt_size = u16::from_le_bytes(exe[pe + 20..pe + 22].try_into().ok()?) as usize;
    for i in 0..sections {
        let s = pe + 24 + opt_size + i * 40;
        let rva = u32::from_le_bytes(exe[s + 12..s + 16].try_into().ok()?);
        let raw_size = u32::from_le_bytes(exe[s + 16..s + 20].try_into().ok()?);
        let raw_ptr = u32::from_le_bytes(exe[s + 20..s + 24].try_into().ok()?);
        let start = 0x0040_0000 + rva;
        if va >= start && va < start + raw_size {
            return Some((raw_ptr + (va - start)) as usize);
        }
    }
    None
}

/// **The campaign map's walk tables are the bytes in the user's own
/// `Lords2.exe`** — `Map_DrawArmies` (`0x00408438`) reads `[facing * 16 +
/// +0x149]` out of `0x004D8108`/`0x004D8188` at the near zoom and
/// `0x004D8308`/`0x004D8388` at the far one, as `i8`.
///
/// `campaign::walk_offset`'s tables were generated from a copy of the file
///; this is what says that copy was the user's, all 256
/// entries of both zooms. The unit test beside the tables checks them against
/// the projection instead, which is the second source.
#[test]
fn the_campaign_walk_tables_are_the_bytes_in_the_binary() {
    use l2_view::campaign;
    let exe = l2_testkit::executable!();
    for (zoom, xs, ys) in [(campaign::NEAR, 0x004D_8108u32, 0x004D_8188u32), (campaign::FAR, 0x004D_8308, 0x004D_8388)] {
        let (tx, ty) = (l2_testkit::pe::Table::at(&exe, xs), l2_testkit::pe::Table::at(&exe, ys));
        for facing in 0..8u8 {
            for step in 0..16u8 {
                let i = facing as usize * 16 + step as usize;
                let want = (tx.u8_at(i) as i8 as i32, ty.u8_at(i) as i8 as i32);
                assert_eq!(
                    campaign::walk_offset(&zoom, facing, step),
                    want,
                    "zoom {}, facing {facing}, +0x149 = {step}: {:#010X} / {:#010X}",
                    zoom.id,
                    xs,
                    ys
                );
            }
        }
    }
}

/// `walk_offset` is a formula standing in for an 8 x 17 table of `(i32, i32)`
/// pairs at `0x004E4030`. This reads that table out of the binary and checks
/// the formula reproduces all 136 entries exactly.
///
/// Width matters here and is easy to get wrong: the entries are pairs of
/// **i32**, which the 0x88-byte per-facing stride confirms — 17 entries times
/// 8 bytes — and which the neighbouring 16-pixel table at `0x004E3BF0` confirms
/// again, sitting exactly `8 * 0x88` bytes earlier.
#[test]
fn the_walk_offsets_match_the_table_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    const TABLE_VA: u32 = 0x004E_4030;
    const STRIDE: u32 = 0x88;
    let Some(base) = va_to_offset(&exe, TABLE_VA) else {
        panic!("0x{TABLE_VA:08X} is not inside any initialised section");
    };

    let read_i32 = |off: usize| i32::from_le_bytes(exe[off..off + 4].try_into().unwrap());
    let mut compared = 0;
    for facing in 0..8u32 {
        for walking in 0..17u32 {
            let off = base + (facing * STRIDE + walking * 8) as usize;
            let want = (read_i32(off), read_i32(off + 4));
            let got = figures::walk_offset(facing as u8, walking as u8);
            assert_eq!(got, want, "facing {facing}, walking {walking}");
            compared += 1;
        }
    }
    assert_eq!(compared, 8 * 17);

    // The sign of facing 0's offset is what independently fixes the facing
    // numbering: north trails to the south.
    assert_eq!(figures::walk_offset(0, 1), (0, 30));
    eprintln!("walk offsets: {compared} entries match the binary");
}

/// The tile artwork's own dimensions, read out of the shipped PL8 frame tables,
/// must agree with the pitch `Map_SetZoom` steps by.
///
/// This is `docs/screens.md` §1.2, and it is the resolution of the 58-versus-60
/// discrepancy `maps-layers.md` §6 left open: **pitch = frame width + 2** and
/// **row step = frame height / 2**, at both zooms, over all ten banks. The
/// constants in `campaign` come from the instruction stream; the widths come
/// from the files; nothing here is written down twice.
#[test]
fn every_campaign_tile_bank_matches_the_pitch_the_binary_steps_by() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let mut checked = 0;
    let mut seen: Vec<&str> = Vec::new();
    for zoom in campaign::ZOOMS {
        for set in zoom.banks {
            for name in set {
                if seen.contains(&name) {
                    continue;
                }
                seen.push(name);
                let Some(bytes) = read(&dir, name) else {
                    panic!("{name} is not in the install");
                };
                let pl8 = l2_formats::Pl8::parse(&bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
                assert!(!pl8.frames.is_empty(), "{name} has no frames");
                for (i, f) in pl8.frames.iter().enumerate() {
                    assert_eq!(
                        f.width as i32, zoom.tile_w,
                        "{name} frame {i} is {} wide, but zoom {} steps by {}",
                        f.width, zoom.id, zoom.pitch
                    );
                    assert_eq!(f.height as i32, zoom.tile_h, "{name} frame {i}");
                    checked += 1;
                }
                assert_eq!(zoom.pitch, zoom.tile_w + 2);
                assert_eq!(zoom.row_step, zoom.tile_h / 2);
            }
        }
    }
    // Twenty near-zoom files — four seasons of base, mtns, roads, town and
    // castle, none of them repeating — plus five far-zoom ones, which are one
    // season named four times. Twenty-five names from a 2 × 4 × 5 table of
    // forty, and the difference is the far zoom's season-invariance.
    assert_eq!(seen.len(), 25, "the distinct bank files across both zooms");
    assert_eq!(checked, 4 * (140 + 25 + 140 + 61 + 100) + (140 + 25 + 140 + 61 + 100));
    eprintln!("campaign tiles: {checked} frames in {} files", seen.len());
}

/// **The measurement the seasonal artwork rests on: do the four seasonal files
/// of a bank share a frame table?**
///
/// If they do not, `campaign::Overrides` does not survive a season — the game
/// rewrites a town's tiles to `Town1a.pl8` frames 47 … 50 and if frame 47 of
/// `Town1c.pl8` were a different picture, every town on the map would turn back
/// into a quarry every autumn. It is a cheap thing to check and it decides
/// whether the season is a lookup table or a real piece of work.
///
/// **It is a lookup table.** Across all five near-zoom banks and all 466 frames
/// of each season:
///
/// * the **frame count** is identical in all four files of every bank;
/// * the **canvas anchor** `(X, Y)` — where the artist put the cell on the
///   sheet — is identical for every frame of every bank, 1,864 of 1,864;
/// * the diamond's `width`, `height` and `shape` are identical for every frame
///   of every bank.
///
/// The **only** structural difference anywhere is the `rows` byte — how many
/// overhang scanlines stand above the diamond — on nine `Roads1?.pl8` frames,
/// 109 and 111 and 113 … 119, and it differs by one or two. Those are frames
/// 108 … 120, which [`campaign::FIELD_BASES`] identifies as the four
/// reclamation crops: a crop that grows needs a row more of picture above the
/// tile in the season it is taller. That is artwork varying, not an index
/// moving.
///
/// So frame *n* of a bank is the same cell of the same sheet in every season,
/// and an override recorded in spring is still correct in winter.
#[test]
fn the_four_seasons_of_a_bank_are_the_same_frame_table() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let zoom = campaign::NEAR;
    let mut frames_checked = 0;
    let mut anchors_checked = 0;
    let mut row_differences = Vec::new();
    for bank in 0..5 {
        let files: Vec<_> = (0..campaign::SEASONS)
            .map(|s| {
                let name = zoom.banks[s][bank];
                let bytes = read(&dir, name).unwrap_or_else(|| panic!("{name} is not installed"));
                (name, bytes)
            })
            .collect();
        let parsed: Vec<_> = files
            .iter()
            .map(|(n, b)| (*n, l2_formats::Pl8::parse(b).unwrap_or_else(|e| panic!("{n}: {e}"))))
            .collect();
        let (spring_name, spring) = &parsed[0];
        for (name, other) in &parsed[1..] {
            assert_eq!(
                other.frames.len(),
                spring.frames.len(),
                "{name} has {} frames and {spring_name} has {}",
                other.frames.len(),
                spring.frames.len()
            );
            for (i, (a, b)) in spring.frames.iter().zip(other.frames.iter()).enumerate() {
                // The anchor is the artist's own sheet coordinate. If frame `i`
                // moved on the sheet between seasons, the index would not mean
                // the same cell — this is the assertion that matters.
                assert_eq!(
                    (a.x, a.y),
                    (b.x, b.y),
                    "{name} frame {i} sits at a different place on the sheet than {spring_name}'s"
                );
                anchors_checked += 1;
                assert_eq!((a.width, a.height), (b.width, b.height), "{name} frame {i} size");
                if a.overhang_rows != b.overhang_rows {
                    row_differences.push((*name, i, a.overhang_rows, b.overhang_rows));
                }
                frames_checked += 1;
            }
        }
    }
    assert_eq!(anchors_checked, 3 * (140 + 25 + 140 + 61 + 100), "every frame of every bank");
    assert_eq!(frames_checked, anchors_checked);

    // The nine overhang differences, and nothing else. Naming them exactly is
    // what turns "we looked and it was fine" into a claim that fails if the
    // artwork ever stops matching this reading.
    let mut differing: Vec<usize> =
        row_differences.iter().map(|(_, i, _, _)| *i).collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
    differing.sort_unstable();
    assert_eq!(
        differing,
        vec![109, 111, 113, 114, 115, 116, 117, 118, 119],
        "the only per-season structural differences should be nine Roads frames"
    );
    assert!(
        row_differences.iter().all(|(n, ..)| n.starts_with("Roads1")),
        "an overhang difference outside the roads bank: {row_differences:?}"
    );
    // Each of the nine is one of the four-frame crop blocks 108/112/116/120.
    for (_, i, ..) in &row_differences {
        let base = (i / 4) * 4;
        assert!(
            (108..=120).contains(&base),
            "frame {i} differs by season but is not in a crop block"
        );
    }
    assert!(
        row_differences.iter().all(|(_, _, a, b)| a.abs_diff(*b) <= 2),
        "an overhang differs by more than two rows: {row_differences:?}"
    );
    eprintln!(
        "seasons: {frames_checked} frames compared, {} overhang differences, 0 index moves",
        row_differences.len()
    );
}

/// **The far zoom is not seasonal, and the shipped files say otherwise.**
///
/// `g_resourceTable`'s zoom-2 half names `base2a`/`mtns2a`/… in all four of its
/// season blocks (see [`campaign::FAR`]), so `Base2b.pl8` and its eleven
/// siblings are dead weight in the install. This asserts the consequence of
/// getting that wrong: the dead `Town2b.pl8` is **not** interchangeable with
/// the live `Town2a.pl8`, so a renderer that derived the far zoom's filenames
/// from the season suffix would draw a different sheet for three seasons in
/// four.
#[test]
fn the_far_zoom_names_one_season_four_times_and_the_unused_files_do_not_match() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    for season in 1..=campaign::SEASONS {
        assert_eq!(
            campaign::FAR.banks[season - 1],
            campaign::FAR.banks[0],
            "season {season} of the far zoom should name spring's files"
        );
    }
    // …and the reason it matters.
    let a = read(&dir, "Town2a.pl8").expect("Town2a.pl8");
    let b = read(&dir, "Town2b.pl8").expect("Town2b.pl8");
    let (a, b) = (l2_formats::Pl8::parse(&a).unwrap(), l2_formats::Pl8::parse(&b).unwrap());
    assert_eq!(a.frames.len(), 61, "the live far-zoom town bank");
    assert_eq!(b.frames.len(), 94, "the dead one is a different sheet entirely");
    eprintln!("far zoom: Town2a has 61 frames, the unused Town2b has 94");
}

/// The minimap's realm colour ramp is `Lords2.exe`'s own data, transcribed into
/// `chrome::MINIMAP_REALM_RAMP`. This reads the 48 bytes back out of the user's
/// binary and fails if a single one differs.
///
/// The stride matters and is not guessable from the values: `Minimap_DrawOverlay`
/// indexes `realmColour * 8 + (shade - 10)`, so the records are **eight** bytes
/// of which four are used. The unused half of each record is the same four
/// bytes with the first replaced by 0x20 — which is exactly the substitution
/// the function makes by hand for the selected county, and is the corroboration
/// that the stride is right.
#[test]
fn the_minimap_realm_ramp_matches_the_table_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    let Some(base) = va_to_offset(&exe, chrome::MINIMAP_REALM_RAMP_VA) else {
        panic!("0x{:08X} is not inside any initialised section", chrome::MINIMAP_REALM_RAMP_VA);
    };
    for (colour, want) in chrome::MINIMAP_REALM_RAMP.iter().enumerate() {
        let rec = &exe[base + colour * 8..base + colour * 8 + 8];
        assert_eq!(&rec[..4], want, "realm colour {colour}");
        // The second half of the record is the first with the selected-county
        // substitution already applied.
        assert_eq!(rec[4], chrome::MINIMAP_SELECTED, "realm colour {colour} selected entry");
        assert_eq!(&rec[5..], &want[1..], "realm colour {colour} tail");
    }
    eprintln!("minimap ramp: 6 realm colours match the binary");
}

/// The **rating** ramp the three statistic overlays index, read back out of the
/// user's own binary at `chrome::MINIMAP_RATING_RAMP_VA`.
///
/// It sits eight bytes below the realm ramp, and the two are separate tables:
/// `Minimap_DrawOverlay` indexes this one with a bare `[band]` and that one with
/// `[colour * 8 + step]`. The two bytes between them are indexed by nothing, and
/// this asserts they are there
/// eight.
#[test]
fn the_minimap_rating_ramp_matches_the_table_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    assert_eq!(
        chrome::MINIMAP_RATING_RAMP_VA + 8,
        chrome::MINIMAP_REALM_RAMP_VA,
        "the two ramps are adjacent, and that is why they get confused"
    );
    let Some(base) = va_to_offset(&exe, chrome::MINIMAP_RATING_RAMP_VA) else {
        panic!("0x{:08X} is not inside any initialised section", chrome::MINIMAP_RATING_RAMP_VA);
    };
    assert_eq!(
        &exe[base..base + chrome::MINIMAP_RATING_RAMP.len()],
        &chrome::MINIMAP_RATING_RAMP,
        "the six rating colours"
    );
    eprintln!("minimap ramp: 6 rating colours match the binary");
}

/// **The rating ramp's direction, from the artwork.**
///
/// `Misc_cty.pl8` frame 91 is the strip the original swaps in beside the minimap
/// while an overlay is up, and it carries a six-swatch colour bar with a tick
/// against one end and a cross against the other. Reading the bar's pixels top
/// to bottom gives `chrome::MINIMAP_RATING_RAMP` **reversed** — so index 0 is
/// the crossed end and index 5 the ticked one, which is what makes
/// "`happiness / 20`" a rating and not an arbitrary number.
///
/// This is a cross-check between two things nobody coordinated: a table of
/// palette indices in the code segment and a painted legend in an art file.
#[test]
fn the_rating_ramp_is_the_colour_bar_the_legend_strip_draws() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(bytes) = read(&dir, "Misc_cty.pl8") else {
        l2_testkit::skip!("Misc_cty.pl8 not present - skipping");
    };
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Misc_cty.pl8 parses");
    let frame = pl8
        .decode(l2_view::chrome::misc_cty::MINIMAP_SIDE_ACTIVE)
        .expect("frame 0x5B decodes");
    assert_eq!((frame.width, frame.height), (29, 123), "the 29 x 123 mode strip");

    // Column 5 runs down the middle of the swatch bar. Collect the runs.
    let w = frame.width as usize;
    let mut runs: Vec<(u8, usize)> = Vec::new();
    for y in 0..frame.height as usize {
        let v = frame.indices[y * w + 5];
        match runs.last_mut() {
            Some((c, n)) if *c == v => *n += 1,
            _ => runs.push((v, 1)),
        }
    }
    // The swatches are the only runs more than ten rows tall.
    let bar: Vec<u8> = runs.iter().filter(|(_, n)| *n >= 10).map(|(c, _)| *c).collect();
    let mut want = chrome::MINIMAP_RATING_RAMP;
    want.reverse();
    assert_eq!(bar, want, "the legend bar is the rating ramp, best first");
    eprintln!("minimap legend: frame 0x5B's colour bar is the rating ramp reversed");
}

/// `Minimap_Load`'s file and frame arithmetic, checked against the install:
/// every used map slot must resolve to a `MAPnn.PL8` that exists and holds two
/// 128 x 128 frames where the formula says, and every *empty* slot must resolve
/// to one of the four files the game does not ship.
///
/// The second half is what makes this evidence: 11
/// files x 4 slots = 44 is exactly the used-slot census in
/// `docs/formats/maps-layers.md` §6, arrived at from a completely different
/// direction.
#[test]
fn every_used_map_slot_has_a_minimap_and_every_empty_one_does_not() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(maps) = read(&dir, "L2_maps.dat") else {
        l2_testkit::skip!("L2_maps.dat not present - skipping");
    };
    let set = l2_formats::MapSet::parse(&maps).expect("L2_maps.dat parses");
    let used = set.used_slots();

    let mut with_minimap = 0;
    for slot in 0..60usize {
        let name = chrome::Minimap::file_for_slot(slot);
        match read(&dir, &name) {
            Some(bytes) => {
                let m = chrome::Minimap::load(&bytes, slot)
                    .unwrap_or_else(|e| panic!("slot {slot} in {name}: {e}"));
                assert_eq!(m.counties.len(), 128 * 128);
                assert!(used.contains(&slot), "slot {slot} has a minimap but no map");
                with_minimap += 1;
            }
            None => assert!(!used.contains(&slot), "slot {slot} is used but {name} is missing"),
        }
    }
    assert_eq!(with_minimap, 44, "11 shipped MAPnn.PL8 files times 4 slots");
    assert_eq!(with_minimap, used.iter().filter(|&&s| s < 60).count());
    eprintln!("minimaps: {with_minimap} slots, matching the used-slot census");
}

/// **The icon table is the shipped one, and a man past his job's ceiling is
/// drawn with the idle townsman's own sprite.**
///
/// A player reported *"when I put peasants into a quarry they show as idle,
/// which should be impossible"*. The **drawing** is faithful and he was reading
/// the picture correctly; what is missing is upstream of it. The quarry was
/// switched off, so `County_RefreshEstimates` left its `labour_useful` at zero,
/// and `Village_RebuildIcons` (`0x0045161E`) draws every worker past a zero
/// ceiling in the **surplus** frame — which is `g_peasantIcons[8]`, the idle
/// townsman's own frame, so the two are pixel-identical.
///
/// In the original that state is unreachable by this route: `FUN_00439CC2`,
/// which `Labour_Move` calls on every drop, **switches the industry on** when
/// men are dropped on a site whose resource the county has. Ours did not, so it
/// reached a picture the original only ever shows for a county that has.
/// resource at all. It does now — `l2_kingdom::Kingdom::move_labour`, and
/// `crates/l2-game/tests/labour_move.rs` drives it through the village. (This
/// comment named a `docs/arms.json` row for it: it is not
/// an input arm but a call inside the drop's.) This pins the drawing half so
/// that the input half cannot quietly change what the icons mean.
///
/// [`l2_view::village::ICON_VALUE`] was transcribed by hand and nothing checked
/// it, which is the shape `docs/agents.md` warns about: a table of nine numbers
/// with no oracle, in the crate that decides what the player sees. This reads
/// the nine dwords back out of the user's own `Lords2.exe` and asserts the
/// identity that caused the confusion, so it cannot be "tidied" by someone who
/// thinks two jobs sharing a frame is a typo.
#[test]
fn the_icon_table_is_the_exes_own_and_surplus_is_the_idle_sprite() {
    use l2_view::village as v;
    let exe = l2_testkit::executable!();
    let t = l2_testkit::pe::Table::at(&exe, v::ICON_VALUE_VA);
    let shipped: Vec<i32> = t.i32s(v::ICON_VALUE.len());
    let ours: Vec<i32> = v::ICON_VALUE.iter().map(|&b| b as i32).collect();
    assert_eq!(
        shipped, ours,
        "g_peasantIcons at {:#010X} is not what l2_view::village::ICON_VALUE says",
        v::ICON_VALUE_VA,
    );
    assert_eq!(
        v::ICON_VALUE[l2_kingdom_job_idle()], v::ICON_SURPLUS,
        "the idle townsman and the surplus worker are drawn with the same frame, and a \
         player has already read one as the other. If this ever stops being true, \
         docs/rules.md's paragraph on a switched-off industry is wrong.",
    );
}

/// Slot 8. Spelled out, which this crate
/// does not and should not know about.
fn l2_kingdom_job_idle() -> usize {
    8
}

/// **`Misc_cty.pl8` frames 0 … 0x16 are the village's peasant icons**, which
/// `docs/screens-county.md` §9 guessed, marked `[I]`, were "almost certainly
/// the top menu bar".
///
/// The icon table at `0x004D6808` names nine (normal, highlighted) pairs among
/// them, plus a shortfall pair and a surplus pair. This checks the shipped file
/// against that: **every frame the table names is 16 × 32 and decodes, and the
/// four it never names — 5, 6, 11 and 12 — are exactly the four frames in that
/// range that are 2 × 2 stubs.** Nineteen icons, nineteen frames, nothing over.
#[test]
fn the_peasant_icons_account_for_every_frame_the_icon_table_names() {
    use l2_view::village as v;
    let Some(dir) = asset_dir() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };
    let Some(bytes) = read(&dir, "Misc_cty.pl8") else {
        eprintln!("no Misc_cty.pl8 - skipping");
        return;
    };
    let sheet = Sheet::new(bytes.clone()).expect("Misc_cty.pl8 parses");
    let pl8 = l2_formats::Pl8::parse(&bytes).expect("Misc_cty.pl8 parses");

    // Every frame the drawing code can ask for: value - 1, and value while the
    // icon is selected.
    let mut named = std::collections::BTreeSet::new();
    for value in v::ICON_VALUE.iter().copied().chain([v::ICON_SHORTFALL, v::ICON_SURPLUS]) {
        named.insert(value as usize - 1);
        named.insert(value as usize);
    }
    assert_eq!(*named.iter().max().unwrap(), 22, "the icons stop at frame 0x16");

    let mut canvas = Canvas::screen();
    for &f in &named {
        assert_eq!(
            (pl8.frames[f].width, pl8.frames[f].height),
            (16, 32),
            "frame {f} is named by the icon table and is not an icon"
        );
        let decoded = sheet.frame(f).expect("frame {f} decodes");
        canvas.blit(&decoded, 100, 100);
    }

    let unnamed: Vec<usize> = (0..=22).filter(|f| !named.contains(f)).collect();
    assert_eq!(unnamed, vec![5, 6, 11, 12], "four frames in the range go unused");
    for f in unnamed {
        assert_eq!(
            (pl8.frames[f].width, pl8.frames[f].height),
            (2, 2),
            "frame {f} is unused and should be a stub"
        );
    }
    eprintln!("Misc_cty: {} named icons, 4 stubs, nothing over", named.len());
}

/// **The ten words `Village_BalanceAll` reads out of an eight-word table.**
///
/// `FUN_00439EDB` loops `i < 10` over `g_jobClusterToSlot`, which has eight
/// entries. The two past the end are the head of the table that follows, and
/// they decide what a double click on the idle townsfolk balances — so
/// they are read out of the user's own executable.
#[test]
fn the_cluster_to_slot_table_and_the_two_words_the_balance_loop_overruns_into() {
    use l2_view::village as v;
    let Some(exe) = l2_testkit::executable() else {
        l2_testkit::skip!("no Lords2.exe - skipping");
    };
    let table = l2_testkit::pe::Table::at(&exe, v::CLUSTER_TO_SLOT_VA);
    let read: Vec<usize> = table.i32s(10).into_iter().map(|v| v as usize).collect();
    assert_eq!(read[..8], v::CLUSTER_TO_SLOT, "the eight the table really has");
    assert_eq!(read, v::CLUSTER_TO_SLOT_BALANCE, "and the ten the balance loop reads");
    // The point of the two extra words: they bring slot 4, iron mining, into a
    // gesture that the eight-entry table can otherwise only reach through
    // cluster 0's override.
    assert!(read[8..].contains(&4), "the overrun reaches iron mining");
}

/// The village's own three files, and the arithmetic that ties them together.
#[test]
fn the_village_files_are_the_size_the_drawing_code_indexes_them_at() {
    use l2_view::village as v;
    let Some(dir) = asset_dir() else {
        eprintln!("LORDS2_DIR not set - skipping");
        return;
    };

    // The scene: one frame, 363 x 320, drawn at (0x40, g_villageTopY).
    let scene_bytes = read(&dir, "vill.pl8").expect("vill.pl8");
    let scene = l2_formats::Pl8::parse(&scene_bytes).unwrap();
    assert_eq!(scene.frames.len(), 1);
    assert_eq!((scene.frames[0].width as i32, scene.frames[0].height as i32), (363, v::SCENE_H));

    // The tops: six frames, one per weather, and `L2.eng` group 66 has six
    // names. Village_Draw indexes both with the same county byte.
    let tops_bytes = read(&dir, "villtops.pl8").expect("villtops.pl8");
    let tops = l2_formats::Pl8::parse(&tops_bytes).unwrap();
    assert_eq!(tops.frames.len(), 6, "six weathers");
    for f in &tops.frames {
        assert_eq!((f.width as i32, f.height), (363, 70));
    }

    // The drop grid: 24 bytes of header and 45 x 40 cells, and nothing else.
    let grid = read(&dir, "vill_gd8.pl8").expect("vill_gd8.pl8");
    assert_eq!(grid.len(), 0x18 + v::GRID_LEN, "1,824 bytes: 24 + 45 * 40");
    assert_eq!(v::GRID_COLS as i32 * v::GRID_CELL, 360, "x 0x40 .. 0x1A8");
    assert_eq!(v::GRID_ROWS as i32 * v::GRID_CELL, v::SCENE_H, "y top .. top + 0x140");
    // Every cell names a cluster or nothing; nothing names a ninth cluster.
    assert!(
        grid[0x18..].iter().all(|&b| b as usize <= v::CLUSTER_COUNT),
        "a cell names a cluster the village does not draw"
    );
    eprintln!("village: 363x320 scene, 6 weather tops, 45x40 grid");
}

/// **The realm pen table is `Lords2.exe`'s own ten bytes**, read back out of
/// the user's copy at `0x004DC1D0`.
///
/// `l2_view::chrome::REALM_PEN` is a transcription, and a transcription that
/// nothing checks is a table somebody eventually edits by eye. The stride is
/// the part that is not guessable from the values: the binary indexes from
/// **two bytes below** the data — `(&g_realmColour)[shieldIndex * 2]` with
/// `g_realmColour` at `0x004DC1CE` — so that the 1-based shield lands on the
/// first pair, and reading it the obvious way is off by one entry.
#[test]
fn the_realm_pen_table_matches_the_bytes_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    let Some(base) = va_to_offset(&exe, chrome::REALM_PEN_VA) else {
        panic!("{:#010X} is not in any section", chrome::REALM_PEN_VA);
    };
    let want: Vec<u8> = chrome::REALM_PEN.iter().flatten().copied().collect();
    assert_eq!(&exe[base..base + want.len()], &want[..], "g_realmColour differs from ours");

    // The two bytes the binary's own base points at are *not* part of this
    // table — they are the tail of `g_lordChoice`. Asserting that pins the
    // 1-based indexing: if the table really started at `0x004DC1CE` these would
    // be shield 1's pen and they would have to be a colour pair.
    assert_eq!(
        &exe[base - 2..base],
        &[0x04, 0x02][..],
        "the two bytes below the table are g_lordChoice's tail, not a sixth pen"
    );

    // And the ten bytes are followed by zeros: five shields and no more.
    assert!(
        exe[base + want.len()..base + want.len() + 8].iter().all(|&b| b == 0),
        "something follows the fifth pair"
    );
    eprintln!("realm pens: {} bytes match Lords2.exe at {:#010X}", want.len(), chrome::REALM_PEN_VA);
}

/// **The pen really is keyed by the shield**, checked against every saved game
/// this project keeps — including the ones that separate the two candidate
/// keys.
///
/// `l2_view::chrome::realm_pen` *derives* the pen from the shield
/// reading realm `+0x08` out of the save, because both writers in the binary
/// derive it the same way and nothing else touches the field
/// (`Realms_AssignLords` at new game, `FUN_0042BA40` for a custom battle). This
/// is that claim tested against data: for every realm of every fixture, the
/// byte the game stored at `+0x08` must equal our table indexed by `+0x0A`.
///
/// **The fixtures are what make this decisive.** In
/// `england-turn1.sav` realm *n* happens to fly shield *n*, so it cannot tell
/// "keyed by the shield" from "keyed by the realm id" — and the realm id is
/// exactly the wrong key our county strip was using. Six of the other fixtures
/// have **realm 1 flying shield 5**, and there realm 1's stored pen is `0x04`,
/// blue, which is shield 5's. That is the observation the fix rests on.
#[test]
fn every_saved_realms_stored_pen_is_its_shields_pen() {
    let Some(dir) = l2_testkit::fixtures_dir() else {
        l2_testkit::skip!("LORDS2_FIXTURES not set - skipping");
    };
    let Some(install) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - the save schema comes from the executable");
    };
    let Some(exe) = read(&install, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };

    let mut realms_checked = 0;
    let mut files_checked = 0;
// Did any fixture exercise a realm whose id differs from its
    // shield? Without one this test would pass on the wrong key too.
    let mut separating = 0;
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("the fixture directory")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.to_ascii_lowercase().ends_with(".sav"))
        .collect();
    names.sort();
    for name in &names {
        let Ok(bytes) = std::fs::read(dir.join(name)) else { continue };
        // Loud, not silent: a save this cannot open is a schema problem, and
        // skipping it quietly is how a test ends up asserting over nothing.
        let save = l2_formats::save::Save::open(&exe, &bytes)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        files_checked += 1;
        for index in 1..l2_formats::save::REALM_RECORDS {
            let Ok(realm) = save.realm(index) else { continue };
            if realm.shield_index == 0 {
                continue;
            }
            let stored = save
                .u8_at(
                    l2_formats::save::REALM_BASE
                        + (index * l2_formats::save::REALM_STRIDE) as u32
                        + 0x08,
                )
                .expect("realm +0x08 is inside the realm block");
            assert_eq!(
                Some(stored),
                chrome::realm_pen(realm.shield_index),
                "{name}: realm {index} flies shield {} and stored pen {stored:#04X}",
                realm.shield_index
            );
            if index as u8 != realm.shield_index {
                separating += 1;
            }
            realms_checked += 1;
        }
    }
    assert!(files_checked >= 1, "no fixture saves were readable");
    assert!(
        separating >= 2,
        "no fixture has a realm whose id differs from its shield, so this cannot tell the \
         two keys apart - it passed for the wrong reason"
    );
    eprintln!(
        "realm pens: {realms_checked} realms across {files_checked} saves, {separating} of them \
         with id != shield"
    );
}

/// **The two tables that decide which lord flies which colour**, read out of
/// the user's own executable — and the arithmetic that says which of them the
/// campaign uses.
///
/// A player described the rule as *"the game will always try to give the Knight
/// yellow, the Countess blue, the Bishop purple/pink … and it'll move a noble's
/// colour around if you pick it."* Every colour is right and the mechanism is
/// two mechanisms:
///
/// * **`g_lordChoice`** (`0x004DC17C`) is per **colour**, four candidate lords
///   per shield. `Realms_AssignLords` hands out the lowest free shield by realm
///   order and then picks the lord *from the shield*, so no lord has a
///   preference on the campaign path — the appearance of one is this table's
///   first column.
/// * **`g_battleLordShield`** (`0x004D4CA8`) is per **lord**, `{preferred,
///   alternate}`, and really is preference-then-fallback. It belongs to the
///   custom battle (`FUN_0042BA40`) and nothing else.
///
/// `docs/rules.md` §7a has both halves and the table of what happens
/// when a person takes each of the five colours.
#[test]
fn the_lord_colour_tables_are_the_bytes_in_the_binary() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    let u32_at = |va: u32| -> u32 {
        let o = va_to_offset(&exe, va).unwrap_or_else(|| panic!("{va:#010X} unmapped"));
        u32::from_le_bytes(exe[o..o + 4].try_into().unwrap())
    };
    let u8_at = |va: u32| -> u8 {
        exe[va_to_offset(&exe, va).unwrap_or_else(|| panic!("{va:#010X} unmapped"))]
    };

    // --- `g_battleLordShield`: the per-lord preference, and the only one.
    // Knight, Baron, Countess, Bishop are lord ids 1 … 4 (`L2.eng` group 7).
    const BATTLE_LORD_SHIELD_VA: u32 = 0x004D_4CA8;
    let pair = |lord: u32| {
        (u32_at(BATTLE_LORD_SHIELD_VA + lord * 8), u32_at(BATTLE_LORD_SHIELD_VA + lord * 8 + 4))
    };
    assert_eq!(pair(1), (2, 4), "the Knight prefers yellow, falling back to magenta");
    assert_eq!(pair(2), (1, 5), "the Baron prefers red, falling back to blue");
    assert_eq!(pair(3), (5, 1), "the Countess prefers blue, falling back to red");
    assert_eq!(pair(4), (4, 2), "the Bishop prefers magenta, falling back to yellow");
    // Three of the four first columns are the player's own words. The Baron is
    // the one he could not remember, and in *this* table it is red — while the
    // campaign gives him black, which is the whole point of §7a.
    assert_ne!(pair(2).0, 3, "the Baron's battle preference is not the campaign's black");

    // --- `g_lordChoice`: per colour, and it is the campaign's.
    const LORD_CHOICE_VA: u32 = 0x004D_C17C;
    let candidates = |group: u32, shield: u32| -> [u8; 4] {
        let base = LORD_CHOICE_VA + group * 0x14 + shield * 4;
        [u8_at(base), u8_at(base + 1), u8_at(base + 2), u8_at(base + 3)]
    };
    // England is slot 0, so group 0 — the arrangement every default game shows.
    assert_eq!(candidates(0, 2), [1, 3, 4, 2], "yellow leads with the Knight");
    assert_eq!(candidates(0, 3), [2, 1, 3, 4], "black leads with the Baron");
    assert_eq!(candidates(0, 4), [4, 3, 2, 1], "magenta leads with the Bishop");
    assert_eq!(candidates(0, 5), [3, 2, 4, 1], "blue leads with the Countess");
    assert_eq!(candidates(0, 1), [2, 1, 3, 4], "red leads with the Baron");

    // Slot 0 is never indexed — shields are 1 … 5 — and the index runs one row
    // off its own group, so slot 5 of group *g* is slot 0 of group *g+1*.
    for group in 0..3 {
        assert_eq!(
            candidates(group, 5),
            candidates(group + 1, 0),
            "group {group}'s slot 5 should be group {}'s slot 0",
            group + 1
        );
    }
    // …and group 3's overrun lands on `g_realmColour`'s own base, overlapping
    // it by two bytes. That is what the two bytes below the pen table are, and
    // `the_realm_pen_table_matches_the_bytes_in_the_binary` asserts their
    // values from the other side.
    let overrun = LORD_CHOICE_VA + 3 * 0x14 + 5 * 4;
    assert_eq!(overrun, 0x004D_C1CC);
    assert_eq!(overrun + 2, chrome::REALM_PEN_VA - 2, "the two tables overlap by two bytes");
    assert_eq!(candidates(3, 5)[2..], [0x04, 0x02], "and those two bytes are shared");

    eprintln!(
        "lord colours: g_lordChoice is per-colour, g_battleLordShield per-lord; \
         the two tables overlap by two bytes at {:#010X}",
        overrun + 2
    );
}

/// **What the campaign produces for each of the five colours a person
/// can take** — the rule, run against the tables the test above pinned.
///
/// This is `Realms_AssignLords`' walk, and it is the thing no fixture can
/// check: every `.sav` this project holds has the human on shield 1 or shield
/// 5, never a middle colour, so none of them exercises a collision the two
/// candidate readings disagree about.
///
/// The row that matters is the second. *"The game always tries to give the
/// Knight yellow"* is true of four rows out of five and false of that one, and
/// it fails in a way no preference rule would produce: the Knight takes black
/// because black's candidate list reaches him second, and the **Baron** takes
/// red. `docs/rules.md` §7a.
#[test]
fn taking_a_middle_colour_moves_the_lords_and_not_only_their_colours() {
    let Some(dir) = asset_dir() else {
        l2_testkit::skip!("LORDS2_DIR not set - skipping");
    };
    let Some(exe) = read(&dir, "Lords2.exe") else {
        l2_testkit::skip!("Lords2.exe not present - skipping");
    };
    let u8_at = |va: u32| -> u8 {
        exe[va_to_offset(&exe, va).unwrap_or_else(|| panic!("{va:#010X} unmapped"))]
    };

    /// `Realms_AssignLords`, for one human on realm 1 and four AI lords.
    /// Returns `(shield, lord)` for realms 2 … 5.
    let assign = |human_shield: u8| -> Vec<(u8, u8)> {
        let mut shield_taken = [false; 6];
        shield_taken[human_shield as usize] = true;
        let mut lord_taken = [false; 8];
        let mut out = Vec::new();
        for _realm in 2..=5u8 {
            // The lowest shield nobody has taken.
            let shield = (1..=5u8).find(|s| !shield_taken[*s as usize]).expect("a free shield");
            shield_taken[shield as usize] = true;
            // Then the lord, from that shield. England is slot 0 -> group 0.
            let base = 0x004D_C17C + u32::from(shield) * 4;
            let lord = (0..4)
                .map(|n| u8_at(base + n))
                .find(|&c| c != 0 && !lord_taken[c as usize & 7])
                .unwrap_or(0);
            lord_taken[lord as usize & 7] = true;
            out.push((shield, lord));
        }
        out
    };

    const KNIGHT: u8 = 1;
    const BARON: u8 = 2;
    const COUNTESS: u8 = 3;
    const BISHOP: u8 = 4;

    // The default, and the one arrangement a fixture can confirm: it is
    // what `england-turn1.sav` holds.
    assert_eq!(
        assign(1),
        vec![(2, KNIGHT), (3, BARON), (4, BISHOP), (5, COUNTESS)],
        "taking red gives the arrangement every default game shows"
    );

// **Take yellow and the Knight does not keep it, and does not shift
    // one along.** He becomes the black lord; the Baron becomes the red one.
    assert_eq!(
        assign(2),
        vec![(1, BARON), (3, KNIGHT), (4, BISHOP), (5, COUNTESS)],
        "taking yellow makes the Baron red and the Knight black"
    );

    assert_eq!(assign(3), vec![(1, BARON), (2, KNIGHT), (4, BISHOP), (5, COUNTESS)]);
    assert_eq!(
        assign(4),
        vec![(1, BARON), (2, KNIGHT), (3, COUNTESS), (5, BISHOP)],
        "taking magenta moves the Countess to black and the Bishop to blue"
    );
    assert_eq!(assign(5), vec![(1, BARON), (2, KNIGHT), (3, COUNTESS), (4, BISHOP)]);

    // The description, stated as the count that makes it a description: the
    // Knight has yellow in four of the five, and never in the one where the
    // person took it.
    let knight_yellow = (1..=5u8)
        .filter(|&h| assign(h).iter().any(|&(s, l)| l == KNIGHT && s == 2))
        .count();
    assert_eq!(knight_yellow, 4, "true four times in five, which is why it reads as a rule");
    assert!(
        !assign(2).iter().any(|&(s, l)| l == KNIGHT && s == 2),
        "and false exactly where the person took it"
    );
    eprintln!("lord colours: the Knight has yellow in {knight_yellow} of 5 openings");

}

