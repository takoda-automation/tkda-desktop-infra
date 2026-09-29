#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
ENV_FILE="${TKDA_DESKTOP_ENV:-$STATE/env}"
TOOL_MANIFEST="$ROOT/tools/tkda-desktop-tool/Cargo.toml"

if [[ -f "$ENV_FILE" ]]; then
  # shellcheck disable=SC1090
  source "$ENV_FILE"
fi

export TKDA_DESKTOP_STATE="$STATE"
export TKDA_CLOUDFLARE_COMMAND="${1:-status}"

exec cargo run --quiet --release   --manifest-path "$TOOL_MANIFEST"   --bin tkda-cloudflare
