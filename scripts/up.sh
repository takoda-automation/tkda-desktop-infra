#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
ENV_FILE="${TKDA_DESKTOP_ENV:-$STATE/env}"
test -f "$ENV_FILE" || { echo "missing $ENV_FILE; run scripts/bootstrap.sh or create the appliance env" >&2; exit 1; }
# shellcheck disable=SC1090
source "$ENV_FILE"

: "${SCINTILLA_DESKTOP_INFRA_ROOT:?SCINTILLA_DESKTOP_INFRA_ROOT is required}"
: "${TKDA_SCINTILLA_RUNTIME_MANIFEST:?TKDA_SCINTILLA_RUNTIME_MANIFEST is required}"
: "${TKDA_DESKTOP_DAEMON_BIN:?TKDA_DESKTOP_DAEMON_BIN is required}"
: "${TKDA_AGENT_URL:?TKDA_AGENT_URL is required}"
: "${TKDA_AGENT_ID:?TKDA_AGENT_ID is required}"
: "${TKDA_AGENT_TOKEN_FILE:?TKDA_AGENT_TOKEN_FILE is required}"
: "${TKDA_LOCAL_CONTROL_TOKEN_FILE:?TKDA_LOCAL_CONTROL_TOKEN_FILE is required}"

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

if [[ -f "$STATE/tkda-daemon.pid" ]] && kill -0 "$(cat "$STATE/tkda-daemon.pid")" 2>/dev/null; then
  :
else
  TKDA_LAUNCH_SUPERVISOR=false   TKDA_LOCAL_SUPERVISOR_URL=http://127.0.0.1:18088   TKDA_LOCAL_SUPERVISOR_BIND=127.0.0.1:18088   TKDA_LOCAL_CONTROL_BIND=127.0.0.1:18087   TKDA_ALLOW_HEADED="${TKDA_ALLOW_HEADED:-false}"   TKDA_AGENT_URL="$TKDA_AGENT_URL"   TKDA_AGENT_ID="$TKDA_AGENT_ID"   TKDA_AGENT_TOKEN_FILE="$TKDA_AGENT_TOKEN_FILE"   TKDA_LOCAL_CONTROL_TOKEN_FILE="$TKDA_LOCAL_CONTROL_TOKEN_FILE"   nohup "$TKDA_DESKTOP_DAEMON_BIN" >>"$STATE/logs/tkda-daemon.log" 2>&1 &
  echo $! >"$STATE/tkda-daemon.pid"
fi

TOKEN="$(tr -d '\r\n' < "$TKDA_LOCAL_CONTROL_TOKEN_FILE")"
for _ in {1..100}; do
  if curl --fail --silent -H "Authorization: Bearer $TOKEN"       http://127.0.0.1:18087/v1/status >/dev/null 2>&1; then
    break
  fi
  sleep 0.1
done

curl --fail --silent -H "Authorization: Bearer $TOKEN"   http://127.0.0.1:18087/v1/status
echo

if [[ "${TKDA_CLOUDFLARE_AUTO_START:-false}" == "true" ]]; then
  bash "$ROOT/scripts/cloudflare-tunnel.sh" start
fi
