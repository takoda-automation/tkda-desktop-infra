use std::{
    env,
    fs::{self, OpenOptions},
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use serde_json::Value;

use crate::secret::read_private_secret;

const DAEMON_ADDR: &str = "127.0.0.1:18087";
const EXPECTED_ORIGIN: &str = "http://127.0.0.1:18087";
const MAX_STATUS_BYTES: u64 = 64 * 1024;

pub fn run_from_env() -> Result<(), String> {
    let command = env::var("TKDA_CLOUDFLARE_COMMAND").unwrap_or_else(|_| "status".to_owned());
    let state = state_dir()?;
    match command.as_str() {
        "start" => start(&state),
        "stop" => stop(&state),
        "status" => status(&state),
        "doctor" => doctor(&state),
        _ => Err("TKDA_CLOUDFLARE_COMMAND must be start, stop, status, or doctor".into()),
    }
}

fn start(state: &Path) -> Result<(), String> {
    let config = Config::from_env()?;
    doctor_with_config(&config)?;

    fs::create_dir_all(state.join("logs")).map_err(|e| format!("create logs directory: {e}"))?;
    let pid_file = state.join("cloudflared.pid");
    if let Some(pid) = read_pid(&pid_file)? {
        if process_alive(pid)? {
            println!("cloudflared: already running pid={pid}");
            return Ok(());
        }
        fs::remove_file(&pid_file).map_err(|e| format!("remove stale pid file: {e}"))?;
    }

    let log_path = state.join("logs/cloudflared.log");
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| format!("open cloudflared log: {e}"))?;
    let stderr = stdout
        .try_clone()
        .map_err(|e| format!("clone cloudflared log handle: {e}"))?;

    let mut command = Command::new(&config.cloudflared_bin);
    command
        .env_clear()
        .args([
            "tunnel",
            "--no-autoupdate",
            "--loglevel",
            &config.log_level,
            "--metrics",
            &config.metrics_bind,
            "run",
            "--token-file",
            path_str(&config.tunnel_token_file)?,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

    for key in ["PATH", "PATHEXT", "SYSTEMROOT", "WINDIR", "HOME", "USERPROFILE", "TMPDIR", "TMP", "TEMP", "LANG", "LC_ALL"] {
        if let Ok(value) = env::var(key) {
            command.env(key, value);
        }
    }

    let child = command
        .spawn()
        .map_err(|e| format!("failed to start cloudflared: {e}"))?;
    let pid = child.id();
    write_pid(&pid_file, pid)?;

    thread::sleep(Duration::from_millis(750));
    if !process_alive(pid)? {
        let _ = fs::remove_file(&pid_file);
        return Err(format!(
            "cloudflared exited during startup; inspect {}",
            log_path.display()
        ));
    }

    println!("cloudflared: running pid={pid}");
    Ok(())
}

fn stop(state: &Path) -> Result<(), String> {
    let pid_file = state.join("cloudflared.pid");
    let Some(pid) = read_pid(&pid_file)? else {
        println!("cloudflared: stopped");
        return Ok(());
    };

    if process_alive(pid)? {
        terminate_process(pid)?;
    }
    if pid_file.exists() {
        fs::remove_file(&pid_file).map_err(|e| format!("remove pid file: {e}"))?;
    }
    println!("cloudflared: stopped");
    Ok(())
}

fn status(state: &Path) -> Result<(), String> {
    let pid_file = state.join("cloudflared.pid");
    let pid = read_pid(&pid_file)?.ok_or_else(|| "cloudflared: stopped".to_owned())?;
    if !process_alive(pid)? {
        return Err("cloudflared: stopped".into());
    }
    println!("cloudflared: running pid={pid}");
    Ok(())
}

fn doctor(_state: &Path) -> Result<(), String> {
    let config = Config::from_env()?;
    doctor_with_config(&config)
}

fn doctor_with_config(config: &Config) -> Result<(), String> {
    ensure_cloudflared_supports_token_file(&config.cloudflared_bin)?;
    let _ = read_private_secret(
        &config.tunnel_token_file,
        "TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE",
        16 * 1024,
    )?;
    let local_control_token = read_private_secret(
        &config.local_control_token_file,
        "TKDA_LOCAL_CONTROL_TOKEN_FILE",
        16 * 1024,
    )?;
    verify_local_daemon(&local_control_token)?;

    if config.origin != EXPECTED_ORIGIN {
        return Err(format!(
            "TKDA_CLOUDFLARE_ORIGIN must be exactly {EXPECTED_ORIGIN}"
        ));
    }

    println!("local daemon: healthy");
    println!("tunnel token file: private regular file");
    println!("cloudflared token-file support: available");
    println!("origin policy: {EXPECTED_ORIGIN} only");
    println!("public endpoint: {}", config.public_url);
    println!("remote configuration E2E: still required before promotion");
    Ok(())
}

struct Config {
    cloudflared_bin: String,
    tunnel_token_file: PathBuf,
    local_control_token_file: PathBuf,
    public_url: String,
    metrics_bind: String,
    log_level: String,
    origin: String,
}

impl Config {
    fn from_env() -> Result<Self, String> {
        let cloudflared_bin = env::var("TKDA_CLOUDFLARED_BIN").unwrap_or_else(|_| "cloudflared".into());
        validate_local_executable(&cloudflared_bin)?;

        let tunnel_token_file = absolute_path_env("TKDA_CLOUDFLARE_TUNNEL_TOKEN_FILE")?;
        let local_control_token_file = absolute_path_env("TKDA_LOCAL_CONTROL_TOKEN_FILE")?;
        let public_url = env::var("TKDA_CLOUDFLARE_PUBLIC_URL")
            .map_err(|_| "TKDA_CLOUDFLARE_PUBLIC_URL is required".to_owned())?;
        validate_public_url(&public_url)?;

        let metrics_bind = env::var("TKDA_CLOUDFLARE_METRICS_BIND")
            .unwrap_or_else(|_| "127.0.0.1:20241".into());
        validate_metrics_bind(&metrics_bind)?;

        let log_level = env::var("TKDA_CLOUDFLARE_LOGLEVEL").unwrap_or_else(|_| "info".into());
        if !matches!(log_level.as_str(), "debug" | "info" | "warn" | "error" | "fatal") {
            return Err("TKDA_CLOUDFLARE_LOGLEVEL must be debug, info, warn, error, or fatal".into());
        }

        let origin = env::var("TKDA_CLOUDFLARE_ORIGIN").unwrap_or_else(|_| EXPECTED_ORIGIN.into());

        Ok(Self {
            cloudflared_bin,
            tunnel_token_file,
            local_control_token_file,
            public_url,
            metrics_bind,
            log_level,
            origin,
        })
    }
}

fn validate_public_url(raw: &str) -> Result<(), String> {
    let url = url::Url::parse(raw).map_err(|_| "TKDA_CLOUDFLARE_PUBLIC_URL must be a valid URL".to_owned())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(
            "TKDA_CLOUDFLARE_PUBLIC_URL must be credential-free root HTTPS without query or fragment".into(),
        );
    }
    Ok(())
}

fn validate_metrics_bind(raw: &str) -> Result<(), String> {
    let address: SocketAddr = raw
        .parse()
        .map_err(|_| "TKDA_CLOUDFLARE_METRICS_BIND must be a socket address".to_owned())?;
    if !address.ip().is_loopback() {
        return Err("TKDA_CLOUDFLARE_METRICS_BIND must be loopback".into());
    }
    Ok(())
}

fn validate_local_executable(raw: &str) -> Result<(), String> {
    let path = Path::new(raw);
    let contains_separator = raw.contains('/') || raw.contains('\\');
    if contains_separator && !path.is_absolute() {
        return Err("TKDA_CLOUDFLARED_BIN paths must be absolute; otherwise use a bare executable name".into());
    }
    if path.is_absolute() {
        let meta = fs::symlink_metadata(path).map_err(|e| format!("inspect TKDA_CLOUDFLARED_BIN: {e}"))?;
        if !meta.file_type().is_file() || meta.file_type().is_symlink() {
            return Err("TKDA_CLOUDFLARED_BIN must be a regular non-symlink file".into());
        }
        return Ok(());
    }
    if raw.is_empty()
        || raw.len() > 128
        || !raw
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err("TKDA_CLOUDFLARED_BIN must be a bounded bare executable name".into());
    }
    Ok(())
}

fn ensure_cloudflared_supports_token_file(bin: &str) -> Result<(), String> {
    let output = Command::new(bin)
        .args(["tunnel", "run", "--help"])
        .output()
        .map_err(|e| format!("failed to inspect cloudflared: {e}"))?;
    if !output.status.success() {
        return Err("cloudflared tunnel run --help failed".into());
    }
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    if !text.contains("--token-file") {
        return Err("cloudflared must support --token-file".into());
    }
    Ok(())
}

fn verify_local_daemon(token: &str) -> Result<(), String> {
    let mut stream = TcpStream::connect_timeout(
        &DAEMON_ADDR.parse().map_err(|_| "invalid internal daemon address".to_owned())?,
        Duration::from_secs(2),
    )
    .map_err(|e| format!("local Takoda daemon is unavailable: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| format!("set daemon read timeout: {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|e| format!("set daemon write timeout: {e}"))?;

    let request = format!(
        "GET /v1/status HTTP/1.1\r\nHost: 127.0.0.1:18087\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("write daemon status request: {e}"))?;

    let mut bytes = Vec::new();
    stream
        .take(MAX_STATUS_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("read daemon status response: {e}"))?;
    let text = String::from_utf8(bytes).map_err(|_| "daemon status response is not UTF-8".to_owned())?;
    let (headers, body) = text
        .split_once("\r\n\r\n")
        .ok_or_else(|| "daemon status response is malformed".to_owned())?;
    if !(headers.starts_with("HTTP/1.1 200 ") || headers.starts_with("HTTP/1.0 200 ")) {
        return Err("daemon status request was not authorized/healthy".into());
    }
    let value: Value = serde_json::from_str(body)
        .map_err(|_| "daemon status response body is invalid JSON".to_owned())?;
    if value.get("ok").and_then(Value::as_bool) != Some(true)
        || value.get("surface").and_then(Value::as_str) != Some("desktop_daemon")
    {
        return Err("daemon status identity is invalid".into());
    }
    Ok(())
}

fn state_dir() -> Result<PathBuf, String> {
    let raw = env::var("TKDA_DESKTOP_STATE").unwrap_or_else(|_| ".desktop".into());
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err("TKDA_DESKTOP_STATE must be absolute for Cloudflare lifecycle management".into());
    }
    fs::create_dir_all(&path).map_err(|e| format!("create desktop state: {e}"))?;
    let meta = fs::symlink_metadata(&path).map_err(|e| format!("inspect desktop state: {e}"))?;
    if !meta.file_type().is_dir() || meta.file_type().is_symlink() {
        return Err("TKDA_DESKTOP_STATE must be a non-symlink directory".into());
    }
    Ok(path)
}

fn absolute_path_env(key: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(env::var(key).map_err(|_| format!("{key} is required"))?);
    if !path.is_absolute() {
        return Err(format!("{key} must be absolute"));
    }
    Ok(path)
}

fn read_pid(path: &Path) -> Result<Option<u32>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let meta = fs::symlink_metadata(path).map_err(|e| format!("inspect pid file: {e}"))?;
    if !meta.file_type().is_file() || meta.file_type().is_symlink() || meta.len() > 32 {
        return Err("cloudflared pid file is invalid".into());
    }
    let raw = fs::read_to_string(path).map_err(|e| format!("read pid file: {e}"))?;
    let pid = raw
        .trim()
        .parse::<u32>()
        .map_err(|_| "cloudflared pid file does not contain a valid PID".to_owned())?;
    if pid == 0 {
        return Err("cloudflared pid must be non-zero".into());
    }
    Ok(Some(pid))
}

fn write_pid(path: &Path, pid: u32) -> Result<(), String> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if !meta.file_type().is_file() || meta.file_type().is_symlink() {
            return Err("cloudflared pid path must be a regular non-symlink file".into());
        }
    }
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    let mut file = options.open(path).map_err(|e| format!("write pid file: {e}"))?;
    writeln!(file, "{pid}").map_err(|e| format!("write pid file: {e}"))
}

#[cfg(unix)]
fn process_alive(pid: u32) -> Result<bool, String> {
    let status = Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map_err(|e| format!("inspect cloudflared process: {e}"))?;
    Ok(status.success())
}

#[cfg(windows)]
fn process_alive(pid: u32) -> Result<bool, String> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .map_err(|e| format!("inspect cloudflared process: {e}"))?;
    Ok(output.status.success() && String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
}

#[cfg(unix)]
fn terminate_process(pid: u32) -> Result<(), String> {
    let pid_text = pid.to_string();
    let _ = Command::new("kill").args(["-TERM", &pid_text]).status();
    for _ in 0..50 {
        if !process_alive(pid)? {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    let status = Command::new("kill")
        .args(["-KILL", &pid_text])
        .status()
        .map_err(|e| format!("force-stop cloudflared: {e}"))?;
    if !status.success() && process_alive(pid)? {
        return Err("cloudflared did not stop".into());
    }
    Ok(())
}

#[cfg(windows)]
fn terminate_process(pid: u32) -> Result<(), String> {
    let status = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
        .map_err(|e| format!("stop cloudflared: {e}"))?;
    if !status.success() && process_alive(pid)? {
        return Err("cloudflared did not stop".into());
    }
    Ok(())
}

fn path_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("path is not UTF-8: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_url_requires_root_https() {
        assert!(validate_public_url("https://browser.example.com").is_ok());
        for value in [
            "http://browser.example.com",
            "https://user:pass@browser.example.com",
            "https://browser.example.com/path",
            "https://browser.example.com?x=1",
        ] {
            assert!(validate_public_url(value).is_err(), "{value}");
        }
    }

    #[test]
    fn metrics_bind_is_loopback_only() {
        assert!(validate_metrics_bind("127.0.0.1:20241").is_ok());
        assert!(validate_metrics_bind("[::1]:20241").is_ok());
        assert!(validate_metrics_bind("0.0.0.0:20241").is_err());
    }

    #[test]
    fn cloudflared_command_is_not_shell_expandable() {
        assert!(validate_local_executable("cloudflared").is_ok());
        assert!(validate_local_executable("cloudflared --token x").is_err());
        assert!(validate_local_executable("./cloudflared").is_err());
    }
}
