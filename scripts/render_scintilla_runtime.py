#!/usr/bin/env python3
import argparse
import json
from pathlib import Path


def absolute_file(raw: str, label: str) -> str:
    path = Path(raw).expanduser()
    if not path.is_absolute():
        raise SystemExit(f"{label} must be an absolute path: {path}")
    return str(path)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Render a Scintilla desktop runtime that supervises Takoda locally."
    )
    parser.add_argument("--scintilla-ingress-bin", required=True)
    parser.add_argument("--scintilla-ingress-root", required=True)
    parser.add_argument("--tkda-main-server-bin", required=True)
    parser.add_argument("--browser-worker-entry", required=True)
    parser.add_argument("--selenium-node-entry", required=True)
    parser.add_argument("--chromedriver-bin", required=True)
    parser.add_argument("--python-worker-entry", required=True)
    parser.add_argument("--rust-worker-bin", required=True)
    parser.add_argument("--go-worker-bin", required=True)
    parser.add_argument("--agent-id", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--node-bin", default="node")
    parser.add_argument("--python-bin", default="python3")
    parser.add_argument("--allow-headed", action="store_true")
    args = parser.parse_args()

    if not args.agent_id or len(args.agent_id) > 256 or any(ch.isspace() for ch in args.agent_id):
        raise SystemExit("--agent-id must contain 1..=256 non-whitespace characters")

    ingress_bin = absolute_file(args.scintilla_ingress_bin, "--scintilla-ingress-bin")
    ingress_root = absolute_file(args.scintilla_ingress_root, "--scintilla-ingress-root")
    main_server = absolute_file(args.tkda_main_server_bin, "--tkda-main-server-bin")
    browser_worker = absolute_file(args.browser_worker_entry, "--browser-worker-entry")
    selenium_node = absolute_file(args.selenium_node_entry, "--selenium-node-entry")
    chromedriver = absolute_file(args.chromedriver_bin, "--chromedriver-bin")
    python_worker = absolute_file(args.python_worker_entry, "--python-worker-entry")
    rust_worker = absolute_file(args.rust_worker_bin, "--rust-worker-bin")
    go_worker = absolute_file(args.go_worker_bin, "--go-worker-bin")
    out = Path(args.out).expanduser()
    if not out.is_absolute():
        raise SystemExit("--out must be an absolute path")

    manifest = {
        "schema": "scintilla.desktop-runtime/v1",
        "runtime_kind": "scintilla-single-beam",
        "ingress": {
            "command": {
                "program": ingress_bin,
                "args": ["foreground"],
                "cwd": ingress_root,
                "env": {"SCINTILLA_LISTEN": "127.0.0.1:8091"},
            },
            "stop": {
                "command": {
                    "program": ingress_bin,
                    "args": ["stop"],
                    "cwd": ingress_root,
                },
                "timeout_seconds": 15,
            },
            "release_command": ingress_bin,
        },
        "workers": [
            {
                "id": "takoda-main-supervisor",
                "runtime": "rust",
                "mode": "host",
                "command": {
                    "program": main_server,
                    "args": [],
                    "env": {
                        "TKDA_BIND": "127.0.0.1:18088",
                        "TKDA_EXECUTION_ROLE": "desktop",
                        "TKDA_AGENT_ID": args.agent_id,
                        "TKDA_ALLOW_HEADED": "true" if args.allow_headed else "false",
                        "TKDA_TYPESCRIPT_WORKER_CMD": args.node_bin,
                        "TKDA_TYPESCRIPT_WORKER_ENTRY": browser_worker,
                        "TKDA_PYTHON_WORKER_CMD": args.python_bin,
                        "TKDA_PYTHON_WORKER_ENTRY": python_worker,
                        "TKDA_RUST_WORKER_CMD": rust_worker,
                        "TKDA_GO_WORKER_CMD": go_worker,
                        "TKDA_SELENIUM_UPSTREAM_URL": "http://127.0.0.1:9515",
                    },
                },
                "stop": {"timeout_seconds": 20},
            },
            {
                "id": "takoda-selenium-node",
                "runtime": "javascript",
                "mode": "host",
                "command": {
                    "program": args.node_bin,
                    "args": [selenium_node],
                    "env": {
                        "TKDA_CHROMEDRIVER_CMD": chromedriver,
                        "TKDA_CHROMEDRIVER_PORT": "9515",
                    },
                },
                "stop": {"timeout_seconds": 15},
            },
        ],
        "tunnel": None,
        "update": None,
        "preferences": {"keep_scintilla_alive_during_lock_screen": False},
        "control_plane": {
            "protocol": "scintilla.local-control/v1",
            "clients": [
                {
                    "kind": "infra",
                    "capabilities": ["runtime.reconcile", "worker.control"],
                },
                {
                    "kind": "cli",
                    "capabilities": ["status.read", "runtime.reconcile", "runtime.stop", "worker.read"],
                },
                {
                    "kind": "rust_desktop",
                    "capabilities": ["status.read", "worker.read"],
                },
                {
                    "kind": "flutter_desktop",
                    "capabilities": ["status.read", "worker.read"],
                },
            ],
        },
    }

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(manifest, indent=2) + "\n")
    print(out)


if __name__ == "__main__":
    main()
