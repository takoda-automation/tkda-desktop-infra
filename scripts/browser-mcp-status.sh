#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
BROWSER_ENV="${TKDA_BROWSER_MCP_ENV:-$STATE/browser-mcp.env}"

[[ -f "$BROWSER_ENV" ]] && source "$BROWSER_ENV"

check_pid() {
  local name="$1"
  local needle="$2"
  local file="$STATE/run/$name.pid"
  local pid command_line
  [[ -f "$file" ]] || { echo "$name: stopped"; return 0; }
  pid="$(cat "$file" 2>/dev/null || true)"
  if [[ ! "$pid" =~ ^[0-9]+$ ]] || ! kill -0 "$pid" 2>/dev/null; then
    echo "$name: stopped (stale pid file)"
    return 0
  fi
  command_line="$(ps -p "$pid" -o command= 2>/dev/null || true)"
  if [[ -z "$command_line" || "$command_line" != *"$needle"* ]]; then
    echo "$name: stopped (pid reused by another process)"
    return 0
  fi
  echo "$name: running pid=$pid"
}

check_pid browser-mcp-adapter "adapter.mjs"
check_pid browser-mcp-gateway "browser-mcp"
check_pid cloudflared "cloudflared"

curl --fail --silent --max-time 3 http://127.0.0.1:18090/agent/healthz && echo
curl --fail --silent --max-time 3 http://127.0.0.1:8092/healthz && echo

if [[ -n "${TKDA_BROWSER_MCP_HOSTNAME:-}" ]]; then
  curl --fail --silent --max-time 10 "https://$TKDA_BROWSER_MCP_HOSTNAME/healthz" && echo
fi
