# Takoda local desktop deployment

Takoda has two complementary desktop contracts:

- `appliance.json` remains the native `tkda.desktop-appliance/v1` authority for the full Takoda + Scintilla appliance and its cross-repository component pins.
- `ores-desktop-appliance.json` is the ORES local-orchestration projection consumed by the standardized desktop deployment checks.

## ORES Compose daemon lifecycle

```sh
export TKDA_AGENT_URL='wss://api.takoda.dev/v1/agents/connect'
export TKDA_AGENT_ID='my-laptop'
export TKDA_AGENT_TOKEN_FILE="$HOME/.config/takoda/agent.token"
export TKDA_LOCAL_CONTROL_TOKEN_FILE="$HOME/.config/takoda/local-control.token"

ores-compose check .ores-compose.yaml
ores-compose plan .ores-compose.yaml
ores-compose up .ores-compose.yaml
```

The compose source pin must equal the native appliance's `desktop-daemon` component revision. The daemon binds only to `127.0.0.1:18087`.

`TKDA_LAUNCH_SUPERVISOR=false` is explicit: Takoda does not create a second long-lived process scheduler. Scintilla remains the owner of `tkda-main-server` and other long-lived local processes. The full substrate is still brought up through the native Takoda/Scintilla appliance path until that multi-repo graph is represented directly in ORES Compose.

## Connectivity and secrets

No public/static/dedicated IP is required. The desktop daemon maintains an outbound authenticated WebSocket to the hosted control plane.

There is intentionally no `.ores-compose.public.yaml` and no inbound Cloudflare Tunnel for Takoda's normal agent path. Browser-control ports remain loopback-only.

Agent and local-control bearer values are read from protected files. The compose contract inherits only the **file paths**, never bearer values embedded in argv.

## Promotion gate

The pinned daemon repository does not currently commit a `Cargo.lock`, so `daemon_lockfile_committed` remains false. Before stable promotion, commit the lockfile and switch the build to `cargo build --locked --release`.
