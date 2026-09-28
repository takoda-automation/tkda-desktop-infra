use std::{
    env,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Appliance {
    schema: String,
    components: Vec<Component>,
}

#[derive(Debug, Deserialize)]
struct Component {
    name: String,
    repo: String,
    rev: String,
}

pub fn materialize_from_env() -> Result<(), String> {
    let appliance_path = absolute_regular_file("TKDA_APPLIANCE_JSON")?;
    let component_root = absolute_directory_or_create("TKDA_COMPONENT_ROOT")?;
    let appliance: Appliance = serde_json::from_slice(
        &fs::read(&appliance_path).map_err(|e| format!("read appliance: {e}"))?,
    )
    .map_err(|e| format!("parse appliance: {e}"))?;
    if appliance.schema != "tkda.desktop-appliance/v1" {
        return Err("unsupported appliance schema".into());
    }

    for component in appliance.components {
        validate_component(&component)?;
        let dest = component_root.join(&component.name);
        let url = format!("https://github.com/{}.git", component.repo);

        if dest.join(".git").is_dir() {
            let actual_remote = output(
                Command::new("git").args(["-C", path_str(&dest)?, "remote", "get-url", "origin"]),
                "read origin",
            )?;
            if actual_remote.trim_end().trim_end_matches(".git") != url.trim_end_matches(".git") {
                return Err(format!("{}: origin mismatch", component.name));
            }
        } else if dest.exists() {
            return Err(format!("{} exists but is not a Git checkout", dest.display()));
        } else {
            run(
                Command::new("git").args([
                    "clone",
                    "--filter=blob:none",
                    "--no-checkout",
                    &url,
                    path_str(&dest)?,
                ]),
                "clone component",
            )?;
        }

        run(
            Command::new("git").args([
                "-C",
                path_str(&dest)?,
                "fetch",
                "--quiet",
                "origin",
                &component.rev,
            ]),
            "fetch exact revision",
        )?;
        run(
            Command::new("git").args([
                "-C",
                path_str(&dest)?,
                "checkout",
                "--quiet",
                "--detach",
                &component.rev,
            ]),
            "checkout exact revision",
        )?;
        let actual = output(
            Command::new("git").args(["-C", path_str(&dest)?, "rev-parse", "HEAD"]),
            "verify revision",
        )?;
        if actual.trim() != component.rev {
            return Err(format!("{}: exact revision verification failed", component.name));
        }
    }

    Ok(())
}

fn validate_component(component: &Component) -> Result<(), String> {
    if !safe_identifier(&component.name, 128) {
        return Err(format!("invalid component name: {}", component.name));
    }
    let allowed_org = component.repo.starts_with("takoda-automation/")
        || component.repo.starts_with("scintilla-run/");
    if !allowed_org
        || component.repo.contains("..")
        || component.repo.chars().any(char::is_whitespace)
    {
        return Err(format!("component repo is outside allowed organizations: {}", component.repo));
    }
    if !is_sha(&component.rev) {
        return Err(format!("{} revision is not exact lowercase 40-hex", component.name));
    }
    Ok(())
}

fn safe_identifier(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.chars().count() <= max
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | ':' | '-'))
}

fn is_sha(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn absolute_regular_file(key: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(env::var(key).map_err(|_| format!("{key} is required"))?);
    if !path.is_absolute() {
        return Err(format!("{key} must be absolute"));
    }
    let meta = fs::symlink_metadata(&path).map_err(|e| format!("{key}: {e}"))?;
    if !meta.file_type().is_file() || meta.file_type().is_symlink() {
        return Err(format!("{key} must reference a regular non-symlink file"));
    }
    Ok(path)
}

fn absolute_directory_or_create(key: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(env::var(key).map_err(|_| format!("{key} is required"))?);
    if !path.is_absolute() {
        return Err(format!("{key} must be absolute"));
    }
    fs::create_dir_all(&path).map_err(|e| format!("{key}: {e}"))?;
    Ok(path)
}

fn path_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("path is not UTF-8: {}", path.display()))
}

fn run(command: &mut Command, label: &str) -> Result<(), String> {
    let status = command.status().map_err(|e| format!("{label}: {e}"))?;
    if !status.success() {
        return Err(format!("{label} failed with {status}"));
    }
    Ok(())
}

fn output(command: &mut Command, label: &str) -> Result<String, String> {
    let output = command.output().map_err(|e| format!("{label}: {e}"))?;
    if !output.status.success() {
        return Err(format!("{label} failed with {}", output.status));
    }
    String::from_utf8(output.stdout).map_err(|_| format!("{label} produced non-UTF8 output"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn immutable_revision_and_repo_validation_fail_closed() {
        let good = Component {
            name: "desktop-daemon".into(),
            repo: "takoda-automation/tkda-desktop-daemon".into(),
            rev: "a".repeat(40),
        };
        assert!(validate_component(&good).is_ok());

        let mut bad = good;
        bad.rev = "main".into();
        assert!(validate_component(&bad).is_err());
    }
}
