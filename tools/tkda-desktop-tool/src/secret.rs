use std::{
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use uuid::Uuid;

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
    let mut file = options.open(&path).map_err(|e| format!("open token file: {e}"))?;
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
