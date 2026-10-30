#!/usr/bin/env bash
set -euo pipefail
cd work
curl -fsS -H "Authorization: Bearer $PANEL_BUILD_SECRET" \
  -F "file=@$(cat artifact.txt)" \
  -F "signed=$([ "$(cat signed.txt)" = true ] && echo 1 || echo 0)" \
  -F "version=${CLIENT_VERSION:-}" \
  -F "github_run_url=$RUN_URL" \
  "$APP_URL/api/builds/$BUILD_ID/artifact"
