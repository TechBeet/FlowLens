use tokio::sync::broadcast;

use crate::models::{TrafficEvent, TrafficFilter, TrafficRow};

/// EventBus trait for publishing and subscribing to traffic events
pub trait EventBus: Send + Sync + 'static {
    /// Publish a traffic event
    fn publish(&self, event: TrafficEvent) -> Result<(), EventBusError>;

    /// Subscribe to traffic events matching the filter
    fn subscribe(&self, filter: TrafficFilter) -> broadcast::Receiver<TrafficEvent>;

    /// Get the total number of subscribers
    fn subscriber_count(&self) -> usize;

    /// Whether at least one subscriber is currently connected.
    fn has_subscribers(&self) -> bool {
        self.subscriber_count() > 0
    }

    /// Whether the broadcast channel has been closed (all receivers dropped).
    /// Note: In tokio broadcast, the channel is only "closed" when all receivers
    /// have been dropped AND a send would fail. Zero subscribers is not the same
    /// as closed - the channel can have zero subscribers but still accept new ones.
    fn is_closed(&self) -> bool {
        // We need to track if the channel sender has been closed separately
        // For now, return false - the channel is only truly closed if send() fails
        // and we can't determine that from subscriber count alone
        false
    }
}

/// Diagnostics trait for BroadcastEventBus-specific querying
pub trait EventBusDiagnostics: EventBus {
    /// Get total events published (diagnostic)
    fn event_count(&self) -> u64;

    /// Get latest events snapshot (for --events reporting)
    fn latest_events(&self, limit: usize) -> Vec<TrafficRow>;

    /// Get current filter subscriptions (for --filters reporting)
    fn filter_list(&self) -> Vec<TrafficFilter>;
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

/// In-memory broadcast event bus implementation with diagnostics.
pub struct BroadcastEventBus {
    sender: broadcast::Sender<TrafficEvent>,
    /// Internal dummy receiver to keep the channel open for publishing.
    /// We never receive from it, but it prevents send() from failing when
    /// external subscribers have not yet connected.
    _internal_rx: broadcast::Receiver<TrafficEvent>,
    evt_cnt: std::sync::Arc<std::sync::atomic::AtomicU64>,
    recent: std::sync::Arc<std::sync::Mutex<Vec<TrafficRow>>>,
    filters: std::sync::Arc<std::sync::Mutex<Vec<TrafficFilter>>>,
}

impl BroadcastEventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, internal_rx) = broadcast::channel(capacity);
        Self {
            sender,
            _internal_rx: internal_rx,
            evt_cnt: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            recent: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            filters: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// Get total events published (diagnostic)
    pub fn get_event_count(&self) -> u64 {
        self.evt_cnt.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Get latest events snapshot (for --events reporting)
    pub fn get_latest_events(&self, limit: usize) -> Vec<TrafficRow> {
        let guard = self.recent.lock().unwrap();
        let start = guard.len().saturating_sub(limit);
        guard[start..].to_vec()
    }

    /// Get current filter subscriptions (for --filters reporting)
    pub fn get_filters(&self) -> Vec<TrafficFilter> {
        let guard = self.filters.lock().unwrap();
        guard.clone()
    }
}

impl Default for BroadcastEventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}

impl EventBus for BroadcastEventBus {
    fn publish(&self, event: TrafficEvent) -> Result<(), EventBusError> {
        let start = std::time::Instant::now();
        
        let row = TrafficRow::from_event(&event);
        
        // Always update diagnostics first (evt_cnt and recent snapshot)
        // This ensures --status / --events reporting works correctly
        self.evt_cnt
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if let Some(r) = row {
            if let Ok(mut guard) = self.recent.lock() {
                guard.push(r);
                while guard.len() > 500 {
                    guard.remove(0);
                }
            }
        }
        
        match self.sender.send(event) {
            Ok(receiver_count) => {
                let elapsed = start.elapsed();
                tracing::debug!(
                    "Published event to {} subscribers (latency: {:?})",
                    receiver_count,
                    elapsed
                );
                if receiver_count == 0 {
                    tracing::debug!("Published event but no subscribers currently connected");
                }
                Ok(())
            }
            Err(_) => {
                let elapsed = start.elapsed();
                tracing::warn!("Failed to publish event: broadcast channel closed (latency: {:?})", elapsed);
                Err(EventBusError::ChannelClosed)
            }
        }
    }

    fn subscribe(&self, filter: TrafficFilter) -> broadcast::Receiver<TrafficEvent> {
        let start = std::time::Instant::now();
        
        // Register the filter for --filters reporting.
        if let Ok(mut f) = self.filters.lock() {
            f.push(filter);
        }
        let rx = self.sender.subscribe();
        let elapsed = start.elapsed();
        tracing::debug!(
            "New subscriber registered; total subscribers = {} (setup latency: {:?})",
            self.sender.receiver_count(),
            elapsed
        );
        rx
    }

    fn subscriber_count(&self) -> usize {
        // Subtract 1 for the internal receiver that keeps the channel alive
        // This gives us the count of *external* subscribers only
        self.sender.receiver_count().saturating_sub(1)
    }
}

impl BroadcastEventBus {
    /// Convenience accessor: total number of events published.
    pub fn event_count(&self) -> u64 {
        self.get_event_count()
    }

    /// Convenience accessor: latest events snapshot for reporting.
    pub fn latest_events(&self, limit: usize) -> Vec<TrafficRow> {
        self.get_latest_events(limit)
    }
}

impl EventBusDiagnostics for BroadcastEventBus {
    fn event_count(&self) -> u64 {
        self.get_event_count()
    }

    fn latest_events(&self, limit: usize) -> Vec<TrafficRow> {
        self.get_latest_events(limit)
    }

    fn filter_list(&self) -> Vec<TrafficFilter> {
        self.get_filters()
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
        assert!(matches!(
            received,
            TrafficEvent::ConnectionEstablished { .. }
        ));
    }

    #[test]
    fn test_event_count_diagnostics() {
        let bus = BroadcastEventBus::new(16);
        assert_eq!(bus.event_count(), 0);
        
        // Need a subscriber before publish (tokio broadcast returns Err when rx_cnt == 0)
        let _rx = bus.subscribe(TrafficFilter::default());
        
        bus.publish(TrafficEvent::CaptureDropped {
            reason: "test".to_string(),
            count: 1,
        })
        .unwrap();
        assert_eq!(bus.event_count(), 1);
    }

    #[test]
    fn test_latest_events_snapshot() {
        let bus = BroadcastEventBus::new(16);
        let events = bus.latest_events(20);
        assert!(events.is_empty());
        
        // Need a subscriber before publish
        let _rx = bus.subscribe(TrafficFilter::default());
        
        for i in 0..5u64 {
            let ev = TrafficEvent::ConnectionEstablished {
                conn: crate::models::ConnectionKey {
                    src_addr: format!("127.0.0.1:{}", 10000 + i).parse().unwrap(),
                    dst_addr: "8.8.8.8:53".parse().unwrap(),
                    protocol: crate::models::TransportProtocol::Tcp,
                    connection_id: i,
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
            bus.publish(ev).unwrap();
        }
        let events = bus.latest_events(3);
        assert_eq!(events.len(), 3);
    }
}
