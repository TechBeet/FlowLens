//! Statistics tracking for FlowLens GUI
//! 
//! Tracks bytes, packets, protocol counts, and top talkers.

use std::collections::HashMap;
use flowlens_core::models::ProtocolKind;

/// Application statistics
#[derive(Clone, Default)]
pub struct AppStatistics {
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_captured: u64,
    pub protocol_counts: HashMap<String, u64>,
    pub top_talkers: Vec<(String, u64)>,
}

impl AppStatistics {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Update statistics with new event
    pub fn update(&mut self) {
        // Real-time update logic
    }
    
    /// Add a protocol count
    pub fn add_protocol(&mut self, protocol: &str, count: u64) {
        *self.protocol_counts.entry(protocol.to_string()).or_insert(0) += count;
    }
}

