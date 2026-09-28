#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
BROWSER_ENV="${TKDA_BROWSER_MCP_ENV:-$STATE/browser-mcp.env}"

[[ -f "$BROWSER_ENV" ]] && source "$BROWSER_ENV"

check_pid() {
  local name="$1"
  local file="$STATE/run/$name.pid"
  if [[ -f "$file" ]] && kill -0 "$(cat "$file")" 2>/dev/null; then
    echo "$name: running pid=$(cat "$file")"
  else
    echo "$name: stopped"
  fi
}

check_pid browser-mcp-adapter
check_pid browser-mcp-gateway
check_pid cloudflared

curl --fail --silent http://127.0.0.1:18090/agent/healthz && echo
curl --fail --silent http://127.0.0.1:8092/healthz && echo

if [[ -n "${TKDA_BROWSER_MCP_HOSTNAME:-}" ]]; then
  curl --fail --silent --max-time 10 "https://$TKDA_BROWSER_MCP_HOSTNAME/healthz" && echo
fi
