use crate::{
    error::{AppError, Result},
    model,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::collections::HashSet;
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
impl PageRequest {
    pub fn validate(&self) -> Result<()> {
        if !(1..=250).contains(&self.limit) {
            Err(AppError::input("Page limit must be 1..250"))
        } else {
            Ok(())
        }
    }
}
#[derive(Debug, Clone)]
pub struct Reply<T = Value> {
    pub data: T,
    pub meta: Value,
}
impl Reply {
    pub fn data(data: impl Serialize) -> Self {
        Self {
            data: serde_json::to_value(data).expect("JSON-compatible workflow data"),
            meta: json!({}),
        }
    }
    pub fn decode<T: DeserializeOwned>(self) -> Result<Reply<T>> {
        match model::decode(self.data.clone()) {
            Ok(data) => Ok(Reply {
                data,
                meta: self.meta,
            }),
            Err(mut e) => {
                e.data = Some(self.data);
                e.meta = (self.meta).into();
                e.meta["complete"] = false.into();
                Err(e)
            }
        }
    }
}
impl<T: Serialize> Reply<T> {
    pub fn into_json(self) -> Reply {
        Reply {
            data: serde_json::to_value(self.data).expect("JSON-compatible workflow data"),
            meta: self.meta,
        }
    }
}

/// Accumulates available nodes before checking metadata so failures never erase data.
pub(crate) struct Pages {
    pub items: Vec<Value>,
    pub cursor: Option<String>,
    pub meta: Value,
    seen: HashSet<String>,
}
impl Pages {
    pub fn new(cursor: Option<String>) -> Self {
        let mut seen = HashSet::new();
        if let Some(c) = &cursor {
            seen.insert(c.clone());
        }
        Self {
            items: vec![],
            cursor,
            meta: unknown_page(),
            seen,
        }
    }
    pub fn ingest(&mut self, value: &Value) -> Result<()> {
        let nodes = value.get("nodes").and_then(Value::as_array);
        if let Some(nodes) = nodes {
            self.items
                .extend(nodes.iter().cloned().map(model::normalize));
        }
        let meta = connection_meta(value);
        match meta {
            Ok(meta) => {
                self.meta = meta;
                self.cursor = self.meta["end_cursor"].as_str().map(str::to_owned);
            }
            Err(e) => {
                self.meta = unknown_page();
                return Err(e);
            }
        }
        if nodes.is_none() {
            self.meta = unknown_page();
            return Err(AppError::new("api", "Missing connection nodes"));
        }
        if self.has_more() && !self.seen.insert(self.cursor.clone().unwrap_or_default()) {
            self.meta["complete"] = false.into();
            return Err(AppError::new("api", "Pagination cursor repeated"));
        }
        Ok(())
    }
    pub fn has_more(&self) -> bool {
        self.meta["has_more"] == true
    }
    pub fn fail(self, mut error: AppError) -> AppError {
        error.data = Some(self.items.into());
        error.meta = (json!({"page":self.meta,"complete":false})).into();
        error.meta["page"]["complete"] = false.into();
        error
    }
    pub fn finish(self) -> Reply {
        Reply {
            data: self.items.into(),
            meta: json!({"complete":self.meta["complete"],"page":self.meta}),
        }
    }
}
pub(crate) fn unknown_page() -> Value {
    json!({"end_cursor":null,"has_more":null,"complete":false})
}
pub(crate) fn connection_meta(value: &Value) -> Result<Value> {
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
    Ok(json!({"end_cursor":cursor,"has_more":more,"complete":!more}))
}
