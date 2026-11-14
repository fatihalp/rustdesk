# SupportDesk

A self-hosted remote support platform. Support teams manage customers, brand a remote-desktop client for each
of them, hand out download links and connect to the customers' devices, all over their own relay infrastructure.

## Components

| Directory | What it is |
|-----------|-----------|
| `app/`, `routes/`, `database/`, `config/` | Laravel 13 backend with a Filament admin panel (`/admin`), `RelayBridgeService` real-time Rust bridge, and REST APIs. |
| `resources/portal/` | React single-page portal (`/portal`) with a live WebSocket device dashboard, brand settings, build manager, and short links. |
| `client/` | Branded remote-desktop client (Flutter + Rust, RustDesk based). Reads `brand.json` for the app name, servers and key. |
| `RelayServer/` | Single-binary Rust rendezvous, relay, and real-time control/WebSocket server (`21115-21120`). See [RelayServer/README.md](RelayServer/README.md). |
| `.github/workflows/` | CI that packages branded client builds on request from the panel. |

## How the pieces fit together

```mermaid
flowchart LR
    Admin[Support team] -->|Filament / Portal| Panel[Laravel panel]
    Portal[React Portal] <-->|Live WebSocket 21120/ws| Relay[RelayServer]
    Panel <-->|HTTP Control API 21120| Relay
    Panel -->|dispatch workflow| CI[GitHub Actions]
    CI -->|branded build| Panel
    Customer[Customer] -->|short link /s/slug| Panel
    Customer -->|installs| Client[Branded client]
    Client -->|heartbeat, sysinfo| Panel
    Client <-->|register, connect| Relay
    Admin <-->|connect| Relay
```

## Real-Time Laravel <-> Rust Bridge

`RelayServer` exposes a lightweight HTTP + WebSocket Control server on port `21120` (`src/controllers/control.rs`):

- **Live Event Stream (`WS /ws`)**: Every UDP peer registration (`peer.online`), peer expiration (`peer.offline`), block/unblock state change (`peer.blocked` / `peer.unblocked`), and active relay stream (`session.started` / `session.ended`) is broadcast over a `tokio::sync::broadcast` channel directly to connected WebSocket clients (`resources/portal/components/DeviceList.jsx`) with zero polling overhead.
- **Snapshot & Control Commands (`GET /status`, `POST /peers/{id}/{disconnect,block,unblock}`)**: Laravel's `App\Services\RelayBridgeService` queries live online peers, active relay sessions, and blocked IDs directly from `RelayServer` memory, and sends immediate disconnect/block/unblock commands that terminate active Rust relay tasks via a `tokio::sync::watch` cancellation channel.

1. A support team creates a customer and a client app (name, logo, unattended access) in the panel.
2. The panel dispatches a GitHub workflow that packages the client with a generated `brand.json`
   (rendezvous server, relay server, public key, API URL) and uploads the artifact back.
3. The customer opens a short link (`/s/{slug}`) and downloads the installer.
4. The installed client reports to the panel API (`/api/heartbeat`, `/api/sysinfo`) and registers with RelayServer,
   which brokers direct connections or relays the session.

## Production install on Ubuntu Server 24.04

On a fresh server, get the code and the PHP CLI, then let the installer do the rest:

```bash
sudo apt-get update && sudo apt-get install -y git unzip php8.3-cli php8.3-xml php8.3-curl php8.3-mbstring php8.3-intl php8.3-zip php8.3-pgsql composer
sudo git clone https://github.com/fatihalp/rustdesk.git /var/www/supportdesk
cd /var/www/supportdesk
sudo composer install --no-dev --optimize-autoloader
sudo php artisan install:first
```

`install:first` asks for the server's domain (or IP), an email for Let's Encrypt and the first admin account. It accepts
`--domain`, `--email`, `--admin-email` and `--admin-password` for unattended runs, and it is safe to run again. It:

- installs nginx, PHP-FPM, PostgreSQL, Node.js, certbot, ufw and the Rust toolchain;
- creates the `supportdesk` database and user;
- builds RelayServer and runs it as the `relay-server` systemd service with a generated control secret;
- writes `.env` (production mode, database, relay host, relay public key, secrets), migrates, creates the admin user
  and builds the portal;
- serves the panel through nginx and proxies the portal's live events from `/relay/ws` to the relay;
- opens SSH, `80`, `443`, `21115-21119/tcp` and `21116/udp` in ufw, keeping the relay control port `21120` private;
- requests a Let's Encrypt certificate and redirects HTTP to HTTPS when an email is given and the server has a domain.

Point the domain's DNS record at the server before running it, otherwise the certificate request fails.

## Quick start

### Panel

```bash
composer install
cp .env.example .env && php artisan key:generate
php artisan migrate --seed
npm install && npm run build
php artisan serve
```

Set `PANEL_RENDEZVOUS_SERVER`, `PANEL_RELAY_SERVER` and `PANEL_RELAY_KEY` in `.env` to your RelayServer host and the
contents of its `id_ed25519.pub`. GitHub build dispatch needs `PANEL_GITHUB_REPOSITORY` and `PANEL_GITHUB_TOKEN`.

### RelayServer

```bash
cd RelayServer
cargo build --release
RELAY_HOST=127.0.0.1 DATA_DIR=data ./target/release/relay-server
```

For production use Docker (`docker compose up -d --build`) or the Ubuntu installer
(`sudo ./deploy/install-ubuntu.sh <host>`). Details are in the [RelayServer README](RelayServer/README.md).

### Client

See `client/` for the upstream build documentation. Place the `brand.json` generated by the panel next to the
executable (or let the panel build and sign the package for you).
