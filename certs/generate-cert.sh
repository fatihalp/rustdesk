#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OPENSSL_CNF="$SCRIPT_DIR/openssl.cnf"
KEY_FILE="$SCRIPT_DIR/visiosoft.key"
CRT_FILE="$SCRIPT_DIR/visiosoft.crt"
P12_FILE="$SCRIPT_DIR/visiosoft.p12"
KEYCHAIN_FILE="$SCRIPT_DIR/visiosoft.keychain-db"
PASSWORD="visiosoft"
IDENTITY_NAME="VISIOSOFT TEKNOLOJI ANONIM SIRKETI"

echo "==> Ensuring VizioSoft Code Signing Certificate..."
echo "    Company: VİSİOSOFT TEKNOLOJİ ANONİM ŞİRKETİ"
echo "    Tax ID : 9251021443"
echo "    Address: YTÜ İkitelli Teknopark No: 103, Başakşehir / İstanbul"
echo "    Contact: alp@visiosoft.com.tr | 0(212) 909 56 76"

# 1. Generate private key and certificate
echo "==> Generating RSA 4096 key and 10-year X.509 certificate..."
openssl req -new -x509 -nodes -days 3650 \
  -config "$OPENSSL_CNF" \
  -keyout "$KEY_FILE" \
  -out "$CRT_FILE"

# 2. Generate PKCS#12 bundle
echo "==> Packaging into PKCS#12 bundle ($P12_FILE)..."
openssl pkcs12 -export \
  -out "$P12_FILE" \
  -inkey "$KEY_FILE" \
  -in "$CRT_FILE" \
  -password "pass:$PASSWORD" \
  -name "$IDENTITY_NAME"

# 3. On macOS, configure dedicated keychain for non-interactive automated codesign
if [[ "$(uname -s)" == "Darwin" ]]; then
  echo "==> Setting up macOS signing keychain ($KEYCHAIN_FILE)..."
  rm -f "$KEYCHAIN_FILE"
  security create-keychain -p "$PASSWORD" "$KEYCHAIN_FILE"
  security set-keychain-settings -lut 7200 "$KEYCHAIN_FILE"
  security unlock-keychain -p "$PASSWORD" "$KEYCHAIN_FILE"
  security import "$P12_FILE" -k "$KEYCHAIN_FILE" -P "$PASSWORD" -T /usr/bin/codesign -T /usr/bin/security
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$PASSWORD" "$KEYCHAIN_FILE" >/dev/null 2>&1 || true
fi

echo "==> VİSİOSOFT TEKNOLOJİ ANONİM ŞİRKETİ Code Signing Certificate is ready!"
