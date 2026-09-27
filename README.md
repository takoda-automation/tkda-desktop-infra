# Takoda Desktop Infra

Single-host infrastructure and desired-state authority for running Takoda automation on a developer- or end-user-owned laptop/desktop.

This repository is intentionally separate from the hosted Takoda control plane. `tkda-desktop-daemon` is the machine-local lifecycle authority; `tkda-cli` and desktop/mobile clients are control-plane clients rather than direct process owners.

Implementation is developed through pull requests from this bootstrap commit.

## ORES Compose local deployment

The audited local lifecycle is declared in `.ores-compose.yaml`:

```sh
ores-compose check .ores-compose.yaml
ores-compose plan .ores-compose.yaml
ores-compose up .ores-compose.yaml
```

Takoda uses an outbound authenticated desktop-agent WebSocket, so no inbound Cloudflare Tunnel or public IP is required.

The daemon source is exact-commit pinned, loopback-only, and executed from the built release binary. Stable promotion remains blocked until the daemon repository commits a Cargo lockfile and the build switches to `--locked`.

See [docs/local-deployment.md](docs/local-deployment.md) and [appliance.json](appliance.json) for the audited boundary and promotion gates.
