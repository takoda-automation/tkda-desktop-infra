#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STATE="${TKDA_DESKTOP_STATE:-$ROOT/.desktop}"
SRC="$STATE/src"
BIN="$STATE/bin"
CONFIG="$STATE/config"
mkdir -p "$SRC" "$BIN" "$CONFIG" "$STATE/logs"

need() { command -v "$1" >/dev/null 2>&1 || { echo "missing required tool: $1" >&2; exit 1; }; }
for tool in git python3 cargo node npm go; do need "$tool"; done

python3 - "$ROOT/appliance.json" "$SRC" <<'PY'
import json
import pathlib
import subprocess
import sys

manifest = json.loads(pathlib.Path(sys.argv[1]).read_text())
root = pathlib.Path(sys.argv[2])
for component in manifest["components"]:
    dest = root / component["name"]
    repo = "https://github.com/" + component["repo"] + ".git"
    rev = component["rev"]
    if not (dest / ".git").exists():
        subprocess.run(["git", "clone", "--filter=blob:none", repo, str(dest)], check=True)
    subprocess.run(["git", "-C", str(dest), "fetch", "--quiet", "origin", rev], check=True)
    subprocess.run(["git", "-C", str(dest), "checkout", "--quiet", "--detach", rev], check=True)
    actual = subprocess.check_output(["git", "-C", str(dest), "rev-parse", "HEAD"], text=True).strip()
    if actual != rev:
        raise SystemExit(f"{component['name']}: expected {rev}, got {actual}")
PY

cargo build --release --manifest-path "$SRC/desktop-daemon/Cargo.toml"
cargo build --release --manifest-path "$SRC/main-supervisor/Cargo.toml"
cargo build --release --manifest-path "$SRC/main-supervisor/workers/rust/Cargo.toml"
(
  cd "$SRC/main-supervisor/workers/go"
  go build -o "$BIN/tkda-go-worker" .
)
(
  cd "$SRC/browser-workers"
  npm install
  npm run build
)
cargo build --release --manifest-path "$SRC/desktop-cli/Cargo.toml"

cp "$SRC/desktop-daemon/target/release/tkda-desktop-daemon" "$BIN/"
cp "$SRC/main-supervisor/target/release/tkda-main-server" "$BIN/"
cp "$SRC/main-supervisor/workers/rust/target/release/tkda-rust-worker" "$BIN/"
cp "$SRC/desktop-cli/target/release/tkda-desktop-cli" "$BIN/"
chmod 0755 "$BIN/"*

TOKEN_FILE="$CONFIG/local-control.token"
if [[ ! -s "$TOKEN_FILE" ]]; then
  python3 - "$TOKEN_FILE" <<'PY'
import secrets
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
path.write_text(secrets.token_urlsafe(48) + "\n")
path.chmod(0o600)
PY
fi

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
export PATH="$BIN:\$PATH"
EOF

echo "Takoda desktop candidate appliance bootstrapped at $STATE"
echo "Add TKDA_AGENT_URL, TKDA_AGENT_ID, TKDA_AGENT_TOKEN_FILE, TKDA_ALLOW_HEADED, TKDA_CHROMEDRIVER_BIN,"
echo "TKDA_SCINTILLA_RUNTIME_MANIFEST, and any Scintilla ingress paths to $STATE/env."
echo "Then render the Scintilla runtime with scripts/render_scintilla_runtime.py."
