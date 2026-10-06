use crate::error::{AppError, Result};
use serde_json::{Value, json};
#[derive(Debug, Clone)]
pub struct PageRequest {
    pub limit: u16,
    pub after: Option<String>,
    pub all: bool,
}
impl Default for PageRequest {
    fn default() -> Self {
        Self {
            limit: 50,
            after: None,
            all: false,
        }
    }
}
#[derive(Debug, Clone)]
pub struct Reply {
    pub data: Value,
    pub meta: Value,
}
impl Reply {
    pub fn data(data: impl serde::Serialize) -> Self {
        Self {
            data: serde_json::to_value(data).expect("JSON-compatible workflow data"),
            meta: json!({}),
        }
    }
}
pub fn connection(value: &Value) -> Result<(Vec<Value>, Value)> {
    let nodes = value
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::new("api", "Missing connection nodes"))?
        .clone();
    let info = &value["pageInfo"];
    let more = info["hasNextPage"]
        .as_bool()
        .ok_or_else(|| AppError::new("api", "Missing page information"))?;
    let cursor = info["endCursor"].as_str();
    if more && cursor.is_none_or(str::is_empty) {
        return Err(AppError::new(
            "api",
            "API reports more pages without a cursor",
        ));
    }
    Ok((
        nodes,
        json!({"end_cursor":cursor,"has_more":more,"complete":!more}),
    ))
}
