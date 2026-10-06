//! Credential persistence. Callers hold auth.lock across read/modify/write.
use super::{config_root, entry, keyring_error};
use crate::error::{AppError, Result};
use std::{collections::BTreeMap, io::Write, path::PathBuf};

pub fn backend() -> Result<&'static str> {
    match std::env::var("LNR_CREDENTIAL_STORE").as_deref() {
        Ok("file") => Ok("file"),
        Ok("keyring") => Ok("keyring"),
        Err(std::env::VarError::NotPresent) => Ok(if cfg!(target_os = "macos") {
            "file"
        } else {
            "keyring"
        }),
        _ => Err(AppError::input(
            "LNR_CREDENTIAL_STORE must be file or keyring",
        )),
    }
}

pub fn private_dir() -> Result<PathBuf> {
    let dir = config_root()?.join("lnr");
    match std::fs::symlink_metadata(&dir) {
        Ok(meta) if !meta.is_dir() || meta.file_type().is_symlink() => {
            return Err(AppError::new(
                "configuration",
                "The lnr config directory must be a real directory, not a symlink",
            ));
        }
        Ok(_) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(&dir)?;
        }
        Err(e) => return Err(e.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(dir)
}

fn file_credentials() -> Result<BTreeMap<String, String>> {
    let path = private_dir()?.join("credentials.json");
    let meta = match std::fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(e.into()),
    };
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(AppError::new(
            "configuration",
            "lnr credentials.json must be a regular file, not a symlink",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o777 != 0o600 {
            return Err(AppError::new(
                "configuration",
                "Credential file must have mode 0600. Run: chmod 600 \"${XDG_CONFIG_HOME:-$HOME/.config}/lnr/credentials.json\"",
            ));
        }
    }
    let bytes = std::fs::read(path)?;
    serde_json::from_slice(&bytes).map_err(|_| AppError::new("configuration", "Invalid lnr credentials.json; restore your credential file backup or move it aside and run lnr auth import-linear --workspace SLUG"))
}

pub fn read(workspace: &str) -> Result<Option<String>> {
    if backend()? == "file" {
        return Ok(file_credentials()?.remove(workspace));
    }
    match entry("lnr", workspace)?.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(keyring_error("read", &e)),
    }
}

pub fn write(workspace: &str, secret: &str) -> Result<()> {
    if backend()? == "keyring" {
        return entry("lnr", workspace)?
            .set_password(secret)
            .map_err(|e| keyring_error("store", &e));
    }
    let mut credentials = file_credentials()?;
    credentials.insert(workspace.into(), secret.into());
    let dir = private_dir()?;
    // NamedTempFile starts with 0600, so secrets are never briefly world-readable.
    let mut file = tempfile::NamedTempFile::new_in(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    let bytes = serde_json::to_vec(&credentials)
        .map_err(|_| AppError::new("configuration", "Cannot encode credentials"))?;
    file.write_all(&bytes)?;
    file.as_file().sync_all()?;
    file.persist(dir.join("credentials.json"))
        .map_err(|_| AppError::new("configuration", "Cannot replace credential file atomically"))?;
    #[cfg(unix)]
    std::fs::File::open(dir)?.sync_all()?;
    Ok(())
}
