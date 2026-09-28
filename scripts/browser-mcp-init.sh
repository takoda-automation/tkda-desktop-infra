#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
CONFIG_DIR="${TKDA_CONFIG_DIR:-$HOME/.config/takoda}"
BROWSER_ENV="${TKDA_BROWSER_MCP_ENV:-$STATE/browser-mcp.env}"

command -v openssl >/dev/null 2>&1 || { echo "missing openssl" >&2; exit 1; }
mkdir -p "$STATE/logs" "$CONFIG_DIR"
chmod 700 "$CONFIG_DIR"
umask 077

generate_secret() {
  local path="$1"
  if [[ ! -s "$path" ]]; then
    openssl rand -hex 48 >"$path"
    chmod 600 "$path"
  fi
}

WORKER_SECRET_FILE="$CONFIG_DIR/browser-mcp-worker.secret"
SIGNING_SECRET_FILE="$CONFIG_DIR/browser-mcp-oauth-signing.secret"
OPERATOR_SECRET_FILE="$CONFIG_DIR/browser-mcp-oauth-operator.secret"
PROFILE_DIR="$CONFIG_DIR/browser-profile"

generate_secret "$WORKER_SECRET_FILE"
generate_secret "$SIGNING_SECRET_FILE"
generate_secret "$OPERATOR_SECRET_FILE"
mkdir -p "$PROFILE_DIR"
chmod 700 "$PROFILE_DIR"

if [[ ! -f "$BROWSER_ENV" ]]; then
  cat >"$BROWSER_ENV" <<EOF
export TKDA_BROWSER_MCP_HOSTNAME='browser-mcp.oresoftware.com'
export TKDA_CLOUDFLARE_TUNNEL='takoda-browser-local'
export TKDA_BROWSER_MCP_ADAPTER_BIND='127.0.0.1:18090'
export TKDA_LOCAL_CONTROL_URL='http://127.0.0.1:18087'
export TKDA_BROWSER_MCP_EXECUTION_MODE='headed'
export TKDA_PLAYWRIGHT_USER_DATA_DIR='$PROFILE_DIR'
export TKDA_BROWSER_MCP_WORKER_SECRET_FILE='$WORKER_SECRET_FILE'
export BROWSER_MCP_OAUTH_SIGNING_SECRET_FILE='$SIGNING_SECRET_FILE'
export BROWSER_MCP_OAUTH_OPERATOR_SECRET_FILE='$OPERATOR_SECRET_FILE'
export BROWSER_MCP_OAUTH_REDIS_URL='redis://127.0.0.1:6379/4'
export TKDA_BROWSER_MCP_ALLOWED_DOMAINS='news.ycombinator.com,greenhouse.io,boards.greenhouse.io,job-boards.greenhouse.io,ashbyhq.com,jobs.ashbyhq.com,lever.co,jobs.lever.co,workday.com,myworkdayjobs.com,smartrecruiters.com,icims.com,jobvite.com,workable.com,bamboohr.com,recruitee.com,applytojob.com,ats.rippling.com,breezy.hr,jobscore.com'
EOF
  chmod 600 "$BROWSER_ENV"
fi

echo "browser MCP local secrets initialized"
echo "config: $BROWSER_ENV"
echo "operator OAuth secret remains local in: $OPERATOR_SECRET_FILE"
