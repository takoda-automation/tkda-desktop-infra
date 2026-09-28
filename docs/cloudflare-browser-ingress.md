# Cloudflare ingress for local Takoda browser automation

This path is optional. Takoda's normal desktop agent already connects outbound to the hosted
control plane and does not require a public IP. Use this ingress only when an external trusted
automation client needs to call the authenticated Takoda desktop control API directly.

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
worker ports. The daemon keeps its own bearer-token authorization even when Cloudflare Access
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

The tunnel helper uses `cloudflared tunnel run --token-file`, binds its metrics/readiness
listener to loopback, and never receives the token value on argv.

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
