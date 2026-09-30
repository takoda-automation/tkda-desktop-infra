use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde_json::{Value, json};
use url::Url;

use crate::secret::read_private_secret;

#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub scintilla_ingress_bin: PathBuf,
    pub scintilla_ingress_root: PathBuf,
    pub tkda_desktop_daemon_bin: PathBuf,
    pub tkda_main_server_bin: PathBuf,
    pub browser_worker_entry: PathBuf,
    pub selenium_node_entry: PathBuf,
    pub chromedriver_bin: PathBuf,
    pub python_worker_entry: PathBuf,
    pub rust_worker_bin: PathBuf,
    pub go_worker_bin: PathBuf,
    pub agent_id: String,
    pub agent_url: String,
    pub agent_token_file: PathBuf,
    pub local_control_token_file: PathBuf,
    pub out: PathBuf,
    pub node_bin: String,
    pub python_bin: String,
    pub allow_headed: bool,
}

pub fn config_from_env() -> Result<RenderConfig, String> {
    Ok(RenderConfig {
        scintilla_ingress_bin: absolute_regular_file("SCINTILLA_INGRESS_BIN")?,
        scintilla_ingress_root: absolute_directory("SCINTILLA_INGRESS_ROOT")?,
        tkda_desktop_daemon_bin: absolute_regular_file("TKDA_DESKTOP_DAEMON_BIN")?,
        tkda_main_server_bin: absolute_regular_file("TKDA_MAIN_SERVER_BIN")?,
        browser_worker_entry: absolute_regular_file("TKDA_BROWSER_WORKER_ENTRY")?,
        selenium_node_entry: absolute_regular_file("TKDA_SELENIUM_NODE_ENTRY")?,
        chromedriver_bin: absolute_regular_file("TKDA_CHROMEDRIVER_BIN")?,
        python_worker_entry: absolute_regular_file("TKDA_PYTHON_WORKER_ENTRY")?,
        rust_worker_bin: absolute_regular_file("TKDA_RUST_WORKER_BIN")?,
        go_worker_bin: absolute_regular_file("TKDA_GO_WORKER_BIN")?,
        agent_id: bounded_identifier_env("TKDA_AGENT_ID", 256)?,
        agent_url: secure_agent_url_env("TKDA_AGENT_URL")?,
        agent_token_file: private_secret_path("TKDA_AGENT_TOKEN_FILE")?,
        local_control_token_file: private_secret_path("TKDA_LOCAL_CONTROL_TOKEN_FILE")?,
        out: absolute_output_path("TKDA_SCINTILLA_RUNTIME_MANIFEST")?,
        node_bin: bounded_program_env("TKDA_NODE_BIN", "node")?,
        python_bin: bounded_program_env("TKDA_PYTHON_BIN", "python3")?,
        allow_headed: env::var("TKDA_ALLOW_HEADED").as_deref() == Ok("true"),
    })
}

pub fn render_manifest(config: &RenderConfig) -> Result<Value, String> {
    validate_agent_id(&config.agent_id)?;
    Ok(json!({
        "schema": "scintilla.desktop-runtime/v1",
        "runtime_kind": "scintilla-single-beam",
        "ingress": {
            "command": {
                "program": path_string(&config.scintilla_ingress_bin)?,
                "args": ["foreground"],
                "cwd": path_string(&config.scintilla_ingress_root)?,
                "env": {"SCINTILLA_LISTEN": "127.0.0.1:8091"}
            },
            "stop": {
                "command": {
                    "program": path_string(&config.scintilla_ingress_bin)?,
                    "args": ["stop"],
                    "cwd": path_string(&config.scintilla_ingress_root)?
                },
                "timeout_seconds": 15
            },
            "release_command": path_string(&config.scintilla_ingress_bin)?
        },
        "workers": [
            {
                "id": "takoda-desktop-daemon",
                "runtime": "rust",
                "mode": "host",
                "command": {
                    "program": path_string(&config.tkda_desktop_daemon_bin)?,
                    "args": [],
                    "env": {
                        "TKDA_LAUNCH_SUPERVISOR": "false",
                        "TKDA_LOCAL_SUPERVISOR_URL": "http://127.0.0.1:18088",
                        "TKDA_LOCAL_SUPERVISOR_BIND": "127.0.0.1:18088",
                        "TKDA_LOCAL_CONTROL_BIND": "127.0.0.1:18087",
                        "TKDA_ALLOW_HEADED": if config.allow_headed { "true" } else { "false" },
                        "TKDA_AGENT_URL": config.agent_url,
                        "TKDA_AGENT_ID": config.agent_id,
                        "TKDA_AGENT_TOKEN_FILE": path_string(&config.agent_token_file)?,
                        "TKDA_LOCAL_CONTROL_TOKEN_FILE": path_string(&config.local_control_token_file)?
                    }
                },
                "stop": {"timeout_seconds": 20}
            },
            {
                "id": "takoda-main-supervisor",
                "runtime": "rust",
                "mode": "host",
                "command": {
                    "program": path_string(&config.tkda_main_server_bin)?,
                    "args": [],
                    "env": {
                        "TKDA_BIND": "127.0.0.1:18088",
                        "TKDA_EXECUTION_ROLE": "desktop",
                        "TKDA_AGENT_ID": config.agent_id,
                        "TKDA_ALLOW_HEADED": if config.allow_headed { "true" } else { "false" },
                        "TKDA_TYPESCRIPT_WORKER_CMD": config.node_bin,
                        "TKDA_TYPESCRIPT_WORKER_ENTRY": path_string(&config.browser_worker_entry)?,
                        "TKDA_PYTHON_WORKER_CMD": config.python_bin,
                        "TKDA_PYTHON_WORKER_ENTRY": path_string(&config.python_worker_entry)?,
                        "TKDA_RUST_WORKER_CMD": path_string(&config.rust_worker_bin)?,
                        "TKDA_GO_WORKER_CMD": path_string(&config.go_worker_bin)?,
                        "TKDA_SELENIUM_UPSTREAM_URL": "http://127.0.0.1:9515"
                    }
                },
                "stop": {"timeout_seconds": 20}
            },
            {
                "id": "takoda-selenium-node",
                "runtime": "javascript",
                "mode": "host",
                "command": {
                    "program": config.node_bin,
                    "args": [path_string(&config.selenium_node_entry)?],
                    "env": {
                        "TKDA_CHROMEDRIVER_CMD": path_string(&config.chromedriver_bin)?,
                        "TKDA_CHROMEDRIVER_PORT": "9515"
                    }
                },
                "stop": {"timeout_seconds": 15}
            }
        ],
        "tunnel": null,
        "update": null,
        "preferences": {"keep_scintilla_alive_during_lock_screen": false},
        "control_plane": {
            "protocol": "scintilla.local-control/v1",
            "clients": [
                {"kind": "infra", "capabilities": ["runtime.reconcile", "worker.control"]},
                {"kind": "cli", "capabilities": ["status.read", "runtime.reconcile", "runtime.stop", "worker.read"]},
                {"kind": "rust_desktop", "capabilities": ["status.read", "worker.read"]},
                {"kind": "flutter_desktop", "capabilities": ["status.read", "worker.read"]}
            ]
        }
    }))
}

pub fn write_manifest(config: &RenderConfig) -> Result<(), String> {
    let manifest = render_manifest(config)?;
    if let Some(parent) = config.out.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create output directory: {e}"))?;
    }
    let encoded = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())? + "\n";
    fs::write(&config.out, encoded).map_err(|e| format!("write {}: {e}", config.out.display()))
}

pub fn validate_agent_id(value: &str) -> Result<(), String> {
    if is_identifier(value, 256) {
        Ok(())
    } else {
        Err("TKDA_AGENT_ID must be a 1..=256 safe identifier".into())
    }
}

fn is_identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.chars().count() <= max
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | ':' | '-'))
}

fn bounded_identifier_env(key: &str, max: usize) -> Result<String, String> {
    let value = env::var(key).map_err(|_| format!("{key} is required"))?;
    if !is_identifier(&value, max) {
        return Err(format!("{key} is invalid"));
    }
    Ok(value)
}

fn secure_agent_url_env(key: &str) -> Result<String, String> {
    let raw = env::var(key).map_err(|_| format!("{key} is required"))?;
    if raw.len() > 2_048 {
        return Err(format!("{key} exceeds 2048 characters"));
    }
    let url = Url::parse(&raw).map_err(|_| format!("{key} is not a valid URL"))?;
    let host = url
        .host_str()
        .ok_or_else(|| format!("{key} is missing a host"))?
        .trim_matches(['[', ']'])
        .to_ascii_lowercase();
    let loopback = host == "127.0.0.1" || host == "::1";
    if (url.scheme() != "wss" && !(url.scheme() == "ws" && loopback))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(format!(
            "{key} must use wss (or ws on literal loopback) without credentials, query, or fragment"
        ));
    }
    Ok(url.to_string())
}

fn private_secret_path(key: &str) -> Result<PathBuf, String> {
    let raw = env::var(key).map_err(|_| format!("{key} is required"))?;
    let path = PathBuf::from(raw);
    let _ = read_private_secret(&path, key, 16 * 1024)?;
    Ok(path)
}

fn bounded_program_env(key: &str, default: &str) -> Result<String, String> {
    let value = env::var(key).unwrap_or_else(|_| default.to_owned());
    if value.is_empty()
        || value.len() > 256
        || value.contains('/')
        || value.contains('\\')
        || value.chars().any(char::is_whitespace)
    {
        return Err(format!("{key} must be a bare executable name"));
    }
    Ok(value)
}

fn absolute_regular_file(key: &str) -> Result<PathBuf, String> {
    let raw = env::var(key).map_err(|_| format!("{key} is required"))?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(format!("{key} must be absolute"));
    }
    let meta = fs::symlink_metadata(&path).map_err(|e| format!("{key}: {e}"))?;
    if !meta.file_type().is_file() || meta.file_type().is_symlink() {
        return Err(format!("{key} must reference a regular non-symlink file"));
    }
    Ok(path)
}

fn absolute_directory(key: &str) -> Result<PathBuf, String> {
    let raw = env::var(key).map_err(|_| format!("{key} is required"))?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(format!("{key} must be an existing absolute directory"));
    }
    let meta = fs::symlink_metadata(&path).map_err(|e| format!("{key}: {e}"))?;
    if !meta.file_type().is_dir() || meta.file_type().is_symlink() {
        return Err(format!(
            "{key} must reference an existing non-symlink directory"
        ));
    }
    Ok(path)
}

fn absolute_output_path(key: &str) -> Result<PathBuf, String> {
    let raw = env::var(key).map_err(|_| format!("{key} is required"))?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(format!("{key} must be absolute"));
    }
    if let Ok(meta) = fs::symlink_metadata(&path) {
        if meta.file_type().is_symlink() || !meta.file_type().is_file() {
            return Err(format!(
                "{key} must reference a regular non-symlink file when it already exists"
            ));
        }
    }
    Ok(path)
}

fn path_string(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("path is not UTF-8: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn render_keeps_browser_control_loopback_only() {
        let root = std::env::temp_dir().join(format!("tkda-desktop-render-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let file = |name: &str| {
            let path = root.join(name);
            fs::write(&path, "").unwrap();
            path
        };
        let config = RenderConfig {
            scintilla_ingress_bin: file("scintilla-ingress"),
            scintilla_ingress_root: root.clone(),
            tkda_desktop_daemon_bin: file("tkda-desktop-daemon"),
            tkda_main_server_bin: file("tkda-main-server"),
            browser_worker_entry: file("worker.js"),
            selenium_node_entry: file("selenium-node.js"),
            chromedriver_bin: file("chromedriver"),
            python_worker_entry: file("worker.py"),
            rust_worker_bin: file("rust-worker"),
            go_worker_bin: file("go-worker"),
            agent_id: "contract-test-agent".into(),
            agent_url: "wss://api.takoda.dev/v1/agents/connect".into(),
            agent_token_file: file("agent.token"),
            local_control_token_file: file("control.token"),
            out: root.join("runtime.json"),
            node_bin: "node".into(),
            python_bin: "python3".into(),
            allow_headed: true,
        };
        let manifest = render_manifest(&config).unwrap();
        let workers = manifest["workers"].as_array().unwrap();
        assert_eq!(workers.len(), 3);
        let daemon = workers
            .iter()
            .find(|v| v["id"] == "takoda-desktop-daemon")
            .unwrap();
        assert_eq!(
            daemon["command"]["env"]["TKDA_LOCAL_CONTROL_BIND"],
            "127.0.0.1:18087"
        );
        assert_eq!(
            daemon["command"]["env"]["TKDA_LOCAL_SUPERVISOR_URL"],
            "http://127.0.0.1:18088"
        );
        assert_eq!(
            daemon["command"]["env"]["TKDA_LAUNCH_SUPERVISOR"],
            "false"
        );
        let supervisor = workers
            .iter()
            .find(|v| v["id"] == "takoda-main-supervisor")
            .unwrap();
        assert_eq!(supervisor["command"]["env"]["TKDA_BIND"], "127.0.0.1:18088");
        assert_eq!(
            supervisor["command"]["env"]["TKDA_SELENIUM_UPSTREAM_URL"],
            "http://127.0.0.1:9515"
        );
        let selenium = workers
            .iter()
            .find(|v| v["id"] == "takoda-selenium-node")
            .unwrap();
        assert_eq!(selenium["command"]["env"]["TKDA_CHROMEDRIVER_PORT"], "9515");
        assert!(manifest["tunnel"].is_null());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checked_in_example_matches_supervisor_identity() {
        let example: Value = serde_json::from_str(include_str!(
            "../../../manifests/scintilla-runtime.example.json"
        ))
        .unwrap();
        let workers = example["workers"].as_array().unwrap();
        assert!(
            workers
                .iter()
                .any(|worker| worker["id"] == "takoda-main-supervisor")
        );
        assert!(
            !workers
                .iter()
                .any(|worker| worker["id"] == "tkda-local-supervisor")
        );
    }

    #[test]
    fn identifiers_fail_closed() {
        assert!(validate_agent_id("../agent").is_err());
        assert!(validate_agent_id("agent-1").is_ok());
    }
}
