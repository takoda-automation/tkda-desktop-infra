# Cloudflare browser gateway

Takoda's preferred desktop control path is still the daemon's outbound authenticated WebSocket to the hosted control plane. A Cloudflare Tunnel is optional and exists for trusted machine clients that need an HTTPS endpoint for submitting local browser runs.

For the ORESoftware job-search pipeline, keep `https://browser-mcp.oresoftware.com/mcp` as the stable public endpoint. That OAuth-protected MCP surface exposes the narrow `browser_state` / `browser_act` contract and should call Takoda through an adapter. Do not repoint that hostname directly at port 18087; the direct daemon gateway described below is an infrastructure/control-plane escape hatch, not the public model-facing API.

## Security model

Publish only the authenticated Takoda desktop-daemon API at `http://127.0.0.1:18087`.

Do **not** publish:

- `127.0.0.1:18088` (Takoda supervisor)
- `127.0.0.1:9515` (ChromeDriver/Selenium)
- `127.0.0.1:8765` (Scintilla daemon)
- `127.0.0.1:8091` (Scintilla ingress)
- browser CDP/WebDriver debugging ports

The public hostname must be protected by Cloudflare Access. Machine callers should authenticate with a Cloudflare Access service token using `CF-Access-Client-Id` and `CF-Access-Client-Secret`. The Takoda API still requires its independent `Authorization: Bearer ...` local-control token, so compromise of either credential alone is insufficient.

## Cloudflare setup

Use a remotely-managed Cloudflare Tunnel. In Cloudflare:

1. Create a tunnel for the Takoda desktop host.
2. Add a published application such as `browser.<your-domain>`.
3. Set the service URL to exactly `http://127.0.0.1:18087`.
4. Protect that hostname with Cloudflare Access.
5. Create a service-token policy for the machine account that will enqueue jobs.
6. Save the tunnel token to a local secret file readable only by the Takoda user.

Example local configuration:

```sh
install -d -m 700 "$HOME/.config/takoda"
printf '%s\n' '<cloudflare-tunnel-token>' > "$HOME/.config/takoda/cloudflared.token"
chmod 600 "$HOME/.config/takoda/cloudflared.token"

cat >> .desktop/env <<'EOF'
export TKDA_CLOUDFLARE_TUNNEL_ENABLED=true
export TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE="$HOME/.config/takoda/cloudflared.token"
export TKDA_CLOUDFLARE_PUBLIC_URL="https://browser.example.com"
EOF
```

Then:

```sh
./scripts/cloudflare-tunnel.sh doctor
./scripts/up.sh
./scripts/status.sh
```

`scripts/up.sh` starts `cloudflared` only when `TKDA_CLOUDFLARE_TUNNEL_ENABLED=true`. The tunnel token is passed through `--token-file`, not the process command line.

## Remote request shape

A machine caller needs both Cloudflare Access credentials and the Takoda local-control bearer:

```sh
curl --fail-with-body \
  -H "CF-Access-Client-Id: $CF_ACCESS_CLIENT_ID" \
  -H "CF-Access-Client-Secret: $CF_ACCESS_CLIENT_SECRET" \
  -H "Authorization: Bearer $TKDA_LOCAL_CONTROL_TOKEN" \
  "$TKDA_CLOUDFLARE_PUBLIC_URL/v1/status"
```

Create runs through `POST /v1/runs`, then poll `GET /v1/runs/{run_id}` or cancel with `POST /v1/runs/{run_id}/cancel`. The daemon enforces desktop placement and headed-mode policy before forwarding a run to the local supervisor.

For the job-search application pipeline, use a dedicated agent/device credential and a dedicated Cloudflare Access service token so audit logs can distinguish scheduler traffic from interactive desktop use.
