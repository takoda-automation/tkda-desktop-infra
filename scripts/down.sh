#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
ENV_FILE="${TKDA_DESKTOP_ENV:-$STATE/env}"

if [[ -f "$ROOT/scripts/cloudflare-tunnel.sh" ]]; then
  bash "$ROOT/scripts/cloudflare-tunnel.sh" stop || true
fi

if [[ -e "$ENV_FILE" ]]; then
  [[ -f "$ENV_FILE" && ! -L "$ENV_FILE" ]] || {
    echo "unsafe desktop env file: $ENV_FILE" >&2
    exit 1
  }
  # shellcheck disable=SC1090
  source "$ENV_FILE"
  if [[ -n "${SCINTILLA_DESKTOP_INFRA_ROOT:-}" ]] && [[ -x "$SCINTILLA_DESKTOP_INFRA_ROOT/scripts/down.sh" ]]; then
    export SCINTILLA_DESKTOP_STATE="${SCINTILLA_DESKTOP_STATE:-$STATE/scintilla}"
    "$SCINTILLA_DESKTOP_INFRA_ROOT/scripts/down.sh" || true
  fi
fi

echo "Takoda desktop appliance stopped"
