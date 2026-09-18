//! The `batfield.pl8` playlist — `PlayerStart_Shuffle` (`0x00497E65`) deals
//! `DAT_0057CAE0`, `Battlefield_BuildRandom` (`0x0047AAA3`) walks it by
//! `DAT_005653F8`.

use l2_kingdom::field_playlist::{FieldPlaylist, PLAYLIST_LEN, WALK_LEN};
use l2_kingdom::Kingdom;

/// Each source index is written to at most one slot and breaks, so **no frame
/// can appear twice**. It is not a permutation: `0` is both frame 0 and the
/// free marker, so a slot written with source 0 stays eligible and a later
/// source can take it, and a source whose 50 probes all land on taken slots is
/// dropped. That is the original's scatter, not ours.
#[test]
fn the_deal_places_each_frame_at_most_once() {
    for seed in [1u64, 0x5EED, 0xDEAD_BEEF, 7, 1996] {
        let list = FieldPlaylist::deal(seed);
        let mut seen = [0u32; PLAYLIST_LEN];
        for &f in list.frames() {
            assert!((f as usize) < PLAYLIST_LEN, "seed {seed}: frame {f} is off the table");
            seen[f as usize] += 1;
        }
        for frame in 1..PLAYLIST_LEN {
            assert!(seen[frame] <= 1, "seed {seed}: frame {frame} dealt {} times", seen[frame]);
        }
        assert!(seen.iter().skip(1).sum::<u32>() >= 40, "seed {seed}: the deal placed almost none");
    }
}

/// `DAT_00553528 = playlist[cursor]; cursor = cursor + 1;` — two battles in one
/// game read slot 0 then slot 1, whatever their own seeds are.
#[test]
fn two_battles_take_consecutive_entries() {
    let mut list = FieldPlaylist::deal(0x5EED);
    let order = *list.frames();
    assert_eq!(list.cursor(), 0);
    assert_eq!(list.take_next(), order[0]);
    assert_eq!(list.cursor(), 1);
    assert_eq!(list.take_next(), order[1]);
    assert_eq!(list.cursor(), 2);
}

/// `if (0x2e < DAT_005653f8) { DAT_005653f8 = 0; }` — the walk is 47 long and
/// entry `0x2F` of the 48 dealt and net-synced is never read.
#[test]
fn the_cursor_wraps_one_short_of_the_table() {
    let mut list = FieldPlaylist::deal(3);
    let order = *list.frames();
    for i in 0..WALK_LEN {
        assert_eq!(list.take_next(), order[i], "entry {i}");
    }
    assert_eq!(list.cursor(), 0, "the walk wraps after {WALK_LEN} entries");
    assert_eq!(list.take_next(), order[0]);
    assert_eq!(WALK_LEN + 1, PLAYLIST_LEN, "one dealt entry is never walked");
}

/// **Ablation.** The frame is the playlist's, not the battle's: if
/// [`FieldPlaylist::take_next`] stopped advancing — or the deal were replaced
/// by `frame = seed % 48` — this fails, because the sequence two different
/// seeds produce would no longer differ while the *positions* stay the same.
#[test]
fn the_order_is_the_game_seed_and_the_position_is_the_cursor() {
    let mut a = FieldPlaylist::deal(11);
    let mut b = FieldPlaylist::deal(12);
    let (a_seq, b_seq): (Vec<u8>, Vec<u8>) =
        ((0..8).map(|_| a.take_next()).collect(), (0..8).map(|_| b.take_next()).collect());
    assert_ne!(a_seq, b_seq, "two games deal different orders");
    assert_eq!(a.cursor(), 8);
    assert_eq!(b.cursor(), 8);

    // And a second walk over the same deal repeats it exactly.
    let mut again = FieldPlaylist::deal(11);
    assert_eq!((0..8).map(|_| again.take_next()).collect::<Vec<u8>>(), a_seq);
}

/// `Game_NewGame` (`0x00497CED`) deals it; a fresh [`Kingdom`] is not left
/// holding the `.bss` zeros.
#[test]
fn a_new_game_deals_the_playlist() {
    let k = Kingdom::new(0x5EED);
    assert_ne!(k.campaign.field_playlist, FieldPlaylist::empty());
    assert_eq!(k.campaign.field_playlist, FieldPlaylist::deal(0x5EED));
    assert_eq!(k.campaign.field_playlist.cursor(), 0);
}
