#!/usr/bin/env bash
# Downloads the brand manifest, logo and unbranded base client into ./work.
set -euo pipefail
platform="$1"
mkdir -p work
curl -fsS -H "Authorization: Bearer $PANEL_BUILD_SECRET" "$APP_URL/api/builds/$BUILD_ID/manifest" -o work/response.json
python3 - <<'PY'
import json
r = json.load(open("work/response.json"))
json.dump(r["manifest"], open("work/brand.json", "w"), indent=2)
open("work/app_name.txt", "w").write(r["manifest"]["app_name"])
open("work/logo_url.txt", "w").write(r.get("logo_url") or "")
PY
logo="$(cat work/logo_url.txt)"
if [ -n "$logo" ]; then curl -fsSL "$logo" -o work/logo.png; fi
gh release download "$BASE_RELEASE" --repo "$GITHUB_REPOSITORY" --pattern "client-base-$platform.zip" --dir work --clobber
