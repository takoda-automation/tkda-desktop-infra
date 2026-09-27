#!/usr/bin/env python3
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SHA = re.compile(r"^[0-9a-f]{40}$")

appliance = json.loads((ROOT / "appliance.json").read_text())
assert appliance["schema"] == "tkda.desktop-appliance/v1"
assert appliance["channel"] == "candidate"
components = {item["name"]: item for item in appliance["components"]}
for required in [
    "desktop-daemon",
    "main-supervisor",
    "browser-workers",
    "desktop-cli",
    "scintilla-desktop-infra",
]:
    assert required in components, required
    assert SHA.fullmatch(components[required]["rev"]), required

policy = (ROOT / ".tkda-desktop.toml").read_text()
for engine in ["selenium", "playwright", "puppeteer"]:
    assert engine in policy
assert "allow_remote_shell = false" in policy
assert "expose_browser_control_ports = false" in policy

schema = json.loads((ROOT / "manifests/local-runtime.schema.json").read_text())
pairs = schema["properties"]["execution"]["properties"]["pairs"]["items"]["properties"]
assert pairs["language"]["enum"] == ["typescript", "python", "go", "rust"]
assert set(pairs["browser_backend"]["enum"]) == {"selenium", "playwright", "puppeteer"}

with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    files = {}
    for name in [
        "scintilla-ingress",
        "tkda-main-server",
        "worker.js",
        "selenium-node.js",
        "chromedriver",
        "tkda_worker.py",
        "tkda-rust-worker",
        "tkda-go-worker",
    ]:
        path = root / name
        path.write_text("")
        files[name] = str(path)
    out = root / "runtime.json"
    subprocess.run(
        [
            sys.executable,
            str(ROOT / "scripts/render_scintilla_runtime.py"),
            "--scintilla-ingress-bin",
            files["scintilla-ingress"],
            "--scintilla-ingress-root",
            str(root),
            "--tkda-main-server-bin",
            files["tkda-main-server"],
            "--browser-worker-entry",
            files["worker.js"],
            "--selenium-node-entry",
            files["selenium-node.js"],
            "--chromedriver-bin",
            files["chromedriver"],
            "--python-worker-entry",
            files["tkda_worker.py"],
            "--rust-worker-bin",
            files["tkda-rust-worker"],
            "--go-worker-bin",
            files["tkda-go-worker"],
            "--agent-id",
            "contract-test-agent",
            "--out",
            str(out),
            "--allow-headed",
        ],
        check=True,
    )
    rendered = json.loads(out.read_text())
    assert rendered["schema"] == "scintilla.desktop-runtime/v1"
    assert rendered["runtime_kind"] == "scintilla-single-beam"
    workers = rendered["workers"]
    assert len(workers) == 2
    by_id = {worker["id"]: worker for worker in workers}

    supervisor = by_id["takoda-main-supervisor"]
    env = supervisor["command"]["env"]
    assert env["TKDA_BIND"] == "127.0.0.1:18088"
    assert env["TKDA_EXECUTION_ROLE"] == "desktop"
    assert env["TKDA_ALLOW_HEADED"] == "true"
    assert env["TKDA_SELENIUM_UPSTREAM_URL"] == "http://127.0.0.1:9515"

    selenium = by_id["takoda-selenium-node"]
    assert selenium["runtime"] == "javascript"
    assert selenium["mode"] == "host"
    assert selenium["command"]["program"] == "node"
    assert selenium["command"]["args"] == [files["selenium-node.js"]]
    assert selenium["command"]["env"]["TKDA_CHROMEDRIVER_CMD"] == files["chromedriver"]
    assert selenium["command"]["env"]["TKDA_CHROMEDRIVER_PORT"] == "9515"
    assert rendered["tunnel"] is None

print("Takoda desktop manifest contract: OK")
