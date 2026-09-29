# Cloudflare ingress for local Takoda browser automation

This path is optional. Takoda's normal desktop agent already connects outbound to the hosted
control plane and does not require a public IP. Use this ingress only when an external trusted
automation client needs to call the authenticated Takoda desktop control API directly.

**Important:** token-based remotely managed tunnels keep their published-application routing
configuration in Cloudflare, not in this repository. The local helper can validate the token
file and its own loopback metrics listener, but it cannot prove which origin Cloudflare has
configured for that token. Therefore remote-token mode is never, by itself, evidence that the
browser-control isolation promotion gate is green. Promotion requires separately reviewed
Cloudflare route/Access policy evidence or a locally managed config whose ingress rules are
validated and pinned.

## Security boundary

Only publish the Takoda desktop daemon origin:

```text
https://<hostname>
      |
Cloudflare Access
      |
Cloudflare Tunnel
      |
http://127.0.0.1:18087
      |
tkda-desktop-daemon
      |
http://127.0.0.1:18088
      |
tkda-main-server -> Playwright / Puppeteer / Selenium workers
```

Never publish ports 18088, 9515, CDP/WebDriver endpoints, Scintilla control ports, or arbitrary
worker ports. A remote dashboard change that points the tunnel at one of those endpoints violates
Takoda's security model even if the tunnel itself remains authenticated. The daemon keeps its own bearer-token authorization even when Cloudflare Access
is enabled, so the intended machine client supplies both Cloudflare Access credentials and the
Takoda local-control bearer token.

## Recommended Cloudflare configuration

Create a **remotely managed** Cloudflare Tunnel and add one published-application route:

- hostname: a dedicated name such as `takoda-browser.example.com`;
- service: `http://localhost:18087`;
- Protect with Access: enabled;
- Access application: self-hosted;
- policy: only explicitly authorized human identities and/or a service token.

For machine-to-machine callers, Cloudflare Access service tokens use
`CF-Access-Client-Id` and `CF-Access-Client-Secret`. Keep those values in the remote
client's secret store, not in this repository.

On the desktop host, save the tunnel token in a protected file:

```sh
install -d -m 0700 "$HOME/.config/takoda"
printf '%s' '<cloudflare tunnel token>' > "$HOME/.config/takoda/cloudflare-tunnel.token"
chmod 0600 "$HOME/.config/takoda/cloudflare-tunnel.token"
```

Do not put the tunnel token in a repository, shell history, checked-in `.env`, or normal
process argv. Current `cloudflared` releases support token files for remotely managed tunnels.

## Takoda desktop configuration

Add these values to `.desktop/env`:

```sh
export TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE="$HOME/.config/takoda/cloudflare-tunnel.token"
export TKDA_CLOUDFLARE_PUBLIC_HOSTNAME="takoda-browser.example.com"
export TKDA_CLOUDFLARE_METRICS_ADDR="127.0.0.1:20241"

# Optional: bring the tunnel up and down with scripts/up.sh and scripts/down.sh.
export TKDA_CLOUDFLARE_AUTO_START=true
```

Then run:

```sh
./scripts/up.sh
bash ./scripts/cloudflare-tunnel.sh status
./scripts/status.sh
```

The compatibility shell helper delegates to the Rust `tkda-cloudflare` tool. The Rust boundary uses a fixed `cloudflared tunnel ... run --token-file` argv, binds metrics/readiness to literal loopback, reads the tunnel and Takoda bearer from private regular files, verifies the authenticated daemon identity at `127.0.0.1:18087`, and never places either secret value on argv.

## Remote client request

The externally visible origin is still the Takoda daemon API. A machine client should call it
with all three credentials:

```sh
curl \
  -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" \
  -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
  -H "Authorization: Bearer $TKDA_LOCAL_CONTROL_TOKEN" \
  "https://$TKDA_CLOUDFLARE_PUBLIC_HOSTNAME/v1/status"
```

Creating a browser run uses `POST /v1/runs`. The request body remains subject to the daemon's
existing execution-mode, placement, identifier, body-size, and headed-browser policy checks.

## Job-search integration

The job-search/application pipeline should treat this endpoint as a browser-execution provider,
not as a general remote shell. A dispatcher may submit bounded Takoda runs for Playwright,
Puppeteer, or Selenium and poll/cancel those run IDs. It must not gain arbitrary command
execution on the laptop.

The preferred control flow is:

```text
job-search scheduler
  -> dedupe / application policy
  -> authenticated Takoda API
  -> local desktop run
  -> browser worker
  -> structured result/evidence
  -> job-search log/dashboard
```

If the public ingress is unavailable, the existing outbound Takoda lease channel remains the
preferred fallback because it needs no inbound route at all.


## Local lifecycle authority

Cloudflare lifecycle and admission are implemented in `tools/tkda-desktop-tool`, not in Bash. `scripts/cloudflare-tunnel.sh` only loads the existing desktop env and dispatches `start`, `stop`, `status`, or `restart` to the Rust tool.

The Rust tool preserves the existing environment contract:

- `TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE`
- `TKDA_CLOUDFLARE_PUBLIC_HOSTNAME`
- `TKDA_CLOUDFLARE_METRICS_ADDR`
- `TKDA_CLOUDFLARED_BIN`
- `TKDA_CLOUDFLARE_AUTO_START`

It also accepts `TKDA_CLOUDFLARE_ORIGIN` only as a local assertion and requires it to equal `http://127.0.0.1:18087`. This does **not** prove the remotely managed tunnel route points there; the external Cloudflare route/Access promotion gate remains false until independently reviewed and negative-tested.
