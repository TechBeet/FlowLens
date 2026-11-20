//! Error types for the replay crate.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ReplayError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Invalid URL: {0}")]
    Url(#[from] url::ParseError),

    #[error("Unsupported HTTP method: {0}")]
    UnsupportedMethod(String),

    #[error("Session {0} not found")]
    SessionNotFound(u64),

    #[error("Storage error: {0}")]
    Storage(#[from] anyhow::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
