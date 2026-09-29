use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use uuid::Uuid;

pub fn read_private_secret(path: &Path, label: &str, max_bytes: u64) -> Result<String, String> {
    if !path.is_absolute() {
        return Err(format!("{label} must be absolute"));
    }
    let meta = fs::symlink_metadata(path).map_err(|e| format!("inspect {label}: {e}"))?;
    if !meta.file_type().is_file() || meta.file_type().is_symlink() {
        return Err(format!("{label} must be a regular non-symlink file"));
    }
    if meta.len() == 0 || meta.len() > max_bytes {
        return Err(format!("{label} must contain 1..={max_bytes} bytes"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(format!(
                "{label} permissions are too broad; expected mode 0600"
            ));
        }
    }
    let value = fs::read_to_string(path).map_err(|e| format!("read {label}: {e}"))?;
    let value = value.trim();
    if value.len() < 32 || value.chars().any(char::is_whitespace) {
        return Err(format!(
            "{label} must contain at least 32 non-whitespace characters"
        ));
    }
    Ok(value.to_owned())
}

pub fn ensure_token_from_env() -> Result<(), String> {
    let path = PathBuf::from(
        env::var("TKDA_TOKEN_FILE").map_err(|_| "TKDA_TOKEN_FILE is required".to_owned())?,
    );
    if !path.is_absolute() {
        return Err("TKDA_TOKEN_FILE must be absolute".into());
    }

    if let Ok(meta) = fs::symlink_metadata(&path) {
        if !meta.file_type().is_file() || meta.file_type().is_symlink() {
            return Err("TKDA_TOKEN_FILE must be a regular non-symlink file".into());
        }
        if meta.len() > 0 {
            harden_mode_0600(&path)?;
            return Ok(());
        }
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create token directory: {e}"))?;
    }

    let token = format!(
        "{}{}{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );

    let mut options = OpenOptions::new();
    options.write(true).truncate(true).create(true);
    let mut file = options
        .open(&path)
        .map_err(|e| format!("open token file: {e}"))?;
    writeln!(file, "{token}").map_err(|e| format!("write token file: {e}"))?;
    harden_mode_0600(&path)
}

#[cfg(unix)]
fn harden_mode_0600(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("set {} mode 0600: {e}", path.display()))
}

#[cfg(not(unix))]
fn harden_mode_0600(_path: &Path) -> Result<(), String> {
    Ok(())
}
