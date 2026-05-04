//! Wallet path helpers: home resolution and tilde expansion (CLI/TUI).

use std::path::{Path, PathBuf};

/// Resolve user home directory (`HOME`, `USERPROFILE`, or Windows `HOMEDRIVE`/`HOMEPATH`).
pub fn resolve_home_dir() -> Result<PathBuf, String> {
    if let Ok(home) = std::env::var("HOME") {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        let trimmed = user_profile.trim();
        if !trimmed.is_empty() {
            return Ok(PathBuf::from(trimmed));
        }
    }
    let drive = std::env::var("HOMEDRIVE").unwrap_or_default();
    let path = std::env::var("HOMEPATH").unwrap_or_default();
    let combined = format!("{}{}", drive.trim(), path.trim());
    if !combined.trim().is_empty() {
        return Ok(PathBuf::from(combined));
    }
    Err("cannot resolve home directory for default wallet path".to_string())
}

/// Expand leading `~` using `home`; returns `path` unchanged when no tilde prefix.
pub fn expand_tilde_path(path: &Path, home: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if raw == "~" {
        return home.to_path_buf();
    }
    if raw.starts_with("~/") || raw.starts_with("~\\") {
        let suffix = raw[2..].trim_start_matches(['/', '\\']);
        return home.join(suffix);
    }
    path.to_path_buf()
}

/// Resolve optional wallet output path: expands `~`, or applies `default_rel` under home when `None`.
pub fn resolve_wallet_out_path(
    path: Option<PathBuf>,
    default_rel: &str,
) -> Result<PathBuf, String> {
    match path {
        Some(base) => {
            let raw = base.to_string_lossy();
            if raw == "~" || raw.starts_with("~/") || raw.starts_with("~\\") {
                let home = resolve_home_dir()?;
                Ok(expand_tilde_path(&base, &home))
            } else {
                Ok(base)
            }
        }
        None => {
            let home = resolve_home_dir()?;
            Ok(expand_tilde_path(&PathBuf::from(default_rel), &home))
        }
    }
}
