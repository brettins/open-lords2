//! `l2_net::trace`, the sink: the record line `FORMAT.md` fixes, one `rand` a frame. CNEW-trace-sink.

use l2_net::rng::Pcg32;
use l2_net::trace::{self, Arg};

#[test]
fn a_record_is_one_json_line_in_the_shared_shape() {
    trace::start();
    trace::set_frame(7);
    l2_net::trace!("sound", 0x00427990, "Sound_PlayFile", file = "S011_01.wav", speech = true);
    let r = trace::stop();
    assert_eq!(
        r[0].to_json(),
        r#"{"frame":7,"kind":"sound","addr":"0x00427990","name":"Sound_PlayFile","caller":null,"args":{"file":"S011_01.wav","speech":1}}"#
    );
}

#[test]
fn nothing_is_kept_until_the_sink_starts() {
    let _ = trace::stop();
    l2_net::trace!("film", 0x0042D91B, "Smk_Play", file = "intro.smk");
    assert!(trace::stop().is_empty());
}

#[test]
fn a_watched_global_is_traced_on_change_only() {
    trace::start();
    for v in [0x1F, 0x1F, 0x22, 0x22, 0x1F] {
        l2_net::trace_changed!(v; "screen", 0x004EAC50, "g_screenId", id = v);
    }
    let ids: Vec<_> = trace::stop().into_iter().map(|r| r.args[0].1.clone()).collect();
    assert_eq!(ids, [Arg::I(0x1F), Arg::I(0x22), Arg::I(0x1F)]);
}

#[test]
fn the_draws_of_a_frame_are_one_rand_record() {
    trace::start();
    trace::set_frame(1);
    let mut rng = Pcg32::from_seed(1);
    (0..5).for_each(|_| _ = rng.next_u32());
    trace::set_frame(2);
    trace::set_frame(3);
    let r = trace::stop();
    assert_eq!(r.len(), 1, "a frame with no draw writes nothing");
    assert_eq!((r[0].frame, r[0].kind, &r[0].args[0].1), (1, "rand", &Arg::I(5)));
}
