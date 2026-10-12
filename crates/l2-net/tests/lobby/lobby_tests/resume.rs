//! C263: a saved game continued in the room.

use super::*;
use l2_net::SavedRealm;

const RICHARD: [u8; 16] = [0xA1; 16];
const MATILDA: [u8; 16] = [0xB2; 16];

fn saved() -> Vec<SavedRealm> {
    let realm = |realm, name: &str, owner| SavedRealm { realm, name: name.into(), shield: realm, owner };
    vec![realm(1, "Richard", RICHARD), realm(2, "Matilda", MATILDA), realm(3, "Baron", [0; 16])]
}

fn table() -> Table {
    let host = Lobby::host(hello(0), "Richard").unwrap().with_install(RICHARD).with_save(saved(), vec![7; 64]);
    Table { host, clients: Vec::new() }
}

fn arrive(t: &mut Table, peer: u32, name: &str, install: [u8; 16]) {
    let mut client = Lobby::join(hello(peer as u8), name).unwrap().with_install(install);
    client.greet().unwrap();
    let out = client.next_outgoing().unwrap();
    t.host.receive(PeerId(peer), out.message).unwrap();
    t.clients.push((PeerId(peer), client));
    t.pump();
}

fn realms(roster: &Roster) -> Vec<u8> {
    roster.players.iter().map(|p| p.realm).collect()
}

/// Ablations: `self.claim` in `on_join` (Matilda arrives unseated); the
/// `holder(..).is_none()` test in `claim` (a second install could take a held realm).
#[test]
fn each_player_is_seated_on_the_realm_its_install_played() {
    let mut t = table();
    assert_eq!(realms(t.host.roster()), vec![1], "the host claims its own realm");
    arrive(&mut t, 1, "Matilda", MATILDA);
    assert_eq!(realms(t.host.roster()), vec![1, 2]);
    assert_eq!(realms(t.client(0).roster()), vec![1, 2], "the room travels with the roster");
    assert_eq!(t.client(0).roster().saved, saved());
    arrive(&mut t, 2, "John", [0xC3; 16]);
    assert_eq!(realms(t.host.roster()), vec![1, 2, 0], "an unknown install waits for the host");
}

/// Ablations: the `unseated` refusal in `start` (John starts with no realm); the
/// `p.realm = 0` arm in `assign` (two players on realm 3).
#[test]
fn the_host_seats_the_rest_and_start_carries_the_save() {
    let mut t = table();
    arrive(&mut t, 1, "Matilda", MATILDA);
    arrive(&mut t, 2, "John", [0xC3; 16]);
    for (_, c) in t.clients.iter_mut() {
        c.set_ready(true).unwrap();
    }
    t.pump();
    assert_eq!(t.host.start(), Err(LobbyError::Unseated("John".into())));
    t.host.assign(PlayerSlot::new(2), 3).unwrap();
    assert_eq!(t.host.assign(PlayerSlot::new(2), 4), Err(LobbyError::NoRealm(4)));
    t.host.assign(PlayerSlot::new(1), 3).unwrap();
    assert_eq!(realms(t.host.roster()), vec![1, 3, 0], "John is unseated by Matilda");
    t.host.assign(PlayerSlot::new(2), 2).unwrap();
    let start = t.host.start().expect("everyone is seated");
    assert_eq!(start.save, vec![7; 64]);
    assert_eq!(realms(&start.roster), vec![1, 3, 2]);
    t.pump();
    assert_eq!(t.client(1).started(), Some(&start));
}
