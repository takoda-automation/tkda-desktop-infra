#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
ENV_FILE="${TKDA_DESKTOP_ENV:-$STATE/env}"
BROWSER_ENV="${TKDA_BROWSER_MCP_ENV:-$STATE/browser-mcp.env}"
K8S_CLUSTER_ROOT="${K8S_CLUSTER_ROOT:-$HOME/codes/k8s-cluster}"
ADAPTER="$ROOT/tools/tkda-browser-mcp-adapter/adapter.mjs"
MCP_MANIFEST="$K8S_CLUSTER_ROOT/remote/deployments/browser-mcp-rs/Cargo.toml"

test -f "$ENV_FILE" || { echo "missing $ENV_FILE; bootstrap Takoda desktop first" >&2; exit 1; }
test -f "$BROWSER_ENV" || { echo "missing $BROWSER_ENV; run scripts/browser-mcp-init.sh" >&2; exit 1; }
test -f "$MCP_MANIFEST" || { echo "missing browser MCP source at $MCP_MANIFEST" >&2; exit 1; }
# shellcheck disable=SC1090
source "$ENV_FILE"
# shellcheck disable=SC1090
source "$BROWSER_ENV"

: "${TKDA_LOCAL_CONTROL_TOKEN_FILE:?TKDA_LOCAL_CONTROL_TOKEN_FILE is required}"
: "${TKDA_BROWSER_MCP_WORKER_SECRET_FILE:?TKDA_BROWSER_MCP_WORKER_SECRET_FILE is required}"
: "${BROWSER_MCP_OAUTH_SIGNING_SECRET_FILE:?BROWSER_MCP_OAUTH_SIGNING_SECRET_FILE is required}"
: "${BROWSER_MCP_OAUTH_OPERATOR_SECRET_FILE:?BROWSER_MCP_OAUTH_OPERATOR_SECRET_FILE is required}"
: "${TKDA_BROWSER_MCP_ALLOWED_DOMAINS:?TKDA_BROWSER_MCP_ALLOWED_DOMAINS is required}"
: "${TKDA_BROWSER_MCP_HOSTNAME:?TKDA_BROWSER_MCP_HOSTNAME is required}"
: "${TKDA_CLOUDFLARE_TUNNEL:?TKDA_CLOUDFLARE_TUNNEL is required}"

for tool in node cargo curl cloudflared redis-cli redis-server; do
  command -v "$tool" >/dev/null 2>&1 || { echo "missing required tool: $tool" >&2; exit 1; }
done

mkdir -p "$STATE/logs" "$STATE/run"
chmod 700 "$STATE/run"

TOKEN="$(tr -d '\r\n' <"$TKDA_LOCAL_CONTROL_TOKEN_FILE")"
curl --fail --silent -H "Authorization: Bearer $TOKEN" http://127.0.0.1:18087/v1/status >/dev/null || {
  echo "Takoda desktop daemon is not healthy; run scripts/up.sh first" >&2
  exit 1
}

if ! redis-cli -h 127.0.0.1 -p 6379 ping 2>/dev/null | grep -q '^PONG$'; then
  redis-server --bind 127.0.0.1 --port 6379 --daemonize yes
fi

stop_stale() {
  local file="$1"
  if [[ -f "$file" ]]; then
    local pid
    pid="$(cat "$file" 2>/dev/null || true)"
    if [[ "$pid" =~ ^[0-9]+$ ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      for _ in {1..20}; do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
      done
    fi
    rm -f "$file"
  fi
}
stop_stale "$STATE/run/browser-mcp-adapter.pid"
stop_stale "$STATE/run/browser-mcp-gateway.pid"
stop_stale "$STATE/run/cloudflared.pid"

export TKDA_LOCAL_CONTROL_TOKEN_FILE
export TKDA_BROWSER_MCP_ADAPTER_BIND="${TKDA_BROWSER_MCP_ADAPTER_BIND:-127.0.0.1:18090}"
export TKDA_LOCAL_CONTROL_URL="${TKDA_LOCAL_CONTROL_URL:-http://127.0.0.1:18087}"
export TKDA_BROWSER_MCP_EXECUTION_MODE="${TKDA_BROWSER_MCP_EXECUTION_MODE:-headless}"
export TKDA_BROWSER_MCP_WORKER_SECRET_FILE
nohup node "$ADAPTER" >>"$STATE/logs/browser-mcp-adapter.log" 2>&1 &
echo $! >"$STATE/run/browser-mcp-adapter.pid"

for _ in {1..100}; do
  curl --fail --silent http://127.0.0.1:18090/agent/healthz >/dev/null 2>&1 && break
  sleep 0.1
done
curl --fail --silent http://127.0.0.1:18090/agent/healthz >/dev/null

export HOST=127.0.0.1
export PORT=8092
export BROWSER_MCP_WORKER_URL=http://127.0.0.1:18090
export BROWSER_MCP_ALLOWED_DOMAINS="$TKDA_BROWSER_MCP_ALLOWED_DOMAINS"
export BROWSER_MCP_REQUIRE_AUTH=true
export BROWSER_MCP_PUBLIC_BASE_URLS="https://$TKDA_BROWSER_MCP_HOSTNAME/mcp"
export BROWSER_MCP_OAUTH_REDIS_URL="${BROWSER_MCP_OAUTH_REDIS_URL:-redis://127.0.0.1:6379/4}"
export BROWSER_MCP_OAUTH_SIGNING_SECRET="$(tr -d '\r\n' <"$BROWSER_MCP_OAUTH_SIGNING_SECRET_FILE")"
export BROWSER_MCP_OAUTH_OPERATOR_SECRET="$(tr -d '\r\n' <"$BROWSER_MCP_OAUTH_OPERATOR_SECRET_FILE")"
export SERVER_AUTH_SECRET="$(tr -d '\r\n' <"$TKDA_BROWSER_MCP_WORKER_SECRET_FILE")"

nohup cargo run --release --locked --manifest-path "$MCP_MANIFEST"   >>"$STATE/logs/browser-mcp-gateway.log" 2>&1 &
echo $! >"$STATE/run/browser-mcp-gateway.pid"

for _ in {1..600}; do
  curl --fail --silent http://127.0.0.1:8092/healthz >/dev/null 2>&1 && break
  sleep 0.1
done
curl --fail --silent http://127.0.0.1:8092/healthz >/dev/null

nohup cloudflared tunnel --url http://127.0.0.1:8092 run "$TKDA_CLOUDFLARE_TUNNEL"   >>"$STATE/logs/cloudflared-browser-mcp.log" 2>&1 &
echo $! >"$STATE/run/cloudflared.pid"

echo "Takoda browser MCP lane started"
echo "local adapter:  http://127.0.0.1:18090/agent/healthz"
echo "local MCP:      http://127.0.0.1:8092/mcp"
echo "public MCP:     https://$TKDA_BROWSER_MCP_HOSTNAME/mcp"
