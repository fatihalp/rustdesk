#!/usr/bin/env bash
# Brands the base Windows client, Authenticode-signs it and zips it. Output: work/out/<name>.zip
set -euo pipefail
: "${WINDOWS_CERT_PFX:?}" "${WINDOWS_CERT_PASSWORD:?}"
cd work
name="$(cat app_name.txt)"
file_name="$(printf '%s' "$name" | tr -cd 'A-Za-z0-9 ._-')"
[ -n "$file_name" ] || file_name="Support"

mkdir -p out/pkg
unzip -q client-base-windows.zip -d out/pkg
exe_src="$(find out/pkg -maxdepth 2 -iname 'rustdesk.exe' -print -quit)"
dir="$(dirname "$exe_src")"
exe="$dir/$file_name.exe"
mv "$exe_src" "$exe"
cp brand.json "$dir/brand.json"

curl -fsSL -o rcedit.exe https://github.com/electron/rcedit/releases/download/v2.0.0/rcedit-x64.exe
args=(--set-version-string ProductName "$name" --set-version-string FileDescription "$name" --set-version-string OriginalFilename "$file_name.exe")
if [ -f logo.png ]; then
  magick logo.png -define icon:auto-resize=256,128,64,48,32,16 brand.ico
  args+=(--set-icon brand.ico)
fi
./rcedit.exe "$exe" "${args[@]}"

echo "$WINDOWS_CERT_PFX" | base64 --decode > cert.pfx
signtool="$(find "/c/Program Files (x86)/Windows Kits/10/bin" -name signtool.exe -path '*x64*' | sort | tail -1)"
"$signtool" sign /f cert.pfx /p "$WINDOWS_CERT_PASSWORD" /fd sha256 /tr http://timestamp.digicert.com /td sha256 "$exe"
"$signtool" verify /pa "$exe"
rm -f cert.pfx

(cd out/pkg && 7z a -tzip "../$file_name.zip" . >/dev/null)
echo "out/$file_name.zip" > artifact.txt
echo true > signed.txt
