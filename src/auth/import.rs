use super::{Config, Credential, config_root, entry};
use crate::{
    api::ApiClient,
    error::{AppError, Result},
    service::Service,
};
use serde_json::{Value, json};
pub async fn import_linear(workspace: &str, replace: bool) -> Result<Value> {
    if workspace.trim().is_empty() {
        return Err(AppError::input("Workspace must not be empty"));
    }
    let root = config_root()?;
    let text = std::fs::read_to_string(root.join("linear/credentials.toml"))?;
    let source: toml::Value = toml::from_str(&text)
        .map_err(|_| AppError::new("configuration", "Invalid Linear credentials file"))?;
    let key = if source.get("workspaces").is_some() {
        let listed = source["workspaces"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v.as_str() == Some(workspace)));
        if !listed {
            return Err(AppError::new(
                "not_found",
                "Workspace not in Linear credentials",
            ));
        }
        entry("linear-cli", workspace)?
            .get_password()
            .map_err(|_| AppError::new("authentication", "Cannot read Linear keyring entry"))?
    } else {
        source
            .get(workspace)
            .and_then(toml::Value::as_str)
            .ok_or_else(|| AppError::new("not_found", "Workspace not in Linear credentials"))?
            .to_owned()
    };
    let credential = Credential::new(key.clone())?;
    let status = Service::new(ApiClient::new(credential)?).status().await?;
    if status["organization"]["urlKey"].as_str() != Some(workspace) {
        return Err(AppError::new(
            "authentication",
            "Credential belongs to a different workspace",
        ));
    }
    let target = entry("lnr", workspace)?;
    match target.get_password() {
        Ok(old) if old != key && !replace => {
            return Err(AppError::input(
                "Workspace already has a different key; use --replace",
            ));
        }
        Ok(_) | Err(keyring::Error::NoEntry) => {}
        Err(_) => {
            return Err(AppError::new(
                "configuration",
                "Cannot inspect lnr keyring entry",
            ));
        }
    }
    let dir = root.join("lnr");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("config.toml");
    let mut config: Config = match std::fs::read_to_string(&path) {
        Ok(s) => {
            toml::from_str(&s).map_err(|_| AppError::new("configuration", "Invalid lnr config"))?
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(e) => return Err(e.into()),
    };
    if !config.workspaces.iter().any(|w| w == workspace) {
        config.workspaces.push(workspace.into());
    }
    if config.default.is_none() {
        config.default = Some(workspace.into());
    }
    target.set_password(&key).map_err(|_| {
        AppError::new(
            "configuration",
            "Cannot store key in OS keyring; use LINEAR_API_KEY instead",
        )
    })?;
    let temp = dir.join(format!("config.{}.tmp", std::process::id()));
    std::fs::write(
        &temp,
        toml::to_string(&config)
            .map_err(|_| AppError::new("configuration", "Cannot encode config"))?,
    )?;
    std::fs::rename(temp, path)?;
    Ok(json!({"workspace":workspace,"imported":true}))
}
