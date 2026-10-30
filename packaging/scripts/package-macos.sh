#!/usr/bin/env bash
# Brands the base .app, then signs, notarizes and packs it into a dmg. Output: work/out/<name>.dmg
set -euo pipefail
: "${MACOS_CERT_P12:?}" "${MACOS_CERT_PASSWORD:?}" "${MACOS_SIGN_IDENTITY:?}" "${APPLE_ID:?}" "${APPLE_TEAM_ID:?}" "${APPLE_APP_PASSWORD:?}"
cd work
name="$(cat app_name.txt)"
file_name="$(printf '%s' "$name" | tr -cd 'A-Za-z0-9 ._-')"
[ -n "$file_name" ] || file_name="Support"

unzip -q client-base-macos.zip -d base
src="$(find base -maxdepth 2 -name '*.app' -print -quit)"
mkdir -p out
app="out/$file_name.app"
cp -R "$src" "$app"

plist="$app/Contents/Info.plist"
/usr/libexec/PlistBuddy -c "Set :CFBundleName $name" "$plist"
/usr/libexec/PlistBuddy -c "Add :CFBundleDisplayName string $name" "$plist" 2>/dev/null \
  || /usr/libexec/PlistBuddy -c "Set :CFBundleDisplayName $name" "$plist"
cp brand.json "$app/Contents/Resources/brand.json"

if [ -f logo.png ]; then
  icons=brand.iconset
  mkdir "$icons"
  for s in 16 32 128 256 512; do
    sips -z $s $s logo.png --out "$icons/icon_${s}x${s}.png" >/dev/null
    sips -z $((s * 2)) $((s * 2)) logo.png --out "$icons/icon_${s}x${s}@2x.png" >/dev/null
  done
  icon_file="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIconFile' "$plist" 2>/dev/null || echo AppIcon)"
  iconutil -c icns "$icons" -o "$app/Contents/Resources/${icon_file%.icns}.icns"
fi

keychain="$RUNNER_TEMP/build.keychain-db"
security create-keychain -p ci "$keychain"
security set-keychain-settings -lut 3600 "$keychain"
security unlock-keychain -p ci "$keychain"
echo "$MACOS_CERT_P12" | base64 --decode > "$RUNNER_TEMP/cert.p12"
security import "$RUNNER_TEMP/cert.p12" -k "$keychain" -P "$MACOS_CERT_PASSWORD" -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple: -s -k ci "$keychain" >/dev/null
security list-keychains -d user -s "$keychain" login.keychain-db

codesign --force --deep --options runtime --timestamp --sign "$MACOS_SIGN_IDENTITY" "$app"
codesign --verify --deep --strict "$app"

dmg="out/$file_name.dmg"
hdiutil create -volname "$name" -srcfolder "$app" -ov -format UDZO "$dmg"
codesign --force --timestamp --sign "$MACOS_SIGN_IDENTITY" "$dmg"
xcrun notarytool submit "$dmg" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_PASSWORD" --wait
xcrun stapler staple "$dmg"
echo "$dmg" > artifact.txt
echo true > signed.txt
