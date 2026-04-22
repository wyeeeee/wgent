use std::fmt;

/// Categorized LLM errors for retry decision-making.
#[derive(Debug)]
pub enum LlmError {
    /// 429 Too Many Requests — retryable, respect retry-after header
    RateLimited {
        retry_after_ms: Option<u64>,
        message: String,
    },
    /// 401/403 — not retryable
    Authentication { message: String },
    /// 400 Bad Request — not retryable
    BadRequest { message: String },
    /// 404 Model not found — not retryable
    NotFound { message: String },
    /// 5xx server errors — retryable
    ServerError {
        status: u16,
        message: String,
    },
    /// Network / connection failures — retryable
    Network { message: String },
    /// JSON deserialization failures — not retryable
    Parse { message: String },
}

impl LlmError {
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::RateLimited { .. } | Self::ServerError { .. } | Self::Network { .. }
        )
    }

    pub fn suggested_delay_ms(&self) -> Option<u64> {
        match self {
            Self::RateLimited { retry_after_ms, .. } => *retry_after_ms,
            _ => None,
        }
    }
}

impl fmt::Display for LlmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RateLimited { message, .. } => write!(f, "Rate limited: {message}"),
            Self::Authentication { message } => write!(f, "Authentication error: {message}"),
            Self::BadRequest { message } => write!(f, "Bad request: {message}"),
            Self::NotFound { message } => write!(f, "Not found: {message}"),
            Self::ServerError { status, message } => write!(f, "Server error {status}: {message}"),
            Self::Network { message } => write!(f, "Network error: {message}"),
            Self::Parse { message } => write!(f, "Parse error: {message}"),
        }
    }
}

impl std::error::Error for LlmError {}
