//! `TcpTransport::add` (C254): a stream connected elsewhere joins the host's
//! transport exactly like one the listener accepted.

use super::*;

/// Red on ablation of `self.joined.push(id)` in `add`, or of its `next_id` step.
#[test]
fn an_added_stream_is_a_peer_like_an_accepted_one() {
    let (mut host, addr) = host();
    let _first = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    wait_for_peers(&mut host, 1);
    assert_eq!(host.take_joined(), vec![PeerId(0)]);

    let side = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let side_at = side.local_addr().unwrap();
    let mut far = TcpTransport::connect(side_at, PeerId(0)).expect("the far end");
    let (near, _) = side.accept().expect("the near end");
    let added = host.add(near).expect("adding");
    assert_eq!(added, PeerId(1), "numbered after the accepted one");
    assert_eq!(host.take_joined(), vec![PeerId(1)], "reported through take_joined");
    assert_eq!(host.peers(), vec![PeerId(0), PeerId(1)]);
    assert_eq!(host.nodelay(PeerId(1)), Some(true));

    far.send(PeerId(0), b"through the side door").expect("sending");
    assert_eq!(wait_for_message(&mut host), (PeerId(1), b"through the side door".to_vec()));
    host.send(PeerId(1), b"welcome").expect("sending back");
    assert_eq!(wait_for_message(&mut far), (PeerId(0), b"welcome".to_vec()));

    drop(far);
    assert_eq!(wait_for_departure(&mut host), vec![PeerId(1)]);
    let _third = TcpTransport::connect(addr, PeerId(0)).expect("connecting");
    wait_for_peers(&mut host, 2);
    assert_eq!(host.take_joined(), vec![PeerId(2)], "the next accepted one follows on");
}
