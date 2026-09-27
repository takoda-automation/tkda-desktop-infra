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
