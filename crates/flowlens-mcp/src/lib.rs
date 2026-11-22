//! MCP (Model Context Protocol) server for FlowLens

use std::net::SocketAddr;

pub mod json_rpc;
pub mod named_pipe;
pub mod server;
pub mod tools;

pub use json_rpc::{ErrorCode, JsonRpcError, JsonRpcRequest, JsonRpcResponse};
pub use named_pipe::NamedPipeServer;
pub use server::McpServerImpl;
pub use tools::{
    GetCaptureStatusTool, GetSessionStatsTool, GetTransactionTool, ListEndpointsTool,
    ListTransactionsTool, McpTool, SearchTransactionsTool, ExportAsCurlTool, DiagnoseCaptureTool,
};

/// MCP server trait
pub trait McpServer: Send + Sync {
    fn start(&mut self, bind_addr: SocketAddr) -> anyhow::Result<()>;
    fn stop(&mut self) -> anyhow::Result<()>;
}