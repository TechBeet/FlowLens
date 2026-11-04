//! Mock data for testing the GUI.

use chrono::Utc;

/// Mock traffic event for testing.
#[allow(dead_code)]
pub struct MockTrafficEvent {
    pub session_id: u64,
    pub conn: String,
    pub process: String,
    pub timestamp: i64,
}

impl MockTrafficEvent {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            session_id: 42,
            conn: "192.168.1.100:8080".to_string(),
            process: "chrome".to_string(),
            timestamp: Utc::now().timestamp(),
        }
    }
}

/// Mock traffic filter.
#[allow(dead_code)]
pub struct MockFilter {
    pub pids: Vec<u32>,
    pub hosts: Vec<String>,
}

impl MockFilter {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            pids: vec![123, 456, 789],
            hosts: vec!["example.com".to_string()],
        }
    }
}