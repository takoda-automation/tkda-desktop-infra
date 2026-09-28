#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
ENV_FILE="${TKDA_DESKTOP_ENV:-$STATE/env}"
ACTION="${1:-status}"

if [[ -f "$ENV_FILE" ]]; then
  # shellcheck disable=SC1090
  source "$ENV_FILE"
fi

CLOUDFLARED_BIN="${TKDA_CLOUDFLARED_BIN:-cloudflared}"
TOKEN_FILE="${TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE:-}"
PID_FILE="$STATE/cloudflared.pid"
LOG_FILE="$STATE/logs/cloudflared.log"
METRICS_ADDR="${TKDA_CLOUDFLARE_METRICS_ADDR:-127.0.0.1:20241}"

validate_metrics_addr() {
  case "$METRICS_ADDR" in
    127.0.0.1:[0-9]*|localhost:[0-9]*|"[::1]":[0-9]*) ;;
    *)
      echo "TKDA_CLOUDFLARE_METRICS_ADDR must be a loopback host:port" >&2
      exit 1
      ;;
  esac
}

is_running() {
  [[ -f "$PID_FILE" ]] || return 1
  local pid
  pid="$(cat "$PID_FILE")"
  kill -0 "$pid" 2>/dev/null
}

require_token_file() {
  [[ -n "$TOKEN_FILE" ]] || {
    echo "TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE is required" >&2
    exit 1
  }
  [[ -f "$TOKEN_FILE" && ! -L "$TOKEN_FILE" ]] || {
    echo "tunnel token file must be an existing regular non-symlink file: $TOKEN_FILE" >&2
    exit 1
  }
  local size
  size="$(wc -c < "$TOKEN_FILE" | tr -d ' ')"
  if [[ "$size" -lt 32 || "$size" -gt 16384 ]]; then
    echo "tunnel token file has an invalid size" >&2
    exit 1
  fi
  if [[ "$(uname -s)" != "MINGW"* && "$(uname -s)" != "MSYS"* && "$(uname -s)" != "CYGWIN"* ]]; then
    local mode
    mode="$(stat -c '%a' "$TOKEN_FILE" 2>/dev/null || stat -f '%Lp' "$TOKEN_FILE" 2>/dev/null || true)"
    if [[ -z "$mode" || $((8#$mode & 077)) -ne 0 ]]; then
      echo "tunnel token file must not be readable or writable by group/other users" >&2
      exit 1
    fi
  fi
}

validate_metrics_addr

case "$ACTION" in
  start)
    command -v "$CLOUDFLARED_BIN" >/dev/null 2>&1 || {
      echo "missing cloudflared; install Cloudflare Tunnel first" >&2
      exit 1
    }
    require_token_file
    mkdir -p "$STATE/logs"
    chmod 0700 "$STATE" "$STATE/logs" 2>/dev/null || true

    if is_running; then
      echo "cloudflared: already running pid=$(cat "$PID_FILE")"
      exit 0
    fi

    rm -f "$PID_FILE"
    nohup "$CLOUDFLARED_BIN" tunnel       --no-autoupdate       --metrics "$METRICS_ADDR"       --loglevel info       run       --token-file "$TOKEN_FILE"       >>"$LOG_FILE" 2>&1 &
    echo $! >"$PID_FILE"

    for _ in {1..80}; do
      if ! is_running; then
        echo "cloudflared exited during startup; inspect $LOG_FILE" >&2
        rm -f "$PID_FILE"
        exit 1
      fi
      if curl --fail --silent "http://$METRICS_ADDR/ready" >/dev/null 2>&1; then
        echo "cloudflared: ready pid=$(cat "$PID_FILE")"
        exit 0
      fi
      sleep 0.25
    done

    echo "cloudflared process is running but readiness did not become healthy" >&2
    exit 1
    ;;

  stop)
    if is_running; then
      pid="$(cat "$PID_FILE")"
      kill "$pid" 2>/dev/null || true
      for _ in {1..50}; do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
      done
      kill -9 "$pid" 2>/dev/null || true
    fi
    rm -f "$PID_FILE"
    echo "cloudflared: stopped"
    ;;

  status)
    if is_running; then
      pid="$(cat "$PID_FILE")"
      if curl --fail --silent "http://$METRICS_ADDR/ready" >/dev/null 2>&1; then
        echo "cloudflared: healthy pid=$pid metrics=$METRICS_ADDR"
      else
        echo "cloudflared: running-not-ready pid=$pid metrics=$METRICS_ADDR"
        exit 2
      fi
    else
      echo "cloudflared: stopped"
      exit 1
    fi
    ;;

  restart)
    "$0" stop || true
    "$0" start
    ;;

  *)
    echo "usage: $0 {start|stop|status|restart}" >&2
    exit 64
    ;;
esac
