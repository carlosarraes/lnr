use crate::{
    auth::Credential,
    error::{AppError, Result},
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    Query,
    Mutation,
}
#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub endpoint: String,
    pub timeout: Duration,
    pub connect_timeout: Duration,
    pub max_response_bytes: usize,
}
impl Default for ClientOptions {
    fn default() -> Self {
        Self {
            endpoint: "https://api.linear.app/graphql".into(),
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(5),
            max_response_bytes: 16 * 1024 * 1024,
        }
    }
}
#[derive(Clone)]
pub struct ApiClient {
    http: reqwest::Client,
    credential: Credential,
    url: String,
    options: ClientOptions,
}
impl ApiClient {
    pub fn new(credential: Credential) -> Result<Self> {
        let mut options = ClientOptions::default();
        if let Ok(url) = std::env::var("LNR_API_URL") {
            options.endpoint = url;
        }
        Self::with_options(credential, options)
    }
    pub fn with_options(credential: Credential, options: ClientOptions) -> Result<Self> {
        if options.timeout.is_zero()
            || options.connect_timeout.is_zero()
            || options.max_response_bytes == 0
        {
            return Err(AppError::input("Request limits must be positive"));
        }
        let url = options.endpoint.clone();
        let parsed = reqwest::Url::parse(&url).map_err(|_| AppError::input("Invalid API URL"))?;
        if parsed.scheme() != "https"
            && !(parsed.scheme() == "http"
                && matches!(parsed.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")))
        {
            return Err(AppError::input("API URL requires HTTPS except on loopback"));
        }
        let http = reqwest::Client::builder()
            .connect_timeout(options.connect_timeout)
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()
            .map_err(|_| AppError::new("network", "Cannot initialize HTTP client"))?;
        Ok(Self {
            http,
            credential,
            url,
            options,
        })
    }
    pub async fn execute(
        &self,
        query: &str,
        variables: Value,
        name: Option<&str>,
        kind: OperationKind,
    ) -> Result<Value> {
        let started = Instant::now();
        let budget = self.options.timeout;
        for attempt in 0..3 {
            let remaining = budget.saturating_sub(started.elapsed());
            let result = tokio::time::timeout(remaining, self.once(query, &variables, name, kind))
                .await
                .unwrap_or_else(|_| {
                    Err(AppError::new(
                        if kind == OperationKind::Mutation {
                            "uncertain_mutation"
                        } else {
                            "network"
                        },
                        "Request timed out",
                    ))
                });
            let error = match result {
                Ok(v) => return Ok(v),
                Err(e) => e,
            };
            if kind == OperationKind::Mutation || attempt == 2 || !error.retryable {
                return Err(error);
            }
            let jitter = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .subsec_millis()
                % 100;
            let delay = error
                .details
                .get("retry_after_ms")
                .and_then(Value::as_u64)
                .unwrap_or(200 * (1 << attempt) + u64::from(jitter));
            if Duration::from_millis(delay) >= budget.saturating_sub(started.elapsed()) {
                return Err(error);
            }
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        unreachable!()
    }
    async fn once(
        &self,
        query: &str,
        variables: &Value,
        name: Option<&str>,
        kind: OperationKind,
    ) -> Result<Value> {
        let mut response = self
            .http
            .post(&self.url)
            .header("Authorization", self.credential.secret())
            .json(&json!({"query":query,"variables":variables,"operationName":name}))
            .send()
            .await
            .map_err(|cause| {
                let reason = format!("{:?}", cause.without_url())
                    .replace(self.credential.secret(), "[redacted]");
                let mut e = AppError::new(
                    if kind == OperationKind::Mutation {
                        "uncertain_mutation"
                    } else {
                        "network"
                    },
                    "Request failed; read current state before retrying an uncertain write",
                );
                e.retryable = kind == OperationKind::Query;
                e.details = (json!({"reason":reason})).into();
                e
            })?;
        let status = response.status();
        let headers = response.headers().clone();
        let mut bytes = vec![];
        while let Some(chunk) = response.chunk().await.map_err(|_| {
            let mut e = AppError::new(
                if kind == OperationKind::Mutation {
                    "uncertain_mutation"
                } else {
                    "network"
                },
                "Response interrupted",
            );
            e.retryable = kind == OperationKind::Query;
            e
        })? {
            if bytes.len() + chunk.len() > self.options.max_response_bytes {
                return Err(AppError::new(
                    if kind == OperationKind::Mutation {
                        "uncertain_mutation"
                    } else {
                        "api"
                    },
                    format!(
                        "Response exceeds configured limit ({} bytes; default 16 MiB)",
                        self.options.max_response_bytes
                    ),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        let mut body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        redact(&mut body, self.credential.secret());
        let gql_errors = body
            .get("errors")
            .and_then(Value::as_array)
            .filter(|e| !e.is_empty());
        if status.is_success() && gql_errors.is_none() {
            return body
                .get("data")
                .filter(|v| !v.is_null())
                .cloned()
                .ok_or_else(|| {
                    AppError::new(
                        if kind == OperationKind::Mutation {
                            "uncertain_mutation"
                        } else {
                            "api"
                        },
                        "API response has no data",
                    )
                });
        }
        let rate = status.as_u16() == 429
            || gql_errors.is_some_and(|es| {
                es.iter().any(|e| {
                    matches!(
                        e.pointer("/extensions/code").and_then(Value::as_str),
                        Some("RATELIMITED" | "RATE_LIMITED")
                    )
                })
            });
        let transient = rate || matches!(status.as_u16(), 502..=504);
        let code = if status.as_u16() == 401 || status.as_u16() == 403 {
            "authentication"
        } else if kind == OperationKind::Mutation && status.is_server_error() {
            "uncertain_mutation"
        } else if rate {
            "rate_limited"
        } else {
            "api"
        };
        let message = gql_errors
            .and_then(|es| es.first())
            .and_then(|e| e["message"].as_str())
            .unwrap_or("Linear API request failed")
            .replace(self.credential.secret(), "[redacted]");
        let mut e = AppError::new(code, message);
        e.retryable = transient && kind == OperationKind::Query;
        e.data = body.get("data").filter(|d| !d.is_null()).cloned();
        e.details = (json!({"http_status":status.as_u16()})).into();
        if let Some(ms) = headers
            .get("retry-after")
            .and_then(|h| h.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .map(|s| s.saturating_mul(1000))
        {
            e.details["retry_after_ms"] = ms.into();
        }
        if rate {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            for category in ["requests", "endpoint-requests", "complexity"] {
                let remaining = format!("x-ratelimit-{category}-remaining");
                let reset = format!("x-ratelimit-{category}-reset");
                if headers.get(&remaining).and_then(|h| h.to_str().ok()) == Some("0")
                    && let Some(reset) = headers
                        .get(&reset)
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                {
                    let delay = reset
                        .saturating_sub(now)
                        .max(e.details["retry_after_ms"].as_u64().unwrap_or(0));
                    e.details["retry_after_ms"] = delay.into();
                }
            }
        }
        Err(e)
    }
}
fn redact(value: &mut Value, secret: &str) {
    match value {
        Value::String(s) => *s = s.replace(secret, "[redacted]"),
        Value::Array(items) => {
            for item in items {
                redact(item, secret);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                redact(item, secret);
            }
        }
        _ => {}
    }
}
pub mod checked;

pub mod document;
