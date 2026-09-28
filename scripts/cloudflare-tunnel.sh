#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
ENV_FILE="${TKDA_DESKTOP_ENV:-$STATE/env}"
PID_FILE="$STATE/cloudflared.pid"
LOG_FILE="$STATE/logs/cloudflared.log"

if [[ -f "$ENV_FILE" ]]; then
  # shellcheck disable=SC1090
  source "$ENV_FILE"
fi

cmd="${1:-status}"

need_cloudflared() {
  command -v cloudflared >/dev/null 2>&1 || {
    echo "cloudflared is required; install Cloudflare Tunnel before enabling remote browser ingress" >&2
    exit 1
  }
  cloudflared tunnel run --help 2>&1 | grep -q -- "--token-file" || {
    echo "cloudflared 2025.4.0+ is required because Takoda reads tunnel credentials from a file" >&2
    exit 1
  }
}

validate_token_file() {
  : "${TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE:?TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE is required}"
  [[ -f "$TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE" ]] || {
    echo "missing tunnel token file: $TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE" >&2
    exit 1
  }
  [[ ! -L "$TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE" ]] || {
    echo "refusing symlink tunnel token file" >&2
    exit 1
  }
}

daemon_ready() {
  : "${TKDA_LOCAL_CONTROL_TOKEN_FILE:?TKDA_LOCAL_CONTROL_TOKEN_FILE is required}"
  local token
  token="$(tr -d '\r\n' < "$TKDA_LOCAL_CONTROL_TOKEN_FILE")"
  curl --fail --silent --show-error     -H "Authorization: Bearer $token"     http://127.0.0.1:18087/v1/status >/dev/null
}

case "$cmd" in
  start)
    need_cloudflared
    validate_token_file
    mkdir -p "$STATE/logs"
    daemon_ready

    if [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
      echo "cloudflared: already running pid=$(cat "$PID_FILE")"
      exit 0
    fi

    nohup cloudflared tunnel       --no-autoupdate       --loglevel "${TKDA_CLOUDFLARE_LOGLEVEL:-info}"       --metrics "${TKDA_CLOUDFLARE_METRICS_BIND:-127.0.0.1:20241}"       run --token-file "$TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE"       >>"$LOG_FILE" 2>&1 &
    echo $! >"$PID_FILE"

    for _ in {1..50}; do
      kill -0 "$(cat "$PID_FILE")" 2>/dev/null || {
        echo "cloudflared exited during startup; inspect $LOG_FILE" >&2
        exit 1
      }
      if grep -Eq "Registered tunnel connection|Connection .* registered" "$LOG_FILE" 2>/dev/null; then
        break
      fi
      sleep 0.2
    done

    echo "cloudflared: running pid=$(cat "$PID_FILE")"
    ;;

  stop)
    if [[ -f "$PID_FILE" ]]; then
      pid="$(cat "$PID_FILE")"
      kill "$pid" 2>/dev/null || true
      for _ in {1..50}; do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
      done
      kill -9 "$pid" 2>/dev/null || true
      rm -f "$PID_FILE"
    fi
    echo "cloudflared: stopped"
    ;;

  status)
    if [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
      echo "cloudflared: running pid=$(cat "$PID_FILE")"
    else
      echo "cloudflared: stopped"
      exit 1
    fi
    ;;

  doctor)
    need_cloudflared
    validate_token_file
    daemon_ready
    echo "local daemon: healthy"
    echo "tunnel token file: present"
    echo "cloudflared token-file support: available"
    echo "origin policy: only publish http://127.0.0.1:18087; never publish 18088, 9515, 8765, 8091, CDP, or WebDriver"
    echo "Cloudflare Access: require a service-token policy for machine callers"
    ;;

  *)
    echo "usage: $0 {start|stop|status|doctor}" >&2
    exit 2
    ;;
esac
