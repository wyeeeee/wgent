use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("tool execution failed: {0}")]
    ToolExecutionFailed(String),

    #[error("max agent loop iterations exceeded")]
    MaxIterationsExceeded,
}
