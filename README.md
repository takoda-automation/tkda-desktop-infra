# Takoda Desktop Infra

Single-host desired-state authority for running Takoda automation on a developer- or end-user-owned laptop/desktop.

## Control loop

`tkda-desktop-daemon` is the single machine-local reconciler and lifecycle writer. The hosted Takoda control plane, `tkda-cli`, `tkda-desktop-app.rs`, and `tkda-flutter` submit desired-state transitions and run requests to the daemon; they do not launch workers, browsers, tunnels, or infrastructure directly.

```text
Takoda hosted control plane
          |
          | authenticated outbound lease/control stream
          v
  tkda-desktop-daemon <---- tkda-cli / desktop apps
          |
          | reconcile desired state from this repo
          v
  tkda-desktop-infra
          |
          +-- task worker: typescript | python | go | rust
          +-- browser backend: selenium | playwright | puppeteer
          +-- optional browser sidecar process
          +-- optional container runtime
          +-- optional Cloudflare Tunnel for explicitly published local services
```

The task-worker language and browser backend are independent axes. A Python/Go/Rust task worker may talk directly to Selenium or delegate Playwright/Puppeteer operations to a bounded browser sidecar. The daemon owns both lifecycles and tears the whole execution graph down on cancellation, timeout, update, or shutdown.

## Authority boundary

This repository owns declarative local topology and policy. `tkda-desktop-daemon` owns reconciliation and process lifecycle. `tkda-main-server.rs` remains the hosted/full control plane and may lease work to connected desktop agents, but normal desktop installs must not require the full main server process.

Cloud leases and local CLI requests enter the same daemon run supervisor. That keeps retries, cancellation, capability matching, browser lifecycle, telemetry, updates, and resource limits consistent regardless of who requested the run.

## Control-plane contract

- `.tkda-desktop.toml` defines the local appliance policy.
- `manifests/local-runtime.schema.json` defines machine desired state.
- `manifests/control-plane.schema.json` defines actors, capabilities, and named transitions.
- `manifests/local-runtime.example.json` demonstrates a safe loopback-only host.

Clients request named operations such as `runtime.reconcile`, `run.create`, `run.cancel`, `tunnel.start`, and `update.apply`. They never submit arbitrary shell commands.

## Security invariants

- daemon control APIs are loopback/IPC only;
- cloud connectivity is outbound from the daemon;
- browser-control ports are never exposed to LAN/mobile/cloud clients;
- process commands come from validated local desired state, not client payloads;
- no remote shell/eval operation exists;
- Cloudflare Tunnel is optional and may publish only explicitly allow-listed local service origins;
- worker/backend compatibility is capability-gated;
- secrets and tunnel credential contents do not belong in this repository.

## Relationship to Scintilla and BeamScale

This follows the same desktop-appliance boundary as `scintilla-run/scintilla-desktop-infra` and `beamscale/beamscale-desktop-infra`: desktop infra describes desired state, the desktop daemon owns machine lifecycle, and CLI/GUI clients participate in a control loop through the daemon. Takoda additionally models a per-run task-worker plus browser-backend execution graph.
