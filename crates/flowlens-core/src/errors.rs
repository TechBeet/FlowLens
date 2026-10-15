/// Core errors for FlowLens
use thiserror::Error;

use crate::models::SessionId;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Event bus error: {0}")]
    EventBus(String),
    
    #[error("Session not found: {0:?}")]
    SessionNotFound(SessionId),
    
    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),
    
    #[error("Already running")]
    AlreadyRunning,
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Session not found: {0:?}")]
    SessionNotFound(SessionId),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_display() {
        let err = CoreError::AlreadyRunning;
        assert_eq!(format!("{}", err), "Already running");
    }
}