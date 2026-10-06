use super::{Credential, config_root};
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
        read_upstream_key(workspace)?
    } else {
        source
            .get(workspace)
            .and_then(toml::Value::as_str)
            .ok_or_else(|| AppError::new("not_found", "Workspace not in Linear credentials"))?
            .to_owned()
    };
    let credential = Credential::new(key.clone())?;
    let status = Service::new(ApiClient::new(credential)?).status().await?;
    if status.organization.as_ref().map(|o| o.url_key.as_str()) != Some(workspace) {
        return Err(AppError::new(
            "authentication",
            "Credential belongs to a different workspace",
        ));
    }
    super::save(workspace, &key, replace).await?;
    Ok(json!({"workspace":workspace,"imported":true}))
}

#[cfg(target_os = "linux")]
fn read_upstream_key(workspace: &str) -> Result<String> {
    let output=std::process::Command::new("secret-tool").args(["lookup","service","linear-cli","account",workspace]).output().map_err(|_|AppError::new("authentication","Install secret-tool to read existing Linear Secret Service credentials, or use LINEAR_API_KEY"))?;
    if !output.status.success() {
        return Err(AppError::new(
            "authentication",
            "Cannot read Linear Secret Service entry",
        ));
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_owned())
        .map_err(|_| AppError::new("authentication", "Invalid keyring entry"))
}
#[cfg(not(target_os = "linux"))]
fn read_upstream_key(workspace: &str) -> Result<String> {
    super::entry("linear-cli", workspace)?
        .get_password()
        .map_err(|_| AppError::new("authentication", "Cannot read Linear keyring entry"))
}

/// Read workspace names only; never expose legacy plaintext API keys.
pub fn available() -> Result<(Option<String>, Vec<String>)> {
    let path = config_root()?.join("linear/credentials.toml");
    let text = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((None, vec![])),
        Err(e) => return Err(e.into()),
    };
    let value: toml::Value = toml::from_str(&text)
        .map_err(|_| AppError::new("configuration", "Invalid Linear credentials file"))?;
    let default = value
        .get("default")
        .and_then(toml::Value::as_str)
        .map(str::to_owned);
    let mut workspaces: Vec<String> =
        if let Some(items) = value.get("workspaces").and_then(toml::Value::as_array) {
            items
                .iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect()
        } else {
            value
                .as_table()
                .into_iter()
                .flat_map(|t| t.iter())
                .filter(|(k, v)| k.as_str() != "default" && v.is_str())
                .map(|(k, _)| k.clone())
                .collect()
        };
    workspaces.sort();
    workspaces.dedup();
    Ok((default.filter(|s| workspaces.contains(s)), workspaces))
}
