//! **Which build is this?** — the one question the player could not answer.
//!
//! Three of the last five interface defects reported were against a binary four
//! merges old. Two of them had already been fixed; one had been fixed twice.
//! Every minute spent re-reading a function to explain a bug that no longer
//! existed was spent because the picture on the screen carried no identity.
//!
//! So the title screen carries the short commit, a `-DIRTY` marker and the
//! commit's date, in the bottom-left corner in the dim ink, where it is legible
//! if you look for it and invisible if you do not. `build.rs` computes it and
//! documents what the marker can and cannot promise.
//!
//! **This is not a version number and must not become one.** A version says
//! what we *intended* to ship; this says what is actually running, which is the
//! only thing a bug report needs. `0.1.0` would have been true of every build
//! for two months.
//!
//! **It reaches no simulation.** A string baked in at compile time cannot vary
//! between two peers running the same binary, which is the only property
//! `docs/netcode.md` asks of anything drawn — but it is also never read by
//! anything but the painter below, so the question does not arise.

use l2_view::Canvas;

use crate::shell::Pen;

/// `env!`, not `option_env!`: if the build script did not run, that is a broken
/// build and should not compile into a game that silently has no stamp.
/// `build.rs` never fails — it falls back to `NO GIT` — so this is always set.
pub const ID: &str = env!("L2_BUILD_ID");

/// Where it sits: hard against the bottom-left, below everything any screen
/// draws. The setup pages centre their window at `y = 0x14 … 0xF` rows, and
/// the conquest and menu screens end at `y = 440`.
///
/// **The y is computed, not written.** It was the literal 468, chosen against
/// a 480-line canvas and the 7-pixel debug font — and once the real
/// `Fntl2_14.pl8` was drawing it, the stamp hung off the bottom of the screen
/// and a player reported it as *"half obscured by the bottom of the screen"*.
/// The test meant to protect it asserted the stamp was **painted**, by drawing
/// it twice and requiring the two canvases to be identical — which is equally
/// true of text painted entirely out of view. *"Is it drawn"* and *"can it be
/// seen"* are different claims, and only the second is what anybody wanted.
const X: i32 = 4;
/// The gap left below the stamp. The top edge is derived from the font
/// actually in use, so a taller font moves the line **up** rather than off.
const BOTTOM_MARGIN: i32 = 2;

/// One line, in the dim ink, over whatever is already there.
///
/// Drawn last so it survives a full-screen background; drawn with
/// [`Pen::flat`] so it takes no emboss and reads as a caption rather than as
/// part of the game's own furniture.
pub fn draw(canvas: &mut Canvas, pen: &Pen) {
    let s = format!("BUILD {ID}");
    pen.flat().body(canvas, X, top_edge(pen, &s), &s, pen.ink.dim);
}

/// The top edge that puts the stamp's **last row** at
/// `canvas::HEIGHT - BOTTOM_MARGIN`, measured with whichever font will draw it.
///
/// The fallback 5 × 7 font is 7 tall and `Fntl2_14.pl8` is taller, so a
/// literal that fits one clips the other — which is exactly what shipped.
/// Reading the height off the font that is about to be used is the shape that
/// cannot be wrong; a constant here is a claim about an asset that may not
/// even be loaded.
pub fn top_edge(pen: &Pen, s: &str) -> i32 {
    let h = match &pen.assets.body {
        Some(f) => f.height(s),
        None => l2_view::text::GLYPH_H,
    };
    l2_view::canvas::HEIGHT as i32 - BOTTOM_MARGIN - h
}
