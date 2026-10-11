# l2-relay: the join-code relay

`docs/netcode.md` §7 *Join codes and the relay*, C254. A host asks the relay for a
six-letter code; a joiner types the code instead of an address; the relay pairs the two
connections and copies bytes until one side closes. One binary, standard library only,
no accounts, no persistence, nothing on disk.

## What it is, and is not

- It holds a table of live codes in memory. A code lives exactly as long as the host's
  control connection; restart the relay and every code is gone, which is fine, the lobby
  just shows none.
- It sees what the game sends: the lobby's `Hello`, the roster, every tick's commands and
  hashes, unencrypted (`docs/netcode.md` §8, *No encryption or authentication in v1*). It
  logs none of it; stdout gets one line per event: `code issued`, `join`, `pair`, `close`,
  with the code and the host's IP, never a token or a game byte.
- No matchmaking, no listing of open games, no chat. A code is given out of band, like an
  address was.
- Limits: 256 live codes, 4 joiners waiting on one code, a 64-byte handshake line, 64 KiB
  buffered per direction. Idle hosts are pinged every 20 s and dropped after 90 s of
  silence; a joiner whose host does not answer within 10 s is told so.

## The game's side

`l2-game --relay host:4714`, or `LORDS2_RELAY=host:4714`, or the build's
`l2_game::net::DEFAULT_RELAY`. A host's lobby then shows `Join code: ABCDEF`; a joiner
types the code in the address field, or starts with `--join ABCDEF`. With no relay
configured hosting shows no code and the game plays by address exactly as before.

## Running it

### Locally, for a LAN or a rehearsal

```
cargo run --release -p l2-relay -- --listen 127.0.0.1:4714
l2-game <install> --relay 127.0.0.1:4714 --host
l2-game <install> --relay 127.0.0.1:4714 --join ABCDEF
```

### On a VPS (any Linux with systemd)

```
cargo build --release -p l2-relay
scp target/release/l2-relay you@vps:/usr/local/bin/l2-relay
scp tools/relay/l2-relay.service you@vps:/etc/systemd/system/
ssh you@vps 'systemctl daemon-reload && systemctl enable --now l2-relay'
journalctl -u l2-relay -f
```

Open TCP 4714 inbound on the VPS firewall. The unit runs it as a dynamic user with no
home, no writable system and no devices beyond the pseudo ones; it needs none of them.

### On fly.io

The app is `open-lords2-relay` in `yyz` (Toronto), personal org; `tools/relay/fly.toml`
describes it: one raw TCP service on 4714 (no handlers), one `shared-cpu-1x` 256 MB machine
that stops when idle and starts on the first connection (allow a few seconds), built from
`tools/relay/Dockerfile` with the repo root as context. Deploy and redeploy from the repo root:

```
flyctl deploy --config tools/relay/fly.toml --ha=false --yes
flyctl logs --app open-lords2-relay
flyctl status --app open-lords2-relay
```

Raw TCP needs a dedicated IPv4 (`flyctl ips allocate-v4 --app open-lords2-relay --yes`,
$2/month; a v6 is free with `ips allocate-v6`). Players use the IPv4 literally,
`<v4>:4714`: the `open-lords2-relay.fly.dev` name resolves to the shared address, which
serves HTTP only. Deployed 2026-10-11: machine `897359a2724048`, IPv4 `37.16.14.84`, IPv6
`2a09:8280:1::1b2:214:0`; `l2_game::net::DEFAULT_RELAY` is `37.16.14.84:4714`. Smoke test
that day over the public address: `HOST 1` answered in 0.3 s, a joiner's code paired,
bytes crossed both ways, `JOIN 1 ZZZZZZ` got `NO unknown code`. One machine is more than enough: a five-seat game is a few kilobytes a
second through two threads.

### Docker, anywhere

```
docker build -f tools/relay/Dockerfile -t l2-relay .
docker run -d --restart unless-stopped -p 4714:4714 l2-relay
```

## Checking it

```
printf 'HOST 1\n' | nc host 4714        # CODE ABCDEF, then PING every 20 s
cargo test -p l2-relay                   # the protocol against an in-process relay
```
