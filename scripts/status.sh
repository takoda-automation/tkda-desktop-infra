#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
ENV_FILE="${TKDA_DESKTOP_ENV:-$STATE/env}"
test -f "$ENV_FILE" || { echo "missing $ENV_FILE" >&2; exit 1; }
# shellcheck disable=SC1090
source "$ENV_FILE"

if [[ -f "$STATE/tkda-daemon.pid" ]] && kill -0 "$(cat "$STATE/tkda-daemon.pid")" 2>/dev/null; then
  echo "tkda-desktop-daemon: running pid=$(cat "$STATE/tkda-daemon.pid")"
else
  echo "tkda-desktop-daemon: stopped"
fi

if curl --fail --silent http://127.0.0.1:18088/healthz >/dev/null 2>&1; then
  echo "tkda-main-server desktop supervisor: healthy"
else
  echo "tkda-main-server desktop supervisor: unavailable"
fi

TOKEN="$(tr -d '\r\n' < "$TKDA_LOCAL_CONTROL_TOKEN_FILE")"
curl --fail --silent -H "Authorization: Bearer $TOKEN"   http://127.0.0.1:18087/v1/status
echo

if [[ -n "${SCINTILLA_DESKTOP_INFRA_ROOT:-}" ]] && [[ -x "$SCINTILLA_DESKTOP_INFRA_ROOT/scripts/status.sh" ]]; then
  export SCINTILLA_DESKTOP_STATE="${SCINTILLA_DESKTOP_STATE:-$STATE/scintilla}"
  "$SCINTILLA_DESKTOP_INFRA_ROOT/scripts/status.sh" || true
fi

if [[ "${TKDA_CLOUDFLARE_TUNNEL_ENABLED:-false}" == "true" ]]; then
  bash "$ROOT/scripts/cloudflare-tunnel.sh" status || true
else
  echo "cloudflared: disabled"
fi
