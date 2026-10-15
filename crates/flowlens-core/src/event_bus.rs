use tokio::sync::broadcast;

use crate::models::{SessionId, TrafficEvent, TrafficFilter};

/// EventBus trait for publishing and subscribing to traffic events
pub trait EventBus: Send + Sync + 'static {
    /// Publish a traffic event
    fn publish(&self, event: TrafficEvent) -> Result<(), EventBusError>;
    
    /// Subscribe to traffic events matching the filter
    fn subscribe(&self, filter: TrafficFilter) -> broadcast::Receiver<TrafficEvent>;
    
    /// Get the total number of subscribers
    fn subscriber_count(&self) -> usize;
}

/// Error type for EventBus operations
#[derive(Debug, thiserror::Error)]
pub enum EventBusError {
    #[error("Channel closed")]
    ChannelClosed,
    
    #[error("Event too large")]
    EventTooLarge,
    
    #[error("Subscription error: {0}")]
    Subscription(String),
}

/// In-memory broadcast event bus implementation
pub struct BroadcastEventBus {
    sender: broadcast::Sender<TrafficEvent>,
    filter: TrafficFilter,
}

impl BroadcastEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender, filter: TrafficFilter::default() }
    }
}

impl Default for BroadcastEventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl EventBus for BroadcastEventBus {
    fn publish(&self, event: TrafficEvent) -> Result<(), EventBusError> {
        let _ = self.sender.send(event);
        Ok(())
    }
    
    fn subscribe(&self, filter: TrafficFilter) -> broadcast::Receiver<TrafficEvent> {
        self.sender.subscribe()
    }
    
    fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bus_creation() {
        let bus = BroadcastEventBus::new(64);
        assert!(bus.subscriber_count() == 0);
    }

    #[test]
    fn test_bus_publish_subscribe() {
        let bus = BroadcastEventBus::new(16);
        let mut rx = bus.subscribe(TrafficFilter::default());
        
        let event = TrafficEvent::ConnectionEstablished {
            conn: crate::models::ConnectionKey {
                src_addr: "127.0.0.1:12345".parse().unwrap(),
                dst_addr: "8.8.8.8:53".parse().unwrap(),
                protocol: crate::models::TransportProtocol::Udp,
                connection_id: 1,
                timestamp: 0,
                interface_guid: None,
            },
            process: crate::models::ProcessInfo {
                pid: 1234,
                name: "test.exe".to_string(),
                exe_path: None,
                user_sid: "S-1-5-21-123".to_string(),
                arch: crate::models::ProcessArch::X64,
                match_quality: crate::models::MatchQuality::ExactTuple,
            },
        };
        
        bus.publish(event.clone()).unwrap();
        let received = rx.blocking_recv().unwrap();
        assert!(matches!(received, TrafficEvent::ConnectionEstablished { .. }));
    }
}