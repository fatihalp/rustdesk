#!/usr/bin/env bash
set -euo pipefail

flutter --version | grep -q "Flutter 3\.44\." || {
  echo "Flutter 3.44.x must be on PATH; found:" >&2
  flutter --version | grep "^Flutter" >&2 || true
  exit 1
}

bash .github/patches/apply_flutter_3.44_source_patches.sh

(cd flutter && flutter pub get)

QR_WEB="${PUB_CACHE:-$HOME/.pub-cache}/hosted/pub.dev/qr_code_scanner-1.0.1/lib/src/web/flutter_qr_web.dart"
if ! grep -qF "dart:ui_web" "$QR_WEB"; then
  sed -i.bak "s|import 'dart:ui' as ui;|import 'dart:ui' as ui; import 'dart:ui_web' as ui_web;|" "$QR_WEB"
  rm -f "$QR_WEB.bak"
fi
if grep -qF "ui.platformViewRegistry" "$QR_WEB"; then
  sed -i.bak "s|ui\.platformViewRegistry|ui_web.platformViewRegistry|g" "$QR_WEB"
  rm -f "$QR_WEB.bak"
fi

python3 flutter/web/fonts/sync_fonts.py

grep -qF "import 'dart:ui' as ui; import 'dart:ui_web' as ui_web;" "$QR_WEB"
grep -qF "ui_web.platformViewRegistry" "$QR_WEB"
grep -qF 'google_fonts: ^8.1.0' flutter/pubspec.yaml

echo "Flutter 3.44 web patches applied."
