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

The compose source pin must equal the native appliance's `desktop-daemon` component revision. The Takoda daemon binds only to `127.0.0.1:18087`; Scintilla binds its authenticated local-control API to `127.0.0.1:8765`. The Scintilla bearer is read from `TKDA_SCINTILLA_TOKEN_FILE` or the daemon's default protected token path and is never sent to the Takoda cloud control plane.

`ores-compose` now starts `scintilla-desktop-daemon` before the Takoda daemon. Takoda sets `TKDA_SUPERVISOR_RUNTIME=scintilla` and `TKDA_LAUNCH_SUPERVISOR=true`; that means Takoda still decides that the trusted `tkda-main-server` supervisor is required, but Scintilla owns the underlying host-process lifecycle through its authenticated loopback worker API. The Takoda job transport remains `execution_target=local`.

## Connectivity and secrets

No public/static/dedicated IP is required. The desktop daemon maintains an outbound authenticated WebSocket to the hosted control plane.

There is intentionally no `.ores-compose.public.yaml` and no inbound Cloudflare Tunnel for Takoda's normal agent path. Browser-control ports remain loopback-only.

Agent and local-control bearer values are read from protected files. The compose contract inherits only the **file paths**, never bearer values embedded in argv.

## Promotion gate

The pinned daemon repository does not currently commit a `Cargo.lock`, so `daemon_lockfile_committed` remains false. Before stable promotion, commit the lockfile and switch the build to `cargo build --locked --release`.

## Common desktop implementation layer

Generic host/security/lifecycle behavior is owned by `ORESoftware/ores-common-desktop-infra`.

This appliance now pins the shared implementation at:

```text
repository = ORESoftware/ores-common-desktop-infra
revision   = 7bb4ed89ab4aa4a81c5e26e36b91f58d6313cc7c
checkout   = tmp/dev/ores-common-desktop-infra
status     = pinned
```

The common implementation supplies the shared Rust consumer checker, loopback policy, secret-file policy, Cloudflare promotion checks, exact-revision update policy, process lifecycle, health/readiness policy, and structured-log redaction.

For an authenticated local checkout, validate this repository through the common code with:

```sh
git clone https://github.com/ORESoftware/ores-common-desktop-infra.git \
  tmp/dev/ores-common-desktop-infra
git -C tmp/dev/ores-common-desktop-infra checkout 7bb4ed89ab4aa4a81c5e26e36b91f58d6313cc7c

ORES_COMMON_DESKTOP_EXPECTED_REVISION=7bb4ed89ab4aa4a81c5e26e36b91f58d6313cc7c \
  cargo run --locked \
  --manifest-path tmp/dev/ores-common-desktop-infra/daemon/rust/Cargo.toml \
  --bin ores-common-desktop-consumer-check
```

Cross-organization GitHub Actions use `.github/workflows/common-layer-certification.yml`. Because the common repository is private, that workflow requires the approved read-only fleet credential boundary (`FLEET_GITHUB_READ_TOKEN` or `TEST_FLEET_READ_TOKEN`). Missing credentials fail closed.

`promotion_gates.common_layer_pinned` is true because the exact source revision is now recorded. `promotion_gates.common_layer_ci_verified` remains false until that certification workflow executes successfully. Stable promotion is not allowed while CI evidence is false.

Product-specific ports, workers, daemon/runtime topology, native contracts, and business behavior remain local to this repository; generic policy should migrate into the pinned common layer rather than be reimplemented here.

