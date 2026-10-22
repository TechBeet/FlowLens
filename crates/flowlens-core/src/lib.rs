//! FlowLens Core - Domain models, event bus, and session management
//!
//! This crate provides the foundational types and abstractions used throughout
//! the FlowLens application.

pub mod errors;
pub mod event_bus;
pub mod models;
pub mod session;

pub use errors::CoreError;
pub use event_bus::{BroadcastEventBus, EventBus, EventBusError};
pub use models::*;
pub use session::SessionManager;
