#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
SRC="$STATE/src"
BIN="$STATE/bin"
CONFIG="$STATE/config"
TOOL_MANIFEST="$ROOT/tools/tkda-desktop-tool/Cargo.toml"
mkdir -p "$SRC" "$BIN" "$CONFIG" "$STATE/logs"

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing required tool: $1" >&2; exit 1; }; }
for tool in git cargo node npm go curl; do need "$tool"; done

TKDA_APPLIANCE_JSON="$ROOT/appliance.json" TKDA_COMPONENT_ROOT="$SRC" cargo run --quiet --release --manifest-path "$TOOL_MANIFEST" --bin tkda-desktop-materialize

cargo build --release --manifest-path "$SRC/desktop-daemon/Cargo.toml"
cargo build --release --manifest-path "$SRC/main-supervisor/Cargo.toml"
cargo build --release --manifest-path "$SRC/main-supervisor/workers/rust/Cargo.toml"
(
  cd "$SRC/main-supervisor/workers/go"
  go build -o "$BIN/tkda-go-worker" .
)
(
  cd "$SRC/browser-workers"
  test -f package-lock.json || {
    echo "browser-workers is missing package-lock.json; refusing mutable npm dependency resolution" >&2
    exit 1
  }
  npm ci --ignore-scripts --no-audit --no-fund
  npm run build
)
cargo build --release --manifest-path "$SRC/desktop-cli/Cargo.toml"

cp "$SRC/desktop-daemon/target/release/tkda-desktop-daemon" "$BIN/"
cp "$SRC/main-supervisor/target/release/tkda-main-server" "$BIN/"
cp "$SRC/main-supervisor/workers/rust/target/release/tkda-rust-worker" "$BIN/"
cp "$SRC/desktop-cli/target/release/tkda-desktop-cli" "$BIN/"
chmod 0755 "$BIN/"*

TOKEN_FILE="$CONFIG/local-control.token"
TKDA_TOKEN_FILE="$TOKEN_FILE" cargo run --quiet --release --manifest-path "$TOOL_MANIFEST" --bin tkda-desktop-token

cat >"$STATE/env" <<EOF
export TKDA_DESKTOP_DAEMON_BIN="$BIN/tkda-desktop-daemon"
export TKDA_DESKTOP_CLI_BIN="$BIN/tkda-desktop-cli"
export TKDA_MAIN_SERVER_BIN="$BIN/tkda-main-server"
export TKDA_BROWSER_WORKER_ENTRY="$SRC/browser-workers/dist/worker.js"
export TKDA_SELENIUM_NODE_ENTRY="$SRC/browser-workers/dist/selenium-node.js"
export TKDA_PYTHON_WORKER_ENTRY="$SRC/main-supervisor/workers/python/tkda_worker.py"
export TKDA_RUST_WORKER_BIN="$BIN/tkda-rust-worker"
export TKDA_GO_WORKER_BIN="$BIN/tkda-go-worker"
export TKDA_LOCAL_CONTROL_TOKEN_FILE="$TOKEN_FILE"
export SCINTILLA_DESKTOP_INFRA_ROOT="$SRC/scintilla-desktop-infra"
# Optional remote ingress. Keep the token in a chmod 0600 file outside the repo.
# export TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE="$HOME/.config/takoda/cloudflare-tunnel.token"
# export TKDA_CLOUDFLARE_PUBLIC_HOSTNAME="takoda-browser.example.com"
# export TKDA_CLOUDFLARE_METRICS_ADDR="127.0.0.1:20241"
export PATH="$BIN:\$PATH"
EOF

echo "Takoda desktop candidate appliance bootstrapped at $STATE"
echo "Add TKDA_AGENT_URL, TKDA_AGENT_ID, TKDA_AGENT_TOKEN_FILE, TKDA_ALLOW_HEADED, TKDA_CHROMEDRIVER_BIN,"
echo "TKDA_SCINTILLA_RUNTIME_MANIFEST, and the Scintilla ingress paths to $STATE/env."
echo "Render the Scintilla runtime with the Rust tkda-desktop-render tool documented in README.md."
