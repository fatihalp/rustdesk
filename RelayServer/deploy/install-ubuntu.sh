#!/usr/bin/env bash
set -euo pipefail

RELAY_HOST="${1:?usage: install-ubuntu.sh <relay-host>}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

apt-get update
apt-get install -y build-essential curl

if ! command -v cargo >/dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    export PATH="$HOME/.cargo/bin:$PATH"
fi

(cd "$ROOT" && cargo build --release --locked)
install -m 0755 "$ROOT/target/release/relay-server" /usr/local/bin/relay-server

id relay >/dev/null 2>&1 || useradd --system --home /var/lib/relay-server --create-home --shell /usr/sbin/nologin relay

cat > /etc/relay-server.env <<ENV
RELAY_HOST=$RELAY_HOST
DATA_DIR=/var/lib/relay-server
ENCRYPTED_ONLY=N
ALWAYS_USE_RELAY=N
ENV

cat > /etc/systemd/system/relay-server.service <<'UNIT'
[Unit]
Description=Relay Server (rendezvous and relay)
After=network-online.target
Wants=network-online.target

[Service]
User=relay
EnvironmentFile=/etc/relay-server.env
ExecStart=/usr/local/bin/relay-server
Restart=always
RestartSec=3
LimitNOFILE=1000000
NoNewPrivileges=true
ProtectSystem=strict
ReadWritePaths=/var/lib/relay-server

[Install]
WantedBy=multi-user.target
UNIT

systemctl daemon-reload
systemctl enable --now relay-server
echo "Relay server started. Public key:"
sleep 2
cat /var/lib/relay-server/id_ed25519.pub
echo
