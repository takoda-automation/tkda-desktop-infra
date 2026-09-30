#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
ENV_FILE="${TKDA_DESKTOP_ENV:-$STATE/env}"
[[ -f "$ENV_FILE" && ! -L "$ENV_FILE" ]] || { echo "missing or unsafe $ENV_FILE; run scripts/bootstrap.sh or create the appliance env" >&2; exit 1; }
# shellcheck disable=SC1090
source "$ENV_FILE"

: "${SCINTILLA_DESKTOP_INFRA_ROOT:?SCINTILLA_DESKTOP_INFRA_ROOT is required}"
: "${TKDA_SCINTILLA_RUNTIME_MANIFEST:?TKDA_SCINTILLA_RUNTIME_MANIFEST is required}"
: "${TKDA_LOCAL_CONTROL_TOKEN_FILE:?TKDA_LOCAL_CONTROL_TOKEN_FILE is required}"
: "${TKDA_DESKTOP_CLI_BIN:?TKDA_DESKTOP_CLI_BIN is required}"

test -x "$SCINTILLA_DESKTOP_INFRA_ROOT/scripts/up.sh" || {
  echo "Scintilla desktop infra is not runnable at $SCINTILLA_DESKTOP_INFRA_ROOT" >&2
  exit 1
}
test -f "$TKDA_SCINTILLA_RUNTIME_MANIFEST" || {
  echo "missing Scintilla runtime manifest: $TKDA_SCINTILLA_RUNTIME_MANIFEST" >&2
  exit 1
}

mkdir -p "$STATE/logs" "$STATE/scintilla"
export SCINTILLA_DESKTOP_STATE="${SCINTILLA_DESKTOP_STATE:-$STATE/scintilla}"
export SCINTILLA_DESKTOP_MANIFEST="$TKDA_SCINTILLA_RUNTIME_MANIFEST"

"$SCINTILLA_DESKTOP_INFRA_ROOT/scripts/up.sh" >"$STATE/logs/scintilla-up.log" 2>&1

for _ in {1..100}; do
  if curl --fail --silent http://127.0.0.1:18088/healthz >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done
curl --fail --silent http://127.0.0.1:18088/healthz >/dev/null || {
  echo "Takoda main supervisor did not become healthy on 127.0.0.1:18088" >&2
  exit 1
}

for _ in {1..100}; do
  if TKDA_LOCAL_CONTROL_TOKEN_FILE="$TKDA_LOCAL_CONTROL_TOKEN_FILE" \
      "$TKDA_DESKTOP_CLI_BIN" --command=status >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done

TKDA_LOCAL_CONTROL_TOKEN_FILE="$TKDA_LOCAL_CONTROL_TOKEN_FILE" \
  "$TKDA_DESKTOP_CLI_BIN" --command=status

if [[ "${TKDA_CLOUDFLARE_AUTO_START:-false}" == "true" ]]; then
  bash "$ROOT/scripts/cloudflare-tunnel.sh" start
fi
