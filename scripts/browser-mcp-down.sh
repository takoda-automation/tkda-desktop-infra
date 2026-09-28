#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"

stop_owned_pid() {
  local name="$1"
  local needle="$2"
  local file="$STATE/run/$name.pid"
  [[ -f "$file" ]] || return 0

  local pid command_line
  pid="$(cat "$file" 2>/dev/null || true)"
  if [[ ! "$pid" =~ ^[0-9]+$ ]]; then
    echo "$name: invalid stale pid file; removing" >&2
    rm -f "$file"
    return 0
  fi

  if ! kill -0 "$pid" 2>/dev/null; then
    rm -f "$file"
    return 0
  fi

  command_line="$(ps -p "$pid" -o command= 2>/dev/null || true)"
  if [[ -z "$command_line" || "$command_line" != *"$needle"* ]]; then
    echo "$name: pid $pid is not owned by the browser MCP lane; refusing to signal it" >&2
    rm -f "$file"
    return 1
  fi

  kill "$pid"
  for _ in {1..30}; do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.1
  done
  if kill -0 "$pid" 2>/dev/null; then
    echo "$name: pid $pid did not stop cleanly" >&2
    return 1
  fi
  rm -f "$file"
}

stop_owned_pid cloudflared "cloudflared"
stop_owned_pid browser-mcp-gateway "browser-mcp"
stop_owned_pid browser-mcp-adapter "adapter.mjs"

echo "Takoda browser MCP lane stopped"
