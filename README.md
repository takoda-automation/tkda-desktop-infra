# Takoda Desktop Infra

Single-host desired-state authority for running Takoda browser automation on a developer- or end-user-owned laptop/desktop.

## Local architecture

Takoda does **not** implement a second generic laptop process scheduler. The installed `scintilla-run/scintilla-desktop-infra` appliance is the local execution substrate and owns long-lived host/container lifecycle. Takoda remains responsible for tasks, runs, browser semantics, retries, cancellation, evidence, and the browser-worker protocol.

```text
Takoda hosted control plane
          |
          | authenticated outbound lease/control stream
          v
  tkda-desktop-daemon :18087 <---- tkda-desktop-cli / tkda-cli
          ^                          tkda-desktop-app.rs / tkda-flutter
          |
          | Takoda run API
          v
  tkda-main-server :18088  (desktop execution role)
          |
          | bounded per-run stdin/stdout worker protocol
          +---- TypeScript browser worker ---- Playwright
          |                            \----- Puppeteer
          |                            \----- Selenium client
          +---- Python / Go / Rust workers
                                      |
                                      +----- Selenium / bounded browser sidecars

  tkda-desktop-daemon
          |
          | named reconcile/lifecycle requests only
          v
  Scintilla desktop daemon :8765
          |
          v
  scintilla-desktop-infra desired state
          +---- one local Scintilla BEAM ingress/control process :8091
          +---- tkda-main-server desktop supervisor :18088
          +---- optional local Selenium/ChromeDriver node :9515
          +---- host/container support processes
          +---- optional Cloudflare Tunnel
```

The important ownership boundary is deliberate:

- **Scintilla owns long-lived process/container lifecycle on the laptop.**
- **`tkda-desktop-daemon` is the Takoda-facing machine-local control API and Scintilla adapter.**
- **`tkda-main-server` in `desktop` execution role owns Takoda run state and per-run worker children.**
- **Takoda browser workers own browser sessions.**
- **CLI and GUI clients never launch browsers, workers, Scintilla, containers, or tunnels directly.**

This preserves the existing Takoda worker ABI: browser workers exchange bounded commands/events with the local supervisor over child stdin/stdout. Scintilla therefore manages the long-lived Takoda supervisor rather than trying to replace its per-run command channel.

## Browser execution

The worker language and browser backend are independent axes. The local appliance supports:

- TypeScript + Playwright;
- TypeScript + Puppeteer;
- TypeScript + Selenium;
- Python / Go / Rust + Selenium;
- non-TypeScript workers using bounded Playwright/Puppeteer sidecars where required.

Headed and headless execution are separate policy choices. Browser-control ports stay loopback-only and are never published through Cloudflare Tunnel.

## Control loop

`tkda-desktop-daemon` is the only local API Takoda clients need. Hosted leases and local CLI/GUI requests converge on the same run supervisor so cancellation, retries, capability matching, browser lifecycle, telemetry, and resource limits do not diverge by caller.

The daemon reconciles the Scintilla substrate before accepting local execution. Normal installations should set `TKDA_LAUNCH_SUPERVISOR=false`: the Takoda daemon discovers/uses `tkda-main-server` at `127.0.0.1:18088`, while Scintilla owns that process. The legacy direct `Command::new(tkda-main-server)` path remains only as a development/compatibility fallback until the Scintilla adapter lands in the daemon.

## Contracts

- `.tkda-desktop.toml` defines Takoda local policy and the Scintilla substrate endpoint.
- `manifests/control-plane.schema.json` defines Takoda actors and named operations.
- `manifests/local-runtime.schema.json` defines Takoda browser-execution policy.
- `manifests/scintilla-runtime.example.json` is the Scintilla desired-state projection for a Takoda laptop.
- `appliance.json` records the exact component revisions used when this integration was authored.

The current Scintilla desktop contract is `scintilla.local-control/v1`, with daemon `127.0.0.1:8765` and ingress `127.0.0.1:8091`.

## Security invariants

- all daemon and browser-control APIs are loopback/IPC only;
- cloud connectivity is outbound from `tkda-desktop-daemon`;
- process commands come from validated desired state, never arbitrary client payloads;
- Takoda clients cannot submit shell commands or executable argv;
- Cloudflare Tunnel can publish explicitly allow-listed service origins, never WebDriver/CDP/browser-control ports;
- local-control and Scintilla tokens are read from protected local secret files and are not committed or passed as normal CLI arguments;
- worker/backend compatibility is capability-gated;
- exact revisions/digests should be promoted before release; mutable `latest` is not a release contract.

## What belongs here vs elsewhere

This repository contains the Takoda-specific projection onto the generic Scintilla laptop appliance. Generic lifecycle mechanisms stay in `scintilla-run/scintilla-desktop-infra`; Takoda task/run contracts stay in `tkda-interfaces`; browser runtime code stays in `tkda-browser-workers.ts` and `tkda-selenium-server`; GUI behavior stays in the Rust/Flutter apps.


## Local Scintilla appliance

The candidate appliance pins exact revisions in `appliance.json`. `scripts/bootstrap.sh` materializes those revisions and builds the Takoda daemon, desktop CLI, main supervisor, TypeScript browser worker, and Rust/Go worker adapters.

Render the Scintilla desired-state manifest through the repository's Rust tooling. All referenced files must be absolute, existing, regular non-symlink files:

```sh
export SCINTILLA_INGRESS_BIN=/opt/scintilla/ingress/bin/scintilla_ingress
export SCINTILLA_INGRESS_ROOT=/opt/scintilla/ingress
export TKDA_MAIN_SERVER_BIN="$PWD/.desktop/bin/tkda-main-server"
export TKDA_BROWSER_WORKER_ENTRY="$PWD/.desktop/src/browser-workers/dist/worker.js"
export TKDA_SELENIUM_NODE_ENTRY="$PWD/.desktop/src/browser-workers/dist/selenium-node.js"
export TKDA_CHROMEDRIVER_BIN=/absolute/path/to/chromedriver
export TKDA_PYTHON_WORKER_ENTRY="$PWD/.desktop/src/main-supervisor/workers/python/tkda_worker.py"
export TKDA_RUST_WORKER_BIN="$PWD/.desktop/bin/tkda-rust-worker"
export TKDA_GO_WORKER_BIN="$PWD/.desktop/bin/tkda-go-worker"
export TKDA_AGENT_ID=my-laptop
export TKDA_ALLOW_HEADED=true
export TKDA_SCINTILLA_RUNTIME_MANIFEST="$PWD/.desktop/runtime.scintilla.json"

cargo run --quiet --release \
  --manifest-path tools/tkda-desktop-tool/Cargo.toml \
  --bin tkda-desktop-render
```

The renderer rejects unsafe agent identifiers, symlinked execution files, non-absolute execution paths, and arbitrary Node/Python command paths.

Then place the required cloud-agent values and manifest path in `.desktop/env`:

```sh
export TKDA_AGENT_URL='wss://api.takoda.dev/v1/agents/connect'
export TKDA_AGENT_ID='my-laptop'
export TKDA_AGENT_TOKEN_FILE="$HOME/.config/takoda/agent.token"
export TKDA_ALLOW_HEADED=true
export TKDA_SCINTILLA_RUNTIME_MANIFEST="$PWD/.desktop/runtime.scintilla.json"
```

Start and inspect the full local stack:

```sh
./scripts/up.sh
./scripts/status.sh
.desktop/bin/tkda-desktop-cli --command=doctor
```

Optional direct remote browser dispatch is enabled with a remotely managed Cloudflare Tunnel token file and `TKDA_CLOUDFLARE_AUTO_START=true`. The tunnel is deliberately outside ORES Compose lifecycle. Use `bash ./scripts/cloudflare-tunnel.sh {start|stop|status|restart}` for explicit control.

The expected local ports are Scintilla ingress on `127.0.0.1:8091`, the Takoda desktop supervisor on `127.0.0.1:18088`, raw ChromeDriver on `127.0.0.1:9515`, and the authenticated Takoda desktop daemon on `127.0.0.1:18087`. Per-run browser workers communicate through stdio; the ChromeDriver port is loopback-only and must never be published through Cloudflare Tunnel.

### Browser engines

`tkda-browser-workers.ts` is the TypeScript browser runtime and supports Playwright, Puppeteer, and Selenium. `tkda-main-server.rs` launches it with the requested `TKDA_BROWSER_ENGINE`. For Selenium, Scintilla separately owns the long-running `dist/selenium-node.js` wrapper, which launches raw ChromeDriver on loopback; the per-run worker connects through `TKDA_SELENIUM_UPSTREAM_URL=http://127.0.0.1:9515`. Non-TypeScript adapters remain separate worker languages and can use their native Selenium path or a bounded browser sidecar as the contracts evolve.

The candidate channel intentionally pins in-review desktop-control commits while this implementation wave is under review. Promotion remains fail-closed until the referenced component CI and local browser E2E gates are green on the exact pinned revisions.

## ORES Compose local daemon lifecycle

The standardized ORES Compose projection is additive to the native Takoda/Scintilla appliance; it does not replace the Scintilla substrate.

```sh
ores-compose check .ores-compose.yaml
ores-compose plan .ores-compose.yaml
ores-compose up .ores-compose.yaml
```

The compose manifest pins the same `tkda-desktop-daemon` revision as native `appliance.json`, requires agent/local-control token **file paths**, binds only to loopback, and sets `TKDA_LAUNCH_SUPERVISOR=false` so Scintilla remains the long-lived process owner.

Takoda needs no inbound Cloudflare Tunnel or public IP for its normal desktop-agent path. For trusted clients that need direct browser-run dispatch, the repository now also supports a separately managed Cloudflare Access + Tunnel path that publishes only the authenticated daemon on `127.0.0.1:18087`; browser-control ports remain private. See [docs/cloudflare-browser-ingress.md](docs/cloudflare-browser-ingress.md), [docs/local-deployment.md](docs/local-deployment.md), and [ores-desktop-appliance.json](ores-desktop-appliance.json).

## Shared desktop infra dependency

Generic desktop lifecycle/security behavior is moving to `ORESoftware/ores-common-desktop-infra`. This repo declares that dependency in its ORES appliance metadata and blocks stable promotion until an exact common-layer commit is pinned.

Current state is intentionally `awaiting-repository` with a null revision because GitHub does not yet expose that repository through the connected installation. Product-local behavior remains candidate-only until the common layer can be consumed by exact SHA.



### Desktop browser network isolation gate

Application-layer URL validation in the Playwright/Puppeteer worker is defense in depth, **not** a complete SSRF boundary. DNS can change between validation and the browser's actual connection, and Selenium cannot reliably intercept every subresource request.

Cloud workers are protected by the Kubernetes egress NetworkPolicy in `tkda-infra`. Desktop promotion therefore has a separate `desktop_browser_network_isolation_green` gate. Keep it false until the Scintilla/desktop substrate proves an OS/runtime-level egress boundary that blocks loopback, link-local, private RFC1918/ULA, metadata, and other special-use destinations for browser child processes while preserving public web access.

Do not mark three-browser desktop E2E as production-safe based only on JavaScript hostname/DNS checks.
