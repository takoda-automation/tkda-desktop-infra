# Local browser MCP over Cloudflare Tunnel

This lane exposes Takoda's **local browser automation** to an OAuth-capable MCP
client without exposing the desktop daemon, supervisor, ChromeDriver, CDP, or
Selenium ports.

## Topology

```text
ChatGPT / MCP client
        |
        | HTTPS + OAuth 2.1 / PKCE
        v
Cloudflare Tunnel
        |
        | public hostname -> 127.0.0.1:8092 only
        v
dd-browser-mcp-rs
  browser_state + browser_act
        |
        | X-Server-Auth
        v
tkda-browser-mcp-adapter :18090
        |
        | loopback bearer
        v
tkda-desktop-daemon :18087
        |
        v
tkda-main-server :18088
        |
        v
per-run Takoda Playwright worker
        |
        +-- optional persistent profile under ~/.config/takoda/browser-profile
```

The Cloudflare origin is the Rust MCP gateway only. Never route a tunnel to
ports 18087, 18088, 9515, a Playwright/CDP socket, or a Selenium endpoint.

A public/static IP is not required. `cloudflared` makes outbound connections
from the laptop and owns the public hostname route.

## Prerequisites

The normal Takoda desktop appliance must already be bootstrapped. The following
must be installed locally:

- Node.js 22+;
- Rust/Cargo;
- Redis;
- `cloudflared`;
- the Takoda/Scintilla local stack;
- a checkout of `ORESoftware/k8s-cluster` containing
  `remote/deployments/browser-mcp-rs`.

The default k8s-cluster checkout location is `$HOME/codes/k8s-cluster`; override
it with `K8S_CLUSTER_ROOT`.

## 1. Initialize secrets and browser policy

```bash
bash scripts/browser-mcp-init.sh
source .desktop/browser-mcp.env
```

This creates mode-0600 local secrets and a mode-0700 persistent browser profile.
It does not commit credentials.

The generated job-search hostname ceiling includes common ATS providers plus
LinkedIn, Indeed, Glassdoor, Wellfound, and their known asset roots. Edit
`.desktop/browser-mcp.env` to add a reviewed company career hostname when
needed; do not replace the ceiling with an unrestricted wildcard.

Two variables deliberately carry the same ceiling:

```text
TKDA_BROWSER_MCP_ALLOWED_DOMAINS  gateway/workflow ceiling
TKDA_BROWSER_ALLOWED_DOMAINS      worker-level navigation/subresource ceiling
```

The second check is below model/browser actions, so page scripts, redirects, and
clicks cannot escape merely because the navigation was not an explicit
`browser_act goto`.

## 2. Re-render the Scintilla runtime

The persistent browser profile and worker hostname ceiling are properties of
the long-running local supervisor. Source both environment files before
rendering the runtime:

```bash
source .desktop/env
source .desktop/browser-mcp.env

cargo run --quiet --release \
  --manifest-path tools/tkda-desktop-tool/Cargo.toml \
  --bin tkda-desktop-render
```

The usual renderer inputs such as `SCINTILLA_INGRESS_BIN`,
`SCINTILLA_INGRESS_ROOT`, `TKDA_MAIN_SERVER_BIN`,
`TKDA_BROWSER_WORKER_ENTRY`, `TKDA_CHROMEDRIVER_BIN`,
`TKDA_AGENT_ID`, and `TKDA_SCINTILLA_RUNTIME_MANIFEST` must already be in
`.desktop/env` or the current shell.

For headed browser login, keep:

```bash
export TKDA_ALLOW_HEADED=true
export TKDA_BROWSER_MCP_EXECUTION_MODE=headed
```

Then reconcile/restart the local appliance so the supervisor receives the new
profile and domain-policy environment:

```bash
bash scripts/up.sh
```

## 3. Create the named Cloudflare Tunnel

Run once per workstation/account:

```bash
bash scripts/browser-mcp-cloudflare-init.sh
```

Defaults:

```text
tunnel   = takoda-browser-local
hostname = browser-mcp.oresoftware.com
origin   = http://127.0.0.1:8092
```

If the tunnel does not exist, the script invokes `cloudflared tunnel login`
and then creates it. It also creates/updates the DNS route for the configured
hostname.

This is a named tunnel, not a Quick Tunnel. Quick Tunnels are for testing and
are not the contract for the scheduled browser lane.

## 4. Start the browser MCP lane

```bash
bash scripts/browser-mcp-up.sh
bash scripts/browser-mcp-status.sh
```

The launcher:

1. verifies the authenticated Takoda daemon on `127.0.0.1:18087`;
2. ensures local Redis is available for OAuth grants;
3. starts the loopback Takoda MCP adapter on `127.0.0.1:18090`;
4. starts `dd-browser-mcp-rs` on `127.0.0.1:8092`;
5. starts the named Cloudflare Tunnel to port 8092 only.

Logs and pid files live under `.desktop/logs` and `.desktop/run`.

Stop the lane with:

```bash
bash scripts/browser-mcp-down.sh
```

## 5. Connect ChatGPT or another MCP client

For the direct-hostname tunnel, the **canonical OAuth MCP resource is the
hostname root**:

```text
https://browser-mcp.oresoftware.com
```

The Rust server accepts MCP JSON-RPC POSTs at both `/` and `/mcp`, but OAuth
metadata, registration, authorization, and token URLs are rooted at the
canonical resource above. Configure the custom MCP app with the root URL.

The authorization page asks for the local operator consent secret. Its value is
stored only in:

```text
~/.config/takoda/browser-mcp-oauth-operator.secret
```

Do not put that value in Git, a URL, a shell argument, or the browser automation
model context.

The model-visible surface remains exactly:

- `browser_state` — read sanitized page state and stable refs;
- `browser_act` — perform bounded declarative actions.

## Persistent logged-in sessions

When `TKDA_PLAYWRIGHT_USER_DATA_DIR` is configured, Playwright uses a dedicated
persistent profile. The profile is local-only and mode 0700.

A practical first-use flow is:

1. start the MCP lane in headed mode;
2. open an allowed login page with Takoda;
3. complete passwords, MFA, CAPTCHA, or other human-only challenges yourself;
4. keep the resulting authenticated cookies in the dedicated Takoda profile;
5. subsequent scheduled runs can reuse that local profile after browser/process
   restarts.

Do not point Takoda at your everyday Chrome profile. Use the dedicated profile
created by `browser-mcp-init.sh`.

## Safety boundary

The adapter and worker intentionally fail closed around job/application flows:

- CAPTCHA, MFA/OTP, payment, electronic-signature, and legal-attestation signals
  block browser interactions;
- submission-like controls return `needs_confirmation` with a digest before
  the click is performed;
- `expected_revision` rejects stale actions;
- browser targets are resolved from observed refs/semantic metadata, not
  caller-provided JavaScript;
- arbitrary `evaluate` remains disabled unless a separate local developer
  policy explicitly enables it;
- private/loopback/link-local/metadata browser destinations remain blocked;
- the worker-level hostname ceiling applies to navigations and browser
  subrequests for Playwright/Puppeteer.

For the Alexander D. Mills job-search workflow, the automation may use already
known profile/resume facts, but it must stop instead of guessing new
work-authorization, visa/sponsorship, compensation, relocation, background,
demographic, legal, or similar attestations.

## Health checks

```bash
curl -fsS http://127.0.0.1:18090/agent/healthz
curl -fsS http://127.0.0.1:8092/healthz
curl -fsS https://browser-mcp.oresoftware.com/healthz
```

The first proves daemon-to-adapter connectivity, the second proves the MCP
gateway process, and the third proves the Cloudflare path.

If the public health check works but OAuth does not, inspect:

```text
.desktop/logs/browser-mcp-gateway.log
.desktop/logs/cloudflared-browser-mcp.log
```

If browser actions fail, inspect:

```text
.desktop/logs/browser-mcp-adapter.log
Takoda daemon/main-supervisor logs
```
