//! FlowLens Core - Domain models, event bus, and session management
//!
//! This crate provides the foundational types and abstractions used throughout
//! the FlowLens application.

pub mod models;
pub mod event_bus;
pub mod session;
pub mod errors;

pub use models::*;
pub use event_bus::{EventBus, EventBusError};
pub use session::{SessionManager};
pub use errors::{CoreError};
