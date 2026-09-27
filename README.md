# Takoda Desktop Infra

Single-host infrastructure and desired-state authority for running Takoda automation on a developer- or end-user-owned laptop/desktop.

This repository is intentionally separate from the hosted Takoda control plane. `tkda-desktop-daemon` is the machine-local lifecycle authority; `tkda-cli` and desktop/mobile clients are control-plane clients rather than direct process owners.

Implementation is developed through pull requests from this bootstrap commit.


## ORES Compose local deployment

The standardized desktop lifecycle is now declared in `.ores-compose.yaml`:

```sh
ores-compose check .ores-compose.yaml
ores-compose plan .ores-compose.yaml
ores-compose up .ores-compose.yaml
```

Where this product supports inbound public hosting, `.ores-compose.public.yaml` adds a remotely managed `cloudflared` connector using an OS-protected token file. A public/static/dedicated IP is not required. See [docs/local-deployment.md](docs/local-deployment.md).
