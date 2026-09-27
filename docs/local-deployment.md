# Local desktop deployment

This repository follows the ORES desktop-appliance contract.

## Local orchestration

`ores-compose` owns the local process lifecycle. The compose manifest pins the desktop daemon to an immutable Git commit and materializes it below `tmp/dev`.

```sh
ores-compose check .ores-compose.yaml
ores-compose plan .ores-compose.yaml
ores-compose up .ores-compose.yaml
```

Stop from another terminal with:

```sh
ores-compose down .ores-compose.yaml
```

The daemon remains loopback-bound and is the privileged local control boundary. Ambient product credentials/tokens must come from the desktop bootstrap or OS secret store; they are never committed to this repo.

## Connectivity

Takoda does not require an inbound Cloudflare Tunnel for its normal desktop-agent path. The daemon already initiates an outbound authenticated WebSocket to the Takoda control plane, so NAT/public-IP setup is unnecessary and an inbound tunnel would add avoidable attack surface.

## Upgrade model

Upgrades change the immutable `source.commit` in the compose manifest after upstream CI is green. Do not use mutable `latest` refs for desktop production/candidate channels.
