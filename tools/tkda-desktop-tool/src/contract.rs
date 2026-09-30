use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
struct Appliance {
    schema: String,
    components: Vec<Component>,
    invariants: Value,
}

#[derive(Debug, Deserialize)]
struct Component {
    name: String,
    repo: String,
    rev: String,
}

#[derive(Debug, Deserialize)]
struct ComposeDocument {
    schema_version: String,
    source: ComposeSource,
    services: BTreeMap<String, ComposeService>,
}

#[derive(Debug, Deserialize)]
struct ComposeSource {
    repository: String,
    commit: String,
    checkout_dir: String,
}

#[derive(Debug, Deserialize)]
struct ComposeService {
    runtime: String,
    build: Option<Vec<Vec<String>>>,
    command: Vec<String>,
    inherit_env: Option<Vec<String>>,
    environment: Option<BTreeMap<String, String>>,
}

pub fn validate_repository_contract(root: &Path) -> Result<(), String> {
    let appliance: Appliance = read_json(root.join("appliance.json"))?;
    let ores: Value = read_json(root.join("ores-desktop-appliance.json"))?;
    let compose: ComposeDocument = serde_yaml::from_slice(
        &fs::read(root.join(".ores-compose.yaml")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("parse .ores-compose.yaml: {e}"))?;

    if appliance.schema != "tkda.desktop-appliance/v1" {
        return Err("native appliance schema drifted".into());
    }
    if compose.schema_version != "ores.compose.v1" {
        return Err("ORES Compose project schema drifted".into());
    }
    if compose.source.repository != "https://github.com/takoda-automation/tkda-desktop-daemon.git" {
        return Err("source must be credential-free GitHub HTTPS".into());
    }
    if !is_sha(&compose.source.commit) {
        return Err("source commit must be immutable lowercase 40-hex".into());
    }
    if !compose.source.checkout_dir.starts_with("tmp/dev/")
        || compose.source.checkout_dir.contains("..")
    {
        return Err("checkout must stay under tmp/dev without traversal".into());
    }

    let daemon_component = appliance
        .components
        .iter()
        .find(|item| item.name == "desktop-daemon")
        .ok_or_else(|| "native appliance missing desktop-daemon component".to_owned())?;
    if daemon_component.repo != "takoda-automation/tkda-desktop-daemon"
        || daemon_component.rev != compose.source.commit
    {
        return Err("native/source daemon repo or revision mismatch".into());
    }

    let daemon = compose
        .services
        .get("daemon")
        .ok_or_else(|| "compose daemon service missing".to_owned())?;
    if daemon.runtime != "host"
        || daemon.build.as_ref()
            != Some(&vec![vec![
                "cargo".to_owned(),
                "build".to_owned(),
                "--release".to_owned(),
            ]])
        || daemon.command != vec!["./target/release/tkda-desktop-daemon".to_owned()]
    {
        return Err("daemon build/runtime/command contract drifted".into());
    }

    let inherited = daemon.inherit_env.as_deref().unwrap_or_default();
    for required in [
        "TKDA_AGENT_URL",
        "TKDA_AGENT_ID",
        "TKDA_AGENT_TOKEN_FILE",
        "TKDA_LOCAL_CONTROL_TOKEN_FILE",
    ] {
        if !inherited.iter().any(|value| value == required) {
            return Err(format!("required inherited input missing: {required}"));
        }
    }

    let daemon_env = daemon
        .environment
        .as_ref()
        .ok_or_else(|| "daemon environment missing".to_owned())?;
    expect_env(daemon_env, "TKDA_LOCAL_CONTROL_BIND", "127.0.0.1:18087")?;
    expect_env(daemon_env, "TKDA_LOCAL_SUPERVISOR_BIND", "127.0.0.1:18088")?;
    expect_env(daemon_env, "TKDA_LAUNCH_SUPERVISOR", "false")?;

    if appliance.invariants.get("arbitrary_remote_shell") != Some(&Value::Bool(false))
        || appliance
            .invariants
            .get("browser_control_ports_loopback_only")
            != Some(&Value::Bool(true))
        || appliance
            .invariants
            .get("remote_cloudflare_origin_not_locally_provable")
            != Some(&Value::Bool(true))
    {
        return Err("native security invariants drifted".into());
    }

    let native_json: Value = read_json(root.join("appliance.json"))?;
    if native_json
        .pointer("/promotion_gates/cloudflare_route_and_access_policy_reviewed")
        .and_then(Value::as_bool)
        != Some(false)
    {
        return Err(
            "Cloudflare route/Access gate must remain false without external reviewed evidence"
                .into(),
        );
    }

    if ores.get("schema").and_then(Value::as_str) != Some("ores.desktop-appliance/v1")
        || ores
            .pointer("/host/requires_public_ip")
            .and_then(Value::as_bool)
            != Some(false)
        || ores.pointer("/cloudflare/mode").and_then(Value::as_str) != Some("not-required")
        || ores
            .pointer("/cloudflare/public_ingress_ready")
            .and_then(Value::as_bool)
            != Some(false)
        || ores
            .pointer("/cloudflare/origin_auth")
            .and_then(Value::as_str)
            != Some("outbound-agent-only")
        || !ores
            .pointer("/orchestrator/public_config")
            .is_some_and(Value::is_null)
        || ores
            .pointer("/promotion_gates/daemon_lockfile_committed")
            .and_then(Value::as_bool)
            != Some(false)
    {
        return Err("ORES appliance security/promotion contract drifted".into());
    }
    if root.join(".ores-compose.public.yaml").exists() {
        return Err("unexpected public compose profile".into());
    }

    let common = ores
        .get("common_layer")
        .and_then(Value::as_object)
        .ok_or_else(|| "ORES appliance common_layer is missing".to_owned())?;
    if common.get("repository").and_then(Value::as_str)
        != Some("ORESoftware/ores-common-desktop-infra")
        || common.get("checkout_dir").and_then(Value::as_str)
            != Some("tmp/dev/ores-common-desktop-infra")
    {
        return Err("common desktop layer identity/path drifted".into());
    }
    let capabilities = common
        .get("capabilities")
        .and_then(Value::as_array)
        .ok_or_else(|| "common desktop capability set missing".to_owned())?;
    for required in [
        "consumer-validation",
        "loopback-policy",
        "secret-file-policy",
        "cloudflare-policy",
        "update-policy",
        "process-lifecycle",
        "health-readiness",
        "structured-logging",
    ] {
        if !capabilities
            .iter()
            .any(|value| value.as_str() == Some(required))
        {
            return Err(format!("common desktop capability missing: {required}"));
        }
    }
    let common_status = common
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| "common desktop layer status missing".to_owned())?;
    let common_gate = ores
        .pointer("/promotion_gates/common_layer_pinned")
        .and_then(Value::as_bool)
        .ok_or_else(|| "common desktop layer promotion gate missing".to_owned())?;
    match common_status {
        "awaiting-repository" => {
            if !common.get("revision").is_some_and(Value::is_null) || common_gate {
                return Err("awaiting common desktop layer must remain unpinned".into());
            }
            if ores.get("channel").and_then(Value::as_str) == Some("stable") {
                return Err("stable channel cannot await the common desktop layer".into());
            }
        }
        "pinned" => {
            let revision = common
                .get("revision")
                .and_then(Value::as_str)
                .ok_or_else(|| "pinned common desktop layer revision missing".to_owned())?;
            if !is_sha(revision) || !common_gate {
                return Err(
                    "pinned common desktop layer must use immutable SHA and green gate".into(),
                );
            }
        }
        _ => return Err("unknown common desktop layer status".into()),
    }

    let native_common_gate = appliance.invariants.get("arbitrary_remote_shell").is_some();
    if !native_common_gate {
        return Err("native appliance invariants missing".into());
    }
    if native_json
        .pointer("/promotion_gates/common_desktop_infra_pinned")
        .and_then(Value::as_bool)
        == Some(true)
        && common_status != "pinned"
    {
        return Err("native common desktop promotion gate cannot be green while unpinned".into());
    }

    let scintilla_example: Value =
        read_json(root.join("manifests/scintilla-runtime.example.json"))?;
    let scintilla_workers = scintilla_example
        .get("workers")
        .and_then(Value::as_array)
        .ok_or_else(|| "Scintilla runtime example workers missing".to_owned())?;
    for required in [
        "takoda-desktop-daemon",
        "takoda-main-supervisor",
        "tkda-selenium-node",
    ] {
        if !scintilla_workers
            .iter()
            .any(|worker| worker.get("id").and_then(Value::as_str) == Some(required))
        {
            return Err(format!(
                "Scintilla runtime example missing long-lived worker {required}"
            ));
        }
    }

    for script in ["scripts/up.sh", "scripts/down.sh", "scripts/status.sh"] {
        let source = fs::read_to_string(root.join(script)).map_err(|e| e.to_string())?;
        for forbidden in [
            "tkda-daemon.pid",
            "nohup ",
            "kill -9",
            "Authorization: Bearer",
        ] {
            if source.contains(forbidden) {
                return Err(format!(
                    "{script} reintroduced shell-owned daemon lifecycle or bearer argv: {forbidden}"
                ));
            }
        }
    }

    let policy = fs::read_to_string(root.join(".tkda-desktop.toml")).map_err(|e| e.to_string())?;
    for engine in ["selenium", "playwright", "puppeteer"] {
        if !policy.contains(engine) {
            return Err(format!("desktop policy missing browser engine {engine}"));
        }
    }
    if !policy.contains("allow_remote_shell = false")
        || !policy.contains("expose_browser_control_ports = false")
    {
        return Err("desktop security policy drifted".into());
    }

    let runtime_schema: Value = read_json(root.join("manifests/local-runtime.schema.json"))?;
    let languages = runtime_schema
        .pointer("/properties/execution/properties/pairs/items/properties/language/enum")
        .and_then(Value::as_array)
        .ok_or_else(|| "runtime language enum missing".to_owned())?;
    let language_values = languages
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    if language_values != ["typescript", "python", "go", "rust"] {
        return Err("runtime language enum drifted".into());
    }

    let engines = runtime_schema
        .pointer("/properties/execution/properties/pairs/items/properties/browser_backend/enum")
        .and_then(Value::as_array)
        .ok_or_else(|| "runtime engine enum missing".to_owned())?;
    let mut engine_values = engines.iter().filter_map(Value::as_str).collect::<Vec<_>>();
    engine_values.sort_unstable();
    if engine_values != ["playwright", "puppeteer", "selenium"] {
        return Err("runtime browser enum drifted".into());
    }

    for forbidden in [
        "scripts/render_scintilla_runtime.py",
        "tests/manifest-contract/check.py",
    ] {
        if root.join(forbidden).exists() {
            return Err(format!("forbidden Python tooling remains: {forbidden}"));
        }
    }
    let tunnel_helper =
        fs::read_to_string(root.join("scripts/cloudflare-tunnel.sh")).map_err(|e| e.to_string())?;
    if tunnel_helper.lines().count() > 30
        || !tunnel_helper.contains("--bin tkda-cloudflare")
        || tunnel_helper.contains("cloudflared tunnel")
        || tunnel_helper.contains("curl ")
        || tunnel_helper.contains("kill ")
    {
        return Err("Cloudflare shell wrapper must remain a thin Rust-tooling shim".into());
    }

    let cloudflare_source =
        fs::read_to_string(root.join("tools/tkda-desktop-tool/src/cloudflare.rs"))
            .map_err(|e| e.to_string())?;
    for required in [
        "http://127.0.0.1:18087",
        "TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE",
        "TKDA_LOCAL_CONTROL_TOKEN_FILE",
        "TKDA_CLOUDFLARE_METRICS_ADDR",
        "--token-file",
        "--no-autoupdate",
    ] {
        if !cloudflare_source.contains(required) {
            return Err(format!("Cloudflare Rust boundary is missing {required}"));
        }
    }

    for checked in [
        "scripts/bootstrap.sh",
        "scripts/cloudflare-tunnel.sh",
        ".github/workflows/ci.yml",
        ".github/workflows/desktop-contract.yml",
    ] {
        let source = fs::read_to_string(root.join(checked)).map_err(|e| e.to_string())?;
        if source.contains("python3")
            || source.contains("setup-python")
            || source.contains("ruby <<")
        {
            return Err(format!("non-Rust durable tooling remains in {checked}"));
        }
    }

    Ok(())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Result<T, String> {
    serde_json::from_slice(&fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?)
        .map_err(|e| format!("parse {}: {e}", path.display()))
}

fn expect_env(env: &BTreeMap<String, String>, key: &str, expected: &str) -> Result<(), String> {
    if env.get(key).map(String::as_str) != Some(expected) {
        return Err(format!("{key} drifted"));
    }
    Ok(())
}

fn is_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_contract_is_consistent() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .unwrap();
        validate_repository_contract(root).unwrap();
    }
}
