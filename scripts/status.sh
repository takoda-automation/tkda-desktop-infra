#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
ENV_FILE="${TKDA_DESKTOP_ENV:-$STATE/env}"
[[ -f "$ENV_FILE" && ! -L "$ENV_FILE" ]] || { echo "missing or unsafe $ENV_FILE" >&2; exit 1; }
# shellcheck disable=SC1090
source "$ENV_FILE"

if curl --fail --silent http://127.0.0.1:18088/healthz >/dev/null 2>&1; then
  echo "tkda-main-server desktop supervisor: healthy"
else
  echo "tkda-main-server desktop supervisor: unavailable"
fi

if [[ -n "${TKDA_DESKTOP_CLI_BIN:-}" ]] \
  && [[ -x "$TKDA_DESKTOP_CLI_BIN" ]] \
  && TKDA_LOCAL_CONTROL_TOKEN_FILE="$TKDA_LOCAL_CONTROL_TOKEN_FILE" \
    "$TKDA_DESKTOP_CLI_BIN" --command=status; then
  echo "tkda-desktop-daemon: healthy (Scintilla-managed)"
else
  echo "tkda-desktop-daemon: unavailable" >&2
fi

if [[ -n "${SCINTILLA_DESKTOP_INFRA_ROOT:-}" ]] && [[ -x "$SCINTILLA_DESKTOP_INFRA_ROOT/scripts/status.sh" ]]; then
  export SCINTILLA_DESKTOP_STATE="${SCINTILLA_DESKTOP_STATE:-$STATE/scintilla}"
  "$SCINTILLA_DESKTOP_INFRA_ROOT/scripts/status.sh" || true
fi

if [[ -n "${TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE:-}" ]] && [[ -f "$ROOT/scripts/cloudflare-tunnel.sh" ]]; then
  bash "$ROOT/scripts/cloudflare-tunnel.sh" status || true
fi
if [[ -n "${TKDA_CLOUDFLARE_PUBLIC_HOSTNAME:-}" ]]; then
  echo "takoda remote ingress: https://${TKDA_CLOUDFLARE_PUBLIC_HOSTNAME}"
fi
