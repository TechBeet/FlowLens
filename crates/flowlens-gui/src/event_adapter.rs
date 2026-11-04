//! Event bus adapter for GUI.

use std::sync::Arc;
use flowlens_core::event_bus::EventBus;
use flowlens_core::models::TrafficEvent;

#[allow(dead_code)]
pub struct EventAdapter {
    bus: Arc<dyn EventBus>,
}

#[allow(dead_code)]
impl EventAdapter {
    pub fn new(bus: Arc<dyn EventBus>) -> Self {
        Self { bus }
    }

    pub fn get_bus(&self) -> Arc<dyn EventBus> {
        self.bus.clone()
    }

    pub fn process_events(&self, _events: &[TrafficEvent]) {
        // Placeholder for event processing
    }
}