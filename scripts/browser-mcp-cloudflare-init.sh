#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
BROWSER_ENV="${TKDA_BROWSER_MCP_ENV:-$STATE/browser-mcp.env}"

test -f "$BROWSER_ENV" || { echo "missing $BROWSER_ENV; run scripts/browser-mcp-init.sh" >&2; exit 1; }
# shellcheck disable=SC1090
source "$BROWSER_ENV"

: "${TKDA_BROWSER_MCP_HOSTNAME:?TKDA_BROWSER_MCP_HOSTNAME is required}"
: "${TKDA_CLOUDFLARE_TUNNEL:?TKDA_CLOUDFLARE_TUNNEL is required}"
command -v cloudflared >/dev/null 2>&1 || { echo "missing cloudflared" >&2; exit 1; }

if ! cloudflared tunnel info "$TKDA_CLOUDFLARE_TUNNEL" >/dev/null 2>&1; then
  echo "Cloudflare browser authentication is required once for this machine."
  cloudflared tunnel login
  cloudflared tunnel create "$TKDA_CLOUDFLARE_TUNNEL"
fi

cloudflared tunnel route dns "$TKDA_CLOUDFLARE_TUNNEL" "$TKDA_BROWSER_MCP_HOSTNAME" || true

echo "Cloudflare tunnel prepared: $TKDA_CLOUDFLARE_TUNNEL"
echo "canonical OAuth MCP endpoint: https://$TKDA_BROWSER_MCP_HOSTNAME"
echo "Only the OAuth MCP gateway on 127.0.0.1:8092 will be published."
