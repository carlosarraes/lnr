use crate::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
#[derive(Clone)]
pub struct Credential(String, bool);
impl Credential {
    pub fn new(key: String) -> Result<Self> {
        if key.trim().is_empty() {
            return Err(AppError::new("authentication", "API key is empty"));
        }
        Ok(Self(key, false))
    }
    pub fn bearer(token: String) -> Result<Self> {
        let mut credential = Self::new(token)?;
        credential.1 = true;
        Ok(credential)
    }
    pub(crate) fn authorization(&self) -> String {
        if self.1 {
            format!("Bearer {}", self.0)
        } else {
            self.0.clone()
        }
    }
    pub(crate) fn secret(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Credential([redacted])")
    }
}
#[derive(Default, Serialize, Deserialize)]
pub struct Config {
    pub default: Option<String>,
    #[serde(default)]
    pub workspaces: Vec<String>,
}
impl Config {
    fn effective_default(&self) -> Option<&str> {
        self.default.as_deref().or_else(|| {
            if self.workspaces.len() == 1 {
                Some(self.workspaces[0].as_str())
            } else {
                None
            }
        })
    }
}
pub fn config_root() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(p));
    }
    std::env::var_os("HOME")
        .map(|p| PathBuf::from(p).join(".config"))
        .ok_or_else(|| AppError::new("configuration", "Set HOME or XDG_CONFIG_HOME"))
}
pub async fn resolve(workspace: Option<&str>) -> Result<Credential> {
    if let Ok(key) = std::env::var("LINEAR_API_KEY") {
        if workspace.is_some() {
            return Err(AppError::input(
                "--workspace conflicts with LINEAR_API_KEY; unset the variable to select stored credentials",
            ));
        }
        return Credential::new(key);
    }
    let config = load_config()?;
    let ws = workspace.or(config.effective_default()).ok_or_else(|| {
        if config.workspaces.is_empty() {
            missing_credentials()
        } else {
            AppError::new(
                "authentication",
                format!(
                    "Choose --workspace or run lnr auth default SLUG. Configured workspaces: {}",
                    config.workspaces.join(", ")
                ),
            )
        }
    })?;
    if !config.workspaces.iter().any(|w| w == ws) {
        return Err(AppError::new(
            "authentication",
            format!(
                "Workspace {ws} is not configured. Run lnr auth status to list configured and importable workspaces"
            ),
        ));
    }
    let _lock = lock().await?;
    let key = store::read(ws)?.ok_or_else(|| AppError::new("authentication", format!(
        "No credential in the {} store for {ws}. Run: lnr auth import-linear --workspace {ws}. Or sign in with: lnr auth login --workspace {ws}",store::backend().unwrap_or("selected")
    )))?;
    oauth::resolve_token(ws, key).await
}
pub fn load_config() -> Result<Config> {
    let path = config_root()?.join("lnr/config.toml");
    let config: Config = match std::fs::read_to_string(path) {
        Ok(s) => toml::from_str(&s)
            .map_err(|_| AppError::new("configuration", "Invalid lnr config.toml"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(e) => return Err(e.into()),
    };
    Ok(config)
}
pub fn missing_credentials() -> AppError {
    let (default, workspaces) = import::available().unwrap_or_default();
    let import = default.as_ref().or(workspaces.first());
    let message = match import {
        Some(ws) => format!("No credentials configured. Run lnr auth import-linear --workspace {ws}, or lnr auth login. Importable workspaces: {}", workspaces.join(", ")),
        None => "No credentials configured. Run lnr auth login or set LINEAR_API_KEY. Run lnr auth status to inspect configured and importable workspaces".into(),
    };
    let mut error = AppError::new("authentication", message);
    error.details =
        Box::new(serde_json::json!({"importable_workspaces":workspaces,"import_default":default}));
    error
}
pub fn keyring_error(action: &str, error: &keyring::Error) -> AppError {
    let reason = match error {
        keyring::Error::NoEntry => "credential not found".to_owned(),
        keyring::Error::PlatformFailure(e) | keyring::Error::NoStorageAccess(e) => e.to_string(),
        _ => "credential store returned an invalid or inaccessible entry".into(),
    };
    AppError::new(
        "configuration",
        format!(
            "Cannot {action} OS keyring entry ({reason}). For headless access, run: LNR_CREDENTIAL_STORE=file lnr auth import-linear --workspace SLUG, then use LNR_CREDENTIAL_STORE=file for commands. Run lnr auth status to find the slug. Or run: LNR_CREDENTIAL_STORE=file lnr auth login"
        ),
    )
}
pub fn inventory() -> Result<serde_json::Value> {
    let config = load_config()?;
    let (default, workspaces) = import::available()?;
    Ok(
        serde_json::json!({"default_workspace":config.effective_default(),"workspaces":config.workspaces,"import_default":default,"importable_workspaces":workspaces,"credential_store":store::backend()?}),
    )
}
pub fn entry(service: &str, ws: &str) -> Result<keyring::Entry> {
    keyring::Entry::new(service, ws)
        .map_err(|_| AppError::new("configuration", "OS keyring unavailable"))
}
pub mod import;

#[derive(Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum AuthResponse {
    Session(crate::model::AuthStatus),
    Inventory {
        default_workspace: Option<String>,
        workspaces: Vec<String>,
        import_default: Option<String>,
        importable_workspaces: Vec<String>,
        credential_store: String,
    },
}

pub mod oauth;
pub mod store;

// Serialize keyring/config updates and token rotation across agent processes.
async fn lock() -> Result<std::fs::File> {
    let dir = store::private_dir()?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(dir.join("auth.lock"))?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(35);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) if std::time::Instant::now() < deadline => {
                tokio::time::sleep(std::time::Duration::from_millis(50)).await
            }
            Err(_) => {
                return Err(AppError::new(
                    "configuration",
                    "Authentication is busy or the credential lock is unavailable; retry shortly",
                ));
            }
        }
    }
}
pub async fn save(workspace: &str, secret: &str, replace: bool) -> Result<()> {
    let _lock = lock().await?;
    if let Some(old) = store::read(workspace)?
        && old != secret
        && !replace
    {
        return Err(AppError::input(
            "Workspace already has a different credential; use --replace",
        ));
    }
    let mut config = load_config()?;
    if !config.workspaces.iter().any(|w| w == workspace) {
        config.workspaces.push(workspace.into());
    }
    if config.default.is_none() {
        config.default = Some(workspace.into());
    }
    let dir = config_root()?.join("lnr");
    let mut temp = tempfile::NamedTempFile::new_in(&dir)?;
    use std::io::Write;
    temp.write_all(
        toml::to_string(&config)
            .map_err(|_| AppError::new("configuration", "Cannot encode config"))?
            .as_bytes(),
    )?;
    store::write(workspace, secret)?;
    temp.persist(dir.join("config.toml")).map_err(|_| {
        AppError::new(
            "configuration",
            "Credential saved, but could not save workspace configuration; retry login/import",
        )
    })?;
    Ok(())
}

pub async fn set_default(workspace: &str) -> Result<serde_json::Value> {
    let _lock = lock().await?;
    let mut config = load_config()?;
    if !config.workspaces.iter().any(|w| w == workspace) {
        return Err(AppError::input(format!(
            "Workspace {workspace} is not configured; run lnr auth status"
        )));
    }
    config.default = Some(workspace.into());
    let dir = config_root()?.join("lnr");
    let mut file = tempfile::NamedTempFile::new_in(&dir)?;
    use std::io::Write;
    file.write_all(
        toml::to_string(&config)
            .map_err(|_| AppError::new("configuration", "Cannot encode config"))?
            .as_bytes(),
    )?;
    file.persist(dir.join("config.toml"))
        .map_err(|_| AppError::new("configuration", "Cannot save default workspace"))?;
    Ok(serde_json::json!({"default_workspace":workspace}))
}
