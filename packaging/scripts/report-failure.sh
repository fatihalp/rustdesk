#!/usr/bin/env bash
curl -sS -H "Authorization: Bearer $PANEL_BUILD_SECRET" -H "Accept: application/json" \
  --data-urlencode "status=failed" \
  --data-urlencode "error=Workflow failed, see the run log" \
  --data-urlencode "github_run_url=$RUN_URL" \
  "$APP_URL/api/builds/$BUILD_ID/status" || true
