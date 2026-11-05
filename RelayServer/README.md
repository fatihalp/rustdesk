# RelayServer

A single-binary rendezvous and relay server for RustDesk-protocol clients, written in Rust.
It replaces the upstream `hbbs` + `hbbr` pair with one small process (about 1,500 lines) and supports
Ubuntu and Docker only.

## What it does

| Role | Ports | Purpose |
|------|-------|---------|
| Rendezvous | `21116/udp` | Peers register their id and public key and send keep-alives. |
| Rendezvous | `21116/tcp`, `21118/tcp` (WebSocket) | A client asks to reach a peer; the server brokers a direct connection (NAT punching) or a relayed one. |
| NAT test / online status | `21115/tcp` | NAT type detection and online-state queries. |
| Relay | `21117/tcp`, `21119/tcp` (WebSocket) | Pairs the two sides of a session by uuid and forwards their traffic. |
| Control & Live Events | `21120/tcp` (HTTP + WebSocket) | Real-time JSON event stream (`/ws`), live status snapshot (`GET /status`), and peer control (`POST /peers/{id}/{block,unblock,disconnect}`) for Laravel & Portal. |

The server keeps an Ed25519 key pair in `DATA_DIR` (`id_ed25519`, `id_ed25519.pub`) and signs the
`(id, public key)` pair of every peer it hands out. Give the contents of `id_ed25519.pub` to your clients.

## Architecture

The code follows MVC, SOLID, and KISS principles:

```
src/
├── main.rs                  entry point
├── config.rs                settings read from environment variables
├── server.rs                sockets, accept loops and per-connection sessions (the router)
├── controllers/
│   ├── rendezvous.rs        decides how to answer registration and connection requests
│   ├── relay.rs             pairs relay sessions, tracks active streams and cancels on disconnect
│   └── control.rs           HTTP + WebSocket control plane for Laravel and React Portal
├── models/
│   ├── peer.rs              Peer, PeerStore, blocklist and real-time broadcast event bus
│   ├── repository.rs        SQLite persistence of peers
│   ├── keys.rs              server key pair and signing
│   └── ip_guard.rs          per-IP rate limiting of key registration
├── views/messages.rs        builders for every protocol message sent to clients
├── net/                     framing codec, TCP/WebSocket connection, address encoding, socket binding
└── proto.rs                 generated protobuf types (see proto/)
```

Controllers are synchronous: they take a decoded request and return a `Reply` (who gets which message).
Only `server.rs` performs I/O, which keeps each layer small and single-purpose.

## Configuration

All settings are environment variables:

| Variable | Default | Description |
|----------|---------|-------------|
| `RELAY_HOST` | empty | Public host (or `host:port`) clients use to reach the relay port. |
| `PORT` | `21116` | Rendezvous port. NAT test is `PORT-1`, WebSocket is `PORT+2`. |
| `RELAY_PORT` | `21117` | Relay port. Relay WebSocket is `RELAY_PORT+2`. |
| `BIND` | all interfaces | Address to bind to. |
| `ENCRYPTED_ONLY` | `N` | `Y` rejects clients that do not present the server's public key. |
| `ALWAYS_USE_RELAY` | `N` | `Y` skips NAT punching and relays every session. |
| `TRUST_PROXY_HEADERS` | `N` | `Y` trusts `X-Real-IP` / `X-Forwarded-For` on WebSocket ports; enable only behind a reverse proxy. |
| `DATA_DIR` | `data` | Directory for the key pair and `db_v2.sqlite3`. |
| `RUST_LOG` | `info` | Log level. |

## Run with Docker

```bash
cp .env.example .env     # set RELAY_HOST
docker compose up -d --build
docker compose logs relay | grep "Public key"
```

Open `21115-21119/tcp` and `21116/udp` in your firewall.

## Install on Ubuntu

```bash
sudo ./deploy/install-ubuntu.sh relay.example.com
```

The script installs the Rust toolchain if needed, builds the release binary, creates the `relay` system user,
writes `/etc/relay-server.env` and enables the `relay-server` systemd service.
Check it with `systemctl status relay-server` and `journalctl -u relay-server -f`.

## Build and run locally

```bash
cargo build --release
RELAY_HOST=127.0.0.1 DATA_DIR=data ./target/release/relay-server
```

Requires a stable Rust toolchain; `protoc` is not needed.

## Differences from upstream hbbs/hbbr

Removed to keep the server simple: the runtime console, bandwidth throttling and blacklists, LAN mask detection,
multiple relay servers, the legacy configuration push, the self-update check, CLI flags and the `.env` file parser.
Configuration is environment-only and the two upstream binaries are one process.
