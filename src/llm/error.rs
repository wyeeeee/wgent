use thiserror::Error;

#[derive(Error, Debug)]
pub enum LlmError {
    #[error("API request failed: {0}")]
    RequestFailed(String),

    #[error("API returned error: status={status}, message={message}")]
    ApiError { status: u16, message: String },

    #[error("Failed to parse response: {0}")]
    ParseError(String),

    #[error("Max retries exceeded")]
    MaxRetriesExceeded,
}
