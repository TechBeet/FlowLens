//! FlowLens Core - Domain models, event bus, and session management
//!
//! This crate provides the foundational types and abstractions used throughout
//! the FlowLens application.

pub mod errors;
pub mod event_bus;
pub mod filter;
pub mod intercept;
pub mod models;
pub mod rewrite;
pub mod session;
pub mod url_parser;

pub use errors::CoreError;
pub use event_bus::{BroadcastEventBus, EventBus, EventBusError};
pub use intercept::*;
pub use models::*;
pub use rewrite::*;
pub use session::SessionManager;
pub use url_parser::*;
