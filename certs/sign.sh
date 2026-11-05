#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="${1:-}"

if [[ -z "$TARGET" || ! -e "$TARGET" ]]; then
  echo "Usage: $0 <path-to-binary-or-app>"
  exit 1
fi

KEYCHAIN_FILE="$SCRIPT_DIR/visiosoft.keychain-db"
PASSWORD="visiosoft"
KEYCHAIN_ARGS=()

# 1. Check for Developer ID Application (Apple's official certificate for Gatekeeper & direct distribution)
DEV_ID=$(security find-identity -v -p codesigning 2>/dev/null | grep -E "Developer ID Application:" | head -n 1 | sed -E 's/.*"([^"]+)".*/\1/' || true)

if [[ -n "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  IDENTITY_NAME="$APPLE_SIGNING_IDENTITY"
  echo "==> Using explicit Apple signing identity: $IDENTITY_NAME"
elif [[ -n "$DEV_ID" ]]; then
  IDENTITY_NAME="$DEV_ID"
  echo "==> Detected Apple Developer ID: $IDENTITY_NAME"
else
  IDENTITY_NAME="VISIOSOFT TEKNOLOJI ANONIM SIRKETI"
  if [[ ! -f "$KEYCHAIN_FILE" ]]; then
    "$SCRIPT_DIR/generate-cert.sh"
  fi
  security unlock-keychain -p "$PASSWORD" "$KEYCHAIN_FILE" 2>/dev/null || true
  KEYCHAIN_ARGS=(--keychain "$KEYCHAIN_FILE")
  echo "==> Using Visiosoft Corporate Certificate: $IDENTITY_NAME"
fi

if [[ "$(uname -s)" == "Darwin" ]]; then
  ENTITLEMENTS="$SCRIPT_DIR/entitlements.plist"
  ENT_ARGS=()
  if [[ -f "$ENTITLEMENTS" ]]; then
    ENT_ARGS=(--entitlements "$ENTITLEMENTS")
  fi

  if [[ -d "$TARGET" && "$TARGET" == *.app ]]; then
    echo "==> Signing macOS application bundle: $TARGET"
    # Sign internal libraries and binaries first
    if [[ -d "$TARGET/Contents/MacOS" ]]; then
      for item in "$TARGET/Contents/MacOS"/*; do
        if [[ -f "$item" ]]; then
          echo "    - Signing component: $(basename "$item")"
          codesign ${KEYCHAIN_ARGS[@]+"${KEYCHAIN_ARGS[@]}"} --sign "$IDENTITY_NAME" --force --options runtime "${ENT_ARGS[@]}" "$item" >/dev/null 2>&1 || \
          codesign ${KEYCHAIN_ARGS[@]+"${KEYCHAIN_ARGS[@]}"} --sign "$IDENTITY_NAME" --force "$item" >/dev/null 2>&1 || true
        fi
      done
    fi
    if [[ -d "$TARGET/Contents/Frameworks" ]]; then
      find "$TARGET/Contents/Frameworks" -type f -exec codesign ${KEYCHAIN_ARGS[@]+"${KEYCHAIN_ARGS[@]}"} --sign "$IDENTITY_NAME" --force --options runtime {} + >/dev/null 2>&1 || true
    fi

    # Sign the overall app bundle
    codesign ${KEYCHAIN_ARGS[@]+"${KEYCHAIN_ARGS[@]}"} --sign "$IDENTITY_NAME" --force --deep --options runtime "${ENT_ARGS[@]}" "$TARGET"
    xattr -cr "$TARGET" 2>/dev/null || true
    codesign --verify --deep --strict --verbose=1 "$TARGET"
    echo "==> Successfully signed and verified $TARGET with $IDENTITY_NAME!"
  else
    echo "==> Signing binary/image: $TARGET"
    codesign ${KEYCHAIN_ARGS[@]+"${KEYCHAIN_ARGS[@]}"} --sign "$IDENTITY_NAME" --force --options runtime "${ENT_ARGS[@]}" "$TARGET" 2>/dev/null || \
    codesign ${KEYCHAIN_ARGS[@]+"${KEYCHAIN_ARGS[@]}"} --sign "$IDENTITY_NAME" --force "$TARGET"
    xattr -cr "$TARGET" 2>/dev/null || true
    echo "==> Successfully signed $TARGET with $IDENTITY_NAME!"
  fi

  # Apple Notary Service submission (if notary profile is configured)
  NOTARY_PROFILE="${NOTARY_PROFILE:-visiosoft-notary}"
  if [[ "$TARGET" == *.dmg || "$TARGET" == *.app ]] && xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" >/dev/null 2>&1; then
    echo "==> Submitting $TARGET to Apple Notary Service (profile: $NOTARY_PROFILE)..."
    if xcrun notarytool submit "$TARGET" --keychain-profile "$NOTARY_PROFILE" --wait; then
      echo "==> Apple Notarization successful! Stapling notary ticket..."
      xcrun stapler staple "$TARGET" 2>/dev/null || true
      echo "==> Stapling complete! Gatekeeper will trust this artifact on all Macs."
    else
      echo "==> Warning: Apple Notary submission did not finish successfully."
    fi
  fi
else
  echo "Notice: Non-Darwin platform detected. Certificate is ready at $SCRIPT_DIR/visiosoft.p12"
fi
