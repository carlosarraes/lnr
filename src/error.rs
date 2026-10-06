use serde::Serialize;
use serde_json::{Value, json};
#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub details: Value,
    #[serde(skip)]
    pub data: Option<Value>,
}
impl AppError {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self { code: code.into(), message: message.into(), retryable: false, details: json!({}), data: None }
    }
    pub fn input(message: impl Into<String>) -> Self { Self::new("invalid_input", message) }
    pub fn exit_code(&self) -> i32 {
        match self.code.as_str() {
            "invalid_input" => 2, "authentication" => 3, "not_found" => 4,
            "ambiguous" => 5, "rate_limited" => 6, "network" => 7,
            "io" | "configuration" => 9, _ => 8,
        }
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", self.message) }
}
impl std::error::Error for AppError {}
impl From<std::io::Error> for AppError {
    fn from(_: std::io::Error) -> Self { Self::new("io", "Unable to read or write the requested file or stream") }
}
pub type Result<T> = std::result::Result<T, AppError>;
