#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
ENV_FILE="${TKDA_DESKTOP_ENV:-$STATE/env}"

if [[ "${TKDA_CLOUDFLARE_TUNNEL_ENABLED:-false}" == "true" || -f "$STATE/cloudflared.pid" ]]; then
  bash "$ROOT/scripts/cloudflare-tunnel.sh" stop || true
fi

if [[ -f "$STATE/tkda-daemon.pid" ]]; then
  pid="$(cat "$STATE/tkda-daemon.pid")"
  kill "$pid" 2>/dev/null || true
  for _ in {1..30}; do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.1
  done
  kill -9 "$pid" 2>/dev/null || true
  rm -f "$STATE/tkda-daemon.pid"
fi

if [[ -f "$ENV_FILE" ]]; then
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  if [[ -n "${SCINTILLA_DESKTOP_INFRA_ROOT:-}" ]] && [[ -x "$SCINTILLA_DESKTOP_INFRA_ROOT/scripts/down.sh" ]]; then
    export SCINTILLA_DESKTOP_STATE="${SCINTILLA_DESKTOP_STATE:-$STATE/scintilla}"
    "$SCINTILLA_DESKTOP_INFRA_ROOT/scripts/down.sh" || true
  fi
fi

echo "Takoda desktop appliance stopped"
