//! MCP (Model Context Protocol) server for FlowLens
//!
//! This module provides a complete JSON-RPC 2.0 server implementation
//! that allows AI agents to query traffic data from FlowLens.

use std::net::SocketAddr;

pub mod json_rpc;
pub mod server;
pub mod tools;

pub use json_rpc::{ErrorCode, JsonRpcError, JsonRpcRequest, JsonRpcResponse};
pub use server::McpServerImpl;
pub use tools::{
    AggregateByHostTool, AggregateByPidTool, GetSessionTool, McpTool, QuerySessionsTool,
};

/// MCP server trait
pub trait McpServer: Send + Sync {
    fn start(&mut self, bind_addr: SocketAddr) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
}
